//! Point objects.

use crate::objects::PointObject;
use crate::symbols::{PointSymbolId, SymbolSet};
use crate::xml::units::{from_file_coords, to_file_coords};
use crate::xml::{CoordinateComponent, Error, ObjectKind, Result, Section};
use geo_types::{Coord, Point};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;

impl PointObject {
    pub(in crate::xml) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
    ) -> Result<()> {
        let is_rotatable = self
            .symbol
            .and_then(|id| symbol_set.point_symbol(id))
            .is_some_and(|symbol| symbol.is_rotatable);
        let index = symbol_set.file_index(self.symbol);

        self.write_content(writer, Some(index), is_rotatable)?;
        Ok(())
    }

    /// Write a full `<object>...</object>` element - used for point symbol elements
    pub(in crate::xml) fn write_as_element<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        is_rotatable: bool,
    ) -> Result<()> {
        self.write_content(writer, None, is_rotatable)?;
        Ok(())
    }

    fn write_content<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_index: Option<i32>,
        is_rotatable: bool,
    ) -> Result<()> {
        let mut bs = BytesStart::new("object").with_attributes([("type", "0")]);
        if let Some(idx) = symbol_index {
            bs.push_attribute(("symbol", idx.to_string().as_str()));
        }

        if self.rotation.abs() > f64::EPSILON && is_rotatable {
            // Map the rotation onto [-PI, PI].
            let rot = (self.rotation + self.rotation.signum() * std::f64::consts::PI)
                % std::f64::consts::TAU
                - self.rotation.signum() * std::f64::consts::PI;
            bs.push_attribute(("rotation", rot.to_string().as_str()));
        }
        writer.write_event(Event::Start(bs))?;
        // elements are not allowed to have tags
        if !self.tags().is_empty() && symbol_index.is_some() {
            crate::xml::objects::write_tags(writer, self.tags())?;
        }
        let file_coord = to_file_coords(self.geometry().0)?;
        writer.write_event(Event::Start(
            BytesStart::new("coords").with_attributes([("count", "1")]),
        ))?;
        writer.write_event(Event::Text(BytesText::new(&format!(
            "{} {};",
            file_coord.x, file_coord.y
        ))))?;
        writer.write_event(Event::End(BytesEnd::new("coords")))?;
        writer.write_event(Event::End(BytesEnd::new("object")))?;
        Ok(())
    }

    /// Parse a point object. The reader should be positioned right after
    /// the `<coords>` start event. Reads through `</object>`.
    pub(in crate::xml) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbol: Option<PointSymbolId>,
        rotation: f64,
    ) -> Result<Self> {
        let mut tags = HashMap::new();
        let mut point = None;
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "object") {
                        break;
                    }
                }
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "tags") {
                        tags = crate::xml::objects::parse_tags(reader)?;
                    }
                }
                Event::Text(bytes_text) => {
                    let raw_xml = bytes_text.as_ref();

                    for vertex in raw_xml.split_terminator(';') {
                        let mut split = vertex.split_whitespace();

                        let x: i32 = split
                            .next()
                            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::X))?
                            .parse()?;
                        let y: i32 = split
                            .next()
                            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::Y))?
                            .parse()?;
                        point = Some(Point::from(from_file_coords(Coord { x, y })));
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(Section::PointObject));
                }
                _ => (),
            }
        }
        let geometry = point.ok_or(Error::MissingObjectGeometry(ObjectKind::Point))?;
        let mut object = Self::new(symbol, geometry).with_tags(tags);
        object.rotation = rotation;
        Ok(object)
    }
}
