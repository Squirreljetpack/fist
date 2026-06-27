use cba::{bait::ResultExt, bird::transform::camelcase_normalized, bog::BogOkExt, prints};

use std::path::Path;

use crate::{
    abspath::AbsPath,
    db::{Connection, DbSortOrder, DbTable, Entry, Epoch},
    errors::DbError,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    // --- filter / decay ---
    /// When checking for existence, follow symlinks to determine if type is consistent.
    pub resolve_symlinks: bool,
    /// The criterion by which a path is determined to match a given set of keywords
    pub query_strategy: QueryMatchStrategy,
    /// Default: smart (only if keyword contains capitalization)
    pub case_sensitive: Option<bool>,
    /// Decay constant λ for EMS scoring. Each tick, the stored score is
    /// multiplied by `exp(-λ × Δticks)` before the new visit count is added.
    /// Default: `Some(8e-3)`. When `None`, uses wall-clock atime scoring
    /// (compatible with zoxide databases).
    pub lambda: Option<f64>,

    #[serde(skip)]
    keywords: Vec<String>,
    // --- maintenence / bump ---
    /// Ignore files matching these globs
    pub exclude: Vec<String>,

    /// Maximum entries before pruning. When the total count exceeds this,
    /// entries beyond `prune_min` are removed from the database.
    pub prune_max: usize,
    /// Number of entries retained after a pruning pass.
    pub prune_min: usize,

    // --- other ---
    /// What to do when the best match by [`Connection::print_best_by_frecency`] is the current directory
    #[serde(deserialize_with = "camelcase_normalized")]
    pub refind: RetryStrat,

    // ---- unimplemented / experimental  ---
    /// Whether to show files that don't exist on the filesystem in queries.
    /// This is set to false by the binary when called with the "--cd" flag.
    pub show_missing: bool,
    /// Only track subpaths [of the given path]
    pub base_dir: Option<String>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum QueryMatchStrategy {
    Monotonic,
    #[default]
    Substring,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            resolve_symlinks: Default::default(),
            case_sensitive: Default::default(),
            query_strategy: Default::default(),
            lambda: Some(8e-3),
            keywords: Default::default(),

            exclude: Default::default(),
            prune_max: 10000,
            prune_min: 8000,

            refind: Default::default(),

            show_missing: Default::default(),
            base_dir: Default::default(),
        }
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

impl HistoryConfig {
    pub fn with_keywords(
        mut self,
        keywords: Vec<String>,
    ) -> Self {
        self.keywords = keywords;
        self
    }

    /// Filter an entry.
    /// Returns `true` to keep, `false` to exclude.
    pub fn filter(
        &self,
        entry: &Entry,
        table: DbTable,
    ) -> bool {
        let path = &entry.path;

        if !self.filter_by_base_dir(path) {
            log::debug!("filtered by: base");
            return false;
        }

        if !self.filter_by_keywords(path) {
            log::debug!("filtered by: kw");
            return false;
        }

        if !self.filter_by_exists(path, table) {
            log::debug!("{path:?} filtered by: exist");
            return false;
        }

        true
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

    fn filter_by_exists(
        &self,
        path: &Path,
        table: DbTable,
    ) -> bool {
        if self.show_missing {
            return true;
        }
        if self.resolve_symlinks {
            match table {
                DbTable::dirs | DbTable::files => std::fs::symlink_metadata(path)
                    .ok()
                    .map(|m| match table {
                        DbTable::dirs => m.is_dir(),
                        DbTable::files => m.is_file(),
                        _ => unreachable!(),
                    })
                    .unwrap_or(false),
                _ => path.exists(),
            }
        } else {
            path.exists()
        }
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
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();

        let keywords: Vec<_> = self
            .keywords
            .iter()
            .map(|k| k.trim_end_matches(std::path::MAIN_SEPARATOR))
            .collect();

        let matcher = |component: &str, keyword: &str| -> bool {
            let (comp, kw) = match self.case_sensitive {
                Some(true) => (component.to_string(), keyword.to_string()),
                Some(false) => (component.to_lowercase(), keyword.to_lowercase()),
                None => {
                    if keyword.chars().any(|ch| ch.is_uppercase()) {
                        (component.to_string(), keyword.to_string())
                    } else {
                        (component.to_lowercase(), keyword.to_string())
                    }
                }
            };

            log::trace!("filter comparing: {comp}, {kw}");

            match self.query_strategy {
                QueryMatchStrategy::Monotonic => is_monotonic_substring(&comp, &kw),
                QueryMatchStrategy::Substring => comp.contains(&kw),
            }
        };

        let mut idx = path_components.len();
        // last kw must succeed
        let mut first = true;

        for keyword in keywords.into_iter().rev() {
            let key_components: Vec<_> = if keyword.contains(std::path::MAIN_SEPARATOR) {
                keyword.split(std::path::MAIN_SEPARATOR).collect()
            } else {
                vec![keyword]
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
                    .all(|(c, k)| matcher(c, k))
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

fn decay(
    score: f64,
    now: Epoch,
    atime: Epoch,
    lambda: f64,
) -> f64 {
    let delta = now - atime;
    score * (-lambda * delta as f64).exp()
}

const FLOAT_TO_I32: f64 = 5000.0; // scalar to avoid losing precision

/// Sorting key: returns the frecency-like score as an integer.
///
/// atime (lambda=None): `count × bucketed_age_multiplier`.
/// EMS (lambda=Some(λ)): `stored_score × exp(-λ × Δticks)`, truncated to i32.
pub fn score(
    now: Epoch,
    e: &Entry,
    lambda: Option<f64>,
) -> i32 {
    if let Some(lambda) = lambda {
        (FLOAT_TO_I32 * decay(e.score, now, e.atime, lambda)) as i32
    } else {
        // bucketed from wall-clock age
        let Entry { atime, count, .. } = e;
        let age_secs = now - atime;
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

// --------------------------------------------
// Fetching with the filter

impl Connection {
    /// some optimizations on the [`Self::get_entries`] for faster printing
    ///
    /// Abuses RetryStrat as a signal:
    /// - Next: Found
    /// - None: No Match
    /// - Search: First match was the current directory and RetryStrat == Search
    pub async fn print_best_by_frecency(
        mut self,
        config: &HistoryConfig,
        table: DbTable,
    ) -> RetryStrat {
        let mut found = None;

        let entries = self
            .get_entries_range(0, 0, DbSortOrder::frecency)
            .await
            .__ebog();

        let maybe_cwd = std::env::current_dir().ok();

        for e in &entries {
            if !config.filter(e, table) {
                continue;
            }

            if let Some(cwd) = maybe_cwd.as_deref()
                && cwd == e.cmd.as_maybe_realpath().unwrap_or(&e.path)
            {
                match config.refind {
                    RetryStrat::Next => continue,
                    RetryStrat::None => {}
                    RetryStrat::Search => {
                        return RetryStrat::Search;
                    }
                }
            };
            prints!(e.path.to_string_lossy());
            if let Some(p) = e.cmd.as_maybe_realpath()
                && let Ok(rp) = e.path.canonicalize()
                && p != rp
            {
                self.set_cmd(&e.path, &rp.into()).await._elog();
            }
            found = Some(e.path.clone());

            break;
        }

        self.prune_tail(&entries, config.prune_max, config.prune_min)
            .await
            ._elog();

        if found.is_some() {
            RetryStrat::Next
        } else {
            RetryStrat::None
        }
    }

    // None -> no match/(cwd is match + refind = RetryStrat::Search)
    pub async fn return_best_by_frecency(
        mut self,
        config: &HistoryConfig,
        table: DbTable,
    ) -> Option<AbsPath> {
        let mut found = None;

        let entries = self
            .get_entries_range(0, 0, DbSortOrder::frecency)
            .await
            .__ebog();

        let maybe_cwd = std::env::current_dir().ok();

        for e in &entries {
            if !config.filter(e, table) {
                continue;
            }

            if let Some(cwd) = maybe_cwd.as_deref()
                && cwd == e.cmd.as_maybe_realpath().unwrap_or(&e.path)
            {
                match config.refind {
                    RetryStrat::Next => continue,
                    RetryStrat::None => {}
                    RetryStrat::Search => {
                        return None;
                    }
                }
            };
            found = Some(e.clone());
            break;
        }

        let _found = found.clone();
        let prune_max = config.prune_max;
        let prune_min = config.prune_min;

        tokio::spawn(async move {
            if let Some(e) = _found {
                if let Some(p) = e.cmd.as_maybe_realpath()
                    && let Ok(rp) = e.path.canonicalize()
                    && p != rp
                {
                    self.set_cmd(&e.path, &rp.into()).await._elog();
                }
                // bump because this opens up an interactive screen
                self.bump_entry(&e, 1).await._elog();
            };
            self.prune_tail(&entries, prune_max, prune_min)
                .await
                ._elog();
        });

        found.map(|e| e.path)
    }

    /// Sweep all entries and remove missing entries with score below threshold.
    /// This is a full sweep that checks filesystem existence for every entry.
    pub async fn prune_missing(
        &mut self,
        score_threshold: i32,
    ) -> Result<u64, DbError> {
        use crate::db::zoxide;
        use chrono::Utc;

        // Get all entries
        let entries = self.get_entries_range(0, 0, DbSortOrder::none).await?;

        // Compute 'now' for scoring
        let now = if self.lambda.is_some() {
            self.get_max_atime().await?
        } else {
            Utc::now().timestamp()
        };

        // Filter missing entries below threshold
        let mut to_remove = Vec::new();
        for entry in entries {
            if score_threshold == 0 || zoxide::score(now, &entry, self.lambda) < score_threshold {
                if !entry.path.exists() {
                    to_remove.push(entry.path);
                }
            }
        }

        // Remove entries
        self.remove_entries(&to_remove).await
    }
}
