use std::convert::Infallible;

use geo_types::{Coord, LineString, Point, Polygon};
use linestring2bezier::{BezierSegment, BezierString};

/// A geometry whose every coordinate can be mapped to a new one, keeping its
/// shape: what [`crate::geo_referencing::MapTransform`] converts between map,
/// projected and WGS84 coordinates.
///
/// Implemented for [`Coord`], [`Point`], [`LineString`], [`Polygon`],
/// [`BezierString`], and this crate's Bézier paths and polygons.
pub trait Transformable: Sized {
    /// Map every coordinate through `transform`.
    ///
    /// Equal input coordinates should map to equal output coordinates, so a
    /// path stays connected.
    fn transform(self, transform: impl Fn(Coord) -> Coord) -> Self {
        match self.try_transform(|coord| Ok::<_, Infallible>(transform(coord))) {
            Ok(geometry) => geometry,
            Err(never) => match never {},
        }
    }

    /// Map every coordinate through `transform`, stopping at the first error.
    ///
    /// # Errors
    ///
    /// Returns the first error `transform` produces.
    fn try_transform<E>(self, transform: impl Fn(Coord) -> Result<Coord, E>) -> Result<Self, E>;
}

/// Transform every element of `items`, stopping at the first error.
pub(crate) fn try_transform_all<T: Transformable, E>(
    items: Vec<T>,
    transform: impl Fn(Coord) -> Result<Coord, E>,
) -> Result<Vec<T>, E> {
    items
        .into_iter()
        .map(|item| item.try_transform(&transform))
        .collect()
}

impl Transformable for Coord {
    fn try_transform<E>(self, transform: impl Fn(Self) -> Result<Self, E>) -> Result<Self, E> {
        transform(self)
    }
}

impl Transformable for Point {
    fn try_transform<E>(self, transform: impl Fn(Coord) -> Result<Coord, E>) -> Result<Self, E> {
        Ok(Self(transform(self.0)?))
    }
}

impl Transformable for LineString {
    fn try_transform<E>(self, transform: impl Fn(Coord) -> Result<Coord, E>) -> Result<Self, E> {
        Ok(Self(try_transform_all(self.0, transform)?))
    }
}

impl Transformable for Polygon {
    fn try_transform<E>(self, transform: impl Fn(Coord) -> Result<Coord, E>) -> Result<Self, E> {
        let (exterior, interiors) = self.into_inner();
        Ok(Self::new(
            exterior.try_transform(&transform)?,
            try_transform_all(interiors, transform)?,
        ))
    }
}

/// Every endpoint and Bézier handle is transformed.
impl Transformable for BezierString {
    fn try_transform<E>(
        mut self,
        transform: impl Fn(Coord) -> Result<Coord, E>,
    ) -> Result<Self, E> {
        for segment in self.segments_mut() {
            match segment {
                BezierSegment::Bezier(curve) => {
                    curve.start = transform(curve.start)?;
                    curve.handle1 = transform(curve.handle1)?;
                    curve.handle2 = transform(curve.handle2)?;
                    curve.end = transform(curve.end)?;
                }
                BezierSegment::Line(line) => {
                    line.start = transform(line.start)?;
                    line.end = transform(line.end)?;
                }
            }
        }
        Ok(self)
    }
}
