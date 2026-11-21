//! Text symbols, and line text symbols read as hidden text symbols.
//!
//! OCAD aligns text by symbol, the map model by object, so a text symbol's
//! alignment is kept beside it for its objects.

use std::io::Cursor;

use binrw::BinRead as _;
use geo_types::Coord;

use super::raw::{BaseSymbol, BasicText, Framing, LineTextTail, TextTail};
use super::{ReadSymbol, Reader};
use crate::NonNegativeF64;
use crate::objects::{HorizontalAlign, VerticalAlign};
use crate::ocd::Result;
use crate::ocd::units::{length, mm};
use crate::symbols::{
    FramingMode, LineBelow, LineFraming, ShadowFraming, SymbolCommon, TextSymbol,
};

/// Millimetres in a typographic point.
const MM_PER_POINT: f64 = 25.4 / 72.;

impl Reader<'_> {
    pub(super) fn text_symbol(
        &mut self,
        common: SymbolCommon,
        base: &BaseSymbol,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ReadSymbol> {
        let tail = TextTail::read_le(cursor)?;
        let code = common.code;
        let mut symbol = TextSymbol::new(code, common.name.clone());
        symbol.common = common;
        let alignment = self.basic_text(&mut symbol, &tail.basic);
        symbol.framing_mode = self.framing(&symbol, &tail.framing);
        symbol.is_rotatable = base.flags & BaseSymbol::ROTATABLE != 0;

        // The file's line spacing is a percentage of the font size.
        symbol.line_spacing = NonNegativeF64::clamped_from(f64::from(tail.line_spacing) / 100.);
        symbol.paragraph_spacing = mm(tail.paragraph_spacing);
        if tail.line_below_on != 0 {
            symbol.line_below = Some(LineBelow {
                color: if tail.line_below_width > 0 {
                    self.color(tail.line_below_color)
                } else {
                    crate::colors::SymbolColor::NoColor
                },
                width: length(tail.line_below_width),
                distance: length(tail.line_below_offset),
            });
        }
        let tab_count = usize::from(tail.tab_count).min(tail.tabs.len());
        symbol.custom_tabs = tail.tabs[..tab_count]
            .iter()
            .map(|&tab| length(tab))
            .collect();
        if tail.indent_first_line != 0 || tail.indent_other_lines != 0 {
            self.symbol_issue(code, "text indents are not supported");
        }
        if tail.framing.point_symbol_on != 0 {
            self.symbol_issue(code, "the point symbol framing the text was skipped");
        }

        let mut read = ReadSymbol::new(symbol);
        read.text_alignment = Some(alignment);
        Ok(read)
    }

    /// A line text symbol, read as a hidden text symbol, as Mapper does.
    pub(super) fn line_text_symbol(
        &mut self,
        common: SymbolCommon,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ReadSymbol> {
        let tail = LineTextTail::read_le(cursor)?;
        let code = common.code;
        let mut symbol = TextSymbol::new(code, common.name.clone());
        symbol.common = common;
        symbol.common.is_hidden = true;
        let alignment = self.basic_text(&mut symbol, &tail.basic);
        symbol.framing_mode = self.framing(&symbol, &tail.framing);
        self.symbol_issue(code, "line text is read as hidden text along no line");
        let mut read = ReadSymbol::new(symbol);
        read.text_alignment = Some(alignment);
        Ok(read)
    }

    /// Font, color, size and spacing, and the alignment the symbol's objects
    /// get.
    fn basic_text(
        &mut self,
        symbol: &mut TextSymbol,
        basic: &BasicText,
    ) -> (HorizontalAlign, VerticalAlign) {
        let code = symbol.common.code;
        symbol.font_family = basic.font_name.text();
        symbol.color = self.color(basic.color);
        symbol.font_size =
            NonNegativeF64::clamped_from(f64::from(basic.font_size) / 10. * MM_PER_POINT);
        symbol.bold = basic.weight >= 550;
        symbol.italic = basic.italic != 0;
        symbol.underline = false;
        symbol.kerning = false;
        symbol.character_spacing = f64::from(basic.character_spacing) / 100.;
        if basic.weight != 400 && basic.weight != 700 {
            self.symbol_issue(code, "a font weight other than normal or bold was rounded");
        }
        if basic.word_spacing != 100 {
            self.symbol_issue(code, "custom word spacing is not supported");
        }

        let horizontal = match basic.alignment & BasicText::HORIZONTAL_MASK {
            BasicText::LEFT => HorizontalAlign::Left,
            BasicText::RIGHT => HorizontalAlign::Right,
            BasicText::CENTER => HorizontalAlign::HCenter,
            _ => {
                self.symbol_issue(code, "justified text is read as centered");
                HorizontalAlign::HCenter
            }
        };
        let vertical = match basic.alignment & BasicText::VERTICAL_MASK {
            BasicText::TOP => VerticalAlign::Top,
            BasicText::MIDDLE => VerticalAlign::VCenter,
            BasicText::BOTTOM => VerticalAlign::Baseline,
            _ => {
                self.symbol_issue(code, "an unknown vertical alignment is read as baseline");
                VerticalAlign::Baseline
            }
        };
        (horizontal, vertical)
    }

    fn framing(&mut self, symbol: &TextSymbol, framing: &Framing) -> Option<FramingMode> {
        match framing.mode {
            Framing::NONE => None,
            Framing::SHADOW => Some(FramingMode::ShadowFraming(ShadowFraming {
                color: self.color(framing.color),
                shadow_offset: Coord {
                    x: mm(framing.offset_x),
                    y: mm(framing.offset_y),
                },
            })),
            Framing::LINE => {
                // 0 is OCAD 8's default, miter; 4 is miter.
                if framing.line_style != 0 && framing.line_style != 4 {
                    self.symbol_issue(
                        symbol.common.code,
                        "the framing line's joins are drawn mitered",
                    );
                }
                Some(FramingMode::LineFraming(LineFraming {
                    color: self.color(framing.color),
                    framing_line_half_width: length(framing.line_width),
                }))
            }
            _ => {
                self.symbol_issue(symbol.common.code, "rectangle framing of text was skipped");
                None
            }
        }
    }
}
