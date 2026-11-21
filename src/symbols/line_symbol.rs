use super::{PointSymbol, SymbolCommon};
use crate::{
    Code, NonNegativeF64,
    colors::{ColorId, SymbolColor},
};

/// A line symbol definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineSymbol {
    /// Common symbol properties.
    pub common: SymbolCommon,

    /// Optional border styling.
    pub border: Option<BorderStyle>,

    /// Optional point symbol placed at the start of the line.
    pub start_symbol: Option<Box<PointSymbol>>,
    /// Optional point symbol placed repeatedly along the line.
    pub mid_symbol: Option<Box<MidSymbol>>,
    /// Optional point symbol placed at the end of the line.
    pub end_symbol: Option<Box<PointSymbol>>,
    /// Optional point symbol placed on dashes.
    pub dash_symbol: Option<Box<DashSymbol>>,

    /// The line colour.
    pub color: SymbolColor,
    /// The line width in mm.
    pub line_width: NonNegativeF64,
    /// Minimum allowed length in mm.
    pub minimum_length: NonNegativeF64,
    /// Offset applied at the start of the line.
    pub start_offset: NonNegativeF64,
    /// Offset applied at the end of the line.
    pub end_offset: NonNegativeF64,

    /// The dash style for this line.
    pub dash_style: DashStyle,
    /// The cap style for line ends.
    pub cap_style: CapStyle,
    /// The join style at line vertices.
    pub join_style: JoinStyle,
}

impl LineSymbol {
    /// Create a new empty line symbol with the given code and name.
    pub fn new(code: Code, name: impl Into<String>) -> Self {
        let common = SymbolCommon {
            code,
            name: name.into(),
            ..Default::default()
        };
        Self {
            common,
            border: None,
            start_symbol: None,
            mid_symbol: None,
            end_symbol: None,
            dash_symbol: None,
            color: SymbolColor::NoColor,
            line_width: NonNegativeF64::default(),
            minimum_length: NonNegativeF64::default(),
            start_offset: NonNegativeF64::default(),
            end_offset: NonNegativeF64::default(),
            dash_style: DashStyle::default(),
            cap_style: CapStyle::default(),
            join_style: JoinStyle::default(),
        }
    }

    /// Get the display name of this line symbol.
    pub fn name(&self) -> &str {
        &self.common.name
    }

    /// Set the line colour (builder-style).
    pub fn with_color(mut self, color: SymbolColor) -> Self {
        self.color = color;
        self
    }

    /// Set the line width in mm (builder-style).
    pub fn with_line_width(mut self, width: NonNegativeF64) -> Self {
        self.line_width = width;
        self
    }

    /// Set the start symbol (builder-style).
    pub fn with_start_symbol(mut self, symbol: PointSymbol) -> Self {
        self.start_symbol = Some(Box::new(symbol));
        self
    }

    /// Set the mid symbol (builder-style).
    pub fn with_mid_symbol(mut self, symbol: MidSymbol) -> Self {
        self.mid_symbol = Some(Box::new(symbol));
        self
    }

    /// Set the end symbol (builder-style).
    pub fn with_end_symbol(mut self, symbol: PointSymbol) -> Self {
        self.end_symbol = Some(Box::new(symbol));
        self
    }

    /// Set the dash symbol (builder-style).
    pub fn with_dash_symbol(mut self, symbol: DashSymbol) -> Self {
        self.dash_symbol = Some(Box::new(symbol));
        self
    }

    /// Set the border style (builder-style).
    pub fn with_border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    /// Set the cap style (builder-style).
    pub fn with_cap_style(mut self, cap: CapStyle) -> Self {
        self.cap_style = cap;
        self
    }

    /// Set the join style (builder-style).
    pub fn with_join_style(mut self, join: JoinStyle) -> Self {
        self.join_style = join;
        self
    }

    /// Set the dash style (builder-style).
    pub fn with_dash_style(mut self, dash: DashStyle) -> Self {
        self.dash_style = dash;
        self
    }

    /// Mark as a helper symbol (builder-style).
    pub fn as_helper_symbol(mut self) -> Self {
        self.common.is_helper_symbol = true;
        self
    }

    /// Every color used in this symbol definition, its borders' and point
    /// symbols' included.
    pub fn colors(&self) -> impl Iterator<Item = ColorId> {
        crate::references::own_colors(self)
    }
}

/// A dash symbol placed on dashes of a dashed line.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DashSymbol {
    /// Whether to suppress the dash symbol at the line ends.
    pub suppress_dash_symbol_at_ends: bool,
    /// Whether the dash symbol is scaled with the dash length.
    pub scale_dash_symbol: bool,
    /// The point symbol placed on each dash.
    pub dash_symbol: PointSymbol,
}

/// Point symbols placed at regular positions along a line.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidSymbol {
    /// Number of mid symbols per placement spot.
    pub mid_symbols_per_spot: u16,
    /// Distance in mm
    pub mid_symbol_distance: NonNegativeF64,
    /// Min number of mid symbols
    pub minimum_mid_symbol_count: u16,
    /// Min number of mid symbols if the line is closed
    pub minimum_mid_symbol_count_when_closed: u16,
    /// Whether to always show at least 1 mid symbol
    pub show_at_least_one_mid_symbol: bool,
    /// How to place the mid symbols
    pub mid_symbol_placement: MidSymbolPlacement,
    /// The mid symbol point symbol
    pub mid_symbol: PointSymbol,
}

/// Cap style for line endpoints.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CapStyle {
    /// Flat cap (no extension beyond endpoint).
    #[default]
    Flat = 0,
    /// Round cap (semi-circular extension).
    Round = 1,
    /// Square cap (rectangular extension).
    Square = 2,
    /// Pointed cap (tapered extension).
    Pointed = 3,
}

/// Join style at line vertices.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum JoinStyle {
    /// Bevel join.
    Bevel = 0,
    /// Miter join.
    #[default]
    Miter = 1,
    /// Round join.
    Round = 2,
}

/// Placement of mid symbols relative to the line's dash pattern.
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MidSymbolPlacement {
    /// Mid symbols on every dash
    #[default]
    CenterOfDash = 0,
    /// Mid symbols on the center of a dash group
    CenterOfDashGroup = 1,
    /// Mid symbols on the main gap (i.e. not between dashes in a group)
    CenterOfGap = 2,
}

/// Whether the border is symmetric (same on both sides) or asymmetric.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BorderStyle {
    /// Both sides share the same border definition.
    SymmetricBorder {
        /// The shared border.
        both: LineSymbolBorder,
    },
    /// Left and right borders have different definitions.
    AsymmetricBorder {
        /// The left border.
        left: LineSymbolBorder,
        /// The right border.
        right: LineSymbolBorder,
    },
}

/// A single border line definition alongside the main line.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineSymbolBorder {
    /// Border colour.
    pub color: SymbolColor,
    /// Border width in mm.
    pub width: NonNegativeF64,
    /// Shift from the main line in mm.
    pub shift: NonNegativeF64,
    /// Optional dashed pattern for the border.
    pub dashed: Option<BorderDash>,
}

/// Dash parameters for a border line.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BorderDash {
    /// Length of each dash in mm.
    pub dash_length: NonNegativeF64,
    /// Length of the gap between dashes in mm.
    pub break_length: NonNegativeF64,
}

/// The dash style of a line symbol.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DashStyle {
    /// The line is dashed.
    Dashed {
        /// Length of each dash in mm.
        dash_length: NonNegativeF64,
        /// Length of the gap between dashes in mm.
        break_length: NonNegativeF64,
        /// Grouping of dashes.
        dash_group: GroupDashes,
    },
    /// The line is not dashed.
    NotDashed {
        /// Segment length in mm.
        segment_length: NonNegativeF64,
        /// End length in mm.
        end_length: NonNegativeF64,
    },
}

impl Default for DashStyle {
    fn default() -> Self {
        Self::NotDashed {
            segment_length: NonNegativeF64::clamped_from(4.),
            end_length: NonNegativeF64::default(),
        }
    }
}

/// Grouping of dashes in a dash style.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GroupDashes {
    /// Dashes are grouped together.
    Grouped {
        /// allowed to be in range 2..=4
        dashes_in_group: u8,
        /// Length of the gap between dashes within a group.
        in_group_break_length: NonNegativeF64,
    },
    /// Dashes are not grouped.
    UnGrouped {
        /// Whether the first and last dashes use half their normal length.
        half_outer_dashes: bool,
    },
}

impl Default for GroupDashes {
    fn default() -> Self {
        Self::UnGrouped {
            half_outer_dashes: false,
        }
    }
}
