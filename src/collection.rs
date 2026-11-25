//! An ordered collection with stable handles, copied on write.
//!
//! Every set in a map is a [`Collection`]: the colors, the symbols, the parts
//! and each part's objects. They share one interface: look an item up by its
//! handle ([`Collection::get`]) or by its position ([`Collection::at`]), add,
//! remove and restore it, and iterate in order. What position means is the
//! set's own: a color's priority, a symbol's file index, a part's or object's
//! place in the file.
//!
//! # Handles
//!
//! A handle keeps naming the same item while others are added, removed or
//! reordered, and in every copy made after the item was added. Removing an item
//! leaves a tombstone: its handle stays reserved and is never issued again, so
//! [`Collection::restore`] can put the item back under it. Tombstones are not
//! serialized; after a serde round trip a removed handle no longer resolves.
//!
//! Handles are slot keys, not pointers, so they carry no record of which
//! collection minted them. Used against a *different* collection, one resolves
//! to whatever occupies that slot there rather than to `None`. Keep a handle
//! with the [`crate::Omap`] it came from.
//!
//! # Copy on write
//!
//! Cloning a collection copies one pointer; both copies share everything until
//! either one changes, and the first change copies the slots and the item
//! pointers. Each item sits behind its own [`Arc`](std::sync::Arc), so changing an item in a
//! shared collection copies that item alone, and every other item stays shared.
//! [`Collection::values_mut`] over a shared collection copies every item it
//! yields.

use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use slotmap::SlotMap;

use crate::{Error, Result};

/// A handle naming one item of a [`Collection`].
///
/// Implemented by [`crate::colors::ColorId`], [`crate::symbols::SymbolId`],
/// [`crate::parts::PartId`] and [`crate::objects::ObjectId`]; it cannot be
/// implemented outside this crate.
pub trait Handle: Copy + sealed::Sealed {}

pub(crate) mod sealed {
    /// The slot key behind a handle.
    pub trait Sealed {
        type Key: slotmap::Key;

        fn key(self) -> Self::Key;

        fn from_key(key: Self::Key) -> Self;
    }
}

/// Implements [`Handle`] for a newtype over a slot key.
macro_rules! impl_handle {
    ($handle:ty, $key:ty) => {
        impl $crate::collection::sealed::Sealed for $handle {
            type Key = $key;

            fn key(self) -> $key {
                self.0
            }

            fn from_key(key: $key) -> Self {
                Self(key)
            }
        }

        impl $crate::collection::Handle for $handle {}
    };
}
pub(crate) use impl_handle;

type Key<Id> = <Id as sealed::Sealed>::Key;

/// An item of a [`Collection`] together with the handle that names it.
///
/// What every lookup hands back. It dereferences to the item for reading. The
/// handle is [`Copy`] and outlives this borrow of the collection, so take it
/// with [`Entry::id`] before changing the collection.
#[derive(Debug)]
pub struct Entry<'a, Id, T> {
    id: Id,
    value: &'a T,
}

impl<Id: Copy, T> Clone for Entry<'_, Id, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Id: Copy, T> Copy for Entry<'_, Id, T> {}

impl<'a, Id: Copy, T> Entry<'a, Id, T> {
    /// The handle naming this item.
    pub fn id(self) -> Id {
        self.id
    }

    /// The item, borrowed for as long as the collection is.
    pub fn value(self) -> &'a T {
        self.value
    }
}

impl<Id, T> Deref for Entry<'_, Id, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

/// An item taken out of a [`Collection`], with what [`Collection::restore`]
/// needs to put it back under its own handle and at its old position.
#[derive(Debug)]
pub struct Removed<Id, T> {
    id: Id,
    position: usize,
    value: Arc<T>,
}

impl<Id: Copy, T> Removed<Id, T> {
    /// The handle that named the item, reserved until it is restored.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Where the item was.
    pub fn position(&self) -> usize {
        self.position
    }

    /// The item. A copy of the collection made before the removal still
    /// shares it.
    pub fn value(&self) -> &Arc<T> {
        &self.value
    }

    /// The item, giving up the chance to restore it.
    pub fn into_value(self) -> Arc<T> {
        self.value
    }
}

/// An ordered collection with stable handles, copied on write. See the
/// [module documentation](self).
pub struct Collection<Id: Handle, T> {
    inner: Arc<Inner<Key<Id>, T>>,
    handle: PhantomData<fn() -> Id>,
}

struct Inner<K: slotmap::Key, T> {
    /// The position in `items` of every key's item; `None` is a tombstone.
    slots: SlotMap<K, Option<usize>>,
    items: Vec<(K, Arc<T>)>,
}

impl<K: slotmap::Key, T> Clone for Inner<K, T> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            items: self.items.clone(),
        }
    }
}

impl<K: slotmap::Key, T> Inner<K, T> {
    /// Points the slots of the items from `from` on at their positions.
    fn reindex(&mut self, from: usize) {
        let Self { slots, items } = self;
        for (position, (key, _)) in items.iter().enumerate().skip(from) {
            if let Some(slot) = slots.get_mut(*key) {
                *slot = Some(position);
            }
        }
    }
}

impl<Id: Handle, T> Clone for Collection<Id, T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            handle: PhantomData,
        }
    }
}

impl<Id: Handle, T> Default for Collection<Id, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Handle, T: std::fmt::Debug> std::fmt::Debug for Collection<Id, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.values()).finish()
    }
}

impl<Id: Handle, T> Collection<Id, T> {
    /// An empty collection.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                slots: SlotMap::with_capacity_and_key(capacity),
                items: Vec::with_capacity(capacity),
            }),
            handle: PhantomData,
        }
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.inner.items.len()
    }

    /// Whether there are no items.
    pub fn is_empty(&self) -> bool {
        self.inner.items.is_empty()
    }

    /// The position of the item `id` names, or `None` if it was removed.
    pub fn position(&self, id: impl Into<Id>) -> Option<usize> {
        *self.inner.slots.get(id.into().key())?
    }

    /// Whether `id` names an item of this collection.
    pub fn contains(&self, id: impl Into<Id>) -> bool {
        self.position(id).is_some()
    }

    /// The item `id` names, or `None` if it was removed.
    pub fn get(&self, id: impl Into<Id>) -> Option<Entry<'_, Id, T>> {
        self.at(self.position(id)?)
    }

    /// The item at `position`.
    pub fn at(&self, position: usize) -> Option<Entry<'_, Id, T>> {
        let (key, value) = self.inner.items.get(position)?;
        Some(Entry {
            id: Id::from_key(*key),
            value,
        })
    }

    /// The shared item `id` names; a copy of this collection holds the same
    /// [`Arc`] until either side changes that item.
    pub fn get_arc(&self, id: impl Into<Id>) -> Option<&Arc<T>> {
        let (_, item) = self.inner.items.get(self.position(id)?)?;
        Some(item)
    }

    /// The slots and items to change, copied first if another collection
    /// shares them.
    fn inner_mut(&mut self) -> &mut Inner<Key<Id>, T> {
        Arc::make_mut(&mut self.inner)
    }

    /// Add `value` at the end, returning its handle.
    pub fn push(&mut self, value: impl Into<T>) -> Id {
        self.push_arc(Arc::new(value.into()))
    }

    fn push_arc(&mut self, value: Arc<T>) -> Id {
        let inner = self.inner_mut();
        let key = inner.slots.insert(Some(inner.items.len()));
        inner.items.push((key, value));
        Id::from_key(key)
    }

    /// Add `value` at `position`, shifting the items from there on back.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PositionOutOfRange`] if `position > self.len()`.
    pub fn insert(&mut self, position: usize, value: impl Into<T>) -> Result<Id> {
        let len = self.len();
        if position > len {
            return Err(Error::PositionOutOfRange { position, len });
        }
        let inner = self.inner_mut();
        let key = inner.slots.insert(Some(position));
        inner.items.insert(position, (key, Arc::new(value.into())));
        inner.reindex(position + 1);
        Ok(Id::from_key(key))
    }

    /// Take out the item `id` names, leaving `id` reserved for
    /// [`Self::restore`].
    pub fn remove(&mut self, id: impl Into<Id>) -> Option<Removed<Id, T>> {
        let position = self.position(id)?;
        self.remove_at(position)
    }

    /// Take out the item at `position`, leaving its handle reserved for
    /// [`Self::restore`].
    pub fn remove_at(&mut self, position: usize) -> Option<Removed<Id, T>> {
        if position >= self.len() {
            return None;
        }
        let inner = self.inner_mut();
        let (key, value) = inner.items.remove(position);
        if let Some(slot) = inner.slots.get_mut(key) {
            *slot = None;
        }
        inner.reindex(position);
        Some(Removed {
            id: Id::from_key(key),
            position,
            value,
        })
    }

    /// Put a removed item back under its own handle, at the position it had.
    /// Everything that still names that handle resolves to it again.
    ///
    /// Restoring several removals in the reverse order they were made, or in
    /// the order one call returned them, puts each back where it was.
    ///
    /// # Errors
    ///
    /// Hands `removed` back if its handle is not a removed handle of this
    /// collection, or its position is now past the end. A handle does not
    /// record what issued it, so one from elsewhere naming a slot removed here
    /// is taken.
    pub fn restore(&mut self, removed: Removed<Id, T>) -> std::result::Result<(), Removed<Id, T>> {
        let key = removed.id.key();
        let is_tombstone = matches!(self.inner.slots.get(key), Some(None));
        if !is_tombstone || removed.position > self.len() {
            return Err(removed);
        }
        let inner = self.inner_mut();
        inner.items.insert(removed.position, (key, removed.value));
        inner.reindex(removed.position);
        Ok(())
    }

    /// Put `value` in place of the item `id` names, returning that item.
    pub fn replace(&mut self, id: impl Into<Id>, value: Arc<T>) -> Option<Arc<T>> {
        let position = self.position(id)?;
        let (_, item) = self.inner_mut().items.get_mut(position)?;
        Some(std::mem::replace(item, value))
    }

    /// Exchange the items at two positions. Every handle stays valid.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PositionOutOfRange`] if either position is past the
    /// end.
    pub fn swap(&mut self, first: usize, second: usize) -> Result<()> {
        let len = self.len();
        if let Some(position) = [first, second].into_iter().find(|&p| p >= len) {
            return Err(Error::PositionOutOfRange { position, len });
        }
        let inner = self.inner_mut();
        inner.items.swap(first, second);
        for position in [first, second] {
            let key = inner.items[position].0;
            if let Some(slot) = inner.slots.get_mut(key) {
                *slot = Some(position);
            }
        }
        Ok(())
    }

    /// Reorder the items by `key`, keeping equal items in their order. Every
    /// handle stays valid.
    pub fn sort_by_key<S: Ord>(&mut self, key: impl Fn(&T) -> S) {
        let inner = self.inner_mut();
        inner.items.sort_by_key(|(_, item)| key(item));
        inner.reindex(0);
    }

    /// Every item with its handle, in order.
    pub fn iter(&self) -> Iter<'_, Id, T> {
        Iter(self.inner.items.iter(), PhantomData)
    }

    /// Every item, in order.
    pub fn values(&self) -> Values<'_, Id, T> {
        Values(self.inner.items.iter())
    }

    /// The handle of every item, in order.
    pub fn ids(&self) -> impl Iterator<Item = Id> {
        self.inner.items.iter().map(|(key, _)| Id::from_key(*key))
    }

    /// Adds every item of `other` to the end, under new handles, sharing each
    /// with `other`.
    pub(crate) fn extend_shared(&mut self, other: &Self) {
        let inner = self.inner_mut();
        inner.items.reserve(other.len());
        inner.slots.reserve(other.len());
        for (_, item) in &other.inner.items {
            let key = inner.slots.insert(Some(inner.items.len()));
            inner.items.push((key, Arc::clone(item)));
        }
    }

    /// Remove every item `matches`, in order, leaving their handles reserved
    /// for [`Self::restore`]. Restoring them in the order returned puts each
    /// back where it was.
    pub(crate) fn remove_where(&mut self, matches: impl Fn(&T) -> bool) -> Vec<Removed<Id, T>> {
        let Some(first) = self.values().position(&matches) else {
            return Vec::new();
        };
        let inner = self.inner_mut();
        let mut position = first;
        let mut positions = Vec::new();
        let taken: Vec<_> = inner
            .items
            .extract_if(first.., |(_, item)| {
                let hit = matches(item);
                if hit {
                    positions.push(position);
                }
                position += 1;
                hit
            })
            .collect();
        for (key, _) in &taken {
            if let Some(slot) = inner.slots.get_mut(*key) {
                *slot = None;
            }
        }
        inner.reindex(first);
        taken
            .into_iter()
            .zip(positions)
            .map(|((key, value), position)| Removed {
                id: Id::from_key(key),
                position,
                value,
            })
            .collect()
    }
}

impl<Id: Handle, T: Clone> Collection<Id, T> {
    /// The item `id` names, to change. Copies it first if a copy of this
    /// collection shares it.
    pub fn get_mut(&mut self, id: impl Into<Id>) -> Option<&mut T> {
        let position = self.position(id)?;
        self.at_mut(position)
    }

    /// The item at `position`, to change. Copies it first if a copy of this
    /// collection shares it.
    pub fn at_mut(&mut self, position: usize) -> Option<&mut T> {
        if position >= self.len() {
            return None;
        }
        let (_, item) = self.inner_mut().items.get_mut(position)?;
        Some(Arc::make_mut(item))
    }

    /// Every item, in order, to change. Each is copied out of a shared
    /// collection as the iterator reaches it.
    pub fn values_mut(&mut self) -> ValuesMut<'_, Id, T> {
        ValuesMut(self.inner_mut().items.iter_mut())
    }

    /// Every item `matches`, in order, to change. Items it passes over are not
    /// copied out of a shared collection, and nothing is if none matches.
    pub(crate) fn values_mut_where(
        &mut self,
        matches: impl Fn(&T) -> bool,
    ) -> impl Iterator<Item = &mut T> {
        let items = if self.values().any(&matches) {
            self.inner_mut().items.iter_mut()
        } else {
            [].iter_mut()
        };
        items
            .filter(move |(_, item)| matches(item))
            .map(|(_, item)| Arc::make_mut(item))
    }
}

/// Implements the iterator traits for a newtype over a double-ended,
/// exact-size iterator, passing each item of the inner iterator through `$map`.
macro_rules! map_iterator {
    ([$($generics:tt)*] $name:ty => $item:ty, $map:expr $(, where $($bounds:tt)*)?) => {
        impl<$($generics)*> Iterator for $name $(where $($bounds)*)? {
            type Item = $item;

            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map($map)
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }

        impl<$($generics)*> DoubleEndedIterator for $name $(where $($bounds)*)? {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back().map($map)
            }
        }

        impl<$($generics)*> ExactSizeIterator for $name $(where $($bounds)*)? {}

        impl<$($generics)*> std::iter::FusedIterator for $name $(where $($bounds)*)? {}
    };
}

/// The items of a [`Collection`] with their handles, in order.
#[derive(Debug)]
pub struct Iter<'a, Id: Handle, T>(
    std::slice::Iter<'a, (Key<Id>, Arc<T>)>,
    PhantomData<fn() -> Id>,
);

impl<Id: Handle, T> Clone for Iter<'_, Id, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

map_iterator!(['a, Id: Handle, T] Iter<'a, Id, T> => Entry<'a, Id, T>,
    |(key, value)| Entry { id: Id::from_key(*key), value: value.as_ref() });

/// The items of a [`Collection`], in order.
#[derive(Debug)]
pub struct Values<'a, Id: Handle, T>(std::slice::Iter<'a, (Key<Id>, Arc<T>)>);

impl<Id: Handle, T> Clone for Values<'_, Id, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

map_iterator!(['a, Id: Handle, T] Values<'a, Id, T> => &'a T, |(_, item)| item.as_ref());

/// The items of a [`Collection`], in order, each copied out of a shared
/// collection as the iterator reaches it.
#[derive(Debug)]
pub struct ValuesMut<'a, Id: Handle, T>(std::slice::IterMut<'a, (Key<Id>, Arc<T>)>);

map_iterator!(['a, Id: Handle, T] ValuesMut<'a, Id, T> => &'a mut T, |(_, item)| Arc::make_mut(item), where T: Clone);

impl<'a, Id: Handle, T> IntoIterator for &'a Collection<Id, T> {
    type Item = Entry<'a, Id, T>;
    type IntoIter = Iter<'a, Id, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<Id: Handle, T> FromIterator<T> for Collection<Id, T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut collection = Self::with_capacity(iter.size_hint().0);
        for value in iter {
            let _id = collection.push(value);
        }
        collection
    }
}

impl<Id: Handle, T> Extend<T> for Collection<Id, T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            let _id = self.push(value);
        }
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize)]
#[serde(bound(serialize = "T: serde::Serialize, K: serde::Serialize"))]
struct ReprRef<'a, K: slotmap::Key, T> {
    slots: std::borrow::Cow<'a, SlotMap<K, Option<usize>>>,
    items: &'a [(K, Arc<T>)],
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "T: serde::Deserialize<'de>, K: serde::Deserialize<'de>"))]
struct Repr<K: slotmap::Key, T> {
    slots: SlotMap<K, Option<usize>>,
    items: Vec<(K, Arc<T>)>,
}

/// Writes the live items only: tombstoned slots are freed in the written copy.
#[cfg(feature = "serde")]
impl<Id: Handle, T: serde::Serialize> serde::Serialize for Collection<Id, T>
where
    Key<Id>: serde::Serialize,
{
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use std::borrow::Cow;
        // A slot without an item is a tombstone; only copy the slots to drop them.
        let slots = if self.inner.slots.len() == self.inner.items.len() {
            Cow::Borrowed(&self.inner.slots)
        } else {
            let mut slots = self.inner.slots.clone();
            slots.retain(|_, position| position.is_some());
            Cow::Owned(slots)
        };
        ReprRef {
            slots,
            items: &self.inner.items,
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, Id: Handle, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Collection<Id, T>
where
    Key<Id>: serde::Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let Repr { mut slots, items } = Repr::<Key<Id>, T>::deserialize(deserializer)?;
        slots.retain(|_, position| position.is_some());

        if items.len() != slots.len() {
            return Err(D::Error::custom("collection items do not cover every slot"));
        }
        for (position, (key, _)) in items.iter().enumerate() {
            if slots.get(*key) != Some(&Some(position)) {
                return Err(D::Error::custom(
                    "collection slot does not point at its item's position",
                ));
            }
        }

        Ok(Self {
            inner: Arc::new(Inner { slots, items }),
            handle: PhantomData,
        })
    }
}

#[expect(clippy::unwrap_used, reason = "tests assert on known-live handles")]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Collection;
    use slotmap::Key as _;

    slotmap::new_key_type! { struct TestKey; }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    struct TestId(TestKey);

    super::impl_handle!(TestId, TestKey);

    type Strings = Collection<TestId, String>;

    fn collection_of(values: &[&str]) -> Strings {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn order(collection: &Strings) -> Vec<&str> {
        collection.values().map(String::as_str).collect()
    }

    fn id_at(collection: &Strings, position: usize) -> TestId {
        collection.at(position).unwrap().id()
    }

    fn ids(collection: &Strings) -> Vec<TestId> {
        collection.ids().collect()
    }

    fn value(collection: &Strings, id: TestId) -> Option<&str> {
        collection.get(id).map(|entry| entry.value().as_str())
    }

    /// The slot a key names, independent of its generation.
    fn slot(id: TestId) -> u64 {
        id.0.data().as_ffi() & 0xffff_ffff
    }

    fn assert_consistent(collection: &Strings) {
        for (position, entry) in collection.iter().enumerate() {
            let id = entry.id();
            assert_eq!(
                collection.position(id),
                Some(position),
                "position disagrees"
            );
            assert_eq!(id_at(collection, position), id, "at disagrees");
            assert_eq!(
                value(collection, id),
                Some(entry.as_str()),
                "handle resolves elsewhere"
            );
        }
        assert_eq!(collection.iter().count(), collection.len());
    }

    #[test]
    fn a_removed_handle_never_resolves_again() {
        let mut collection = collection_of(&["a", "b", "c"]);
        let b = id_at(&collection, 1);

        let removed = collection.remove(b).unwrap();
        assert_eq!(
            (removed.id(), removed.position(), removed.value().as_str()),
            (b, 1, "b")
        );
        assert!(!collection.contains(b));
        assert!(collection.get(b).is_none());
        assert_eq!(collection.position(b), None);

        let added = collection.push("d");
        assert_ne!(slot(added), slot(b), "a tombstoned slot is not reused");
        assert!(collection.get(b).is_none());
        assert_consistent(&collection);
    }

    #[test]
    fn removing_does_not_disturb_other_handles() {
        let mut collection = collection_of(&["a", "b", "c", "d"]);
        let ids = ids(&collection);

        let _removed = collection.remove(ids[1]);

        assert_eq!(order(&collection), ["a", "c", "d"]);
        assert_eq!(value(&collection, ids[0]), Some("a"));
        assert_eq!(value(&collection, ids[2]), Some("c"));
        assert_eq!(value(&collection, ids[3]), Some("d"));
        assert_eq!(collection.position(ids[3]), Some(2));
        assert_consistent(&collection);
    }

    #[test]
    fn inserting_shifts_positions_and_keeps_handles() {
        let mut collection = collection_of(&["a", "c"]);
        let c = id_at(&collection, 1);

        let b = collection.insert(1, "b").unwrap();

        assert_eq!(order(&collection), ["a", "b", "c"]);
        assert_eq!(collection.position(b), Some(1));
        assert_eq!(collection.position(c), Some(2));
        assert_consistent(&collection);

        let front = collection.insert(0, "z").unwrap();
        assert_eq!(order(&collection), ["z", "a", "b", "c"]);
        assert_eq!(collection.position(front), Some(0));
        assert_eq!(collection.position(c), Some(3));
        assert_consistent(&collection);

        assert!(collection.insert(5, "past the end").is_err());
        assert_eq!(collection.len(), 4);
    }

    #[test]
    fn swapping_exchanges_positions() {
        let mut collection = collection_of(&["a", "b", "c"]);
        let a = id_at(&collection, 0);
        let c = id_at(&collection, 2);

        collection.swap(0, 2).unwrap();

        assert_eq!(order(&collection), ["c", "b", "a"]);
        assert_eq!(collection.position(a), Some(2));
        assert_eq!(collection.position(c), Some(0));
        assert_consistent(&collection);

        assert!(collection.swap(0, 3).is_err());
        assert_eq!(order(&collection), ["c", "b", "a"]);
    }

    #[test]
    fn sorting_reorders_values_without_invalidating_handles() {
        let mut collection = collection_of(&["c", "a", "d", "b"]);
        let ids = ids(&collection);

        collection.sort_by_key(Clone::clone);

        assert_eq!(order(&collection), ["a", "b", "c", "d"]);
        assert_eq!(value(&collection, ids[0]), Some("c"));
        assert_eq!(value(&collection, ids[1]), Some("a"));
        assert_eq!(value(&collection, ids[2]), Some("d"));
        assert_eq!(value(&collection, ids[3]), Some("b"));
        assert_eq!(collection.position(ids[0]), Some(2));
        assert_eq!(collection.position(ids[1]), Some(0));
        assert_consistent(&collection);
    }

    #[test]
    fn sorting_is_stable() {
        let mut collection = collection_of(&["b1", "a1", "b2", "a2"]);
        let ids = ids(&collection);

        collection.sort_by_key(|value| value.as_bytes()[0]);

        assert_eq!(order(&collection), ["a1", "a2", "b1", "b2"]);
        assert_eq!(collection.position(ids[0]), Some(2));
        assert_eq!(collection.position(ids[2]), Some(3));
    }

    #[test]
    fn restoring_brings_an_item_back_under_its_own_handle_and_place() {
        let mut collection = collection_of(&["a", "b", "c"]);
        let b = id_at(&collection, 1);
        let removed = collection.remove(b).unwrap();

        collection.restore(removed).unwrap();

        assert_eq!(order(&collection), ["a", "b", "c"]);
        assert_eq!(value(&collection, b), Some("b"));
        assert_consistent(&collection);
    }

    #[test]
    fn restoring_is_refused_for_a_live_or_foreign_handle_or_a_bad_position() {
        let mut collection = collection_of(&["a", "b"]);
        let mut other = collection_of(&["a", "b", "c", "d"]);

        let removed = collection.remove_at(0).unwrap();
        let _live_again = collection.push("x");
        let foreign = other.remove_at(3).unwrap();
        assert!(
            collection.restore(foreign).is_err(),
            "a slot this collection never had"
        );

        let mut short = collection_of(&["a", "b", "c"]);
        let last = short.remove_at(2).unwrap();
        let _gone = short.remove_at(1);
        assert!(short.restore(last).is_err(), "past the end");

        collection.restore(removed).unwrap();
        assert_eq!(order(&collection), ["a", "b", "x"]);
        assert_eq!(order(&short), ["a"]);
    }

    #[test]
    fn a_clone_is_isolated_from_changes_to_the_original() {
        let mut collection = collection_of(&["a", "b", "c"]);
        let ids = ids(&collection);
        let snapshot = collection.clone();

        collection.get_mut(ids[0]).unwrap().push('!');
        let _removed = collection.remove(ids[1]);
        let _added = collection.push("d");
        collection.swap(0, 1).unwrap();

        assert_eq!(order(&snapshot), ["a", "b", "c"]);
        assert_eq!(value(&snapshot, ids[1]), Some("b"));
        assert_eq!(order(&collection), ["c", "a!", "d"]);
        assert_consistent(&snapshot);
        assert_consistent(&collection);
    }

    #[test]
    fn an_edit_copies_only_the_item_it_touches() {
        let mut collection = collection_of(&["a", "b", "c"]);
        let ids = ids(&collection);
        let snapshot = collection.clone();

        collection.get_mut(ids[1]).unwrap().push('!');

        for &id in &ids {
            let shared = Arc::ptr_eq(
                collection.get_arc(id).unwrap(),
                snapshot.get_arc(id).unwrap(),
            );
            assert_eq!(shared, id != ids[1], "only the edited item is copied");
        }
    }

    #[test]
    fn an_unshared_edit_happens_in_place() {
        let mut collection = collection_of(&["a"]);
        let a = id_at(&collection, 0);
        let before = Arc::as_ptr(collection.get_arc(a).unwrap());

        collection.get_mut(a).unwrap().push('!');

        assert_eq!(Arc::as_ptr(collection.get_arc(a).unwrap()), before);
    }

    #[test]
    fn replacing_hands_back_the_item_it_displaced() {
        let mut collection = collection_of(&["a", "b"]);
        let a = id_at(&collection, 0);

        let old = collection.replace(a, Arc::new("z".to_owned()));

        assert_eq!(old.as_deref().map(String::as_str), Some("a"));
        assert_eq!(order(&collection), ["z", "b"]);
        let _removed = collection.remove(a);
        assert!(collection.replace(a, Arc::new("y".to_owned())).is_none());
        assert_eq!(order(&collection), ["b"]);
    }

    #[test]
    fn removing_where_tombstones_the_matches_and_keeps_the_rest_in_order() {
        let mut collection = collection_of(&["a1", "b1", "a2", "b2", "a3"]);
        let ids = ids(&collection);

        let removed = collection.remove_where(|value| value.starts_with('b'));

        let named: Vec<_> = removed
            .iter()
            .map(|removed| (removed.id(), removed.position(), removed.value().as_str()))
            .collect();
        assert_eq!(named, [(ids[1], 1, "b1"), (ids[3], 3, "b2")]);
        assert_eq!(order(&collection), ["a1", "a2", "a3"]);
        assert_eq!(collection.position(ids[4]), Some(2));
        assert!(!collection.contains(ids[1]));
        assert_consistent(&collection);

        for removed in removed {
            collection.restore(removed).unwrap();
        }
        assert_eq!(order(&collection), ["a1", "b1", "a2", "b2", "a3"]);
        assert_consistent(&collection);
        assert!(collection.remove_where(|value| value == "z").is_empty());
    }

    #[test]
    fn mutating_where_copies_only_the_matches_out_of_a_shared_collection() {
        let mut collection = collection_of(&["a1", "b1", "a2"]);
        let ids = ids(&collection);
        let snapshot = collection.clone();

        for value in collection.values_mut_where(|value| value.starts_with('a')) {
            value.push('!');
        }

        assert_eq!(order(&collection), ["a1!", "b1", "a2!"]);
        assert_eq!(order(&snapshot), ["a1", "b1", "a2"]);
        for (i, &id) in ids.iter().enumerate() {
            let shared = Arc::ptr_eq(
                collection.get_arc(id).unwrap(),
                snapshot.get_arc(id).unwrap(),
            );
            assert_eq!(shared, i == 1);
        }
    }

    #[test]
    fn mutation_reaches_the_value_behind_the_handle() {
        let mut collection = collection_of(&["a", "b"]);
        let b = id_at(&collection, 1);

        collection.get_mut(b).unwrap().push('!');
        collection.sort_by_key(Clone::clone);

        assert_eq!(value(&collection, b), Some("b!"));
        assert_eq!(order(&collection), ["a", "b!"]);
    }

    #[test]
    fn an_empty_collection_resolves_nothing() {
        let mut collection = Strings::new();
        assert!(collection.is_empty());
        assert!(collection.at(0).is_none());
        assert!(collection.remove_at(0).is_none());

        let only = collection.push("a");
        assert_eq!(
            collection.remove(only).map(|removed| removed.position()),
            Some(0)
        );
        assert!(collection.is_empty());
        assert!(collection.get(only).is_none());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_collection_round_trips_through_serde() {
        let mut collection = collection_of(&["c", "a", "d", "b"]);
        let doomed = id_at(&collection, 2);
        let removed = collection.remove(doomed).unwrap();
        let added = collection.push("e");
        collection.sort_by_key(Clone::clone);

        let before: Vec<(TestId, String, usize)> = collection
            .iter()
            .map(|entry| {
                let position = collection.position(entry.id()).unwrap();
                (entry.id(), entry.value().clone(), position)
            })
            .collect();

        let json = serde_json::to_string(&collection).unwrap();
        let mut restored: Strings = serde_json::from_str(&json).unwrap();

        assert_consistent(&restored);
        assert_eq!(order(&restored), order(&collection));
        for (id, expected, position) in before {
            assert_eq!(value(&restored, id), Some(expected.as_str()));
            assert_eq!(restored.position(id), Some(position));
        }
        assert!(restored.get(doomed).is_none(), "a removed value stays gone");
        assert!(
            restored.restore(removed).is_err(),
            "a tombstone is not serialized"
        );
        assert_eq!(value(&restored, added), Some("e"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_broken_order_is_rejected() {
        let collection = collection_of(&["a", "b", "c"]);
        let json = serde_json::to_string(&collection).unwrap();
        let repr: serde_json::Value = serde_json::from_str(&json).unwrap();

        let mut short = repr.clone();
        short["items"].as_array_mut().unwrap().pop();
        assert!(
            serde_json::from_value::<Strings>(short).is_err(),
            "items that miss a slot must be rejected"
        );

        let mut duplicated = repr.clone();
        let first = duplicated["items"][0][0].clone();
        duplicated["items"][1][0] = first;
        assert!(
            serde_json::from_value::<Strings>(duplicated).is_err(),
            "items that name a key twice must be rejected"
        );

        let mut swapped = repr.clone();
        swapped["items"].as_array_mut().unwrap().swap(0, 2);
        assert!(
            serde_json::from_value::<Strings>(swapped).is_err(),
            "a slot pointing at another item's position must be rejected"
        );

        let mut missing = repr;
        missing["slots"].as_array_mut().unwrap().pop();
        assert!(
            serde_json::from_value::<Strings>(missing).is_err(),
            "an item naming a missing slot must be rejected"
        );
    }
}
