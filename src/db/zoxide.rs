use chrono::Utc;
use cli_boilerplate_automation::{
    bait::ResultExt, bog::BogOkExt, impl_transparent_wrapper, prints,
};
use std::path::Path;

use crate::{
    abspath::AbsPath,
    db::{Connection, DbSortOrder, Entry, Epoch},
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    /// Ignore files matching these globs
    pub exclude: Vec<String>,
    /// Whether to show missing files in queries.
    /// This is set to false by the binary when called with the "--cd" flag.
    pub show_missing: bool,
    /// Lazily remove nonexistant entries older than this many days
    pub missing_expiry: TtlDays,

    /// Lazily remove entries older than this many days
    pub atime_expiry: TtlDays,

    /// What to do when the best match by [`Connection::print_best_by_frecency`] is the current directory
    pub refind: RetryStrat,

    /// Whether to save resolved paths (todo)
    pub resolve_symlinks: bool,
    /// Experimental
    pub base_dir: Option<String>,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            exclude: Default::default(),
            show_missing: Default::default(),
            missing_expiry: TtlDays(7),
            resolve_symlinks: Default::default(),
            atime_expiry: Default::default(),
            base_dir: Default::default(),
            refind: Default::default(),
        }
    }
}

impl Connection {
    /// some optimizations on the [`Self::get_entries`] for faster printing
    /// Abuses RetryStrat as a signal: Next => Success, None => NoMatch
    pub async fn print_best_by_frecency(
        mut self,
        db_filter: &DbFilter,
    ) -> RetryStrat {
        let mut remove = Vec::new();
        let mut found = None;

        let mut entries = self
            .get_entries_range(0, 0, DbSortOrder::none)
            .await
            .__ebog();

        entries.sort_by_key(|e| std::cmp::Reverse(db_filter.score(e)));

        for e in entries {
            match db_filter.filter(&e.path, e.atime) {
                None => {
                    remove.push(e.path.clone());
                }
                Some(true) => {
                    if let Ok(cwd) = std::env::current_dir()
                        && cwd.as_path() == e.path.as_path()
                    {
                        match db_filter.refind {
                            RetryStrat::Next => continue,
                            RetryStrat::None => {}
                            RetryStrat::Search => {
                                if !remove.is_empty() {
                                    tokio::spawn(async move {
                                        self.remove_entries(&remove).await._elog();
                                    });
                                }
                                return RetryStrat::Search;
                            }
                        }
                    };
                    prints!(e.path.to_string_lossy());
                    found = Some(e.path);
                    break;
                }
                Some(false) => {}
            }
        }

        if let Some(p) = found.as_ref() {
            self.bump(p, 1).await._elog();
        };
        if !remove.is_empty() {
            self.remove_entries(&remove).await._elog();
        }

        if found.is_some() {
            RetryStrat::Next
        } else {
            RetryStrat::None
        }
    }

    // None -> cwd is match
    // Some(None) -> No match
    pub async fn return_best_by_frecency(
        mut self,
        db_filter: &DbFilter,
    ) -> Option<Option<AbsPath>> {
        let mut remove = Vec::new();
        let mut found = None;

        let mut entries = self
            .get_entries_range(0, 0, DbSortOrder::none)
            .await
            .__ebog();

        entries.sort_by_key(|e| std::cmp::Reverse(db_filter.score(e)));

        for e in entries {
            match db_filter.filter(&e.path, e.atime) {
                None => {
                    remove.push(e.path.clone());
                }
                Some(true) => {
                    if let Ok(cwd) = std::env::current_dir()
                        && cwd.as_path() == e.path.as_path()
                    {
                        match db_filter.refind {
                            RetryStrat::Next => continue,
                            RetryStrat::None => {}
                            RetryStrat::Search => {
                                if !remove.is_empty() {
                                    tokio::spawn(async move {
                                        self.remove_entries(&remove).await._elog();
                                    });
                                }
                                return None;
                            }
                        }
                    };
                    found = Some(e.path);
                    break;
                }
                Some(false) => {}
            }
        }

        let _found = found.clone();

        tokio::spawn(async move {
            if let Some(p) = _found.as_ref() {
                self.bump(p, 1).await._elog();
            };
            if !remove.is_empty() {
                self.remove_entries(&remove).await._elog();
            }
        });

        Some(found)
    }
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
    pub keywords: Vec<String>,
    // exclude: GlobSet,
    atime_expiration: Epoch,

    pub show_missing: bool,
    pub missing_expiration: Epoch,
    pub resolve_symlinks: bool,
    /// filter_before is stored as epoch seconds
    base_dir: Option<String>,
    pub refind: RetryStrat,
}

impl DbFilter {
    pub fn new(config: &HistoryConfig) -> Self {
        let now = Utc::now().timestamp();
        let atime_expiry = now - config.atime_expiry.0 * 24 * 60 * 60; // convert TTL days to seconds
        let missing_expiry = now - config.missing_expiry.0 * 24 * 60 * 60; // convert TTL days to seconds

        DbFilter {
            now,
            keywords: Default::default(),
            // exclude,
            atime_expiration: atime_expiry,

            show_missing: config.show_missing,
            missing_expiration: missing_expiry,
            resolve_symlinks: config.resolve_symlinks,
            refind: config.refind,
            base_dir: config.base_dir.clone(),
        }
    }

    pub fn with_keywords(
        mut self,
        keywords: Vec<String>,
    ) -> Self {
        self.keywords = keywords;
        self
    }
    pub fn keywords(
        &mut self,
        keywords: Vec<String>,
    ) -> &mut Self {
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
        if atime <= self.atime_expiration {
            log::debug!("filtered by: atime");
            return None;
        }
        if !self.filter_by_keywords(path) {
            log::debug!("filtered by: kw");
            return Some(false);
        }
        if !self.filter_by_exists(path) {
            log::debug!("filtered by: exist");
            return if atime <= self.missing_expiration {
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
        if self.show_missing {
            return true;
        }
        path.exists()

        // let resolver = if self.resolve_symlinks {
        //     std::fs::symlink_metadata
        // } else {
        //     std::fs::metadata
        // };

        // resolver(path).map(|meta| meta.is_dir()).unwrap_or(false)
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

// Transparent wrappers for type safety
impl_transparent_wrapper!(TtlDays, i64, 90);
