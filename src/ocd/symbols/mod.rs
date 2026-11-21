//! Symbols: the symbol index and the structures it lists.
//!
//! The file names a symbol by its number, 1000 times the symbol code's first
//! two parts. Reading keeps those numbers beside the symbol set for objects
//! to look up. A symbol that refers to another by number (an area's border
//! line) is resolved once every symbol is read, as Mapper does.

mod area;
mod line;
mod point;
pub(in crate::ocd) mod raw;
mod text;
mod write;

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use binrw::{BinRead as _, binrw};

use crate::Code;
use crate::colors::SymbolColor;
use crate::objects::{HorizontalAlign, VerticalAlign};
use crate::ocd::Result;
use crate::ocd::colors::ColorNumbers;
use crate::ocd::file::{IndexEntry, OcdFile};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::symbols::{
    CapStyle, JoinStyle, PublicOrPrivateSymbol, Symbol, SymbolCommon, SymbolId, SymbolSet,
};
use raw::{BaseSymbol, LineTail, symbol_type};
pub(in crate::ocd) use write::{Drawn, WrittenSymbols, write_symbols};

/// A symbol index entry: the symbol's file position.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct SymbolIndexEntry {
    pub(in crate::ocd) position: u32,
}

impl IndexEntry for SymbolIndexEntry {
    const SIZE: usize = 4;

    fn position(&self) -> u32 {
        self.position
    }

    fn set_position(&mut self, position: u32) {
        self.position = position;
    }
}

/// The code of the symbol the file numbers `number`.
pub(in crate::ocd) fn code(number: i32) -> Code {
    let number = number.max(0);
    Code::new(
        u16::try_from(number / 1000).unwrap_or(u16::MAX),
        // Below 1000.
        (number % 1000) as u16,
        0,
    )
}

/// The file's symbol numbers, the symbols they name, and what the file says
/// about a symbol that its objects need.
#[derive(Debug, Default)]
pub(in crate::ocd) struct SymbolNumbers {
    by_number: HashMap<i32, SymbolId>,
    /// How text objects of each text symbol are aligned.
    pub(in crate::ocd) text_alignment: HashMap<SymbolId, (HorizontalAlign, VerticalAlign)>,
    /// The line symbols that were rectangle symbols, whose objects are
    /// rectangles.
    pub(in crate::ocd) rectangles: HashSet<SymbolId>,
}

impl SymbolNumbers {
    /// The symbol `number` names, if the file defines it.
    pub(in crate::ocd) fn get(&self, number: i32) -> Option<SymbolId> {
        self.by_number.get(&number).copied()
    }
}

/// What reading the symbols needs to hand around.
pub(in crate::ocd) struct Reader<'a> {
    colors: &'a ColorNumbers,
    warnings: &'a mut Warnings,
}

impl Reader<'_> {
    /// The color the file numbers `number`.
    fn color(&mut self, number: u16) -> SymbolColor {
        self.colors.get(i32::from(number), self.warnings)
    }

    /// Report that something of the symbol `code` was not read exactly.
    fn symbol_issue(&mut self, code: Code, issue: &'static str) {
        push_once(self.warnings, Warning::Symbol { code, issue });
    }
}

/// A symbol as read, before references to other symbols are resolved.
struct ReadSymbol {
    symbol: Symbol,
    /// For an area with a border: the border line's symbol number.
    border: Option<i32>,
    /// For a text symbol: how its objects are aligned.
    text_alignment: Option<(HorizontalAlign, VerticalAlign)>,
    /// Whether this was a rectangle symbol.
    rectangle: bool,
}

impl ReadSymbol {
    fn new(symbol: impl Into<Symbol>) -> Self {
        Self {
            symbol: symbol.into(),
            border: None,
            text_alignment: None,
            rectangle: false,
        }
    }
}

/// Read every symbol in `file`, in file order.
pub(in crate::ocd) fn read_symbols(
    file: &OcdFile,
    colors: &ColorNumbers,
    warnings: &mut Warnings,
) -> Result<(SymbolSet, SymbolNumbers)> {
    let entries: Vec<SymbolIndexEntry> = file.index(file.header().first_symbol_block)?;
    let mut reader = Reader { colors, warnings };
    let mut symbols = SymbolSet::new();
    let mut numbers = SymbolNumbers::default();
    let mut borders = Vec::new();
    for entry in entries {
        let mut cursor = Cursor::new(file.bytes_from(entry.position)?);
        let base = BaseSymbol::read_le(&mut cursor)?;
        let Some(read) = reader.read_symbol(&base, &mut cursor)? else {
            continue;
        };
        let id = symbols.push(read.symbol);
        numbers.by_number.insert(base.number, id);
        if let Some(border) = read.border {
            borders.push((id, border));
        }
        if let Some(alignment) = read.text_alignment {
            numbers.text_alignment.insert(id, alignment);
        }
        if read.rectangle {
            numbers.rectangles.insert(id);
        }
    }
    resolve_borders(&mut symbols, &numbers, borders, reader.warnings);
    Ok((symbols, numbers))
}

impl Reader<'_> {
    fn read_symbol(
        &mut self,
        base: &BaseSymbol,
        cursor: &mut Cursor<&[u8]>,
    ) -> Result<Option<ReadSymbol>> {
        let common = common(base);
        let read = match base.symbol_type {
            symbol_type::POINT => ReadSymbol::new(self.point_symbol(common, base, cursor)?),
            symbol_type::LINE => ReadSymbol::new(self.line_symbol(common, cursor)?),
            symbol_type::AREA => self.area_symbol(common, base, cursor)?,
            symbol_type::TEXT => self.text_symbol(common, base, cursor)?,
            symbol_type::LINE_TEXT => self.line_text_symbol(common, cursor)?,
            symbol_type::RECTANGLE => self.rectangle_symbol(common, cursor)?,
            other => {
                self.warnings.push(Warning::UnsupportedSymbolType {
                    code: code(base.number),
                    symbol_type: other,
                });
                return Ok(None);
            }
        };
        Ok(Some(read))
    }
}

/// The fields every symbol has.
fn common(base: &BaseSymbol) -> SymbolCommon {
    SymbolCommon {
        name: base.name(),
        code: code(base.number),
        description: String::new(),
        is_helper_symbol: false,
        is_hidden: base.status & BaseSymbol::HIDDEN != 0,
        is_protected: base.status & BaseSymbol::PROTECTED != 0,
        custom_icon: None,
    }
}

/// The cap and join style a line style value stands for.
fn line_styles(style: u16) -> Option<(CapStyle, JoinStyle)> {
    Some(match style {
        LineTail::BEVEL_FLAT => (CapStyle::Flat, JoinStyle::Bevel),
        LineTail::ROUND_ROUND => (CapStyle::Round, JoinStyle::Round),
        LineTail::BEVEL_POINTED => (CapStyle::Pointed, JoinStyle::Bevel),
        LineTail::ROUND_POINTED => (CapStyle::Pointed, JoinStyle::Round),
        LineTail::MITER_FLAT => (CapStyle::Flat, JoinStyle::Miter),
        LineTail::MITER_POINTED => (CapStyle::Pointed, JoinStyle::Miter),
        _ => return None,
    })
}

/// Point each bordered area's second part at its border line.
fn resolve_borders(
    symbols: &mut SymbolSet,
    numbers: &SymbolNumbers,
    borders: Vec<(SymbolId, i32)>,
    warnings: &mut Warnings,
) {
    for (area, number) in borders {
        let border = numbers
            .get(number)
            .and_then(|id| symbols.get(id))
            .and_then(|entry| entry.as_line_path());
        if border.is_none() {
            push_once(warnings, Warning::UnknownSymbol(number));
        }
        let Some(Symbol::CombinedArea(combined)) = symbols.get_mut(area) else {
            continue;
        };
        if let Some(part) = combined.parts.get_mut(1) {
            *part = PublicOrPrivateSymbol::Public(border.map(Into::into));
        }
    }
}
