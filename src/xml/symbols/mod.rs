//! The `<symbols>` section: the symbol set and each kind of symbol.

mod area;
mod combined;
mod line;
mod point;
mod text;

use crate::colors::ColorSet;
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, CombinedAreaSymbol, CombinedLineSymbol, LineSymbol, PointSymbol,
    PublicOrPrivateSymbol, Symbol, SymbolCommon, SymbolId, SymbolKind, SymbolSet, TextSymbol,
};
use crate::xml::attrs::{parse_attr, parse_attr_raw, try_get_attr, try_get_attr_raw};
use crate::{Code, Error, OmapSection, Result};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

/// Where a `<symbol>` element sits in the file, which decides whether its
/// opening tag carries an index and a code.
#[derive(Clone, Copy)]
pub(crate) enum SymbolPosition {
    /// A symbol in the set: index and code.
    Indexed(usize),
    /// A private sub-symbol of a combined symbol: code only.
    Private,
    /// A point-symbol element: neither.
    Element,
}

/// A public part read from the file, to resolve once every symbol is parsed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UnresolvedPart {
    /// The part's place among the combined symbol's parts.
    pub(crate) position: usize,
    /// The file index of the symbol it names.
    pub(crate) file_index: usize,
}

impl SymbolCommon {
    /// Write the `<symbol>` opening tag and the `<description>` that follows it.
    pub(crate) fn write_open<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_type: &str,
        position: SymbolPosition,
    ) -> Result<()> {
        let code = match position {
            SymbolPosition::Element => String::new(),
            SymbolPosition::Indexed(_) | SymbolPosition::Private => self.code.to_string(),
        };
        let mut bs = BytesStart::new("symbol").with_attributes([
            ("type", symbol_type),
            ("code", code.as_str()),
            ("name", self.name.as_str()),
        ]);
        if let SymbolPosition::Indexed(index) = position {
            bs.push_attribute(("id", index.to_string().as_str()));
        }
        if self.is_hidden {
            bs.push_attribute(("is_hidden", "true"));
        }
        if self.is_helper_symbol {
            bs.push_attribute(("is_helper_symbol", "true"));
        }
        if self.is_protected {
            bs.push_attribute(("is_protected", "true"));
        }
        writer.write_event(Event::Start(bs))?;

        if !self.description.is_empty() {
            writer.write_event(Event::Start(BytesStart::new("description")))?;
            writer.write_event(Event::Text(BytesText::new(&self.description)))?;
            writer.write_event(Event::End(BytesEnd::new("description")))?;
        }
        Ok(())
    }

    /// Write the `<icon>` and close the `<symbol>`.
    pub(crate) fn write_close<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        if let Some(icon) = &self.custom_icon {
            writer.write_event(Event::Empty(
                BytesStart::new("icon").with_attributes([("src", icon.as_str())]),
            ))?;
        }
        writer.write_event(Event::End(BytesEnd::new("symbol")))?;
        Ok(())
    }
}

impl Symbol {
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        color_set: &ColorSet,
    ) -> Result<(usize, Self, Vec<UnresolvedPart>)> {
        let mut id = usize::MAX;
        let mut symbol_type = u8::MAX;
        let mut common = SymbolCommon::default();
        for attr in element.attributes().filter_map(std::result::Result::ok) {
            match attr.key.local_name().as_ref() {
                "type" => symbol_type = parse_attr_raw(attr.value).unwrap_or(symbol_type),
                "name" => common.name = parse_attr(attr).unwrap_or(common.name),
                "code" => common.code = parse_attr_raw(attr.value).unwrap_or(common.code),
                "id" => id = parse_attr_raw(attr.value).unwrap_or(id),
                "is_helper_symbol" => {
                    common.is_helper_symbol = attr.as_bool().unwrap_or(false);
                }
                "is_hidden" => {
                    common.is_hidden = attr.as_bool().unwrap_or(false);
                }
                "is_protected" => {
                    common.is_protected = attr.as_bool().unwrap_or(false);
                }
                _ => {}
            }
        }

        if id == usize::MAX {
            return Err(Error::MissingSymbolId);
        }

        // Components can only be resolved once every symbol is parsed.
        let mut unresolved = Vec::new();
        let symbol = match symbol_type {
            1 => Self::Point(Box::new(PointSymbol::parse(reader, color_set, common)?)),
            2 => Self::Line(Box::new(LineSymbol::parse(reader, color_set, common)?)),
            4 => Self::Area(Box::new(AreaSymbol::parse(reader, color_set, common)?)),
            8 => Self::Text(Box::new(TextSymbol::parse(reader, color_set, common)?)),
            16 => {
                // Reclassified as a line symbol later if its components say so.
                let (symbol, parts) = CombinedAreaSymbol::parse(reader, color_set, common)?;
                unresolved = parts;

                Self::CombinedArea(Box::new(symbol))
            }
            _ => {
                return Err(Error::UnknownSymbolType(symbol_type));
            }
        };

        Ok((id, symbol, unresolved))
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbol_set: &SymbolSet,
        color_set: &ColorSet,
        index: usize,
    ) -> Result<()> {
        let common = self.common();
        common.write_open(
            writer,
            self.kind().type_id(),
            SymbolPosition::Indexed(index),
        )?;
        match self {
            Self::Line(symbol) => symbol.write_body(writer, color_set),
            Self::Area(symbol) => symbol.write_body(writer, color_set),
            Self::Point(symbol) => symbol.write_body(writer, color_set),
            Self::Text(symbol) => symbol.write_body(writer, color_set),
            Self::CombinedArea(symbol) => symbol.write_body(writer, symbol_set, color_set),
            Self::CombinedLine(symbol) => symbol.write_body(writer, symbol_set, color_set),
        }?;
        common.write_close(writer)
    }
}

impl SymbolSet {
    /// The integer the `.omap` format stores for a symbol reference. No symbol,
    /// or a removed one, is the format's `-1` sentinel.
    pub(crate) fn file_index(&self, id: Option<impl Into<SymbolId>>) -> i32 {
        id.and_then(|id| self.position(id))
            .map_or(-1, |index| index as i32)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "symbol-set parsing also resolves combined-symbol references"
    )]
    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        colors: &ColorSet,
    ) -> Result<(Self, String)> {
        let symbol_set_name = try_get_attr(element, "id")?.unwrap_or_else(|| "Custom".to_owned());
        let count = try_get_attr_raw(element, "count")
            .ok()
            .flatten()
            .unwrap_or(1);

        let mut symbols = vec![None; count];
        let mut components = vec![Vec::new(); count];

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "symbol") {
                        let (symbol_id, symbol, combined_components) =
                            Symbol::parse(reader, &bytes_start, colors)?;
                        if symbol_id >= symbols.len() {
                            return Err(Error::SymbolIdOutOfRange(symbol_id));
                        }
                        if symbols[symbol_id].is_some() {
                            return Err(Error::DuplicateSymbolId(symbol_id));
                        }
                        components[symbol_id] = combined_components;
                        symbols[symbol_id] = Some(symbol);
                    }
                }
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "symbols") {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::Symbols));
                }
                _ => (),
            }
        }
        let mut symbols: Vec<Symbol> = symbols
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::SymbolCountMismatch)?;

        // The format has no combined line symbol, so one arrives as a combined area
        // symbol that references only lines. Only private parts are populated yet.
        let mut candidate_indices: Vec<usize> = Vec::new();
        for (i, symbol) in symbols.iter().enumerate() {
            if let Symbol::CombinedArea(combined) = symbol {
                let has_private_area = combined.components().any(|p| {
                    matches!(p, PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(_)))
                });
                if has_private_area {
                    continue;
                }
                let has_area_public = components[i].iter().any(|part| {
                    matches!(
                        symbols.get(part.file_index),
                        // Point and text is not allowed in combined symbol, that is treated later on
                        Some(Symbol::Area(_) | Symbol::Point(_) | Symbol::Text(_))
                    )
                });
                if has_area_public {
                    continue;
                }
                candidate_indices.push(i);
            }
        }

        // Referencing a combined area symbol that is not itself a candidate
        // disqualifies a candidate; referencing a fellow candidate does not.
        loop {
            let prev_len = candidate_indices.len();
            let current_candidates = candidate_indices.clone();
            candidate_indices.retain(|&idx| {
                !components[idx].iter().any(|part| {
                    matches!(symbols.get(part.file_index), Some(Symbol::CombinedArea(_)))
                        && !current_candidates.contains(&part.file_index)
                })
            });
            if candidate_indices.len() == prev_len {
                break;
            }
        }

        for &idx in &candidate_indices {
            let Symbol::CombinedArea(combined) = &mut symbols[idx] else {
                continue;
            };
            let mut converted = CombinedLineSymbol::new(Code::default(), String::new());
            converted.common = std::mem::take(&mut combined.common);
            let part_count = combined.components().count();
            // Public parts are still unresolved, so only their places carry over.
            for _ in 0..part_count {
                match combined.remove_component(0) {
                    Some(PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Line(line))) => {
                        converted.push_component(PublicOrPrivateSymbol::Private(line));
                    }
                    Some(PublicOrPrivateSymbol::Public(_)) => {
                        converted.push_component(PublicOrPrivateSymbol::Public(None));
                    }
                    Some(PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(_))) | None => {}
                }
            }
            symbols[idx] = Symbol::CombinedLine(Box::new(converted));
        }

        let mut symbol_set: Self = symbols.into_iter().collect();

        // After conversion, so the handles name the correct types.
        for (index, parts) in components.into_iter().enumerate() {
            if parts.is_empty() {
                continue;
            }
            let Some(target) = symbol_set.at(index) else {
                continue;
            };
            let combined_area = target.as_combined_area();
            let combined_line = target.as_combined_line();

            for UnresolvedPart {
                position,
                file_index,
            } in parts
            {
                let component = symbol_set
                    .at(file_index)
                    .ok_or(Error::SymbolSetIndexOutOfRange(file_index))?;
                let kind = component.kind();

                if let Some(target) = combined_area {
                    let Some(path) = component.as_path() else {
                        return Err(Error::CombinedSymbolContainsPointOrText(kind));
                    };
                    if symbol_set.would_cycle(target.into(), path) {
                        return Err(Error::CyclicSymbolDefinition);
                    }
                    symbol_set
                        .combined_area_symbol_mut(target)
                        .ok_or(Error::SymbolConversionError)?
                        .set_public(position, path);
                } else if let Some(target) = combined_line {
                    let Some(line) = component.as_line_path() else {
                        return Err(Error::CombinedLineSymbolContainsNonLine(kind));
                    };
                    if symbol_set.would_cycle(target.into(), line.into()) {
                        return Err(Error::CyclicSymbolDefinition);
                    }
                    symbol_set
                        .combined_line_symbol_mut(target)
                        .ok_or(Error::SymbolConversionError)?
                        .set_public(position, line);
                } else {
                    return Err(Error::ComponentsInNonCombinedSymbol);
                }
            }
        }

        Ok((symbol_set, symbol_set_name))
    }

    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        colors: &ColorSet,
        name: &str,
    ) -> Result<()> {
        writer.write_event(Event::Start(BytesStart::new("symbols").with_attributes([
            ("count", self.len().to_string().as_str()),
            ("id", name),
        ])))?;
        writer.get_mut().write_all(b"\n".as_slice())?;
        for (index, symbol) in self.values().enumerate() {
            symbol.write(writer, self, colors, index)?;
            writer.get_mut().write_all(b"\n".as_slice())?;
        }
        writer.write_event(Event::End(BytesEnd::new("symbols")))?;
        Ok(())
    }
}

impl SymbolKind {
    /// The `type` attribute the `.omap` format stores for this kind. The format
    /// has no combined line symbol, so both combined kinds write `16`.
    pub(crate) fn type_id(self) -> &'static str {
        match self {
            Self::Point => "1",
            Self::Line => "2",
            Self::Area => "4",
            Self::Text => "8",
            Self::CombinedArea | Self::CombinedLine => "16",
        }
    }
}
