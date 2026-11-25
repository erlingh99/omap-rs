//! The bundled default maps, which are stored as `.omap` files.

#[cfg(feature = "geo_ref")]
use std::num::NonZeroU32;

#[cfg(feature = "geo_ref")]
use geo_types::Coord;

use super::{Result, read};
use crate::Omap;
#[cfg(feature = "geo_ref")]
use crate::geo_referencing::{CrsType, GeoRef};

const DEFAULT_ISOM_15000: &[u8] = include_bytes!("../default_maps/isom_15000.omap");
const DEFAULT_ISOM_10000: &[u8] = include_bytes!("../default_maps/isom_10000.omap");
const DEFAULT_ISSPROM_4000: &[u8] = include_bytes!("../default_maps/issprom_4000.omap");

/// The scale denominators the bundled default maps are drawn at.
/// Unwrap on consts is compile time
#[cfg(feature = "geo_ref")]
const SCALE_15_000: NonZeroU32 = NonZeroU32::new(15_000).unwrap();
#[cfg(feature = "geo_ref")]
const SCALE_10_000: NonZeroU32 = NonZeroU32::new(10_000).unwrap();
#[cfg(feature = "geo_ref")]
const SCALE_4_000: NonZeroU32 = NonZeroU32::new(4_000).unwrap();

impl Omap {
    /// Create a new georeferenced `1:15_000` map with a complete ISOM symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if georeferencing cannot be initialized or the
    /// embedded default map cannot be parsed.
    #[cfg(feature = "geo_ref")]
    pub fn default_15_000_geo_referenced(
        projected_ref_point: Coord,
        crs: CrsType,
        meters_above_sea: f64,
    ) -> Result<Self> {
        let geo_ref = GeoRef::initialize(projected_ref_point, crs, meters_above_sea, SCALE_15_000)?;
        let mut omap = read(DEFAULT_ISOM_15000)?;
        omap.geo_referencing = geo_ref;
        Ok(omap)
    }

    /// Create a new georeferenced `1:10_000` map with a complete ISOM symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if georeferencing cannot be initialized or the
    /// embedded default map cannot be parsed.
    #[cfg(feature = "geo_ref")]
    pub fn default_10_000_geo_referenced(
        projected_ref_point: Coord,
        crs: CrsType,
        meters_above_sea: f64,
    ) -> Result<Self> {
        let geo_ref = GeoRef::initialize(projected_ref_point, crs, meters_above_sea, SCALE_10_000)?;
        let mut omap = read(DEFAULT_ISOM_10000)?;
        omap.geo_referencing = geo_ref;
        Ok(omap)
    }

    /// Create a new georeferenced `1:4_000` map with a complete `ISSprOM` symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if georeferencing cannot be initialized or the
    /// embedded default map cannot be parsed.
    #[cfg(feature = "geo_ref")]
    pub fn default_4_000_geo_referenced(
        projected_ref_point: Coord,
        crs: CrsType,
        meters_above_sea: f64,
    ) -> Result<Self> {
        let geo_ref = GeoRef::initialize(projected_ref_point, crs, meters_above_sea, SCALE_4_000)?;
        let mut omap = read(DEFAULT_ISSPROM_4000)?;
        omap.geo_referencing = geo_ref;
        Ok(omap)
    }

    /// Create a new `1:15_000` map with a complete ISOM symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded default map cannot be parsed.
    pub fn default_15_000() -> Result<Self> {
        read(DEFAULT_ISOM_15000)
    }

    /// Create a new `1:10_000` map with a complete ISOM symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded default map cannot be parsed.
    pub fn default_10_000() -> Result<Self> {
        read(DEFAULT_ISOM_10000)
    }

    /// Create a new `1:4_000` map with a complete `ISSprOM` symbolset and color order
    ///
    /// # Errors
    ///
    /// Returns an error if the embedded default map cannot be parsed.
    pub fn default_4_000() -> Result<Self> {
        read(DEFAULT_ISSPROM_4000)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, num::NonZeroU32};

    /// What a test returns: any error fails it.
    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    use geo_types::{Coord, Point};

    use super::Omap;
    use crate::{Error, ValidationError, objects::PointObject};

    fn point_positions(map: &Omap) -> Vec<Coord> {
        map.iter_all_objects()
            .filter_map(|object| match object {
                crate::objects::MapObject::Point(point) => Some(point.geometry().0),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn try_transform_is_transactional() -> TestResult {
        let mut map = Omap::new(NonZeroU32::new(10_000).expect("the scale is nonzero"));
        let part = map.parts.at_mut(0).ok_or(Error::ObjectError)?;
        part.objects
            .push(PointObject::new(None, Point::new(1.0, 2.0)));
        part.objects
            .push(PointObject::new(None, Point::new(10.0, 20.0)));
        let before = point_positions(&map);

        let result = map.try_transform(|coord| {
            if coord.x > 5.0 {
                Err(Error::ObjectError)
            } else {
                Ok(Coord {
                    x: coord.x + 100.0,
                    y: coord.y,
                })
            }
        });

        assert!(matches!(result, Err(Error::ObjectError)));
        assert_eq!(point_positions(&map), before);
        Ok(())
    }

    #[test]
    fn validate_reports_the_dangling_object_location() -> TestResult {
        let mut map = Omap::new(NonZeroU32::new(10_000).ok_or(Error::ObjectError)?);
        let point = map
            .symbols
            .add_point_symbol(crate::symbols::PointSymbol::new(
                crate::Code::new(1, 0, 0),
                "dot",
            ));

        let part = map.parts.at_mut(0).ok_or(Error::ObjectError)?;
        part.objects
            .push(PointObject::new(Some(point), Point::new(1.0, 2.0)));
        assert!(map.validate().is_ok(), "a live handle must validate");

        let _removed = map.symbols.remove(point);
        assert!(
            matches!(
                map.validate(),
                Err(ValidationError::DanglingObjectSymbol { object_index: 0 })
            ),
            "a handle to a removed symbol must not validate"
        );
        Ok(())
    }

    #[test]
    fn an_object_without_a_symbol_validates() -> TestResult {
        let mut map = Omap::new(NonZeroU32::new(10_000).ok_or(Error::ObjectError)?);
        let part = map.parts.at_mut(0).ok_or(Error::ObjectError)?;
        part.objects
            .push(PointObject::new(None, Point::new(1.0, 2.0)));

        assert!(map.validate().is_ok());
        Ok(())
    }

    #[test]
    fn to_file_preserves_existing_file_when_serialization_fails() -> TestResult {
        let path = std::env::temp_dir().join(format!(
            "omap-rs-atomic-write-test-{}.omap",
            std::process::id(),
        ));
        fs::write(&path, b"previous map")?;

        let mut map = Omap::new(NonZeroU32::new(10_000).ok_or(Error::ObjectError)?);
        let part = map.parts.at_mut(0).ok_or(Error::ObjectError)?;
        part.objects
            .push(PointObject::new(None, Point::new(3_000_000.0, 0.0)));

        let result = map.to_file(&path);
        assert!(matches!(
            result,
            Err(crate::format::Error::Xml(
                crate::xml::Error::MapCoordOutOfBounds
            ))
        ));
        assert_eq!(fs::read(&path)?, b"previous map");
        fs::remove_file(path)?;
        Ok(())
    }

    /// A dangling reference must prune to `None`, never to whatever value later
    /// takes the removed symbol's place.
    #[test]
    fn pruning_drops_references_to_removed_symbols() -> TestResult {
        let mut map = crate::xml::read(super::DEFAULT_ISOM_15000)?;
        let point = map
            .symbols
            .iter_point_symbols()
            .next()
            .map(|(id, _)| id)
            .expect("the default map contains a point symbol");

        let part = map
            .parts
            .at_mut(0)
            .expect("the default map has a first part");
        part.objects
            .push(PointObject::new(Some(point), Point::new(1.0, 2.0)));

        let _removed = map.symbols.remove(point);
        assert!(map.validate().is_err(), "the object now dangles");

        map.prune_dangling_references();

        let object = map
            .iter_all_objects()
            .last()
            .expect("the added point object remains after pruning");
        assert_eq!(
            object.symbol(),
            None,
            "a dangling reference must prune to None"
        );
        assert!(map.validate().is_ok());
        Ok(())
    }

    /// A map whose objects, symbols and colors dangle: a point object drawn
    /// with a removed symbol, and a removed color some symbol drew with.
    fn dangling_map() -> TestResult<Omap> {
        let mut map = crate::xml::read(super::DEFAULT_ISOM_15000)?;
        let (point, _) = map
            .symbols
            .iter_point_symbols()
            .next()
            .expect("the default map contains a point symbol");
        let _id = map
            .parts
            .at_mut(0)
            .expect("the default map has a first part")
            .objects
            .push(PointObject::new(Some(point), Point::new(1.0, 2.0)));
        let _removed = map.symbols.remove(point);
        let color = map
            .colors
            .at(0)
            .expect("the default map has a first color")
            .id();
        let _removed = map.colors.remove(color);
        assert!(map.validate().is_err(), "the map dangles");
        Ok(map)
    }

    #[test]
    fn writing_a_dangling_map_writes_it_as_pruned() -> TestResult {
        let map = dangling_map()?;
        let mut pruned = map.clone();
        pruned.prune_dangling_references();

        let mut written = Vec::new();
        crate::xml::write(&map, &mut written)?;
        let mut expected = Vec::new();
        crate::xml::write(&pruned, &mut expected)?;

        assert_eq!(written, expected);
        Ok(())
    }

    #[test]
    fn writing_leaves_the_map_as_it_was() -> TestResult {
        let map = dangling_map()?;
        let before = map.validate();

        crate::xml::write(&map, &mut Vec::new())?;

        assert_eq!(map.validate(), before, "the map still dangles");
        assert!(
            map.iter_all_objects()
                .last()
                .is_some_and(|object| object.symbol().is_some()),
            "the object keeps its handle"
        );
        Ok(())
    }
}
