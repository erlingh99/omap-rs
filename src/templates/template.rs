use std::path::PathBuf;

use super::template_transform::{AdjustmentState, TemplateTransformations};
use crate::templates::template_transform::{PassPoint, TemplateTransform};

/// A template attached to the map. Each variant carries type-specific data.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Template {
    /// A raster image template.
    Image(ImageTemplate),
    /// A map file template.
    Map(MapTemplate),
    /// A GPS track template.
    Track(TrackTemplate),
    /// A geospatial raster data template via GDAL.
    Gdal(GdalTemplate),
    /// A geospatial vector data template via OGR.
    Ogr(OgrTemplate),
}

/// A raster image template.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageTemplate {
    /// Fields shared by every template kind.
    pub common: TemplateCommon,
}

/// A map file template.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapTemplate {
    /// Fields shared by every template kind.
    pub common: TemplateCommon,
}

/// A GPS track template.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrackTemplate {
    /// Fields shared by every template kind.
    pub common: TemplateCommon,
    /// Coordinate reference system of the track data.
    pub track_crs_spec: String,
    /// Projected coordinate reference system used for the track.
    pub projected_crs_spec: String,
}

/// A geospatial raster data template (via GDAL).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GdalTemplate {
    /// Fields shared by every template kind.
    pub common: TemplateCommon,
    /// Coordinate reference system of the raster data.
    pub crs_spec: String,
}

/// A geospatial vector data template (via OGR).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OgrTemplate {
    /// Fields shared by every template kind.
    pub common: TemplateCommon,
    /// Coordinate reference system of the vector data.
    pub crs_spec: String,
    /// Coordinate reference system of any associated track data.
    pub track_crs_spec: String,
    /// Projected coordinate reference system used for the data.
    pub projected_crs_spec: String,
}

impl Template {
    /// Get the common properties shared by all template types.
    pub fn common(&self) -> &TemplateCommon {
        match self {
            Self::Image(t) => &t.common,
            Self::Map(t) => &t.common,
            Self::Track(t) => &t.common,
            Self::Gdal(t) => &t.common,
            Self::Ogr(t) => &t.common,
        }
    }

    /// Get the mutable common properties shared by all template types.
    pub fn common_mut(&mut self) -> &mut TemplateCommon {
        match self {
            Self::Image(t) => &mut t.common,
            Self::Map(t) => &mut t.common,
            Self::Track(t) => &mut t.common,
            Self::Gdal(t) => &mut t.common,
            Self::Ogr(t) => &mut t.common,
        }
    }

    /// Transform a non-georeferenced template in map coordinates.
    ///
    /// Georeferenced templates are left unchanged because their placement is
    /// derived from CRS metadata. For non-georeferenced templates, map-space
    /// transform parameters and passpoint map coordinates are transformed, and
    /// the adjustment state is marked dirty.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(geo_types::Coord) -> geo_types::Coord,
    {
        if self.common().is_georeferenced {
            return;
        }
        if let Some(transformations) = &mut self.common_mut().transformations {
            transformations.transform(transform);
        }
    }

    /// Try to transform a non-georeferenced template in map coordinates.
    ///
    /// Georeferenced templates are left unchanged because their placement is
    /// derived from CRS metadata. The template is unchanged on failure.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `transform`.
    pub fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(geo_types::Coord) -> Result<geo_types::Coord, E>,
    {
        if self.common().is_georeferenced {
            return Ok(());
        }
        if let Some(transformations) = &mut self.common_mut().transformations {
            transformations.try_transform(transform)?;
        }
        Ok(())
    }

    /// Returns the template type name as used in the XML format.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Image(_) => "TemplateImage",
            Self::Map(_) => "TemplateMap",
            Self::Track(_) => "TemplateTrack",
            Self::Gdal(_) => "GdalTemplate",
            Self::Ogr(_) => "OgrTemplate",
        }
    }
}

/// The common properties shared by all template types.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateCommon {
    /// Whether the template file was open (loaded) when the file was saved.
    pub is_open: bool,
    /// The filename without path, e.g. `"map.bmp"`.
    pub name: String,
    /// Absolute path to the template file.
    pub path: PathBuf,
    /// Path relative to the map file.
    pub relpath: PathBuf,
    /// Whether the template is in georeferenced mode.
    pub is_georeferenced: bool,
    /// Template group number (None = ungrouped).
    pub group: Option<u8>,
    /// Transformation data for non-georeferenced templates.
    pub transformations: Option<TemplateTransformations>,
}

impl TemplateCommon {
    /// Returns true if this template is georeferenced.
    pub fn is_georeferenced(&self) -> bool {
        self.is_georeferenced
    }

    /// Returns the active template transform, if present.
    pub fn active_transform(&self) -> Option<&TemplateTransform> {
        self.transformations.as_ref().map(|t| &t.active_transform)
    }

    /// Returns the other (inactive) template transform.
    pub fn other_transform(&self) -> Option<&TemplateTransform> {
        self.transformations.as_ref().map(|t| &t.other_transform)
    }

    /// Returns the pass-points, if any.
    pub fn passpoints(&self) -> &[PassPoint] {
        self.transformations.as_ref().map_or(&[], |t| &t.passpoints)
    }

    /// Returns the adjustment state.
    pub fn adjustment_state(&self) -> Option<&AdjustmentState> {
        self.transformations.as_ref().map(|t| &t.adjustment)
    }
}
