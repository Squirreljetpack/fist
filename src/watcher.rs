use crate::run::action::FsAction;
use matchmaker::{action::Action, event::RenderSender, message::RenderCommand};
use notify::{
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
    event::ModifyKind,
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::watch;

// ----------------- WatcherMessage -----------------
#[derive(Debug)]
pub enum WatcherMessage {
    Switch(PathBuf, RecursiveMode),
    Reload,
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
            debounce_ms: Duration::from_millis(100),
        }
    }
}

// ----------------- Watcher -----------------
pub struct FsWatcher {
    path_rx: watch::Receiver<WatcherMessage>,
    path_tx: watch::Sender<WatcherMessage>,
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
            path_tx: path_tx.clone(),
            current_path: None,
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

    /// Start the filesystem watcher on a seperate thread, then listen for events to change the watched directory.
    pub fn spawn(mut self) -> notify::Result<()> {
        let mut watcher = self.start_watcher()?;

        tokio::spawn(async move {
            let debounce_timer = tokio::time::sleep(Duration::from_secs(3600 * 24 * 365));
            tokio::pin!(debounce_timer);
            let mut pending_reload = false;

            loop {
                tokio::select! {
                    res = self.path_rx.changed() => {
                        if res.is_err() { break; }
                        let msg = self.path_rx.borrow_and_update();
                        match &*msg {
                            WatcherMessage::Switch(new_path, recursive_mode) => {
                                match &mut self.current_path {
                                    None => {
                                        let _ = watcher.watch(new_path, *recursive_mode);
                                        self.current_path = Some(new_path.clone());
                                        log::debug!("Watching: {:?}", new_path);
                                    }
                                    Some(old_path) => {
                                        if new_path != old_path {
                                            let _ = watcher.unwatch(old_path);
                                            let _ = watcher.watch(new_path, *recursive_mode);
                                            *old_path = new_path.clone();
                                            log::debug!("Watching: {:?}", new_path);
                                        }
                                    }
                                }
                                pending_reload = false
                            }
                            WatcherMessage::Pause => {
                                if let Some(old_path) = self.current_path.take() {
                                    let _ = watcher.unwatch(&old_path);
                                }
                                pending_reload = false
                            }
                            WatcherMessage::Reload => {
                                pending_reload = true;
                                debounce_timer.as_mut().reset(tokio::time::Instant::now() + self.config.debounce_ms);
                            }
                        }
                    }
                    _ = &mut debounce_timer, if pending_reload => {
                        // debounce_timer expires, send events
                        pending_reload = false;
                        debounce_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(3600 * 24 * 365));

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
