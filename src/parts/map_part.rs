use std::collections::HashSet;

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::objects::{MapObject, ObjectId};
use crate::symbols::{SymbolId, SymbolSet};
use crate::utils::try_get_attr;
use crate::{Collection, Error, OmapSection, Removed, Result};

/// The objects of a [`MapPart`], in file order.
///
/// Empty line and area objects are kept for further editing, but are omitted
/// when the map is written. See [`Collection`] for everything a set does.
pub type MapObjects = Collection<ObjectId, MapObject>;

/// A map part (layer) containing objects.
///
/// Cloning a part is cheap: the copies share every object until one of them
/// changes it, and a change copies only the object it touches.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapPart {
    /// The name of this map part.
    pub name: String,
    /// The objects in this part, in file order.
    pub objects: MapObjects,
}

impl MapPart {
    /// Create a new empty map part with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            objects: MapObjects::new(),
        }
    }

    /// Adds every object of `other` to the end of this part, under new
    /// handles, sharing them with `other`.
    pub(super) fn merge(&mut self, other: &Self) {
        self.objects.extend_shared(&other.objects);
    }
}

impl MapObjects {
    /// Remove every object drawn with `symbol`. They come back in file order:
    /// [`Collection::restore`] on each in that order puts them all back.
    pub fn remove_by_symbol(&mut self, symbol: SymbolId) -> Vec<Removed<ObjectId, MapObject>> {
        self.remove_where(|object| object.symbol() == Some(symbol))
    }

    /// The objects drawn with `symbol`.
    pub fn by_symbol(&self, symbol: SymbolId) -> impl Iterator<Item = &MapObject> {
        self.values()
            .filter(move |object| object.symbol() == Some(symbol))
    }

    /// The objects drawn with `symbol`, to change. Only those are copied out of
    /// a shared part.
    pub fn by_symbol_mut(&mut self, symbol: SymbolId) -> impl Iterator<Item = &mut MapObject> {
        self.values_mut_where(move |object| object.symbol() == Some(symbol))
    }

    /// The number of distinct symbols these objects are drawn with.
    pub fn num_symbols(&self) -> usize {
        self.values()
            .map(MapObject::symbol)
            .collect::<HashSet<_>>()
            .len()
    }
}

impl MapPart {
    pub(super) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        symbols: &SymbolSet,
    ) -> Result<Self> {
        let name = try_get_attr(element, "name")
            .ok()
            .flatten()
            .unwrap_or(String::new());

        let mut objects = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "object") {
                        let object = MapObject::parse(reader, &bytes_start, symbols, false)?;
                        if object.geometry_is_empty() {
                            continue;
                        }
                        objects.push(object);
                    }
                }
                Event::End(bytes_end) => {
                    if matches!(bytes_end.local_name().as_ref(), "part") {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::MapPart));
                }
                _ => (),
            }
        }

        Ok(Self {
            name,
            objects: objects.into_iter().collect(),
        })
    }

    pub(super) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbols: &SymbolSet,
    ) -> Result<()> {
        let object_count = self
            .objects
            .values()
            .filter(|object| !object.geometry_is_empty())
            .count();

        writer.write_event(Event::Start(
            BytesStart::new("part").with_attributes([("name", self.name.as_str())]),
        ))?;
        writer.write_event(Event::Start(
            BytesStart::new("objects")
                .with_attributes([("count", object_count.to_string().as_str())]),
        ))?;
        writer.get_mut().write_all(b"\n")?;
        for object in self.objects.values() {
            if object.geometry_is_empty() {
                continue;
            }
            object.write(writer, symbols)?;
            writer.get_mut().write_all(b"\n")?;
        }
        writer.write_event(Event::End(BytesEnd::new("objects")))?;
        writer.write_event(Event::End(BytesEnd::new("part")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use geo_types::{LineString, coord};
    use quick_xml::{Reader, Writer, events::Event};

    use super::MapPart;
    use crate::{
        Result,
        objects::{LineObject, MapObject},
        symbols::SymbolSet,
    };

    fn empty_symbol_set() -> SymbolSet {
        SymbolSet::new()
    }

    #[test]
    fn parsed_objects_without_geometry_are_ignored() -> Result<()> {
        let mut reader = Reader::from_str(
            r#"<part name="Map"><objects count="1"><object type="1"><coords count="0"></coords></object></objects></part>"#,
        );
        let Event::Start(start) = reader.read_event()? else {
            panic!("expected part start");
        };

        let part = MapPart::parse(&mut reader, &start, &empty_symbol_set())?;

        assert!(part.objects.is_empty());
        Ok(())
    }

    #[test]
    fn empty_objects_are_retained_in_memory_and_skipped_on_write() -> Result<()> {
        let symbol = None;
        let mut part = MapPart::new("Map");
        let _id = part
            .objects
            .push(LineObject::new(symbol, LineString::new(Vec::new())));
        assert_eq!(part.objects.len(), 1);

        let _id = part.objects.push(LineObject::new(
            symbol,
            LineString::new(vec![coord! { x: 0., y: 0. }, coord! { x: 1., y: 0. }]),
        ));
        let Some(MapObject::Line(line)) = part.objects.values_mut().nth(1) else {
            panic!("expected line object");
        };
        *line.geometry_mut() = crate::objects::BezierPath::empty();
        assert_eq!(part.objects.len(), 2);

        let mut writer = Writer::new(Vec::new());
        part.write(&mut writer, &empty_symbol_set())?;
        let output = String::from_utf8(writer.into_inner())?;

        assert!(output.contains(r#"<objects count="0">"#));
        assert!(!output.contains("<object "));
        Ok(())
    }
}
