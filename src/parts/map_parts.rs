use std::sync::Arc;

use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, Event},
};

use super::{MapPart, PartId, PartKey};
use crate::arena::Arena;
use crate::{Error, OmapSection, Result, symbols::SymbolSet};

/// An ordered collection of map parts (layers).
///
/// Parts are named by [`PartId`] handles as well as by position. Cloning is
/// cheap: the copies share every part until one of them changes it.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapParts(Arena<MapPart, PartKey>);

/// Iterator over the parts of [`MapParts`], in order.
#[derive(Clone, Debug)]
pub struct Iter<'a>(std::slice::Iter<'a, (PartKey, Arc<MapPart>)>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a MapPart;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, part)| part.as_ref())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, part)| part.as_ref())
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl std::iter::FusedIterator for Iter<'_> {}

/// Mutable iterator over the parts of [`MapParts`], in order. A part shared
/// with a copy of the map is copied when the iterator reaches it.
#[derive(Debug)]
pub struct IterMut<'a>(std::slice::IterMut<'a, (PartKey, Arc<MapPart>)>);

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut MapPart;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, part)| Arc::make_mut(part))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for IterMut<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, part)| Arc::make_mut(part))
    }
}

impl ExactSizeIterator for IterMut<'_> {}

impl std::iter::FusedIterator for IterMut<'_> {}

/// Owning iterator over the parts of [`MapParts`], in order.
#[derive(Debug)]
pub struct IntoIter(std::vec::IntoIter<MapPart>);

impl Iterator for IntoIter {
    type Item = MapPart;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl ExactSizeIterator for IntoIter {}

impl std::iter::FusedIterator for IntoIter {}

impl MapParts {
    /// Create a new empty [`MapParts`] object
    pub fn new() -> Self {
        Self(Arena::new())
    }

    /// Create a new [`MapParts`] object with a [`MapPart`] named "Map"
    pub fn new_with_default_part() -> Self {
        let mut parts = Self::new();
        let _id = parts.push(MapPart::new("Map"));
        parts
    }

    /// Add a new map part to the end of the collection and return its handle.
    pub fn push(&mut self, part: MapPart) -> PartId {
        PartId(self.0.push(part))
    }

    /// Get the number of map parts.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are no map parts.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Merge all map parts into the first one, which keeps its handle. The
    /// moved objects get new handles in it; their old ones no longer resolve.
    /// The merged parts' handles must not be restored: their objects now live
    /// in the first part.
    /// If `new_name` is some, then the new name is applied, else the name of the first map part is kept
    pub fn merge_all_parts(&mut self, new_name: Option<String>) {
        while let Some(part) = self.0.remove_at(1) {
            if let Some(first) = self.0.get_at_mut(0) {
                first.merge(Arc::unwrap_or_clone(part));
            }
        }

        if let Some(name) = new_name
            && let Some(first) = self.0.get_at_mut(0)
        {
            first.name = name;
        }
    }

    /// Merge two of the map parts.
    /// The second part is merged into the first part. The name and handle of the first part are kept;
    /// the moved objects get new handles in it and their old ones no longer resolve. The second
    /// part's handle must not be restored: its objects now live in the first part.
    /// The order of parts is also kept
    ///
    /// # Errors
    ///
    /// Returns [`Error::MapPartMergeError`] if either index is out of bounds or
    /// both indices identify the same part.
    pub fn merge_two_parts(&mut self, mut part_1_index: usize, part_2_index: usize) -> Result<()> {
        if part_1_index >= self.len() || part_2_index >= self.len() || part_1_index == part_2_index
        {
            return Err(Error::MapPartMergeError);
        }
        let part2 = self
            .0
            .remove_at(part_2_index)
            .ok_or(Error::MapPartMergeError)?;
        if part_1_index > part_2_index {
            part_1_index -= 1;
        }
        self.0
            .get_at_mut(part_1_index)
            .ok_or(Error::MapPartMergeError)?
            .merge(Arc::unwrap_or_clone(part2));
        Ok(())
    }

    /// Remove the map part at the given index, returning its handle and the
    /// part as the collection shared it, or `None` if out of bounds. Its handle
    /// stays reserved for [`MapParts::restore`].
    pub fn remove(&mut self, index: usize) -> Option<(PartId, Arc<MapPart>)> {
        let id = self.0.id_at(index)?;
        Some((PartId(id), self.0.remove_at(index)?))
    }

    /// Remove the map part `id` names. Its handle stays reserved for
    /// [`MapParts::restore`].
    pub fn remove_by_id(&mut self, id: PartId) -> Option<Arc<MapPart>> {
        self.0.remove(id.0)
    }

    /// Put a removed part back under its own handle at `index`.
    ///
    /// # Errors
    ///
    /// Hands `part` back if `id` is not a removed handle of this collection, or
    /// `index > self.len()`. A handle does not record what issued it, so one
    /// from elsewhere naming a slot removed here is taken.
    pub fn restore(
        &mut self,
        id: PartId,
        part: Arc<MapPart>,
        index: usize,
    ) -> std::result::Result<(), Arc<MapPart>> {
        self.0.restore(id.0, part, index)
    }

    /// Case sensitive
    pub fn by_name(&self, name: &str) -> Option<&MapPart> {
        self.0.values().find(|p| p.name.as_str() == name)
    }

    /// Case sensitive
    pub fn by_name_mut(&mut self, name: &str) -> Option<&mut MapPart> {
        let index = self.0.values().position(|p| p.name.as_str() == name)?;
        self.0.get_at_mut(index)
    }

    /// Get a map part by its index.
    pub fn get(&self, index: usize) -> Option<&MapPart> {
        self.0.get_at(index)
    }

    /// Get a mutable reference to a map part by its index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut MapPart> {
        self.0.get_at_mut(index)
    }

    /// Get the map part `id` names.
    pub fn get_by_id(&self, id: PartId) -> Option<&MapPart> {
        self.0.get(id.0)
    }

    /// Get the map part `id` names, to change.
    pub fn get_by_id_mut(&mut self, id: PartId) -> Option<&mut MapPart> {
        self.0.get_mut(id.0)
    }

    /// The shared part behind a handle; a copy of the map holds the same
    /// [`Arc`] until either side changes that part.
    pub fn get_arc(&self, id: PartId) -> Option<&Arc<MapPart>> {
        self.0.get_arc(id.0)
    }

    /// The handle of the part at `index`.
    pub fn id_at(&self, index: usize) -> Option<PartId> {
        self.0.id_at(index).map(PartId)
    }

    /// The index of the part `id` names.
    pub fn index_of(&self, id: PartId) -> Option<usize> {
        self.0.position(id.0)
    }

    /// The handle of every part, in order.
    pub fn ids(&self) -> impl Iterator<Item = PartId> {
        self.0.iter().map(|(key, _)| PartId(key))
    }

    /// Iterate over map parts.
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.0.entries())
    }

    /// Iterate mutably over map parts.
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut(self.0.entries_mut())
    }
}

impl IntoIterator for MapParts {
    type Item = MapPart;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let parts: Vec<MapPart> = self.0.into_values().map(Arc::unwrap_or_clone).collect();
        IntoIter(parts.into_iter())
    }
}

impl<'a> IntoIterator for &'a MapParts {
    type Item = &'a MapPart;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut MapParts {
    type Item = &'a mut MapPart;
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl MapParts {
    pub(crate) fn write<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        symbols: &SymbolSet,
    ) -> Result<()> {
        writer.write_event(Event::Start(BytesStart::new("parts").with_attributes([
            ("count", self.len().to_string().as_str()),
            ("current", "0"),
        ])))?;
        writer.get_mut().write_all(b"\n")?;

        for part in self {
            part.write(writer, symbols)?;
            writer.get_mut().write_all(b"\n")?;
        }

        writer.write_event(Event::End(BytesEnd::new("parts")))?;
        Ok(())
    }

    pub(crate) fn parse<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        symbols: &SymbolSet,
    ) -> Result<Self> {
        let mut parts = Self::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(bytes_start) => {
                    if matches!(bytes_start.local_name().as_ref(), "part") {
                        let _id = parts.push(MapPart::parse(reader, &bytes_start, symbols)?);
                    }
                }
                Event::End(_) => break,
                Event::Eof => {
                    return Err(Error::UnexpectedEof(OmapSection::Parts));
                }
                _ => (),
            }
        }

        Ok(parts)
    }
}
