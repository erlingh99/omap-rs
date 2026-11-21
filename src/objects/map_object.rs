use std::collections::HashMap;

use super::{AreaObject, LineObject, PointObject, TextObject};
use crate::symbols::SymbolId;

/// A map object that can be a point, line, area, or text.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MapObject {
    /// A point object.
    Point(PointObject),
    /// A line object.
    Line(LineObject),
    /// An area object.
    Area(AreaObject),
    /// A text object.
    Text(TextObject),
}

impl MapObject {
    /// The symbol this object is rendered with, or `None` for the format's
    /// unknown-symbol sentinel.
    pub fn symbol(&self) -> Option<SymbolId> {
        match self {
            Self::Point(object) => object.symbol.map(Into::into),
            Self::Line(object) => object.symbol.map(Into::into),
            Self::Area(object) => object.symbol.map(Into::into),
            Self::Text(object) => object.symbol.map(Into::into),
        }
    }

    /// Get the tags of the object
    pub fn tags(&self) -> &HashMap<String, String> {
        match self {
            Self::Point(o) => o.tags(),
            Self::Line(o) => o.tags(),
            Self::Area(o) => o.tags(),
            Self::Text(o) => o.tags(),
        }
    }

    /// Get mutable tags of the object
    pub fn tags_mut(&mut self) -> &mut HashMap<String, String> {
        match self {
            Self::Point(o) => o.tags_mut(),
            Self::Line(o) => o.tags_mut(),
            Self::Area(o) => o.tags_mut(),
            Self::Text(o) => o.tags_mut(),
        }
    }

    pub(crate) fn geometry_is_empty(&self) -> bool {
        match self {
            Self::Line(object) => object.geometry_is_empty(),
            Self::Area(object) => object.geometry_is_empty(),
            Self::Point(_) | Self::Text(_) => false,
        }
    }

    /// Transform this object, preserving Bézier control points for line and
    /// area objects.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(geo_types::Coord) -> geo_types::Coord,
    {
        match self {
            Self::Point(object) => object.transform(transform),
            Self::Line(object) => object.transform(transform),
            Self::Area(object) => object.transform(transform),
            Self::Text(object) => object.transform(transform),
        }
    }

    /// Try to transform this object, preserving Bézier control points for line
    /// and area objects.
    ///
    /// # Errors
    ///
    /// Returns any error produced while transforming the object. The object is
    /// unchanged on failure.
    pub fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(geo_types::Coord) -> Result<geo_types::Coord, E>,
    {
        match self {
            Self::Point(object) => object.try_transform(transform),
            Self::Line(object) => object.try_transform(transform),
            Self::Area(object) => object.try_transform(transform),
            Self::Text(object) => object.try_transform(transform),
        }
    }
}

impl From<AreaObject> for MapObject {
    fn from(value: AreaObject) -> Self {
        Self::Area(value)
    }
}

impl From<LineObject> for MapObject {
    fn from(value: LineObject) -> Self {
        Self::Line(value)
    }
}

impl From<PointObject> for MapObject {
    fn from(value: PointObject) -> Self {
        Self::Point(value)
    }
}

impl From<TextObject> for MapObject {
    fn from(value: TextObject) -> Self {
        Self::Text(value)
    }
}
