use std::collections::HashSet;

use super::Symbol;
use crate::{
    Code, Collection, Entry, Error, Removed, Result,
    symbols::{
        AreaOrLineSymbol, AreaPathSymbolId, AreaSymbol, AreaSymbolId, CombinedAreaSymbol,
        CombinedAreaSymbolId, CombinedLineSymbol, CombinedLineSymbolId, LinePathSymbolId,
        LineSymbol, LineSymbolId, PathSymbolId, PointSymbol, PointSymbolId, PublicOrPrivateSymbol,
        SymbolId, TextSymbol, TextSymbolId,
    },
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
}
