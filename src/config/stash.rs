use serde::{Deserialize, Serialize};

/// How a stashed item is executed. Builtin kinds (`copy`/`cut`/`symlink`)
/// map directly onto the matching variant; any other item kind is treated as
/// a [`ExecuteStrategy::Script`] reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecuteStrategy {
    Copy,
    Cut,
    Symlink,
    None,
    /// Runs the lua script, feeding it `(item, dest)`. Supports the `@file`
    /// syntax of [`crate::run::lua::load_script`].
    Script(String),
}
