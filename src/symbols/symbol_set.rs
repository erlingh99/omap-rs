use super::combined_area_symbol::UnresolvedPart;
use std::collections::HashSet;

use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, Event},
};

use super::Symbol;
use crate::{
    Code, Collection, Entry, Error, OmapSection, Removed, Result,
    colors::ColorSet,
    symbols::{
        AreaOrLineSymbol, AreaPathSymbolId, AreaSymbol, AreaSymbolId, CombinedAreaSymbol,
        CombinedAreaSymbolId, CombinedLineSymbol, CombinedLineSymbolId, LinePathSymbolId,
        LineSymbol, LineSymbolId, PathSymbolId, PointSymbol, PointSymbolId, PublicOrPrivateSymbol,
        SymbolId, TextSymbol, TextSymbolId,
    },
    utils::{try_get_attr, try_get_attr_raw},
};

/// The map's symbols, in file order.
///
/// A symbol's position in the set is its file index, which is what the `.omap`
/// format stores in every object and every combined-symbol part. A
/// [`SymbolId`] keeps naming the same symbol across [`SymbolSet::sort`], and
/// stops resolving once that symbol is removed; see [`Collection`] for
/// everything a set does.
///
/// A [`SymbolId`] left over from a removed symbol is written as the format's
/// `-1` unknown-symbol sentinel.
pub type SymbolSet = Collection<SymbolId, Symbol>;

macro_rules! impl_typed_accessors {
    ($add:ident, $get:ident, $get_mut:ident, $iter:ident, $id:ident, $symbol:ident, $variant:ident) => {
        /// Add a symbol of this kind, returning the handle for that kind.
        ///
        /// The handle needs no narrowing conversion before it is given to an
        /// object, because the symbol's type already fixed its kind.
        pub fn $add(&mut self, symbol: $symbol) -> $id {
            $id(self.push(Symbol::$variant(Box::new(symbol))).0)
        }

        /// Get a symbol of this kind by its handle. Cannot return another kind.
        pub fn $get(&self, id: $id) -> Option<&$symbol> {
            match self.get(id)?.value() {
                Symbol::$variant(symbol) => Some(symbol),
                _ => None,
            }
        }

        /// Mutably get a symbol of this kind by its handle.
        pub fn $get_mut(&mut self, id: $id) -> Option<&mut $symbol> {
            match self.get_mut(id)? {
                Symbol::$variant(symbol) => Some(symbol),
                _ => None,
            }
        }

        /// Iterate over only the symbols of this kind, with their handles.
        pub fn $iter(&self) -> impl Iterator<Item = ($id, &$symbol)> {
            self.iter().filter_map(|entry| match entry.value() {
                Symbol::$variant(symbol) => Some(($id(entry.id().0), &**symbol)),
                _ => None,
            })
        }
    };
}

impl SymbolSet {
    impl_typed_accessors!(
        add_point_symbol,
        point_symbol,
        point_symbol_mut,
        iter_point_symbols,
        PointSymbolId,
        PointSymbol,
        Point
    );
    impl_typed_accessors!(
        add_line_symbol,
        line_symbol,
        line_symbol_mut,
        iter_line_symbols,
        LineSymbolId,
        LineSymbol,
        Line
    );
    impl_typed_accessors!(
        add_area_symbol,
        area_symbol,
        area_symbol_mut,
        iter_area_symbols,
        AreaSymbolId,
        AreaSymbol,
        Area
    );
    impl_typed_accessors!(
        add_text_symbol,
        text_symbol,
        text_symbol_mut,
        iter_text_symbols,
        TextSymbolId,
        TextSymbol,
        Text
    );
    impl_typed_accessors!(
        add_combined_area_symbol,
        combined_area_symbol,
        combined_area_symbol_mut,
        iter_combined_area_symbols,
        CombinedAreaSymbolId,
        CombinedAreaSymbol,
        CombinedArea
    );
    impl_typed_accessors!(
        add_combined_line_symbol,
        combined_line_symbol,
        combined_line_symbol_mut,
        iter_combined_line_symbols,
        CombinedLineSymbolId,
        CombinedLineSymbol,
        CombinedLine
    );

    /// Remove the first symbol with the given [`Code`], as
    /// [`Collection::remove`] does.
    pub fn remove_by_code(&mut self, code: Code) -> Option<Removed<SymbolId, Symbol>> {
        let position = self.values().position(|s| s.common().code == code)?;
        self.remove_at(position)
    }

    /// Remove the first symbol with the given name, as
    /// [`Collection::remove`] does.
    pub fn remove_by_name(&mut self, name: &str) -> Option<Removed<SymbolId, Symbol>> {
        let position = self.values().position(|s| s.common().name == name)?;
        self.remove_at(position)
    }

    /// Find a symbol by its [`Code`]. The first match is returned.
    pub fn find_by_code(&self, code: Code) -> Option<Entry<'_, SymbolId, Symbol>> {
        self.iter().find(|symbol| symbol.common().code == code)
    }

    /// Find a symbol by its display name. The first match is returned.
    pub fn find_by_name(&self, name: &str) -> Option<Entry<'_, SymbolId, Symbol>> {
        self.iter().find(|symbol| symbol.common().name == name)
    }

    /// Find all symbols matching the given major component in the symbols' [`Code`]
    pub fn find_all_by_code_major_component(
        &self,
        major: u16,
    ) -> impl Iterator<Item = Entry<'_, SymbolId, Symbol>> {
        self.iter().filter(move |s| s.common().code.major == major)
    }

    /// Find all symbols matching the given major and minor component in the symbols' [`Code`]
    pub fn find_all_by_code_major_and_minor_component(
        &self,
        major: u16,
        minor: u16,
    ) -> impl Iterator<Item = Entry<'_, SymbolId, Symbol>> {
        self.iter()
            .filter(move |s| s.common().code.major == major && s.common().code.minor == minor)
    }

    /// The integer the `.omap` format stores for a symbol reference. No symbol,
    /// or a removed one, is the format's `-1` sentinel.
    pub fn file_index(&self, id: Option<impl Into<SymbolId>>) -> i32 {
        id.and_then(|id| self.position(id))
            .map_or(-1, |index| index as i32)
    }

    /// Sort the symbols by [`Code`]. Every handle stays valid.
    pub fn sort(&mut self) {
        self.sort_by_key(|symbol| symbol.common().code);
    }
}

/// Narrowing a looked-up symbol to a typed handle.
///
/// A lookup and a narrowing compose in one expression:
///
/// ```
/// # use omap::{Code, Omap};
/// # fn example(map: &Omap) {
/// let gully = map
///     .symbols
///     .find_by_code(Code::new(107, 0, 0))
///     .and_then(|symbol| symbol.as_line_path());
/// # }
/// ```
impl Entry<'_, SymbolId, Symbol> {
    /// Narrow to a path handle, or `None` if this names another kind.
    pub fn as_path(self) -> Option<PathSymbolId> {
        PathSymbolId::try_from(self).ok()
    }

    /// Narrow to a line path handle, or `None` if this names another kind.
    pub fn as_line_path(self) -> Option<LinePathSymbolId> {
        LinePathSymbolId::try_from(self).ok()
    }

    /// Narrow to an area path handle, or `None` if this names another kind.
    pub fn as_area_path(self) -> Option<AreaPathSymbolId> {
        AreaPathSymbolId::try_from(self).ok()
    }

    /// Narrow to a point handle, or `None` if this names another kind.
    pub fn as_point(self) -> Option<PointSymbolId> {
        PointSymbolId::try_from(self).ok()
    }

    /// Narrow to a text handle, or `None` if this names another kind.
    pub fn as_text(self) -> Option<TextSymbolId> {
        TextSymbolId::try_from(self).ok()
    }

    /// Narrow to a line handle, or `None` if this names another kind.
    pub fn as_line(self) -> Option<LineSymbolId> {
        LineSymbolId::try_from(self).ok()
    }

    /// Narrow to an area handle, or `None` if this names another kind.
    pub fn as_area(self) -> Option<AreaSymbolId> {
        AreaSymbolId::try_from(self).ok()
    }

    /// Narrow to a combined line handle, or `None` if this names another kind.
    pub fn as_combined_line(self) -> Option<CombinedLineSymbolId> {
        CombinedLineSymbolId::try_from(self).ok()
    }

    /// Narrow to a combined area handle, or `None` if this names another kind.
    pub fn as_combined_area(self) -> Option<CombinedAreaSymbolId> {
        CombinedAreaSymbolId::try_from(self).ok()
    }
}

impl SymbolSet {
    /// Add a component to a combined area symbol.
    ///
    /// Cycle detection has to walk the whole set, which a `&mut` borrow of a
    /// single symbol cannot do, so the checked mutation lives here rather than
    /// on [`CombinedAreaSymbol`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::CyclicSymbolDefinition`] if the component would make
    /// the definition cyclic, or [`Error::SymbolConversionError`] if `target`
    /// does not name a combined area symbol in this set.
    pub fn add_area_component(
        &mut self,
        target: CombinedAreaSymbolId,
        component: PublicOrPrivateSymbol<PathSymbolId, AreaOrLineSymbol>,
    ) -> Result<()> {
        if let PublicOrPrivateSymbol::Public(Some(public)) = &component
            && self.would_cycle(target.into(), *public)
        {
            return Err(Error::CyclicSymbolDefinition);
        }
        self.combined_area_symbol_mut(target)
            .ok_or(Error::SymbolConversionError)?
            .push_component(component);
        Ok(())
    }

    /// Add a component to a combined line symbol.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CyclicSymbolDefinition`] if the component would make
    /// the definition cyclic, or [`Error::SymbolConversionError`] if `target`
    /// does not name a combined line symbol in this set.
    pub fn add_line_component(
        &mut self,
        target: CombinedLineSymbolId,
        component: PublicOrPrivateSymbol<LinePathSymbolId, LineSymbol>,
    ) -> Result<()> {
        if let PublicOrPrivateSymbol::Public(Some(public)) = &component
            && self.would_cycle(target.into(), (*public).into())
        {
            return Err(Error::CyclicSymbolDefinition);
        }
        self.combined_line_symbol_mut(target)
            .ok_or(Error::SymbolConversionError)?
            .push_component(component);
        Ok(())
    }

    /// Would adding `component` to `target` make the combined symbol
    /// definitions cyclic?
    ///
    /// One visited set, no raw pointers, no borrows, and no way to fail.
    pub fn would_cycle(&self, target: SymbolId, component: PathSymbolId) -> bool {
        let mut visited = HashSet::new();
        self.reaches(SymbolId::from(component), target, &mut visited)
    }

    fn reaches(&self, from: SymbolId, goal: SymbolId, visited: &mut HashSet<SymbolId>) -> bool {
        if from == goal {
            return true;
        }
        if !visited.insert(from) {
            return false;
        }
        let components: Vec<SymbolId> = match self.get(from).map(Entry::value) {
            Some(Symbol::CombinedArea(symbol)) => {
                symbol.public_components().map(Into::into).collect()
            }
            Some(Symbol::CombinedLine(symbol)) => {
                symbol.public_components().map(Into::into).collect()
            }
            _ => return false,
        };
        components
            .into_iter()
            .any(|next| self.reaches(next, goal, visited))
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
