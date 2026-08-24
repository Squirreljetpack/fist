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

/// What a directory transfer does when the target directory already
/// exists. Single-file transfers never consult it — their conflicts are
/// handled per-entry by [`ConflictStrategy`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergeStrategy {
    /// The existing target is replaced wholesale (removed, then copied
    /// fresh).
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CopyParams {
    pub workers: NonZeroUsize,
    pub preserve_metadata: bool,
    pub reflink: ReflinkMode,
    pub conflict: ConflictStrategy,
    pub merge: MergeStrategy,
    pub buffer_size: NonZeroUsize,
}

impl Default for CopyParams {
    fn default() -> Self {
        Self {
            workers: std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN),
            preserve_metadata: true,
            #[cfg(debug_assertions)]
            reflink: ReflinkMode::Never,
            #[cfg(not(debug_assertions))]
            reflink: ReflinkMode::Auto,
            conflict: ConflictStrategy::Overwrite,
            merge: MergeStrategy::default(),
            buffer_size: NonZeroUsize::new(512 * 1024).expect("static"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MoveParams {
    #[serde(flatten)]
    pub copy: CopyParams,
    pub delete_source: bool,
}

impl Default for MoveParams {
    fn default() -> Self {
        Self {
            copy: CopyParams::default(),
            delete_source: true,
        }
    }
}
