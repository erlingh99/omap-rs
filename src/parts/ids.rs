slotmap::new_key_type! {
    /// The arena slot a part handle names.
    pub struct PartKey;
}

/// A handle to a part in [`crate::parts::MapParts`].
///
/// Keeps naming the same part while others are added, removed or reordered,
/// and in every copy of the map made after the part was added. Stops resolving
/// once the part is removed, until it is restored under it. Meaningless
/// against any other [`crate::Omap`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartId(pub(crate) PartKey);

crate::collection::impl_handle!(PartId, PartKey);
