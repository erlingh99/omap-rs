use std::num::NonZeroU32;

use geo_types::Coord;
#[cfg(feature = "geo_ref")]
use proj_core::{CrsDef, LinearUnit, ProjectedCrsDef, ProjectionMethod, Transform};

use super::CrsType;
#[cfg(feature = "geo_ref")]
use crate::{Error, Result};
use crate::{PositiveF64, geo_referencing::MapTransform};

/// The georeferencing information of the map. We assume the projected units are meters
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeoRef {
    /// Map scale
    /// Remember to scale all map coordinates after changing this
    pub scale_denominator: NonZeroU32,
    /// Grid scale factor
    /// Remember to scale all map coordinates after changing this
    pub grid_scale_factor: PositiveF64,
    /// Scale factor due too elevation
    /// Remember to scale all map coordinates after changing this
    pub auxiliary_scale_factor: PositiveF64,
    /// Angle between geographic north and magnetic north at the projected reference point
    /// Remember to rotate all map coordinates around the map center after changing this
    pub declination_deg: f64,
    /// Angle between projected north and geographic north at the projected reference point
    /// Remember to rotate all map coordinates around the map center after changing this
    pub convergence_deg: f64,
    /// The coordinate reference system definition
    /// Changing this might invalidate the ref points, scale factors and declination/convergence
    pub crs_type: CrsType,
    /// in millimeters on map
    /// Remember to translate all map coordinates after changing this
    pub map_ref_point: Coord,
    /// in whatever units the projection is in (should be meters)
    /// Changing this might invalidate the scale factors, declination/grivation and geographic reference point
    pub projected_ref_point: Coord,
    /// in WGS84 degrees
    /// Should be the inverse projection of the projected ref point into lat lon (ignored for local crs type)
    pub geographic_ref_point_deg: Coord,
}

impl GeoRef {
    /// The transform is used to go from map coordinates to projected coordinates or back
    #[cfg_attr(
        feature = "geo_ref",
        doc = "",
        doc = "The returned transform compiles its WGS84 operation once and reuses it,",
        doc = "so keep it around instead of asking for a new one per object."
    )]
    pub fn create_transform(&self) -> MapTransform {
        MapTransform::from_geo_ref(self)
    }

    /// Like [`Self::create_transform`], but fails up front if the CRS cannot be
    /// related to WGS84 instead of deferring that error to every call to a WGS84 conversion.
    ///
    /// # Errors
    ///
    /// Returns an error of a transform between the map and WGS84 cannot be made
    #[cfg(feature = "geo_ref")]
    pub fn try_get_transform(&self) -> Result<MapTransform> {
        MapTransform::try_from_geo_ref(self)
    }

    /// Create a new local georeferencing with the given map scale.
    pub fn new(scale: NonZeroU32) -> Self {
        Self {
            scale_denominator: scale,
            grid_scale_factor: PositiveF64::default(),
            auxiliary_scale_factor: PositiveF64::default(),
            declination_deg: 0.,
            convergence_deg: 0.,
            crs_type: CrsType::Local,
            map_ref_point: Coord::zero(),
            projected_ref_point: Coord::zero(),
            geographic_ref_point_deg: Coord::zero(),
        }
    }

    /// Get the angle between projected north and magnetic north (map north)
    /// grivation = declination - convergence
    pub fn grivation_deg(&self) -> f64 {
        self.declination_deg - self.convergence_deg
    }

    /// Get the combined grid and auxiliary scale factor.
    pub fn combined_scale_factor(&self) -> f64 {
        self.auxiliary_scale_factor.get() * self.grid_scale_factor.get()
    }

    /// Get the ground distance, in the projection's units, spanned by one millimetre of map
    pub fn map_to_ground_scale(&self) -> f64 {
        self.combined_scale_factor() * self.scale_denominator.get() as f64 / 1000.
    }

    /// Get the PROJ.4 projection string for this CRS, if available.
    pub fn proj_string(&self) -> Option<String> {
        self.crs_type.proj_string()
    }

    /// Get the EPSG code for this CRS, if available.
    pub fn epsg_code(&self) -> Option<u16> {
        self.crs_type.epsg_code()
    }
}

#[cfg(feature = "geo_ref")]
impl GeoRef {
    /// Initialise full georeferencing from a projected reference point, CRS, elevation and scale.
    ///
    /// Computes declination, convergence and scale factors automatically.
    /// Requires the `geo_ref` feature.
    ///
    /// # Errors
    ///
    /// Returns an error if the CRS cannot be parsed or transformed, the
    /// reference point is invalid, or magnetic-model data cannot be computed.
    pub fn initialize(
        projected_ref_point: Coord,
        crs: CrsType,
        meters_above_sea: f64,
        scale: NonZeroU32,
    ) -> Result<Self> {
        if matches!(crs, CrsType::Local) {
            let mut gr = Self::new(scale);
            gr.projected_ref_point = projected_ref_point;
            return Ok(gr);
        }

        let local_crs = crs.to_crs_def()?;
        let geographic_crs = proj_wkt::parse_crs("EPSG:4326")?;

        let transform = Transform::from_horizontal_components(&local_crs, &geographic_crs)?;

        let geographic_ref_point_deg = transform.convert(projected_ref_point)?;

        let declination_deg = Self::get_declination(geographic_ref_point_deg, meters_above_sea)?;
        let auxiliary_scale_factor =
            Self::get_elevation_scale_factor(geographic_ref_point_deg, meters_above_sea);

        let (convergence_deg, grid_scale_factor) =
            Self::get_convergence_and_grid_scale_factor(&local_crs, geographic_ref_point_deg)?;

        Ok(Self {
            scale_denominator: scale,
            grid_scale_factor,
            auxiliary_scale_factor: auxiliary_scale_factor.try_into()?,
            declination_deg,
            convergence_deg,
            crs_type: crs,
            map_ref_point: Coord::zero(),
            projected_ref_point,
            geographic_ref_point_deg,
        })
    }

    #[cfg(feature = "geo_ref")]
    fn get_convergence_and_grid_scale_factor(
        local_proj: &CrsDef,
        geo_ref_point_deg: Coord,
    ) -> Result<(f64, PositiveF64)> {
        let baseline_proj = CrsDef::Projected(ProjectedCrsDef::new_with_base_geographic_crs(
            0,
            4326,
            proj_core::datum::WGS84,
            ProjectionMethod::ObliqueStereographic {
                lon0: geo_ref_point_deg.x,
                lat0: geo_ref_point_deg.y,
                k0: 1.0,
                false_easting: 0.0,
                false_northing: 0.0,
            },
            LinearUnit::metre(),
            "WGS 84 convergence baseline",
        ));

        // The projected CRS is anonymous, but its registered WGS84 base CRS
        // lets proj-core select an operation directly to the local CRS.
        let transform = Transform::from_horizontal_components(&baseline_proj, local_proj)?;

        const D: f64 = 1000.0;
        let meridian =
            geo_types::Line::new(Coord { x: 0., y: -D / 2. }, Coord { x: 0., y: D / 2. });
        let parallel =
            geo_types::Line::new(Coord { x: -D / 2., y: 0. }, Coord { x: D / 2., y: 0. });

        // Project the stereographic baselines to the local grid
        let projected_meridian = transform.convert_geometry(meridian)?;
        let projected_parallel = transform.convert_geometry(parallel)?;

        // Points on the same meridian
        let meridian_delta = projected_meridian.delta() / D;
        let parallel_delta = projected_parallel.delta() / D;

        let determinant = parallel_delta.x * meridian_delta.y - parallel_delta.y * meridian_delta.x;
        if determinant < 0.00001 {
            Err(Error::ProjScaleToleranceError)?;
        }

        let convergence =
            (parallel_delta.y - meridian_delta.x).atan2(parallel_delta.x + meridian_delta.y);

        let grid_scale_factor = determinant.sqrt();

        Ok((convergence.to_degrees(), grid_scale_factor.try_into()?))
    }

    #[cfg(feature = "geo_ref")]
    fn get_elevation_scale_factor(geo_ref_point_deg: Coord, meters_above_sea_level: f64) -> f64 {
        // this is (ellipsoid_radius / (ellipsoid_radius + m_above_ellipsoid))
        //
        // ellipsoid_radius = R_equator * (1 - f * sin^2(lat))
        // f = 1 / 298.257223563
        // R_equator = 6378137.0m
        const F: f64 = 1. / 298.257223563;
        const R_EQUATOR: f64 = 6378137.;

        let ellipsoid_radius =
            R_EQUATOR * (1. - F * geo_ref_point_deg.y.to_radians().sin().powi(2));

        ellipsoid_radius / (ellipsoid_radius + meters_above_sea_level)
    }

    #[cfg(feature = "geo_ref")]
    fn get_declination(geo_ref_point_deg: Coord, meters_above_sea_level: f64) -> Result<f64> {
        use chrono::Datelike as _;
        use world_magnetic_model::{
            GeomagneticField,
            time::Date,
            uom::si::{
                angle::{Angle, degree},
                length::{Length, meter},
            },
        };

        let date = chrono::Local::now();
        let year = date.year();
        let day = date.ordinal() as u16;

        let model_date = Date::from_ordinal_date(year, day)
            .or_else(|_| Date::from_ordinal_date(2026, 180))
            .map_err(|_invalid_date| Error::InvalidGeoreferencing)?;

        let field = GeomagneticField::new(
            Length::new::<meter>(meters_above_sea_level as f32),
            Angle::new::<degree>(geo_ref_point_deg.y as f32),
            Angle::new::<degree>(geo_ref_point_deg.x as f32),
            model_date,
        )?;
        let dec = field.declination().get::<degree>();

        Ok(dec as f64)
    }
}
