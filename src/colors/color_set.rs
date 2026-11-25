use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, Event},
};

use super::{
    Cmyk, Color, ColorComponent, ColorId, MixedColor, MixedColorId, Rgb, SpotColor, SpotColorId,
    color::ColorParseReturn,
};

use crate::utils::{UnitF64, try_get_attr_raw};
use crate::{Collection, Entry, Error, OmapSection, Result};

/// The map's colors, in priority order.
///
/// A color's position in the set is its priority, which is what the `.omap`
/// format stores. A [`ColorId`] keeps naming the same color across
/// [`Collection::swap`], [`Collection::insert`] and [`Collection::remove`];
/// see [`Collection`] for everything a set does.
///
/// A [`ColorId`] left over from a removed color contributes no color when the
/// map is written.
pub type ColorSet = Collection<ColorId, Color>;

impl ColorSet {
    /// Get a spot color by its handle. Cannot return a mixed color.
    pub fn spot_color(&self, id: SpotColorId) -> Option<&SpotColor> {
        match self.get(id)?.value() {
            Color::SpotColor(color) => Some(color),
            Color::MixedColor(_) => None,
        }
    }

    /// Mutably get a spot color by its handle.
    pub fn spot_color_mut(&mut self, id: SpotColorId) -> Option<&mut SpotColor> {
        match self.get_mut(id)? {
            Color::SpotColor(color) => Some(color),
            Color::MixedColor(_) => None,
        }
    }

    /// Get a mixed color by its handle. Cannot return a spot color.
    pub fn mixed_color(&self, id: MixedColorId) -> Option<&MixedColor> {
        match self.get(id)?.value() {
            Color::MixedColor(color) => Some(color),
            Color::SpotColor(_) => None,
        }
    }

    /// Mutably get a mixed color by its handle.
    pub fn mixed_color_mut(&mut self, id: MixedColorId) -> Option<&mut MixedColor> {
        match self.get_mut(id)? {
            Color::MixedColor(color) => Some(color),
            Color::SpotColor(_) => None,
        }
    }

    /// Find a color by exact name match. The first match is returned.
    pub fn find_by_name(&self, name: &str) -> Option<Entry<'_, ColorId, Color>> {
        self.iter().find(|color| color.name() == name)
    }

    /// Get the effective CMYK value of a color in this set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `id` is not in this set or the color
    /// definition cannot produce a CMYK value.
    pub fn cmyk(&self, id: ColorId) -> Result<Cmyk> {
        self.get(id).ok_or(Error::ColorError)?.cmyk(self)
    }

    /// Get the effective RGB value of a color in this set.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `id` is not in this set or the color
    /// definition cannot produce an RGB value.
    pub fn rgb(&self, id: ColorId) -> Result<Rgb> {
        self.get(id).ok_or(Error::ColorError)?.rgb(self)
    }
}

/// Narrowing a looked-up color to a typed handle.
///
/// ```
/// # use omap::Omap;
/// # fn example(map: &Omap) {
/// let purple = map.colors.find_by_name("Purple").and_then(|color| color.as_spot());
/// # }
/// ```
impl Entry<'_, ColorId, Color> {
    /// Narrow to a spot color handle, or `None` if this names a mixed color.
    pub fn as_spot(self) -> Option<SpotColorId> {
        SpotColorId::try_from(self).ok()
    }

    /// Narrow to a mixed color handle, or `None` if this names a spot color.
    pub fn as_mixed(self) -> Option<MixedColorId> {
        MixedColorId::try_from(self).ok()
    }
}

impl ColorSet {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
    ) -> Result<Self> {
        let num_colors = try_get_attr_raw(element, "count")?.ok_or(Error::ColorError)?;
        let mut colors_and_components = Vec::with_capacity(num_colors);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "color") {
                        colors_and_components.push(Color::parse(reader, &bytes_start)?);
                    }
                }
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "colors") {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::ColorSet));
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

        let mut color_set = Self::with_capacity(pending.len());
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

    pub(crate) fn write<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        writer.write_event(Event::Start(
            BytesStart::new("colors").with_attributes([("count", self.len().to_string().as_str())]),
        ))?;
        writer.get_mut().write_all(b"\n".as_slice())?;
        for (priority, color) in self.values().enumerate() {
            match color {
                Color::SpotColor(color) => color.write(writer, priority)?,
                Color::MixedColor(color) => color.write(writer, priority, self)?,
            }
            writer.get_mut().write_all(b"\n".as_slice())?;
        }
        writer.write_event(Event::End(BytesEnd::new("colors")))?;
        Ok(())
    }
}
