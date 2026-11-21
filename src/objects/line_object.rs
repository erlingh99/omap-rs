use crate::Transformable as _;
use std::collections::HashMap;

use geo_types::Coord;

use super::{BezierPath, FlattenedPath};
use crate::{NonNegativeF64, Result, symbols::LinePathSymbolId};

/// A line object whose geometry retains straight and cubic segments.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineObject {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[expect(
        clippy::box_collection,
        reason = "the map header is 48 bytes inline and most objects carry no tags"
    )]
    tags: Option<Box<HashMap<String, String>>>,
    /// The line or combined-line symbol used to render this object.
    pub symbol: Option<LinePathSymbolId>,
    geometry: BezierPath,
}

impl LineObject {
    /// Create a line object from a Bézier path, flattened path, or line string.
    pub fn new(symbol: Option<LinePathSymbolId>, geometry: impl Into<BezierPath>) -> Self {
        Self {
            tags: None,
            symbol,
            geometry: geometry.into(),
        }
    }

    /// This object with the tags a file gave it.
    pub(crate) fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = (!tags.is_empty()).then(|| Box::new(tags));
        self
    }

    /// The tags associated with the object.
    pub fn tags(&self) -> &HashMap<String, String> {
        match &self.tags {
            Some(tags) => tags,
            None => super::empty_tags(),
        }
    }

    /// Mutably access the tags, allocating the map on first use.
    pub fn tags_mut(&mut self) -> &mut HashMap<String, String> {
        self.tags.get_or_insert_with(Box::default)
    }

    /// Get the mixed straight/cubic geometry.
    pub fn geometry(&self) -> &BezierPath {
        &self.geometry
    }

    /// Mutably access the mixed straight/cubic geometry.
    pub fn geometry_mut(&mut self) -> &mut BezierPath {
        &mut self.geometry
    }

    /// Consume the object and return its geometry.
    pub fn into_geometry(self) -> BezierPath {
        self.geometry
    }

    /// Create an owned flattened path with dash-point metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the tolerance is too small or the path is invalid.
    pub fn flatten(&self, allowed_error: NonNegativeF64) -> Result<FlattenedPath> {
        self.geometry.flatten(allowed_error)
    }

    /// Replace the path with straight segments from flattened geometry.
    pub fn replace_with_flattened(&mut self, geometry: FlattenedPath) {
        self.geometry = geometry.into();
    }

    /// Permanently replace curves with their flattened straight segments.
    ///
    /// # Errors
    ///
    /// Returns an error when flattening fails.
    pub fn flatten_in_place(&mut self, allowed_error: NonNegativeF64) -> Result<()> {
        self.geometry = self.flatten(allowed_error)?.into();
        Ok(())
    }

    /// Create a line object for use as a point-symbol element.
    pub fn new_element(geometry: impl Into<BezierPath>) -> Self {
        Self::new(None, geometry)
    }

    pub(crate) fn geometry_is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// Transform the geometry while preserving curves and dash points.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        self.geometry =
            std::mem::replace(&mut self.geometry, BezierPath::empty()).transform(transform);
    }

    /// Try to transform the geometry while preserving curves and dash points.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `transform`. The object is unchanged
    /// when transformation fails.
    pub fn try_transform<E, F>(&mut self, transform: F) -> std::result::Result<(), E>
    where
        F: Fn(Coord) -> std::result::Result<Coord, E>,
    {
        self.geometry = self.geometry.clone().try_transform(transform)?;
        Ok(())
    }

    /// Reverse the path while keeping dash flags attached to their vertices.
    pub fn reverse(&mut self) {
        self.geometry.reverse();
    }
}
