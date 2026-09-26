use crate::run::action::FsAction;
use crate::run::state::{TOAST, ToastStyle};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use matchmaker::{action::Action, event::RenderSender, message::RenderCommand, nucleo::Span};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
    event::ModifyKind,
};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::mpsc;

// ----------------- WatcherMessage -----------------
#[derive(Debug)]
pub enum WatcherMessage {
    Switch(PathBuf, WatchOptions),
    /// Watch a directory whose event storms must still produce reloads:
    /// thrash throttling stays disabled and the watch stays nonrecursive
    /// until the watcher pauses or is switched to a path outside the
    /// directory. Debouncing still collapses event storms into single
    /// reloads.
    MustWatch(PathBuf),
    Reload(Vec<PathBuf>),
    Pause,
}

// ----------------- WatchOptions -----------------
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WatchOptions {
    /// Show hidden files (true = do not filter; false = drop events if any component relative to root starts with '.')
    pub hidden: bool,
    /// Hide ignored files (true = drop events matching .gitignore; false = do not filter)
    pub ignore: bool,
    /// Recursive mode (true = Recursive, false = NonRecursive)
    pub recursive: bool,
}

pub fn build_gitignore(root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    let mut curr = Some(root);
    let mut gitignores = Vec::new();
    while let Some(dir) = curr {
        let gi = dir.join(".gitignore");
        if gi.is_file() {
            gitignores.push(gi);
        }
        if dir.join(".git").exists() {
            break;
        }
        curr = dir.parent();
    }
    for gi in gitignores.into_iter().rev() {
        let _ = builder.add(&gi);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn is_path_ignored(
    path: &Path,
    root: &Path,
    options: &WatchOptions,
    gitignore: Option<&Gitignore>,
) -> bool {
    if !options.hidden {
        let rel = path.strip_prefix(root).unwrap_or(path);
        if rel
            .components()
            .any(|c| c.as_os_str().to_str().is_some_and(|s| s.starts_with('.')))
        {
            return true;
        }
    }
    if options.ignore {
        if let Some(gi) = gitignore {
            if gi
                .matched_path_or_any_parents(path, path.is_dir())
                .is_ignore()
            {
                return true;
            }
        }
    }
    false
}

// ----------------- WatcherConfig -----------------
/// Thrash throttle: when `count` or more filesystem events land within
/// `duration`, the watcher stops emitting reloads until the filesystem has
/// been quiet for `resume_delay`, then emits one authoritative reload.
/// Bounds recompute storms
/// (auto-save, periodic build output).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThrashSetting {
    /// Number of events within `duration` that trips the throttle.
    pub count: usize,
    /// Sliding window for counting events.
    #[serde(with = "serde_duration_ms")]
    pub duration_ms: Duration,
    /// Quiet period after the last event before processing resumes.
    #[serde(with = "serde_duration_ms")]
    pub resume_delay_ms: Duration,
}

impl Default for ThrashSetting {
    fn default() -> Self {
        Self {
            count: 5,
            duration_ms: Duration::from_millis(5000),
            resume_delay_ms: Duration::from_millis(10000),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatcherConfig {
    /// Filesystem poll interval
    #[serde(with = "serde_duration_ms")]
    pub fs_poll_ms: Duration,
    /// Drop events within this interval
    #[serde(with = "serde_duration_ms")]
    pub debounce_ms: Duration,
    /// Event-storm throttle (see [`ThrashSetting`])
    #[serde(default)]
    pub thrash_threshold: ThrashSetting,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            fs_poll_ms: Duration::from_secs(2),
            debounce_ms: Duration::from_millis(100),
            thrash_threshold: Default::default(),
        }
    }
}

// ----------------- Watcher -----------------
pub struct FsWatcher {
    path_rx: mpsc::UnboundedReceiver<WatcherMessage>,
    path_tx: mpsc::UnboundedSender<WatcherMessage>,
    current_path: Option<PathBuf>,
    must_watch: Option<PathBuf>,
    pub config: WatcherConfig,
    render_tx: RenderSender<FsAction>,
}

pub type WatcherSender = mpsc::UnboundedSender<WatcherMessage>;

impl FsWatcher {
    /// Creates a new Watcher.
    pub fn new(
        config: WatcherConfig,
        render_tx: RenderSender<FsAction>,
    ) -> (Self, WatcherSender) {
        let (path_tx, path_rx) = mpsc::unbounded_channel();
        let watcher_struct = Self {
            path_rx,
            path_tx: path_tx.clone(),
            current_path: None,
            must_watch: None,
            config,
            render_tx,
        };
        (watcher_struct, path_tx)
    }

    // start watcher, returning a handle
    pub fn start_watcher(&self) -> Result<RecommendedWatcher, notify::Error> {
        let watcher_tx = self.path_tx.clone();
        let notify_config = Config::default().with_poll_interval(self.config.fs_poll_ms);

        RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Create(_)
                        | EventKind::Modify(ModifyKind::Name(_))
                        | EventKind::Remove(_) => {
                            log::debug!("WatcherEvent: {:?} paths: {:?}", event.kind, event.paths);
                            let _ = watcher_tx.send(WatcherMessage::Reload(event.paths));
                        }
                        _ => {}
                    }
                }
            },
            notify_config,
        )
    }

    /// Start the filesystem watcher on a separate thread, then listen for events to change the watched directory.
    pub fn spawn(mut self) -> notify::Result<()> {
        let mut watcher = self.start_watcher()?;

        tokio::spawn(async move {
            const FAR_FUTURE: Duration = Duration::from_secs(3600 * 24 * 365);
            let far_future = || tokio::time::Instant::now() + FAR_FUTURE;

            let debounce_timer = tokio::time::sleep(FAR_FUTURE);
            tokio::pin!(debounce_timer);
            let mut pending_reload = false;

            // thrash throttle: raw event timestamps within the sliding
            // window, plus a resume timer that is (re)armed on every event
            // while throttled — so it fires resume_delay after the FS goes
            // quiet, and the settle reload is the one authoritative
            // recompute per storm.
            let thrash = self.config.thrash_threshold.clone();
            let mut events: VecDeque<tokio::time::Instant> = VecDeque::new();
            let resume_timer = tokio::time::sleep(FAR_FUTURE);
            tokio::pin!(resume_timer);
            let mut throttled = false;

            let mut current_options: Option<WatchOptions> = None;
            let mut current_gitignore: Option<Gitignore> = None;
            let mut is_paused = true;

            loop {
                tokio::select! {
                    msg = self.path_rx.recv() => {
                        let Some(msg) = msg else { break };
                        match msg {
                            WatcherMessage::MustWatch(path) => {
                                self.must_watch = Some(path);
                                throttled = false;
                                pending_reload = false;
                                events.clear();
                                resume_timer.as_mut().reset(far_future());
                            }
                            WatcherMessage::Switch(new_path, options) => {
                                let same_path = self.current_path.as_ref() == Some(&new_path);
                                let recursive_mode = if self
                                    .must_watch
                                    .as_ref()
                                    .is_some_and(|mw| new_path.starts_with(mw))
                                {
                                    RecursiveMode::NonRecursive
                                } else if options.recursive {
                                    RecursiveMode::Recursive
                                } else {
                                    RecursiveMode::NonRecursive
                                };

                                if !same_path {
                                    if let Some(old_path) = self.current_path.take() {
                                        if !is_paused {
                                            let _ = watcher.unwatch(&old_path);
                                        }
                                    }
                                    let _ = watcher.watch(&new_path, recursive_mode);
                                    is_paused = false;
                                    self.current_path = Some(new_path.clone());
                                    log::debug!("Watching: {:?}", new_path);

                                    pending_reload = false;
                                    events.clear();
                                    throttled = false;
                                    debounce_timer.as_mut().reset(far_future());
                                    resume_timer.as_mut().reset(far_future());
                                } else {
                                    if is_paused {
                                        let _ = watcher.watch(&new_path, recursive_mode);
                                        is_paused = false;
                                    } else if current_options.as_ref().map(|o| o.recursive)
                                        != Some(options.recursive)
                                    {
                                        let _ = watcher.unwatch(&new_path);
                                        let _ = watcher.watch(&new_path, recursive_mode);
                                    }
                                }

                                if self.must_watch.as_ref().is_some_and(|mw| !new_path.starts_with(mw)) {
                                    self.must_watch = None;
                                }

                                current_gitignore = if options.ignore {
                                    Some(build_gitignore(&new_path))
                                } else {
                                    None
                                };
                                current_options = Some(options);
                            }
                            WatcherMessage::Pause => {
                                if !is_paused {
                                    if let Some(old_path) = &self.current_path {
                                        let _ = watcher.unwatch(old_path);
                                    }
                                    is_paused = true;
                                }
                                self.must_watch = None;
                                pending_reload = false;
                                debounce_timer.as_mut().reset(far_future());
                            }
                            WatcherMessage::Reload(paths) => {
                                if is_paused {
                                    continue;
                                }
                                let Some(current_path) = &self.current_path else {
                                    continue;
                                };
                                let Some(options) = &current_options else {
                                    continue;
                                };

                                if !paths.is_empty()
                                    && paths.iter().all(|p| {
                                        is_path_ignored(
                                            p,
                                            current_path,
                                            options,
                                            current_gitignore.as_ref(),
                                        )
                                    })
                                {
                                    continue;
                                }

                                let now = tokio::time::Instant::now();

                                if self.must_watch.is_some() {
                                    pending_reload = true;
                                    debounce_timer.as_mut().reset(now + self.config.debounce_ms);
                                } else if throttled {
                                    resume_timer.as_mut().reset(now + thrash.resume_delay_ms);
                                    continue;
                                } else {
                                    pending_reload = true;
                                    debounce_timer.as_mut().reset(now + self.config.debounce_ms);
                                }
                            }
                        }
                    }
                    _ = &mut debounce_timer, if pending_reload && !throttled => {
                        pending_reload = false;
                        debounce_timer.as_mut().reset(far_future());

                        let now = tokio::time::Instant::now();
                        while events.front().is_some_and(|t| now - *t > thrash.duration_ms) {
                            events.pop_front();
                        }
                        events.push_back(now);

                        if self.must_watch.is_none() && events.len() >= thrash.count {
                            log::debug!(
                                "Watcher throttling: {} reloads in {:?}",
                                events.len(),
                                thrash.duration_ms
                            );
                            throttled = true;
                            resume_timer.as_mut().reset(now + thrash.resume_delay_ms);
                            TOAST::replace(
                                ToastStyle::Normal,
                                "file watcher ",
                                Span::styled("paused", ToastStyle::Warning),
                            );
                            continue;
                        }

                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::SaveInput)));
                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::Reload)));
                    }
                    _ = &mut resume_timer, if throttled => {
                        // the FS settled after a storm: one authoritative reload
                        throttled = false;
                        events.clear();
                        resume_timer.as_mut().reset(far_future());

                        TOAST::replace(
                            ToastStyle::Normal,
                            "file watcher ",
                            Span::styled("resumed", ToastStyle::Info),
                        );

                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::SaveInput)));
                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::Reload)));
                    }
                }
            }
        });
        Ok(())
    }
}

// ----------- SERDE ----------------------

pub mod serde_duration_ms {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(
        duration: &Duration,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ms = duration.as_millis() as u64;
        serializer.serialize_u64(ms)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_thrash_prevention_throttles_repeated_events_on_same_directory() {
        let (render_tx, mut render_rx) = mpsc::unbounded_channel();
        let config = WatcherConfig {
            fs_poll_ms: Duration::from_millis(100),
            debounce_ms: Duration::from_millis(20),
            thrash_threshold: ThrashSetting {
                count: 3,
                duration_ms: Duration::from_millis(1000),
                resume_delay_ms: Duration::from_millis(150),
            },
        };

        let (watcher, tx) = FsWatcher::new(config, render_tx);
        watcher.spawn().unwrap();

        let temp_dir = std::env::temp_dir();
        tx.send(WatcherMessage::Switch(
            temp_dir.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // 1st reload: debounces and emits reload
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        let cmd1 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd1,
            RenderCommand::Action(Action::Custom(FsAction::SaveInput))
        ));
        let cmd2 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd2,
            RenderCommand::Action(Action::Custom(FsAction::Reload))
        ));
        // Application reload populates and sends Switch for the same directory:
        tx.send(WatcherMessage::Switch(
            temp_dir.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // 2nd reload: debounces and emits reload
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        tx.send(WatcherMessage::Switch(
            temp_dir.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // 3rd reload: trips thrash threshold (count = 3). Reload is throttled!
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        // Should NOT emit reload within 100ms
        let throttled_check =
            tokio::time::timeout(Duration::from_millis(80), render_rx.recv()).await;
        assert!(
            throttled_check.is_err(),
            "Expected watcher to be throttled, but received: {:?}",
            throttled_check
        );

        // While throttled, more events arrive:
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        let throttled_check2 =
            tokio::time::timeout(Duration::from_millis(80), render_rx.recv()).await;
        assert!(
            throttled_check2.is_err(),
            "Expected watcher to stay throttled during event storm"
        );

        // After quiet period (> resume_delay_ms = 150ms), one authoritative reload is emitted
        let settle1 = tokio::time::timeout(Duration::from_millis(1000), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            settle1,
            RenderCommand::Action(Action::Custom(FsAction::SaveInput))
        ));
        let settle2 = tokio::time::timeout(Duration::from_millis(1000), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            settle2,
            RenderCommand::Action(Action::Custom(FsAction::Reload))
        ));
    }

    #[tokio::test]
    async fn test_thrash_prevention_resets_on_path_switch() {
        let (render_tx, mut render_rx) = mpsc::unbounded_channel();
        let config = WatcherConfig {
            fs_poll_ms: Duration::from_millis(100),
            debounce_ms: Duration::from_millis(20),
            thrash_threshold: ThrashSetting {
                count: 2,
                duration_ms: Duration::from_millis(1000),
                resume_delay_ms: Duration::from_millis(150),
            },
        };

        let (watcher, tx) = FsWatcher::new(config, render_tx);
        watcher.spawn().unwrap();

        let dir1 = std::env::temp_dir();
        let dir2 = std::env::temp_dir().join("fist_test_dir2");
        let _ = std::fs::create_dir_all(&dir2);

        tx.send(WatcherMessage::Switch(
            dir1.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // 1st reload on dir1: emitted
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();

        // Switch to dir2: should reset storm state
        tx.send(WatcherMessage::Switch(
            dir2.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // 1st reload on dir2: should NOT be throttled because count is 1 for dir2
        tx.send(WatcherMessage::Reload(vec![])).unwrap();
        let cmd1 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd1,
            RenderCommand::Action(Action::Custom(FsAction::SaveInput))
        ));
        let cmd2 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd2,
            RenderCommand::Action(Action::Custom(FsAction::Reload))
        ));

        let _ = std::fs::remove_dir_all(&dir2);
    }

    #[tokio::test]
    async fn test_must_watch_disables_throttling() {
        let (render_tx, mut render_rx) = mpsc::unbounded_channel();
        let config = WatcherConfig {
            fs_poll_ms: Duration::from_millis(100),
            debounce_ms: Duration::from_millis(20),
            thrash_threshold: ThrashSetting {
                count: 2,
                duration_ms: Duration::from_millis(1000),
                resume_delay_ms: Duration::from_millis(150),
            },
        };

        let (watcher, tx) = FsWatcher::new(config, render_tx);
        watcher.spawn().unwrap();

        let temp_dir = std::env::temp_dir();
        tx.send(WatcherMessage::MustWatch(temp_dir.clone()))
            .unwrap();
        tx.send(WatcherMessage::Switch(
            temp_dir.clone(),
            WatchOptions::default(),
        ))
        .unwrap();

        // Send 3 reloads (> count 2)
        for _ in 0..3 {
            tx.send(WatcherMessage::Reload(vec![])).unwrap();
            let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
                .await
                .unwrap()
                .unwrap();
            let _ = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
                .await
                .unwrap()
                .unwrap();
            tx.send(WatcherMessage::Switch(
                temp_dir.clone(),
                WatchOptions::default(),
            ))
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_watcher_filters_hidden_and_ignored_paths() {
        let (render_tx, mut render_rx) = mpsc::unbounded_channel();
        let config = WatcherConfig {
            fs_poll_ms: Duration::from_millis(100),
            debounce_ms: Duration::from_millis(20),
            thrash_threshold: ThrashSetting {
                count: 2,
                duration_ms: Duration::from_millis(1000),
                resume_delay_ms: Duration::from_millis(150),
            },
        };

        let (watcher, tx) = FsWatcher::new(config, render_tx);
        watcher.spawn().unwrap();

        let test_root = std::env::temp_dir().join("fist_test_watcher_filter");
        let _ = std::fs::create_dir_all(&test_root);
        let gitignore = test_root.join(".gitignore");
        std::fs::write(&gitignore, "target/\n*.tmp\n").unwrap();

        tx.send(WatcherMessage::Switch(
            test_root.clone(),
            WatchOptions {
                ignore: true,
                recursive: true,
                ..Default::default()
            },
        ))
        .unwrap();

        // Hidden event: should be dropped
        tx.send(WatcherMessage::Reload(vec![
            test_root.join(".git").join("index.lock"),
        ]))
        .unwrap();
        let res = tokio::time::timeout(Duration::from_millis(80), render_rx.recv()).await;
        assert!(res.is_err(), "Expected hidden path to be filtered");

        // Ignored event: should be dropped
        tx.send(WatcherMessage::Reload(vec![
            test_root.join("target").join("debug").join("app"),
        ]))
        .unwrap();
        let res = tokio::time::timeout(Duration::from_millis(80), render_rx.recv()).await;
        assert!(res.is_err(), "Expected target/ path to be filtered");

        // Normal event: should trigger reload
        tx.send(WatcherMessage::Reload(vec![
            test_root.join("src").join("main.rs"),
        ]))
        .unwrap();
        let cmd1 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd1,
            RenderCommand::Action(Action::Custom(FsAction::SaveInput))
        ));
        let cmd2 = tokio::time::timeout(Duration::from_millis(100), render_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            cmd2,
            RenderCommand::Action(Action::Custom(FsAction::Reload))
        ));

        let _ = std::fs::remove_dir_all(&test_root);
    }
}
