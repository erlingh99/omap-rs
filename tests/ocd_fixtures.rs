//! Reading a real OCAD file: `example_data/from_path.ocd`, exported from
//! `example_data/from_path.omap`.
//!
//! The file reads into a valid map, reads back as the same map after
//! writing, and holds what the map it was exported from holds.

#![cfg(feature = "ocd")]
#![expect(
    clippy::expect_used,
    reason = "a test should fail loudly at the point it breaks"
)]

use omap::geo_referencing::CrsType;
use omap::objects::{HorizontalAlign, MapObject, VerticalAlign};
use omap::ocd;

const FROM_PATH_OCD: &[u8] = include_bytes!("../example_data/from_path.ocd");

fn reading() -> ocd::Reading {
    ocd::read(FROM_PATH_OCD).expect("the example reads")
}

#[test]
fn the_example_reads_without_warnings_into_a_valid_map() {
    let reading = reading();
    assert!(reading.warnings.is_empty(), "{:?}", reading.warnings);
    assert!(
        reading.map.validate().is_ok(),
        "{:?}",
        reading.map.validate()
    );
    assert_eq!(reading.map.colors.len(), 39);
    assert_eq!(reading.map.symbols.len(), 171);
    assert_eq!(reading.map.iter_all_objects().count(), 12);
    assert_eq!(reading.map.templates.len(), 1);
}

#[test]
fn the_example_reads_back_as_the_same_map_after_writing() {
    let map = reading().map;
    let mut written = Vec::new();
    let warnings = ocd::write(&map, &mut written).expect("the map writes");
    assert!(warnings.is_empty(), "{warnings:?}");
    let again = ocd::read(written.as_slice())
        .expect("the written map reads")
        .map;
    assert!(
        format!("{again:#?}") == format!("{map:#?}"),
        "the map read back differs from the map written"
    );
}

#[test]
fn the_example_keeps_its_scale_system_and_grivation() {
    let georef = reading().map.geo_referencing;
    assert_eq!(georef.scale_denominator.get(), 15_000);
    assert_eq!(georef.crs_type, CrsType::Epsg(25832));
    assert!(
        (georef.grivation_deg() - 4.57).abs() < 1e-9,
        "{}",
        georef.grivation_deg()
    );
}

#[test]
fn text_objects_keep_their_text_and_their_symbols_alignment() {
    let map = reading().map;
    let texts: Vec<_> = map
        .iter_all_objects()
        .filter_map(|object| match object {
            MapObject::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>(),
        ["some <text>", "Some more text"]
    );
    for text in texts {
        assert_eq!(text.h_align, HorizontalAlign::HCenter, "{}", text.text);
        assert_eq!(text.v_align, VerticalAlign::Baseline, "{}", text.text);
    }
}

#[cfg(feature = "xml")]
mod against_the_omap {
    //! The OCAD file holds what `from_path.omap` holds, as far as OCAD can.

    use std::cell::RefCell;
    use std::f64::consts::TAU;

    use omap::Omap;

    use super::{MapObject, reading};

    /// The code and name of the symbol `object` uses.
    fn symbol_of(map: &Omap, object: &MapObject) -> Option<(String, String)> {
        let symbol = map.symbols.get(object.symbol()?)?;
        let symbol = symbol.value();
        Some((symbol.code().to_string(), symbol.name().to_owned()))
    }

    fn exported_from() -> Omap {
        omap::xml::read(include_bytes!("../example_data/from_path.omap").as_slice())
            .expect("the omap reads")
    }

    /// Every coordinate of `object`'s geometry, control points included, in
    /// order. An area's pattern origin is left out: OCAD does not store it.
    fn coords(object: &MapObject) -> Vec<geo_types::Coord> {
        let mut object = object.clone();
        if let MapObject::Area(area) = &mut object {
            area.pattern_rotation.coord = geo_types::Coord::zero();
        }
        let seen = RefCell::new(Vec::new());
        object.transform(|coord| {
            seen.borrow_mut().push(coord);
            coord
        });
        seen.into_inner()
    }

    /// `angle` in `[0, TAU)`.
    fn normalized(angle: f64) -> f64 {
        angle.rem_euclid(TAU)
    }

    #[test]
    fn the_notes_are_the_same() {
        assert_eq!(reading().map.notes, exported_from().notes);
    }

    #[test]
    fn the_colors_are_the_same_in_the_same_order() {
        let names = |map: &Omap| -> Vec<String> {
            map.colors
                .values()
                // The export names a spot color by its separation alone.
                .map(|color| color.name().trim_start_matches("DEFINITION ").to_owned())
                .collect()
        };
        assert_eq!(names(&reading().map), names(&exported_from()));
    }

    #[test]
    fn every_symbol_is_there_and_only_broken_down_parts_are_added() {
        let codes = |map: &Omap| -> Vec<String> {
            map.symbols
                .values()
                .map(|symbol| symbol.code().to_string())
                .collect()
        };
        let (ocd, omap) = (codes(&reading().map), codes(&exported_from()));
        for code in &omap {
            assert!(ocd.contains(code), "symbol {code} is missing");
        }
        let added: Vec<_> = ocd.iter().filter(|code| !omap.contains(code)).collect();
        assert_eq!(added, ["501.3", "502.3"]);
    }

    #[test]
    fn the_objects_are_the_same_in_the_same_order() {
        let ocd = reading().map;
        let omap = exported_from();
        let ocd_objects: Vec<_> = ocd.iter_all_objects().collect();
        let omap_objects: Vec<_> = omap.iter_all_objects().collect();
        assert_eq!(ocd_objects.len(), omap_objects.len());

        for (index, (read, original)) in ocd_objects.into_iter().zip(omap_objects).enumerate() {
            let symbol = symbol_of(&ocd, read);
            assert_eq!(symbol, symbol_of(&omap, original), "object {index}");
            let context = format!("object {index}, {symbol:?}");
            match (read, original) {
                (MapObject::Point(read), MapObject::Point(original)) => {
                    let difference = normalized(read.rotation - original.rotation);
                    assert!(
                        difference.min(TAU - difference) < 1e-3,
                        "{context}: rotation {} against {}",
                        read.rotation,
                        original.rotation
                    );
                }
                (MapObject::Text(read), MapObject::Text(original)) => {
                    assert_eq!(read.text, original.text, "{context}");
                    // OCAD anchors text differently, so only the text is
                    // compared, not where it is.
                    continue;
                }
                (MapObject::Line(_), MapObject::Line(_))
                | (MapObject::Area(_), MapObject::Area(_)) => {}
                _ => panic!("{context}: the object changed kind"),
            }

            let (read, original) = (coords(read), coords(original));
            assert_eq!(read.len(), original.len(), "{context}");
            for (a, b) in read.iter().zip(&original) {
                assert!(
                    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01,
                    "{context}: {a:?} against {b:?}"
                );
            }
        }
    }
}
