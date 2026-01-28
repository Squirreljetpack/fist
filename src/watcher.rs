use crate::run::action::FsAction;
use matchmaker::{action::Action, event::RenderSender, message::RenderCommand};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
    event::ModifyKind,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::watch;

// ----------------- WatcherMessage -----------------
#[derive(Debug)]
pub enum WatcherMessage {
    Switch(PathBuf, RecursiveMode),
    Pause,
}

// ----------------- WatcherConfig -----------------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatcherConfig {
    /// Filesystem poll interval
    #[serde(with = "serde_duration_ms")]
    pub fs_poll_ms: Duration,
    /// Drop events within this interval
    #[serde(with = "serde_duration_ms")]
    pub debounce_ms: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            fs_poll_ms: Duration::from_secs(2),
            debounce_ms: Duration::from_millis(200),
        }
    }
}

// ----------------- Watcher -----------------
pub struct FsWatcher {
    path_rx: watch::Receiver<WatcherMessage>,
    current_path: Option<PathBuf>,
    pub config: WatcherConfig,
    render_tx: RenderSender<FsAction>,
}

pub type WatcherSender = watch::Sender<WatcherMessage>;

impl FsWatcher {
    /// Creates a new Watcher.
    pub fn new(
        config: WatcherConfig,
        render_tx: RenderSender<FsAction>,
    ) -> (Self, WatcherSender) {
        let (path_tx, path_rx) = watch::channel(WatcherMessage::Pause);
        let watcher_struct = Self {
            path_rx,
            current_path: None,
            config,
            render_tx,
        };
        (watcher_struct, path_tx)
    }

    // todo: directly send events to thread to impl edge debouncing
    /// Start the filesystem watcher on a seperate thread, then listen for events to change the watched directory.
    pub fn spawn(mut self) -> notify::Result<()> {
        // create config
        let notify_config = Config::default().with_poll_interval(self.config.fs_poll_ms);
        let mut last_event = Instant::now();

        // start watcher
        let mut watcher = RecommendedWatcher::new(
            {
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        match event.kind {
                            EventKind::Create(_)
                            | EventKind::Modify(ModifyKind::Name(_))
                            | EventKind::Remove(_) => {
                                log::debug!("WatcherEvent: {:?}", event.kind);

                                // todo: async to preserve last event
                                let now = Instant::now();
                                if now.duration_since(last_event) < self.config.debounce_ms {
                                    return;
                                }
                                last_event = now;

                                let _ = self.render_tx.send(RenderCommand::Action(Action::Custom(
                                    FsAction::SaveInput,
                                )));
                                let _ = self
                                    .render_tx
                                    .send(RenderCommand::Action(Action::Custom(FsAction::Reload)));
                            }
                            _ => {}
                        }
                    }
                }
            },
            notify_config,
        )?;

        tokio::spawn(async move {
            while self.path_rx.changed().await.is_ok() {
                match &*self.path_rx.borrow() {
                    WatcherMessage::Switch(new_path, recursive_mode) => {
                        match &mut self.current_path {
                            None => {
                                // Watch new directory non-recursively
                                let _ = watcher.watch(new_path, *recursive_mode);
                                self.current_path = Some(new_path.clone());

                                log::debug!("Watching: {:?}", new_path);
                            }
                            Some(old_path) => {
                                if new_path != old_path {
                                    let _ = watcher.unwatch(old_path);

                                    // Watch new directory non-recursively
                                    let _ = watcher.watch(new_path, *recursive_mode);
                                    *old_path = new_path.clone();

                                    log::debug!("Watching: {:?}", new_path);
                                }
                            }
                        }
                    }
                    WatcherMessage::Pause => {
                        if let Some(old_path) = self.current_path.take() {
                            let _ = watcher.unwatch(&old_path);
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

// ----------- SERDE ----------------------

mod serde_duration_ms {
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
