//! A copy of a map shares its colors, symbols, parts and objects with the
//! original until one side changes them, and a removed item can be restored
//! under its own handle.

#![expect(
    clippy::expect_used,
    reason = "a test should fail loudly at the point it breaks"
)]

use std::sync::Arc;

use omap::objects::{MapObject, ObjectId};
use omap::parts::{MapPart, MapParts};
use omap::symbols::SymbolId;
use omap::{Omap, Result};

const FROM_PATH: &[u8] = include_bytes!("../example_data/from_path.omap");
const ISOM_15000: &[u8] = include_bytes!("../src/default_maps/isom_15000.omap");

fn from_path() -> Omap {
    Omap::from_bytes(FROM_PATH).expect("from_path.omap parses")
}

fn first_part(map: &Omap) -> &MapPart {
    map.parts.get(0).expect("the map has a part")
}

fn first_part_mut(map: &mut Omap) -> &mut MapPart {
    map.parts.get_mut(0).expect("the map has a part")
}

fn object_ids(map: &Omap) -> Vec<ObjectId> {
    first_part(map).objects().map(|(id, _)| id).collect()
}

fn mark(object: &mut MapObject, value: &str) {
    let _previous = object
        .tags_mut()
        .insert("edited".to_owned(), value.to_owned());
}

fn marked(map: &Omap, id: ObjectId) -> Option<&str> {
    first_part(map)
        .object(id)
        .expect("the object is in the part")
        .tags()
        .get("edited")
        .map(String::as_str)
}

#[test]
fn editing_a_copy_leaves_the_original_unchanged_and_back() {
    let mut original = from_path();
    let ids = object_ids(&original);
    let symbol = original.symbols.ids().next().expect("the map has symbols");
    let symbol_name = original
        .symbols
        .get(symbol)
        .expect("the symbol resolves")
        .name()
        .to_owned();
    let part_name = first_part(&original).name.clone();

    let mut copy = original.clone();
    mark(
        first_part_mut(&mut copy)
            .object_mut(ids[0])
            .expect("resolves"),
        "copy",
    );
    copy.symbols
        .get_mut(symbol)
        .expect("resolves")
        .set_name("renamed".to_owned());
    first_part_mut(&mut copy).name = "renamed".to_owned();

    assert_eq!(marked(&original, ids[0]), None);
    assert_eq!(
        original.symbols.get(symbol).expect("resolves").name(),
        symbol_name
    );
    assert_eq!(first_part(&original).name, part_name);

    mark(
        first_part_mut(&mut original)
            .object_mut(ids[1])
            .expect("resolves"),
        "original",
    );
    assert_eq!(marked(&copy, ids[0]), Some("copy"));
    assert_eq!(marked(&copy, ids[1]), None);
    assert_eq!(marked(&original, ids[1]), Some("original"));
}

#[test]
fn replacing_a_color_in_a_copy_leaves_the_original_unchanged() -> Result<()> {
    let original = Omap::from_bytes(ISOM_15000)?;
    let ids: Vec<_> = original.colors.ids().collect();
    let name = |map: &Omap, at: usize| map.colors.get(ids[at]).expect("resolves").name().to_owned();
    let first = name(&original, 0);

    let mut copy = original.clone();
    let other = Arc::clone(copy.colors.get_arc(ids[1]).expect("resolves"));
    let _replaced = copy.colors.replace(ids[0], other);

    assert_eq!(name(&original, 0), first);
    assert_eq!(name(&copy, 0), name(&copy, 1));
    Ok(())
}

#[test]
fn an_edit_copies_only_the_object_it_touches() {
    let original = from_path();
    let ids = object_ids(&original);
    let mut copy = original.clone();

    mark(
        first_part_mut(&mut copy)
            .object_mut(ids[3])
            .expect("resolves"),
        "copy",
    );

    for &id in &ids {
        let shared = Arc::ptr_eq(
            first_part(&original).object_arc(id).expect("resolves"),
            first_part(&copy).object_arc(id).expect("resolves"),
        );
        assert_eq!(shared, id != ids[3], "only the edited object is copied");
    }
}

#[test]
fn an_object_removed_from_a_copy_stays_in_the_original() {
    let original = from_path();
    let ids = object_ids(&original);
    let mut copy = original.clone();

    let removed = first_part_mut(&mut copy).remove_object(ids[2]);

    assert!(removed.is_some());
    assert!(!first_part(&copy).contains(ids[2]));
    assert!(first_part(&original).contains(ids[2]));
    assert_eq!(first_part(&original).index_of(ids[2]), Some(2));
    assert_eq!(first_part(&copy).len() + 1, first_part(&original).len());
}

#[test]
fn a_restored_object_comes_back_under_its_own_handle() {
    let mut map = from_path();
    let ids = object_ids(&map);
    let part = first_part_mut(&mut map);

    let (id, index, removed) = part.remove_object(ids[2]).expect("resolves");
    assert_eq!((id, index), (ids[2], 2));
    assert_eq!(part.index_of(ids[3]), Some(2));

    part.restore_object(ids[2], 0, removed)
        .expect("the handle is a tombstone");

    assert_eq!(part.index_of(ids[2]), Some(0));
    assert_eq!(part.index_of(ids[3]), Some(3));
    assert_eq!(part.objects().next().map(|(id, _)| id), Some(ids[2]));
}

#[test]
fn restoring_an_object_is_refused_for_a_live_or_foreign_handle() {
    let mut map = from_path();
    let ids = object_ids(&map);
    let part = first_part_mut(&mut map);
    let object = Arc::clone(part.object_arc(ids[0]).expect("resolves"));

    assert!(part.restore_object(ids[0], 0, Arc::clone(&object)).is_err());

    let mut other = MapPart::new("other");
    let foreign = other.add_object(Arc::unwrap_or_clone(Arc::clone(&object)));
    let mut small = MapPart::new("small");
    assert!(
        small.restore_object(foreign, 0, object).is_err(),
        "a handle this part never issued"
    );
    assert!(small.is_empty());
}

/// The objects the map, written and read back, draws with the symbol coded like `code`.
fn written_objects_with(map: &Omap, code: omap::Code) -> Result<usize> {
    let mut bytes = Vec::new();
    map.to_writer(&mut bytes)?;
    let read = Omap::from_bytes(&bytes)?;
    Ok(read
        .iter_all_objects()
        .filter_map(MapObject::symbol)
        .filter_map(|symbol| read.symbols.get(symbol))
        .filter(|symbol| symbol.common().code == code)
        .count())
}

#[test]
fn objects_of_a_restored_symbol_use_it_again() -> Result<()> {
    let mut map = from_path();
    let symbol = map
        .iter_all_objects()
        .find_map(MapObject::symbol)
        .expect("an object has a symbol");
    let code = map.symbols.get(symbol).expect("resolves").common().code;
    let index = map.symbols.index_of(symbol).expect("resolves");
    let before = written_objects_with(&map, code)?;
    assert!(before > 0);

    let (id, removed_index, removed) = map.symbols.remove(symbol).expect("resolves");
    assert_eq!((id, removed_index), (symbol, index));
    assert_eq!(written_objects_with(&map, code)?, 0);

    map.symbols
        .restore(id, removed_index, removed)
        .expect("the handle is a tombstone");

    assert!(map.symbols.get(symbol).is_some());
    assert_eq!(written_objects_with(&map, code)?, before);
    Ok(())
}

#[test]
fn a_part_handle_follows_its_part_through_removals_and_restores() {
    let mut parts = MapParts::new();
    let a = parts.push(MapPart::new("a"));
    let b = parts.push(MapPart::new("b"));
    let c = parts.push(MapPart::new("c"));

    let (_, index, removed) = parts.remove(a).expect("resolves");
    assert_eq!(index, 0);
    assert_eq!(parts.index_of(b), Some(0));
    assert_eq!(parts.get_by_id(c).map(|part| part.name.as_str()), Some("c"));
    assert!(parts.get_by_id(a).is_none());

    parts
        .restore(a, 2, removed)
        .expect("the handle is a tombstone");

    let names: Vec<_> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, ["b", "c", "a"]);
    assert_eq!(parts.index_of(a), Some(2));
    let in_order: Vec<_> = parts.iter_mut().map(|part| part.name.clone()).collect();
    assert_eq!(in_order, ["b", "c", "a"]);
}

#[test]
fn merging_parts_keeps_the_first_part_handle() {
    let mut parts = MapParts::new();
    let mut first = MapPart::new("first");
    let mut second = MapPart::new("second");
    let source = from_path();
    let object = first_part(&source)
        .iter_all_objects()
        .next()
        .cloned()
        .expect("from_path has an object");
    let _id = first.add_object(object.clone());
    let _id = second.add_object(object);
    let first = parts.push(first);
    let _second = parts.push(second);

    parts.merge_all_parts(None);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts.index_of(first), Some(0));
    assert_eq!(parts.get_by_id(first).map(MapPart::len), Some(2));
}

#[cfg(feature = "serde")]
#[test]
fn a_serde_round_trip_drops_tombstones_and_keeps_live_handles() {
    let mut map = from_path();
    let ids = object_ids(&map);
    let (_, _, removed) = first_part_mut(&mut map)
        .remove_object(ids[0])
        .expect("resolves");

    let json = serde_json::to_string(&map).expect("serialize");
    let mut read: Omap = serde_json::from_str(&json).expect("deserialize");

    for &id in &ids[1..] {
        assert!(first_part(&read).contains(id));
    }
    assert!(
        first_part_mut(&mut read)
            .restore_object(ids[0], 0, removed)
            .is_err(),
        "a tombstone is not serialized"
    );
}

#[test]
fn pruning_copies_only_what_held_a_dangling_reference() {
    let mut map = from_path();
    let objects: Vec<(ObjectId, Option<SymbolId>)> = first_part(&map)
        .objects()
        .map(|(id, object)| (id, object.symbol()))
        .collect();
    let gone = objects
        .iter()
        .find_map(|&(_, symbol)| symbol)
        .expect("an object has a symbol");
    let kept = map
        .symbols
        .ids()
        .find(|&id| {
            id != gone
                && !matches!(
                    &*map.symbols.get(id).expect("resolves"),
                    omap::symbols::Symbol::CombinedArea(_) | omap::symbols::Symbol::CombinedLine(_)
                )
        })
        .expect("another symbol that is not combined");
    let _removed = map.symbols.remove(gone).expect("resolves");
    let copy = map.clone();

    map.prune_dangling_references();
    map.validate().expect("nothing dangles after pruning");

    let shared = |map: &Omap, id| {
        Arc::ptr_eq(
            first_part(map).object_arc(id).expect("resolves"),
            first_part(&copy).object_arc(id).expect("resolves"),
        )
    };
    for &(id, symbol) in &objects {
        assert_eq!(shared(&map, id), symbol != Some(gone), "{symbol:?}");
    }
    assert!(Arc::ptr_eq(
        map.symbols.get_arc(kept).expect("resolves"),
        copy.symbols.get_arc(kept).expect("resolves"),
    ));

    let again = map.clone();
    map.prune_dangling_references();
    for &(id, _) in &objects {
        assert!(Arc::ptr_eq(
            first_part(&map).object_arc(id).expect("resolves"),
            first_part(&again).object_arc(id).expect("resolves"),
        ));
    }
}

#[test]
fn removing_a_symbols_objects_shares_them_and_restoring_puts_them_back_in_place() {
    let mut map = from_path();
    let symbol = map
        .iter_all_objects()
        .find_map(MapObject::symbol)
        .expect("an object has a symbol");
    let before: Vec<ObjectId> = object_ids(&map);
    let copy = map.clone();

    let removed = first_part_mut(&mut map).remove_by_symbol(symbol);

    assert!(!removed.is_empty());
    for (id, index, object) in &removed {
        assert_eq!(first_part(&copy).index_of(*id), Some(*index));
        assert!(Arc::ptr_eq(
            object,
            first_part(&copy).object_arc(*id).expect("in the copy")
        ));
        assert!(first_part(&map).object(*id).is_none());
    }
    for (id, index, object) in removed {
        first_part_mut(&mut map)
            .restore_object(id, index, object)
            .expect("a tombstone");
    }
    assert_eq!(object_ids(&map), before);
}

#[test]
fn a_removed_symbol_or_color_comes_back_as_the_very_value_the_set_shared() {
    let mut map = from_path();
    let symbol = map.symbols.ids().next().expect("a symbol");
    let shared = Arc::clone(map.symbols.get_arc(symbol).expect("resolves"));
    let (_, _, removed) = map.symbols.remove(symbol).expect("resolves");
    assert!(Arc::ptr_eq(&removed, &shared));

    let (color, shared) = {
        let id = map.colors.ids().next().expect("a color");
        (id, Arc::clone(map.colors.get_arc(id).expect("resolves")))
    };
    let (id, priority, removed) = map.colors.remove_at(0).expect("a color");
    assert_eq!(priority, 0);
    assert_eq!(id, color);
    assert!(Arc::ptr_eq(&removed, &shared));
    map.colors
        .restore(id, priority, removed)
        .expect("a tombstone");
    assert!(Arc::ptr_eq(
        map.colors.get_arc(color).expect("back"),
        &shared
    ));
}

/// A combined symbol whose parts are all public symbols, one of those parts, and a color only that part draws with.
fn combined_of_public_parts(map: &Omap) -> Option<(SymbolId, SymbolId, omap::colors::ColorId)> {
    use omap::symbols::{PublicOrPrivateSymbol, Symbol};
    map.symbols.ids().find_map(|id| {
        let symbol = map.symbols.get(id)?;
        let parts: Vec<SymbolId> = match &*symbol {
            Symbol::CombinedArea(c) => c
                .components()
                .map(|p| match p {
                    PublicOrPrivateSymbol::Public(id) => id.map(Into::into),
                    PublicOrPrivateSymbol::Private(_) => None,
                })
                .collect::<Option<_>>()?,
            Symbol::CombinedLine(c) => c
                .components()
                .map(|p| match p {
                    PublicOrPrivateSymbol::Public(id) => id.map(Into::into),
                    PublicOrPrivateSymbol::Private(_) => None,
                })
                .collect::<Option<_>>()?,
            _ => return None,
        };
        let part = *parts.first()?;
        let color = map.symbols.get(part)?.colors(&map.symbols).next()?;
        Some((id, part, color))
    })
}

#[test]
fn pruning_a_color_gone_from_a_public_part_leaves_the_combined_symbol_shared() {
    let mut map = Omap::from_bytes(ISOM_15000).expect("bundled map parses");
    let (combined, part, color) =
        combined_of_public_parts(&map).expect("a combined symbol of public parts");
    let _removed = map.colors.remove(color).expect("resolves");
    let copy = map.clone();

    map.prune_dangling_references();
    map.validate().expect("nothing dangles after pruning");
    assert!(
        !map.symbols
            .get(part)
            .expect("resolves")
            .colors(&map.symbols)
            .any(|used| used == color)
    );

    assert!(
        Arc::ptr_eq(
            map.symbols.get_arc(combined).expect("resolves"),
            copy.symbols.get_arc(combined).expect("resolves"),
        ),
        "the combined symbol names no color itself"
    );
    assert!(
        !Arc::ptr_eq(
            map.symbols.get_arc(part).expect("resolves"),
            copy.symbols.get_arc(part).expect("resolves"),
        ),
        "the part drew with the color"
    );
}

/// As in Mapper, a part whose symbol is gone stays in its place, empty, and
/// is written as `-1`.
#[test]
fn pruning_empties_a_removed_public_part_in_place() {
    use omap::symbols::{PublicOrPrivateSymbol, Symbol};

    let mut map = Omap::from_bytes(ISOM_15000).expect("bundled map parses");
    let (combined, part, _) =
        combined_of_public_parts(&map).expect("a combined symbol of public parts");
    // Whether each part names a symbol, in order.
    let parts = |map: &Omap, combined: SymbolId| -> Vec<bool> {
        match &*map.symbols.get(combined).expect("resolves") {
            Symbol::CombinedArea(c) => c
                .components()
                .map(|p| !matches!(p, PublicOrPrivateSymbol::Public(None)))
                .collect(),
            Symbol::CombinedLine(c) => c
                .components()
                .map(|p| !matches!(p, PublicOrPrivateSymbol::Public(None)))
                .collect(),
            _ => unreachable!("a combined symbol"),
        }
    };
    let before = parts(&map, combined);
    let _removed = map.symbols.remove(part).expect("resolves");
    assert_eq!(
        map.validate(),
        Err(omap::ValidationError::DanglingCombinedComponent {
            symbol_index: map.symbols.index_of(combined).expect("resolves"),
        })
    );
    let copy = map.clone();

    map.prune_dangling_references();

    map.validate().expect("nothing dangles after pruning");
    let after = parts(&map, combined);
    assert_eq!(after.len(), before.len(), "the part keeps its place");
    assert!(!after[0], "the first part was the removed one");
    assert!(!Arc::ptr_eq(
        map.symbols.get_arc(combined).expect("resolves"),
        copy.symbols.get_arc(combined).expect("resolves"),
    ));

    let mut bytes = Vec::new();
    map.to_writer(&mut bytes).expect("writes");
    let reread = Omap::from_bytes(&bytes).expect("reads back");
    let reread_combined = reread
        .symbols
        .find_at(map.symbols.index_of(combined).expect("resolves"))
        .expect("same place")
        .id();
    assert_eq!(
        parts(&reread, reread_combined),
        after,
        "an empty part survives a round trip"
    );
}
