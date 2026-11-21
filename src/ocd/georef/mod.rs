//! Scale and georeferencing: the type 1039 parameter string.
//!
//! The file gives the real-world coordinates of the paper origin and the
//! grid's rotation (grivation), so the reference point is the paper origin.
//! The file has no declination of its own: the grivation is read as the
//! declination with no convergence, which gives the same grivation. Its
//! coordinate reference system is a grid and zone code, which a table
//! translates to EPSG codes where one exists.

mod grids;

use std::num::NonZeroU32;

use geo_types::Coord;

use crate::geo_referencing::{CrsType, GaussKrueger, GeoRef, UtmCode};
use crate::ocd::strings::{ParameterString, kind, number};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::view::{Grid, GridUnit};
use grids::GRID_ZONE_EPSG;

/// The grid and zone code of local coordinates.
const LOCAL_CODE: i32 = 1000;

/// Grid numbers with a meaning of their own.
mod grid {
    pub(super) const LOCAL: u16 = 1;
    pub(super) const UTM: u16 = 2;
    pub(super) const GERMANY: u16 = 8;
}

/// The scale and georeferencing, and the grid spacing in metres if the file
/// states one.
pub(in crate::ocd) fn read_georef(
    strings: &[ParameterString],
    default_scale: NonZeroU32,
    warnings: &mut Warnings,
) -> (GeoRef, Option<f64>) {
    let mut georef = GeoRef::new(default_scale);
    let mut grid_spacing = None;
    let Some(string) = strings.iter().find(|string| string.kind == kind::SCALE) else {
        return (georef, grid_spacing);
    };
    let mut real_world = false;
    let mut code = None;
    let mut offset = Coord::zero();
    for (field, value) in string.fields() {
        let Some(value) = number(value) else {
            continue;
        };
        match field {
            'm' => {
                if let Some(scale) =
                    NonZeroU32::new(value.round().clamp(0., f64::from(u32::MAX)) as u32)
                {
                    georef.scale_denominator = scale;
                }
            }
            'a' => georef.declination_deg = value,
            'x' => offset.x = value,
            'y' => offset.y = value,
            'i' => code = Some(value.round() as i32),
            'r' => real_world = value.round() as i32 != 0,
            'b' | 'c' if value != 0. => push_once(
                warnings,
                Warning::Map("an additional local coordinate offset was ignored"),
            ),
            'd' if value >= 0.001 => grid_spacing = Some(value),
            _ => (),
        }
    }
    georef.projected_ref_point = offset;
    if real_world && let Some(code) = code {
        match crs(code) {
            Some(crs) => georef.crs_type = crs,
            None => push_once(
                warnings,
                Warning::Map("the coordinate reference system is not known; read as local"),
            ),
        }
    }
    (georef, grid_spacing)
}

/// The coordinate reference system a grid and zone code names. A negative
/// code is in the southern hemisphere.
fn crs(code: i32) -> Option<CrsType> {
    let grid = u16::try_from(code.unsigned_abs() / 1000).ok()?;
    let zone = u16::try_from(code.unsigned_abs() % 1000).ok()?;
    match grid {
        grid::LOCAL => return Some(CrsType::Local),
        grid::UTM => {
            let zone = i8::try_from(zone).ok()?;
            let zone = if code < 0 { -zone } else { zone };
            return UtmCode::new(zone).ok().map(CrsType::Utm);
        }
        grid::GERMANY if (2..=5).contains(&zone) => {
            return GaussKrueger::new(u8::try_from(zone).ok()?)
                .ok()
                .map(CrsType::GaussKrueger);
        }
        _ => (),
    }
    if code < 0 {
        return None;
    }
    GRID_ZONE_EPSG
        .iter()
        .find(|&&(g, z, _)| g == grid && z == zone)
        .map(|&(_, _, epsg)| CrsType::Epsg(epsg))
}

/// The scale string for `georef`, with the grid spacing of `grid`.
///
/// OCAD stores the paper origin's easting and northing as whole metres, as
/// Mapper writes them; a fraction is rounded off with a warning.
pub(in crate::ocd) fn write_georef(
    georef: &GeoRef,
    grid: &Grid,
    warnings: &mut Warnings,
) -> ParameterString {
    let scale = f64::from(georef.scale_denominator.get());
    let spacing = grid.h_spacing.min(grid.v_spacing);
    let (on_map, on_ground) = match grid.unit {
        GridUnit::MetersOnGround => (spacing * 1000. / scale, spacing),
        GridUnit::MillimetresOnMap => (spacing, spacing * scale / 1000.),
        // Mapper's defaults.
        GridUnit::PixelsOnScreen => (50., 500.),
    };
    let origin = georef.create_transform().to_projected(Coord::zero());
    let rounded = Coord {
        x: origin.x.round(),
        y: origin.y.round(),
    };
    if (origin - rounded).x.hypot((origin - rounded).y) > 0.001 {
        push_once(
            warnings,
            Warning::Map("the paper origin's real-world coordinates were rounded to whole metres"),
        );
    }
    let code = grid_code(&georef.crs_type);
    if code.is_none() {
        push_once(
            warnings,
            Warning::Map(
                "the coordinate reference system has no OCAD grid code; written as paper coordinates",
            ),
        );
    }

    let mut string = ParameterString::new(kind::SCALE, "");
    string.push('m', georef.scale_denominator);
    string.push('g', on_map);
    string.push('r', i32::from(code.is_some()));
    string.push('x', rounded.x);
    string.push('y', rounded.y);
    string.push('a', georef.grivation_deg());
    string.push('d', on_ground);
    string.push('i', code.unwrap_or(LOCAL_CODE));
    string.push('b', 0);
    string.push('c', 0);
    string
}

/// The grid and zone code for `crs`, if OCAD has one.
fn grid_code(crs: &CrsType) -> Option<i32> {
    let code = |grid: u16, zone: i32| i32::from(grid) * 1000 + zone.abs();
    match crs {
        CrsType::Local => Some(code(grid::LOCAL, 0)),
        CrsType::Utm(zone) => {
            let zone = i32::from(zone.get());
            Some(code(grid::UTM, zone) * zone.signum())
        }
        CrsType::GaussKrueger(zone) if (2..=5).contains(&zone.get()) => {
            Some(code(grid::GERMANY, i32::from(zone.get())))
        }
        CrsType::GaussKrueger(_) => None,
        CrsType::Epsg(_) | CrsType::Proj4(_) => {
            let epsg = crs.epsg_code()?;
            match epsg {
                32601..=32660 => Some(code(grid::UTM, i32::from(epsg - 32600))),
                32701..=32760 => Some(-code(grid::UTM, i32::from(epsg - 32700))),
                _ => GRID_ZONE_EPSG
                    .iter()
                    .find(|&&(_, _, table_epsg)| table_epsg == epsg)
                    .map(|&(grid, zone, _)| code(grid, i32::from(zone))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::crs;
    use crate::geo_referencing::{CrsType, GaussKrueger, UtmCode};

    #[test]
    fn grid_and_zone_codes_name_systems() -> crate::Result<()> {
        assert_eq!(crs(1000), Some(CrsType::Local));
        assert_eq!(crs(2032), Some(CrsType::Utm(UtmCode::new(32)?)));
        assert_eq!(crs(-2033), Some(CrsType::Utm(UtmCode::new(-33)?)));
        assert_eq!(
            crs(8003),
            Some(CrsType::GaussKrueger(GaussKrueger::new(3)?))
        );
        assert_eq!(crs(8007), Some(CrsType::Epsg(25832)));
        assert_eq!(crs(13002), Some(CrsType::Epsg(3006)));
        assert_eq!(crs(2061), None);
        assert_eq!(crs(99001), None);
        Ok(())
    }

    #[test]
    fn systems_write_back_as_their_codes() -> crate::Result<()> {
        use super::grid_code;
        for code in [1000, 2032, -2033, 8003, 13002, 41004] {
            let crs = crs(code).ok_or(crate::Error::InvalidGeoreferencing)?;
            assert_eq!(grid_code(&crs), Some(code), "{crs:?}");
        }
        assert_eq!(grid_code(&CrsType::Epsg(32632)), Some(2032));
        assert_eq!(grid_code(&CrsType::Epsg(32733)), Some(-2033));
        assert_eq!(
            grid_code(&CrsType::Proj4("+init=epsg:25833".to_owned())),
            Some(8008)
        );
        assert_eq!(grid_code(&CrsType::Proj4("+proj=merc".to_owned())), None);
        Ok(())
    }
}
