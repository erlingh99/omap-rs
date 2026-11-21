//! Reading and writing the OCAD format (`.ocd`), versions 12 and 2018.
//!
//! The domain types know nothing of the format: everything about how a map is
//! laid out in an OCAD file, including how handles become the file's color and
//! symbol numbers, lives here.
//!
//! OCAD and the map model do not describe exactly the same things. Reading
//! and writing never fail on a feature only one side has: they skip or
//! approximate it and report a [`Warning`].

mod colors;
mod error;
mod file;
mod geometry;
mod georef;
mod objects;
mod strings;
mod symbols;
mod templates;
mod units;
mod view;
mod warning;

pub use error::{Error, Result};
pub use warning::Warning;

use std::io::{Read, Write};
use std::num::NonZeroU32;

use crate::{Omap, format};
use colors::ColorWriter;
use file::{FileHeader, Layout, OCAD_MARK, OcdFile};
use warning::Warnings;

/// The OCAD format: `.ocd` files.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ocd;

impl format::Format for Ocd {
    fn extensions(&self) -> &'static [&'static str] {
        &["ocd"]
    }

    fn read(&self, reader: &mut dyn Read) -> std::result::Result<Omap, format::Error> {
        read(reader)
            .map(|reading| reading.map)
            .map_err(format::Error::Ocd)
    }

    fn write(&self, map: &Omap, writer: &mut dyn Write) -> std::result::Result<(), format::Error> {
        write(map, writer)
            .map(|_warnings| ())
            .map_err(format::Error::Ocd)
    }
}

/// A map read from an OCAD file, with what could not be read exactly.
#[derive(Debug, Clone)]
pub struct Reading {
    /// The map.
    pub map: Omap,
    /// What the map could not hold of the file, in the order it was met.
    pub warnings: Vec<Warning>,
}

/// The symbol set name a map read from OCAD gets, as Mapper names it.
const SYMBOL_SET_NAME: &str = "OCD";

/// The scale a map gets when its file does not state one.
const DEFAULT_SCALE: NonZeroU32 = NonZeroU32::new(15_000).expect("15000 is not zero");

/// Read a map in the OCAD format, versions 12 or 2018.
///
/// # Errors
///
/// Returns an error if the input cannot be read, is not an OCAD file, is of
/// another OCAD version, or is damaged.
pub fn read(mut reader: impl Read) -> Result<Reading> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let file = OcdFile::new(bytes)?;
    let mut warnings = Warnings::new();

    let strings = strings::read_strings(&file)?;
    let (colors, color_numbers) = colors::read_colors(&strings, &mut warnings);
    let (symbols, symbol_numbers) = symbols::read_symbols(&file, &color_numbers, &mut warnings)?;
    let objects = objects::read_objects(&file, &symbols, &symbol_numbers, &mut warnings)?;

    let (geo_referencing, grid_spacing) =
        georef::read_georef(&strings, DEFAULT_SCALE, &mut warnings);

    let mut map = Omap::new(DEFAULT_SCALE);
    map.notes = view::read_notes(&strings);
    map.geo_referencing = geo_referencing;
    map.colors = colors;
    map.symbols = symbols;
    map.symbol_set_name = SYMBOL_SET_NAME.to_owned();
    if let Some(part) = map.parts.at_mut(0) {
        part.objects = objects;
    }
    map.templates = templates::read_templates(&strings, &mut warnings);
    map.view = view::read_view(&strings, grid_spacing);
    Ok(Reading { map, warnings })
}

/// The version this crate writes.
const WRITTEN_VERSION: u16 = 12;

/// Write `map` in the OCAD format, version 12, and return what the file
/// could not hold of it.
///
/// The format cannot hold a reference to a removed color or symbol, so the
/// map is written as [`Omap::prune_dangling_references`] would leave it. The
/// map itself is not changed, and is only copied if something dangles.
///
/// # Errors
///
/// Returns an error if the output cannot be written, or the map is too large
/// for the format.
pub fn write(map: &Omap, writer: impl Write) -> Result<Vec<Warning>> {
    if map.validate().is_ok() {
        return write_valid(map, writer);
    }
    let mut pruned = map.clone();
    pruned.prune_dangling_references();
    write_valid(&pruned, writer)
}

/// Write a map that holds no dangling reference.
fn write_valid(map: &Omap, mut writer: impl Write) -> Result<Vec<Warning>> {
    let mut warnings = Warnings::new();
    let colors = ColorWriter::new(&map.colors, &map.symbols)?;
    let written_symbols = symbols::write_symbols(map, &colors, &mut warnings)?;
    let objects = objects::write_objects(map, &written_symbols, &mut warnings)?;
    let mut strings = colors.strings(&map.colors, &mut warnings);
    strings.push(georef::write_georef(
        &map.geo_referencing,
        &map.view.grid,
        &mut warnings,
    ));
    strings.extend(templates::write_templates(&map.templates, &mut warnings));
    strings.extend(view::write_notes(&map.notes));
    strings.push(view::write_view(&map.view));

    let mut layout = Layout::new();
    let mut header = FileHeader {
        mark: OCAD_MARK,
        version: WRITTEN_VERSION,
        ..FileHeader::default()
    };
    header.first_string_block = layout.add_index(strings::string_items(&strings)?)?;
    header.first_symbol_block = layout.add_index(written_symbols.items)?;
    header.first_object_block = layout.add_index(objects)?;
    writer.write_all(&layout.finish(&header)?)?;
    writer.flush()?;
    Ok(warnings)
}
