use geo_types::Coord;

use super::SymbolCommon;
use crate::{
    Code, NonNegativeF64,
    colors::{ColorId, SymbolColor},
};

/// The framing mode for a text symbol.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FramingMode {
    /// No framing.
    #[default]
    NoFraming,
    /// An outline framing around each character.
    LineFraming(LineFraming),
    /// A shadow behind the text.
    ShadowFraming(ShadowFraming),
}

impl FramingMode {
    /// Get the numeric identifier for this framing mode.
    pub fn id(&self) -> u8 {
        match self {
            Self::NoFraming => 0,
            Self::LineFraming(_) => 1,
            Self::ShadowFraming(_) => 2,
        }
    }
}

/// Line-based framing (halo) around text characters.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineFraming {
    /// Color of the framing line.
    pub color: SymbolColor,
    /// Half-width of the framing line.
    pub framing_line_half_width: NonNegativeF64,
}

/// Shadow framing behind text characters.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShadowFraming {
    /// Color of the shadow.
    pub color: SymbolColor,
    /// Offset of the shadow from the text.
    pub shadow_offset: Coord<f64>,
}

/// A line drawn below the text (underline).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineBelow {
    /// Color of the line.
    pub color: SymbolColor,
    /// Width of the line.
    pub width: NonNegativeF64,
    /// Distance between the text and the line.
    pub distance: NonNegativeF64,
}

/// A text symbol definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextSymbol {
    /// The common symbol fields
    pub common: SymbolCommon,
    /// f.ex Arial
    pub font_family: String,
    /// Should not be more than 3 chars long
    pub icon_text: String,
    /// Color of the text
    pub color: SymbolColor,

    /// OCD custom tab positions in mm
    pub custom_tabs: Vec<NonNegativeF64>,
    /// OCD underlining
    pub line_below: Option<LineBelow>,

    /// as factor of original line spacing
    pub line_spacing: NonNegativeF64,
    /// as a factor of the space character width
    pub character_spacing: f64,
    /// this defines the font size in mm. How big the letters really are depends on the design of the font though
    pub font_size: NonNegativeF64,
    /// Spacing between paragraphs in mm.
    pub paragraph_spacing: f64,
    /// The framing mode (outline, shadow, or none).
    pub framing_mode: Option<FramingMode>,

    /// is the text allowed to be rotated
    pub is_rotatable: bool,
    /// bold text
    pub bold: bool,
    /// italix text
    pub italic: bool,
    /// underlined text
    pub underline: bool,
    /// kerning (adaptive character spacing for better readability)
    pub kerning: bool,
}

impl TextSymbol {
    /// Create a new text symbol with the given code, name, and font family.
    pub fn new(code: Code, name: impl Into<String>) -> Self {
        let common = SymbolCommon {
            code,
            name: name.into(),
            ..Default::default()
        };
        Self {
            common,
            font_family: String::from("Arial"),
            icon_text: String::new(),
            color: SymbolColor::NoColor,
            custom_tabs: Vec::new(),
            line_below: None,
            line_spacing: NonNegativeF64::one(),
            character_spacing: 0.0,
            font_size: NonNegativeF64::clamped_from(4.0),
            paragraph_spacing: 0.0,
            framing_mode: None,
            is_rotatable: false,
            bold: false,
            italic: false,
            underline: false,
            kerning: true,
        }
    }

    /// Get the display name of this text symbol.
    pub fn name(&self) -> &str {
        &self.common.name
    }

    /// Set the font family (builder-style).
    pub fn with_font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Set the font size in mm (builder-style).
    pub fn with_font_size(mut self, size: NonNegativeF64) -> Self {
        self.font_size = size;
        self
    }

    /// Set the text colour (builder-style).
    pub fn with_color(mut self, color: SymbolColor) -> Self {
        self.color = color;
        self
    }

    /// Set bold style (builder-style).
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Set italic style (builder-style).
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Set whether the symbol is rotatable (builder-style).
    pub fn with_rotatable(mut self, rotatable: bool) -> Self {
        self.is_rotatable = rotatable;
        self
    }

    /// Set line spacing as factor of original (builder-style).
    pub fn with_line_spacing(mut self, spacing: NonNegativeF64) -> Self {
        self.line_spacing = spacing;
        self
    }

    /// Mark as a helper symbol (builder-style).
    pub fn as_helper_symbol(mut self) -> Self {
        self.common.is_helper_symbol = true;
        self
    }

    /// Every color used in this symbol definition.
    pub fn colors(&self) -> impl Iterator<Item = ColorId> {
        crate::references::own_colors(self)
    }
}
