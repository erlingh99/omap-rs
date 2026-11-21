use std::{convert::Infallible, str::FromStr};

use geo_types::Coord;

use crate::{Error, Result};

/// A three-part version or symbol code of the form `A.B.C`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Code {
    /// Major version / code component.
    pub major: u16,
    /// Minor version / code component.
    pub minor: u16,
    /// Patch version / code component.
    pub patch: u16,
}

impl Code {
    /// Construct a new Code
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl FromStr for Code {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        let mut parts = value.split('.');

        let major = parts.next().ok_or(Error::BadCode)?.parse()?;
        let minor = if let Some(part) = parts.next() {
            part.parse()?
        } else {
            0
        };
        let patch = if let Some(part) = parts.next() {
            part.parse()?
        } else {
            0
        };

        if parts.next().is_some() {
            return Err(Error::BadCode);
        }

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.patch > 0 {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else if self.minor > 0 {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}", self.major)
        }
    }
}

/// A f64, but only allowed to be in the unit interval 0.0..=1.0
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnitF64(f64);

impl UnitF64 {
    /// Get the inner `f64` value.
    pub fn get(self) -> f64 {
        self.0
    }

    /// Get `UnitF64` from a f64, clamp values outside of the unit interval and map NaN to 0.
    pub fn clamped_from(value: f64) -> Self {
        if value.is_nan() {
            Self(0.)
        } else {
            Self(value.clamp(0., 1.))
        }
    }

    /// Get the unit value
    pub fn one() -> Self {
        Self(1.)
    }

    /// Get the zero value
    pub fn zero() -> Self {
        Self(0.)
    }
}

/// Tries to create a `UnitF64` from a f64, but succeeds only for values in the unit interval
impl TryFrom<f64> for UnitF64 {
    type Error = Error;

    fn try_from(v: f64) -> std::result::Result<Self, Self::Error> {
        if v.is_finite() && (0.0..=1.0).contains(&v) {
            Ok(Self(v))
        } else {
            Err(Error::NotInUnitInterval)
        }
    }
}

/// A f64, but not allowed to be 0 or less
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PositiveF64(f64);

impl Default for PositiveF64 {
    fn default() -> Self {
        Self::one()
    }
}

impl PositiveF64 {
    /// Get the inner `f64` value.
    pub fn get(self) -> f64 {
        self.0
    }

    /// Get the unit value
    pub fn one() -> Self {
        Self(1.)
    }
}

/// Tries to create a [`PositiveF64`] from a f64, but succeeds only for strictly positive values
impl TryFrom<f64> for PositiveF64 {
    type Error = Error;

    fn try_from(v: f64) -> Result<Self> {
        if v.is_finite() && v > 0. {
            Ok(Self(v))
        } else {
            Err(Error::NotPositiveF64)
        }
    }
}

/// A f64, but not allowed to be negative
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NonNegativeF64(f64);

impl NonNegativeF64 {
    /// Get the inner f64
    pub fn get(self) -> f64 {
        self.0
    }

    /// Get `NonNegativeF64` from a f64, clamp negative values to 0. and non-finite values to 0..
    pub fn clamped_from(value: f64) -> Self {
        if !value.is_finite() {
            Self(0.)
        } else {
            Self(value.max(0.))
        }
    }

    /// Get the unit value
    pub fn one() -> Self {
        Self(1.)
    }

    /// Get the zero value
    pub fn zero() -> Self {
        Self(0.)
    }
}

/// Tries to create a `NonNegativeF64` from a f64, but succeeds only for non-negative values
impl TryFrom<f64> for NonNegativeF64 {
    type Error = Error;

    fn try_from(v: f64) -> Result<Self> {
        if v.is_finite() && v >= 0. {
            Ok(Self(v))
        } else {
            Err(Error::NotNonNegativeF64)
        }
    }
}

/// Transform a position and estimate the transform's local rotation and scale.
pub(crate) fn transform_position<F>(position: Coord, transform: F) -> (Coord, f64, f64)
where
    F: Fn(Coord) -> Coord,
{
    match try_transform_position(position, |coord| Ok::<_, Infallible>(transform(coord))) {
        Ok(transformed) => transformed,
        Err(never) => match never {},
    }
}

/// Try to transform a position and estimate the transform's local rotation and scale.
pub(crate) fn try_transform_position<E, F>(
    position: Coord,
    transform: F,
) -> std::result::Result<(Coord, f64, f64), E>
where
    F: Fn(Coord) -> std::result::Result<Coord, E>,
{
    const D: f64 = 1.;
    let meridian = (
        position + Coord { x: 0., y: -D / 2. },
        position + Coord { x: 0., y: D / 2. },
    );
    let parallel = (
        position + Coord { x: -D / 2., y: 0. },
        position + Coord { x: D / 2., y: 0. },
    );

    let transformed_position = transform(position)?;
    let transformed_meridian = (transform(meridian.0)?, transform(meridian.1)?);
    let transformed_parallel = (transform(parallel.0)?, transform(parallel.1)?);
    let meridian_delta = (transformed_meridian.1 - transformed_meridian.0) / D;
    let parallel_delta = (transformed_parallel.1 - transformed_parallel.0) / D;
    let determinant = parallel_delta.x * meridian_delta.y - parallel_delta.y * meridian_delta.x;
    let rotation = (parallel_delta.y - meridian_delta.x).atan2(parallel_delta.x + meridian_delta.y);

    Ok((transformed_position, rotation, determinant.sqrt()))
}
