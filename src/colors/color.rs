use super::{Cmyk, CmykMode, ColorId, ColorKind, ColorSet, Rgb, RgbMode, SpotColorId};
use crate::utils::UnitF64;
use crate::{Error, NonNegativeF64, Result};

/// A named spot color with its own CMYK/RGB representation and screen parameters.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpotColor {
    /// The display name of this color.
    pub color_name: String,
    /// Whether this color knocks out colors beneath it.
    pub knockout: bool,
    cmyk_mode: CmykMode, // not allowed to be FromSpotColors and both this and rgb_mode cannot point at eachother
    rgb_mode: RgbMode,   // same as above
    /// The internal spot-color name used in printing.
    pub spotcolor_name: String,
    /// Screen ruling frequency (lines per inch).
    pub screen_frequency: NonNegativeF64,
    /// Screen ruling angle in degrees.
    pub screen_angle_deg: f64,
}

impl SpotColor {
    /// Create a new spot color with the given name and CMYK values.
    ///
    /// The RGB mode defaults to `FromCmyk` and screen parameters to `150.0` frequency and `0.0` angle.
    pub fn new(
        color_name: impl Into<String>,
        spotcolor_name: impl Into<String>,
        cmyk: Cmyk,
    ) -> Self {
        Self {
            color_name: color_name.into(),
            knockout: false,
            cmyk_mode: CmykMode::Cmyk(cmyk),
            rgb_mode: RgbMode::FromCmyk,
            spotcolor_name: spotcolor_name.into(),
            screen_frequency: NonNegativeF64::default(),
            screen_angle_deg: 0.0,
        }
    }

    /// This color with the given modes, taken as they are: for a reader that
    /// has already made them consistent.
    pub(crate) fn with_modes(mut self, cmyk_mode: CmykMode, rgb_mode: RgbMode) -> Self {
        self.cmyk_mode = cmyk_mode;
        self.rgb_mode = rgb_mode;
        self
    }

    /// Get the effective CMYK value of this spot color.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if the color modes do not define a valid
    /// CMYK value.
    pub fn cmyk(&self) -> Result<Cmyk> {
        match self.cmyk_mode {
            CmykMode::FromSpotColors => Err(Error::ColorError),
            CmykMode::FromRgb => match self.rgb_mode {
                RgbMode::FromSpotColors | RgbMode::FromCmyk => Err(Error::ColorError),
                RgbMode::Rgb(rgb) => Ok(rgb.into()),
            },
            CmykMode::Cmyk(cmyk) => Ok(cmyk),
        }
    }

    /// Get the effective RGB value of this spot color.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if the color modes do not define a valid
    /// RGB value.
    pub fn rgb(&self) -> Result<Rgb> {
        match self.rgb_mode {
            RgbMode::FromSpotColors => Err(Error::ColorError),
            RgbMode::FromCmyk => match self.cmyk_mode {
                CmykMode::FromSpotColors | CmykMode::FromRgb => Err(Error::ColorError),
                CmykMode::Cmyk(cmyk) => Ok(cmyk.into()),
            },
            RgbMode::Rgb(rgb) => Ok(rgb),
        }
    }

    /// Set the CMYK derivation mode. Fails if a circular dependency would be created.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `new` is not valid for a spot color or
    /// would create a circular dependency with the RGB mode.
    pub fn set_cmyk_mode(&mut self, new: CmykMode) -> Result<()> {
        if new == CmykMode::FromSpotColors
            || new == CmykMode::FromRgb && self.rgb_mode == RgbMode::FromCmyk
        {
            Err(Error::ColorError)
        } else {
            self.cmyk_mode = new;
            Ok(())
        }
    }

    /// Get the current CMYK derivation mode.
    pub fn cmyk_mode(&self) -> CmykMode {
        self.cmyk_mode
    }

    /// Set the RGB derivation mode. Fails if a circular dependency would be created.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `new` is not valid for a spot color or
    /// would create a circular dependency with the CMYK mode.
    pub fn set_rgb_mode(&mut self, new: RgbMode) -> Result<()> {
        if new == RgbMode::FromSpotColors
            || new == RgbMode::FromCmyk && self.cmyk_mode == CmykMode::FromRgb
        {
            Err(Error::ColorError)
        } else {
            self.rgb_mode = new;
            Ok(())
        }
    }

    /// Get the current RGB derivation mode.
    pub fn rgb_mode(&self) -> RgbMode {
        self.rgb_mode
    }

    /// Get the display name of this spot color.
    pub fn name(&self) -> &str {
        &self.color_name
    }

    /// Returns `true` if this color knocks out colors beneath it.
    pub fn is_knockout(&self) -> bool {
        self.knockout
    }
}

/// A weighted reference to a spot color, used as a component in [`MixedColor`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorComponent {
    /// in range [0, 1]
    pub factor: UnitF64,
    /// the spot color being mixed in
    pub color: SpotColorId,
}

/// A color that is a weighted mixture of one or more spot colors.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MixedColor {
    /// The display name of this mixed color.
    pub color_name: String,
    /// Whether this color knocks out colors beneath it.
    pub knockout: bool,
    cmyk_mode: CmykMode,
    rgb_mode: RgbMode,
    /// The spot-color components and their weights.
    pub components: Vec<ColorComponent>,
}

impl MixedColor {
    /// Create a new mixed color with the given name and spot-color components.
    ///
    /// Both CMYK and RGB modes default to `FromSpotColors`.
    pub fn new(color_name: impl Into<String>, components: Vec<ColorComponent>) -> Self {
        Self {
            color_name: color_name.into(),
            knockout: false,
            cmyk_mode: CmykMode::FromSpotColors,
            rgb_mode: RgbMode::FromSpotColors,
            components,
        }
    }

    /// This color with the given modes, taken as they are: for a reader that
    /// has already made them consistent.
    pub(crate) fn with_modes(mut self, cmyk_mode: CmykMode, rgb_mode: RgbMode) -> Self {
        self.cmyk_mode = cmyk_mode;
        self.rgb_mode = rgb_mode;
        self
    }

    /// Get the effective CMYK value of this mixed color.
    ///
    /// Takes the [`ColorSet`] that owns the component spot colors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if a component is unavailable or the
    /// color modes form an invalid dependency.
    pub fn cmyk(&self, color_set: &ColorSet) -> Result<Cmyk> {
        match self.cmyk_mode {
            CmykMode::FromSpotColors => self.cmyk_from_spotcolors(color_set),
            CmykMode::FromRgb => match self.rgb_mode {
                RgbMode::FromSpotColors => {
                    self.rgb_from_spotcolors(color_set).map(|rgb| rgb.into())
                }
                RgbMode::FromCmyk => Err(Error::ColorError),
                RgbMode::Rgb(rgb) => Ok(rgb.into()),
            },
            CmykMode::Cmyk(cmyk) => Ok(cmyk),
        }
    }

    /// Get the effective RGB value of this mixed color.
    ///
    /// Takes the [`ColorSet`] that owns the component spot colors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if a component is unavailable or the
    /// color modes form an invalid dependency.
    pub fn rgb(&self, color_set: &ColorSet) -> Result<Rgb> {
        match self.rgb_mode {
            RgbMode::FromSpotColors => self.rgb_from_spotcolors(color_set),
            RgbMode::FromCmyk => match self.cmyk_mode {
                CmykMode::FromSpotColors => {
                    self.cmyk_from_spotcolors(color_set).map(|cmyk| cmyk.into())
                }
                CmykMode::FromRgb => Err(Error::ColorError),
                CmykMode::Cmyk(cmyk) => Ok(cmyk.into()),
            },
            RgbMode::Rgb(rgb) => Ok(rgb),
        }
    }

    /// Get the display name of this mixed color.
    pub fn name(&self) -> &str {
        &self.color_name
    }

    /// Returns `true` if this color knocks out colors beneath it.
    pub fn is_knockout(&self) -> bool {
        self.knockout
    }

    /// Set the CMYK derivation mode. Fails if a circular dependency would be created.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `new` would create a circular
    /// dependency with the RGB mode.
    pub fn set_cmyk_mode(&mut self, new: CmykMode) -> Result<()> {
        if new == CmykMode::FromRgb && self.rgb_mode == RgbMode::FromCmyk {
            Err(Error::ColorError)
        } else {
            self.cmyk_mode = new;
            Ok(())
        }
    }

    /// Get the current CMYK derivation mode.
    pub fn cmyk_mode(&self) -> CmykMode {
        self.cmyk_mode
    }

    /// Set the RGB derivation mode. Fails if a circular dependency would be created.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if `new` would create a circular
    /// dependency with the CMYK mode.
    pub fn set_rgb_mode(&mut self, new: RgbMode) -> Result<()> {
        if new == RgbMode::FromCmyk && self.cmyk_mode == CmykMode::FromRgb {
            Err(Error::ColorError)
        } else {
            self.rgb_mode = new;
            Ok(())
        }
    }

    /// Get the current RGB derivation mode.
    pub fn rgb_mode(&self) -> RgbMode {
        self.rgb_mode
    }

    /// Compute the CMYK value by blending the component spot colors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if a component color is not in
    /// `color_set` or does not define an effective CMYK value.
    pub fn cmyk_from_spotcolors(&self, color_set: &ColorSet) -> Result<Cmyk> {
        let mut cmyk = Cmyk::default();

        for component in &self.components {
            let other = color_set
                .spot_color(component.color)
                .ok_or(Error::ColorError)?
                .cmyk()?;

            cmyk.c = UnitF64::clamped_from(
                cmyk.c.get() + component.factor.get() * other.c.get() * (1.0 - cmyk.c.get()),
            );
            cmyk.m = UnitF64::clamped_from(
                cmyk.m.get() + component.factor.get() * other.m.get() * (1.0 - cmyk.m.get()),
            );
            cmyk.y = UnitF64::clamped_from(
                cmyk.y.get() + component.factor.get() * other.y.get() * (1.0 - cmyk.y.get()),
            );
            cmyk.k = UnitF64::clamped_from(
                cmyk.k.get() + component.factor.get() * other.k.get() * (1.0 - cmyk.k.get()),
            );
        }
        Ok(cmyk)
    }

    /// Compute the RGB value by blending the component spot colors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ColorError`] if a component color is not in
    /// `color_set` or does not define an effective RGB value.
    pub fn rgb_from_spotcolors(&self, color_set: &ColorSet) -> Result<Rgb> {
        let mut rgb = Rgb::default();

        for component in &self.components {
            let other = color_set
                .spot_color(component.color)
                .ok_or(Error::ColorError)?
                .rgb()?;

            rgb.r = UnitF64::clamped_from(
                rgb.r.get() * (1.0 - component.factor.get() * (1.0 - other.r.get())),
            );
            rgb.g = UnitF64::clamped_from(
                rgb.g.get() * (1.0 - component.factor.get() * (1.0 - other.g.get())),
            );
            rgb.b = UnitF64::clamped_from(
                rgb.b.get() * (1.0 - component.factor.get() * (1.0 - other.b.get())),
            );
        }
        Ok(rgb)
    }
}

/// Either a [`SpotColor`] or a [`MixedColor`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Color {
    /// A spot color.
    SpotColor(SpotColor),
    /// A mixed color.
    MixedColor(MixedColor),
}

impl Color {
    /// The kind of this color.
    pub fn kind(&self) -> ColorKind {
        match self {
            Self::SpotColor(_) => ColorKind::Spot,
            Self::MixedColor(_) => ColorKind::Mixed,
        }
    }

    /// Get the effective CMYK value of this color.
    ///
    /// Takes the [`ColorSet`] that owns any component spot colors.
    ///
    /// # Errors
    ///
    /// Returns an error if the definition cannot produce a CMYK value.
    pub fn cmyk(&self, color_set: &ColorSet) -> Result<Cmyk> {
        match self {
            Self::SpotColor(color) => color.cmyk(),
            Self::MixedColor(color) => color.cmyk(color_set),
        }
    }

    /// Get the effective RGB value of this color.
    ///
    /// Takes the [`ColorSet`] that owns any component spot colors.
    ///
    /// # Errors
    ///
    /// Returns an error if the definition cannot produce an RGB value.
    pub fn rgb(&self, color_set: &ColorSet) -> Result<Rgb> {
        match self {
            Self::SpotColor(color) => color.rgb(),
            Self::MixedColor(color) => color.rgb(color_set),
        }
    }

    /// Get the display name of this color.
    pub fn name(&self) -> &str {
        match self {
            Self::SpotColor(color) => color.name(),
            Self::MixedColor(color) => color.name(),
        }
    }

    /// Returns `true` if this color knocks out colors beneath it.
    pub fn is_knockout(&self) -> bool {
        match self {
            Self::SpotColor(color) => color.is_knockout(),
            Self::MixedColor(color) => color.is_knockout(),
        }
    }

    /// Set the CMYK derivation mode for this color.
    ///
    /// # Errors
    ///
    /// Returns an error if `new` would make the color definition invalid.
    pub fn set_cmyk_mode(&mut self, new: CmykMode) -> Result<()> {
        match self {
            Self::SpotColor(color) => color.set_cmyk_mode(new),
            Self::MixedColor(color) => color.set_cmyk_mode(new),
        }
    }

    /// Set the RGB derivation mode for this color.
    ///
    /// # Errors
    ///
    /// Returns an error if `new` would make the color definition invalid.
    pub fn set_rgb_mode(&mut self, new: RgbMode) -> Result<()> {
        match self {
            Self::SpotColor(color) => color.set_rgb_mode(new),
            Self::MixedColor(color) => color.set_rgb_mode(new),
        }
    }

    /// Get the current CMYK derivation mode.
    pub fn cmyk_mode(&self) -> CmykMode {
        match self {
            Self::SpotColor(color) => color.cmyk_mode(),
            Self::MixedColor(color) => color.cmyk_mode(),
        }
    }

    /// Get the current RGB derivation mode.
    pub fn rgb_mode(&self) -> RgbMode {
        match self {
            Self::SpotColor(color) => color.rgb_mode(),
            Self::MixedColor(color) => color.rgb_mode(),
        }
    }
}

impl From<SpotColor> for Color {
    fn from(value: SpotColor) -> Self {
        Self::SpotColor(value)
    }
}

impl From<MixedColor> for Color {
    fn from(value: MixedColor) -> Self {
        Self::MixedColor(value)
    }
}

/// A color reference used by symbols: a regular color, registration black, or no color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SymbolColor {
    /// A reference to a map color.
    Color(ColorId),
    /// Registration black (prints on all separations).
    RegistrationBlack,
    /// No color (transparent).
    NoColor,
}

impl SymbolColor {
    /// The color this refers to, if it is a map color still in `color_set`.
    pub fn id(&self) -> Option<ColorId> {
        match self {
            Self::Color(id) => Some(*id),
            Self::RegistrationBlack | Self::NoColor => None,
        }
    }
}
