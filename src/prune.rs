//! Dropping references to values that are no longer in their set.
//!
//! Removing a symbol or a color leaves every handle that named it dangling.
//! [`crate::Omap::prune_dangling_references`] walks the map and drops those
//! references: a dangling symbol reference becomes `None`, which writes as the
//! format's `-1`; a dangling color becomes
//! [`crate::colors::SymbolColor::NoColor`]; and a dangling combined-symbol or
//! mixed-color component is removed outright.
//!
//! A handle that still names a live value is left exactly as it is, so handles
//! taken before a prune survive it.

use std::collections::HashSet;

use crate::colors::{Color, ColorId, ColorKey, MixedColor, SymbolColor};
use crate::objects::MapObject;
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, BorderStyle, CombinedSymbol, Element, FillPattern, FramingMode,
    LineSymbol, PointSymbol, PublicOrPrivateSymbol, Symbol, SymbolId, SymbolKey, TextSymbol,
};

/// The handles still present in each set.
pub(crate) struct Live {
    pub(crate) symbols: HashSet<SymbolKey>,
    pub(crate) colors: HashSet<ColorKey>,
}

impl Live {
    fn color<T: Copy + Into<ColorId>>(&self, id: T) -> Option<T> {
        self.colors.contains(&id.into().0).then_some(id)
    }

    fn symbol<T: Copy + Into<SymbolId>>(&self, id: T) -> Option<T> {
        self.symbols.contains(&id.into().0).then_some(id)
    }
}

/// Drop every handle a value holds that no longer resolves.
///
/// `dangles` walks the same handles as `prune` without changing anything, so
/// a caller can leave alone, and keep sharing, every value a prune would not
/// change. Keep the two methods of each impl in step.
pub(crate) trait Prune {
    fn prune(&mut self, live: &Live);

    /// Whether [`Prune::prune`] would change this value.
    fn dangles(&self, live: &Live) -> bool;
}

impl Prune for SymbolColor {
    fn prune(&mut self, live: &Live) {
        if self.dangles(live) {
            *self = Self::NoColor;
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        self.id().is_some_and(|id| live.color(id).is_none())
    }
}

impl<T: Prune> Prune for Option<T> {
    fn prune(&mut self, live: &Live) {
        if let Some(value) = self {
            value.prune(live);
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        self.as_ref().is_some_and(|value| value.dangles(live))
    }
}

impl<T: Prune> Prune for Vec<T> {
    fn prune(&mut self, live: &Live) {
        for value in self.iter_mut() {
            value.prune(live);
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        self.iter().any(|value| value.dangles(live))
    }
}

impl<T: Prune> Prune for Box<T> {
    fn prune(&mut self, live: &Live) {
        (**self).prune(live);
    }

    fn dangles(&self, live: &Live) -> bool {
        (**self).dangles(live)
    }
}

impl Prune for Color {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::SpotColor(_) => (),
            Self::MixedColor(color) => color.prune(live),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::SpotColor(_) => false,
            Self::MixedColor(color) => color.dangles(live),
        }
    }
}

impl Prune for MixedColor {
    fn prune(&mut self, live: &Live) {
        self.components
            .retain(|component| live.color(component.color).is_some());
    }

    fn dangles(&self, live: &Live) -> bool {
        self.components
            .iter()
            .any(|component| live.color(component.color).is_none())
    }
}

impl Prune for PointSymbol {
    fn prune(&mut self, live: &Live) {
        self.inner_color.prune(live);
        self.outer_color.prune(live);
        self.elements.prune(live);
    }

    fn dangles(&self, live: &Live) -> bool {
        self.inner_color.dangles(live)
            || self.outer_color.dangles(live)
            || self.elements.dangles(live)
    }
}

impl Prune for Element {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::Point { symbol, .. } => symbol.prune(live),
            Self::Line { symbol, .. } => symbol.prune(live),
            Self::Area { symbol, .. } => symbol.prune(live),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::Point { symbol, .. } => symbol.dangles(live),
            Self::Line { symbol, .. } => symbol.dangles(live),
            Self::Area { symbol, .. } => symbol.dangles(live),
        }
    }
}

impl Prune for LineSymbol {
    fn prune(&mut self, live: &Live) {
        self.color.prune(live);
        self.border.prune(live);
        self.start_symbol.prune(live);
        self.end_symbol.prune(live);
        if let Some(mid) = &mut self.mid_symbol {
            mid.mid_symbol.prune(live);
        }
        if let Some(dash) = &mut self.dash_symbol {
            dash.dash_symbol.prune(live);
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        self.color.dangles(live)
            || self.border.dangles(live)
            || self.start_symbol.dangles(live)
            || self.end_symbol.dangles(live)
            || self
                .mid_symbol
                .as_ref()
                .is_some_and(|mid| mid.mid_symbol.dangles(live))
            || self
                .dash_symbol
                .as_ref()
                .is_some_and(|dash| dash.dash_symbol.dangles(live))
    }
}

impl Prune for BorderStyle {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::SymmetricBorder { both } => both.color.prune(live),
            Self::AsymmetricBorder { left, right } => {
                left.color.prune(live);
                right.color.prune(live);
            }
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::SymmetricBorder { both } => both.color.dangles(live),
            Self::AsymmetricBorder { left, right } => {
                left.color.dangles(live) || right.color.dangles(live)
            }
        }
    }
}

impl Prune for AreaSymbol {
    fn prune(&mut self, live: &Live) {
        self.color.prune(live);
        self.patterns.prune(live);
    }

    fn dangles(&self, live: &Live) -> bool {
        self.color.dangles(live) || self.patterns.dangles(live)
    }
}

impl Prune for FillPattern {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::LinePattern { line_color, .. } => line_color.prune(live),
            Self::PointPattern { point, .. } => point.prune(live),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::LinePattern { line_color, .. } => line_color.dangles(live),
            Self::PointPattern { point, .. } => point.dangles(live),
        }
    }
}

impl Prune for TextSymbol {
    fn prune(&mut self, live: &Live) {
        self.color.prune(live);
        if let Some(below) = &mut self.line_below {
            below.color.prune(live);
        }
        match &mut self.framing_mode {
            Some(FramingMode::LineFraming(framing)) => framing.color.prune(live),
            Some(FramingMode::ShadowFraming(framing)) => framing.color.prune(live),
            Some(FramingMode::NoFraming) | None => (),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        let framing = match &self.framing_mode {
            Some(FramingMode::LineFraming(framing)) => framing.color.dangles(live),
            Some(FramingMode::ShadowFraming(framing)) => framing.color.dangles(live),
            Some(FramingMode::NoFraming) | None => false,
        };
        framing
            || self.color.dangles(live)
            || self
                .line_below
                .as_ref()
                .is_some_and(|below| below.color.dangles(live))
    }
}

impl Prune for AreaOrLineSymbol {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::Area(symbol) => symbol.prune(live),
            Self::Line(symbol) => symbol.prune(live),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::Area(symbol) => symbol.dangles(live),
            Self::Line(symbol) => symbol.dangles(live),
        }
    }
}

/// A public component that is gone is dropped and a private one is pruned in
/// place. A public component's own colors belong to that symbol, which is
/// pruned on its own.
impl<Id: Copy + Into<SymbolId>, Private: Prune> Prune for CombinedSymbol<Id, Private> {
    fn prune(&mut self, live: &Live) {
        self.retain_map_components(|part| match part {
            PublicOrPrivateSymbol::Public(id) => live.symbol(id).map(PublicOrPrivateSymbol::Public),
            PublicOrPrivateSymbol::Private(mut symbol) => {
                symbol.prune(live);
                Some(PublicOrPrivateSymbol::Private(symbol))
            }
        });
    }

    fn dangles(&self, live: &Live) -> bool {
        self.components().any(|part| match part {
            PublicOrPrivateSymbol::Public(id) => live.symbol(*id).is_none(),
            PublicOrPrivateSymbol::Private(symbol) => symbol.dangles(live),
        })
    }
}

impl Prune for Symbol {
    fn prune(&mut self, live: &Live) {
        match self {
            Self::Line(symbol) => symbol.prune(live),
            Self::Area(symbol) => symbol.prune(live),
            Self::Point(symbol) => symbol.prune(live),
            Self::Text(symbol) => symbol.prune(live),
            Self::CombinedArea(symbol) => symbol.prune(live),
            Self::CombinedLine(symbol) => symbol.prune(live),
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        match self {
            Self::Line(symbol) => symbol.dangles(live),
            Self::Area(symbol) => symbol.dangles(live),
            Self::Point(symbol) => symbol.dangles(live),
            Self::Text(symbol) => symbol.dangles(live),
            Self::CombinedArea(symbol) => symbol.dangles(live),
            Self::CombinedLine(symbol) => symbol.dangles(live),
        }
    }
}

impl Prune for MapObject {
    fn prune(&mut self, live: &Live) {
        if self.dangles(live) {
            match self {
                Self::Point(object) => object.symbol = None,
                Self::Line(object) => object.symbol = None,
                Self::Area(object) => object.symbol = None,
                Self::Text(object) => object.symbol = None,
            }
        }
    }

    fn dangles(&self, live: &Live) -> bool {
        self.symbol()
            .is_some_and(|symbol| live.symbol(symbol).is_none())
    }
}
