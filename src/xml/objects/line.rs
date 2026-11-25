//! Line objects.

use crate::objects::{BezierPath, LineObject};
use crate::symbols::{LinePathSymbolId, SymbolSet};
use crate::xml::attrs::try_get_attr_raw;
use crate::xml::objects::boxed_tags;
use crate::xml::objects::{COORD_FLAGS_RING_END, bezier_from_file_coords, file_coords_from_bezier};
use crate::xml::{Error, Result, Section};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;

impl LineObject {
    pub(in crate::xml) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
    ) -> Result<()> {
        let index = symbol_set.file_index(self.symbol);

        self.write_content(writer, Some(index))
    }

    /// Write a full object element for use inside a point symbol.
    pub(in crate::xml) fn write_as_element<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        self.write_content(writer, None)
    }

    fn write_content<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_index: Option<i32>,
    ) -> Result<()> {
        if self.geometry_is_empty() {
            return Ok(());
        }

        let mut start = BytesStart::new("object").with_attributes([("type", "1")]);
        if let Some(symbol_index) = symbol_index {
            start.push_attribute(("symbol", symbol_index.to_string().as_str()));
        }
        writer.write_event(Event::Start(start))?;

        if !self.tags().is_empty() && symbol_index.is_some() {
            crate::xml::objects::write_tags(writer, self.tags())?;
        }

        let final_flags = if self.geometry().is_closed() {
            COORD_FLAGS_RING_END
        } else {
            0
        };
        let coords = file_coords_from_bezier(self.geometry(), final_flags)?;
        crate::xml::objects::write_file_coords(writer, &coords)?;
        writer.write_event(Event::End(BytesEnd::new("object")))?;
        Ok(())
    }

    /// Parse a line object through its closing `object` element.
    pub(in crate::xml) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbol: Option<LinePathSymbolId>,
    ) -> Result<Self> {
        let mut tags = HashMap::new();
        let mut file_coords = Vec::new();
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(start) => match start.local_name().as_ref() {
                    "coords" => {
                        let count = try_get_attr_raw(&start, "count")
                            .ok()
                            .flatten()
                            .unwrap_or(0);
                        file_coords.reserve(count);
                    }
                    "tags" => tags = crate::xml::objects::parse_tags(reader)?,
                    _ => (),
                },
                Event::End(end) if end.local_name().as_ref() == "object" => break,
                Event::Text(text) => {
                    crate::xml::objects::parse_file_coords(text.as_ref(), &mut file_coords)?;
                }
                Event::Eof => return Err(Error::UnexpectedEof(Section::LineObject)),
                _ => (),
            }
        }

        let geometry = bezier_from_file_coords(&file_coords).unwrap_or_else(BezierPath::empty);
        let mut object = Self::new(symbol, geometry);
        object.tags = boxed_tags(tags);
        Ok(object)
    }
}

#[cfg(test)]
mod tests {
    use geo_types::LineString;
    use quick_xml::{Reader, Writer, events::Event};

    use super::LineObject;
    use crate::NonNegativeF64;
    use crate::objects::BezierPath;
    use crate::objects::BezierSegment;
    use crate::objects::FlattenedPath;
    use crate::xml::Result;

    #[test]
    fn fitted_line_string_can_construct_line_object() -> Result<()> {
        let path = BezierPath::fit_line_string(
            LineString::from(vec![
                (0.0, 0.0),
                (1.0, 1.0),
                (2.0, 0.0),
                (3.0, -1.0),
                (4.0, 0.0),
            ]),
            NonNegativeF64::clamped_from(0.1),
        )?;
        let line = LineObject::new(None, path);

        assert!(
            line.geometry()
                .geometry()
                .segments()
                .any(BezierSegment::is_bezier_curve)
        );
        assert_eq!(
            line.geometry().num_vertices(),
            line.geometry().num_segments() + 1
        );
        Ok(())
    }

    #[test]
    fn parsed_line_owns_exact_beziers_and_dash_points() -> Result<()> {
        let mut reader = Reader::from_str(
            r#"<object><coords count="4">0 0 33;0 1000;1000 1000;1000 0 32;</coords></object>"#,
        );
        assert!(matches!(reader.read_event()?, Event::Start(_)));

        let line = LineObject::parse(&mut reader, None)?;
        assert!(matches!(
            line.geometry().geometry().segments().next(),
            Some(BezierSegment::Bezier(_))
        ));
        assert_eq!(line.geometry().vertex_is_dash_point(), [true, true]);
        assert_eq!(
            line.geometry().num_vertices(),
            line.geometry().num_segments() + 1
        );

        let flattened = line.flatten(NonNegativeF64::clamped_from(0.1))?;
        assert_eq!(
            flattened.vertex_is_dash_point().len(),
            flattened.geometry().0.len()
        );
        assert_eq!(flattened.num_vertices(), flattened.num_segments() + 1);
        assert_eq!(flattened.vertex_is_dash_point().first(), Some(&true));
        assert_eq!(flattened.vertex_is_dash_point().last(), Some(&true));

        let mut writer = Writer::new(Vec::new());
        line.write_content(&mut writer, None)?;
        let output = String::from_utf8(writer.into_inner())?;
        assert!(output.contains("0 0 33;0 1000;1000 1000;1000 0 32;"));
        Ok(())
    }

    #[test]
    fn replacing_with_flattened_geometry_makes_straight_segments() -> Result<()> {
        let flattened = FlattenedPath::new(
            vec![(0., 0.), (1., 0.), (2., 0.)].into(),
            vec![true, false, true],
        )?;
        let mut line = LineObject::new(None, flattened.clone());
        assert!(
            line.geometry()
                .geometry()
                .segments()
                .all(|segment| matches!(segment, BezierSegment::Line(_)))
        );

        line.replace_with_flattened(flattened);
        assert_eq!(line.geometry().vertex_is_dash_point(), [true, false, true]);
        assert_eq!(
            line.geometry().num_vertices(),
            line.geometry().num_segments() + 1
        );
        Ok(())
    }

    #[test]
    fn parsed_empty_line_is_not_written() -> Result<()> {
        let mut reader = Reader::from_str(r#"<object><coords count="1">0 0;</coords></object>"#);
        assert!(matches!(reader.read_event()?, Event::Start(_)));
        let line = LineObject::parse(&mut reader, None)?;

        assert!(line.geometry().is_empty());
        let mut writer = Writer::new(Vec::new());
        line.write_content(&mut writer, None)?;
        assert!(writer.into_inner().is_empty());
        Ok(())
    }
}
