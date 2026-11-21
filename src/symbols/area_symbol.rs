use super::{PointSymbol, SymbolCommon};
use crate::{
    Code, NonNegativeF64,
    colors::{ColorId, SymbolColor},
};

/// A fill pattern applied to an area.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FillPattern {
    /// A pattern of parallel lines.
    LinePattern {
        /// Rotation angle of the pattern.
        angle: f64,
        /// Spacing between adjacent pattern lines.
        line_spacing: NonNegativeF64,
        /// Perpendicular offset of the pattern lines.
        line_offset: NonNegativeF64,
        /// Color of the pattern lines.
        line_color: SymbolColor,
        /// Width of the pattern lines.
        line_width: NonNegativeF64,
        /// Whether the pattern may be rotated with an object.
        rotatable: bool, // stored as flag 16 with the clip options
    },
    /// A pattern of regularly spaced points.
    PointPattern {
        /// How point elements are clipped at the area boundary.
        clip_options: ClippingOption,
        /// Rotation angle of the pattern.
        angle: f64,
        /// Spacing between rows of point elements.
        line_spacing: NonNegativeF64,
        /// Perpendicular offset of the point rows.
        line_offset: NonNegativeF64,
        /// Offset of the first point along each row.
        offset_along_line: NonNegativeF64,
        /// Spacing between point elements within a row.
        point_distance: NonNegativeF64,
        /// Point symbol repeated by the pattern.
        point: PointSymbol,
        /// Whether the pattern may be rotated with an object.
        rotatable: bool, // stored as flag 16 with the clip options
    },
}

/// Clipping option for point patterns at area boundaries.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ClippingOption {
    /// Clip elements at the boundary.
    #[default]
    ClipElementsAtBoundary = 0,
    /// No clipping if the element is completely inside.
    NoClippingIfCompletelyInside = 1,
    /// No clipping if the element centre is inside.
    NoClippingIfCenterInside = 2,
    /// No clipping if the element is at least partially inside.
    NoClippingIfPartiallyInside = 3,
}

/// An area symbol definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AreaSymbol {
    /// Common symbol properties.
    pub common: SymbolCommon,

    /// Whether the fill pattern is rotatable.
    pub is_rotatable: bool,

    /// The area fill colour.
    pub color: SymbolColor,
    /// Fill patterns applied to the area.
    pub patterns: Vec<FillPattern>,
    /// Minimum allowed area in mm².
    pub minimum_area: NonNegativeF64,
}

impl AreaSymbol {
    /// Get the display name of this area symbol.
    pub fn name(&self) -> &str {
        &self.common.name
    }

    /// Create a new empty area symbol with the given code and name.
    pub fn new(code: Code, name: impl Into<String>) -> Self {
        let common = SymbolCommon {
            code,
            name: name.into(),
            ..Default::default()
        };
        Self {
            common,
            is_rotatable: true,
            color: SymbolColor::NoColor,
            patterns: Vec::new(),
            minimum_area: NonNegativeF64::zero(),
        }
    }

    /// Set the fill colour (builder-style).
    pub fn with_color(mut self, color: SymbolColor) -> Self {
        self.color = color;
        self
    }

    /// Add a fill pattern (builder-style).
    pub fn with_pattern(mut self, pattern: FillPattern) -> Self {
        self.patterns.push(pattern);
        self
    }

    /// Set the minimum area in mm² (builder-style).
    pub fn with_minimum_area(mut self, area: NonNegativeF64) -> Self {
        self.minimum_area = area;
        self
    }

    /// Set whether the symbol is rotatable (builder-style).
    pub fn with_rotatable(mut self, rotatable: bool) -> Self {
        self.is_rotatable = rotatable;
        self
    }

    /// Mark as a helper symbol (builder-style).
    pub fn as_helper_symbol(mut self) -> Self {
        self.common.is_helper_symbol = true;
        self
    }

    /// Every color used in this symbol definition, its patterns' included.
    pub fn colors(&self) -> impl Iterator<Item = ColorId> {
        crate::references::own_colors(self)
    }
}
