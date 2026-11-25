use geo_types::Coord;

use crate::utils::{transform_position, try_transform_position};

/// A 3×3 matrix stored in row-major order.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Matrix3x3(pub [f64; 9]);

/// The `<transformations>` block for a non-georeferenced template.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateTransformations {
    /// Adjustment state.
    pub adjustment: AdjustmentState,
    /// The currently active transform (role="active").
    pub active_transform: TemplateTransform,
    /// The other (inactive) transform (role="other").
    pub other_transform: TemplateTransform,
    /// Pass-points used for adjustment.
    pub passpoints: Vec<PassPoint>,
    /// The 3×3 map-to-template matrix, row-major.
    pub map_to_template: Option<Matrix3x3>,
    /// The 3×3 template-to-map matrix, row-major.
    pub template_to_map: Option<Matrix3x3>,
    /// The 3×3 template-to-map matrix for the "other" transform, row-major.
    pub template_to_map_other: Option<Matrix3x3>,
}

/// Whether the adjustment is applied, dirty, or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AdjustmentState {
    /// No adjustment has been applied.
    NoAdjustment,
    /// Adjustment successfully applied (`adjusted="true"`).
    Adjusted,
    /// Adjustment needs recalculation (`adjustment_dirty="true"`).
    AdjustmentDirty,
}

/// Parameters for a single `<transformation>` element.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateTransform {
    /// Template position in mm of paper.
    pub template_pos: Coord,
    /// Rotation in radians (positive = counter-clockwise).
    pub template_rotation: f64,
    /// Template scaling (< 1 shrinks).
    pub template_scale: Coord,
    /// Shear component (rare, usually 0).
    pub template_shear: f64,
}

impl Default for TemplateTransform {
    fn default() -> Self {
        Self {
            template_pos: Coord::zero(),
            template_rotation: 0.,
            template_scale: Coord { x: 1., y: 1. },
            template_shear: 0.,
        }
    }
}

/// A pass-point relating source (template) coords to destination (map) coords.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PassPoint {
    /// Coordinate in the source template.
    pub src_coord: Coord,
    /// Corresponding coordinate on the map.
    pub dest_coord: Coord,
    /// Coordinate calculated by the current transformation.
    pub calculated_coord: Coord,
}

impl PassPoint {
    /// Return the distance between the calculated and destination coordinates.
    ///
    /// Returns `-1.0` when no calculated coordinate is available.
    pub fn error(&self) -> f64 {
        if self.calculated_coord == Coord::zero() {
            return -1.;
        }
        let diff = self.calculated_coord - self.dest_coord;
        diff.x.hypot(diff.y)
    }
}

impl TemplateTransformations {
    pub(crate) fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        self.active_transform.transform(&transform);
        self.other_transform.transform(&transform);
        for passpoint in &mut self.passpoints {
            passpoint.transform(&transform);
        }

        self.map_to_template = None;
        self.template_to_map = None;
        self.template_to_map_other = None;
        self.adjustment = AdjustmentState::AdjustmentDirty;
    }

    pub(crate) fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(Coord) -> Result<Coord, E>,
    {
        let mut transformed = self.clone();
        transformed.active_transform.try_transform(&transform)?;
        transformed.other_transform.try_transform(&transform)?;
        for passpoint in &mut transformed.passpoints {
            passpoint.try_transform(&transform)?;
        }

        transformed.map_to_template = None;
        transformed.template_to_map = None;
        transformed.template_to_map_other = None;
        transformed.adjustment = AdjustmentState::AdjustmentDirty;
        *self = transformed;
        Ok(())
    }
}

impl TemplateTransform {
    pub(crate) fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        let (position, rotation, scale_factor) = transform_position(self.template_pos, transform);
        self.template_pos = position;
        self.template_rotation += rotation;
        self.template_scale.x *= scale_factor;
        self.template_scale.y *= scale_factor;
    }

    pub(crate) fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(Coord) -> Result<Coord, E>,
    {
        let (position, rotation, scale_factor) =
            try_transform_position(self.template_pos, transform)?;
        self.template_pos = position;
        self.template_rotation += rotation;
        self.template_scale.x *= scale_factor;
        self.template_scale.y *= scale_factor;
        Ok(())
    }
}

impl PassPoint {
    pub(crate) fn transform<F>(&mut self, transform: F)
    where
        F: Fn(Coord) -> Coord,
    {
        self.src_coord = transform(self.src_coord);
        self.dest_coord = transform(self.dest_coord);
        if self.calculated_coord != Coord::zero() {
            self.calculated_coord = transform(self.calculated_coord);
        }
    }

    pub(crate) fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(Coord) -> Result<Coord, E>,
    {
        let src_coord = transform(self.src_coord)?;
        let dest_coord = transform(self.dest_coord)?;
        let calculated_coord = if self.calculated_coord == Coord::zero() {
            self.calculated_coord
        } else {
            transform(self.calculated_coord)?
        };

        self.src_coord = src_coord;
        self.dest_coord = dest_coord;
        self.calculated_coord = calculated_coord;
        Ok(())
    }
}
