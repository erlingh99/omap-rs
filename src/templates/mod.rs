mod template;
mod template_transform;

pub use template::{
    GdalTemplate, ImageTemplate, MapTemplate, OgrTemplate, Template, TemplateCommon, TrackTemplate,
};
pub use template_transform::{
    AdjustmentState, Matrix3x3, PassPoint, TemplateTransform, TemplateTransformations,
};

use crate::{NonNegativeF64, view::TemplateVisibility};

/// Default display settings for newly added templates.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateDefaults {
    /// Whether to use metres-per-pixel (instead of DPI).
    pub use_meters_per_pixel: bool,
    /// Metres per pixel when `use_meters_per_pixel` is true.
    pub meters_per_pixel: NonNegativeF64,
    /// Dots per inch when `use_meters_per_pixel` is false.
    pub dpi: NonNegativeF64,
    /// The map scale used for templates.
    pub scale: u32,
}

impl Default for TemplateDefaults {
    fn default() -> Self {
        Self {
            use_meters_per_pixel: true,
            meters_per_pixel: NonNegativeF64::default(),
            dpi: NonNegativeF64::default(),
            scale: 0,
        }
    }
}

/// A template entry pairing a template with its visibility settings.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateEntry {
    /// The template data.
    pub template: Template,
    /// The visibility settings for this template.
    pub visibility: TemplateVisibility,
}

/// All templates attached to the map, plus default display settings.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Templates {
    /// The template entries, ordered back-to-front.
    /// A [`TemplateEntry`] is a [Template] and [`TemplateVisibility`]
    pub template_entries: Vec<TemplateEntry>,
    /// Index of the first [Template] that is drawn in front of the map.
    /// Templates with `index >= first_front_template` are in front of the map.
    pub first_front_template: u32,
    /// Default rendering parameters shown in the template setup dialog.
    pub defaults: TemplateDefaults,
}

impl Templates {
    /// Number of templates.
    pub fn len(&self) -> usize {
        self.template_entries.len()
    }

    /// Iterate through the templates
    pub fn iter(&self) -> impl Iterator<Item = &TemplateEntry> {
        self.template_entries.iter()
    }

    /// Whether there are no templates.
    pub fn is_empty(&self) -> bool {
        self.template_entries.is_empty()
    }

    /// Transform all non-georeferenced templates in map coordinates.
    pub fn transform<F>(&mut self, transform: F)
    where
        F: Fn(geo_types::Coord) -> geo_types::Coord,
    {
        for entry in &mut self.template_entries {
            entry.template.transform(&transform);
        }
    }

    /// Try to transform all non-georeferenced templates in map coordinates.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `transform`. The collection is unchanged
    /// on failure.
    pub fn try_transform<E, F>(&mut self, transform: F) -> Result<(), E>
    where
        F: Fn(geo_types::Coord) -> Result<geo_types::Coord, E>,
    {
        let mut transformed = self.clone();
        for entry in &mut transformed.template_entries {
            entry.template.try_transform(&transform)?;
        }
        *self = transformed;
        Ok(())
    }
}
