mod config;
mod copier;
mod error;
pub mod extract;
mod job;
mod log;
mod meta;
mod progress;
mod reflink;
mod scheduler;
mod token;
mod walker;
mod work;

pub use config::{ConflictStrategy, CopyParams, MergeStrategy, MoveParams, ReflinkMode};
pub use job::{ExtractParams, JobKind, JobRequest, SubmitError};
pub use log::TaskLog;
pub use progress::{CleanupState, TaskSnapshot, TaskState};
pub use scheduler::{Scheduler, SchedulerOptions, TaskHandle, TaskId};
pub use token::CancelToken;

pub fn prune_nested_sources(sources: &mut Vec<std::path::PathBuf>) {
    sources.sort_by_key(|p| p.components().count());
    let mut kept: Vec<std::path::PathBuf> = Vec::new();
    sources.retain(|p| {
        if kept.iter().any(|k| p.starts_with(k)) {
            false
        } else {
            kept.push(p.clone());
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_sources_inside_other_sources() {
        let mut v = vec![
            std::path::PathBuf::from("/a/b/c"),
            std::path::PathBuf::from("/a"),
            std::path::PathBuf::from("/x"),
            std::path::PathBuf::from("/ab"),
        ];
        prune_nested_sources(&mut v);
        assert_eq!(
            v,
            vec![
                std::path::PathBuf::from("/a"),
                std::path::PathBuf::from("/x"),
                std::path::PathBuf::from("/ab")
            ]
        );
    }

    #[test]
    fn within_check_is_component_wise() {
        let w = |c: &str, a: &str| std::path::Path::new(c).starts_with(a);
        assert!(w("/a/b", "/a"));
        assert!(!w("/ab", "/a"));
        assert!(w("/a", "/a"));
    }
}
