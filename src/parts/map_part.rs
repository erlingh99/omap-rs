use std::collections::HashSet;
use std::sync::Arc;

use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};

use crate::arena::Arena;
use crate::objects::{MapObject, ObjectId, ObjectKey};
use crate::symbols::{SymbolId, SymbolSet};
use crate::utils::try_get_attr;
use crate::{Error, OmapSection, Result};

/// A map part (layer) containing objects.
///
/// Objects are kept in file order and named by [`ObjectId`] handles. Cloning a
/// part is cheap: the copies share every object until one of them changes it,
/// and a change copies only the object it touches.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapPart {
    /// The name of this map part.
    pub name: String,
    objects: Arena<MapObject, ObjectKey>,
}

impl MapPart {
    /// Create a new empty map part with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            objects: Arena::new(),
        }
    }
}

impl MapPart {
    /// Add an object to the end of the part and return its handle.
    ///
    /// Empty line and area objects are retained for further editing, but are
    /// omitted when the map is written.
    pub fn add_object(&mut self, object: impl Into<MapObject>) -> ObjectId {
        ObjectId(self.objects.push(object.into()))
    }

    /// Moves every object of `other` to the end of this part, under new handles.
    pub(super) fn merge(&mut self, other: Self) {
        for object in other.objects.into_values() {
            let _key = self.objects.push_arc(object);
        }
    }

    /// Remove every object drawn with `key` from this part. Each comes back
    /// with its handle and the index it had, in file order:
    /// [`MapPart::restore_object`] on each in that order puts them all back.
    pub fn remove(&mut self, key: SymbolId) -> Vec<(ObjectId, usize, Arc<MapObject>)> {
        self.objects
            .remove_where(|object| object.symbol() == Some(key))
            .into_iter()
            .map(|(id, index, object)| (ObjectId(id), index, object))
            .collect()
    }

    /// Get objects associated with a symbol.
    pub fn objects_by_symbol(&self, key: SymbolId) -> impl Iterator<Item = &MapObject> {
        self.objects
            .values()
            .filter(move |mo| mo.symbol() == Some(key))
    }

    /// Get a mutable reference to objects associated with a symbol.
    pub fn objects_by_symbol_mut(&mut self, key: SymbolId) -> impl Iterator<Item = &mut MapObject> {
        self.objects
            .values_mut_where(move |mo| mo.symbol() == Some(key))
    }

    /// Get the number of distinct symbols with objects in this part.
    pub fn num_symbols(&self) -> usize {
        let mut seen = HashSet::new();
        let mut count = 0;
        for obj in self.objects.values() {
            if seen.insert(obj.symbol()) {
                count += 1;
            }
        }
        count
    }

    /// Get the total number of objects in this part, including objects with
    /// empty geometry that will be omitted when the map is written.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns `true` if this part contains no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Iterate through all objects in this map-part in a flat iterator, in file order.
    pub fn iter_all_objects(&self) -> impl Iterator<Item = &MapObject> {
        self.objects.values()
    }

    /// Iterate mutably through all objects in this map-part in a flat iterator.
    ///
    /// On a part that shares its objects with a copy, this copies every object.
    pub fn iter_all_objects_mut(&mut self) -> impl Iterator<Item = &mut MapObject> {
        self.objects.values_mut()
    }

    /// Consume this map-part and get all the objects it contains, in file order.
    pub fn into_objects(self) -> Vec<MapObject> {
        self.objects
            .into_values()
            .map(Arc::unwrap_or_clone)
            .collect()
    }

    /// Every object with its handle, in file order.
    pub fn objects(&self) -> impl Iterator<Item = (ObjectId, &MapObject)> {
        self.objects
            .iter()
            .map(|(key, object)| (ObjectId(key), object))
    }

    /// Returns `true` if `id` names an object in this part.
    pub fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains(id.0)
    }

    /// The object `id` names.
    pub fn object(&self, id: ObjectId) -> Option<&MapObject> {
        self.objects.get(id.0)
    }

    /// The object `id` names, to change. Copies it first if a copy of this
    /// part shares it.
    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut MapObject> {
        self.objects.get_mut(id.0)
    }

    /// The shared object behind a handle; a copy of this part holds the same
    /// [`Arc`] until either side changes that object.
    pub fn object_arc(&self, id: ObjectId) -> Option<&Arc<MapObject>> {
        self.objects.get_arc(id.0)
    }

    /// The position of the object `id` names, in file order.
    pub fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.objects.position(id.0)
    }

    /// Remove the object `id` names. The handle stays reserved for
    /// [`MapPart::restore_object`].
    pub fn remove_object(&mut self, id: ObjectId) -> Option<Arc<MapObject>> {
        self.objects.remove(id.0)
    }

    /// Put a removed object back under its own handle at `index` in file order.
    ///
    /// # Errors
    ///
    /// Hands `object` back if `id` is not a removed handle of this part, or
    /// `index > self.len()`. A handle does not record what issued it, so one
    /// from elsewhere naming a slot removed here is taken.
    pub fn restore_object(
        &mut self,
        id: ObjectId,
        object: Arc<MapObject>,
        index: usize,
    ) -> std::result::Result<(), Arc<MapObject>> {
        self.objects.restore(id.0, object, index)
    }

    /// Put `object` in place of the object `id` names, returning the one replaced.
    pub fn replace_object(
        &mut self,
        id: ObjectId,
        object: Arc<MapObject>,
    ) -> Option<Arc<MapObject>> {
        self.objects.replace(id.0, object)
    }

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
        SymbolSet::new("Empty")
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

        assert!(part.is_empty());
        Ok(())
    }

    #[test]
    fn empty_objects_are_retained_in_memory_and_skipped_on_write() -> Result<()> {
        let symbol = None;
        let mut part = MapPart::new("Map");
        part.add_object(LineObject::new(symbol, LineString::new(Vec::new())));
        assert_eq!(part.len(), 1);

        part.add_object(LineObject::new(
            symbol,
            LineString::new(vec![coord! { x: 0., y: 0. }, coord! { x: 1., y: 0. }]),
        ));
        let Some(MapObject::Line(line)) = part.iter_all_objects_mut().nth(1) else {
            panic!("expected line object");
        };
        *line.geometry_mut() = crate::objects::BezierPath::empty();
        assert_eq!(part.len(), 2);

        let mut writer = Writer::new(Vec::new());
        part.write(&mut writer, &empty_symbol_set())?;
        let output = String::from_utf8(writer.into_inner())?;

        assert!(output.contains(r#"<objects count="0">"#));
        assert!(!output.contains("<object "));
        Ok(())
    }
}
