mod area_object;
mod ids;
mod line_object;
mod point_object;
mod text_object;

mod map_object;

use crate::Transformable;
use geo_types::{Coord, LineString};
pub use linestring2bezier::{BezierSegment, BezierString};
use std::collections::HashMap;

pub use area_object::{AreaObject, BezierPolygon, FlattenedPolygon, PatternRotation};
pub use line_object::LineObject;
pub use point_object::PointObject;
pub use text_object::{HorizontalAlign, TextGeometry, TextObject, VerticalAlign, WrapBox};

pub use ids::ObjectId;
pub use map_object::MapObject;

use super::{Error, Result};

/// A mixed straight/cubic Bézier path with dash-point metadata on its
/// vertices.
///
/// A nonempty path has one dash flag for the start of every segment, followed
/// by one flag for the final segment's end. Consequently, a nonempty path with
/// `n` segments always has `n + 1` flags. An empty path has no vertices and no
/// flags. For a closed path, the first and final entries describe the same
/// seam vertex and have the same value.
///
/// The path is the geometry stored by line and area objects.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierPath {
    /// The straight and cubic segments forming the path.
    geometry: BezierString,
    /// Whether each path vertex is a forced dash point.
    vertex_is_dash_point: Vec<bool>,
}

impl BezierPath {
    /// Build a path from its geometry and per-vertex dash flags.
    ///
    /// Empty geometry must have no flags. Otherwise there must be one flag for
    /// every segment start plus one for the final segment end, adjacent
    /// segments must be connected, and the two copies of a closed path's seam
    /// flag must agree.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidBezierPath`] when an invariant is not upheld.
    pub fn new(geometry: BezierString, vertex_is_dash_point: Vec<bool>) -> Result<Self> {
        let path = Self {
            geometry,
            vertex_is_dash_point,
        };
        path.validate()?;
        Ok(path)
    }

    /// Construct an empty path.
    pub fn empty() -> Self {
        Self {
            geometry: BezierString::empty(),
            vertex_is_dash_point: Vec::new(),
        }
    }

    /// Fit a smooth Bézier path to a line string within `allowed_error`.
    ///
    /// All vertices in the fitted path initially have their forced dash-point
    /// state disabled. Use [`Self::set_vertex_is_dash_point`] to enable it on
    /// fitted segment boundaries.
    ///
    /// Unlike [`From<LineString>`], which preserves every input segment as a
    /// straight line, this method may replace several input segments with one
    /// cubic Bézier segment.
    ///
    /// # Errors
    ///
    /// Returns an error when the line string contains fewer than two
    /// coordinates or `allowed_error` is too small for fitting.
    pub fn fit_line_string(
        line_string: LineString,
        allowed_error: crate::NonNegativeF64,
    ) -> Result<Self> {
        let geometry = BezierString::from_line_string(line_string, allowed_error.get())?;
        let vertex_is_dash_point = vec![false; geometry.num_segments() + 1];
        Self::new(geometry, vertex_is_dash_point)
    }

    /// Get the mixed straight/cubic geometry.
    pub fn geometry(&self) -> &BezierString {
        &self.geometry
    }

    /// Get one dash flag per segment start plus the final segment's end flag.
    pub fn vertex_is_dash_point(&self) -> &[bool] {
        &self.vertex_is_dash_point
    }

    /// Return the number of segments in the path.
    pub fn num_segments(&self) -> usize {
        self.geometry.num_segments()
    }

    /// Return the number of path vertices represented by dash flags.
    ///
    /// This is zero for an empty path and [`Self::num_segments`] plus one for
    /// every nonempty path.
    pub fn num_vertices(&self) -> usize {
        self.vertex_is_dash_point.len()
    }

    /// Set the dash-point state of a path vertex.
    ///
    /// When the path is closed, changing either copy of the seam vertex also
    /// changes the other copy. Returns `false` for an out-of-range index.
    pub fn set_vertex_is_dash_point(&mut self, index: usize, is_dash_point: bool) -> bool {
        let Some(flag) = self.vertex_is_dash_point.get_mut(index) else {
            return false;
        };
        *flag = is_dash_point;

        if self.is_closed() && self.vertex_is_dash_point.len() > 1 {
            let last = self.vertex_is_dash_point.len() - 1;
            if index == 0 {
                self.vertex_is_dash_point[last] = is_dash_point;
            } else if index == last {
                self.vertex_is_dash_point[0] = is_dash_point;
            }
        }
        true
    }

    /// Mutably iterate over the straight and cubic segments.
    ///
    /// This permits editing endpoints and handles but not changing the segment
    /// count. Use [`Self::into_parts`] and [`Self::new`] for structural edits.
    pub fn segments_mut(&mut self) -> std::slice::IterMut<'_, BezierSegment> {
        self.geometry.segments_mut()
    }

    /// Return whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// Return whether the path is empty or its final endpoint equals its start.
    pub fn is_closed(&self) -> bool {
        self.geometry.is_closed()
    }

    /// Close a nonempty open path with a straight segment.
    pub fn close(&mut self) {
        if self.is_empty() || self.is_closed() {
            return;
        }
        self.geometry.close();
        self.vertex_is_dash_point
            .push(self.vertex_is_dash_point.first().copied().unwrap_or(false));
    }

    /// Reverse the path while keeping dash flags attached to their vertices.
    pub fn reverse(&mut self) {
        self.geometry.0 = self
            .geometry
            .segments()
            .rev()
            .map(|segment| match segment {
                BezierSegment::Bezier(curve) => {
                    BezierSegment::new(curve.end, Some((curve.handle2, curve.handle1)), curve.start)
                }
                BezierSegment::Line(line) => BezierSegment::new(line.end, None, line.start),
            })
            .collect();
        self.vertex_is_dash_point.reverse();
    }

    /// Flatten the path while retaining dash-point metadata.
    ///
    /// Approximation vertices inserted inside a Bézier segment are not forced
    /// dash points. Original segment ends retain their flags.
    ///
    /// # Errors
    ///
    /// Returns an error when the tolerance is too small or the path has become
    /// invalid after mutable segment access.
    pub fn flatten(&self, allowed_error: crate::NonNegativeF64) -> Result<FlattenedPath> {
        self.validate()?;
        let (geometry, segment_end_indices) = self
            .geometry
            .to_line_string_with_segment_ends(allowed_error.get())?;
        let mut vertex_is_dash_point = vec![false; geometry.0.len()];

        if let Some(first) = vertex_is_dash_point.first_mut() {
            *first = self.vertex_is_dash_point.first().copied().unwrap_or(false);
        }
        for (index, is_dash_point) in segment_end_indices
            .into_iter()
            .zip(self.vertex_is_dash_point.iter().copied().skip(1))
        {
            vertex_is_dash_point[index] = is_dash_point;
        }

        FlattenedPath::new(geometry, vertex_is_dash_point)
    }

    /// Consume the path and return its geometry and vertex dash flags.
    pub fn into_parts(self) -> (BezierString, Vec<bool>) {
        (self.geometry, self.vertex_is_dash_point)
    }

    /// Iterate over segments paired with the dash-point state of their start
    /// vertices.
    ///
    /// The final segment-end state is the last item returned by
    /// [`Self::vertex_is_dash_point`].
    pub fn segments(&self) -> impl ExactSizeIterator<Item = (&BezierSegment, bool)> {
        self.geometry
            .segments()
            .zip(self.vertex_is_dash_point.iter().copied())
    }

    /// Validate segment continuity and dash-point metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidBezierPath`] when an invariant is not upheld.
    pub fn validate(&self) -> Result<()> {
        if self.geometry.is_empty() {
            return if self.vertex_is_dash_point.is_empty() {
                Ok(())
            } else {
                Err(Error::InvalidBezierPath)
            };
        }

        if self.num_vertices() != self.num_segments() + 1 {
            return Err(Error::InvalidBezierPath);
        }

        let mut segments = self.geometry.segments();
        let Some(first) = segments.next() else {
            return Err(Error::InvalidBezierPath);
        };
        let mut previous_end = first.end();
        for segment in segments {
            if segment.start() != previous_end {
                return Err(Error::InvalidBezierPath);
            }
            previous_end = segment.end();
        }

        if self.is_closed() && self.vertex_is_dash_point.first() != self.vertex_is_dash_point.last()
        {
            return Err(Error::InvalidBezierPath);
        }
        Ok(())
    }
}

/// Every endpoint and Bézier handle is transformed; topology and dash-point
/// metadata are kept.
impl Transformable for BezierPath {
    fn try_transform<E>(
        mut self,
        transform: impl Fn(Coord) -> std::result::Result<Coord, E>,
    ) -> std::result::Result<Self, E> {
        self.geometry = self.geometry.try_transform(transform)?;
        Ok(self)
    }
}

impl From<LineString> for BezierPath {
    fn from(geometry: LineString) -> Self {
        if geometry.0.len() < 2 {
            return Self::empty();
        }

        let vertex_is_dash_point = vec![false; geometry.0.len()];
        let segments = geometry
            .0
            .windows(2)
            .map(|pair| BezierSegment::new(pair[0], None, pair[1]))
            .collect();
        let path = Self {
            geometry: BezierString::new(segments),
            vertex_is_dash_point,
        };
        debug_assert_eq!(
            path.num_vertices(),
            path.num_segments() + 1,
            "a line string must create one dash flag per segment start plus the final end"
        );
        path
    }
}

/// An owned flattened path with one dash flag for every coordinate.
///
/// Coordinates are implicit straight-segment starts except for the last,
/// which is the final segment end. Thus a flattened path with `n` segments
/// has `n + 1` flags. A path without coordinates has no flags.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlattenedPath {
    geometry: LineString,
    vertex_is_dash_point: Vec<bool>,
}

impl FlattenedPath {
    /// Construct flattened geometry and validate its metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidFlattenedPath`] unless the flag count matches
    /// the coordinate count and a closed path's seam flags agree.
    pub fn new(geometry: LineString, vertex_is_dash_point: Vec<bool>) -> Result<Self> {
        let path = Self {
            geometry,
            vertex_is_dash_point,
        };
        path.validate()?;
        Ok(path)
    }

    /// Get the flattened coordinates.
    pub fn geometry(&self) -> &LineString {
        &self.geometry
    }

    /// Get the dash-point state corresponding to every coordinate.
    pub fn vertex_is_dash_point(&self) -> &[bool] {
        &self.vertex_is_dash_point
    }

    /// Return the number of implicit straight segments.
    pub fn num_segments(&self) -> usize {
        self.geometry.0.len().saturating_sub(1)
    }

    /// Return the number of flattened vertices and corresponding dash flags.
    pub fn num_vertices(&self) -> usize {
        self.vertex_is_dash_point.len()
    }

    /// Consume the path and return its coordinates and dash flags.
    pub fn into_parts(self) -> (LineString, Vec<bool>) {
        (self.geometry, self.vertex_is_dash_point)
    }

    /// Validate the coordinate and dash-flag counts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidFlattenedPath`] when an invariant is not upheld.
    pub fn validate(&self) -> Result<()> {
        if self.geometry.0.len() != self.num_vertices()
            || (self.geometry.0.len() > 1
                && self.geometry.is_closed()
                && self.vertex_is_dash_point.first() != self.vertex_is_dash_point.last())
        {
            return Err(Error::InvalidFlattenedPath);
        }
        Ok(())
    }
}

impl From<FlattenedPath> for BezierPath {
    fn from(path: FlattenedPath) -> Self {
        let (geometry, vertex_is_dash_point) = path.into_parts();
        if geometry.0.len() < 2 {
            return Self::empty();
        }
        let segments = geometry
            .0
            .windows(2)
            .map(|pair| BezierSegment::new(pair[0], None, pair[1]))
            .collect();
        let path = Self {
            geometry: BezierString::new(segments),
            vertex_is_dash_point,
        };
        debug_assert_eq!(
            path.num_vertices(),
            path.num_segments() + 1,
            "flattened coordinates must create one dash flag per segment start plus the final end"
        );
        path
    }
}

fn empty_tags() -> &'static HashMap<String, String> {
    static EMPTY: std::sync::LazyLock<HashMap<String, String>> =
        std::sync::LazyLock::new(HashMap::new);
    &EMPTY
}

#[cfg(test)]
mod tests {
    use geo_types::{Coord, LineString};
    use linestring2bezier::{BezierSegment, BezierString};

    use super::{BezierPath, FlattenedPath};
    use crate::{Error, NonNegativeF64, Result, Transformable as _};

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
    fn bezier_path_construction_enforces_invariants() -> Result<()> {
        assert!(BezierPath::new(BezierString::empty(), Vec::new())?.is_empty());
        assert!(matches!(
            BezierPath::new(BezierString::empty(), vec![false]),
            Err(Error::InvalidBezierPath)
        ));

        let open_geometry = BezierString::new(vec![BezierSegment::new(
            Coord { x: 0.0, y: 0.0 },
            None,
            Coord { x: 1.0, y: 0.0 },
        )]);
        assert!(matches!(
            BezierPath::new(open_geometry.clone(), vec![false]),
            Err(Error::InvalidBezierPath)
        ));
        assert!(matches!(
            BezierPath::new(open_geometry.clone(), vec![false, false, false]),
            Err(Error::InvalidBezierPath)
        ));
        assert!(BezierPath::new(open_geometry, vec![false, true]).is_ok());

        let closed_geometry = BezierString::new(vec![
            BezierSegment::new(Coord { x: 0.0, y: 0.0 }, None, Coord { x: 1.0, y: 0.0 }),
            BezierSegment::new(Coord { x: 1.0, y: 0.0 }, None, Coord { x: 0.0, y: 0.0 }),
        ]);
        assert!(matches!(
            BezierPath::new(closed_geometry.clone(), vec![false, false, true]),
            Err(Error::InvalidBezierPath)
        ));
        assert!(BezierPath::new(closed_geometry, vec![true, false, true]).is_ok());

        assert!(matches!(
            FlattenedPath::new(LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]), vec![false]),
            Err(Error::InvalidFlattenedPath)
        ));
        assert!(matches!(
            FlattenedPath::new(
                LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]),
                vec![false, false, false]
            ),
            Err(Error::InvalidFlattenedPath)
        ));
        Ok(())
    }

    #[test]
    fn every_conversion_and_structural_operation_preserves_vertex_flag_count() -> Result<()> {
        let mut path = BezierPath::from(LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 1.0)]));
        assert_vertex_flag_count(&path);

        path.close();
        assert_vertex_flag_count(&path);

        path.reverse();
        assert_vertex_flag_count(&path);

        path = path.transform(|coord| Coord {
            x: coord.x + 1.0,
            y: coord.y - 1.0,
        });
        assert_vertex_flag_count(&path);

        let flattened = path.flatten(NonNegativeF64::clamped_from(0.1))?;
        assert_eq!(flattened.num_vertices(), flattened.num_segments() + 1);
        assert_eq!(
            flattened.vertex_is_dash_point().len(),
            flattened.geometry().0.len()
        );

        let rebuilt = BezierPath::from(flattened);
        assert_vertex_flag_count(&rebuilt);
        Ok(())
    }

    #[test]
    fn try_transform_preserves_bezier_structure_and_flags() -> Result<()> {
        let geometry = BezierString::new(vec![BezierSegment::new(
            Coord { x: 0.0, y: 0.0 },
            Some((Coord { x: 0.0, y: 1.0 }, Coord { x: 1.0, y: 1.0 })),
            Coord { x: 1.0, y: 0.0 },
        )]);
        let path = BezierPath::new(geometry, vec![true, false])?;

        let path = path.try_transform(|coord| {
            Ok::<_, Error>(Coord {
                x: coord.x + 2.0,
                y: coord.y - 3.0,
            })
        })?;

        assert_eq!(path.vertex_is_dash_point(), [true, false]);
        assert_vertex_flag_count(&path);
        let Some(BezierSegment::Bezier(curve)) = path.geometry().segments().next() else {
            return Err(Error::InvalidBezierPath);
        };
        assert_eq!(curve.start, Coord { x: 2.0, y: -3.0 });
        assert_eq!(curve.handle1, Coord { x: 2.0, y: -2.0 });
        assert_eq!(curve.handle2, Coord { x: 3.0, y: -2.0 });
        assert_eq!(curve.end, Coord { x: 3.0, y: -3.0 });
        Ok(())
    }

    #[test]
    fn fitting_line_string_creates_curves_with_one_flag_per_vertex() -> Result<()> {
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

        assert!(
            path.geometry()
                .segments()
                .any(BezierSegment::is_bezier_curve)
        );
        assert_vertex_flag_count(&path);
        assert!(path.vertex_is_dash_point().iter().all(|flag| !flag));
        Ok(())
    }

    #[test]
    fn reversing_keeps_flags_on_their_vertices() -> Result<()> {
        let mut path = BezierPath::new(
            BezierString::new(vec![
                BezierSegment::new(Coord { x: 0.0, y: 0.0 }, None, Coord { x: 1.0, y: 0.0 }),
                BezierSegment::new(Coord { x: 1.0, y: 0.0 }, None, Coord { x: 2.0, y: 0.0 }),
            ]),
            vec![true, false, true],
        )?;
        path.reverse();
        assert_eq!(path.vertex_is_dash_point(), [true, false, true]);
        assert_vertex_flag_count(&path);
        assert_eq!(
            path.geometry().segments().next().map(BezierSegment::start),
            Some(Coord { x: 2.0, y: 0.0 })
        );
        Ok(())
    }
}
