use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReflinkMode {
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConflictStrategy {
    Fail,
    Overwrite,
    Skip,
    RenameSuffix,
    Abort,
}

use cba::claim::ClaimPolicy;

/// What a transfer does when its root destination already exists — for
/// directory sources *and* single-file sources. [`ConflictStrategy`] only
/// governs entries *inside* a merged or copied tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RootStrategy {
    /// The existing target is replaced wholesale.
    Overwrite,
    /// The existing target is kept; entries are resolved per-entry by
    /// [`ConflictStrategy`] inside it.
    Merge,
    /// The existing target is left untouched and the transfer lands in
    /// the first free suffixed sibling (`name`, `name_1`, ...), claimed
    /// atomically.
    #[default]
    Rename,
    /// A pre-existing target fails the task.
    Fail,
}

impl RootStrategy {
    /// Converts strategy into the appropriate reservation claim policy.
    pub fn claim_policy(self) -> ClaimPolicy<'static> {
        match self {
            Self::Rename => ClaimPolicy::default(),
            Self::Fail => ClaimPolicy::Strict,
            Self::Overwrite | Self::Merge => ClaimPolicy::Ignore,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TransferParams {
    pub workers: NonZeroUsize,
    pub preserve_metadata: bool,
    pub reflink: ReflinkMode,
    pub conflict: ConflictStrategy,
    #[serde(alias = "merge")]
    pub root: RootStrategy,
    pub buffer_size: NonZeroUsize,
    /// Removes sources as entries land, turning the transfer into a move.
    /// Not part of the config file: it is set at dispatch time from the
    /// queue-row kind.
    #[serde(skip)]
    pub r#move: bool,
}

impl Default for TransferParams {
    fn default() -> Self {
        Self {
            workers: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
            preserve_metadata: true,
            #[cfg(debug_assertions)]
            reflink: ReflinkMode::Never,
            #[cfg(not(debug_assertions))]
            reflink: ReflinkMode::Auto,
            conflict: ConflictStrategy::Overwrite,
            root: RootStrategy::default(),
            r#move: false,
            buffer_size: NonZeroUsize::new(512 * 1024).expect("static"),
        }
    }
}
