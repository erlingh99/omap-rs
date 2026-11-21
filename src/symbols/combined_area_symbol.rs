use super::{CombinedAreaSymbol, PublicOrPrivateSymbol};
use crate::{
    colors::ColorId,
    symbols::{AreaOrLineSymbol, Symbol, SymbolId, SymbolSet},
};

impl CombinedAreaSymbol {
    /// Get the minimum area (in paper dimensions mm²) among all area sub-symbols.
    ///
    /// Takes the [`SymbolSet`] that owns the public components. A component no
    /// longer in the set contributes nothing.
    pub fn minimum_area(&self, symbol_set: &SymbolSet) -> f64 {
        let mut min = f64::MAX;
        for part in self.components() {
            let area = match part {
                PublicOrPrivateSymbol::Public(Some(id)) => {
                    match symbol_set.get(SymbolId::from(*id)).map(crate::Entry::value) {
                        Some(Symbol::Area(symbol)) => symbol.minimum_area.get(),
                        Some(Symbol::CombinedArea(symbol)) => symbol.minimum_area(symbol_set),
                        _ => 0.,
                    }
                }
                PublicOrPrivateSymbol::Public(None) => 0.,
                PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(symbol)) => {
                    symbol.minimum_area.get()
                }
                PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Line(_)) => 0.,
            };
            if area > 0. {
                min = min.min(area);
            }
        }
        if min == f64::MAX { 0. } else { min }
    }

    /// Every color used in this symbol definition.
    ///
    /// Takes the [`SymbolSet`] that owns the public components. A component no
    /// longer in the set contributes no colors.
    pub fn colors<'a>(&'a self, symbol_set: &'a SymbolSet) -> impl Iterator<Item = ColorId> {
        crate::references::colors_through(self, symbol_set)
    }
}
