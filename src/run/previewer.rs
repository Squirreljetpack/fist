use crate::{
    aliases::MMState,
    run::{FsMatchmaker, item::PathItem, state::STACK},
};
use cba::env_vars;
use log::warn;
use matchmaker::{
    AttachmentFormatter,
    config::{HelpDisplayConfig, PreviewerConfig},
    message::Event,
    nucleo::Text,
    preview::previewer::{PreviewMessage, Previewer},
    use_formatter,
};

/// Causes the program to display a preview of the active result.
/// The Previewer can be connected to [`Matchmaker`] using [`PickOptions::previewer`]
pub fn make_previewer(
    mm: &mut FsMatchmaker,
    previewer_config: PreviewerConfig,
    formatter: AttachmentFormatter<PathItem, ()>,
    help_factory: Box<dyn Fn(&HelpDisplayConfig) -> Text<'static> + Send + Sync>,
) -> Previewer {
    // initialize previewer
    let (previewer, tx) = Previewer::new(previewer_config.clone());
    let preview_tx = tx.clone();
    let formatter_clone = formatter.clone();
    let help_config = previewer_config.help.clone();

    // preview handler
    // important that PreviewSet events don't accidentally trigger this!
    mm.register_event_handler(
        Event::CursorChange | Event::PreviewChange | Event::Synced,
        move |state: &mut MMState<'_,>, _e| {
            // don't clobber previewset events
            if state.contains(Event::PreviewSet) {
                return;
            }

            if state.preview_visible()
                && let m = state.preview_payload().clone()
                && let cmd = use_formatter(&formatter, state, &m, None)
                && !cmd.is_empty()
            {
                // rg panes: jump the preview to the match line
                let target = STACK::in_rg().then(|| {
                    state.current_raw().filter(|it| it.value() != u64::MAX).map(|item| item.loc().0 as isize)
                }).flatten();
                if let Some(p) = state.preview_ui {
                    p.set_target(target);
                    p.jump = Default::default();
                };

                let mut envs = state.make_env_vars();
                let extra = env_vars!(
                    "COLUMNS" => state.previewer_area().map_or("0".to_string(), |r| r.width.to_string()),
                    "LINES" => state.previewer_area().map_or("0".to_string(), |r| r.height.to_string()),
                );
                envs.extend(extra);
                if let Some(t) = target {
                    envs.push(("HIGHLIGHT_LINE".to_string(), t.to_string()));
                    if let Some(item) = state.current_raw() {
                        let (_, col) = item.loc();
                        if col != 0 {
                            envs.push(("HIGHLIGHT_COLUMN".to_string(), col.to_string()));
                        }
                    }
                }

                let msg = PreviewMessage::Run(cmd, envs);
                if preview_tx.send(msg.clone()).is_err() {
                    warn!("Failed to send to preview: {}", msg)
                }
            } else if preview_tx.send(PreviewMessage::Stop).is_err() {
                warn!("Failed to send to preview: stop")
            }

            state.preview_set_payload = None; // reset None here instead of on consume so that ::Help can toggle
        },
    );

    mm.register_event_handler(Event::PreviewSet, move |state, _event| {
        if state.preview_visible() {
            let payload = state.preview_set_payload();
            log::trace!("Recieved PreviewSet: {payload:?}");
            let msg = match payload {
                Some(Err(m)) => {
                    let m = if m.to_string().trim().is_empty() {
                        help_factory(&help_config)
                    } else {
                        m
                    };
                    PreviewMessage::Set(m)
                }
                None => PreviewMessage::Unset,
                Some(Ok(template)) => {
                    let cmd = use_formatter(&formatter_clone, state, &template, None);
                    if cmd.is_empty() {
                        PreviewMessage::Stop
                    } else {
                        let mut envs = state.make_env_vars();
                        let extra = env_vars!(
                            "COLUMNS" => state.previewer_area().map_or("0".to_string(), |r| r.width.to_string()),
                            "LINES" => state.previewer_area().map_or("0".to_string(), |r| r.height.to_string()),
                        );
                        envs.extend(extra);
                        PreviewMessage::Run(cmd, envs)
                    }
                }
            };

            if tx.send(msg.clone()).is_err() {
                warn!("Failed to send: {}", msg)
            }
        }
    });

    previewer
}
