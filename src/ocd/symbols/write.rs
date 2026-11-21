//! Writing symbols, the inverse of reading them.
//!
//! Each symbol gets the number its code makes, or the next free one if
//! another symbol has it. A combined symbol is written as one OCAD symbol
//! when it has the shape reading gives one (an area with a border line, a
//! line with a framing and a double line). Any other combined symbol is
//! broken down, as Mapper does: each part becomes an OCAD symbol of its own,
//! and every object of the combined symbol is written once per part. Text is
//! aligned by symbol in OCAD, so a text symbol is written once for each
//! alignment its objects use.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use binrw::BinWrite;

use super::SymbolIndexEntry;
use super::raw::{
    AreaTail, BaseSymbol, BasicText, Element, ElementHeader, Framing, LineTail, PascalString31,
    PointTail, TextTail, symbol_type,
};
use crate::colors::SymbolColor;
use crate::objects::{HorizontalAlign, MapObject, TextGeometry, VerticalAlign};
use crate::ocd::colors::ColorWriter;
use crate::ocd::geometry::{write_polygon, write_rings};
use crate::ocd::units::{FileCoord, to_tenths_of_degree, to_units};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::ocd::{Error, Result};
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, BorderStyle, CapStyle, ClippingOption, DashStyle, FillPattern,
    FramingMode, GroupDashes, JoinStyle, LineSymbol, LineSymbolBorder, MidSymbolPlacement,
    PointSymbol, PublicOrPrivateSymbol, Symbol, SymbolCommon, SymbolId, SymbolSet, TextSymbol,
};
use crate::{Code, Omap, Transformable as _};

/// What kind of object a symbol number is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ocd) enum Drawn {
    Point,
    Line,
    Area,
    Text,
}

/// The symbols as written, and the numbers objects are written with.
#[derive(Debug, Default)]
pub(in crate::ocd) struct WrittenSymbols {
    pub(in crate::ocd) items: Vec<(SymbolIndexEntry, Vec<u8>)>,
    /// The numbers each symbol's objects are written under: one, or one per
    /// part of a combined symbol broken down.
    numbers: HashMap<SymbolId, Vec<(i32, Drawn)>>,
    /// For each text symbol, the number of each alignment variant.
    text_variants: HashMap<SymbolId, Vec<(HorizontalAlign, VerticalAlign, i32)>>,
}

impl WrittenSymbols {
    /// The numbers objects of `symbol` are written under, and as what.
    pub(in crate::ocd) fn numbers(&self, symbol: SymbolId) -> &[(i32, Drawn)] {
        self.numbers.get(&symbol).map_or(&[], Vec::as_slice)
    }

    /// The number a text object of `symbol` with this alignment is written
    /// under. Box text takes any variant of its horizontal alignment.
    pub(in crate::ocd) fn text_number(
        &self,
        symbol: SymbolId,
        horizontal: HorizontalAlign,
        vertical: Option<VerticalAlign>,
    ) -> Option<i32> {
        let variants = self.text_variants.get(&symbol)?;
        variants
            .iter()
            .find(|(h, v, _)| *h == horizontal && vertical.is_none_or(|vertical| *v == vertical))
            .or_else(|| variants.first())
            .map(|&(_, _, number)| number)
    }
}

/// The number the file gives the symbol with `code`: its first two parts,
/// as Mapper writes them. 0 is not a valid number.
fn number_of(code: Code) -> i32 {
    let number = i32::from(code.major) * 1000 + i32::from(code.minor % 1000);
    number.max(1)
}

/// The UTF-16 units of a symbol name the file holds, leaving room for its
/// terminating zero.
const NAME_UNITS: usize = 63;

/// Numbers already given.
#[derive(Default)]
struct Numbers(HashSet<i32>);

impl Numbers {
    /// `preferred`, or the next free number after it.
    fn claim(&mut self, preferred: i32) -> i32 {
        let mut number = preferred;
        while !self.0.insert(number) {
            number = number.saturating_add(1);
        }
        number
    }
}

/// How one symbol is written.
enum Plan<'a> {
    Point(&'a PointSymbol),
    Line(LineParts<'a>),
    Area {
        area: &'a AreaSymbol,
        border: Option<Border<'a>>,
    },
    Text(&'a TextSymbol),
    /// One OCAD symbol per part.
    Breakdown(Vec<Part<'a>>),
}

/// The lines one OCAD line symbol draws.
#[derive(Clone, Copy, Default)]
struct LineParts<'a> {
    main: Option<&'a LineSymbol>,
    framing: Option<&'a LineSymbol>,
    double: Option<&'a LineSymbol>,
}

enum Border<'a> {
    /// A symbol of the set, written under its own number.
    Public(SymbolId),
    /// A line only the area has, written as a symbol of its own.
    Private(&'a LineSymbol),
}

/// A part of a combined symbol broken down.
enum Part<'a> {
    Public(SymbolId),
    Line(&'a LineSymbol),
    Area(&'a AreaSymbol),
}

/// Number every symbol by its code, as Mapper does: a symbol whose number
/// an earlier one has gets the next free number.
fn number_symbols(
    symbols: &SymbolSet,
    warnings: &mut Warnings,
) -> (Numbers, HashMap<SymbolId, i32>) {
    let mut numbers = Numbers::default();
    let mut own_number = HashMap::new();
    let mut renumbered = Vec::new();
    for entry in symbols {
        let code = entry.common().code;
        let preferred = number_of(code);
        if numbers.0.insert(preferred) {
            own_number.insert(entry.id(), preferred);
        } else {
            renumbered.push((entry.id(), code));
        }
        if entry.common().name.encode_utf16().count() > NAME_UNITS {
            push_once(
                warnings,
                Warning::Symbol {
                    code,
                    issue: "the name is longer than the 63 characters OCAD holds; it was cut",
                },
            );
        }
        if code.patch != 0 || code.minor >= 1000 {
            push_once(
                warnings,
                Warning::Symbol {
                    code,
                    issue: "the code has more than two parts of up to three digits; it was shortened",
                },
            );
        }
    }
    for (id, code) in renumbered {
        own_number.insert(id, numbers.claim(number_of(code)));
        push_once(
            warnings,
            Warning::Symbol {
                code,
                issue: "another symbol has the same number; it was renumbered",
            },
        );
    }
    (numbers, own_number)
}

/// Write every symbol of `map`, asking `colors` for the colors they use.
pub(in crate::ocd) fn write_symbols(
    map: &Omap,
    colors: &ColorWriter,
    warnings: &mut Warnings,
) -> Result<WrittenSymbols> {
    let symbols = &map.symbols;
    let (numbers, own_number) = number_symbols(symbols, warnings);

    let alignments = text_alignments(map);
    let plans: Vec<(SymbolId, Plan<'_>)> = symbols
        .iter()
        .map(|entry| (entry.id(), plan(entry.value(), symbols)))
        .collect();
    let mut writer = Writer {
        colors,
        warnings,
        written: WrittenSymbols::default(),
        numbers,
        own_number,
        pending_parts: Vec::new(),
    };
    let kinds: HashMap<SymbolId, Drawn> = plans
        .iter()
        .filter_map(|(id, plan)| Some((*id, plan.drawn()?)))
        .collect();
    for (id, plan) in &plans {
        let Some(entry) = symbols.get(*id) else {
            continue;
        };
        let number = writer.own_number[id];
        let common = entry.common();
        match plan {
            Plan::Point(point) => {
                let bytes = writer.point_symbol(number, common, point)?;
                writer.push(*id, number, Drawn::Point, bytes);
            }
            Plan::Line(parts) => {
                let bytes = writer.line_symbol(number, common, *parts)?;
                writer.push(*id, number, Drawn::Line, bytes);
            }
            Plan::Area { area, border } => {
                let border = match border {
                    None => None,
                    Some(Border::Public(border)) => writer.own_number.get(border).copied(),
                    Some(Border::Private(line)) => {
                        let border_number = writer.numbers.claim(number);
                        let mut border_common = common.clone();
                        border_common.name = format!("Border of {}", common.name);
                        let parts = LineParts::single(line);
                        let bytes = writer.line_symbol(border_number, &border_common, parts)?;
                        writer
                            .written
                            .items
                            .push((SymbolIndexEntry::default(), bytes));
                        Some(border_number)
                    }
                };
                let bytes = writer.area_symbol(number, common, area, border)?;
                writer.push(*id, number, Drawn::Area, bytes);
            }
            Plan::Text(text) => {
                let variants = alignments.get(id).cloned().unwrap_or_default();
                writer.text_symbol(*id, number, common, text, variants)?;
            }
            Plan::Breakdown(parts) => {
                push_once(
                    writer.warnings,
                    Warning::Symbol {
                        code: common.code,
                        issue: "the combined symbol was written as one symbol per part, and its objects once per part",
                    },
                );
                writer.breakdown(*id, number, common, parts, &kinds)?;
            }
        }
    }
    writer.resolve_pending_parts();
    Ok(writer.written)
}

impl Plan<'_> {
    /// What objects of a symbol written whole are drawn as.
    fn drawn(&self) -> Option<Drawn> {
        Some(match self {
            Plan::Point(_) => Drawn::Point,
            Plan::Line(_) => Drawn::Line,
            Plan::Area { .. } => Drawn::Area,
            Plan::Text(_) => Drawn::Text,
            Plan::Breakdown(_) => return None,
        })
    }
}

/// Decide how `symbol` is written.
fn plan<'a>(symbol: &'a Symbol, symbols: &'a SymbolSet) -> Plan<'a> {
    match symbol {
        Symbol::Point(point) => Plan::Point(point),
        Symbol::Line(line) => Plan::Line(LineParts::single(line)),
        Symbol::Area(area) => Plan::Area { area, border: None },
        Symbol::Text(text) => Plan::Text(text),
        Symbol::CombinedArea(combined) => {
            let parts: Vec<Part<'a>> = combined
                .components()
                .filter_map(|part| match part {
                    PublicOrPrivateSymbol::Public(id) => id.map(|id| Part::Public(id.into())),
                    PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(area)) => {
                        Some(Part::Area(area))
                    }
                    PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Line(line)) => {
                        Some(Part::Line(line))
                    }
                })
                .collect();
            area_with_border(&parts, symbols).unwrap_or(Plan::Breakdown(parts))
        }
        Symbol::CombinedLine(combined) => {
            let parts: Vec<Part<'a>> = combined
                .components()
                .filter_map(|part| match part {
                    PublicOrPrivateSymbol::Public(id) => id.map(|id| Part::Public(id.into())),
                    PublicOrPrivateSymbol::Private(line) => Some(Part::Line(line)),
                })
                .collect();
            let lines: Option<Vec<&LineSymbol>> = parts
                .iter()
                .map(|part| match part {
                    Part::Line(line) => Some(*line),
                    Part::Public(id) => symbols.line_symbol(symbols.get(*id)?.as_line()?),
                    Part::Area(_) => None,
                })
                .collect();
            match lines.as_deref().and_then(LineParts::assign) {
                Some(parts) => Plan::Line(parts),
                None => Plan::Breakdown(parts),
            }
        }
    }
}

/// An area and a line written as one area symbol with a border, if the parts
/// are that.
fn area_with_border<'a>(parts: &[Part<'a>], symbols: &'a SymbolSet) -> Option<Plan<'a>> {
    let area_of = |part: &Part<'a>| match part {
        Part::Area(area) => Some(*area),
        Part::Public(id) => symbols.area_symbol(symbols.get(*id)?.as_area()?),
        Part::Line(_) => None,
    };
    let border_of = |part: &Part<'a>| match part {
        Part::Line(line) => Some(Border::Private(line)),
        Part::Public(id) => match symbols.get(*id).map(crate::Entry::value) {
            Some(Symbol::Line(_) | Symbol::CombinedLine(_)) => Some(Border::Public(*id)),
            _ => None,
        },
        Part::Area(_) => None,
    };
    match parts {
        [single] => Some(Plan::Area {
            area: area_of(single)?,
            border: None,
        }),
        [first, second] => {
            let (area, border) = match (area_of(first), area_of(second)) {
                (Some(area), _) => (area, border_of(second)?),
                (None, Some(area)) => (area, border_of(first)?),
                (None, None) => return None,
            };
            Some(Plan::Area {
                area,
                border: Some(border),
            })
        }
        _ => None,
    }
}

impl<'a> LineParts<'a> {
    /// A single line symbol: the main line, or the double line if it has
    /// borders.
    fn single(line: &'a LineSymbol) -> Self {
        if line.border.is_some() {
            Self {
                double: Some(line),
                ..Self::default()
            }
        } else {
            Self {
                main: Some(line),
                ..Self::default()
            }
        }
    }

    /// Up to three lines as main line, framing and double line, chosen as
    /// Mapper chooses them, if they fit those roles.
    fn assign(lines: &[&'a LineSymbol]) -> Option<Self> {
        let (mut main, mut framing, mut double) = match *lines {
            [single] => return Some(Self::single(single)),
            [first, second] => (Some(first), Some(second), None),
            [first, second, third] => (Some(first), Some(second), Some(third)),
            _ => return None,
        };
        if !may_be_double(double) {
            if may_be_double(main) {
                std::mem::swap(&mut main, &mut double);
            } else if may_be_double(framing) {
                std::mem::swap(&mut framing, &mut double);
            } else if double.is_some() {
                return None;
            }
        }
        if !may_be_framing(framing) {
            if main.is_none() || may_be_framing(main) {
                std::mem::swap(&mut main, &mut framing);
            } else if framing.is_some() {
                return None;
            }
        }
        if !main.is_some_and(|main| main.border.is_none()) {
            if main.is_some() {
                return None;
            }
            std::mem::swap(&mut main, &mut framing);
        }
        Some(Self {
            main,
            framing,
            double,
        })
    }
}

fn has_point_symbols(line: &LineSymbol) -> bool {
    let empty = super::point::is_empty;
    line.mid_symbol
        .as_ref()
        .is_some_and(|mid| !empty(&mid.mid_symbol))
        || line
            .start_symbol
            .as_ref()
            .is_some_and(|point| !empty(point))
        || line.end_symbol.as_ref().is_some_and(|point| !empty(point))
        || line
            .dash_symbol
            .as_ref()
            .is_some_and(|dash| !empty(&dash.dash_symbol))
}

fn may_be_double(line: Option<&LineSymbol>) -> bool {
    line.is_some_and(|line| {
        line.border.is_some()
            && !matches!(line.cap_style, CapStyle::Pointed)
            && !has_point_symbols(line)
    })
}

fn may_be_framing(line: Option<&LineSymbol>) -> bool {
    line.is_some_and(|line| {
        line.border.is_none()
            && matches!(line.dash_style, DashStyle::NotDashed { .. })
            && !matches!(line.cap_style, CapStyle::Pointed)
            && !has_point_symbols(line)
    })
}

/// A text object's alignment: its vertical alignment only for point text.
type Alignment = (HorizontalAlign, Option<VerticalAlign>);

/// The alignments the text objects of each text symbol use: horizontal and
/// vertical for point text, horizontal only for box text, whose vertical
/// alignment the file does not keep. Most used first.
fn text_alignments(map: &Omap) -> HashMap<SymbolId, Vec<Alignment>> {
    let mut counts: HashMap<SymbolId, Vec<(Alignment, usize)>> = HashMap::new();
    for object in map.iter_all_objects() {
        let MapObject::Text(text) = object else {
            continue;
        };
        let Some(symbol) = text.symbol else {
            continue;
        };
        let vertical = match text.geometry() {
            TextGeometry::SingleAnchor(_) => Some(text.v_align),
            TextGeometry::WrapBox(_) => None,
        };
        let key = (text.h_align, vertical);
        let symbol_counts = counts.entry(symbol.into()).or_default();
        match symbol_counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, count)) => *count += 1,
            None => symbol_counts.push((key, 1)),
        }
    }
    counts
        .into_iter()
        .map(|(symbol, mut keys)| {
            keys.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            (symbol, keys.into_iter().map(|(key, _)| key).collect())
        })
        .collect()
}

struct Writer<'a> {
    colors: &'a ColorWriter,
    warnings: &'a mut Warnings,
    written: WrittenSymbols,
    numbers: Numbers,
    own_number: HashMap<SymbolId, i32>,
    /// Combined symbols broken down, each with a part broken down too.
    pending_parts: Vec<(SymbolId, SymbolId)>,
}

/// The bytes of one OCAD symbol, built up from its base, its structure and
/// its elements.
struct SymbolBytes {
    base: BaseSymbol,
    rest: Vec<u8>,
    colors: Vec<u16>,
}

impl SymbolBytes {
    fn new(number: i32, symbol_type: u8, common: &SymbolCommon) -> Self {
        let mut base = BaseSymbol::new(number, symbol_type);
        for (unit, slot) in common
            .name
            .encode_utf16()
            .zip(base.description.iter_mut().take(NAME_UNITS))
        {
            *slot = unit;
        }
        if common.is_protected {
            base.status |= BaseSymbol::PROTECTED;
        }
        if common.is_hidden {
            base.status |= BaseSymbol::HIDDEN;
        }
        Self {
            base,
            rest: Vec::new(),
            colors: Vec::new(),
        }
    }

    fn append(&mut self, value: &impl for<'b> BinWrite<Args<'b> = ()>) -> Result<()> {
        let mut cursor = Cursor::new(&mut self.rest);
        cursor.set_position(cursor.get_ref().len() as u64);
        value.write_le(&mut cursor)?;
        Ok(())
    }

    fn append_elements(&mut self, elements: &[Element]) -> Result<()> {
        for element in elements {
            element.write(&mut self.rest)?;
        }
        Ok(())
    }

    fn finish(mut self, extent: i32) -> Result<Vec<u8>> {
        self.colors.sort_unstable();
        self.colors.dedup();
        self.base.color_count = if self.colors.len() > self.base.colors.len() {
            -1
        } else {
            // At most 14.
            self.colors.len() as i16
        };
        for (slot, color) in self.base.colors.iter_mut().zip(&self.colors) {
            *slot = *color;
        }
        self.base.extent = extent;
        self.base.size = i32::try_from(BaseSymbol::SIZE + self.rest.len())
            .map_err(|_too_large| Error::TooMany("symbol data"))?;
        let mut bytes = Cursor::new(Vec::new());
        self.base.write_le(&mut bytes)?;
        let mut bytes = bytes.into_inner();
        bytes.extend(self.rest);
        Ok(bytes)
    }
}

/// A length in millimetres as a file length.
fn units_i16(mm: f64) -> i16 {
    to_units(mm).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

fn units_u16(mm: f64) -> u16 {
    to_units(mm).clamp(0., f64::from(u16::MAX)) as u16
}

/// The data size of `elements`, in 8-byte units.
fn data_size(elements: &[Element]) -> Result<u16> {
    let units: usize = elements
        .iter()
        .map(|element| 2 + element.coords.len())
        .sum();
    u16::try_from(units).map_err(|_too_large| Error::TooMany("point symbol elements"))
}

/// How far `elements` reach from the origin, in file units.
fn extent(elements: &[Element]) -> i32 {
    elements
        .iter()
        .map(|element| {
            let reach = element
                .coords
                .iter()
                .map(|coord| {
                    let coord = coord.coord();
                    to_units(coord.x.hypot(coord.y))
                })
                .fold(0., f64::max);
            let width = f64::from(element.header.line_width.max(element.header.diameter)) / 2.;
            reach + width
        })
        .fold(0., f64::max)
        .ceil() as i32
}

impl Writer<'_> {
    fn push(&mut self, id: SymbolId, number: i32, drawn: Drawn, bytes: Vec<u8>) {
        self.written
            .items
            .push((SymbolIndexEntry::default(), bytes));
        self.written
            .numbers
            .entry(id)
            .or_default()
            .push((number, drawn));
    }

    fn issue(&mut self, code: Code, issue: &'static str) {
        push_once(self.warnings, Warning::Symbol { code, issue });
    }

    /// The color number for `color`, noted as used by `bytes`.
    fn color(&self, bytes: &mut SymbolBytes, color: SymbolColor) -> Option<u16> {
        let number = self.colors.number(color)?;
        bytes.colors.push(number);
        Some(number)
    }

    fn point_symbol(
        &mut self,
        number: i32,
        common: &SymbolCommon,
        point: &PointSymbol,
    ) -> Result<Vec<u8>> {
        let mut bytes = SymbolBytes::new(number, symbol_type::POINT, common);
        if point.is_rotatable {
            bytes.base.flags |= BaseSymbol::ROTATABLE;
        }
        let elements = self.elements(&mut bytes, common.code, point, geo_types::Coord::zero())?;
        bytes.append(&PointTail {
            data_size: data_size(&elements)?,
            reserved: 0,
        })?;
        bytes.append_elements(&elements)?;
        bytes.finish(extent(&elements))
    }

    /// The elements that draw `point`, placed at `offset`.
    fn elements(
        &mut self,
        bytes: &mut SymbolBytes,
        code: Code,
        point: &PointSymbol,
        offset: geo_types::Coord,
    ) -> Result<Vec<Element>> {
        let mut elements = Vec::new();
        let at = FileCoord::from_coord(offset)?;
        if point.inner_radius.get() > 0.
            && let Some(color) = self.color(bytes, point.inner_color)
        {
            elements.push(Element {
                header: ElementHeader {
                    element_type: ElementHeader::DOT,
                    color,
                    diameter: units_i16(2. * point.inner_radius.get()),
                    coord_count: 1,
                    ..ElementHeader::default()
                },
                coords: vec![at],
            });
        }
        if point.outer_width.get() > 0.
            && let Some(color) = self.color(bytes, point.outer_color)
        {
            elements.push(Element {
                header: ElementHeader {
                    element_type: ElementHeader::CIRCLE,
                    color,
                    line_width: units_i16(point.outer_width.get()),
                    diameter: units_i16(2. * point.inner_radius.get() + point.outer_width.get()),
                    coord_count: 1,
                    ..ElementHeader::default()
                },
                coords: vec![at],
            });
        }
        for element in &point.elements {
            match element {
                crate::symbols::Element::Point { symbol, object } => {
                    if !symbol.elements.is_empty() || object.rotation != 0. {
                        self.issue(
                            code,
                            "an embedded point symbol's own rotation and elements were dropped",
                        );
                    }
                    let position = offset + object.geometry().0;
                    let flat = PointSymbol {
                        elements: Vec::new(),
                        ..(**symbol).clone()
                    };
                    elements.extend(self.elements(bytes, code, &flat, position)?);
                }
                crate::symbols::Element::Line { symbol, object } => {
                    let Some(color) = self.color(bytes, symbol.color) else {
                        continue;
                    };
                    if symbol.border.is_some()
                        || matches!(symbol.dash_style, DashStyle::Dashed { .. })
                        || has_point_symbols(symbol)
                    {
                        self.issue(
                            code,
                            "an element line's dashes, borders and symbols were dropped",
                        );
                    }
                    let flags = match (symbol.cap_style, symbol.join_style) {
                        (CapStyle::Round, _) | (_, JoinStyle::Round) => ElementHeader::ROUND,
                        (_, JoinStyle::Miter) => ElementHeader::FLAT_MITER,
                        _ => ElementHeader::FLAT_BEVEL,
                    };
                    let shifted = object.geometry().clone().transform(|c| c + offset);
                    let coords = write_rings([&shifted], FileCoord::CORNER_POINT)?;
                    elements.push(element_with(
                        ElementHeader::LINE,
                        flags,
                        color,
                        units_i16(symbol.line_width.get()),
                        coords,
                    )?);
                }
                crate::symbols::Element::Area { symbol, object } => {
                    let Some(color) = self.color(bytes, symbol.color) else {
                        continue;
                    };
                    if !symbol.patterns.is_empty() {
                        self.issue(code, "an element area's patterns were dropped");
                    }
                    let shifted = object.geometry().clone().transform(|c| c + offset);
                    let coords = write_polygon(&shifted, FileCoord::CORNER_POINT)?;
                    elements.push(element_with(ElementHeader::AREA, 0, color, 0, coords)?);
                }
            }
        }
        Ok(elements)
    }
}

fn element_with(
    element_type: u16,
    flags: u16,
    color: u16,
    line_width: i16,
    coords: Vec<FileCoord>,
) -> Result<Element> {
    Ok(Element {
        header: ElementHeader {
            element_type,
            flags,
            color,
            line_width,
            coord_count: u16::try_from(coords.len())
                .map_err(|_too_many| Error::TooMany("element coordinates"))?,
            ..ElementHeader::default()
        },
        coords,
    })
}

/// The line style value for a cap and join style. OCAD has no square caps
/// and no flat caps with round joins.
fn line_style(cap: CapStyle, join: JoinStyle) -> Option<u16> {
    Some(match (cap, join) {
        (CapStyle::Flat, JoinStyle::Bevel) => LineTail::BEVEL_FLAT,
        (CapStyle::Round, JoinStyle::Round) => LineTail::ROUND_ROUND,
        (CapStyle::Pointed, JoinStyle::Bevel) => LineTail::BEVEL_POINTED,
        (CapStyle::Pointed, JoinStyle::Round) => LineTail::ROUND_POINTED,
        (CapStyle::Flat, JoinStyle::Miter) => LineTail::MITER_FLAT,
        (CapStyle::Pointed, JoinStyle::Miter) => LineTail::MITER_POINTED,
        _ => return None,
    })
}

impl Writer<'_> {
    /// One OCAD line symbol drawing `parts`.
    fn line_symbol(
        &mut self,
        number: i32,
        common: &SymbolCommon,
        parts: LineParts<'_>,
    ) -> Result<Vec<u8>> {
        let code = common.code;
        let mut bytes = SymbolBytes::new(number, symbol_type::LINE, common);
        let mut tail = LineTail::default();
        // The main line, or else the double line, carries the line's point
        // symbols and offsets.
        let carrier = parts.main.or(parts.double);
        if let Some(main) = parts.main {
            if let Some(color) = self.color(&mut bytes, main.color) {
                tail.line_color = color;
                tail.line_width = units_u16(main.line_width.get());
            }
            tail.line_style = line_style(main.cap_style, main.join_style).unwrap_or_else(|| {
                self.issue(code, "the line's caps and joins have no OCAD style; written as bevel joins and flat caps");
                LineTail::BEVEL_FLAT
            });
            self.dash_pattern(code, main, &mut tail);
        } else if let Some(double) = parts.double
            && let DashStyle::NotDashed {
                segment_length,
                end_length,
            } = &double.dash_style
        {
            tail.main_length = units_i16(segment_length.get());
            tail.end_length = units_i16(end_length.get());
        }
        if let Some(carrier) = carrier {
            tail.distance_from_start = units_i16(carrier.start_offset.get());
            tail.distance_to_end = units_i16(carrier.end_offset.get());
        }
        if let Some(framing) = parts.framing
            && let Some(color) = self.color(&mut bytes, framing.color)
        {
            tail.framing_color = color;
            tail.framing_width = units_i16(framing.line_width.get());
            let style = line_style(framing.cap_style, framing.join_style);
            tail.framing_style = if let Some(
                style @ (LineTail::BEVEL_FLAT | LineTail::ROUND_ROUND | LineTail::MITER_FLAT),
            ) = style
            {
                style
            } else {
                self.issue(
                    code,
                    "the framing's caps and joins have no OCAD style; written as bevel joins and flat caps",
                );
                LineTail::BEVEL_FLAT
            };
        }
        if let Some(double) = parts.double {
            self.double_line(&mut bytes, code, double, &mut tail);
        }
        let elements = match carrier {
            Some(carrier) => self.line_point_symbols(&mut bytes, carrier, &mut tail)?,
            None => Vec::new(),
        };
        bytes.append(&tail)?;
        bytes.append_elements(&elements)?;
        let width = [
            tail.line_width,
            tail.framing_width.max(0).unsigned_abs(),
            tail.double_width.max(0).unsigned_abs(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        bytes.finish(i32::from(width) / 2 + extent(&elements))
    }

    /// The dash pattern of the main line.
    fn dash_pattern(&mut self, code: Code, main: &LineSymbol, tail: &mut LineTail) {
        match &main.dash_style {
            DashStyle::NotDashed {
                segment_length,
                end_length,
            } => {
                tail.main_length = units_i16(segment_length.get());
                tail.end_length = units_i16(end_length.get());
            }
            DashStyle::Dashed {
                dash_length,
                break_length,
                dash_group: GroupDashes::UnGrouped { half_outer_dashes },
            } => {
                let centered_in_dashes = main.mid_symbol.as_ref().is_some_and(|mid| {
                    !super::point::is_empty(&mid.mid_symbol)
                        && !matches!(mid.mid_symbol_placement, MidSymbolPlacement::CenterOfGap)
                });
                if centered_in_dashes {
                    // Only a secondary gap, as Mapper writes a dashed line
                    // with mid symbols: they are centered in the dashes then.
                    if *half_outer_dashes {
                        self.issue(code, "half outer dashes were dropped");
                    }
                    tail.main_length = units_i16(dash_length.get() + break_length.get());
                    tail.end_length = tail.main_length / 2;
                    tail.secondary_gap = units_i16(break_length.get());
                } else {
                    tail.main_length = units_i16(dash_length.get());
                    tail.end_length = if *half_outer_dashes {
                        tail.main_length / 2
                    } else {
                        tail.main_length
                    };
                    tail.main_gap = units_i16(break_length.get());
                }
            }
            DashStyle::Dashed {
                dash_length,
                break_length,
                dash_group:
                    GroupDashes::Grouped {
                        dashes_in_group,
                        in_group_break_length,
                    },
            } => {
                if *dashes_in_group != 2 {
                    self.issue(code, "dash groups other than pairs were written as pairs");
                }
                tail.main_length = units_i16(2. * dash_length.get() + in_group_break_length.get());
                tail.end_length = tail.main_length;
                tail.main_gap = units_i16(break_length.get());
                tail.secondary_gap = units_i16(in_group_break_length.get());
                tail.end_gap = tail.secondary_gap;
            }
        }
    }

    /// The double line's filling and borders.
    fn double_line(
        &mut self,
        bytes: &mut SymbolBytes,
        code: Code,
        double: &LineSymbol,
        tail: &mut LineTail,
    ) {
        let (left, right) = match &double.border {
            Some(BorderStyle::SymmetricBorder { both }) => (both, both),
            Some(BorderStyle::AsymmetricBorder { left, right }) => (left, right),
            None => return,
        };
        tail.double_width = units_i16(double.line_width.get());
        if let Some(color) = self.color(bytes, double.color) {
            tail.double_flags |= LineTail::DOUBLE_FILL_ON;
            tail.double_fill_color = color;
        }
        let mut border = |border: &LineSymbolBorder| {
            if (border.shift.get() - border.width.get() / 2.).abs() > 0.005 {
                self.issue(code, "a border's shift was written as half its width");
            }
            match self.color(bytes, border.color) {
                Some(color) => (color, units_i16(border.width.get())),
                None => (0, 0),
            }
        };
        (tail.double_left_color, tail.double_left_width) = border(left);
        (tail.double_right_color, tail.double_right_width) = border(right);
        let dash = left.dashed.as_ref().or(right.dashed.as_ref());
        tail.double_mode = match (&left.dashed, &right.dashed) {
            (None, None) => LineTail::DOUBLE_CONTINUOUS,
            (Some(_), None) => LineTail::DOUBLE_LEFT_DASHED,
            (Some(_), Some(_)) if matches!(double.dash_style, DashStyle::Dashed { .. }) => {
                LineTail::DOUBLE_ALL_DASHED
            }
            (Some(_), Some(_)) => LineTail::DOUBLE_BORDERS_DASHED,
            (None, Some(_)) => {
                self.issue(
                    code,
                    "a dashed right border alone was written with the left one dashed too",
                );
                LineTail::DOUBLE_BORDERS_DASHED
            }
        };
        if let Some(dash) = dash {
            tail.double_length = units_i16(dash.dash_length.get());
            tail.double_gap = units_i16(dash.break_length.get());
        }
    }

    /// The point symbols along, at the corners of and at the ends of
    /// `line`, with their data sizes set in `tail`.
    fn line_point_symbols(
        &mut self,
        bytes: &mut SymbolBytes,
        line: &LineSymbol,
        tail: &mut LineTail,
    ) -> Result<Vec<Element>> {
        let code = line.common.code;
        let origin = geo_types::Coord::zero();
        let mut all = Vec::new();
        let mut primary = Vec::new();
        let mut secondary = Vec::new();
        if let Some(mid) = &line.mid_symbol {
            // At least one symbol (0), or at least none (-1).
            tail.minimum_symbols = if mid.show_at_least_one_mid_symbol {
                0
            } else {
                -1
            };
            let elements = self.elements(bytes, code, &mid.mid_symbol, origin)?;
            let in_dash_groups = matches!(
                mid.mid_symbol_placement,
                MidSymbolPlacement::CenterOfDashGroup
            );
            let dashed = matches!(line.dash_style, DashStyle::Dashed { .. });
            if in_dash_groups && !dashed && mid.mid_symbols_per_spot <= 1 {
                secondary = elements;
            } else {
                tail.primary_symbol_count =
                    i16::try_from(mid.mid_symbols_per_spot).unwrap_or(i16::MAX);
                tail.primary_symbol_distance = units_i16(mid.mid_symbol_distance.get());
                primary = elements;
            }
        }
        let corner = match &line.dash_symbol {
            Some(dash) => self.elements(bytes, code, &dash.dash_symbol, origin)?,
            None => Vec::new(),
        };
        let start = match &line.start_symbol {
            Some(start) => self.elements(bytes, code, start, origin)?,
            None => Vec::new(),
        };
        let end = match &line.end_symbol {
            Some(end) => self.elements(bytes, code, end, origin)?,
            None => Vec::new(),
        };
        tail.primary_data_size = data_size(&primary)?;
        tail.secondary_data_size = data_size(&secondary)?;
        tail.corner_data_size = data_size(&corner)?;
        tail.start_data_size = data_size(&start)?;
        tail.end_data_size = data_size(&end)?;
        for elements in [primary, secondary, corner, start, end] {
            all.extend(elements);
        }
        Ok(all)
    }

    /// An area symbol, with `border` the number of its border line.
    fn area_symbol(
        &mut self,
        number: i32,
        common: &SymbolCommon,
        area: &AreaSymbol,
        border: Option<i32>,
    ) -> Result<Vec<u8>> {
        let code = common.code;
        let mut bytes = SymbolBytes::new(number, symbol_type::AREA, common);
        if area.is_rotatable {
            bytes.base.flags |= BaseSymbol::ROTATABLE;
        }
        let mut tail = AreaTail::default();
        if let Some(color) = self.color(&mut bytes, area.color) {
            tail.fill_on = 1;
            tail.fill_color = color;
        }
        if let Some(border) = border {
            tail.border_on = 1;
            tail.border_symbol = border;
        }
        let mut hatches = Vec::new();
        let mut structures = Vec::new();
        for pattern in &area.patterns {
            match pattern {
                FillPattern::LinePattern { .. } => hatches.push(pattern),
                FillPattern::PointPattern { .. } => structures.push(pattern),
            }
        }
        self.hatching(&mut bytes, code, &hatches, &mut tail);
        let elements = self.structure(&mut bytes, code, &structures, &mut tail)?;
        bytes.append(&tail)?;
        bytes.append_elements(&elements)?;
        bytes.finish(0)
    }

    fn hatching(
        &mut self,
        bytes: &mut SymbolBytes,
        code: Code,
        hatches: &[&FillPattern],
        tail: &mut AreaTail,
    ) {
        let mut lines = hatches.iter().filter_map(|pattern| match pattern {
            FillPattern::LinePattern {
                angle,
                line_spacing,
                line_offset,
                line_color,
                line_width,
                ..
            } => Some((
                *angle,
                *line_spacing,
                *line_offset,
                *line_color,
                *line_width,
            )),
            FillPattern::PointPattern { .. } => None,
        });
        let Some((angle, spacing, offset, color, width)) = lines.next() else {
            return;
        };
        let Some(color_number) = self.color(bytes, color) else {
            return;
        };
        if offset.get() != 0. {
            self.issue(code, "a hatch's offset was dropped");
        }
        tail.hatch_mode = 1;
        tail.hatch_color = color_number;
        tail.hatch_line_width = units_u16(width.get());
        tail.hatch_distance = units_u16(spacing.get());
        tail.hatch_angle_1 = to_tenths_of_degree(angle);
        if let Some((second_angle, second_spacing, _, second_color, second_width)) = lines.next() {
            if second_color == color && second_spacing == spacing && second_width == width {
                tail.hatch_mode = AreaTail::HATCH_CROSS;
                tail.hatch_angle_2 = to_tenths_of_degree(second_angle);
            } else {
                self.issue(code, "a hatch unlike the first was dropped");
            }
        }
        if lines.next().is_some() {
            self.issue(code, "hatches beyond two were dropped");
        }
    }

    fn structure(
        &mut self,
        bytes: &mut SymbolBytes,
        code: Code,
        structures: &[&FillPattern],
        tail: &mut AreaTail,
    ) -> Result<Vec<Element>> {
        let points: Vec<_> = structures
            .iter()
            .filter_map(|pattern| match pattern {
                FillPattern::PointPattern {
                    clip_options,
                    angle,
                    line_spacing,
                    line_offset,
                    offset_along_line,
                    point_distance,
                    point,
                    ..
                } => Some((
                    *clip_options,
                    *angle,
                    *line_spacing,
                    *line_offset,
                    *offset_along_line,
                    *point_distance,
                    point,
                )),
                FillPattern::LinePattern { .. } => None,
            })
            .collect();
        let Some(&(clip, angle, spacing, offset, along, distance, point)) = points.first() else {
            return Ok(Vec::new());
        };
        let shifted = points.get(1).is_some_and(|&(_, a, s, o, al, d, _)| {
            (a - angle).abs() < 1e-9
                && s == spacing
                && d == distance
                && (o.get() - spacing.get() / 2.).abs() < 0.005
                && (al.get() - distance.get() / 2.).abs() < 0.005
        });
        if offset.get() != 0. || along.get() != 0. {
            self.issue(code, "a structure's offsets were dropped");
        }
        if points.len() > if shifted { 2 } else { 1 } {
            self.issue(code, "structures beyond the first were dropped");
        }
        tail.structure_mode = if shifted {
            AreaTail::STRUCTURE_SHIFTED
        } else {
            AreaTail::STRUCTURE_ALIGNED
        };
        tail.structure_draw = match clip {
            ClippingOption::ClipElementsAtBoundary => 0,
            ClippingOption::NoClippingIfCompletelyInside => 1,
            ClippingOption::NoClippingIfCenterInside => 2,
            ClippingOption::NoClippingIfPartiallyInside => 3,
        };
        tail.structure_width = units_u16(distance.get());
        tail.structure_height = units_u16(if shifted {
            spacing.get() / 2.
        } else {
            spacing.get()
        });
        tail.structure_angle = to_tenths_of_degree(angle);
        let elements = self.elements(bytes, code, point, geo_types::Coord::zero())?;
        tail.data_size = data_size(&elements)?;
        Ok(elements)
    }

    /// One OCAD text symbol per alignment variant `variants` lists; the most
    /// used gets the symbol's own number.
    fn text_symbol(
        &mut self,
        id: SymbolId,
        number: i32,
        common: &SymbolCommon,
        text: &TextSymbol,
        variants: Vec<Alignment>,
    ) -> Result<()> {
        let mut resolved: Vec<(HorizontalAlign, VerticalAlign)> = Vec::new();
        for (horizontal, vertical) in &variants {
            if let Some(vertical) = vertical {
                if !resolved.contains(&(*horizontal, *vertical)) {
                    resolved.push((*horizontal, *vertical));
                }
            } else if !resolved.iter().any(|(h, _)| h == horizontal) {
                resolved.push((*horizontal, VerticalAlign::Baseline));
            }
        }
        if resolved.is_empty() {
            resolved.push((HorizontalAlign::HCenter, VerticalAlign::Baseline));
        }
        let mut numbered = Vec::new();
        for (index, (horizontal, vertical)) in resolved.into_iter().enumerate() {
            let variant_number = if index == 0 {
                number
            } else {
                self.numbers.claim(number)
            };
            let bytes = self.text_bytes(variant_number, common, text, horizontal, vertical)?;
            self.written
                .items
                .push((SymbolIndexEntry::default(), bytes));
            numbered.push((horizontal, vertical, variant_number));
        }
        self.written
            .numbers
            .entry(id)
            .or_default()
            .push((number, Drawn::Text));
        self.written.text_variants.insert(id, numbered);
        Ok(())
    }

    fn text_bytes(
        &mut self,
        number: i32,
        common: &SymbolCommon,
        text: &TextSymbol,
        horizontal: HorizontalAlign,
        vertical: VerticalAlign,
    ) -> Result<Vec<u8>> {
        let code = common.code;
        let mut bytes = SymbolBytes::new(number, symbol_type::TEXT, common);
        if text.is_rotatable {
            bytes.base.flags |= BaseSymbol::ROTATABLE;
        }
        if text.underline {
            self.issue(code, "underlined text is not supported");
        }
        let horizontal = match horizontal {
            HorizontalAlign::Left => BasicText::LEFT,
            HorizontalAlign::HCenter => BasicText::CENTER,
            HorizontalAlign::Right => BasicText::RIGHT,
        };
        let vertical = match vertical {
            VerticalAlign::Top => BasicText::TOP,
            VerticalAlign::VCenter => BasicText::MIDDLE,
            VerticalAlign::Baseline => BasicText::BOTTOM,
            VerticalAlign::Bottom => {
                self.issue(
                    code,
                    "text aligned to its bottom was aligned to its baseline",
                );
                BasicText::BOTTOM
            }
        };
        let mut tail = TextTail {
            basic: BasicText {
                font_name: PascalString31::new(&text.font_family),
                color: self.color(&mut bytes, text.color).unwrap_or(0),
                font_size: (text.font_size.get() / MM_PER_POINT * 10.)
                    .round()
                    .clamp(0., f64::from(u16::MAX)) as u16,
                weight: if text.bold { 700 } else { 400 },
                italic: u8::from(text.italic),
                character_spacing: (text.character_spacing * 100.)
                    .round()
                    .clamp(f64::from(i16::MIN), f64::from(i16::MAX))
                    as i16,
                word_spacing: 100,
                alignment: horizontal | vertical,
                ..BasicText::default()
            },
            line_spacing: (text.line_spacing.get() * 100.)
                .round()
                .clamp(0., f64::from(u16::MAX)) as u16,
            paragraph_spacing: units_i16(text.paragraph_spacing),
            ..TextTail::default()
        };
        if let Some(below) = &text.line_below
            && let Some(color) = self.color(&mut bytes, below.color)
        {
            tail.line_below_on = 1;
            tail.line_below_color = color;
            tail.line_below_width = units_u16(below.width.get());
            tail.line_below_offset = units_u16(below.distance.get());
        }
        if text.custom_tabs.len() > tail.tabs.len() {
            self.issue(code, "tabs beyond 32 were dropped");
        }
        for (slot, tab) in tail.tabs.iter_mut().zip(&text.custom_tabs) {
            *slot = to_units(tab.get()) as i32;
            tail.tab_count += 1;
        }
        match &text.framing_mode {
            None | Some(FramingMode::NoFraming) => (),
            Some(FramingMode::ShadowFraming(shadow)) => {
                tail.framing.mode = Framing::SHADOW;
                tail.framing.color = self.color(&mut bytes, shadow.color).unwrap_or(0);
                tail.framing.offset_x = units_i16(shadow.shadow_offset.x);
                tail.framing.offset_y = units_i16(shadow.shadow_offset.y);
            }
            Some(FramingMode::LineFraming(line)) => {
                tail.framing.mode = Framing::LINE;
                tail.framing.color = self.color(&mut bytes, line.color).unwrap_or(0);
                tail.framing.line_width = units_u16(line.framing_line_half_width.get());
            }
        }
        bytes.append(&tail)?;
        bytes.finish(0)
    }

    /// One OCAD symbol per part of a combined symbol; its objects are
    /// written once with each part's number.
    fn breakdown(
        &mut self,
        id: SymbolId,
        number: i32,
        common: &SymbolCommon,
        parts: &[Part<'_>],
        kinds: &HashMap<SymbolId, Drawn>,
    ) -> Result<()> {
        let mut numbers = Vec::new();
        let mut own_number_used = false;
        for (index, part) in parts.iter().enumerate() {
            let mut part_common = common.clone();
            part_common.name = format!("{} - part {}", common.name, index + 1);
            match part {
                Part::Public(public) => match kinds.get(public) {
                    Some(&drawn) => {
                        if let Some(&public_number) = self.own_number.get(public) {
                            numbers.push((public_number, drawn));
                        }
                    }
                    // Broken down itself: its numbers are known once it is
                    // written.
                    None => self.pending_parts.push((id, *public)),
                },
                Part::Line(line) => {
                    let part_number = self.part_number(number, &mut own_number_used);
                    let bytes =
                        self.line_symbol(part_number, &part_common, LineParts::single(line))?;
                    self.written
                        .items
                        .push((SymbolIndexEntry::default(), bytes));
                    numbers.push((part_number, Drawn::Line));
                }
                Part::Area(area) => {
                    let part_number = self.part_number(number, &mut own_number_used);
                    let bytes = self.area_symbol(part_number, &part_common, area, None)?;
                    self.written
                        .items
                        .push((SymbolIndexEntry::default(), bytes));
                    numbers.push((part_number, Drawn::Area));
                }
            }
        }
        self.written.numbers.insert(id, numbers);
        Ok(())
    }

    /// Add the numbers of broken-down parts to the combined symbols that
    /// have them. The map holds no cycles, so taking first the parts with
    /// nothing pending settles them all.
    fn resolve_pending_parts(&mut self) {
        let mut pending = std::mem::take(&mut self.pending_parts);
        while !pending.is_empty() {
            let before = pending.len();
            let settled: Vec<(SymbolId, SymbolId)> = pending
                .iter()
                .copied()
                .filter(|&(_, part)| pending.iter().all(|&(combined, _)| combined != part))
                .collect();
            for (combined, part) in &settled {
                let numbers = self.written.numbers(*part).to_vec();
                self.written
                    .numbers
                    .entry(*combined)
                    .or_default()
                    .extend(numbers);
            }
            pending.retain(|pair| !settled.contains(pair));
            if pending.len() == before {
                break;
            }
        }
    }

    /// The combined symbol's own number for its first private part, a new
    /// one for every later part.
    fn part_number(&mut self, number: i32, own_number_used: &mut bool) -> i32 {
        if std::mem::replace(own_number_used, true) {
            self.numbers.claim(number)
        } else {
            number
        }
    }
}

/// Millimetres in a typographic point.
const MM_PER_POINT: f64 = 25.4 / 72.;
