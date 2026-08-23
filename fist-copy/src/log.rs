use std::collections::VecDeque;
use std::sync::Mutex;

const DEFAULT_CAP: usize = 4096;
const ERROR_KEEP: usize = 1024;

#[derive(Debug)]
pub struct TaskLog {
    cap: usize,
    inner: Mutex<LogInner>,
}

#[derive(Debug)]
struct LogInner {
    lines: VecDeque<String>,
    errors: VecDeque<String>,
}

impl Default for TaskLog {
    fn default() -> Self {
        Self::new(DEFAULT_CAP)
    }
}

impl TaskLog {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: Mutex::new(LogInner {
                lines: VecDeque::with_capacity(cap.min(4096)),
                errors: VecDeque::new(),
            }),
        }
    }

    pub(crate) fn info(
        &self,
        msg: impl Into<String>,
    ) {
        let msg = msg.into();
        log::debug!("fist-copy: {msg}");
        if let Ok(mut g) = self.inner.lock() {
            push_capped(&mut g.lines, msg, self.cap);
        }
    }

    pub(crate) fn error(
        &self,
        msg: impl Into<String>,
    ) {
        let msg = msg.into();
        log::warn!("fist-copy: {msg}");
        if let Ok(mut g) = self.inner.lock() {
            push_capped(&mut g.lines, msg.clone(), self.cap);
            push_capped(&mut g.errors, msg, ERROR_KEEP);
        }
    }

    pub fn lines(&self) -> Vec<String> {
        self.inner
            .lock()
            .map(|g| g.lines.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn errors(&self) -> Vec<String> {
        self.inner
            .lock()
            .map(|g| g.errors.iter().cloned().collect())
            .unwrap_or_default()
    }
}

fn push_capped(
    q: &mut VecDeque<String>,
    msg: String,
    cap: usize,
) {
    while q.len() >= cap {
        q.pop_front();
    }
    q.push_back(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_evicts_oldest_and_errors_are_kept() {
        let l = TaskLog::new(3);
        for i in 0..5 {
            l.info(format!("line {i}"));
        }
        assert_eq!(l.lines(), vec!["line 2", "line 3", "line 4"]);
        assert!(l.errors().is_empty());

        l.error("boom");
        assert_eq!(l.lines().last(), Some(&"boom".to_string()));
        assert_eq!(l.errors(), vec!["boom".to_string()]);
    }
}
