//! Objects: the object index and the objects it lists.
//!
//! The file has no map parts, so every object goes into the map's one part,
//! as Mapper reads it. An object's symbol decides what kind of object it
//! becomes; an object without a symbol is placed by the file's object type.

use std::io::Cursor;

use binrw::{BinRead as _, binrw};
use geo_types::{Coord, Point};

use crate::objects::{
    AreaObject, HorizontalAlign, LineObject, MapObject, PointObject, TextGeometry, TextObject,
    VerticalAlign, WrapBox,
};
use crate::ocd::Result;
use crate::ocd::file::{IndexEntry, OcdFile};
use crate::ocd::geometry::{read_polygon, read_rings};
use crate::ocd::symbols::SymbolNumbers;
use crate::ocd::units::{FileCoord, radians};
use crate::ocd::warning::{Warning, Warnings, push_once};
use crate::parts::MapObjects;
use crate::symbols::{Symbol, SymbolId, SymbolSet};
use crate::{Entry, NonNegativeF64};

/// An object index entry.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct ObjectIndexEntry {
    pub(in crate::ocd) bottom_left: FileCoord,
    pub(in crate::ocd) top_right: FileCoord,
    pub(in crate::ocd) position: u32,
    /// The room the object has in the file. OCAD's documentation gives it as
    /// a number of coordinates, but Mapper writes bytes and OCAD reads that;
    /// the reader does not rely on it.
    pub(in crate::ocd) size: u32,
    pub(in crate::ocd) symbol: i32,
    pub(in crate::ocd) object_type: u8,
    pub(in crate::ocd) encryption: u8,
    pub(in crate::ocd) status: u8,
    pub(in crate::ocd) view_type: u8,
    pub(in crate::ocd) color: i16,
    pub(in crate::ocd) group: i16,
    pub(in crate::ocd) layer: i16,
    pub(in crate::ocd) dataset_hash: u8,
    pub(in crate::ocd) key_hash: u8,
}

impl ObjectIndexEntry {
    /// Status: deleted.
    pub(in crate::ocd) const DELETED: u8 = 0;
    /// Status: normal.
    pub(in crate::ocd) const NORMAL: u8 = 1;
    /// Status: hidden.
    pub(in crate::ocd) const HIDDEN: u8 = 2;
    /// Status: deleted, kept for undo.
    pub(in crate::ocd) const DELETED_FOR_UNDO: u8 = 3;

    /// View type: a course setting preview, modified or not, or a temporary
    /// object; none of them is part of the map.
    pub(in crate::ocd) const NOT_MAP_VIEW_TYPES: std::ops::RangeInclusive<u8> = 2..=4;
}

impl IndexEntry for ObjectIndexEntry {
    const SIZE: usize = 40;

    fn position(&self) -> u32 {
        self.position
    }

    fn set_position(&mut self, position: u32) {
        self.position = position;
    }
}

/// An object's header; its coordinates, then its text, follow it.
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Default)]
pub(in crate::ocd) struct ObjectHeader {
    pub(in crate::ocd) symbol: i32,
    pub(in crate::ocd) object_type: u8,
    pub(in crate::ocd) customer: u8,
    /// Tenths of a degree, counter-clockwise.
    pub(in crate::ocd) angle: i16,
    pub(in crate::ocd) color: i32,
    pub(in crate::ocd) line_width: u16,
    pub(in crate::ocd) flags: u16,
    pub(in crate::ocd) server_object_id: u32,
    /// 1/256 mm.
    pub(in crate::ocd) height: u32,
    pub(in crate::ocd) creation_date: f64,
    pub(in crate::ocd) multi_representation_id: u32,
    pub(in crate::ocd) modification_date: f64,
    pub(in crate::ocd) coord_count: u32,
    /// The text's length, in 8-byte units.
    pub(in crate::ocd) text_length: u16,
    pub(in crate::ocd) object_string_length: u16,
    pub(in crate::ocd) database_link_length: u16,
    pub(in crate::ocd) object_string_type: u8,
    pub(in crate::ocd) reserved: u8,
}

/// The object type values.
pub(in crate::ocd) mod object_type {
    pub(in crate::ocd) const POINT: u8 = 1;
    pub(in crate::ocd) const LINE: u8 = 2;
    pub(in crate::ocd) const AREA: u8 = 3;
    pub(in crate::ocd) const UNFORMATTED_TEXT: u8 = 4;
    pub(in crate::ocd) const FORMATTED_TEXT: u8 = 5;
    pub(in crate::ocd) const LINE_TEXT: u8 = 6;
    pub(in crate::ocd) const RECTANGLE: u8 = 7;
}

/// What kind of map object a file object becomes.
#[derive(Clone, Copy)]
enum Kind {
    Point,
    Line,
    Area,
    Text,
    Rectangle,
}

/// An object as the file stores it.
struct FileObject {
    header: ObjectHeader,
    coords: Vec<FileCoord>,
    text: String,
}

/// Read every object in `file` that is not deleted, in file order.
pub(in crate::ocd) fn read_objects(
    file: &OcdFile,
    symbols: &SymbolSet,
    numbers: &SymbolNumbers,
    warnings: &mut Warnings,
) -> Result<MapObjects> {
    let entries: Vec<ObjectIndexEntry> = file.index(file.header().first_object_block)?;
    let mut objects = MapObjects::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.symbol == 0
            || entry.status == ObjectIndexEntry::DELETED
            || entry.status == ObjectIndexEntry::DELETED_FOR_UNDO
            || ObjectIndexEntry::NOT_MAP_VIEW_TYPES.contains(&entry.view_type)
        {
            continue;
        }
        if entry.encryption != 0 {
            warnings.push(Warning::Object {
                index,
                issue: "the object is encrypted and was skipped",
            });
            continue;
        }
        if entry.status == ObjectIndexEntry::HIDDEN {
            push_once(warnings, Warning::Map("hidden objects are read as visible"));
        }
        let object = read_object(file, entry.position)?;
        let mut reader = ObjectReader {
            symbols,
            numbers,
            warnings,
            index,
        };
        for map_object in reader.map_objects(object) {
            objects.push(map_object);
        }
    }
    Ok(objects)
}

fn read_object(file: &OcdFile, position: u32) -> Result<FileObject> {
    let mut cursor = Cursor::new(file.bytes_from(position)?);
    let header = ObjectHeader::read_le(&mut cursor)?;
    let coords = (0..header.coord_count)
        .map(|_| FileCoord::read_le(&mut cursor))
        .collect::<binrw::BinResult<Vec<_>>>()?;
    let units = (0..usize::from(header.text_length) * 4)
        .map(|_| u16::read_le(&mut cursor))
        .collect::<binrw::BinResult<Vec<_>>>()?;
    Ok(FileObject {
        header,
        coords,
        text: read_text(&units),
    })
}

/// Zero-terminated UTF-16 text, with OCAD's line breaks as `\n` and the
/// leading line break Mapper drops dropped too.
fn read_text(units: &[u16]) -> String {
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    let text = String::from_utf16_lossy(&units[..end]);
    let text = text.strip_prefix("\r\n").unwrap_or(&text);
    text.replace("\r\n", "\n")
}

/// The symbol `id` names as a handle of the kind `convert` asks for.
fn typed<T>(
    symbols: &SymbolSet,
    id: Option<SymbolId>,
    convert: impl FnOnce(Entry<'_, SymbolId, Symbol>) -> Option<T>,
) -> Option<T> {
    id.and_then(|id| symbols.get(id)).and_then(convert)
}

struct ObjectReader<'a> {
    symbols: &'a SymbolSet,
    numbers: &'a SymbolNumbers,
    warnings: &'a mut Warnings,
    index: usize,
}

impl ObjectReader<'_> {
    fn issue(&mut self, issue: &'static str) {
        self.warnings.push(Warning::Object {
            index: self.index,
            issue,
        });
    }

    /// The object's symbol, if it has one the file defines.
    fn symbol(&mut self, number: i32) -> Option<Entry<'_, SymbolId, Symbol>> {
        match number {
            1.. => {
                let Some(id) = self.numbers.get(number) else {
                    push_once(self.warnings, Warning::UnknownSymbol(number));
                    return None;
                };
                self.symbols.get(id)
            }
            -1 => None,
            _ => {
                push_once(
                    self.warnings,
                    Warning::Map("graphic, image and layout objects are read without a symbol"),
                );
                None
            }
        }
    }

    /// The map objects `object` becomes: usually one, one per part for a
    /// line of several parts, none if it has no geometry.
    fn map_objects(&mut self, object: FileObject) -> Vec<MapObject> {
        let symbols = self.symbols;
        let numbers = self.numbers;
        let symbol = self.symbol(object.header.symbol);
        let kind = match symbol.map(Entry::value) {
            Some(Symbol::Point(_)) => Kind::Point,
            Some(Symbol::Line(_))
                if symbol.is_some_and(|s| numbers.rectangles.contains(&s.id())) =>
            {
                Kind::Rectangle
            }
            Some(Symbol::Line(_) | Symbol::CombinedLine(_)) => Kind::Line,
            Some(Symbol::Area(_) | Symbol::CombinedArea(_)) => Kind::Area,
            Some(Symbol::Text(_)) => Kind::Text,
            None => match object.header.object_type {
                object_type::POINT => Kind::Point,
                object_type::LINE => Kind::Line,
                object_type::AREA => Kind::Area,
                object_type::UNFORMATTED_TEXT
                | object_type::FORMATTED_TEXT
                | object_type::LINE_TEXT => Kind::Text,
                object_type::RECTANGLE => Kind::Rectangle,
                _ => {
                    self.issue("the object is of an unknown type and was skipped");
                    return Vec::new();
                }
            },
        };
        let id = symbol.map(|entry| entry.id());
        let angle = radians(object.header.angle);
        let objects = match kind {
            Kind::Point => object
                .coords
                .first()
                .map(|coord| {
                    let mut point = PointObject::new(
                        typed(symbols, id, |entry| entry.as_point()),
                        Point(coord.coord()),
                    );
                    point.rotation = angle;
                    MapObject::from(point)
                })
                .into_iter()
                .collect(),
            Kind::Line => read_rings(&object.coords)
                .into_iter()
                .map(|ring| {
                    LineObject::new(typed(symbols, id, |entry| entry.as_line_path()), ring).into()
                })
                .collect(),
            Kind::Rectangle => {
                Self::rectangle(&object, typed(symbols, id, |entry| entry.as_line_path()))
            }
            Kind::Area => read_polygon(&object.coords)
                .map(|polygon| {
                    let mut area =
                        AreaObject::new(typed(symbols, id, |entry| entry.as_area_path()), polygon);
                    area.pattern_rotation.rotation = angle;
                    MapObject::from(area)
                })
                .into_iter()
                .collect(),
            Kind::Text => {
                let alignment = id.and_then(|id| numbers.text_alignment.get(&id)).copied();
                self.text(
                    object,
                    typed(symbols, id, |entry| entry.as_text()),
                    alignment,
                )
                .into_iter()
                .collect()
            }
        };
        if objects.is_empty() {
            self.issue("the object has no geometry and was skipped");
        }
        objects
    }

    /// A rectangle as the closed line of its outline: the file stores its
    /// corners counter-clockwise from the bottom left.
    fn rectangle(
        object: &FileObject,
        symbol: Option<crate::symbols::LinePathSymbolId>,
    ) -> Vec<MapObject> {
        let [bottom_left, bottom_right, top_right, top_left, ..] = object.coords.as_slice() else {
            return Vec::new();
        };
        let corners: Vec<Coord> = [top_left, top_right, bottom_right, bottom_left, top_left]
            .into_iter()
            .map(|coord| coord.coord())
            .collect();
        vec![LineObject::new(symbol, geo_types::LineString(corners)).into()]
    }

    /// A text object. Four coordinates are a box, counter-clockwise from the
    /// bottom left; otherwise the first is the anchor.
    fn text(
        &mut self,
        object: FileObject,
        symbol: Option<crate::symbols::TextSymbolId>,
        alignment: Option<(HorizontalAlign, VerticalAlign)>,
    ) -> Option<MapObject> {
        let (h_align, v_align) = alignment.unwrap_or_default();
        let (geometry, v_align) = match object.coords.as_slice() {
            [] => return None,
            [bottom_left, _, top_right, top_left] => {
                let bottom_left = bottom_left.coord();
                let top_right = top_right.coord();
                let top_left = top_left.coord();
                let distance = |a: Coord, b: Coord| (a - b).x.hypot((a - b).y);
                let geometry = TextGeometry::WrapBox(WrapBox {
                    anchor: (bottom_left + top_right) / 2.,
                    width: NonNegativeF64::clamped_from(distance(top_left, top_right)),
                    height: NonNegativeF64::clamped_from(distance(top_left, bottom_left)),
                });
                (geometry, VerticalAlign::Top)
            }
            coords => {
                if coords.len() != 5 {
                    self.issue("the text's coordinates are not a known layout; using the first");
                }
                (TextGeometry::SingleAnchor(coords[0].coord()), v_align)
            }
        };
        let mut text = TextObject::new(symbol, geometry, object.text);
        text.h_align = h_align;
        text.v_align = v_align;
        text.rotation = radians(object.header.angle);
        Some(text.into())
    }
}

mod write;

pub(in crate::ocd) use write::write_objects;
