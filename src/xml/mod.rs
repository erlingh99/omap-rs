//! Reading and writing the `OpenOrienteering` Mapper XML format (`.omap`,
//! `.xmap`).
//!
//! The domain types know nothing of the format: everything about how a map is
//! laid out as XML, including how handles become the format's integer
//! indices, lives here.

mod attrs;
mod colors;
mod error;
mod format_info;
mod georef;
mod objects;
mod parts;
mod symbols;
mod templates;
mod text;
mod units;
mod view;

pub use error::{CoordinateComponent, Error, ObjectKind, Result, Section};
pub(in crate::xml) use text::{read_text, write_notes};

use crate::geo_referencing::GeoRef;
use crate::parts::MapParts;
use crate::symbols::SymbolSet;
use crate::templates::Templates;
use crate::view::View;
use crate::{Omap, format};
use format_info::{OmapVersion, XmlDeclaration};
use quick_xml::events::{BytesEnd, Event};
use quick_xml::{Reader, Writer};
use std::io::{BufReader, Read, Write};

/// The `OpenOrienteering` Mapper XML format: `.omap` and `.xmap` files.
#[derive(Debug, Clone, Copy, Default)]
pub struct Xml;

impl format::Format for Xml {
    fn extensions(&self) -> &'static [&'static str] {
        &["omap", "xmap"]
    }

    fn read(&self, reader: &mut dyn Read) -> std::result::Result<Omap, format::Error> {
        read(reader).map_err(format::Error::Xml)
    }

    fn write(&self, map: &Omap, writer: &mut dyn Write) -> std::result::Result<(), format::Error> {
        write(map, writer).map_err(format::Error::Xml)
    }
}

/// Read a map in the `OpenOrienteering` Mapper XML format.
///
/// Parsing is intentionally permissive for some sections.
/// This function falls back to sensible defaults
/// for `notes`, `templates`, or `view`
/// if those sections cannot be parsed
///
/// `barrier`s, `undo` and `redo` sections of the file are ignored
///
/// The core sections `georeferencing`, `colors`, `symbols`, and `parts`
/// must still parse successfully or else loading fails.
///
/// # Errors
///
/// Returns an error if a required map section cannot be parsed.
pub fn read(reader: impl Read) -> Result<Omap> {
    let mut reader = Reader::from_reader(BufReader::new(reader));
    reader.config_mut().expand_empty_elements = true;

    let mut georef = None;
    let mut colors = None;
    let mut symbols = None;
    let mut parts = None;

    let mut notes = String::new();
    let mut templates = Templates::default();
    let mut view = View::default();

    let mut xml_buf = Vec::new();
    loop {
        match reader.read_event_into(&mut xml_buf)? {
            Event::Decl(dec) => XmlDeclaration::parse(dec)?,
            Event::Start(bytes_start) => match bytes_start.local_name().as_ref() {
                "map" => OmapVersion::parse(&bytes_start)?,
                "notes" => notes = read_text(&mut reader).unwrap_or_default(),
                "georeferencing" => georef = Some(GeoRef::parse(&mut reader, &bytes_start)?),
                "colors" => {
                    colors = Some(colors::read_color_set(&mut reader, &bytes_start)?);
                }
                "symbols" => {
                    if let Some(colors) = &colors {
                        symbols = Some(SymbolSet::parse(&mut reader, &bytes_start, colors)?);
                    } else {
                        return Err(Error::SectionOutOfOrder {
                            section: Section::Symbols,
                            required_before: Section::Colors,
                        });
                    }
                }
                "parts" => {
                    if let Some((symbols, _)) = &symbols {
                        parts = Some(MapParts::parse(&mut reader, symbols)?);
                    } else {
                        return Err(Error::SectionOutOfOrder {
                            section: Section::Parts,
                            required_before: Section::Symbols,
                        });
                    }
                }
                "templates" => {
                    templates = Templates::parse(&mut reader, &bytes_start).unwrap_or_default();
                }
                "view" => {
                    view =
                        View::parse(&mut reader, &bytes_start, &mut templates).unwrap_or_default();
                }
                _ => (),
            },
            Event::End(bytes_end) => {
                if bytes_end.local_name().as_ref() == "map" {
                    break;
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    let (symbols, symbol_set_name) =
        symbols.ok_or(Error::MissingRequiredSection(Section::Symbols))?;
    Ok(Omap {
        notes,
        geo_referencing: georef.ok_or(Error::MissingRequiredSection(Section::Georeferencing))?,
        colors: colors.ok_or(Error::MissingRequiredSection(Section::Colors))?,
        symbols,
        symbol_set_name,
        parts: parts.ok_or(Error::MissingRequiredSection(Section::Parts))?,
        templates,
        view,
    })
}

/// Write `map` in the `OpenOrienteering` Mapper XML format.
///
/// The format cannot hold a reference to a removed color or symbol, so
/// the map is written as [`Omap::prune_dangling_references`] would leave
/// it. The map itself is not changed, and is only copied if something
/// dangles. Call [`Omap::validate`] first to catch such references.
///
/// Symbols are written in the order the symbol set holds them. Call
/// [`crate::symbols::SymbolSet::sort`] first to write them by
/// [`crate::Code`].
///
/// # Errors
///
/// Returns an error if any of the map data cannot be serialized.
pub fn write(map: &Omap, mut writer: impl Write) -> Result<()> {
    if map.validate().is_ok() {
        return write_valid(map, &mut writer);
    }
    let mut pruned = map.clone();
    pruned.prune_dangling_references();
    write_valid(&pruned, &mut writer)
}

/// Write a map that holds no dangling reference.
fn write_valid(map: &Omap, writer: &mut impl Write) -> Result<()> {
    let mut writer = Writer::new(writer);

    XmlDeclaration::write(&mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    OmapVersion::write(&mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;

    write_notes(map.notes.as_str(), &mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;

    map.geo_referencing.write(&mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;

    // write colors
    colors::write_color_set(&map.colors, &mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    // write symbols
    map.symbols
        .write(&mut writer, &map.colors, &map.symbol_set_name)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    // write objects
    map.parts.write(&mut writer, &map.symbols)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    // write templates
    let vis = map.templates.write(&mut writer)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    // write view
    map.view.write(&mut writer, vis)?;
    writer.get_mut().write_all(b"\n".as_slice())?;
    // write eof
    writer.write_event(Event::End(BytesEnd::new("map")))?;
    writer.get_mut().flush()?;
    Ok(())
}
