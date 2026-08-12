//! Selection-aware reload (TODO.md): selections are saved as hashed absolute
//! paths before a worker restart invalidates their nucleo indices, then
//! rehydrated against the fresh item store once the pane finishes
//! populating. The hashes survive the wipe period between the two.
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

use ahash::AHasher;

use crate::{abspath::AbsPath, run::item::PathItem};

/// Fast, deterministic path hash. The seed is fixed across threads within
/// the process (hashes are never persisted, so cross-process/cross-version
/// stability is irrelevant).
#[inline]
pub fn hash_path(path: &AbsPath) -> u64 {
    let mut hasher = AHasher::default();
    path.hash(&mut hasher);
    hasher.finish()
}

/// Hashed selection paths pending rehydration after a reload. Stored in the
/// [`crate::run::state::STORE`] by [`crate::run::ahandlers::fs_reload`] and
/// consumed by [`crate::run::dhandlers::selection_refill_handler`].
#[derive(Debug)]
pub struct PendingSelections(pub Vec<u64>);

/// Map saved hashes back onto the fresh item store (`(nucleo index, item)`
/// pairs): new nucleo indices in the original selection order. Hash
/// collisions resolve to the first path.
///
/// Generic over the item iterator because the worker's storage type
/// (matchmaker-nucleo's private boxcar vec) cannot be named here.
pub fn rehydrate<'a>(
    saved_hashes: &[u64],
    items: impl IntoIterator<Item = (u32, &'a PathItem)>,
) -> Vec<u32> {
    // reverse lookup: hash -> new index
    let mut reverse: HashMap<u64, u32> = HashMap::new();
    for (idx, item) in items {
        reverse.entry(hash_path(&item.path)).or_insert(idx);
    }

    saved_hashes
        .iter()
        .filter_map(|hash| reverse.get(hash).copied())
        .collect()
}
