//! Line symbols, and rectangle symbols read as lines.
//!
//! One OCAD line symbol can draw a main line, a framing line around it and a
//! double line with borders. The map model draws each with a line symbol of
//! its own, so a symbol that needs more than one becomes a combined line
//! symbol of private parts, as Mapper reads it: main line, then double line,
//! then framing.

use std::io::Cursor;

use binrw::BinRead as _;

use super::point::is_empty;
use super::raw::{LineTail, RectangleTail, read_elements};
use super::{ReadSymbol, Reader, line_styles};
use crate::Code;
use crate::NonNegativeF64;
use crate::colors::SymbolColor;
use crate::ocd::Result;
use crate::ocd::units::length;
use crate::symbols::{
    BorderDash, BorderStyle, CapStyle, CombinedLineSymbol, DashStyle, DashSymbol, GroupDashes,
    JoinStyle, LineSymbol, LineSymbolBorder, MidSymbol, MidSymbolPlacement, PublicOrPrivateSymbol,
    Symbol, SymbolCommon,
};

impl Reader<'_> {
    /// A line symbol, or a combined line symbol if it has a framing or double
    /// line besides its main line.
    pub(super) fn line_symbol(
        &mut self,
        common: SymbolCommon,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<Symbol> {
        let tail = LineTail::read_le(cursor)?;
        let code = common.code;
        if tail.decrease_mode != 0 {
            self.symbol_issue(
                code,
                "symbols decreasing along the line are drawn at full size",
            );
        }

        let mut main = LineSymbol::new(code, common.name.clone());
        main.common = common.clone();
        self.main_line(&mut main, &tail);
        self.line_point_symbols(&mut main, &tail, cursor)?;

        let framing = if tail.framing_width > 0 {
            if draws_on_its_own(&main) {
                let mut framing = LineSymbol::new(code, common.name.clone());
                self.framing_line(&mut framing, &tail);
                Some(framing)
            } else {
                // Nothing else draws with the main line, so it can be the
                // framing.
                self.framing_line(&mut main, &tail);
                None
            }
        } else {
            None
        };

        let double = if is_visible_double_line(&tail) {
            if matches!(main.dash_style, DashStyle::Dashed { .. }) || draws_on_its_own(&main) {
                let mut double = LineSymbol::new(code, common.name.clone());
                self.double_line(&mut double, &tail);
                Some(double)
            } else {
                self.double_line(&mut main, &tail);
                None
            }
        } else {
            None
        };

        if tail.double_flags & LineTail::DOUBLE_BACKGROUND_ON != 0 {
            self.symbol_issue(code, "the double line's background color is not supported");
        }

        if framing.is_none() && double.is_none() {
            return Ok(main.into());
        }
        let mut combined = CombinedLineSymbol::new(code, common.name.clone());
        combined.common = common;
        let parts = [
            Some((main, " - main line")),
            double.map(|d| (d, " - double line")),
            framing.map(|f| (f, " - framing")),
        ];
        for (mut part, suffix) in parts.into_iter().flatten() {
            part.common.name.push_str(suffix);
            part.common.is_hidden = false;
            part.common.is_protected = false;
            combined.parts.push(PublicOrPrivateSymbol::Private(part));
        }
        Ok(combined.into())
    }

    /// Width, color, caps, joins, offsets and dashes of the main line.
    fn main_line(&mut self, symbol: &mut LineSymbol, tail: &LineTail) {
        let code = symbol.common.code;
        symbol.line_width = length(tail.line_width);
        symbol.color = if tail.line_width > 0 {
            self.color(tail.line_color)
        } else {
            SymbolColor::NoColor
        };
        let (cap_style, join_style) = line_styles(tail.line_style).unwrap_or_else(|| {
            self.symbol_issue(
                code,
                "an unknown line style was read as bevel joins and flat caps",
            );
            (CapStyle::Flat, JoinStyle::Bevel)
        });
        symbol.cap_style = cap_style;
        // OCAD draws round joins whenever the caps are pointed.
        symbol.join_style = if matches!(cap_style, CapStyle::Pointed) {
            JoinStyle::Round
        } else {
            join_style
        };
        symbol.start_offset = length(tail.distance_from_start);
        symbol.end_offset = length(tail.distance_to_end);
        symbol.dash_style = self.dash_style(code, tail);
    }

    /// The dash pattern, read as Mapper reads it.
    fn dash_style(&mut self, code: Code, tail: &LineTail) -> DashStyle {
        let main_length = i32::from(tail.main_length);
        let end_length = i32::from(tail.end_length);
        if tail.main_gap == 0 && tail.secondary_gap == 0 {
            return DashStyle::NotDashed {
                segment_length: length(tail.main_length),
                end_length: length(tail.end_length),
            };
        }
        if main_length == 0 {
            self.symbol_issue(code, "the dash pattern has no dash length and was not read");
            return DashStyle::default();
        }
        let end_length_is_half = (main_length - 2 * end_length).abs() <= 1;
        if tail.main_gap == 0 {
            // Only the secondary gap: one dash per group, the gap swapped.
            if end_length != 0 && !end_length_is_half {
                self.symbol_issue(code, "the dash pattern's end length was approximated");
            }
            if end_length != 0 && tail.end_gap != 0 {
                self.symbol_issue(code, "the dash pattern's end gap was approximated");
            }
            return DashStyle::Dashed {
                dash_length: length(main_length - i32::from(tail.secondary_gap)),
                break_length: length(tail.secondary_gap),
                dash_group: GroupDashes::UnGrouped {
                    half_outer_dashes: false,
                },
            };
        }
        let distinct_end = end_length != 0 && end_length != main_length;
        if distinct_end && !end_length_is_half {
            self.symbol_issue(code, "the dash pattern's end length was approximated");
        }
        if tail.secondary_gap == 0 {
            return DashStyle::Dashed {
                dash_length: length(tail.main_length),
                break_length: length(tail.main_gap),
                dash_group: GroupDashes::UnGrouped {
                    half_outer_dashes: distinct_end
                        && f64::from(end_length) / f64::from(main_length) <= 0.75,
                },
            };
        }
        if end_length != 0 && tail.end_gap != tail.secondary_gap {
            self.symbol_issue(code, "the dash pattern's end gap was approximated");
        }
        let in_group_break = i32::from(tail.secondary_gap);
        DashStyle::Dashed {
            dash_length: length(f64::from(main_length - in_group_break) / 2.),
            break_length: length(tail.main_gap),
            dash_group: GroupDashes::Grouped {
                dashes_in_group: 2,
                in_group_break_length: length(in_group_break),
            },
        }
    }

    /// The point symbols along the line, at its corners and at its ends,
    /// which follow the line symbol in the file.
    fn line_point_symbols(
        &mut self,
        symbol: &mut LineSymbol,
        tail: &LineTail,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<()> {
        let code = symbol.common.code;
        let primary = read_elements(cursor, tail.primary_data_size)?;
        let secondary = read_elements(cursor, tail.secondary_data_size)?;
        let corner = read_elements(cursor, tail.corner_data_size)?;
        let start = read_elements(cursor, tail.start_data_size)?;
        let end = read_elements(cursor, tail.end_data_size)?;

        // With only a secondary gap, Mapper swaps the gaps.
        let gaps_swapped = tail.secondary_gap != 0 && tail.main_gap == 0 && tail.main_length != 0;
        let mid = if !primary.is_empty() {
            if !secondary.is_empty() {
                self.symbol_issue(code, "the secondary point symbol was skipped");
            }
            Some(MidSymbol {
                mid_symbols_per_spot: u16::try_from(tail.primary_symbol_count).unwrap_or(0),
                mid_symbol_distance: length(tail.primary_symbol_distance),
                minimum_mid_symbol_count: 0,
                minimum_mid_symbol_count_when_closed: 0,
                show_at_least_one_mid_symbol: true,
                mid_symbol_placement: if gaps_swapped {
                    MidSymbolPlacement::CenterOfDashGroup
                } else {
                    MidSymbolPlacement::CenterOfGap
                },
                mid_symbol: self.point_pattern(code, &primary),
            })
        } else if !secondary.is_empty() {
            Some(MidSymbol {
                mid_symbols_per_spot: 1,
                mid_symbol_distance: NonNegativeF64::zero(),
                minimum_mid_symbol_count: 0,
                minimum_mid_symbol_count_when_closed: 0,
                show_at_least_one_mid_symbol: true,
                mid_symbol_placement: if gaps_swapped {
                    MidSymbolPlacement::CenterOfGap
                } else {
                    MidSymbolPlacement::CenterOfDashGroup
                },
                mid_symbol: self.point_pattern(code, &secondary),
            })
        } else {
            None
        };
        symbol.mid_symbol = mid.map(Box::new);

        if !start.is_empty() {
            symbol.start_symbol = Some(Box::new(self.point_pattern(code, &start)));
        }
        if !end.is_empty() {
            symbol.end_symbol = Some(Box::new(self.point_pattern(code, &end)));
        }
        if !corner.is_empty() {
            // OCAD leaves out the corner symbol at the ends when the line has
            // both a start and an end symbol.
            let suppress = symbol.start_symbol.is_some() && symbol.end_symbol.is_some();
            if suppress && code.major != 799 {
                self.symbol_issue(code, "the dash symbol is suppressed at the line ends");
            }
            symbol.dash_symbol = Some(Box::new(DashSymbol {
                suppress_dash_symbol_at_ends: suppress,
                scale_dash_symbol: true,
                dash_symbol: self.point_pattern(code, &corner),
            }));
        }
        Ok(())
    }

    /// Make `symbol` the framing line.
    fn framing_line(&mut self, symbol: &mut LineSymbol, tail: &LineTail) {
        symbol.line_width = length(tail.framing_width);
        symbol.color = self.color(tail.framing_color);
        match tail.framing_style {
            LineTail::BEVEL_FLAT | LineTail::ROUND_ROUND | LineTail::MITER_FLAT => {
                if let Some((cap_style, join_style)) = line_styles(tail.framing_style) {
                    symbol.cap_style = cap_style;
                    symbol.join_style = join_style;
                }
            }
            _ => self.symbol_issue(
                symbol.common.code,
                "the framing line style is not supported",
            ),
        }
    }

    /// Make `symbol` the double line: its filling, borders and their dashes.
    fn double_line(&mut self, symbol: &mut LineSymbol, tail: &LineTail) {
        symbol.line_width = length(tail.double_width);
        symbol.color = if tail.double_width > 0 && tail.double_flags & LineTail::DOUBLE_FILL_ON != 0
        {
            self.color(tail.double_fill_color)
        } else {
            SymbolColor::NoColor
        };
        symbol.cap_style = CapStyle::Flat;
        symbol.join_style = JoinStyle::Miter;

        let dashed = tail.double_gap > 0 && tail.double_mode != LineTail::DOUBLE_CONTINUOUS;
        let dash = || BorderDash {
            dash_length: length(tail.double_length),
            break_length: length(tail.double_gap),
        };
        let left = LineSymbolBorder {
            color: self.border_color(tail.double_left_width, tail.double_left_color),
            width: length(tail.double_left_width),
            shift: length(f64::from(tail.double_left_width) / 2.),
            dashed: dashed.then(dash),
        };
        let right = LineSymbolBorder {
            color: self.border_color(tail.double_right_width, tail.double_right_color),
            width: length(tail.double_right_width),
            shift: length(f64::from(tail.double_right_width) / 2.),
            dashed: (dashed && tail.double_mode != LineTail::DOUBLE_LEFT_DASHED).then(dash),
        };
        if dashed && tail.double_mode == LineTail::DOUBLE_ALL_DASHED {
            symbol.dash_style = DashStyle::Dashed {
                dash_length: length(tail.double_length),
                break_length: length(tail.double_gap),
                dash_group: GroupDashes::UnGrouped {
                    half_outer_dashes: false,
                },
            };
        }
        symbol.border = Some(if borders_equal(&left, &right) {
            BorderStyle::SymmetricBorder { both: left }
        } else {
            BorderStyle::AsymmetricBorder { left, right }
        });
    }

    fn border_color(&mut self, width: i16, color: u16) -> SymbolColor {
        if width > 0 {
            self.color(color)
        } else {
            SymbolColor::NoColor
        }
    }

    /// A rectangle symbol, read as the line symbol of its outline. Its grid
    /// is not read.
    pub(super) fn rectangle_symbol(
        &mut self,
        common: SymbolCommon,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<ReadSymbol> {
        let tail = RectangleTail::read_le(cursor)?;
        let code = common.code;
        let mut symbol = LineSymbol::new(code, common.name.clone())
            .with_line_width(length(tail.line_width))
            .with_cap_style(CapStyle::Round)
            .with_join_style(JoinStyle::Round);
        symbol.common = common;
        if tail.line_width > 0 {
            symbol.color = self.color(tail.line_color);
        }
        if tail.corner_radius > 0 {
            self.symbol_issue(code, "the rectangle's rounded corners are drawn square");
        }
        if tail.grid_flags & RectangleTail::GRID_ON != 0 {
            self.symbol_issue(code, "the rectangle's grid was skipped");
        }
        let mut read = ReadSymbol::new(symbol);
        read.rectangle = true;
        Ok(read)
    }
}

/// Whether the main line draws something itself, so another line cannot
/// take its place.
fn draws_on_its_own(symbol: &LineSymbol) -> bool {
    (symbol.line_width.get() > 0. && symbol.color != SymbolColor::NoColor)
        || symbol
            .mid_symbol
            .as_ref()
            .is_some_and(|mid| !is_empty(&mid.mid_symbol))
}

fn is_visible_double_line(tail: &LineTail) -> bool {
    tail.double_mode != LineTail::DOUBLE_OFF
        && (tail.double_width > 0 || tail.double_left_width > 0 || tail.double_right_width > 0)
}

fn borders_equal(left: &LineSymbolBorder, right: &LineSymbolBorder) -> bool {
    let dash = |border: &LineSymbolBorder| {
        border
            .dashed
            .as_ref()
            .map(|dash| (dash.dash_length, dash.break_length))
    };
    left.color == right.color
        && left.width == right.width
        && left.shift == right.shift
        && dash(left) == dash(right)
}
