use geo_types::Coord;
#[cfg(feature = "geo_ref")]
use proj_core::Transform;

#[cfg(feature = "geo_ref")]
use crate::Error;
use crate::{Result, geo_referencing::CrsType};

use super::{GeoRef, Transformable};

/// Coordinate transform between map and projected (CRS) coordinates.
#[cfg_attr(
    feature = "geo_ref",
    doc = "",
    doc = "With the `geo_ref` feature the same transform also reaches WGS84,",
    doc = "`x` longitude and `y` latitude in degrees, through the",
    doc = "[`to_wgs84`](Self::to_wgs84) and [`from_wgs84`](Self::from_wgs84),",
    doc = "which chain the paper to projected step with a projection",
    doc = "between the map's CRS and WGS84."
)]
#[derive(Debug, Clone)]
pub struct MapTransform {
    map_center: Coord,
    proj_center: Coord,
    scale_factor: f64,
    sin: f64,
    cos: f64,
    /// The CRS definition used by this transform.
    crs_type: CrsType,
    /// The WGS84 transform pair
    #[cfg(feature = "geo_ref")]
    wgs84_transform: Option<Wgs84Transforms>,
}

/// The compiled projections between the map's CRS and WGS84.
#[cfg(feature = "geo_ref")]
#[derive(Debug, Clone)]
struct Wgs84Transforms {
    to_wgs84: Transform,
    from_wgs84: Transform,
}

impl MapTransform {
    /// Convert a geometry in projected (CRS) coordinates to map coordinates.
    pub fn to_map<G: Transformable>(&self, projected: G) -> G {
        projected.transform(|coord| self.coord_to_map(coord))
    }

    /// Convert a geometry in map coordinates to projected (CRS) coordinates.
    pub fn to_projected<G: Transformable>(&self, map: G) -> G {
        map.transform(|coord| self.coord_to_projected(coord))
    }

    fn coord_to_map(&self, proj_coord: Coord) -> Coord {
        let (x, mut y) = ((proj_coord - self.proj_center) / self.scale_factor).x_y();

        let x_r = x * self.cos - y * self.sin;
        y = x * self.sin + y * self.cos;

        Coord { x: x_r, y } + self.map_center
    }

    fn coord_to_projected(&self, map_coord: Coord) -> Coord {
        let (x, mut y) = ((map_coord - self.map_center) * self.scale_factor).x_y();

        // we want to rotate other way so flip the signs of the sins
        let x_r = x * self.cos + y * self.sin;
        y = -x * self.sin + y * self.cos;

        Coord { x: x_r, y } + self.proj_center
    }

    /// The ground distance, in the projection's units, spanned by one millimeter of map
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Like [`Self::from_geo_ref`], but reports why the CRS could not be
    /// related to WGS84 rather than storing the absence and failing later.
    #[cfg(feature = "geo_ref")]
    pub(super) fn try_from_geo_ref(geo_ref: &GeoRef) -> Result<Self> {
        Ok(Self {
            map_center: geo_ref.map_ref_point,
            proj_center: geo_ref.projected_ref_point,
            sin: geo_ref.grivation_deg().to_radians().sin(),
            cos: geo_ref.grivation_deg().to_radians().cos(),
            scale_factor: geo_ref.map_to_ground_scale(),
            crs_type: geo_ref.crs_type.clone(),
            wgs84_transform: Some(Self::wgs84_transforms(&geo_ref.crs_type)?),
        })
    }

    pub(super) fn from_geo_ref(geo_ref: &GeoRef) -> Self {
        Self {
            map_center: geo_ref.map_ref_point,
            proj_center: geo_ref.projected_ref_point,
            sin: geo_ref.grivation_deg().to_radians().sin(),
            cos: geo_ref.grivation_deg().to_radians().cos(),
            scale_factor: geo_ref.map_to_ground_scale(),
            crs_type: geo_ref.crs_type.clone(),
            #[cfg(feature = "geo_ref")]
            wgs84_transform: Self::wgs84_transforms(&geo_ref.crs_type).ok(),
        }
    }

    /// Compute a transform from the `old` map coordinate frame to the `new`
    /// one, preserving real-world positions.
    ///
    /// Use this when changing the map's georeferencing and you need to
    /// transform all existing map objects and non-georeferenced templates so
    /// they remain at the same real-world locations.
    ///
    /// Without the `geo_ref` feature, the coordinate reference systems must be
    /// identical. With `geo_ref`, differing coordinate reference systems are
    /// converted using `proj-core`.
    ///
    /// # Errors
    ///
    /// Returns an error if the coordinate reference systems cannot be related.
    pub fn transform_between(
        old: &Self,
        new: &Self,
    ) -> Result<Box<dyn Fn(Coord) -> Result<Coord>>> {
        if old.crs_type == new.crs_type {
            let old = old.clone();
            let new = new.clone();
            return Ok(Box::new(move |coord| {
                Ok(new.to_map(old.to_projected(coord)))
            }));
        }

        #[cfg(feature = "geo_ref")]
        {
            let projection = Transform::from_horizontal_components(
                &old.crs_type.to_crs_def()?,
                &new.crs_type.to_crs_def()?,
            )?;
            let old = old.clone();
            let new = new.clone();

            Ok(Box::new(move |coord| {
                let projected = projection.convert(old.to_projected(coord))?;
                Ok(new.to_map(projected))
            }))
        }

        #[cfg(not(feature = "geo_ref"))]
        Err(crate::Error::CannotGetTransformBetweenDifferentGeoRef)
    }
}

#[cfg(feature = "geo_ref")]
impl MapTransform {
    /// Build the projections between the map's CRS and WGS84
    fn wgs84_transforms(crs_type: &CrsType) -> Result<Wgs84Transforms> {
        let geographic_crs = proj_wkt::parse_crs("EPSG:4326")?;
        let to_wgs84 =
            Transform::from_horizontal_components(&crs_type.to_crs_def()?, &geographic_crs)?;
        let from_wgs84 = to_wgs84.inverse()?;

        Ok(Wgs84Transforms {
            to_wgs84,
            from_wgs84,
        })
    }

    fn wgs84(&self) -> Result<&Wgs84Transforms> {
        self.wgs84_transform
            .as_ref()
            .ok_or(Error::NoWGS84TransformAvailable)
    }

    /// Convert a geometry in map coordinates to WGS84 degrees, `x` longitude
    /// and `y` latitude.
    ///
    /// # Errors
    ///
    /// Returns an error if the map's CRS cannot be related to WGS84, or if a
    /// coordinate falls outside the transform's domain.
    pub fn to_wgs84<G: Transformable>(&self, map: G) -> Result<G> {
        let wgs84 = self.wgs84()?;
        map.try_transform(|coord| Ok(wgs84.to_wgs84.convert(self.coord_to_projected(coord))?))
    }

    /// Convert a geometry in WGS84 degrees, `x` longitude and `y` latitude, to
    /// map coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error if the map's CRS cannot be related to WGS84, or if a
    /// coordinate falls outside the transform's domain.
    pub fn from_wgs84<G: Transformable>(&self, wgs84: G) -> Result<G> {
        let transforms = self.wgs84()?;
        wgs84.try_transform(|coord| Ok(self.coord_to_map(transforms.from_wgs84.convert(coord)?)))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use geo_types::{Coord, LineString, Point, Polygon, coord};

    use super::MapTransform;
    use crate::{
        Transformable,
        geo_referencing::{CrsType, GeoRef, UtmCode},
        objects::{BezierPath, BezierPolygon},
    };

    fn map_transform(crs_type: CrsType, projected_ref_point: Coord) -> MapTransform {
        GeoRef {
            scale_denominator: NonZeroU32::new(15_000).expect("the scale is nonzero"),
            grid_scale_factor: Default::default(),
            auxiliary_scale_factor: Default::default(),
            declination_deg: 2.,
            convergence_deg: -1.,
            crs_type,
            map_ref_point: coord! { x: 25., y: -50. },
            projected_ref_point,
            geographic_ref_point_deg: Coord::zero(),
        }
        .create_transform()
    }

    #[test]
    fn transform_between_preserves_coordinates_in_the_same_projection() -> crate::Result<()> {
        let old = map_transform(
            CrsType::Utm(UtmCode::new(33).expect("33 is a valid UTM zone")),
            coord! { x: 500_000., y: 6_600_000. },
        );
        let mut new_geo_ref = GeoRef {
            scale_denominator: NonZeroU32::new(10_000).expect("the scale is nonzero"),
            grid_scale_factor: 0.9998
                .try_into()
                .expect("the grid scale factor is positive"),
            auxiliary_scale_factor: 1.0001
                .try_into()
                .expect("the auxiliary scale factor is positive"),
            declination_deg: -3.,
            convergence_deg: 1.,
            crs_type: CrsType::Utm(UtmCode::new(33).expect("33 is a valid UTM zone")),
            map_ref_point: coord! { x: -100., y: 75. },
            projected_ref_point: coord! { x: 500_250., y: 6_599_800. },
            geographic_ref_point_deg: Coord::zero(),
        };
        let new = new_geo_ref.create_transform();
        let input = coord! { x: 12.5, y: -8.25 };

        let transform = MapTransform::transform_between(&old, &new)?;
        let transformed = transform(input)?;
        let expected = old.to_projected(input);
        let actual = new.to_projected(transformed);

        assert!((expected.x - actual.x).abs() < 1e-8);
        assert!((expected.y - actual.y).abs() < 1e-8);

        // The returned transform owns everything it needs.
        new_geo_ref.projected_ref_point = Coord::zero();
        assert_eq!(transform(input)?, transformed);
        Ok(())
    }

    #[cfg(not(feature = "geo_ref"))]
    #[test]
    fn transform_between_rejects_different_projections_without_geo_ref() {
        let old = map_transform(
            CrsType::Utm(UtmCode::new(33).expect("33 is a valid UTM zone")),
            coord! { x: 500_000., y: 6_600_000. },
        );
        let new = map_transform(
            CrsType::Utm(UtmCode::new(32).expect("32 is a valid UTM zone")),
            coord! { x: 500_000., y: 6_600_000. },
        );

        assert!(matches!(
            MapTransform::transform_between(&old, &new),
            Err(crate::Error::CannotGetTransformBetweenDifferentGeoRef)
        ));
    }

    #[cfg(feature = "geo_ref")]
    #[test]
    fn transform_between_converts_different_projections_with_geo_ref() -> crate::Result<()> {
        let old = map_transform(
            CrsType::Utm(UtmCode::new(33).expect("33 is a valid UTM zone")),
            coord! { x: 500_000., y: 6_600_000. },
        );
        let new = map_transform(
            CrsType::Utm(UtmCode::new(32).expect("32 is a valid UTM zone")),
            coord! { x: 500_000., y: 6_600_000. },
        );
        let input = coord! { x: 12.5, y: -8.25 };

        let transform = MapTransform::transform_between(&old, &new)?;
        let transformed = transform(input)?;
        let expected = old.to_wgs84(input)?;
        let actual = new.to_wgs84(transformed)?;

        assert!((expected.x - actual.x).abs() < 1e-9);
        assert!((expected.y - actual.y).abs() < 1e-9);
        Ok(())
    }

    /// Every coordinate of `geometry`, in the order it transforms them.
    fn coords<G: Transformable>(geometry: G) -> Vec<Coord> {
        let seen = std::cell::RefCell::new(Vec::new());
        let _same = geometry.transform(|coord| {
            seen.borrow_mut().push(coord);
            coord
        });
        seen.into_inner()
    }

    fn assert_close(expected: &[Coord], actual: &[Coord], tolerance: f64) {
        assert_eq!(expected.len(), actual.len(), "same number of coordinates");
        for (expected, actual) in expected.iter().zip(actual) {
            assert!(
                (expected.x - actual.x).abs() < tolerance
                    && (expected.y - actual.y).abs() < tolerance,
                "{expected:?} != {actual:?}"
            );
        }
    }

    fn rotated() -> MapTransform {
        map_transform(
            CrsType::Utm(UtmCode::new(33).unwrap()),
            coord! { x: 500_000., y: 6_600_000. },
        )
    }

    fn ring(offset: f64, size: f64) -> LineString {
        LineString::from(vec![
            (offset, offset),
            (offset + size, offset),
            (offset + size, offset + size),
            (offset, offset),
        ])
    }

    /// Calls `$check` with every kind of geometry [`Transformable`] is
    /// implemented for.
    macro_rules! for_every_geometry {
        ($check:ident) => {{
            let line = LineString::from(vec![(0., 0.), (10., 5.), (20., -3.)]);
            let polygon = Polygon::new(ring(0., 30.), vec![ring(5., 3.)]);
            let path = BezierPath::from(line.clone());
            let area = BezierPolygon::from(polygon.clone());

            $check(coord! { x: 12.5, y: -8.25 });
            $check(Point::new(1., 2.));
            $check(line.clone());
            $check(polygon.clone());
            $check(path.geometry().clone());
            $check(path);
            $check(area);
        }};
    }

    #[test]
    fn every_geometry_round_trips_between_map_and_projected() {
        fn round_trip<G: Transformable + Clone>(geometry: G) {
            let transform = rotated();
            let back = transform.to_map(transform.to_projected(geometry.clone()));
            assert_close(&coords(geometry), &coords(back), 1e-9);
        }
        for_every_geometry!(round_trip);
    }

    #[test]
    fn a_polygon_converts_as_its_coordinates_do() {
        let transform = rotated();
        let polygon = Polygon::new(ring(0., 30.), vec![ring(5., 3.)]);

        let converted = coords(transform.to_projected(polygon.clone()));
        let one_by_one: Vec<Coord> = coords(polygon)
            .into_iter()
            .map(|coord| transform.to_projected(coord))
            .collect();

        assert_eq!(converted, one_by_one);
    }

    #[cfg(feature = "geo_ref")]
    #[test]
    fn every_geometry_round_trips_through_wgs84() {
        fn round_trip<G: Transformable + Clone>(geometry: G) {
            let transform = rotated();
            let wgs84 = transform.to_wgs84(geometry.clone()).unwrap();
            let back = transform.from_wgs84(wgs84).unwrap();
            assert_close(&coords(geometry), &coords(back), 1e-6);
        }
        for_every_geometry!(round_trip);
    }
}
