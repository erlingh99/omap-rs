//! Choosing a file format by extension, and reading and writing through it.

#![cfg(feature = "xml")]
use omap::{Omap, format};

const ISOM_15000: &[u8] = include_bytes!("../src/default_maps/isom_15000.omap");

#[test]
fn the_xml_format_claims_omap_and_xmap_in_any_case() {
    for path in ["map.omap", "map.xmap", "dir/MAP.OMAP", "map.Xmap"] {
        let found = format::for_path(path).expect("a format claims it");
        assert!(found.extensions().contains(&"omap"), "{path}");
    }
}

#[test]
fn no_format_claims_an_unknown_or_missing_extension() {
    for path in ["map.dwg", "map.json", "map", ""] {
        assert!(format::for_path(path).is_none(), "{path}");
    }
}

#[test]
fn every_format_listed_is_one_for_path_finds() {
    assert!(!format::all().is_empty());
    for listed in format::all() {
        for extension in listed.extensions() {
            assert!(format::for_path(format!("map.{extension}")).is_some());
        }
    }
}

#[test]
fn an_unsupported_extension_is_reported_before_touching_the_disk() {
    let map = omap::xml::read(ISOM_15000).expect("the bundled map reads");
    let path = std::env::temp_dir().join("omap-rs-unsupported-format-test.dwg");

    assert!(matches!(
        map.to_file(&path),
        Err(format::Error::UnsupportedExtension(extension)) if extension == "dwg"
    ));
    assert!(!path.exists(), "nothing is written");
    assert!(matches!(
        Omap::from_path(&path),
        Err(format::Error::UnsupportedExtension(_))
    ));
}

#[test]
fn a_format_reads_what_it_wrote() {
    let format = format::for_path("map.omap").expect("the xml format");
    let map = format
        .read(&mut { ISOM_15000 })
        .expect("the bundled map reads");

    let mut written = Vec::new();
    format.write(&map, &mut written).expect("the map writes");
    let again = format
        .read(&mut written.as_slice())
        .expect("the written map reads");

    assert_eq!(again.symbols.len(), map.symbols.len());
    assert_eq!(again.colors.len(), map.colors.len());
}
