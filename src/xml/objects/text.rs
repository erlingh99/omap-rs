//! Text objects.

use crate::NonNegativeF64;
use crate::objects::{HorizontalAlign, TextGeometry, TextObject, VerticalAlign, WrapBox};
use crate::symbols::{SymbolSet, TextSymbolId};
use crate::xml::attrs::try_get_attr_raw;
use crate::xml::units::{from_file_coords, to_file_coords};
use crate::xml::{CoordinateComponent, Error, Result, Section};
use geo_types::Coord;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::str::FromStr;

impl FromStr for HorizontalAlign {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Left),
            "1" => Ok(Self::HCenter),
            "2" => Ok(Self::Right),
            _ => Err(Error::ObjectError),
        }
    }
}

impl FromStr for VerticalAlign {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Baseline),
            "1" => Ok(Self::Top),
            "2" => Ok(Self::VCenter),
            "3" => Ok(Self::Bottom),
            _ => Err(Error::ObjectError),
        }
    }
}

impl TextObject {
    pub(in crate::xml) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
    ) -> Result<()> {
        let is_rotatable = self
            .symbol
            .and_then(|id| symbol_set.text_symbol(id))
            .is_some_and(|symbol| symbol.is_rotatable);
        let index = symbol_set.file_index(self.symbol);

        let mut bs = BytesStart::new("object").with_attributes([
            ("type", "4"),
            ("symbol", index.to_string().as_str()),
            ("h_align", (self.h_align as u8).to_string().as_str()),
            ("v_align", (self.v_align as u8).to_string().as_str()),
        ]);

        if self.rotation.abs() > f64::EPSILON && is_rotatable {
            // Map the rotation onto [-PI, PI].
            let rot = (self.rotation + self.rotation.signum() * std::f64::consts::PI)
                % std::f64::consts::TAU
                - self.rotation.signum() * std::f64::consts::PI;
            bs.push_attribute(("rotation", rot.to_string().as_str()));
        }
        writer.write_event(Event::Start(bs))?;
        if !self.tags().is_empty() {
            crate::xml::objects::write_tags(writer, self.tags())?;
        }

        match self.geometry() {
            TextGeometry::SingleAnchor(coord) => {
                writer.write_event(Event::Start(
                    BytesStart::new("coords").with_attributes([("count", "1")]),
                ))?;
                let fc = to_file_coords(*coord)?;
                writer.write_event(Event::Text(BytesText::new(&format!("{} {};", fc.x, fc.y))))?;
                writer.write_event(Event::End(BytesEnd::new("coords")))?;
            }
            TextGeometry::WrapBox(wb) => {
                writer.write_event(Event::Start(
                    BytesStart::new("coords").with_attributes([("count", "2")]),
                ))?;
                let fc = to_file_coords(wb.anchor)?;
                let width = wb.width.to_file_value()?;
                let height = wb.height.to_file_value()?;
                writer.write_event(Event::Text(BytesText::new(&format!(
                    "{} {};{} {};",
                    fc.x, fc.y, width, height
                ))))?;
                writer.write_event(Event::End(BytesEnd::new("coords")))?;
                writer.write_event(Event::Empty(BytesStart::new("size").with_attributes([
                    ("width", width.to_string().as_str()),
                    ("height", height.to_string().as_str()),
                ])))?;
            }
        }
        writer.write_event(Event::Start(BytesStart::new("text")))?;
        writer.write_event(Event::Text(BytesText::new(&self.text)))?;
        writer.write_event(Event::End(BytesEnd::new("text")))?;
        writer.write_event(Event::End(BytesEnd::new("object")))?;
        Ok(())
    }

    /// Parse a text object. The reader should be positioned right after
    /// the `<object>` start event. Reads through `</object>`.
    pub(in crate::xml) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbol: Option<TextSymbolId>,
        h_align: HorizontalAlign,
        v_align: VerticalAlign,
        rotation: f64,
    ) -> Result<Self> {
        let mut text_geo = TextGeometry::SingleAnchor(Coord::default());
        let mut tags = HashMap::new();
        let mut text = String::new();
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    match bytes_start.local_name().as_ref() {
                        "tags" => tags = crate::xml::objects::parse_tags(reader)?,
                        "size" => {
                            // Override box size from <size> element (takes precedence)
                            let w = try_get_attr_raw(&bytes_start, "width")?.unwrap_or(0);
                            let h = try_get_attr_raw(&bytes_start, "height")?.unwrap_or(0);
                            if let TextGeometry::WrapBox(wb) = &mut text_geo {
                                wb.width = NonNegativeF64::from_file_value(w);
                                wb.height = NonNegativeF64::from_file_value(h);
                            }
                        }
                        "coords" => match try_get_attr_raw::<u8>(&bytes_start, "count")? {
                            Some(1) => text_geo = TextGeometry::SingleAnchor(Coord::default()),
                            Some(2) => text_geo = TextGeometry::WrapBox(WrapBox::default()),
                            _ => return Err(Error::ObjectError),
                        },
                        "text" => text = crate::xml::read_text(reader)?,
                        _ => (),
                    }
                }
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "object") {
                        break;
                    }
                }
                Event::Text(bytes_text) => {
                    let raw_xml = bytes_text.as_ref();

                    if let Some((coords_str, opt_wh)) = raw_xml.split_once(';') {
                        let mut split = coords_str.split_whitespace();

                        let x: i32 = split
                            .next()
                            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::X))?
                            .parse()?;
                        let y: i32 = split
                            .next()
                            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::Y))?
                            .parse()?;

                        let coord = from_file_coords(Coord { x, y });

                        let box_size = if !opt_wh.is_empty() {
                            // opt_wh might be "w h;" or "w h;rest..."
                            if let Some(wh_str) = opt_wh.split(';').next() {
                                let mut wh_split = wh_str.split_whitespace();
                                let w = wh_split.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                                let h = wh_split.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                                Some((
                                    NonNegativeF64::from_file_value(w),
                                    NonNegativeF64::from_file_value(h),
                                ))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        match &mut text_geo {
                            TextGeometry::SingleAnchor(point) => *point = coord,
                            TextGeometry::WrapBox(wrap_box) => {
                                wrap_box.anchor = coord;
                                if let Some((w, h)) = box_size {
                                    wrap_box.width = w;
                                    wrap_box.height = h;
                                }
                            }
                        }
                    } else {
                        return Err(Error::MissingTextObjectCoordinates);
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::TextObject));
                }
                _ => (),
            }
        }
        let mut object = Self::new(symbol, text_geo, text).with_tags(tags);
        object.h_align = h_align;
        object.v_align = v_align;
        object.rotation = rotation;
        Ok(object)
    }
}
