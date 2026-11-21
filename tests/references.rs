//! `validate` and `prune_dangling_references` agree on every map we have:
//! removing any one color or symbol makes `validate` fail exactly when
//! something referred to it, a prune makes it pass, and a second prune
//! changes nothing.

use std::sync::Arc;

use omap::{
    Omap,
    colors::{Color, ColorId},
    symbols::{Symbol, SymbolId},
};

const MAPS: [(&str, &[u8]); 4] = [
    (
        "isom_15000",
        include_bytes!("../src/default_maps/isom_15000.omap"),
    ),
    (
        "isom_10000",
        include_bytes!("../src/default_maps/isom_10000.omap"),
    ),
    (
        "issprom_4000",
        include_bytes!("../src/default_maps/issprom_4000.omap"),
    ),
    (
        "from_path",
        include_bytes!("../example_data/from_path.omap"),
    ),
];

fn maps() -> impl Iterator<Item = (&'static str, Omap)> {
    MAPS.into_iter().map(|(name, bytes)| {
        let map = omap::xml::read(bytes).expect("map parses");
        map.validate()
            .expect("a map as read has no dangling reference");
        (name, map)
    })
}

/// Whether a mixed color or a symbol in `map` uses `color`.
fn uses_color(map: &Omap, color: ColorId) -> bool {
    let mixed = map.colors.values().any(|value| match value {
        Color::MixedColor(mixed) => mixed
            .components
            .iter()
            .any(|component| ColorId::from(component.color) == color),
        Color::SpotColor(_) => false,
    });
    mixed
        || map
            .symbols
            .values()
            .any(|symbol| symbol.colors(&map.symbols).any(|used| used == color))
}

/// Whether an object or a combined symbol in `map` uses `symbol`.
fn uses_symbol(map: &Omap, symbol: SymbolId) -> bool {
    let part = map.symbols.values().any(|value| match value {
        Symbol::CombinedArea(c) => c.public_components().any(|id| SymbolId::from(id) == symbol),
        Symbol::CombinedLine(c) => c.public_components().any(|id| SymbolId::from(id) == symbol),
        _ => false,
    });
    part || map
        .iter_all_objects()
        .any(|object| object.symbol() == Some(symbol))
}

/// Every color, symbol and part of `a` is the very value `b` holds.
fn shares_everything(a: &Omap, b: &Omap) -> bool {
    let colors = a.colors.ids().all(|id| {
        Arc::ptr_eq(
            a.colors.get_arc(id).expect("resolves"),
            b.colors.get_arc(id).expect("resolves"),
        )
    });
    let symbols = a.symbols.ids().all(|id| {
        Arc::ptr_eq(
            a.symbols.get_arc(id).expect("resolves"),
            b.symbols.get_arc(id).expect("resolves"),
        )
    });
    let parts = a.parts.ids().all(|id| {
        Arc::ptr_eq(
            a.parts.get_arc(id).expect("resolves"),
            b.parts.get_arc(id).expect("resolves"),
        )
    });
    colors && symbols && parts
}

/// After `remove` takes one value out of a copy of `map`: `validate` fails
/// exactly when `used`, a prune fixes it, and pruning again copies nothing.
fn assert_agree(name: &str, map: &Omap, used: bool, remove: impl FnOnce(&mut Omap)) {
    let mut map = map.clone();
    remove(&mut map);
    assert_eq!(map.validate().is_err(), used, "{name}: validate");

    map.prune_dangling_references();
    map.validate()
        .unwrap_or_else(|error| panic!("{name}: still dangling after a prune: {error}"));

    let pruned = map.clone();
    map.prune_dangling_references();
    assert!(
        shares_everything(&map, &pruned),
        "{name}: a second prune copied something"
    );
}

#[test]
fn removing_any_color_and_pruning_agrees_with_validate() {
    for (name, map) in maps() {
        for color in map.colors.ids() {
            let used = uses_color(&map, color);
            assert_agree(name, &map, used, |map| {
                let _removed = map.colors.remove(color);
            });
        }
    }
}

#[test]
fn removing_any_symbol_and_pruning_agrees_with_validate() {
    for (name, map) in maps() {
        for symbol in map.symbols.ids() {
            let used = uses_symbol(&map, symbol);
            assert_agree(name, &map, used, |map| {
                let _removed = map.symbols.remove(symbol);
            });
        }
    }
}
