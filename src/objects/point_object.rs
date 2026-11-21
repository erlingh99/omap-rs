use std::collections::HashMap;

use geo_types::{Coord, Point};

use crate::{
    symbols::PointSymbolId,
    utils::{transform_position, try_transform_position},
};

/// A point object placed at a single location on the map.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointObject {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[expect(
        clippy::box_collection,
        reason = "the map header is 48 bytes inline and most objects carry no tags"
    )]
    tags: Option<Box<HashMap<String, String>>>,
    /// Rotation of the symbol in radians.
    pub rotation: f64,
    /// The point symbol used to render this object, or `None` for the
    /// format's unknown-symbol sentinel.
    pub symbol: Option<PointSymbolId>,
    geometry: Point,
}

impl PointObject {
    /// Create a new point object with the given symbol and position.
    pub fn new(symbol: Option<PointSymbolId>, geometry: Point) -> Self {
        Self {
            tags: None,
            rotation: 0.0,
            symbol,
            geometry,
        }
    }

    /// Create a point object for use as a point-symbol element.
    pub fn new_element(geometry: Point) -> Self {
        Self::new(None, geometry)
    }

    /// This object with the tags a file gave it.
    #[cfg(feature = "xml")]
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

    /// Get a shared reference to the point geometry.
    pub fn geometry(&self) -> &Point {
        &self.geometry
    }

    /// Get a mutable reference to the point geometry.
    pub fn geometry_mut(&mut self) -> &mut Point {
        &mut self.geometry
    }

    /// Transform the point position and symbol rotation.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        let (position, rotation, _) = transform_position(self.geometry.0, transform);
        self.geometry.0 = position;
        self.rotation += rotation;
    }

    /// Try to transform the point position and symbol rotation.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `transform`. The object is unchanged on
    /// failure.
    pub fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(Coord) -> Result<Coord, E>,
    {
        let (position, rotation, _) = try_transform_position(self.geometry.0, transform)?;
        self.geometry.0 = position;
        self.rotation += rotation;
        Ok(())
    }

    /// Consume this object and return its geometry.
    pub fn into_geometry(self) -> Point {
        self.geometry
    }
}
