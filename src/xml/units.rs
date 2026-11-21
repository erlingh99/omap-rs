//! The file's integer units: coordinates and lengths in micrometres of paper.

use crate::{Error, NonNegativeF64, Result};
use geo_types::Coord;

/// The largest coordinate, in millimetres, the file's integer micrometres hold.
const FILE_COORD_MAX: f64 = ((i32::MAX / 1000) - 1) as f64;

pub(crate) fn to_file_coords(map_coord: Coord) -> Result<Coord<i32>> {
    Ok(Coord {
        x: to_file_value(map_coord.x)?,
        y: -to_file_value(map_coord.y)?,
    })
}

pub(crate) fn to_file_value(value: f64) -> Result<i32> {
    if !value.is_finite() || value.abs() > FILE_COORD_MAX {
        return Err(Error::MapCoordOutOfBounds);
    }
    Ok((value * 1000.).round() as i32)
}

pub(crate) fn from_file_value(value: i32) -> f64 {
    value as f64 / 1000.
}

pub(crate) fn from_file_coords(file_coord: Coord<i32>) -> Coord {
    Coord {
        x: from_file_value(file_coord.x),
        y: -from_file_value(file_coord.y),
    }
}

impl NonNegativeF64 {
    /// The files uses 1/1000 mm as the unit
    pub(crate) fn to_file_value(self) -> Result<u32> {
        Ok(to_file_value(self.get())?.try_into()?)
    }

    /// Create from file value (1/1000 mm integer) to mm
    pub(crate) fn from_file_value(value: u32) -> Self {
        Self::clamped_from(from_file_value(value as i32))
    }
}
