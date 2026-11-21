//! Paths: the file's coordinate lists, as Bézier paths of the map model.
//!
//! A curve is stored as its start point, two control points flagged as such,
//! then its end point. The first point of a hole carries the hole flag, which
//! starts a new ring.

use geo_types::Coord;
use linestring2bezier::{BezierSegment, BezierString};

use crate::objects::{BezierPath, BezierPolygon};
use crate::ocd::Result;
use crate::ocd::units::FileCoord;

/// The rings `coords` describe, each as a Bézier path; empty rings are left
/// out.
pub(in crate::ocd) fn read_rings(coords: &[FileCoord]) -> Vec<BezierPath> {
    let mut rings = Vec::new();
    let mut ring = RingBuilder::default();
    let mut index = 0;
    while let Some(&coord) = coords.get(index) {
        if index > 0 && coord.is_hole_start() {
            rings.extend(std::mem::take(&mut ring).finish());
        }
        if coord.is_first_control_point()
            && let (Some(second), Some(&end)) = (coords.get(index + 1), coords.get(index + 2))
        {
            ring.curve_to(coord.coord(), second.coord(), end);
            index += 3;
        } else {
            ring.line_to(coord);
            index += 1;
        }
    }
    rings.extend(ring.finish());
    rings
}

/// The area `coords` describe: the first ring is its outline, any later ones
/// its holes. Every ring is closed.
pub(in crate::ocd) fn read_polygon(coords: &[FileCoord]) -> Option<BezierPolygon> {
    let mut rings = read_rings(coords).into_iter();
    let exterior = rings.next()?;
    BezierPolygon::new(exterior, rings.collect()).ok()
}

/// The coordinates of `rings`, each after the first starting a hole. Dash
/// points get `dash_flag` on y.
pub(in crate::ocd) fn write_rings<'a>(
    rings: impl IntoIterator<Item = &'a BezierPath>,
    dash_flag: i32,
) -> Result<Vec<FileCoord>> {
    let mut coords = Vec::new();
    for (ring_index, ring) in rings.into_iter().enumerate() {
        let flags = ring.vertex_is_dash_point();
        let mut segments = ring.geometry().segments();
        let Some(first) = segments.next() else {
            continue;
        };
        let mut start = FileCoord::from_coord(first.start())?;
        if ring_index > 0 {
            start.y |= FileCoord::HOLE_START;
        }
        if flags.first().copied().unwrap_or(false) {
            start.y |= dash_flag;
        }
        coords.push(start);
        for (index, segment) in std::iter::once(first).chain(segments).enumerate() {
            if let BezierSegment::Bezier(curve) = segment {
                let mut first = FileCoord::from_coord(curve.handle1)?;
                first.x |= FileCoord::FIRST_CONTROL_POINT;
                let mut second = FileCoord::from_coord(curve.handle2)?;
                second.x |= FileCoord::SECOND_CONTROL_POINT;
                coords.extend([first, second]);
            }
            let mut end = FileCoord::from_coord(segment.end())?;
            if flags.get(index + 1).copied().unwrap_or(false) {
                end.y |= dash_flag;
            }
            coords.push(end);
        }
    }
    Ok(coords)
}

/// The coordinates of `polygon`: its outline, then its holes.
pub(in crate::ocd) fn write_polygon(
    polygon: &BezierPolygon,
    dash_flag: i32,
) -> Result<Vec<FileCoord>> {
    write_rings(
        std::iter::once(polygon.exterior()).chain(polygon.interiors()),
        dash_flag,
    )
}

/// A ring being read.
#[derive(Default)]
struct RingBuilder {
    last: Option<Coord>,
    segments: Vec<BezierSegment>,
    dash_points: Vec<bool>,
}

impl RingBuilder {
    fn line_to(&mut self, coord: FileCoord) {
        let end = coord.coord();
        match self.last {
            None => self.dash_points.push(coord.is_dash_point()),
            Some(start) => {
                self.segments.push(BezierSegment::new(start, None, end));
                self.dash_points.push(coord.is_dash_point());
            }
        }
        self.last = Some(end);
    }

    fn curve_to(&mut self, first: Coord, second: Coord, end: FileCoord) {
        let Some(start) = self.last else {
            // A curve with no point before it: keep only where it ends.
            self.line_to(end);
            return;
        };
        let end_coord = end.coord();
        self.segments
            .push(BezierSegment::new(start, Some((first, second)), end_coord));
        self.dash_points.push(end.is_dash_point());
        self.last = Some(end_coord);
    }

    fn finish(self) -> Option<BezierPath> {
        if self.segments.is_empty() {
            return None;
        }
        let mut dash_points = self.dash_points;
        let geometry = BezierString::new(self.segments);
        // A closed path's first and last vertex are one point, so one flag.
        if geometry.is_closed()
            && let Some(&first) = dash_points.first()
            && let Some(last) = dash_points.last_mut()
        {
            *last = first;
        }
        BezierPath::new(geometry, dash_points).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{read_polygon, read_rings};
    use crate::ocd::units::FileCoord;

    fn point(x: i32, y: i32) -> FileCoord {
        FileCoord {
            x: x << 8,
            y: y << 8,
        }
    }

    fn flagged(mut coord: FileCoord, x_flags: i32, y_flags: i32) -> FileCoord {
        coord.x |= x_flags;
        coord.y |= y_flags;
        coord
    }

    #[test]
    fn control_points_make_a_curve_and_flags_mark_dash_points() {
        let coords = [
            point(0, 0),
            flagged(point(100, 0), FileCoord::FIRST_CONTROL_POINT, 0),
            flagged(point(200, 100), FileCoord::SECOND_CONTROL_POINT, 0),
            flagged(point(200, 200), 0, FileCoord::DASH_POINT),
            point(0, 200),
        ];
        let rings = read_rings(&coords);
        assert_eq!(rings.len(), 1);
        let path = &rings[0];
        assert_eq!(path.num_segments(), 2);
        assert!(
            path.geometry()
                .segments()
                .next()
                .is_some_and(|s| s.is_bezier_curve())
        );
        assert_eq!(path.vertex_is_dash_point(), [false, true, false]);
    }

    #[test]
    fn a_hole_flag_starts_the_next_ring_and_rings_close() -> crate::Result<()> {
        let coords = [
            point(0, 0),
            point(1000, 0),
            point(1000, 1000),
            point(0, 1000),
            flagged(point(200, 200), 0, FileCoord::HOLE_START),
            point(400, 200),
            point(400, 400),
        ];
        let polygon = read_polygon(&coords).ok_or(crate::Error::ObjectError)?;
        assert_eq!(polygon.interiors().len(), 1);
        assert!(polygon.exterior().is_closed());
        assert!(polygon.interiors()[0].is_closed());
        Ok(())
    }
}
