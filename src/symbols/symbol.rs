use super::{
    AreaSymbol, CombinedAreaSymbol, CombinedLineSymbol, LineSymbol, PointSymbol, SymbolKind,
    SymbolSet, TextSymbol,
};

use crate::{Code, colors::ColorId};

/// Common properties shared by all symbol types.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymbolCommon {
    /// The symbol's name
    pub name: String,
    /// The symbol's code, of the form A.B.C
    pub code: Code,
    /// A description of the symbol
    pub description: String,
    /// Do not show the symbol on the printed map
    pub is_helper_symbol: bool,
    /// Hide the symbol in oomapper
    pub is_hidden: bool,
    /// Protect the symbol in oomapper
    pub is_protected: bool,
    /// base64 encoded symbol icon
    pub custom_icon: Option<String>,
}

/// A symbol of any type.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Symbol {
    /// A line symbol.
    Line(Box<LineSymbol>),
    /// An area symbol.
    Area(Box<AreaSymbol>),
    /// A point symbol.
    Point(Box<PointSymbol>),
    /// A text symbol.
    Text(Box<TextSymbol>),
    /// Combined symbols can be either `CombinedArea` or `CombinedLine`
    /// The difference is what object geometry to relate with the symbol
    /// Mapper does not discern between any line and area objects
    CombinedArea(Box<CombinedAreaSymbol>),
    /// A combined line symbol.
    CombinedLine(Box<CombinedLineSymbol>),
}

macro_rules! impl_from_symbol {
    ($symbol_ty:ty, $variant:ident) => {
        impl From<$symbol_ty> for Symbol {
            fn from(value: $symbol_ty) -> Self {
                Symbol::$variant(Box::new(value))
            }
        }
    };
}

impl_from_symbol!(AreaSymbol, Area);
impl_from_symbol!(LineSymbol, Line);
impl_from_symbol!(PointSymbol, Point);
impl_from_symbol!(TextSymbol, Text);
impl_from_symbol!(CombinedAreaSymbol, CombinedArea);
impl_from_symbol!(CombinedLineSymbol, CombinedLine);

macro_rules! impl_symbol_getter {
    ($method:ident -> $ret_type:ty, |$s:ident| $expr:expr) => {
        /// Read a property shared by every symbol type.
        pub fn $method(&self) -> $ret_type {
            let $s = self.common();
            $expr
        }
    };
}

macro_rules! impl_symbol_setter {
    ($method:ident($param:ident: $param_type:ty), |$s:ident| $expr:expr) => {
        /// Update a property shared by every symbol type.
        pub fn $method(&mut self, $param: $param_type) {
            let $s = self.common_mut();
            $expr;
        }
    };
}

impl Symbol {
    /// The [`SymbolCommon`] properties shared by every symbol type.
    pub fn common(&self) -> &SymbolCommon {
        match self {
            Self::Line(symbol) => &symbol.common,
            Self::Area(symbol) => &symbol.common,
            Self::Point(symbol) => &symbol.common,
            Self::Text(symbol) => &symbol.common,
            Self::CombinedLine(symbol) => &symbol.common,
            Self::CombinedArea(symbol) => &symbol.common,
        }
    }

    /// The [`SymbolCommon`] properties shared by every symbol type, mutably.
    pub fn common_mut(&mut self) -> &mut SymbolCommon {
        match self {
            Self::Line(symbol) => &mut symbol.common,
            Self::Area(symbol) => &mut symbol.common,
            Self::Point(symbol) => &mut symbol.common,
            Self::Text(symbol) => &mut symbol.common,
            Self::CombinedLine(symbol) => &mut symbol.common,
            Self::CombinedArea(symbol) => &mut symbol.common,
        }
    }

    /// Every color used in this symbol definition.
    ///
    /// Takes the [`SymbolSet`] that owns the symbol, because a combined
    /// symbol's public components live in the set rather than in the symbol.
    /// A component no longer in the set contributes no colors.
    pub fn colors<'a>(&'a self, symbol_set: &'a SymbolSet) -> impl Iterator<Item = ColorId> {
        crate::references::colors_through(self, symbol_set)
    }

    impl_symbol_getter!(custom_icon -> Option<&str>, |s| s.custom_icon.as_deref());
    impl_symbol_getter!(has_custom_icon -> bool, |s| s.custom_icon.is_some());
    impl_symbol_setter!(set_custom_icon(icon: Option<String>), |s| s.custom_icon =
        icon);
    impl_symbol_getter!(code -> Code, |s| s.code);
    impl_symbol_setter!(set_code(code: Code), |s| s.code = code);
    impl_symbol_getter!(is_helper_symbol -> bool, |s| s.is_helper_symbol);
    impl_symbol_setter!(set_helper_symbol(is_helper: bool), |s| s.is_helper_symbol =
        is_helper);
    impl_symbol_getter!(is_hidden -> bool, |s| s.is_hidden);
    impl_symbol_setter!(set_hidden(is_hidden: bool), |s| s.is_hidden = is_hidden);
    impl_symbol_getter!(is_protected -> bool, |s| s.is_protected);
    impl_symbol_setter!(set_protected(is_protected: bool), |s| s.is_protected =
        is_protected);
    impl_symbol_getter!(name -> &str, |s| s.name.as_str());
    impl_symbol_setter!(set_name(name: String), |s| s.name = name);
    impl_symbol_getter!(description -> &str, |s| s.description.as_str());
    impl_symbol_setter!(set_description(description: String), |s| s.description =
        description);

    /// The kind of this symbol, which is also the kind of any handle to it.
    pub fn kind(&self) -> SymbolKind {
        match self {
            Self::Line(_) => SymbolKind::Line,
            Self::Area(_) => SymbolKind::Area,
            Self::Point(_) => SymbolKind::Point,
            Self::Text(_) => SymbolKind::Text,
            Self::CombinedArea(_) => SymbolKind::CombinedArea,
            Self::CombinedLine(_) => SymbolKind::CombinedLine,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Symbol;
    use crate::{
        Code,
        symbols::{
            AreaSymbol, CombinedAreaSymbol, CombinedLineSymbol, LineSymbol, PointSymbol, TextSymbol,
        },
    };

    fn one_of_each() -> Vec<Symbol> {
        let code = Code::new(1, 2, 3);
        vec![
            LineSymbol::new(code, "line").into(),
            AreaSymbol::new(code, "area").into(),
            PointSymbol::new(code, "point").into(),
            TextSymbol::new(code, "text").into(),
            CombinedAreaSymbol::new(code, "combined area").into(),
            CombinedLineSymbol::new(code, "combined line").into(),
        ]
    }

    #[test]
    fn common_sees_the_same_values_as_the_individual_getters() {
        for mut symbol in one_of_each() {
            symbol.set_helper_symbol(true);
            symbol.set_description("a description".to_owned());

            let common = symbol.common();
            assert_eq!(common.code, symbol.code(), "code mismatch");
            assert_eq!(common.name, symbol.name(), "name mismatch");
            assert_eq!(common.description, "a description", "description mismatch");
            assert!(common.is_helper_symbol, "helper flag mismatch");
            assert!(!common.is_hidden, "hidden flag mismatch");
        }
    }

    #[test]
    fn common_mut_writes_through_to_the_getters() {
        for mut symbol in one_of_each() {
            {
                let common = symbol.common_mut();
                common.name = "renamed".to_owned();
                common.code = Code::new(4, 5, 6);
                common.is_hidden = true;
            }

            assert_eq!(symbol.name(), "renamed");
            assert_eq!(symbol.code(), Code::new(4, 5, 6));
            assert!(symbol.is_hidden());
        }
    }
}
