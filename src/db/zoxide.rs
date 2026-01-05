use chrono::Utc;
use cli_boilerplate_automation::impl_transparent_wrapper;
use std::path::Path;

use crate::db::{Entry, Epoch};

/// s2 maps monotonically and injectively into s1
fn is_monotonic_substring(
    s1: &str,
    s2: &str,
) -> bool {
    let mut prev_index = 0;
    let s1_chars: Vec<char> = s1.chars().collect();

    for c in s2.chars() {
        let mut found = false;
        while prev_index < s1_chars.len() {
            if s1_chars[prev_index] == c {
                found = true;
                prev_index += 1;
                break;
            }
            prev_index += 1;
        }
        if !found {
            return false;
        }
    }
    true
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DbConfig {
    pub exclude: Vec<String>,
    pub filter_missing: bool,
    pub remove_missing: bool,
    pub resolve_symlinks: bool,
    pub filter_before: TtlDays,
    pub base_dir: Option<String>,
    pub refind: RetryStrat,
}

#[derive(Default, Copy, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// What to do when the best match by [`crate::db::Connection::print_best_by_frecency`] is the current directory
pub enum RetryStrat {
    /// search interface
    Search,
    #[default]
    /// keep looking
    Next,
    /// return anyway (the current directory)
    None,
}

/// History filter/matcher
#[derive(Debug, Clone)]
pub struct DbFilter {
    now: Epoch,
    keywords: Vec<String>,
    // exclude: GlobSet,
    filter_before: Epoch,

    filter_missing: bool,
    remove_missing: bool,
    resolve_symlinks: bool,
    /// filter_before is stored as epoch seconds
    base_dir: Option<String>,
    pub refind: RetryStrat,
}

impl DbFilter {
    pub fn new(config: &DbConfig) -> Self {
        let now = Utc::now().timestamp();
        let filter_before = now - config.filter_before.0 * 24 * 60 * 60; // convert TTL days to seconds

        DbFilter {
            now,
            keywords: Default::default(),
            // exclude,
            filter_before,

            filter_missing: config.filter_missing,
            remove_missing: config.remove_missing,
            resolve_symlinks: config.resolve_symlinks,
            refind: config.refind,
            base_dir: config.base_dir.clone(),
        }
    }

    pub fn keywords(
        mut self,
        keywords: Vec<String>,
    ) -> Self {
        self.keywords = keywords;
        self
    }

    /// Filter an entry by path + last-access epoch
    /// Returns:
    /// Some(true) => keep
    /// Some(false) => exclude
    /// None => remove lazily
    pub fn filter(
        &self,
        path: &Path,
        atime: Epoch,
    ) -> Option<bool> {
        if !self.filter_by_base_dir(path) {
            log::debug!("filtered by: base");
            return Some(false);
        }
        // if !self.filter_by_exclude(path) {
        //     log::debug!("filtered by: exclude");
        //     return Some(false);
        // }
        if atime <= self.filter_before {
            log::debug!("filtered by: atime");
            return None;
        }
        if !self.filter_by_keywords(path) {
            log::debug!("filtered by: kw");
            return Some(false);
        }
        if !self.filter_by_exists(path) {
            log::debug!("filtered by: exist");
            return if self.remove_missing {
                None
            } else {
                Some(false)
            };
        }
        Some(true)
    }

    /// Compute a frecency score using epoch seconds
    pub fn score(
        &self,
        e: &Entry,
    ) -> i32 {
        let Entry { atime, count, .. } = e;
        let age_secs = self.now - atime;
        let count = *count * 4;

        if age_secs <= 60 * 60 {
            count * 4 // last hour
        } else if age_secs <= 60 * 60 * 24 {
            count * 2 // last day
        } else if age_secs <= 60 * 60 * 24 * 7 {
            count / 2 // last week
        } else {
            count / 4 // older
        }
    }

    fn filter_by_base_dir(
        &self,
        path: &Path,
    ) -> bool {
        match &self.base_dir {
            Some(base) => path.starts_with(base),
            None => true,
        }
    }

    // fn filter_by_exclude(
    //     &self,
    //     path: &Path,
    // ) -> bool {
    //     !self.exclude.is_match(path)
    // }

    fn filter_by_exists(
        &self,
        path: &Path,
    ) -> bool {
        if !self.filter_missing {
            return true;
        }

        let resolver = if self.resolve_symlinks {
            std::fs::symlink_metadata
        } else {
            std::fs::metadata
        };

        resolver(path).map(|meta| meta.is_dir()).unwrap_or(false)
    }

    /// zoxide algorithm with some adjustments:
    /// A key is matched by a path if it's letters maps to a component's letters in the same order (without needing to be consecutive).
    /// Keys containing path seperators imply consecutive matches in the path component
    fn filter_by_keywords(
        &self,
        path: &Path,
    ) -> bool {
        if self.keywords.is_empty() {
            return true;
        }

        let path_components: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect();

        let keywords_lower: Vec<_> = self
            .keywords
            .iter()
            .map(|k| k.trim_end_matches(std::path::MAIN_SEPARATOR).to_lowercase())
            .collect();

        let mut idx = path_components.len();

        // Last kw must succeed
        let mut first = true;

        for keyword in keywords_lower.iter().rev() {
            let key_components: Vec<_> = if keyword.contains(std::path::MAIN_SEPARATOR) {
                keyword.split(std::path::MAIN_SEPARATOR).collect()
            } else {
                vec![keyword.as_str()]
            };

            let mut found = false;
            for start in (0..idx).rev() {
                if start + key_components.len() > idx {
                    continue;
                }

                let slice = &path_components[start..start + key_components.len()];
                if slice
                    .iter()
                    .zip(key_components.iter())
                    .all(|(c, k)| is_monotonic_substring(c, k))
                {
                    idx = start;
                    found = true;
                    first = false;
                    break;
                } else if first {
                    return false;
                }
            }

            if !found {
                return false;
            }
        }

        true
    }
}

// Transparent wrappers for type safety
impl_transparent_wrapper!(TtlDays, i64, 90);
