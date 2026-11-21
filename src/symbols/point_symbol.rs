use super::{AreaSymbol, LineSymbol};
use crate::{
    Code, NonNegativeF64,
    colors::{ColorId, SymbolColor},
    objects::{AreaObject, LineObject, PointObject},
    symbols::SymbolCommon,
};

/// An element within a point symbol definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Element {
    /// A nested point sub-symbol with its object.
    Point {
        /// The point sub-symbol.
        symbol: Box<PointSymbol>,
        /// The object rendered by this element.
        object: Box<PointObject>,
    },
    /// A line sub-symbol with its object.
    Line {
        /// The line sub-symbol.
        symbol: Box<LineSymbol>,
        /// The object rendered by this element.
        object: Box<LineObject>,
    },
    /// An area sub-symbol with its object.
    Area {
        /// The area sub-symbol.
        symbol: Box<AreaSymbol>,
        /// The object rendered by this element.
        object: Box<AreaObject>,
    },
}

impl Element {
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Point { symbol, .. } => {
                symbol.inner_color == SymbolColor::NoColor
                    && symbol.outer_color == SymbolColor::NoColor
                    && symbol.elements.is_empty()
            }
            Self::Line { object, .. } => object.geometry_is_empty(),
            Self::Area { object, .. } => object.geometry_is_empty(),
        }
    }
}

/// A point symbol definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointSymbol {
    /// Common symbol properties.
    pub common: SymbolCommon,

    /// Whether the symbol is rotatable.
    pub is_rotatable: bool,
    /// The graphical elements that make up this symbol.
    pub elements: Vec<Element>,

    /// Inner circle colour.
    pub inner_color: SymbolColor,
    /// Outer ring colour.
    pub outer_color: SymbolColor,
    /// Inner circle radius in mm.
    pub inner_radius: NonNegativeF64,
    /// Outer ring width in mm.
    pub outer_width: NonNegativeF64,
}

impl PointSymbol {
    /// Create a new empty point symbol with the given code and name.
    pub fn new(code: Code, name: impl Into<String>) -> Self {
        let common = SymbolCommon {
            code,
            name: name.into(),
            ..Default::default()
        };
        Self {
            common,
            is_rotatable: true,
            elements: Vec::new(),
            inner_color: SymbolColor::NoColor,
            outer_color: SymbolColor::NoColor,
            inner_radius: NonNegativeF64::default(),
            outer_width: NonNegativeF64::default(),
        }
    }

    /// Get the display name of this point symbol.
    pub fn name(&self) -> &str {
        &self.common.name
    }

    /// Set the inner circle colour (builder-style).
    pub fn with_inner_color(mut self, color: SymbolColor) -> Self {
        self.inner_color = color;
        self
    }

    /// Set the outer ring colour (builder-style).
    pub fn with_outer_color(mut self, color: SymbolColor) -> Self {
        self.outer_color = color;
        self
    }

    /// Set the inner circle radius in mm (builder-style).
    pub fn with_inner_radius(mut self, radius: NonNegativeF64) -> Self {
        self.inner_radius = radius;
        self
    }

    /// Set the outer ring width in mm (builder-style).
    pub fn with_outer_width(mut self, width: NonNegativeF64) -> Self {
        self.outer_width = width;
        self
    }

    /// Add a graphical element (builder-style).
    pub fn with_element(mut self, element: Element) -> Self {
        if !element.is_empty() {
            self.elements.push(element);
        }
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

    /// Every color used in this symbol definition, its elements' included.
    pub fn colors(&self) -> impl Iterator<Item = ColorId> {
        crate::references::own_colors(self)
    }
}
