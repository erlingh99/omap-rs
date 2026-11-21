//! Area objects.

use crate::objects::{AreaObject, BezierPolygon, PatternRotation};
use crate::symbols::{AreaPathSymbolId, SymbolSet};
use crate::xml::attrs::try_get_attr_raw;
use crate::xml::objects::{
    COORD_FLAGS_RING_END, FileCoord, bezier_from_file_coords, file_coords_from_bezier,
};
use crate::xml::units::{from_file_coords, to_file_coords};
use crate::xml::{Error, Result, Section};
use geo_types::Coord;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;

impl AreaObject {
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
    ) -> Result<()> {
        let index = symbol_set.file_index(self.symbol);

        self.write_content(writer, Some(index))
    }

    /// Write a full object element for use inside a point symbol.
    pub(crate) fn write_as_element<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
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
        self.geometry().validate()?;

        let mut start = BytesStart::new("object").with_attributes([("type", "1")]);
        if let Some(symbol_index) = symbol_index {
            start.push_attribute(("symbol", symbol_index.to_string().as_str()));
        }
        writer.write_event(Event::Start(start))?;

        if !self.tags().is_empty() && symbol_index.is_some() {
            crate::xml::objects::write_tags(writer, self.tags())?;
        }

        let mut all_coords =
            file_coords_from_bezier(self.geometry().exterior(), COORD_FLAGS_RING_END)?;
        for ring in self
            .geometry()
            .interiors()
            .iter()
            .filter(|ring| !ring.is_empty())
        {
            all_coords.extend(file_coords_from_bezier(ring, COORD_FLAGS_RING_END)?);
        }
        crate::xml::objects::write_file_coords(writer, &all_coords)?;
        self.write_pattern(writer)?;
        writer.write_event(Event::End(BytesEnd::new("object")))?;
        Ok(())
    }

    fn write_pattern<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let pattern = &self.pattern_rotation;
        let mut start = BytesStart::new("pattern");
        start.push_attribute(("rotation", pattern.rotation.to_string().as_str()));
        writer.write_event(Event::Start(start))?;
        let coord = to_file_coords(pattern.coord)?;
        writer.write_event(Event::Empty(BytesStart::new("coord").with_attributes([
            ("x", coord.x.to_string().as_str()),
            ("y", coord.y.to_string().as_str()),
        ])))?;
        writer.write_event(Event::End(BytesEnd::new("pattern")))?;
        Ok(())
    }

    /// Parse an area object through its closing `object` element.
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbol: Option<AreaPathSymbolId>,
    ) -> Result<Self> {
        let mut tags = HashMap::new();
        let mut pattern_rotation = PatternRotation::default();
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
                    "pattern" => {
                        pattern_rotation.rotation = try_get_attr_raw(&start, "rotation")
                            .ok()
                            .flatten()
                            .unwrap_or(pattern_rotation.rotation);
                    }
                    "tags" => tags = crate::xml::objects::parse_tags(reader)?,
                    "coord" => {
                        let x = try_get_attr_raw(&start, "x")?.unwrap_or(0);
                        let y = try_get_attr_raw(&start, "y")?.unwrap_or(0);
                        pattern_rotation.coord = from_file_coords(Coord { x, y });
                    }
                    _ => (),
                },
                Event::End(end) if end.local_name().as_ref() == "object" => break,
                Event::Text(text) => {
                    crate::xml::objects::parse_file_coords(text.as_ref(), &mut file_coords)?;
                }
                Event::Eof => return Err(Error::UnexpectedEof(Section::AreaObject)),
                _ => (),
            }
        }

        let geometry =
            bezier_polygon_from_file_coords(&file_coords).unwrap_or_else(BezierPolygon::empty);
        let mut object = Self::new(symbol, geometry).with_tags(tags);
        object.pattern_rotation = pattern_rotation;
        Ok(object)
    }
}

fn bezier_polygon_from_file_coords(coords: &[FileCoord]) -> Option<BezierPolygon> {
    let mut rings = Vec::new();
    let mut ring_start = 0;

    for (index, (_, flags)) in coords.iter().enumerate() {
        if flags & COORD_FLAGS_RING_END != 0 {
            if let Some(ring) = bezier_from_file_coords(&coords[ring_start..=index]) {
                rings.push(ring);
            }
            ring_start = index + 1;
        }
    }
    if ring_start < coords.len()
        && let Some(ring) = bezier_from_file_coords(&coords[ring_start..])
    {
        rings.push(ring);
    }

    let mut rings = rings.into_iter();
    BezierPolygon::new(rings.next()?, rings.collect()).ok()
}

#[cfg(test)]
mod tests {
    use geo_types::{LineString, Polygon};
    use quick_xml::{Reader, Writer, events::Event};

    use super::{AreaObject, BezierPolygon};
    use crate::NonNegativeF64;
    use crate::objects::BezierSegment;
    use crate::xml::Result;

    #[test]
    fn fitted_polygon_can_construct_area_object() -> Result<()> {
        let polygon = Polygon::new(
            LineString::from(vec![
                (0.0, 2.0),
                (1.4, 1.4),
                (2.0, 0.0),
                (1.4, -1.4),
                (0.0, -2.0),
                (-1.4, -1.4),
                (-2.0, 0.0),
                (-1.4, 1.4),
                (0.0, 2.0),
            ]),
            vec![LineString::from(vec![
                (0.0, 1.0),
                (0.7, 0.7),
                (1.0, 0.0),
                (0.7, -0.7),
                (0.0, -1.0),
                (-0.7, -0.7),
                (-1.0, 0.0),
                (-0.7, 0.7),
                (0.0, 1.0),
            ])],
        );
        let polygon = BezierPolygon::fit_polygon(polygon, NonNegativeF64::clamped_from(0.2))?;
        let area = AreaObject::new(None, polygon);

        assert_eq!(area.geometry().interiors().len(), 1);
        for ring in std::iter::once(area.geometry().exterior()).chain(area.geometry().interiors()) {
            assert!(ring.is_closed());
            assert_eq!(ring.num_vertices(), ring.num_segments() + 1);
            assert!(ring.vertex_is_dash_point().iter().all(|flag| !flag));
        }
        assert!(
            area.geometry()
                .exterior()
                .geometry()
                .segments()
                .any(BezierSegment::is_bezier_curve)
        );
        Ok(())
    }

    #[test]
    fn parsed_polygon_owns_closed_bezier_rings_and_dash_points() -> Result<()> {
        let mut reader = Reader::from_str(
            r#"<object><coords count="7">0 0 32;2000 0;1000 1000 2;500 250 32;1000 250;750 750;500 250 50;</coords><pattern rotation="0"></pattern></object>"#,
        );
        assert!(matches!(reader.read_event()?, Event::Start(_)));

        let area = AreaObject::parse(&mut reader, None)?;
        assert!(area.geometry().exterior().is_closed());
        assert_eq!(area.geometry().interiors().len(), 1);
        assert!(area.geometry().interiors()[0].is_closed());
        for ring in std::iter::once(area.geometry().exterior()).chain(area.geometry().interiors()) {
            assert_eq!(ring.num_vertices(), ring.num_segments() + 1);
        }
        assert_eq!(
            area.geometry().exterior().vertex_is_dash_point(),
            [true, false, false, true]
        );

        let flattened = area.flatten(NonNegativeF64::clamped_from(0.1))?;
        assert_eq!(flattened.interiors().len(), 1);
        for ring in std::iter::once(flattened.exterior()).chain(flattened.interiors()) {
            assert_eq!(ring.num_vertices(), ring.num_segments() + 1);
        }
        assert_eq!(
            flattened.exterior().vertex_is_dash_point().len(),
            flattened.exterior().geometry().0.len()
        );

        let mut writer = Writer::new(Vec::new());
        area.write_content(&mut writer, None)?;
        let output = String::from_utf8(writer.into_inner())?;
        assert!(output.contains("0 0 32;2000 0;1000 1000;0 0 50;"));
        assert!(output.contains("500 250 32;1000 250;750 750;500 250 50;"));
        Ok(())
    }

    #[test]
    fn flatten_in_place_replaces_curves_with_lines() -> Result<()> {
        let mut reader = Reader::from_str(
            r#"<object><coords count="5">0 0 1;0 1000;1000 1000;1000 0;0 0 2;</coords><pattern rotation="0"></pattern></object>"#,
        );
        assert!(matches!(reader.read_event()?, Event::Start(_)));
        let mut area = AreaObject::parse(&mut reader, None)?;

        area.flatten_in_place(NonNegativeF64::clamped_from(0.1))?;
        assert!(
            area.geometry()
                .exterior()
                .geometry()
                .segments()
                .all(|segment| matches!(segment, BezierSegment::Line(_)))
        );
        Ok(())
    }
}
