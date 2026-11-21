//! Area symbols and their fill patterns.

use crate::colors::{ColorSet, SymbolColor};
use crate::symbols::{
    AreaSymbol, ClippingOption, FillPattern, PointSymbol, SymbolCommon, SymbolKind,
};
use crate::xml::attrs::{parse_attr, try_get_attr_raw};
use crate::xml::symbols::SymbolPosition;
use crate::{Error, NonNegativeF64, OmapSection, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::str::FromStr;

impl FromStr for ClippingOption {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "0" => Ok(Self::ClipElementsAtBoundary),
            "1" => Ok(Self::NoClippingIfCompletelyInside),
            "2" => Ok(Self::NoClippingIfCenterInside),
            "3" => Ok(Self::NoClippingIfPartiallyInside),
            _ => Err(Error::UnknownClippingOption),
        }
    }
}

impl FillPattern {
    fn parse<R: std::io::BufRead>(
        element: &BytesStart<'_>,
        reader: &mut Reader<R>,
        color_set: &ColorSet,
    ) -> Result<Self> {
        let pattern_type = try_get_attr_raw(element, "type")?.unwrap_or(0);
        let angle = try_get_attr_raw(element, "angle")?.unwrap_or(0.0);
        let clip_options = try_get_attr_raw(element, "no_clipping")?.unwrap_or_default();
        let rotatable = try_get_attr_raw(element, "rotatable")?.unwrap_or(false);
        let line_spacing = NonNegativeF64::from_file_value(
            try_get_attr_raw(element, "line_spacing")?.unwrap_or(0),
        );
        let line_offset =
            NonNegativeF64::from_file_value(try_get_attr_raw(element, "line_offset")?.unwrap_or(0));
        let offset_along_line = NonNegativeF64::from_file_value(
            try_get_attr_raw(element, "offset_along_line")?.unwrap_or(0),
        );

        match pattern_type {
            1 => {
                // LinePattern
                let ci = try_get_attr_raw(element, "color")?.unwrap_or(-1);
                let line_color = SymbolColor::from_index(ci, color_set);
                let line_width = NonNegativeF64::from_file_value(
                    try_get_attr_raw(element, "line_width")?.unwrap_or(0),
                );
                let mut buf = Vec::new();
                loop {
                    match reader.read_event_into(&mut buf)? {
                        Event::End(e) if e.local_name().as_ref() == "pattern" => {
                            break;
                        }
                        Event::Eof => {
                            return Err(Error::UnexpectedEof(OmapSection::FillPattern));
                        }
                        _ => {}
                    }
                }
                Ok(Self::LinePattern {
                    angle,
                    line_spacing,
                    line_offset,
                    line_color,
                    line_width,
                    rotatable,
                })
            }
            2 => {
                // PointPattern
                let point_distance = NonNegativeF64::from_file_value(
                    try_get_attr_raw(element, "point_distance")?.unwrap_or(0),
                );
                let mut point = None;
                let mut buf = Vec::new();
                loop {
                    match reader.read_event_into(&mut buf)? {
                        Event::Start(e) if e.local_name().as_ref() == "symbol" => {
                            let mut sub_common = SymbolCommon::default();
                            for attr in e.attributes().filter_map(std::result::Result::ok) {
                                match attr.key.local_name().as_ref() {
                                    "name" => {
                                        sub_common.name =
                                            parse_attr(attr).unwrap_or(sub_common.name);
                                    }
                                    "code" => {
                                        sub_common.code =
                                            crate::xml::attrs::parse_attr_raw(attr.value)
                                                .unwrap_or_default();
                                    }
                                    _ => {}
                                }
                            }
                            point = Some(PointSymbol::parse(reader, color_set, sub_common)?);
                        }
                        Event::End(e) if e.local_name().as_ref() == "pattern" => {
                            break;
                        }
                        Event::Eof => {
                            return Err(Error::UnexpectedEof(OmapSection::FillPatternPoint));
                        }
                        _ => {}
                    }
                }
                let point = point.ok_or(Error::MissingPointPatternSymbol)?;
                Ok(Self::PointPattern {
                    clip_options,
                    angle,
                    line_spacing,
                    line_offset,
                    offset_along_line,
                    point_distance,
                    point,
                    rotatable,
                })
            }
            _ => Err(Error::UnknownFillPatternType(pattern_type)),
        }
    }

    fn write<W: std::io::Write>(&self, writer: &mut Writer<W>, color_set: &ColorSet) -> Result<()> {
        match self {
            Self::LinePattern {
                angle,
                line_spacing,
                line_offset,
                line_color,
                line_width,
                rotatable,
            } => {
                let mut bs = BytesStart::new("pattern")
                    .with_attributes([("type", "1"), ("angle", angle.to_string().as_str())]);
                if *rotatable {
                    bs.push_attribute(("rotatable", "true"));
                }
                bs.push_attribute((
                    "line_spacing",
                    line_spacing.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "line_offset",
                    line_offset.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute(("offset_along_line", "0"));
                bs.push_attribute(("color", line_color.priority(color_set).to_string().as_str()));
                bs.push_attribute((
                    "line_width",
                    line_width.to_file_value()?.to_string().as_str(),
                ));
                writer.write_event(Event::Empty(bs))?;
            }
            Self::PointPattern {
                clip_options,
                angle,
                line_spacing,
                line_offset,
                offset_along_line,
                point_distance,
                point,
                rotatable,
            } => {
                let mut bs = BytesStart::new("pattern")
                    .with_attributes([("type", "2"), ("angle", angle.to_string().as_str())]);
                if *clip_options as u8 > 0 {
                    bs.push_attribute(("no_clipping", (*clip_options as u8).to_string().as_str()));
                }
                if *rotatable {
                    bs.push_attribute(("rotatable", "true"));
                }
                bs.push_attribute((
                    "line_spacing",
                    line_spacing.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "line_offset",
                    line_offset.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "offset_along_line",
                    offset_along_line.to_file_value()?.to_string().as_str(),
                ));
                bs.push_attribute((
                    "point_distance",
                    point_distance.to_file_value()?.to_string().as_str(),
                ));
                writer.write_event(Event::Start(bs))?;
                point.write(writer, color_set, SymbolPosition::Element)?;
                writer.write_event(Event::End(BytesEnd::new("pattern")))?;
            }
        }
        Ok(())
    }
}

impl AreaSymbol {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        color_set: &ColorSet,
        attributes: SymbolCommon,
    ) -> Result<Self> {
        let mut common = attributes;
        let mut color = SymbolColor::NoColor;
        let mut minimum_area = NonNegativeF64::default();
        let mut is_rotatable = false;
        let mut patterns = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    "description" => common.description = crate::xml::read_text(reader)?,
                    "area_symbol" => {
                        let ci = try_get_attr_raw(&e, "inner_color")?.unwrap_or(-1);
                        color = SymbolColor::from_index(ci, color_set);
                        minimum_area = NonNegativeF64::from_file_value(
                            try_get_attr_raw(&e, "min_area")?.unwrap_or(0),
                        );
                        is_rotatable = try_get_attr_raw(&e, "rotatable")?.unwrap_or(false);
                    }
                    "pattern" => {
                        patterns.push(FillPattern::parse(&e, reader, color_set)?);
                    }
                    "icon" => {
                        common.custom_icon = try_get_attr_raw(&e, "src")?;
                    }
                    _ => {}
                },
                Event::End(e) if e.local_name().as_ref() == "symbol" => {
                    break;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::AreaSymbol));
                }
                _ => {}
            }
        }

        Ok(Self {
            common,
            is_rotatable,
            color,
            patterns,
            minimum_area,
        })
    }

    /// Write this symbol on its own, for the sub-symbol positions that
    /// [`Symbol::write`] does not reach: private parts of a combined symbol,
    /// and point-symbol elements.
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
        position: SymbolPosition,
    ) -> Result<()> {
        self.common
            .write_open(writer, SymbolKind::Area.type_id(), position)?;
        self.write_body(writer, color_set)?;
        self.common.write_close(writer)
    }

    /// Write the type-specific body, between the halves of the shared
    /// `<symbol>` frame written by [`Symbol::write`].
    pub(crate) fn write_body<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        color_set: &ColorSet,
    ) -> Result<()> {
        let mut bs = BytesStart::new("area_symbol");
        bs.push_attribute((
            "inner_color",
            self.color.priority(color_set).to_string().as_str(),
        ));
        bs.push_attribute((
            "min_area",
            self.minimum_area.to_file_value()?.to_string().as_str(),
        ));
        if self.is_rotatable {
            bs.push_attribute(("rotatable", "true"));
        }
        bs.push_attribute(("patterns", self.patterns.len().to_string().as_str()));
        writer.write_event(Event::Start(bs))?;

        for pattern in &self.patterns {
            pattern.write(writer, color_set)?;
        }

        writer.write_event(Event::End(BytesEnd::new("area_symbol")))?;

        Ok(())
    }
}
