use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicU8>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicU8::new(0)))
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed) != 0
    }

    pub(crate) fn cancel(&self) {
        self.0.store(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_uncancelled_and_clones_share_state() {
        let t = CancelToken::new();
        let c = t.clone();
        assert!(!t.is_cancelled());
        assert!(!c.is_cancelled());
        t.cancel();
        assert!(t.is_cancelled());
        assert!(c.is_cancelled());
    }
}
