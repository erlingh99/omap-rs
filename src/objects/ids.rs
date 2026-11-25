slotmap::new_key_type! {
    /// The arena slot an object handle names.
    pub struct ObjectKey;
}

/// A handle to an object in a [`crate::parts::MapPart`].
///
/// Keeps naming the same object while others are added, removed or reordered,
/// and in every copy of the part made after the object was added. Stops
/// resolving once the object is removed, until it is restored under it.
///
/// # A handle belongs to one part
///
/// Handles are slot keys, not pointers, so they carry no record of which part
/// minted them. Used against a *different* part, one resolves to whatever
/// occupies that slot there rather than to `None`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectId(pub(crate) ObjectKey);

crate::collection::impl_handle!(ObjectId, ObjectKey);
