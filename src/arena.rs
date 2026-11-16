//! An ordered arena with stable keys, copied on write.
//!
//! The live items sit in order in `items`; `slots` maps every key ever issued
//! to its item's position, or to `None` once the item is removed. Cloning an
//! arena copies one pointer; both copies share everything until either one
//! changes, and the first change copies the slots and the item pointers. Each
//! item sits behind its own [`Arc`], so changing an item in a shared arena
//! copies that item alone, and every other item stays shared between the
//! copies. Mutating iteration over a shared arena copies every item it yields.
//!
//! A removed item leaves a tombstone: its key stays reserved and is never
//! issued again, so [`Arena::restore`] can put the item back under the same
//! key. Tombstones are not serialized; after a serde round trip a removed key
//! no longer resolves and cannot be restored.

use std::sync::Arc;

use slotmap::{Key, SlotMap};

pub(crate) struct Arena<T, K: Key> {
    inner: Arc<Inner<T, K>>,
}

struct Inner<T, K: Key> {
    /// The position in `items` of every key's item; `None` is a tombstone.
    slots: SlotMap<K, Option<usize>>,
    items: Vec<(K, Arc<T>)>,
}

impl<T, K: Key> Clone for Inner<T, K> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            items: self.items.clone(),
        }
    }
}

impl<T, K: Key> Default for Inner<T, K> {
    fn default() -> Self {
        Self {
            slots: SlotMap::with_key(),
            items: Vec::new(),
        }
    }
}

impl<T, K: Key> Inner<T, K> {
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

impl<T, K: Key> Clone for Arena<T, K> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T, K: Key> Default for Arena<T, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: std::fmt::Debug, K: Key> std::fmt::Debug for Arena<T, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.values()).finish()
    }
}

impl<T, K: Key> Arena<T, K> {
    pub(crate) fn new() -> Self {
        Self::with_capacity(0)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                slots: SlotMap::with_capacity_and_key(capacity),
                items: Vec::with_capacity(capacity),
            }),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.items.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.items.is_empty()
    }

    pub(crate) fn position(&self, id: K) -> Option<usize> {
        *self.inner.slots.get(id)?
    }

    pub(crate) fn contains(&self, id: K) -> bool {
        self.position(id).is_some()
    }

    pub(crate) fn get(&self, id: K) -> Option<&T> {
        self.get_arc(id).map(Arc::as_ref)
    }

    pub(crate) fn get_arc(&self, id: K) -> Option<&Arc<T>> {
        let (_, item) = self.inner.items.get(self.position(id)?)?;
        Some(item)
    }

    pub(crate) fn get_at(&self, position: usize) -> Option<&T> {
        let (_, item) = self.inner.items.get(position)?;
        Some(item)
    }

    pub(crate) fn id_at(&self, position: usize) -> Option<K> {
        let (key, _) = self.inner.items.get(position)?;
        Some(*key)
    }

    /// The slots and items to change, copied first if another arena shares them.
    fn inner_mut(&mut self) -> &mut Inner<T, K> {
        Arc::make_mut(&mut self.inner)
    }

    pub(crate) fn push(&mut self, value: T) -> K {
        self.push_arc(Arc::new(value))
    }

    pub(crate) fn push_arc(&mut self, value: Arc<T>) -> K {
        let inner = self.inner_mut();
        let key = inner.slots.insert(Some(inner.items.len()));
        inner.items.push((key, value));
        key
    }

    /// # Panics
    ///
    /// Panics if `position > self.len()`.
    pub(crate) fn insert(&mut self, position: usize, value: T) -> K {
        assert!(position <= self.len(), "position out of bounds");
        let inner = self.inner_mut();
        let key = inner.slots.insert(Some(position));
        inner.items.insert(position, (key, Arc::new(value)));
        inner.reindex(position + 1);
        key
    }

    /// Takes the item out, leaving `id` reserved for [`Self::restore`].
    pub(crate) fn remove(&mut self, id: K) -> Option<Arc<T>> {
        let position = self.position(id)?;
        self.remove_at(position)
    }

    pub(crate) fn remove_at(&mut self, position: usize) -> Option<Arc<T>> {
        if position >= self.len() {
            return None;
        }
        let inner = self.inner_mut();
        let (key, item) = inner.items.remove(position);
        if let Some(slot) = inner.slots.get_mut(key) {
            *slot = None;
        }
        inner.reindex(position);
        Some(item)
    }

    /// Puts `value` back under the removed `id`, at `position` in the order.
    ///
    /// # Errors
    ///
    /// Hands `value` back if `id` does not name a removed slot of this arena,
    /// or `position > self.len()`. A key does not record which arena issued
    /// it, so a key from another arena naming a slot removed here is taken.
    pub(crate) fn restore(&mut self, id: K, value: Arc<T>, position: usize) -> Result<(), Arc<T>> {
        let is_tombstone = matches!(self.inner.slots.get(id), Some(None));
        if !is_tombstone || position > self.len() {
            return Err(value);
        }
        let inner = self.inner_mut();
        inner.items.insert(position, (id, value));
        inner.reindex(position);
        Ok(())
    }

    /// Puts `value` in place of the live item at `id`, returning that item.
    pub(crate) fn replace(&mut self, id: K, value: Arc<T>) -> Option<Arc<T>> {
        let position = self.position(id)?;
        let (_, item) = self.inner_mut().items.get_mut(position)?;
        Some(std::mem::replace(item, value))
    }

    pub(crate) fn get_mut(&mut self, id: K) -> Option<&mut T>
    where
        T: Clone,
    {
        let position = self.position(id)?;
        self.get_at_mut(position)
    }

    pub(crate) fn get_at_mut(&mut self, position: usize) -> Option<&mut T>
    where
        T: Clone,
    {
        if position >= self.len() {
            return None;
        }
        let (_, item) = self.inner_mut().items.get_mut(position)?;
        Some(Arc::make_mut(item))
    }

    pub(crate) fn swap(&mut self, first: usize, second: usize) {
        let inner = self.inner_mut();
        inner.items.swap(first, second);
        for position in [first, second] {
            let key = inner.items[position].0;
            if let Some(slot) = inner.slots.get_mut(key) {
                *slot = Some(position);
            }
        }
    }

    pub(crate) fn sort_by_key<S: Ord, F: Fn(&T) -> S>(&mut self, key: F) {
        let inner = self.inner_mut();
        inner.items.sort_by_key(|(_, item)| key(item));
        inner.reindex(0);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, &T)> {
        self.inner
            .items
            .iter()
            .map(|(key, item)| (*key, item.as_ref()))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.inner.items.iter().map(|(_, item)| item.as_ref())
    }

    /// The live items with their keys, in order.
    pub(crate) fn entries(&self) -> std::slice::Iter<'_, (K, Arc<T>)> {
        self.inner.items.iter()
    }

    /// The live items with their keys, in order, to copy on write one by one.
    pub(crate) fn entries_mut(&mut self) -> std::slice::IterMut<'_, (K, Arc<T>)> {
        self.inner_mut().items.iter_mut()
    }

    /// Every live item, in order.
    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut T>
    where
        T: Clone,
    {
        self.values_mut_where(|_| true)
    }

    /// Every live item `matches`, in order. Items it passes over are not
    /// copied out of a shared arena, and nothing is if none matches.
    pub(crate) fn values_mut_where<F: Fn(&T) -> bool>(
        &mut self,
        matches: F,
    ) -> impl Iterator<Item = &mut T>
    where
        T: Clone,
    {
        let items = if self.values().any(&matches) {
            self.inner_mut().items.iter_mut()
        } else {
            [].iter_mut()
        };
        items
            .filter(move |(_, item)| matches(item))
            .map(|(_, item)| Arc::make_mut(item))
    }

    /// Removes every item `matches`, in order, leaving their keys reserved
    /// for [`Self::restore`]. Each comes with the position it had before the
    /// removal: restoring them in the order returned puts each back there.
    pub(crate) fn remove_where<F: Fn(&T) -> bool>(
        &mut self,
        matches: F,
    ) -> Vec<(K, usize, Arc<T>)> {
        let Some(first) = self.values().position(&matches) else {
            return Vec::new();
        };
        let inner = self.inner_mut();
        let mut removed = Vec::new();
        let mut kept = Vec::with_capacity(inner.items.len());
        for (position, (key, item)) in std::mem::take(&mut inner.items).into_iter().enumerate() {
            if position >= first && matches(&item) {
                if let Some(slot) = inner.slots.get_mut(key) {
                    *slot = None;
                }
                removed.push((key, position, item));
            } else {
                kept.push((key, item));
            }
        }
        inner.items = kept;
        inner.reindex(first);
        removed
    }

    /// Every live item in order.
    pub(crate) fn into_values(self) -> impl Iterator<Item = Arc<T>> {
        let items = match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner.items,
            Err(shared) => shared.items.clone(),
        };
        items.into_iter().map(|(_, item)| item)
    }
}

impl<T, K: Key> FromIterator<T> for Arena<T, K> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut arena = Self::with_capacity(iter.size_hint().0);
        for value in iter {
            let _key = arena.push(value);
        }
        arena
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize)]
#[serde(bound(serialize = "T: serde::Serialize, K: serde::Serialize"))]
struct ArenaReprRef<'a, T, K: Key> {
    slots: SlotMap<K, Option<usize>>,
    items: &'a [(K, Arc<T>)],
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "T: serde::Deserialize<'de>, K: serde::Deserialize<'de>"))]
struct ArenaRepr<T, K: Key> {
    slots: SlotMap<K, Option<usize>>,
    items: Vec<(K, Arc<T>)>,
}

/// Writes the live items only: tombstoned slots are freed in the written copy.
#[cfg(feature = "serde")]
impl<T: serde::Serialize, K: Key + serde::Serialize> serde::Serialize for Arena<T, K> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut slots = self.inner.slots.clone();
        slots.retain(|_, position| position.is_some());
        ArenaReprRef {
            slots,
            items: &self.inner.items,
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>, K: Key + serde::Deserialize<'de>> serde::Deserialize<'de>
    for Arena<T, K>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let ArenaRepr { mut slots, items } = ArenaRepr::<T, K>::deserialize(deserializer)?;
        slots.retain(|_, position| position.is_some());

        if items.len() != slots.len() {
            return Err(D::Error::custom("arena items do not cover every slot"));
        }
        for (position, (key, _)) in items.iter().enumerate() {
            if slots.get(*key) != Some(&Some(position)) {
                return Err(D::Error::custom(
                    "arena slot does not point at its item's position",
                ));
            }
        }

        Ok(Self {
            inner: Arc::new(Inner { slots, items }),
        })
    }
}

#[expect(clippy::unwrap_used, reason = "tests assert on known-live handles")]
#[cfg(test)]
mod tests {
    use super::Arena;
    use slotmap::Key as _;

    slotmap::new_key_type! { struct TestKey; }

    type TestArena = Arena<String, TestKey>;

    fn arena_of(values: &[&str]) -> TestArena {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn order(arena: &TestArena) -> Vec<&str> {
        arena.values().map(String::as_str).collect()
    }

    /// The slot a key names, independent of its generation.
    fn slot(key: TestKey) -> u64 {
        key.data().as_ffi() & 0xffff_ffff
    }

    fn assert_consistent(arena: &TestArena) {
        for (position, (id, value)) in arena.iter().enumerate() {
            assert_eq!(arena.position(id), Some(position), "position disagrees");
            assert_eq!(arena.id_at(position), Some(id), "id_at disagrees");
            assert_eq!(arena.get(id), Some(value), "handle resolves elsewhere");
        }
        assert_eq!(arena.iter().count(), arena.len());
    }

    fn removed(value: Option<std::sync::Arc<String>>) -> Option<String> {
        value.map(std::sync::Arc::unwrap_or_clone)
    }

    #[test]
    fn a_removed_handle_never_resolves_again() {
        let mut arena = arena_of(&["a", "b", "c"]);
        let b = arena.id_at(1).unwrap();

        assert_eq!(removed(arena.remove(b)).as_deref(), Some("b"));
        assert!(!arena.contains(b));
        assert_eq!(arena.get(b), None);
        assert_eq!(arena.position(b), None);

        let added = arena.push("d".to_owned());
        assert_ne!(slot(added), slot(b), "a tombstoned slot is not reused");
        assert_eq!(arena.get(b), None);
        assert_consistent(&arena);
    }

    #[test]
    fn removing_does_not_disturb_other_handles() {
        let mut arena = arena_of(&["a", "b", "c", "d"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();

        let _removed = arena.remove(ids[1]);

        assert_eq!(order(&arena), ["a", "c", "d"]);
        assert_eq!(arena.get(ids[0]).unwrap(), "a");
        assert_eq!(arena.get(ids[2]).unwrap(), "c");
        assert_eq!(arena.get(ids[3]).unwrap(), "d");
        assert_eq!(arena.position(ids[3]), Some(2));
        assert_consistent(&arena);
    }

    #[test]
    fn inserting_shifts_positions_and_keeps_handles() {
        let mut arena = arena_of(&["a", "c"]);
        let c = arena.id_at(1).unwrap();

        let b = arena.insert(1, "b".to_owned());

        assert_eq!(order(&arena), ["a", "b", "c"]);
        assert_eq!(arena.position(b), Some(1));
        assert_eq!(arena.position(c), Some(2));
        assert_consistent(&arena);

        let front = arena.insert(0, "z".to_owned());
        assert_eq!(order(&arena), ["z", "a", "b", "c"]);
        assert_eq!(arena.position(front), Some(0));
        assert_eq!(arena.position(c), Some(3));
        assert_consistent(&arena);
    }

    #[test]
    fn swapping_exchanges_positions() {
        let mut arena = arena_of(&["a", "b", "c"]);
        let a = arena.id_at(0).unwrap();
        let c = arena.id_at(2).unwrap();

        arena.swap(0, 2);

        assert_eq!(order(&arena), ["c", "b", "a"]);
        assert_eq!(arena.position(a), Some(2));
        assert_eq!(arena.position(c), Some(0));
        assert_consistent(&arena);
    }

    #[test]
    fn sorting_reorders_values_without_invalidating_handles() {
        let mut arena = arena_of(&["c", "a", "d", "b"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();

        arena.sort_by_key(Clone::clone);

        assert_eq!(order(&arena), ["a", "b", "c", "d"]);
        assert_eq!(arena.get(ids[0]).unwrap(), "c");
        assert_eq!(arena.get(ids[1]).unwrap(), "a");
        assert_eq!(arena.get(ids[2]).unwrap(), "d");
        assert_eq!(arena.get(ids[3]).unwrap(), "b");
        assert_eq!(arena.position(ids[0]), Some(2));
        assert_eq!(arena.position(ids[1]), Some(0));
        assert_consistent(&arena);
    }

    #[test]
    fn sorting_is_stable() {
        let mut arena = arena_of(&["b1", "a1", "b2", "a2"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();

        arena.sort_by_key(|value| value.as_bytes()[0]);

        assert_eq!(order(&arena), ["a1", "a2", "b1", "b2"]);
        assert_eq!(arena.position(ids[0]), Some(2));
        assert_eq!(arena.position(ids[2]), Some(3));
    }

    #[test]
    fn restoring_brings_an_item_back_under_its_own_handle() {
        let mut arena = arena_of(&["a", "b", "c"]);
        let b = arena.id_at(1).unwrap();
        let taken = arena.remove(b).unwrap();

        arena.restore(b, taken, 0).unwrap();

        assert_eq!(order(&arena), ["b", "a", "c"]);
        assert_eq!(arena.get(b).unwrap(), "b");
        assert_consistent(&arena);
    }

    #[test]
    fn restoring_is_refused_for_a_live_or_foreign_handle_or_a_bad_position() {
        let mut arena = arena_of(&["a", "b"]);
        let a = arena.id_at(0).unwrap();
        let value = std::sync::Arc::new("x".to_owned());

        assert!(
            arena.restore(a, std::sync::Arc::clone(&value), 0).is_err(),
            "a is live"
        );

        let mut other = arena_of(&["a", "b", "c", "d"]);
        let foreign = other.id_at(3).unwrap();
        assert!(
            arena
                .restore(foreign, std::sync::Arc::clone(&value), 0)
                .is_err(),
            "a slot this arena never had"
        );

        let _taken = other.remove(foreign);
        assert!(other.restore(foreign, value, 4).is_err(), "past the end");
        assert_eq!(order(&arena), ["a", "b"]);
        assert_eq!(order(&other), ["a", "b", "c"]);
    }

    #[test]
    fn a_clone_is_isolated_from_changes_to_the_original() {
        let mut arena = arena_of(&["a", "b", "c"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();
        let snapshot = arena.clone();

        arena.get_mut(ids[0]).unwrap().push('!');
        let _taken = arena.remove(ids[1]);
        let _added = arena.push("d".to_owned());
        arena.swap(0, 1);

        assert_eq!(order(&snapshot), ["a", "b", "c"]);
        assert_eq!(snapshot.get(ids[1]).unwrap(), "b");
        assert_eq!(order(&arena), ["c", "a!", "d"]);
        assert_consistent(&snapshot);
        assert_consistent(&arena);
    }

    #[test]
    fn an_edit_copies_only_the_item_it_touches() {
        let mut arena = arena_of(&["a", "b", "c"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();
        let snapshot = arena.clone();

        arena.get_mut(ids[1]).unwrap().push('!');

        for &id in &ids {
            let shared =
                std::sync::Arc::ptr_eq(arena.get_arc(id).unwrap(), snapshot.get_arc(id).unwrap());
            assert_eq!(shared, id != ids[1], "only the edited item is copied");
        }
    }

    #[test]
    fn an_unshared_edit_happens_in_place() {
        let mut arena = arena_of(&["a"]);
        let a = arena.id_at(0).unwrap();
        let before = std::sync::Arc::as_ptr(arena.get_arc(a).unwrap());

        arena.get_mut(a).unwrap().push('!');

        assert_eq!(std::sync::Arc::as_ptr(arena.get_arc(a).unwrap()), before);
    }

    #[test]
    fn replacing_hands_back_the_item_it_displaced() {
        let mut arena = arena_of(&["a", "b"]);
        let a = arena.id_at(0).unwrap();

        let old = arena.replace(a, std::sync::Arc::new("z".to_owned()));

        assert_eq!(removed(old).as_deref(), Some("a"));
        assert_eq!(order(&arena), ["z", "b"]);
        let _taken = arena.remove(a);
        assert!(
            arena
                .replace(a, std::sync::Arc::new("y".to_owned()))
                .is_none()
        );
        assert_eq!(order(&arena), ["b"]);
    }

    #[test]
    fn into_values_yields_the_live_items_in_order() {
        let mut arena = arena_of(&["c", "a", "b"]);
        let _taken = arena.remove_at(1);
        let snapshot = arena.clone();

        let values: Vec<String> = arena
            .into_values()
            .map(std::sync::Arc::unwrap_or_clone)
            .collect();

        assert_eq!(values, ["c", "b"]);
        assert_eq!(order(&snapshot), ["c", "b"]);
    }

    #[test]
    fn removing_where_tombstones_the_matches_and_keeps_the_rest_in_order() {
        let mut arena = arena_of(&["a1", "b1", "a2", "b2", "a3"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();

        let taken = arena.remove_where(|value| value.starts_with('b'));

        let named: Vec<_> = taken
            .iter()
            .map(|(id, position, v)| (*id, *position, v.as_str()))
            .collect();
        assert_eq!(named, [(ids[1], 1, "b1"), (ids[3], 3, "b2")]);
        assert_eq!(order(&arena), ["a1", "a2", "a3"]);
        assert_eq!(arena.position(ids[4]), Some(2));
        assert!(!arena.contains(ids[1]));
        assert_consistent(&arena);

        for (id, position, value) in taken {
            arena.restore(id, value, position).unwrap();
        }
        assert_eq!(order(&arena), ["a1", "b1", "a2", "b2", "a3"]);
        assert_consistent(&arena);
        assert!(arena.remove_where(|value| value == "z").is_empty());
    }

    #[test]
    fn mutating_where_copies_only_the_matches_out_of_a_shared_arena() {
        let mut arena = arena_of(&["a1", "b1", "a2"]);
        let ids: Vec<_> = arena.iter().map(|(id, _)| id).collect();
        let snapshot = arena.clone();

        for value in arena.values_mut_where(|value| value.starts_with('a')) {
            value.push('!');
        }

        assert_eq!(order(&arena), ["a1!", "b1", "a2!"]);
        assert_eq!(order(&snapshot), ["a1", "b1", "a2"]);
        for (i, &id) in ids.iter().enumerate() {
            let shared =
                std::sync::Arc::ptr_eq(arena.get_arc(id).unwrap(), snapshot.get_arc(id).unwrap());
            assert_eq!(shared, i == 1);
        }
    }

    #[test]
    fn mutation_reaches_the_value_behind_the_handle() {
        let mut arena = arena_of(&["a", "b"]);
        let b = arena.id_at(1).unwrap();

        arena.get_mut(b).unwrap().push('!');
        arena.sort_by_key(Clone::clone);

        assert_eq!(arena.get(b).unwrap(), "b!");
        assert_eq!(order(&arena), ["a", "b!"]);
    }

    #[test]
    fn an_empty_arena_resolves_nothing() {
        let mut arena: TestArena = Arena::new();
        assert!(arena.is_empty());
        assert_eq!(arena.id_at(0), None);
        assert_eq!(arena.get_at(0), None);
        assert_eq!(arena.remove_at(0), None);

        let only = arena.push("a".to_owned());
        assert_eq!(removed(arena.remove(only)).as_deref(), Some("a"));
        assert!(arena.is_empty());
        assert_eq!(arena.get(only), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn an_arena_round_trips_through_serde() {
        let mut arena = arena_of(&["c", "a", "d", "b"]);
        let doomed = arena.id_at(2).unwrap();
        let _removed = arena.remove(doomed);
        let added = arena.push("e".to_owned());
        arena.sort_by_key(Clone::clone);

        let before: Vec<(TestKey, String, usize)> = arena
            .iter()
            .map(|(id, value)| (id, value.clone(), arena.position(id).unwrap()))
            .collect();

        let json = serde_json::to_string(&arena).unwrap();
        let restored: TestArena = serde_json::from_str(&json).unwrap();

        assert_consistent(&restored);
        assert_eq!(order(&restored), order(&arena));
        for (id, value, position) in before {
            assert_eq!(restored.get(id).map(String::as_str), Some(value.as_str()));
            assert_eq!(restored.position(id), Some(position));
        }
        assert!(restored.get(doomed).is_none(), "a removed value stays gone");
        let mut restored = restored;
        assert!(
            restored
                .restore(doomed, std::sync::Arc::new("c".to_owned()), 0)
                .is_err(),
            "a tombstone is not serialized"
        );
        assert_eq!(restored.get(added).map(String::as_str), Some("e"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_broken_order_is_rejected() {
        let arena = arena_of(&["a", "b", "c"]);
        let json = serde_json::to_string(&arena).unwrap();
        let repr: serde_json::Value = serde_json::from_str(&json).unwrap();

        let mut short = repr.clone();
        short["items"].as_array_mut().unwrap().pop();
        assert!(
            serde_json::from_value::<TestArena>(short).is_err(),
            "items that miss a slot must be rejected"
        );

        let mut duplicated = repr.clone();
        let first = duplicated["items"][0][0].clone();
        duplicated["items"][1][0] = first;
        assert!(
            serde_json::from_value::<TestArena>(duplicated).is_err(),
            "items that name a key twice must be rejected"
        );

        let mut swapped = repr.clone();
        swapped["items"].as_array_mut().unwrap().swap(0, 2);
        assert!(
            serde_json::from_value::<TestArena>(swapped).is_err(),
            "a slot pointing at another item's position must be rejected"
        );

        let mut missing = repr;
        missing["slots"].as_array_mut().unwrap().pop();
        assert!(
            serde_json::from_value::<TestArena>(missing).is_err(),
            "an item naming a missing slot must be rejected"
        );
    }
}
