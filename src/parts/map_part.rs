use std::collections::HashSet;

use crate::objects::{MapObject, ObjectId};
use crate::symbols::SymbolId;
use crate::{Collection, Removed};

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
