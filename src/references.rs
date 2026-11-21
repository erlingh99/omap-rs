//! The references a map value holds to colors and symbols.
//!
//! Every value that can name a color or a symbol says which ones through
//! [`References::references`], and drops the ones that no longer resolve
//! through [`References::prune`]. Everything else is derived from those two:
//! [`References::dangles`], [`crate::Omap::validate`],
//! [`crate::Omap::prune_dangling_references`] and the `colors()` methods of the
//! symbols.
//!
//! A value's references are its own fields and those of any symbol embedded in
//! it (a line's start symbol, a point's elements, a combined symbol's private
//! parts). A public combined-symbol part is one symbol reference: the colors of
//! that symbol are its own, not the combined symbol's.
//!
//! Pruning follows the format, as Mapper does when a color or symbol is deleted:
//! a dangling color becomes [`SymbolColor::NoColor`], a dangling symbol becomes
//! `None` (written as `-1`), and a dangling mixed-color component is removed.
//! A reference that still resolves is left exactly as it is.

use std::collections::HashSet;

use crate::colors::{Color, ColorId, ColorKey, ColorSet, MixedColor, SymbolColor};
use crate::objects::MapObject;
use crate::symbols::{
    AreaOrLineSymbol, AreaSymbol, BorderStyle, CombinedSymbol, Element, FillPattern, FramingMode,
    LineSymbol, PointSymbol, PublicOrPrivateSymbol, Symbol, SymbolId, SymbolKey, SymbolSet,
    TextSymbol,
};

/// A color or symbol a value refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reference {
    Color(ColorId),
    Symbol(SymbolId),
}

/// The handles still present in each set.
///
/// A snapshot, so the sets can be changed while it is consulted.
pub(crate) struct Live {
    symbols: HashSet<SymbolKey>,
    colors: HashSet<ColorKey>,
}

impl Live {
    pub(crate) fn of(colors: &ColorSet, symbols: &SymbolSet) -> Self {
        Self {
            symbols: symbols.ids().map(|id| id.0).collect(),
            colors: colors.ids().map(|id| id.0).collect(),
        }
    }

    pub(crate) fn holds(&self, reference: Reference) -> bool {
        match reference {
            Reference::Color(id) => self.colors.contains(&id.0),
            Reference::Symbol(id) => self.symbols.contains(&id.0),
        }
    }

    fn holds_color(&self, id: impl Into<ColorId>) -> bool {
        self.holds(Reference::Color(id.into()))
    }

    fn holds_symbol(&self, id: impl Into<SymbolId>) -> bool {
        self.holds(Reference::Symbol(id.into()))
    }
}

/// The colors and symbols a value refers to.
pub(crate) trait References {
    /// Call `visit` with every color and symbol this value refers to.
    fn references(&self, visit: &mut impl FnMut(Reference));

    /// Drop every reference `live` does not hold.
    ///
    /// Must reach exactly the references [`References::references`] visits:
    /// after a prune, [`References::dangles`] is false.
    fn prune(&mut self, live: &Live);

    /// Whether this value refers to anything `live` does not hold, which is
    /// whether [`References::prune`] would change it.
    fn dangles(&self, live: &Live) -> bool {
        let mut dangles = false;
        self.references(&mut |reference| dangles |= !live.holds(reference));
        dangles
    }

    /// The first reference `live` does not hold.
    fn first_dangling(&self, live: &Live) -> Option<Reference> {
        let mut first = None;
        self.references(&mut |reference| {
            if first.is_none() && !live.holds(reference) {
                first = Some(reference);
            }
        });
        first
    }
}

/// Every color `value` refers to itself, in walk order.
pub(crate) fn own_colors(value: &impl References) -> std::vec::IntoIter<ColorId> {
    let mut colors = Vec::new();
    value.references(&mut |reference| {
        if let Reference::Color(id) = reference {
            colors.push(id);
        }
    });
    colors.into_iter()
}

/// Every color `symbol` refers to, following public combined-symbol parts
/// into `symbol_set`. A part no longer in the set contributes no colors.
pub(crate) fn colors_through(
    symbol: &impl References,
    symbol_set: &SymbolSet,
) -> std::vec::IntoIter<ColorId> {
    let mut colors = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = Vec::new();
    let mut collect = |reference| match reference {
        Reference::Color(id) => colors.push(id),
        Reference::Symbol(id) => pending.push(id),
    };
    symbol.references(&mut collect);
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(part) = symbol_set.get(id) {
            part.symbol().references(&mut |reference| match reference {
                Reference::Color(id) => colors.push(id),
                Reference::Symbol(id) => pending.push(id),
            });
        }
    }
    colors.into_iter()
}

impl References for SymbolColor {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        if let Some(id) = self.id() {
            visit(Reference::Color(id));
        }
    }

    fn prune(&mut self, live: &Live) {
        if self.id().is_some_and(|id| !live.holds_color(id)) {
            *self = Self::NoColor;
        }
    }
}

impl<T: References> References for Option<T> {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        if let Some(value) = self {
            value.references(visit);
        }
    }

    fn prune(&mut self, live: &Live) {
        if let Some(value) = self {
            value.prune(live);
        }
    }
}

impl<T: References> References for Vec<T> {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        for value in self {
            value.references(visit);
        }
    }

    fn prune(&mut self, live: &Live) {
        for value in self {
            value.prune(live);
        }
    }
}

impl<T: References> References for Box<T> {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        (**self).references(visit);
    }

    fn prune(&mut self, live: &Live) {
        (**self).prune(live);
    }
}

impl References for Color {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::SpotColor(_) => (),
            Self::MixedColor(color) => color.references(visit),
        }
    }

    fn prune(&mut self, live: &Live) {
        match self {
            Self::SpotColor(_) => (),
            Self::MixedColor(color) => color.prune(live),
        }
    }
}

/// A share of a removed spot color means nothing, so it is removed, as
/// Mapper does. A mixed color can be left with no components.
impl References for MixedColor {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        for component in &self.components {
            visit(Reference::Color(component.color.into()));
        }
    }

    fn prune(&mut self, live: &Live) {
        self.components
            .retain(|component| live.holds_color(component.color));
    }
}

impl References for PointSymbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        self.inner_color.references(visit);
        self.outer_color.references(visit);
        self.elements.references(visit);
    }

    fn prune(&mut self, live: &Live) {
        self.inner_color.prune(live);
        self.outer_color.prune(live);
        self.elements.prune(live);
    }
}

impl References for Element {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::Point { symbol, .. } => symbol.references(visit),
            Self::Line { symbol, .. } => symbol.references(visit),
            Self::Area { symbol, .. } => symbol.references(visit),
        }
    }

    fn prune(&mut self, live: &Live) {
        match self {
            Self::Point { symbol, .. } => symbol.prune(live),
            Self::Line { symbol, .. } => symbol.prune(live),
            Self::Area { symbol, .. } => symbol.prune(live),
        }
    }
}

impl References for LineSymbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        self.color.references(visit);
        self.border.references(visit);
        self.start_symbol.references(visit);
        self.end_symbol.references(visit);
        if let Some(mid) = &self.mid_symbol {
            mid.mid_symbol.references(visit);
        }
        if let Some(dash) = &self.dash_symbol {
            dash.dash_symbol.references(visit);
        }
    }

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
}

impl References for BorderStyle {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::SymmetricBorder { both } => both.color.references(visit),
            Self::AsymmetricBorder { left, right } => {
                left.color.references(visit);
                right.color.references(visit);
            }
        }
    }

    fn prune(&mut self, live: &Live) {
        match self {
            Self::SymmetricBorder { both } => both.color.prune(live),
            Self::AsymmetricBorder { left, right } => {
                left.color.prune(live);
                right.color.prune(live);
            }
        }
    }
}

impl References for AreaSymbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        self.color.references(visit);
        self.patterns.references(visit);
    }

    fn prune(&mut self, live: &Live) {
        self.color.prune(live);
        self.patterns.prune(live);
    }
}

impl References for FillPattern {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::LinePattern { line_color, .. } => line_color.references(visit),
            Self::PointPattern { point, .. } => point.references(visit),
        }
    }

    fn prune(&mut self, live: &Live) {
        match self {
            Self::LinePattern { line_color, .. } => line_color.prune(live),
            Self::PointPattern { point, .. } => point.prune(live),
        }
    }
}

impl References for TextSymbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        self.color.references(visit);
        if let Some(below) = &self.line_below {
            below.color.references(visit);
        }
        match &self.framing_mode {
            Some(FramingMode::LineFraming(framing)) => framing.color.references(visit),
            Some(FramingMode::ShadowFraming(framing)) => framing.color.references(visit),
            Some(FramingMode::NoFraming) | None => (),
        }
    }

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
}

impl References for AreaOrLineSymbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::Area(symbol) => symbol.references(visit),
            Self::Line(symbol) => symbol.references(visit),
        }
    }

    fn prune(&mut self, live: &Live) {
        match self {
            Self::Area(symbol) => symbol.prune(live),
            Self::Line(symbol) => symbol.prune(live),
        }
    }
}

/// A public part is a symbol reference: a gone one becomes `None` and keeps
/// its place, as Mapper does. A private part is walked like any embedded
/// symbol.
impl<Id: Copy + Into<SymbolId>, Private: References> References for CombinedSymbol<Id, Private> {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        for part in self.components() {
            match part {
                PublicOrPrivateSymbol::Public(Some(id)) => visit(Reference::Symbol((*id).into())),
                PublicOrPrivateSymbol::Public(None) => (),
                PublicOrPrivateSymbol::Private(symbol) => symbol.references(visit),
            }
        }
    }

    fn prune(&mut self, live: &Live) {
        for part in self.components_mut() {
            match part {
                PublicOrPrivateSymbol::Public(id) => {
                    if id.is_some_and(|id| !live.holds_symbol(id)) {
                        *id = None;
                    }
                }
                PublicOrPrivateSymbol::Private(symbol) => symbol.prune(live),
            }
        }
    }
}

impl References for Symbol {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        match self {
            Self::Line(symbol) => symbol.references(visit),
            Self::Area(symbol) => symbol.references(visit),
            Self::Point(symbol) => symbol.references(visit),
            Self::Text(symbol) => symbol.references(visit),
            Self::CombinedArea(symbol) => symbol.references(visit),
            Self::CombinedLine(symbol) => symbol.references(visit),
        }
    }

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
}

impl References for MapObject {
    fn references(&self, visit: &mut impl FnMut(Reference)) {
        if let Some(id) = self.symbol() {
            visit(Reference::Symbol(id));
        }
    }

    fn prune(&mut self, live: &Live) {
        if self.symbol().is_some_and(|id| !live.holds_symbol(id)) {
            match self {
                Self::Point(object) => object.symbol = None,
                Self::Line(object) => object.symbol = None,
                Self::Area(object) => object.symbol = None,
                Self::Text(object) => object.symbol = None,
            }
        }
    }
}

#[expect(clippy::unwrap_used, reason = "tests build known-good values")]
#[cfg(test)]
mod tests {
    use geo_types::{LineString, Point};

    use super::{Live, Reference, References};
    use crate::{
        Code, NonNegativeF64, UnitF64,
        colors::{
            Cmyk, Color, ColorComponent, ColorId, ColorSet, MixedColor, SpotColor, SpotColorId,
            SymbolColor,
        },
        objects::{LineObject, MapObject, PointObject},
        symbols::{
            AreaOrLineSymbol, AreaSymbol, BorderStyle, ClippingOption, CombinedAreaSymbol,
            DashSymbol, Element, FillPattern, FramingMode, LineBelow, LineFraming, LineSymbol,
            LineSymbolBorder, MidSymbol, MidSymbolPlacement, PathSymbolId, PointSymbol,
            PointSymbolId, PublicOrPrivateSymbol, Symbol, SymbolSet, TextSymbol,
        },
    };

    /// Handles to a color and two symbols that have all been removed again.
    struct Gone {
        color: ColorId,
        spot: SpotColorId,
        point: PointSymbolId,
        path: PathSymbolId,
        live: Live,
    }

    fn gone() -> Gone {
        let mut colors = ColorSet::new();
        let cmyk = Cmyk::new(0., 0., 0., 1.).unwrap();
        let color = colors.push(SpotColor::new("black", "black", cmyk));
        let spot = colors.get(color).unwrap().as_spot().unwrap();
        let _removed = colors.remove(color);

        let mut symbols = SymbolSet::new("test");
        let point = symbols.add_point_symbol(PointSymbol::new(Code::default(), "point"));
        let line = symbols.add_line_symbol(LineSymbol::new(Code::default(), "line"));
        let path = symbols.get(line.into()).unwrap().as_path().unwrap();
        let _removed = symbols.remove(point.into());
        let _removed = symbols.remove(line.into());

        Gone {
            color,
            spot,
            point,
            path,
            live: Live::of(&colors, &symbols),
        }
    }

    fn color(gone: &Gone) -> SymbolColor {
        SymbolColor::Color(gone.color)
    }

    /// A point symbol with every reference set: 4 colors.
    fn point(gone: &Gone) -> PointSymbol {
        let mut inner = PointSymbol::new(Code::default(), "inner");
        inner.inner_color = color(gone);
        let mut line = LineSymbol::new(Code::default(), "element");
        line.color = color(gone);

        let mut point = PointSymbol::new(Code::default(), "point");
        point.inner_color = color(gone);
        point.outer_color = color(gone);
        point.elements = vec![
            Element::Point {
                symbol: Box::new(inner),
                object: Box::new(PointObject::new_element(Point::new(0., 0.))),
            },
            Element::Line {
                symbol: Box::new(line),
                object: Box::new(LineObject::new_element(LineString::from(vec![
                    (0., 0.),
                    (1., 1.),
                ]))),
            },
        ];
        point
    }

    /// A line symbol with every reference set: 3 colors and 4 points.
    fn line(gone: &Gone) -> LineSymbol {
        let border = || LineSymbolBorder {
            color: color(gone),
            width: NonNegativeF64::default(),
            shift: NonNegativeF64::default(),
            dashed: None,
        };
        let mut line = LineSymbol::new(Code::default(), "line");
        line.color = color(gone);
        line.border = Some(BorderStyle::AsymmetricBorder {
            left: border(),
            right: border(),
        });
        line.start_symbol = Some(Box::new(point(gone)));
        line.end_symbol = Some(Box::new(point(gone)));
        line.mid_symbol = Some(Box::new(MidSymbol {
            mid_symbols_per_spot: 1,
            mid_symbol_distance: NonNegativeF64::default(),
            minimum_mid_symbol_count: 0,
            minimum_mid_symbol_count_when_closed: 0,
            show_at_least_one_mid_symbol: false,
            mid_symbol_placement: MidSymbolPlacement::default(),
            mid_symbol: point(gone),
        }));
        line.dash_symbol = Some(Box::new(DashSymbol {
            suppress_dash_symbol_at_ends: false,
            scale_dash_symbol: false,
            dash_symbol: point(gone),
        }));
        line
    }

    /// An area symbol with every reference set: 2 colors and a point.
    fn area(gone: &Gone) -> AreaSymbol {
        let mut area = AreaSymbol::new(Code::default(), "area");
        area.color = color(gone);
        area.patterns = vec![
            FillPattern::LinePattern {
                angle: 0.,
                line_spacing: NonNegativeF64::default(),
                line_offset: NonNegativeF64::default(),
                line_color: color(gone),
                line_width: NonNegativeF64::default(),
                rotatable: false,
            },
            FillPattern::PointPattern {
                clip_options: ClippingOption::default(),
                angle: 0.,
                line_spacing: NonNegativeF64::default(),
                line_offset: NonNegativeF64::default(),
                offset_along_line: NonNegativeF64::default(),
                point_distance: NonNegativeF64::default(),
                point: point(gone),
                rotatable: false,
            },
        ];
        area
    }

    /// A text symbol with every reference set: 3 colors.
    fn text(gone: &Gone) -> TextSymbol {
        let mut text = TextSymbol::new(Code::default(), "text");
        text.color = color(gone);
        text.line_below = Some(LineBelow {
            color: color(gone),
            width: NonNegativeF64::default(),
            distance: NonNegativeF64::default(),
        });
        text.framing_mode = Some(FramingMode::LineFraming(LineFraming {
            color: color(gone),
            framing_line_half_width: NonNegativeF64::default(),
        }));
        text
    }

    /// A public part, then a private area and a private line.
    fn combined(gone: &Gone) -> CombinedAreaSymbol {
        let mut combined = CombinedAreaSymbol::new(Code::default(), "combined");
        combined.push_component(PublicOrPrivateSymbol::Public(Some(gone.path)));
        combined.push_component(PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Area(
            area(gone),
        )));
        combined.push_component(PublicOrPrivateSymbol::Private(AreaOrLineSymbol::Line(
            line(gone),
        )));
        combined
    }

    fn count(value: &impl References) -> usize {
        let mut count = 0;
        value.references(&mut |_| count += 1);
        count
    }

    /// `references` visits every field set, and `prune` drops every one it
    /// visits, so the two cannot drift apart unnoticed.
    fn assert_prunes_all(mut value: impl References, live: &Live, references: usize) {
        assert_eq!(count(&value), references, "references visits every field");
        assert!(value.dangles(live), "every reference is gone");
        value.prune(live);
        assert!(!value.dangles(live), "prune reaches what references visits");
        assert_eq!(
            count(&value),
            0,
            "every reference was gone, so none is left"
        );
    }

    #[test]
    fn prune_reaches_every_reference_of_every_value() {
        let gone = gone();
        let live = &gone.live;
        let point_refs = 4;
        let line_refs = 3 + 4 * point_refs;
        let area_refs = 2 + point_refs;

        assert_prunes_all(Symbol::Point(Box::new(point(&gone))), live, point_refs);
        assert_prunes_all(Symbol::Line(Box::new(line(&gone))), live, line_refs);
        assert_prunes_all(Symbol::Area(Box::new(area(&gone))), live, area_refs);
        assert_prunes_all(Symbol::Text(Box::new(text(&gone))), live, 3);
        assert_prunes_all(
            Symbol::CombinedArea(Box::new(combined(&gone))),
            live,
            1 + area_refs + line_refs,
        );
        let component = ColorComponent {
            factor: UnitF64::try_from(1.).unwrap(),
            color: gone.spot,
        };
        assert_prunes_all(
            Color::MixedColor(MixedColor::new("mix", vec![component])),
            live,
            1,
        );
        assert_prunes_all(
            MapObject::Point(PointObject::new(Some(gone.point), Point::new(0., 0.))),
            live,
            1,
        );
    }

    #[test]
    fn a_public_part_is_one_symbol_reference_and_keeps_its_place() {
        let gone = gone();
        let mut combined = combined(&gone);
        let mut first = None;
        combined.references(&mut |reference| {
            first.get_or_insert(reference);
        });
        assert_eq!(first, Some(Reference::Symbol(gone.path.into())));

        combined.prune(&gone.live);

        assert_eq!(combined.num_components(), 3, "no part is dropped");
        assert!(matches!(
            combined.components().next(),
            Some(PublicOrPrivateSymbol::Public(None))
        ));
    }
}
