use geo_types::Coord;

use crate::NonNegativeF64;
use crate::colors::Argb;
use crate::utils::UnitF64;

/// Visibility settings for a template or the map layer.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateVisibility {
    /// Opacity from 0.0 (invisible) to 1.0 (opaque).
    pub opacity: UnitF64,
    /// Whether this layer is visible.
    pub visible: bool,
}

impl Default for TemplateVisibility {
    fn default() -> Self {
        Self {
            opacity: UnitF64::one(),
            visible: false,
        }
    }
}

/// How the grid is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridDisplay {
    /// Grid is hidden.
    #[default]
    Hidden = 0,
    /// All grid lines are shown.
    AllLines = 1,
    /// Only horizontal lines are shown.
    HorizontalLines = 2,
    /// Only vertical lines are shown.
    VerticalLines = 3,
}

/// Grid alignment reference direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridAlignment {
    /// Aligned to magnetic north.
    #[default]
    MagneticNorth = 0,
    /// Aligned to grid north.
    GridNorth = 1,
    /// Aligned to true north.
    TrueNorth = 2,
}

/// Grid spacing unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridUnit {
    /// Meters on the ground.
    #[default]
    MetersOnGround = 0,
    /// Millimetres on the map.
    MillimetresOnMap = 1,
    /// Pixels on screen.
    PixelsOnScreen = 2,
}

/// The map grid display settings.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grid {
    /// Rgb Grid colour parsed from a hex string, e.g. `"#646464"`.
    pub color: Argb,
    /// Display mode.
    pub display: GridDisplay,
    /// Grid alignment reference direction.
    pub alignment: GridAlignment,
    /// Additional rotation in radians.
    pub additional_rotation: f64,
    /// Grid spacing unit.
    pub unit: GridUnit,
    /// Horizontal spacing.
    pub h_spacing: f64,
    /// Vertical spacing.
    pub v_spacing: f64,
    /// Horizontal offset.
    pub h_offset: f64,
    /// Vertical offset.
    pub v_offset: f64,
    /// Whether snapping to the grid is enabled.
    pub snapping_enabled: bool,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            color: Argb {
                a: UnitF64::one(),
                r: UnitF64::clamped_from(100. / 255.),
                g: UnitF64::clamped_from(100. / 255.),
                b: UnitF64::clamped_from(100. / 255.),
            },
            display: Default::default(),
            alignment: Default::default(),
            additional_rotation: 0.0,
            unit: Default::default(),
            h_spacing: 500.0,
            v_spacing: 500.0,
            h_offset: 0.0,
            v_offset: 0.0,
            snapping_enabled: false,
        }
    }
}

/// The view onto the map, including zoom, position, rotation, grid settings,
/// and visibility of the map layer and templates.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct View {
    /// Grid display settings.
    pub grid: Grid,
    /// Zoom factor.
    pub zoom: NonNegativeF64,
    /// View rotation in radians (counter-clockwise).
    pub rotation: f64,
    /// Horizontal position of the view centre (in mm map coordinates).
    pub view_centre: Coord,
    /// Visibility of the map drawing itself.
    pub map_visibility: TemplateVisibility,
    /// Whether all templates are hidden in this view.
    pub all_templates_hidden: bool,
    /// Whether the grid is visible.
    pub grid_visible: bool,
    /// Whether overprinting simulation is enabled.
    pub overprinting_simulation_enabled: bool,
    /// Render hatched polygons for the area objects
    pub area_hatching_enabled: bool,
    /// Render only baselines for the objects
    pub baseline_view_enabled: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            grid: Grid::default(),
            zoom: NonNegativeF64::one(),
            rotation: 0.0,
            view_centre: Coord::zero(),
            map_visibility: TemplateVisibility {
                opacity: UnitF64::one(),
                visible: true,
            },
            all_templates_hidden: false,
            grid_visible: false,
            overprinting_simulation_enabled: false,
            area_hatching_enabled: false,
            baseline_view_enabled: false,
        }
    }
}

impl View {}
