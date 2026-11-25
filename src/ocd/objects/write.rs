//! Writing objects, the inverse of reading them.
//!
//! Every part's objects go into the file's one object index. An object is
//! written once for each number its symbol was written under: once, or once
//! per part of a combined symbol broken down.

use std::io::Cursor;

use binrw::BinWrite as _;
use geo_types::Coord;

use super::{ObjectHeader, ObjectIndexEntry, object_type};
use crate::Omap;
use crate::objects::{HorizontalAlign, MapObject, TextGeometry, TextObject, VerticalAlign};
use crate::ocd::geometry::{write_polygon, write_rings};
use crate::ocd::symbols::{Drawn, WrittenSymbols};
use crate::ocd::units::{FileCoord, mm, to_tenths_of_degree};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::ocd::{Error, Result};
use crate::symbols::{DashStyle, Symbol};

/// The symbol number of an object without a symbol.
const NO_SYMBOL: i32 = -1;

/// The most text an object holds, in bytes, as Mapper writes it.
const MAX_TEXT_BYTES: usize = 8192;

/// Text is stored in chunks of this many bytes.
const TEXT_CHUNK_BYTES: usize = 64;

/// The size of the box estimated around point text, relative to its font
/// size: a character's width and a line's height.
const CHARACTER_WIDTH: f64 = 0.6;
const LINE_HEIGHT: f64 = 1.2;

/// Write every object of `map`.
pub(in crate::ocd) fn write_objects(
    map: &Omap,
    symbols: &WrittenSymbols,
    warnings: &mut Warnings,
) -> Result<Vec<(ObjectIndexEntry, Vec<u8>)>> {
    if map.parts.len() > 1 {
        warnings.push(Warning::Map(
            "OCAD has no map parts; the objects of every part were written together",
        ));
    }
    let extents = symbols.extents();
    let mut items = Vec::new();
    for (index, object) in map.iter_all_objects().enumerate() {
        if !object.tags().is_empty() {
            push_once(warnings, Warning::Map("object tags are not supported"));
        }
        let symbol = object.symbol().and_then(|id| map.symbols.get(id));
        let numbers: Vec<(i32, Drawn)> = match (object, symbol) {
            (_, None) => vec![(NO_SYMBOL, drawn_without_symbol(object))],
            (MapObject::Text(text), Some(entry)) => {
                let vertical = match text.geometry() {
                    TextGeometry::SingleAnchor(_) => Some(text.v_align),
                    TextGeometry::WrapBox(_) => None,
                };
                symbols
                    .text_number(entry.id(), text.h_align, vertical)
                    .map(|number| vec![(number, Drawn::Text)])
                    .unwrap_or_default()
            }
            (_, Some(entry)) => symbols.numbers(entry.id()).to_vec(),
        };
        if numbers.is_empty() {
            warnings.push(Warning::Object {
                index,
                issue: "the object's symbol draws nothing in OCAD; skipped",
            });
        }
        let dash_flag = dash_flag(symbol.map(|entry| entry.value()));
        for (number, drawn) in numbers {
            let font_size = match symbol.map(|entry| entry.value()) {
                Some(Symbol::Text(text)) => text.font_size.get(),
                _ => 4.,
            };
            let extent = mm(extents.get(&number).copied().unwrap_or_default());
            for file_object in
                FileObject::new(object, number, drawn, dash_flag, font_size, index, warnings)?
            {
                items.push(file_object.item(extent)?);
            }
        }
    }
    Ok(items)
}

fn drawn_without_symbol(object: &MapObject) -> Drawn {
    match object {
        MapObject::Point(_) => Drawn::Point,
        MapObject::Line(_) => Drawn::Line,
        MapObject::Area(_) => Drawn::Area,
        MapObject::Text(_) => Drawn::Text,
    }
}

/// The flag dash points get, as Mapper writes them: a dash point on a dashed
/// line without a dash symbol, a corner point otherwise. Reading takes both
/// as dash points.
fn dash_flag(symbol: Option<&Symbol>) -> i32 {
    match symbol {
        Some(Symbol::Line(line))
            if line.dash_symbol.is_none()
                && matches!(line.dash_style, DashStyle::Dashed { .. }) =>
        {
            FileCoord::DASH_POINT
        }
        _ => FileCoord::CORNER_POINT,
    }
}

/// An object as it is written.
struct FileObject {
    header: ObjectHeader,
    coords: Vec<FileCoord>,
    text: Vec<u16>,
}

impl FileObject {
    /// `object` written under `symbol`: one object, or, for an area drawn
    /// by a line symbol, one line object per ring, since OCAD knows holes
    /// only in areas.
    fn new(
        object: &MapObject,
        symbol: i32,
        drawn: Drawn,
        dash_flag: i32,
        font_size: f64,
        index: usize,
        warnings: &mut Warnings,
    ) -> Result<Vec<Self>> {
        if let MapObject::Area(area) = object
            && matches!(drawn, Drawn::Line)
        {
            let polygon = area.geometry();
            let mut objects = Vec::new();
            for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                let coords = write_rings([ring], dash_flag)?;
                if coords.is_empty() {
                    continue;
                }
                let header = ObjectHeader {
                    symbol,
                    object_type: object_type::LINE,
                    ..ObjectHeader::default()
                };
                objects.push(Self::with_header(header, coords, Vec::new())?);
            }
            return Ok(objects);
        }
        Self::single(object, symbol, drawn, dash_flag, font_size, index, warnings)
            .map(|one| vec![one])
    }

    fn single(
        object: &MapObject,
        symbol: i32,
        drawn: Drawn,
        dash_flag: i32,
        font_size: f64,
        index: usize,
        warnings: &mut Warnings,
    ) -> Result<Self> {
        let mut header = ObjectHeader {
            symbol,
            ..ObjectHeader::default()
        };
        let mut text = Vec::new();
        let coords = match object {
            MapObject::Point(point) => {
                header.object_type = object_type::POINT;
                header.angle = to_tenths_of_degree(point.rotation);
                vec![FileCoord::from_coord(point.geometry().0)?]
            }
            MapObject::Line(line) => {
                header.object_type = path_type(drawn);
                write_rings([line.geometry()], dash_flag)?
            }
            MapObject::Area(area) => {
                header.object_type = path_type(drawn);
                header.angle = to_tenths_of_degree(area.pattern_rotation.rotation);
                write_polygon(area.geometry(), dash_flag)?
            }
            MapObject::Text(object) => {
                header.angle = to_tenths_of_degree(object.rotation);
                text = text_units(&object.text, index, warnings);
                let (object_type, coords) = text_coords(object, font_size)?;
                header.object_type = object_type;
                coords
            }
        };
        Self::with_header(header, coords, text)
    }

    /// An object of `header` with `coords` and `text`, counted in its header.
    fn with_header(
        mut header: ObjectHeader,
        coords: Vec<FileCoord>,
        text: Vec<u16>,
    ) -> Result<Self> {
        header.coord_count = u32::try_from(coords.len())
            .map_err(|_too_many| Error::TooMany("object coordinates"))?;
        header.text_length = u16::try_from(text.len() / 4)
            .map_err(|_too_long| Error::TooMany("characters in a text"))?;
        Ok(Self {
            header,
            coords,
            text,
        })
    }

    /// The object's index entry and bytes. The entry's box is grown by
    /// `extent`, in millimetres, for what the symbol draws beyond the
    /// coordinates, as OCAD writes it.
    fn item(&self, extent: f64) -> Result<(ObjectIndexEntry, Vec<u8>)> {
        let mut bytes = Cursor::new(Vec::new());
        self.header.write_le(&mut bytes)?;
        for coord in &self.coords {
            coord.write_le(&mut bytes)?;
        }
        for unit in &self.text {
            unit.write_le(&mut bytes)?;
        }
        let bytes = bytes.into_inner();
        let mut bottom_left = Coord {
            x: f64::INFINITY,
            y: f64::INFINITY,
        };
        let mut top_right = Coord {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        };
        for coord in &self.coords {
            let coord = coord.coord();
            bottom_left = Coord {
                x: bottom_left.x.min(coord.x),
                y: bottom_left.y.min(coord.y),
            };
            top_right = Coord {
                x: top_right.x.max(coord.x),
                y: top_right.y.max(coord.y),
            };
        }
        let (bottom_left, top_right) = if self.coords.is_empty() {
            (FileCoord::default(), FileCoord::default())
        } else {
            // A symbol reaching past the coordinate range only clips its box.
            let grown = |coord: Coord, by: f64| {
                FileCoord::from_coord(Coord {
                    x: coord.x + by,
                    y: coord.y + by,
                })
                .or_else(|_out_of_range| FileCoord::from_coord(coord))
            };
            (grown(bottom_left, -extent)?, grown(top_right, extent)?)
        };
        let entry = ObjectIndexEntry {
            bottom_left,
            top_right,
            // Mapper writes the object's padded size in bytes, which OCAD
            // needs, though the documentation says coordinates.
            size: u32::try_from(bytes.len().next_multiple_of(8))
                .map_err(|_too_large| Error::TooMany("object data"))?,
            symbol: self.header.symbol,
            object_type: self.header.object_type,
            status: ObjectIndexEntry::NORMAL,
            ..ObjectIndexEntry::default()
        };
        Ok((entry, bytes))
    }
}

/// The object type of a path drawn as `drawn`.
fn path_type(drawn: Drawn) -> u8 {
    match drawn {
        Drawn::Area => object_type::AREA,
        Drawn::Point | Drawn::Line | Drawn::Text => object_type::LINE,
    }
}

/// `text` as the file stores it: UTF-16 with CR LF line breaks, a leading
/// line break doubled since reading drops one, zero-terminated and padded to
/// whole chunks.
fn text_units(text: &str, index: usize, warnings: &mut Warnings) -> Vec<u16> {
    let mut text = text.to_owned();
    if text.starts_with('\n') {
        text.insert(0, '\n');
    }
    let text = text.replace('\n', "\r\n");
    let max_units = MAX_TEXT_BYTES / 2 - 1;
    let mut units: Vec<u16> = text.encode_utf16().collect();
    if units.len() > max_units {
        warnings.push(Warning::Object {
            index,
            issue: "the text is longer than OCAD holds and was cut",
        });
        units.truncate(max_units);
        // Do not leave half a surrogate pair.
        if units
            .last()
            .is_some_and(|unit| (0xd800..0xdc00).contains(unit))
        {
            units.pop();
        }
    }
    units.push(0);
    units.resize(units.len().next_multiple_of(TEXT_CHUNK_BYTES / 2), 0);
    units
}

/// The object type and coordinates of a text object. Box text is its four
/// corners counter-clockwise from the bottom left. Point text is its anchor
/// followed by the corners of a box around it, which is only estimated from
/// the font size: the map model knows nothing of font metrics.
fn text_coords(text: &TextObject, font_size: f64) -> Result<(u8, Vec<FileCoord>)> {
    let rotate = |offset: Coord| {
        let (sin, cos) = text.rotation.sin_cos();
        Coord {
            x: offset.x * cos - offset.y * sin,
            y: offset.x * sin + offset.y * cos,
        }
    };
    let corners = |center: Coord, width: f64, height: f64| {
        [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)].map(|(x, y)| {
            center
                + rotate(Coord {
                    x: x * width / 2.,
                    y: y * height / 2.,
                })
        })
    };
    match text.geometry() {
        TextGeometry::WrapBox(wrap) => {
            let coords = corners(wrap.anchor, wrap.width.get(), wrap.height.get())
                .into_iter()
                .map(FileCoord::from_coord)
                .collect::<Result<Vec<_>>>()?;
            Ok((object_type::FORMATTED_TEXT, coords))
        }
        TextGeometry::SingleAnchor(anchor) => {
            let lines: Vec<&str> = text.text.lines().collect();
            let longest = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0);
            let width = longest as f64 * CHARACTER_WIDTH * font_size;
            let height = lines.len().max(1) as f64 * LINE_HEIGHT * font_size;
            let center = Coord {
                x: match text.h_align {
                    HorizontalAlign::Left => width / 2.,
                    HorizontalAlign::HCenter => 0.,
                    HorizontalAlign::Right => -width / 2.,
                },
                y: match text.v_align {
                    VerticalAlign::Top => -height / 2.,
                    VerticalAlign::VCenter => 0.,
                    VerticalAlign::Baseline | VerticalAlign::Bottom => height / 2.,
                },
            };
            let mut coords = vec![FileCoord::from_coord(*anchor)?];
            for corner in corners(Coord::zero(), width, height) {
                let corner = *anchor + rotate(center) + corner;
                coords.push(FileCoord::from_coord(corner)?);
            }
            Ok((object_type::UNFORMATTED_TEXT, coords))
        }
    }
}
