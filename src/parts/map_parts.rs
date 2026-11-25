use super::{MapPart, PartId};
use crate::{Collection, Error, Result};

/// The map's parts (layers), in file order. See [`Collection`] for everything
/// a set does.
pub type MapParts = Collection<PartId, MapPart>;

impl MapParts {
    /// A collection holding one empty part named "Map".
    pub fn new_with_default_part() -> Self {
        std::iter::once(MapPart::new("Map")).collect()
    }

    /// Merge all map parts into the first one, which keeps its handle. The
    /// moved objects get new handles in it; their old ones no longer resolve.
    /// The merged parts' handles must not be restored: their objects now live
    /// in the first part.
    /// If `new_name` is some, then the new name is applied, else the name of the first map part is kept
    pub fn merge_all_parts(&mut self, new_name: Option<String>) {
        while let Some(removed) = self.remove_at(1) {
            if let Some(first) = self.at_mut(0) {
                first.merge(removed.value());
            }
        }

        if let Some(name) = new_name
            && let Some(first) = self.at_mut(0)
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
            .remove_at(part_2_index)
            .ok_or(Error::MapPartMergeError)?;
        if part_1_index > part_2_index {
            part_1_index -= 1;
        }
        self.at_mut(part_1_index)
            .ok_or(Error::MapPartMergeError)?
            .merge(part2.value());
        Ok(())
    }

    /// The first part with this name. Case sensitive.
    pub fn by_name(&self, name: &str) -> Option<&MapPart> {
        self.values().find(|p| p.name.as_str() == name)
    }

    /// The first part with this name, to change. Case sensitive.
    pub fn by_name_mut(&mut self, name: &str) -> Option<&mut MapPart> {
        let index = self.values().position(|p| p.name.as_str() == name)?;
        self.at_mut(index)
    }
}
