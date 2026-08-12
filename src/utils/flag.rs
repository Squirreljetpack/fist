//! A single runtime override flag for once-set global config values.
//!
//! Used by [`crate::run::state::ui`] to relocate the mutable `path.relative`
//! mirror out of the (immutable-after-init) global [`crate::config::ui::StyleConfig`]
//! so the config itself can live in a `OnceLock`.
//!
//! The flag is tri-state: an override can carry a *value* (`true`/`false`)
//! distinct from "no override" (fall back to the config default).

use std::sync::atomic::{AtomicU8, Ordering};

const UNSET: u8 = 0;
const OVERRIDE_FALSE: u8 = 1;
const OVERRIDE_TRUE: u8 = 2;

/// Runtime override flag backed by a single atomic word.
///
/// `new()` / `has_changed()` / `mark_changed()` follow the classic single-change
/// flag API; `override_value()` additionally recovers the override *value*,
/// which a bare boolean flag cannot carry.
///
/// All operations are relaxed single-word loads/stores — a single value written
/// from one thread, so no ordering guarantees are needed and reads compile to a
/// plain load. (Relaxed ordering performs no cache invalidation; coherence is
/// hardware-managed and irrelevant for one word.)
#[derive(Debug)]
pub struct SingleChangeFlag {
    state: AtomicU8,
}

impl SingleChangeFlag {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(UNSET),
        }
    }

    /// Hot loop check: whether an override is active.
    /// Translates to a single basic load instruction.
    #[inline(always)]
    pub fn has_changed(&self) -> bool {
        self.state.load(Ordering::Relaxed) != UNSET
    }

    /// Activate the override with `value`.
    ///
    /// Note this takes the value on purpose: callers may override with either
    /// `true` or `false`, and a bare "changed" bit cannot distinguish them.
    #[inline]
    pub fn mark_changed(
        &self,
        value: bool,
    ) {
        self.state.store(
            if value { OVERRIDE_TRUE } else { OVERRIDE_FALSE },
            Ordering::Relaxed,
        );
    }

    /// The active override value, or `None` when the config default applies.
    #[inline(always)]
    pub fn override_value(&self) -> Option<bool> {
        match self.state.load(Ordering::Relaxed) {
            OVERRIDE_FALSE => Some(false),
            OVERRIDE_TRUE => Some(true),
            _ => None,
        }
    }
}
