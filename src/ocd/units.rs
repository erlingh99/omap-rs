//! The file's units: lengths in hundredths of a millimetre of paper, angles in
//! tenths of a degree counter-clockwise, and coordinates that carry flags in
//! their lowest 8 bits.
//!
//! The file's y axis points up, as the map's does, so coordinates change only
//! in scale.

use binrw::binrw;
use geo_types::Coord;

use crate::NonNegativeF64;
use crate::ocd::{Error, Result};

/// File length units in one millimetre.
const UNITS_PER_MM: f64 = 100.;

/// A file length in millimetres.
pub(in crate::ocd) fn mm(value: impl Into<f64>) -> f64 {
    value.into() / UNITS_PER_MM
}

/// A file length in millimetres, with negative lengths read as 0.
pub(in crate::ocd) fn length(value: impl Into<f64>) -> NonNegativeF64 {
    NonNegativeF64::clamped_from(mm(value))
}

/// Millimetres as a file length, rounded to the nearest unit.
pub(in crate::ocd) fn to_units(mm: f64) -> f64 {
    (mm * UNITS_PER_MM).round()
}

/// A file angle in radians, counter-clockwise, in `[0, 2π)`.
pub(in crate::ocd) fn radians(tenths_of_degree: impl Into<f64>) -> f64 {
    (tenths_of_degree.into() / 10.)
        .rem_euclid(360.)
        .to_radians()
}

/// Radians as a file angle in tenths of a degree, in `[0, 3600)`.
pub(in crate::ocd) fn to_tenths_of_degree(radians: f64) -> i16 {
    let tenths = (radians.to_degrees() * 10.).round().rem_euclid(3600.);
    // In [0, 3600), so it fits.
    tenths as i16
}

/// A coordinate as the file stores it: the value in the upper 24 bits of
/// each component, flags in the lowest 8.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::ocd) struct FileCoord {
    pub(in crate::ocd) x: i32,
    pub(in crate::ocd) y: i32,
}

impl FileCoord {
    /// Flag on x: the first control point of a Bézier curve.
    pub(in crate::ocd) const FIRST_CONTROL_POINT: i32 = 0x01;
    /// Flag on x: the second control point of a Bézier curve.
    pub(in crate::ocd) const SECOND_CONTROL_POINT: i32 = 0x02;
    /// Flag on y: a corner point.
    pub(in crate::ocd) const CORNER_POINT: i32 = 0x01;
    /// Flag on y: the first point of a hole in an area.
    pub(in crate::ocd) const HOLE_START: i32 = 0x02;
    /// Flag on y: a dash point.
    pub(in crate::ocd) const DASH_POINT: i32 = 0x08;

    /// The largest coordinate value in file units, which has 24 bits.
    const MAX_VALUE: f64 = ((1 << 23) - 1) as f64;

    /// The point in map millimetres.
    pub(in crate::ocd) fn coord(self) -> Coord {
        Coord {
            x: mm(value(self.x)),
            y: mm(value(self.y)),
        }
    }

    /// `coord`, in map millimetres, as a file coordinate without flags.
    pub(in crate::ocd) fn from_coord(coord: Coord) -> Result<Self> {
        let component = |mm: f64| {
            let units = to_units(mm);
            if !units.is_finite() || units.abs() > Self::MAX_VALUE {
                return Err(Error::CoordinateOutOfRange);
            }
            // In range, so the shift cannot overflow.
            Ok((units as i32) << 8)
        };
        Ok(Self {
            x: component(coord.x)?,
            y: component(coord.y)?,
        })
    }

    pub(in crate::ocd) fn is_first_control_point(self) -> bool {
        self.x & Self::FIRST_CONTROL_POINT != 0
    }

    pub(in crate::ocd) fn is_hole_start(self) -> bool {
        self.y & Self::HOLE_START != 0
    }

    /// Whether this is a dash point. Corner points count too, as Mapper reads
    /// them.
    pub(in crate::ocd) fn is_dash_point(self) -> bool {
        self.y & (Self::DASH_POINT | Self::CORNER_POINT) != 0
    }
}

/// A coordinate component's value, without its flags.
fn value(component: i32) -> i32 {
    // Mapper 0.6.2 to 0.6.4 wrote -0.001 to -0.004 mm as this value (its
    // issue 749). It is the largest negative value, so no real data uses it.
    const BROKEN_ZERO: i32 = i32::MIN >> 8;
    match component >> 8 {
        BROKEN_ZERO => 0,
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::{FileCoord, radians, to_tenths_of_degree};
    use geo_types::Coord;

    #[test]
    fn coordinates_are_hundredths_of_a_millimetre_above_the_flags() -> crate::ocd::Result<()> {
        let file = FileCoord {
            x: (1234 << 8) | FileCoord::FIRST_CONTROL_POINT,
            y: (-50 << 8) | FileCoord::DASH_POINT,
        };
        assert_eq!(file.coord(), Coord { x: 12.34, y: -0.5 });
        assert!(file.is_first_control_point());
        assert!(file.is_dash_point());
        assert_eq!(
            FileCoord::from_coord(Coord { x: 12.34, y: -0.5 })?,
            FileCoord {
                x: 1234 << 8,
                y: -50 << 8
            }
        );
        Ok(())
    }

    #[test]
    fn a_coordinate_beyond_24_bits_cannot_be_written() {
        assert!(FileCoord::from_coord(Coord { x: 84_000., y: 0. }).is_err());
    }

    #[test]
    fn angles_wrap_into_one_turn() {
        assert!((radians(-900) - 270_f64.to_radians()).abs() < 1e-12);
        assert_eq!(to_tenths_of_degree(radians(-900)), 2700);
        assert_eq!(to_tenths_of_degree(-std::f64::consts::FRAC_PI_2), 2700);
    }
}
