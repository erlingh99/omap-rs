//! The `<colors>` section: the color set in priority order.

use super::read_text;
use crate::NonNegativeF64;
use crate::colors::{
    Cmyk, CmykMode, Color, ColorComponent, ColorSet, MixedColor, Rgb, RgbMode, SpotColor,
    SymbolColor,
};
use crate::utils::UnitF64;
use crate::xml::attrs::{parse_attr, parse_attr_raw, try_get_attr_raw};
use crate::xml::{Error, Result, Section};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

pub(in crate::xml) fn read_color_set<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
) -> Result<ColorSet> {
    let num_colors = try_get_attr_raw(element, "count")?.ok_or(Error::ColorError)?;
    let mut colors_and_components = Vec::with_capacity(num_colors);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(bytes_start) => {
                if matches!(bytes_start.local_name().as_ref(), "color") {
                    colors_and_components.push(read_color(reader, &bytes_start)?);
                }
            }
            Event::End(bytes_end) => {
                if matches!(bytes_end.local_name().as_ref(), "colors") {
                    break;
                }
            }
            Event::Eof => {
                return Err(Error::UnexpectedEof(Section::ColorSet));
            }
            _ => (),
        }
    }

    // Colors are pushed in priority order first so that a mixed color's
    // components can be resolved by priority afterwards.
    let mut pending = Vec::with_capacity(colors_and_components.len());
    for color_parse_return in colors_and_components {
        match color_parse_return {
            ColorParseReturn::Spot { color, priority } => {
                pending.push((priority, Color::SpotColor(color), Vec::new()));
            }
            ColorParseReturn::Mix {
                color,
                priority,
                components,
            } => pending.push((priority, Color::MixedColor(color), components)),
        }
    }
    pending.sort_by_key(|(priority, _, _)| *priority);

    let mut color_set = ColorSet::with_capacity(pending.len());
    let mut all_components = Vec::with_capacity(pending.len());
    for (_, color, components) in pending {
        let _id = color_set.push(color);
        all_components.push(components);
    }

    for (position, components) in all_components.into_iter().enumerate() {
        if components.is_empty() {
            continue;
        }
        let resolved: Vec<ColorComponent> = components
            .into_iter()
            .filter_map(|(priority, factor)| {
                let priority = usize::try_from(priority).ok()?;
                let color = color_set.at(priority)?.as_spot()?;
                Some(ColorComponent {
                    factor: UnitF64::clamped_from(factor),
                    color,
                })
            })
            .collect();
        if let Some(Color::MixedColor(color)) = color_set.at_mut(position) {
            color.components = resolved;
        }
    }

    Ok(color_set)
}

pub(in crate::xml) fn write_color_set<W: std::io::Write>(
    colors: &ColorSet,
    writer: &mut Writer<W>,
) -> Result<()> {
    writer.write_event(Event::Start(
        BytesStart::new("colors").with_attributes([("count", colors.len().to_string().as_str())]),
    ))?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    for (priority, color) in colors.values().enumerate() {
        match color {
            Color::SpotColor(color) => write_spot_color(color, writer, priority)?,
            Color::MixedColor(color) => write_mixed_color(color, writer, priority, colors)?,
        }
        writer.get_mut().write_all(b"\n".as_slice())?;
    }
    writer.write_event(Event::End(BytesEnd::new("colors")))?;
    Ok(())
}
enum ColorParseReturn {
    Spot {
        color: SpotColor,
        priority: usize,
    },
    Mix {
        color: MixedColor,
        priority: usize,
        components: Vec<(i32, f64)>,
    },
}

/// Parsing return both the parsed color (or almost parsed color) and the spotcolor references which might not be parseable yet and must wait
#[expect(
    clippy::too_many_lines,
    reason = "color parsing follows the nested OMAP XML structure"
)]
fn read_color<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    element: &BytesStart<'_>,
) -> Result<ColorParseReturn> {
    let mut name = String::new();
    let mut cmyk = Cmyk::default();
    let mut id = usize::MAX;

    for attr in element.attributes().filter_map(std::result::Result::ok) {
        match attr.key.local_name().as_ref() {
            "name" => {
                if let Ok(n) = parse_attr::<String>(attr) {
                    name.push_str(&n);
                }
            }
            "c" => {
                cmyk.c = UnitF64::clamped_from(
                    parse_attr_raw(attr.value).unwrap_or_else(|_| cmyk.c.get()),
                );
            }
            "m" => {
                cmyk.m = UnitF64::clamped_from(
                    parse_attr_raw(attr.value).unwrap_or_else(|_| cmyk.m.get()),
                );
            }
            "y" => {
                cmyk.y = UnitF64::clamped_from(
                    parse_attr_raw(attr.value).unwrap_or_else(|_| cmyk.y.get()),
                );
            }
            "k" => {
                cmyk.k = UnitF64::clamped_from(
                    parse_attr_raw(attr.value).unwrap_or_else(|_| cmyk.k.get()),
                );
            }
            "priority" => id = parse_attr_raw(attr.value).unwrap_or(id),
            _ => (),
        }
    }

    let mut is_spotcolor = false;
    let mut cmyk_mode = CmykMode::Cmyk(cmyk);
    let mut rgb_mode = RgbMode::FromCmyk;

    let mut spot_angle = 0.;
    let mut spot_frequency = 0.;
    let mut spotcolor_name = String::new();
    let mut spotcolor_components = Vec::new();
    let mut knockout = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(bytes_start) => match bytes_start.local_name().as_ref() {
                "cmyk" => {
                    if let Some(mode) = bytes_start
                        .try_get_attribute("method")
                        .ok()
                        .flatten()
                        .and_then(|s| match s.value.as_ref() {
                            "custom" => Some(CmykMode::Cmyk(cmyk)),
                            "spotcolor" => Some(CmykMode::FromSpotColors),
                            "rgb" => Some(CmykMode::FromRgb),
                            _ => None,
                        })
                    {
                        cmyk_mode = mode;
                    }
                }
                "rgb" => {
                    if let Some(mode) = bytes_start
                        .try_get_attribute("method")?
                        .map(|s| -> Result<Option<RgbMode>> {
                            match s.value.as_ref() {
                                "custom" => {
                                    let r = UnitF64::clamped_from(
                                        try_get_attr_raw(&bytes_start, "r")?.unwrap_or(0.),
                                    );
                                    let g = UnitF64::clamped_from(
                                        try_get_attr_raw(&bytes_start, "g")?.unwrap_or(0.),
                                    );
                                    let b = UnitF64::clamped_from(
                                        try_get_attr_raw(&bytes_start, "b")?.unwrap_or(0.),
                                    );
                                    Ok(Some(RgbMode::Rgb(Rgb { r, g, b })))
                                }
                                "spotcolor" => Ok(Some(RgbMode::FromSpotColors)),
                                "cmyk" => Ok(Some(RgbMode::FromCmyk)),
                                _ => Ok(None),
                            }
                        })
                        .transpose()?
                        .flatten()
                    {
                        rgb_mode = mode;
                    }
                }
                "spotcolors" => {
                    knockout = try_get_attr_raw(&bytes_start, "knockout")
                        .ok()
                        .flatten()
                        .unwrap_or(false);

                    loop {
                        match reader.read_event_into(&mut buf)? {
                            Event::Start(bytes_start) => {
                                // if the next event is called namedcolor we've got a new spotcolor
                                match bytes_start.local_name().as_ref() {
                                    "namedcolor" => {
                                        is_spotcolor = true;
                                        spot_angle = try_get_attr_raw(&bytes_start, "screen_angle")
                                            .ok()
                                            .flatten()
                                            .unwrap_or(0.);
                                        spot_frequency =
                                            try_get_attr_raw(&bytes_start, "screen_frequency")
                                                .ok()
                                                .flatten()
                                                .unwrap_or(0.);
                                        spotcolor_name = read_text(reader)?;
                                    }
                                    // Components may reference colors not yet read, so
                                    // they are resolved once the whole set is parsed.
                                    "component" => {
                                        let factor =
                                            try_get_attr_raw(&bytes_start, "factor")?.unwrap_or(0.);

                                        let spotcolor_id =
                                            try_get_attr_raw(&bytes_start, "spotcolor")?
                                                .unwrap_or(-1);
                                        spotcolor_components.push((spotcolor_id, factor));
                                    }
                                    _ => (),
                                }
                            }
                            Event::End(bytes_end)
                                if bytes_end.local_name().as_ref() == "spotcolors" =>
                            {
                                break;
                            }
                            Event::Eof => {
                                return Err(Error::UnexpectedEof(Section::Color));
                            }
                            _ => (),
                        }
                    }
                }
                _ => (),
            },
            Event::End(bytes_end) if bytes_end.local_name().as_ref() == "color" => {
                break;
            }
            Event::Eof => return Err(Error::UnexpectedEof(Section::Color)),
            _ => (),
        }
    }

    if id == usize::MAX {
        return Err(Error::MissingColorId);
    }

    if is_spotcolor {
        // fix possible bad color definition modes
        if cmyk_mode == CmykMode::FromSpotColors {
            cmyk_mode = CmykMode::Cmyk(cmyk);
        }
        if rgb_mode == RgbMode::FromSpotColors {
            rgb_mode = RgbMode::FromCmyk;
        }
        if rgb_mode == RgbMode::FromCmyk && cmyk_mode == CmykMode::FromRgb {
            cmyk_mode = CmykMode::Cmyk(cmyk);
        }

        Ok(ColorParseReturn::Spot {
            color: {
                let mut color = SpotColor::new(name, spotcolor_name, cmyk);
                color.cmyk_mode = cmyk_mode;
                color.rgb_mode = rgb_mode;
                color.knockout = knockout;
                color.screen_frequency = NonNegativeF64::clamped_from(spot_frequency);
                color.screen_angle_deg = spot_angle;
                color
            },
            priority: id,
        })
    } else {
        // fix possible bad color definition modes
        if cmyk_mode == CmykMode::FromSpotColors && spotcolor_components.is_empty() {
            cmyk_mode = CmykMode::Cmyk(cmyk);
        }
        if rgb_mode == RgbMode::FromSpotColors && spotcolor_components.is_empty() {
            rgb_mode = RgbMode::FromCmyk;
        }
        if rgb_mode == RgbMode::FromCmyk && cmyk_mode == CmykMode::FromRgb {
            cmyk_mode = CmykMode::Cmyk(cmyk);
        }
        Ok(ColorParseReturn::Mix {
            color: {
                let mut color = MixedColor::new(name, Vec::new());
                color.cmyk_mode = cmyk_mode;
                color.rgb_mode = rgb_mode;
                color.knockout = knockout;
                color
            },
            priority: id,
            components: spotcolor_components,
        })
    }
}
fn write_spot_color<W: std::io::Write>(
    color: &SpotColor,
    writer: &mut Writer<W>,
    priority: usize,
) -> Result<()> {
    let cmyk = color.cmyk()?;
    writer.write_event(Event::Start(BytesStart::new("color").with_attributes([
        ("priority", priority.to_string().as_str()),
        ("name", color.name()),
        ("c", format!("{:.3}", cmyk.c.get()).as_str()),
        ("m", format!("{:.3}", cmyk.m.get()).as_str()),
        ("y", format!("{:.3}", cmyk.y.get()).as_str()),
        ("k", format!("{:.3}", cmyk.k.get()).as_str()),
        ("opacity", "1"),
    ])))?;
    writer
        .write_event(Event::Start(BytesStart::new("spotcolors").with_attributes(
            [("knockout", color.knockout.to_string().as_str())],
        )))?;
    writer.write_event(Event::Start(BytesStart::new("namedcolor").with_attributes(
        [
            (
                "screen_angle",
                format!("{:.1}", color.screen_angle_deg).as_str(),
            ),
            (
                "screen_frequency",
                format!("{:.1}", color.screen_frequency.get()).as_str(),
            ),
        ],
    )))?;
    writer.write_event(Event::Text(BytesText::new(&color.spotcolor_name)))?;
    writer.write_event(Event::End(BytesEnd::new("namedcolor")))?;
    writer.write_event(Event::End(BytesEnd::new("spotcolors")))?;
    write_cmyk_mode(color.cmyk_mode(), writer)?;
    write_rgb_mode(color.rgb_mode(), writer)?;
    writer.write_event(Event::End(BytesEnd::new("color")))?;
    Ok(())
}
fn write_mixed_color<W: std::io::Write>(
    color: &MixedColor,
    writer: &mut Writer<W>,
    priority: usize,
    color_set: &ColorSet,
) -> Result<()> {
    let cmyk = color.cmyk(color_set)?;

    writer.write_event(Event::Start(BytesStart::new("color").with_attributes([
        ("priority", priority.to_string().as_str()),
        ("name", color.name()),
        ("c", format!("{:.3}", cmyk.c.get()).as_str()),
        ("m", format!("{:.3}", cmyk.m.get()).as_str()),
        ("y", format!("{:.3}", cmyk.y.get()).as_str()),
        ("k", format!("{:.3}", cmyk.k.get()).as_str()),
        ("opacity", "1"),
    ])))?;
    writer
        .write_event(Event::Start(BytesStart::new("spotcolors").with_attributes(
            [("knockout", color.knockout.to_string().as_str())],
        )))?;

    for component in &color.components {
        if let Some(priority) = color_set.position(component.color) {
            writer.write_event(Event::Empty(BytesStart::new("component").with_attributes(
                [
                    ("factor", format!("{:.3}", component.factor.get()).as_str()),
                    ("spotcolor", priority.to_string().as_str()),
                ],
            )))?;
        }
    }
    writer.write_event(Event::End(BytesEnd::new("spotcolors")))?;
    write_cmyk_mode(color.cmyk_mode(), writer)?;
    write_rgb_mode(color.rgb_mode(), writer)?;
    writer.write_event(Event::End(BytesEnd::new("color")))?;
    Ok(())
}
fn write_cmyk_mode<W: std::io::Write>(mode: CmykMode, writer: &mut Writer<W>) -> Result<()> {
    let string = match mode {
        CmykMode::FromSpotColors => "spotcolor",
        CmykMode::FromRgb => "rgb",
        CmykMode::Cmyk(_) => "custom",
    };
    writer.write_event(Event::Empty(
        BytesStart::new("cmyk").with_attributes([("method", string)]),
    ))?;
    Ok(())
}

fn write_rgb_mode<W: std::io::Write>(mode: RgbMode, writer: &mut Writer<W>) -> Result<()> {
    let bs = BytesStart::new("rgb");
    let bs = match mode {
        RgbMode::FromSpotColors => bs.with_attributes([("method", "spotcolor")]),
        RgbMode::FromCmyk => bs.with_attributes([("method", "cmyk")]),
        RgbMode::Rgb(rgb) => bs.with_attributes([
            ("method", "custom"),
            ("r", format!("{:.3}", rgb.r.get()).as_str()),
            ("g", format!("{:.3}", rgb.g.get()).as_str()),
            ("b", format!("{:.3}", rgb.b.get()).as_str()),
        ]),
    };
    writer.write_event(Event::Empty(bs))?;
    Ok(())
}

impl SymbolColor {
    /// Create a `SymbolColor` from a file color index.
    /// -1 or missing => `NoColor`, -900 => `RegistrationBlack`, >= 0 => Color lookup.
    pub(in crate::xml) fn from_index(index: i32, color_set: &ColorSet) -> Self {
        match index {
            -900 => Self::RegistrationBlack,
            i if i >= 0 => match color_set.at(i as usize) {
                Some(color) => Self::Color(color.id()),
                None => Self::NoColor,
            },
            _ => Self::NoColor,
        }
    }

    /// Get the priority index of this color in the color set.
    /// Returns -1 for `NoColor`, -900 for `RegistrationBlack`.
    pub(in crate::xml) fn priority(&self, color_set: &ColorSet) -> i32 {
        match self {
            Self::Color(id) => color_set.position(*id).map_or(-1, |p| p as i32),
            Self::RegistrationBlack => -900,
            Self::NoColor => -1,
        }
    }
}
