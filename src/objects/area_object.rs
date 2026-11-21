use crate::{Transformable, geo_referencing::try_transform_all};
use std::collections::HashMap;

use geo_types::{Coord, Polygon};

use super::{BezierPath, FlattenedPath};
use crate::{
    Error, NonNegativeF64, Result, symbols::AreaPathSymbolId, utils::try_transform_position,
};

/// A polygon whose exterior and interior rings retain straight and cubic
/// Bézier segments and dash-point metadata.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierPolygon {
    /// The polygon's exterior ring.
    exterior: BezierPath,
    /// The polygon's interior rings.
    interiors: Vec<BezierPath>,
}

impl BezierPolygon {
    /// Construct a polygon, closing every nonempty ring with a straight segment.
    ///
    /// # Errors
    ///
    /// Returns an error when a path is invalid.
    pub fn new(mut exterior: BezierPath, mut interiors: Vec<BezierPath>) -> Result<Self> {
        exterior.validate()?;
        exterior.close();
        interiors.retain(|ring| !ring.is_empty());
        for ring in &mut interiors {
            ring.validate()?;
            ring.close();
        }
        Ok(Self {
            exterior,
            interiors,
        })
    }

    /// Construct an empty polygon.
    pub fn empty() -> Self {
        Self {
            exterior: BezierPath::empty(),
            interiors: Vec::new(),
        }
    }

    /// Fit smooth Bézier paths to every ring within `allowed_error`.
    ///
    /// Every fitted vertex initially has its forced dash-point state disabled.
    /// Empty interior rings are omitted, and an empty exterior produces an
    /// empty polygon.
    ///
    /// Unlike [`From<Polygon>`], which preserves every input segment as a
    /// straight line, this method may replace several input segments with one
    /// cubic Bézier segment.
    ///
    /// # Errors
    ///
    /// Returns an error when a nonempty ring contains fewer than two
    /// coordinates or `allowed_error` is too small for fitting.
    pub fn fit_polygon(polygon: Polygon, allowed_error: NonNegativeF64) -> Result<Self> {
        let (exterior, interiors) = polygon.into_inner();
        let exterior = if exterior.0.is_empty() {
            BezierPath::empty()
        } else {
            BezierPath::fit_line_string(exterior, allowed_error)?
        };
        let interiors = interiors
            .into_iter()
            .filter(|ring| !ring.0.is_empty())
            .map(|ring| BezierPath::fit_line_string(ring, allowed_error))
            .collect::<Result<Vec<_>>>()?;
        Self::new(exterior, interiors)
    }

    /// Get the exterior ring.
    pub fn exterior(&self) -> &BezierPath {
        &self.exterior
    }

    /// Get the interior rings.
    pub fn interiors(&self) -> &[BezierPath] {
        &self.interiors
    }

    /// Get mutable access to the exterior ring.
    pub fn exterior_mut(&mut self) -> &mut BezierPath {
        &mut self.exterior
    }

    /// Get mutable access to the interior rings.
    pub fn interiors_mut(&mut self) -> &mut [BezierPath] {
        &mut self.interiors
    }

    /// Consume the polygon and return its rings.
    pub fn into_parts(self) -> (BezierPath, Vec<BezierPath>) {
        (self.exterior, self.interiors)
    }

    /// Return whether the exterior ring is empty.
    pub fn is_empty(&self) -> bool {
        self.exterior.is_empty()
    }

    /// Flatten every ring while retaining dash-point metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when a tolerance is too small or a ring is invalid.
    pub fn flatten(&self, allowed_error: NonNegativeF64) -> Result<FlattenedPolygon> {
        self.validate()?;
        let exterior = self.exterior.flatten(allowed_error)?;
        let interiors = self
            .interiors
            .iter()
            .filter(|ring| !ring.is_empty())
            .map(|ring| ring.flatten(allowed_error))
            .collect::<Result<Vec<_>>>()?;
        FlattenedPolygon::new(exterior, interiors)
    }

    /// Reverse the winding order of every ring.
    pub fn reverse(&mut self) {
        self.exterior.reverse();
        for ring in &mut self.interiors {
            ring.reverse();
        }
    }

    /// Validate ring topology and path metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when a path is invalid or a nonempty ring is open.
    pub fn validate(&self) -> Result<()> {
        self.exterior.validate()?;
        if !self.exterior.is_empty() && !self.exterior.is_closed() {
            return Err(Error::OpenPolygonRing);
        }
        for ring in &self.interiors {
            ring.validate()?;
            if !ring.is_empty() && !ring.is_closed() {
                return Err(Error::OpenPolygonRing);
            }
        }
        Ok(())
    }
}

/// Every ring is transformed; topology and dash-point metadata are kept.
impl Transformable for BezierPolygon {
    fn try_transform<E>(
        mut self,
        transform: impl Fn(Coord) -> std::result::Result<Coord, E>,
    ) -> std::result::Result<Self, E> {
        self.exterior = self.exterior.try_transform(&transform)?;
        self.interiors = try_transform_all(self.interiors, transform)?;
        Ok(self)
    }
}

impl From<Polygon> for BezierPolygon {
    fn from(polygon: Polygon) -> Self {
        let (exterior, interiors) = polygon.into_inner();
        Self {
            exterior: exterior.into(),
            interiors: interiors.into_iter().map(BezierPath::from).collect(),
        }
    }
}

/// An owned flattened polygon whose rings retain dash-point metadata.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FlattenedPolygon {
    /// The flattened exterior ring.
    exterior: FlattenedPath,
    /// The flattened interior rings.
    interiors: Vec<FlattenedPath>,
}

impl FlattenedPolygon {
    /// Construct a flattened polygon from closed rings.
    ///
    /// # Errors
    ///
    /// Returns [`Error::OpenPolygonRing`] when a nonempty ring is not closed.
    pub fn new(exterior: FlattenedPath, interiors: Vec<FlattenedPath>) -> Result<Self> {
        if (!exterior.geometry().0.is_empty() && !exterior.geometry().is_closed())
            || interiors
                .iter()
                .any(|ring| !ring.geometry().0.is_empty() && !ring.geometry().is_closed())
        {
            return Err(Error::OpenPolygonRing);
        }
        Ok(Self {
            exterior,
            interiors,
        })
    }

    /// Get the exterior ring.
    pub fn exterior(&self) -> &FlattenedPath {
        &self.exterior
    }

    /// Get the interior rings.
    pub fn interiors(&self) -> &[FlattenedPath] {
        &self.interiors
    }

    /// Consume the polygon and return its rings.
    pub fn into_parts(self) -> (FlattenedPath, Vec<FlattenedPath>) {
        (self.exterior, self.interiors)
    }

    /// Consume the flattened polygon and discard dash metadata.
    pub fn into_polygon(self) -> Polygon {
        let exterior = self.exterior.into_parts().0;
        let interiors = self
            .interiors
            .into_iter()
            .map(|ring| ring.into_parts().0)
            .collect();
        Polygon::new(exterior, interiors)
    }
}

impl From<FlattenedPolygon> for BezierPolygon {
    fn from(polygon: FlattenedPolygon) -> Self {
        let (exterior, interiors) = polygon.into_parts();
        Self {
            exterior: exterior.into(),
            interiors: interiors.into_iter().map(BezierPath::from).collect(),
        }
    }
}

/// A fill pattern rotation and origin used by area objects.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PatternRotation {
    /// Rotation of the fill pattern in radians.
    pub rotation: f64,
    /// Origin coordinate for the pattern.
    pub coord: Coord,
}

/// An area object whose geometry retains straight and cubic rings.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AreaObject {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[expect(
        clippy::box_collection,
        reason = "the map header is 48 bytes inline and most objects carry no tags"
    )]
    tags: Option<Box<HashMap<String, String>>>,
    /// The fill-pattern rotation and origin.
    pub pattern_rotation: PatternRotation,
    /// The area or combined-area symbol used to render this object.
    pub symbol: Option<AreaPathSymbolId>,
    geometry: BezierPolygon,
}

impl AreaObject {
    /// Create an area object from a Bézier or `geo_types` polygon.
    pub fn new(symbol: Option<AreaPathSymbolId>, geometry: impl Into<BezierPolygon>) -> Self {
        Self {
            tags: None,
            pattern_rotation: PatternRotation::default(),
            symbol,
            geometry: geometry.into(),
        }
    }

    /// This object with the tags a file gave it.
    pub(crate) fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = (!tags.is_empty()).then(|| Box::new(tags));
        self
    }

    /// The tags associated with the object.
    pub fn tags(&self) -> &HashMap<String, String> {
        match &self.tags {
            Some(tags) => tags,
            None => super::empty_tags(),
        }
    }

    /// Mutably access the tags, allocating the map on first use.
    pub fn tags_mut(&mut self) -> &mut HashMap<String, String> {
        self.tags.get_or_insert_with(Box::default)
    }

    /// Get the mixed straight/cubic polygon.
    pub fn geometry(&self) -> &BezierPolygon {
        &self.geometry
    }

    /// Mutably access the mixed straight/cubic polygon.
    pub fn geometry_mut(&mut self) -> &mut BezierPolygon {
        &mut self.geometry
    }

    /// Consume the object and return its geometry.
    pub fn into_geometry(self) -> BezierPolygon {
        self.geometry
    }

    /// Create an owned flattened polygon with dash-point metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when flattening fails.
    pub fn flatten(&self, allowed_error: NonNegativeF64) -> Result<FlattenedPolygon> {
        self.geometry.flatten(allowed_error)
    }

    /// Replace the rings with straight flattened segments.
    pub fn replace_with_flattened(&mut self, geometry: FlattenedPolygon) {
        self.geometry = geometry.into();
    }

    /// Permanently replace curves with their flattened straight segments.
    ///
    /// # Errors
    ///
    /// Returns an error when flattening fails.
    pub fn flatten_in_place(&mut self, allowed_error: NonNegativeF64) -> Result<()> {
        self.geometry = self.flatten(allowed_error)?.into();
        Ok(())
    }

    /// Create an area object for use as a point-symbol element.
    pub fn new_element(geometry: impl Into<BezierPolygon>) -> Self {
        Self::new(None, geometry)
    }

    pub(crate) fn geometry_is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// Transform the polygon and pattern orientation.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        let geometry =
            std::mem::replace(&mut self.geometry, BezierPolygon::empty()).transform(&transform);
        let (pattern_coord, pattern_rotation, _) =
            crate::utils::transform_position(self.pattern_rotation.coord, &transform);

        self.geometry = geometry;
        self.pattern_rotation.coord = pattern_coord;
        self.pattern_rotation.rotation += pattern_rotation;
    }

    /// Try to transform the polygon and pattern orientation.
    ///
    /// # Errors
    ///
    /// Returns the first transformation error. The object is unchanged on
    /// failure.
    pub fn try_transform<E, F>(&mut self, transform: F) -> std::result::Result<(), E>
    where
        F: Fn(Coord) -> std::result::Result<Coord, E>,
    {
        let geometry = self.geometry.clone().try_transform(&transform)?;
        let (pattern_coord, pattern_rotation, _) =
            try_transform_position(self.pattern_rotation.coord, &transform)?;

        self.geometry = geometry;
        self.pattern_rotation.coord = pattern_coord;
        self.pattern_rotation.rotation += pattern_rotation;
        Ok(())
    }

    /// Reverse the winding order of every ring.
    pub fn reverse(&mut self) {
        self.geometry.reverse();
    }
}
