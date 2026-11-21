use std::collections::HashMap;

use crate::{
    NonNegativeF64,
    symbols::TextSymbolId,
    utils::{transform_position, try_transform_position},
};
use geo_types::Coord;

/// The geometry of a text object, which is either a single anchor or a wrap box.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextGeometry {
    /// A single anchor point.
    SingleAnchor(Coord),
    /// A rectangular bounding box for wrapped text.
    WrapBox(WrapBox),
}

impl TextGeometry {
    /// Get a shared reference to the anchor coordinate.
    pub fn anchor_coord(&self) -> &Coord {
        match self {
            Self::SingleAnchor(coord) => coord,
            Self::WrapBox(wrap_box) => &wrap_box.anchor,
        }
    }

    /// Get a mutable reference to the anchor coordinate.
    pub fn anchor_coord_mut(&mut self) -> &mut Coord {
        match self {
            Self::SingleAnchor(coord) => coord,
            Self::WrapBox(wrap_box) => &mut wrap_box.anchor,
        }
    }
}

/// A rectangular bounding box for wrapped text.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WrapBox {
    /// The anchor (origin) coordinate of the box.
    pub anchor: Coord,
    /// Width of the text box in mm
    pub width: NonNegativeF64,
    /// Height of the text box in mm
    pub height: NonNegativeF64,
}

/// Horizontal text alignment.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HorizontalAlign {
    /// Align to the left.
    Left = 0,
    /// Centre horizontally.
    #[default]
    HCenter = 1,
    /// Align to the right.
    Right = 2,
}

/// Vertical text alignment.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum VerticalAlign {
    /// Align to the text baseline.
    Baseline = 0,
    /// Align to the top.
    Top = 1,
    /// Centre vertically.
    #[default]
    VCenter = 2,
    /// Align to the bottom.
    Bottom = 3,
}

/// A text object placed on the map.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextObject {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[expect(
        clippy::box_collection,
        reason = "the map header is 48 bytes inline and most objects carry no tags"
    )]
    tags: Option<Box<HashMap<String, String>>>,
    /// Weak reference to the text symbol used to render this object.
    pub symbol: Option<TextSymbolId>,
    geometry: TextGeometry,
    /// The text content.
    pub text: String,
    /// Horizontal alignment.
    pub h_align: HorizontalAlign,
    /// Vertical alignment.
    pub v_align: VerticalAlign,
    /// Rotation of the text in radians.
    pub rotation: f64,
}

impl TextObject {
    /// Create a new text object with the given symbol, geometry, and text content.
    pub fn new(symbol: Option<TextSymbolId>, geometry: TextGeometry, text: String) -> Self {
        Self {
            tags: None,
            symbol,
            geometry,
            text,
            h_align: HorizontalAlign::default(),
            v_align: VerticalAlign::default(),
            rotation: 0.0,
        }
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

    /// Get a shared reference to the text geometry.
    pub fn geometry(&self) -> &TextGeometry {
        &self.geometry
    }

    /// Get a mutable reference to the text geometry.
    pub fn geometry_mut(&mut self) -> &mut TextGeometry {
        &mut self.geometry
    }

    /// Transform the text anchor and rotation.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        let anchor = match &self.geometry {
            TextGeometry::SingleAnchor(coord) => *coord,
            TextGeometry::WrapBox(wrap_box) => wrap_box.anchor,
        };
        let (anchor, rotation, _) = transform_position(anchor, transform);

        match &mut self.geometry {
            TextGeometry::SingleAnchor(coord) => *coord = anchor,
            TextGeometry::WrapBox(wrap_box) => wrap_box.anchor = anchor,
        }
        self.rotation += rotation;
    }

    /// Try to transform the text anchor and rotation.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `transform`. The object is unchanged on
    /// failure.
    pub fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(Coord) -> Result<Coord, E>,
    {
        let anchor = match &self.geometry {
            TextGeometry::SingleAnchor(coord) => *coord,
            TextGeometry::WrapBox(wrap_box) => wrap_box.anchor,
        };
        let (anchor, rotation, _) = try_transform_position(anchor, transform)?;

        match &mut self.geometry {
            TextGeometry::SingleAnchor(coord) => *coord = anchor,
            TextGeometry::WrapBox(wrap_box) => wrap_box.anchor = anchor,
        }
        self.rotation += rotation;
        Ok(())
    }

    /// Consume this object and return its geometry.
    pub fn into_geometry(self) -> TextGeometry {
        self.geometry
    }
}
