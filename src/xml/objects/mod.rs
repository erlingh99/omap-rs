//! Map objects: their coordinates, flags and tags.

mod area;
mod line;
mod map_object;
mod point;
mod text;

use crate::objects::{BezierPath, BezierSegment, BezierString};
use crate::xml::attrs::try_get_attr;
use crate::xml::units::{from_file_coords, to_file_coords};
use crate::{CoordinateComponent, Error, OmapSection, Result};
use geo_types::Coord;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;

pub(crate) type FileCoord = (Coord<i32>, u8);

/// A coordinate starts a cubic Bézier segment.
pub(crate) const COORD_FLAG_CURVE_START: u8 = 1;
/// A coordinate closes the current path.
pub(crate) const COORD_FLAG_CLOSE_POINT: u8 = 2;
/// A coordinate closes a polygon ring.
pub(crate) const COORD_FLAG_HOLE_POINT: u8 = 16;
/// A coordinate is a forced dash point.
pub(crate) const COORD_FLAG_DASH_POINT: u8 = 32;

pub(crate) const COORD_FLAGS_RING_END: u8 = COORD_FLAG_CLOSE_POINT | COORD_FLAG_HOLE_POINT;

/// Build the exact mixed line/Bézier representation encoded by Mapper's raw
/// coordinate flags.
///
/// A coordinate with bit 0 set starts a cubic Bézier whose two handles and end
/// point are the following three coordinates. An end point may also start the
/// next curve, so it is deliberately visited again in that case.
pub(crate) fn bezier_from_file_coords(coords: &[FileCoord]) -> Option<BezierPath> {
    let mut segments = Vec::new();
    let mut vertex_is_dash_point = coords
        .first()
        .map(|(_, flag)| flag & COORD_FLAG_DASH_POINT != 0)
        .into_iter()
        .collect::<Vec<_>>();
    let mut previous_anchor = None;
    let mut index = 0;

    while index < coords.len() {
        let (file_coord, flag) = coords[index];
        let coord = from_file_coords(file_coord);

        if let Some((previous_index, previous_coord)) = previous_anchor
            && previous_index != index
        {
            segments.push(BezierSegment::new(previous_coord, None, coord));
            vertex_is_dash_point.push(flag & COORD_FLAG_DASH_POINT != 0);
        }

        if flag & COORD_FLAG_CURVE_START != 0 && index + 3 < coords.len() {
            let handle1 = from_file_coords(coords[index + 1].0);
            let handle2 = from_file_coords(coords[index + 2].0);
            let end_index = index + 3;
            let end = from_file_coords(coords[end_index].0);
            segments.push(BezierSegment::new(coord, Some((handle1, handle2)), end));
            vertex_is_dash_point.push(coords[end_index].1 & COORD_FLAG_DASH_POINT != 0);
            previous_anchor = Some((end_index, end));

            if coords[end_index].1 & COORD_FLAG_CURVE_START != 0 {
                index = end_index;
            } else {
                index = end_index + 1;
            }
        } else {
            previous_anchor = Some((index, coord));
            index += 1;
        }
    }

    let geometry = BezierString::new(segments);
    if geometry.is_closed() && vertex_is_dash_point.len() > 1 {
        let seam_is_dash_point = vertex_is_dash_point.first().copied().unwrap_or(false)
            || vertex_is_dash_point.last().copied().unwrap_or(false);
        let last = vertex_is_dash_point.len() - 1;
        vertex_is_dash_point[0] = seam_is_dash_point;
        vertex_is_dash_point[last] = seam_is_dash_point;
    }

    BezierPath::new(geometry, vertex_is_dash_point).ok()
}

pub(crate) fn file_coords_from_bezier(
    path: &BezierPath,
    final_vertex_flags: u8,
) -> Result<Vec<FileCoord>> {
    path.validate()?;
    let final_vertex = path
        .geometry()
        .segments()
        .last()
        .map(BezierSegment::end)
        .ok_or(Error::ObjectError)?;

    let mut coords = Vec::with_capacity(path.geometry().num_points());
    for (index, segment) in path.geometry().segments().enumerate() {
        let dash_flag = if path.vertex_is_dash_point()[index] {
            COORD_FLAG_DASH_POINT
        } else {
            0
        };
        match segment {
            BezierSegment::Bezier(curve) => {
                coords.push((
                    to_file_coords(curve.start)?,
                    COORD_FLAG_CURVE_START | dash_flag,
                ));
                coords.push((to_file_coords(curve.handle1)?, 0));
                coords.push((to_file_coords(curve.handle2)?, 0));
            }
            BezierSegment::Line(line) => {
                coords.push((to_file_coords(line.start)?, dash_flag));
            }
        }
    }
    let final_dash_flag = if path.vertex_is_dash_point().last().copied().unwrap_or(false) {
        COORD_FLAG_DASH_POINT
    } else {
        0
    };
    coords.push((
        to_file_coords(final_vertex)?,
        final_vertex_flags | final_dash_flag,
    ));
    Ok(coords)
}

pub(crate) fn parse_file_coords(text: &str, coords: &mut Vec<FileCoord>) -> Result<()> {
    for vertex in text.split_terminator(';') {
        let mut parts = vertex.split_whitespace();
        let x = parts
            .next()
            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::X))?
            .parse()?;
        let y = parts
            .next()
            .ok_or(Error::MissingCoordinateComponent(CoordinateComponent::Y))?
            .parse()?;
        let flags = parts.next().map(str::parse).transpose()?.unwrap_or(0);
        coords.push((Coord { x, y }, flags));
    }
    Ok(())
}

pub(crate) fn parse_tags<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<HashMap<String, String>> {
    let mut buf = Vec::new();

    let mut tags = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(bytes_start) => {
                if matches!(bytes_start.local_name().as_ref(), "t") {
                    let key = try_get_attr(&bytes_start, "k")?.unwrap_or(String::new());
                    let value = crate::xml::read_text(reader)?;
                    if !key.is_empty() && !value.is_empty() {
                        let _old_value = tags.insert(key, value);
                    }
                }
            }
            Event::End(bytes_end) if bytes_end.local_name().as_ref() == "tags" => {
                break;
            }
            Event::Eof => {
                return Err(Error::UnexpectedEof(OmapSection::Tags));
            }
            _ => (),
        }
    }
    Ok(tags)
}

pub(crate) fn write_tags<W: std::io::Write>(
    writer: &mut Writer<W>,
    tags: &HashMap<String, String>,
) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new("tags")))?;
    // Sorted, because `HashMap` iteration order is arbitrary and writing the
    // same map twice must produce the same bytes.
    let mut keys: Vec<&String> = tags.keys().collect();
    keys.sort_unstable();
    for key in keys {
        let Some(value) = tags.get(key) else { continue };
        writer.write_event(Event::Start(
            BytesStart::new("t").with_attributes([("k", key.as_str())]),
        ))?;
        writer.write_event(Event::Text(BytesText::new(value)))?;
        writer.write_event(Event::End(BytesEnd::new("t")))?;
    }
    writer.write_event(Event::End(BytesEnd::new("tags")))?;
    Ok(())
}

/// Write file coordinates as the content of a `<coords>` element.
pub(crate) fn write_file_coords<W: std::io::Write>(
    writer: &mut Writer<W>,
    coords: &[FileCoord],
) -> Result<()> {
    let bs =
        BytesStart::new("coords").with_attributes([("count", coords.len().to_string().as_str())]);
    writer.write_event(Event::Start(bs))?;
    let mut content = String::new();
    for (coord, flag) in coords {
        content.push_str(&coord.x.to_string());
        content.push(' ');
        content.push_str(&coord.y.to_string());
        if *flag != 0 {
            content.push(' ');
            content.push_str(&flag.to_string());
        }
        content.push(';');
    }
    writer.write_event(Event::Text(BytesText::new(&content)))?;
    writer.write_event(Event::End(BytesEnd::new("coords")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use geo_types::Coord;
    use linestring2bezier::{BezierSegment, BezierString};

    use super::{
        COORD_FLAG_CURVE_START, COORD_FLAG_DASH_POINT, bezier_from_file_coords,
        file_coords_from_bezier,
    };
    use crate::{NonNegativeF64, Result, objects::BezierPath};

    #[test]
    fn flattening_and_serialization_retain_dash_points() -> Result<()> {
        let path = BezierPath::new(
            BezierString::new(vec![BezierSegment::new(
                Coord { x: 0.0, y: 0.0 },
                Some((Coord { x: 0.0, y: 1.0 }, Coord { x: 1.0, y: 1.0 })),
                Coord { x: 1.0, y: 0.0 },
            )]),
            vec![true, true],
        )?;

        let flattened = path.flatten(NonNegativeF64::clamped_from(0.05))?;
        assert!(flattened.geometry().0.len() > 2);
        assert_eq!(flattened.vertex_is_dash_point().first(), Some(&true));
        assert_eq!(flattened.vertex_is_dash_point().last(), Some(&true));
        assert!(
            flattened.vertex_is_dash_point()[1..flattened.geometry().0.len() - 1]
                .iter()
                .all(|flag| !flag)
        );

        let coords = file_coords_from_bezier(&path, 0)?;
        assert_eq!(coords[0].1, COORD_FLAG_CURVE_START | COORD_FLAG_DASH_POINT);
        assert_eq!(
            coords.last().map(|coord| coord.1),
            Some(COORD_FLAG_DASH_POINT)
        );
        Ok(())
    }

    fn assert_vertex_flag_count(path: &BezierPath) {
        let expected = if path.is_empty() {
            0
        } else {
            path.num_segments() + 1
        };
        assert_eq!(path.num_vertices(), expected);
        assert_eq!(path.vertex_is_dash_point().len(), expected);
    }

    #[test]
    fn dash_flags_align_with_segment_start_vertices_and_final_end() {
        let path = bezier_from_file_coords(&[
            (Coord { x: 0, y: 0 }, COORD_FLAG_DASH_POINT),
            (Coord { x: 1_000, y: 0 }, 33),
            (Coord { x: 1_000, y: 1_000 }, 0),
            (Coord { x: 2_000, y: 1_000 }, 0),
            (Coord { x: 2_000, y: 0 }, 32),
            (Coord { x: 3_000, y: 0 }, 0),
        ]);
        assert!(path.is_some());
        let Some(path) = path else {
            return;
        };

        assert_eq!(path.geometry().num_segments(), 3);
        assert_eq!(path.vertex_is_dash_point(), [true, true, true, false]);
        assert_vertex_flag_count(&path);
        assert!(matches!(path.geometry().0[0], BezierSegment::Line(_)));
        assert!(matches!(path.geometry().0[1], BezierSegment::Bezier(_)));
        assert!(matches!(path.geometry().0[2], BezierSegment::Line(_)));
        assert_eq!(
            path.segments()
                .map(|(_, start_is_dash_point)| start_is_dash_point)
                .collect::<Vec<_>>(),
            [true, true, true]
        );
    }

    #[test]
    fn closed_path_combines_first_and_closing_vertex_dash_flags() {
        let path = bezier_from_file_coords(&[
            (Coord { x: 0, y: 0 }, 32),
            (Coord { x: 1_000, y: 0 }, 0),
            (Coord { x: 0, y: 0 }, 2),
        ]);
        assert!(path.is_some());
        let Some(path) = path else {
            return;
        };

        assert_eq!(path.geometry().num_segments(), 2);
        assert_eq!(path.vertex_is_dash_point(), [true, false, true]);
        assert_vertex_flag_count(&path);
    }

    #[test]
    fn open_path_preserves_both_endpoint_dash_flags() {
        let path = bezier_from_file_coords(&[
            (Coord { x: 0, y: 0 }, COORD_FLAG_DASH_POINT),
            (Coord { x: 1_000, y: 0 }, COORD_FLAG_DASH_POINT),
            (Coord { x: 2_000, y: 0 }, COORD_FLAG_DASH_POINT),
            (Coord { x: 3_000, y: 0 }, COORD_FLAG_DASH_POINT),
        ]);
        assert!(path.is_some());
        let Some(path) = path else {
            return;
        };

        assert_eq!(path.geometry().num_segments(), 3);
        assert_eq!(path.vertex_is_dash_point(), [true, true, true, true]);
        assert_vertex_flag_count(&path);
    }
}
