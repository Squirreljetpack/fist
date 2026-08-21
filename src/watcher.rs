use crate::run::action::FsAction;
use matchmaker::{action::Action, event::RenderSender, message::RenderCommand};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
    event::ModifyKind,
};
use std::{collections::VecDeque, path::PathBuf, time::Duration};
use tokio::sync::mpsc;

// ----------------- WatcherMessage -----------------
#[derive(Debug)]
pub enum WatcherMessage {
    Switch(PathBuf, RecursiveMode),
    /// Watch a directory whose event storms must still produce reloads:
    /// thrash throttling stays disabled and the watch stays nonrecursive
    /// until the watcher pauses or is switched to a path outside the
    /// directory. Debouncing still collapses event storms into single
    /// reloads.
    MustWatch(PathBuf),
    Reload,
    Pause,
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
    /// Set by `MustWatch`: thrash throttling is disabled while this dir is
    /// being watched.
    must_watch: Option<PathBuf>,
    pub config: WatcherConfig,
    render_tx: RenderSender<FsAction>,
}

pub type WatcherSender = mpsc::UnboundedSender<WatcherMessage>;

impl FsWatcher {
    /// Creates a new Watcher.
    pub fn new(config: WatcherConfig, render_tx: RenderSender<FsAction>) -> (Self, WatcherSender) {
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
                            log::debug!("WatcherEvent: {:?}", event.kind);
                            let _ = watcher_tx.send(WatcherMessage::Reload);
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

            loop {
                tokio::select! {
                    msg = self.path_rx.recv() => {
                        let Some(msg) = msg else { break };
                        // ordered delivery matters: MustWatch must precede
                        // the Switch that follows it for the same dir
                        match msg {
                            WatcherMessage::MustWatch(path) => {
                                self.must_watch = Some(path);
                                // thrash accounting is suspended until we
                                // leave the dir or pause
                                throttled = false;
                                pending_reload = false;
                                events.clear();
                                resume_timer.as_mut().reset(far_future());
                            }
                            WatcherMessage::Switch(new_path, recursive_mode) => {
                                // while inside the must-watch dir the watch
                                // stays nonrecursive
                                let recursive_mode = if self
                                    .must_watch
                                    .as_ref()
                                    .is_some_and(|mw| new_path.starts_with(mw))
                                {
                                    RecursiveMode::NonRecursive
                                } else {
                                    recursive_mode
                                };
                                match &mut self.current_path {
                                    None => {
                                        let _ = watcher.watch(&new_path, recursive_mode);
                                        self.current_path = Some(new_path.clone());
                                        log::debug!("Watching: {:?}", new_path);
                                    }
                                    Some(old_path) => {
                                        if &new_path != old_path {
                                            let _ = watcher.unwatch(old_path);
                                            let _ = watcher.watch(&new_path, recursive_mode);
                                            *old_path = new_path.clone();
                                            log::debug!("Watching: {:?}", new_path);
                                        }
                                    }
                                }
                                // the old storm belongs to the old path
                                pending_reload = false;
                                events.clear();
                                throttled = false;

                                // leaving the must-watch dir re-enables
                                // thrash throttling
                                if self.must_watch.as_ref().is_some_and(|mw| !new_path.starts_with(mw)) {
                                    self.must_watch = None;
                                }
                            }
                            WatcherMessage::Pause => {
                                if let Some(old_path) = self.current_path.take() {
                                    let _ = watcher.unwatch(&old_path);
                                }
                                self.must_watch = None;
                                pending_reload = false;
                                events.clear();
                                throttled = false;
                            }
                            WatcherMessage::Reload => {
                                let now = tokio::time::Instant::now();

                                if self.must_watch.is_some() {
                                    // must-watch: thrash throttling is
                                    // disabled — storms still collapse into
                                    // debounced reloads
                                    pending_reload = true;
                                    debounce_timer.as_mut().reset(now + self.config.debounce_ms);
                                } else if throttled {
                                    // storm ongoing: stay quiet; resume once
                                    // the FS has settled
                                    resume_timer.as_mut().reset(now + thrash.resume_delay_ms);
                                    continue;
                                } else {
                                    // slide the window, count this event
                                    while events.front().is_some_and(|t| now - *t > thrash.duration_ms) {
                                        events.pop_front();
                                    }
                                    events.push_back(now);

                                    if events.len() >= thrash.count {
                                        // threshold tripped: drop the pending
                                        // debounced reload, go quiet
                                        log::debug!(
                                            "Watcher throttling: {} events in {:?}",
                                            events.len(),
                                            thrash.duration_ms
                                        );
                                        throttled = true;
                                        pending_reload = false;
                                        debounce_timer.as_mut().reset(far_future());
                                        resume_timer.as_mut().reset(now + thrash.resume_delay_ms);
                                    } else {
                                        pending_reload = true;
                                        debounce_timer.as_mut().reset(now + self.config.debounce_ms);
                                    }
                                }
                            }
                        }
                    }
                    _ = &mut debounce_timer, if pending_reload && !throttled => {
                        // debounce window closed without tripping the throttle
                        pending_reload = false;
                        debounce_timer.as_mut().reset(far_future());

                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::SaveInput)));
                        let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(FsAction::Reload)));
                    }
                    _ = &mut resume_timer, if throttled => {
                        // the FS settled after a storm: one authoritative reload
                        throttled = false;
                        events.clear();
                        resume_timer.as_mut().reset(far_future());

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

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
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
