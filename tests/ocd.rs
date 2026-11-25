//! Reading and writing the OCAD format.

#![cfg(feature = "ocd")]
use omap::{format, ocd};

/// A bare 60-byte OCAD file header of `version`, with no index blocks.
fn header(version: u16) -> Vec<u8> {
    let mut bytes = vec![0; 60];
    bytes[0..2].copy_from_slice(&0x0cad_u16.to_le_bytes());
    bytes[4..6].copy_from_slice(&version.to_le_bytes());
    bytes
}

#[test]
fn the_ocad_format_claims_ocd_in_any_case() {
    for path in ["map.ocd", "dir/MAP.OCD", "map.Ocd"] {
        let found = format::for_path(path).expect("a format claims it");
        assert_eq!(found.extensions(), ["ocd"], "{path}");
    }
}

#[test]
fn versions_12_and_2018_are_read() {
    for version in [12, 2018] {
        assert!(
            ocd::read(header(version).as_slice()).is_ok(),
            "version {version}"
        );
    }
}

#[test]
fn other_versions_are_rejected_by_number() {
    for version in [6, 8, 9, 10, 11, 13, 2019, 2020] {
        assert!(
            matches!(
                ocd::read(header(version).as_slice()),
                Err(ocd::Error::UnsupportedVersion(found)) if found == version
            ),
            "version {version}"
        );
    }
}

#[test]
fn course_setting_projects_and_server_files_are_not_maps() {
    for file_type in [1, 8] {
        let mut bytes = header(12);
        bytes[2] = file_type;
        assert!(
            matches!(
                ocd::read(bytes.as_slice()),
                Err(ocd::Error::NotAMap(found)) if found == file_type
            ),
            "file type {file_type}"
        );
    }
}

#[test]
fn a_file_without_the_ocad_mark_is_not_ocad() {
    let mut bytes = header(12);
    bytes[0] = 0;
    assert!(matches!(
        ocd::read(bytes.as_slice()),
        Err(ocd::Error::NotOcad)
    ));
    assert!(matches!(
        ocd::read(b"<?xml version=\"1.0\"?>".as_slice()),
        Err(ocd::Error::NotOcad)
    ));
}

#[test]
fn the_format_reads_what_it_wrote() {
    let format = format::for_path("map.ocd").expect("the OCAD format");
    let map = omap::Omap::new(std::num::NonZeroU32::new(10_000).expect("not zero"));
    let mut written = Vec::new();
    format.write(&map, &mut written).expect("the map writes");
    let again = format
        .read(&mut written.as_slice())
        .expect("the written map reads");
    assert_eq!(again.colors.len(), map.colors.len());
}

#[cfg(feature = "xml")]
const BUNDLED: [(&str, &[u8]); 4] = [
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

#[cfg(feature = "xml")]
#[test]
fn every_bundled_map_writes_and_reads_back_valid() {
    for (name, bytes) in BUNDLED {
        let map = omap::xml::read(bytes).expect("the bundled map reads");
        let mut written = Vec::new();
        ocd::write(&map, &mut written).expect("the map writes as OCAD");
        let again = ocd::read(written.as_slice()).expect("the written map reads");
        assert!(again.map.validate().is_ok(), "{name}");
        assert!(
            again.map.colors.len() >= map.colors.len(),
            "{name}: colors are kept"
        );
    }
}

/// `map` written as OCAD, then read back.
#[cfg(feature = "xml")]
fn through_ocad(map: &omap::Omap) -> omap::Omap {
    let mut written = Vec::new();
    ocd::write(map, &mut written).expect("the map writes as OCAD");
    ocd::read(written.as_slice())
        .expect("the written map reads")
        .map
}

#[cfg(feature = "xml")]
#[test]
fn a_map_through_ocad_once_comes_through_again_unchanged() {
    for (name, bytes) in BUNDLED {
        let map = omap::xml::read(bytes).expect("the bundled map reads");
        let once = through_ocad(&map);
        let twice = through_ocad(&once);
        assert!(
            format!("{twice:#?}") == format!("{once:#?}"),
            "{name}: the second pass through OCAD changed the map"
        );
    }
}
