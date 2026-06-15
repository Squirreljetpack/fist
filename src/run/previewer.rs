use crate::{
    aliases::MMState,
    run::{FsMatchmaker, dhandlers::path_formatter, state::STACK},
};
use cba::{env_vars, unwrap};
use log::warn;
use matchmaker::{
    config::PreviewerConfig,
    message::Event,
    preview::previewer::{PreviewMessage, Previewer},
};

/// Causes the program to display a preview of the active result.
/// The Previewer can be connected to [`Matchmaker`] using [`PickOptions::previewer`]
pub fn make_previewer(
    mm: &mut FsMatchmaker,
    previewer_config: PreviewerConfig,
) -> Previewer {
    // initialize previewer
    let (previewer, tx) = Previewer::new(previewer_config);
    let preview_tx = tx.clone();

    // preview handler
    mm.register_event_handler(Event::CursorChange | Event::PreviewChange, move |state: &mut MMState<'_, '_>, _e| {
        if state.preview_visible() &&
        let Some(item) = state.current_raw() &&
        let m = state.preview_payload() &&
        !m.is_empty()
        {
            let cmd = path_formatter(item, m);

            // get target line from col 3
            let target = if STACK::in_rg() {
                state.picker_ui.worker.format_with(item, "3").and_then(|t| atoi::atoi(t.as_bytes()))
            } else {
                None
            };
            state.preview_ui.as_mut().unwrap().set_target(target);

            let mut envs = state.make_env_vars();
            let extra = env_vars!(
                "COLUMNS" => state.previewer_area().map_or("0".to_string(), |r| r.width.to_string()),
                "LINES" => state.previewer_area().map_or("0".to_string(), |r| r.height.to_string()),
            );
            envs.extend(extra);
            if let Some(t) = target {
                envs.push(("HIGHLIGHT_LINE".to_string(), t.to_string()));
            }

            let msg = PreviewMessage::Run(cmd.clone(), envs);
            if preview_tx.send(msg.clone()).is_err() {
                warn!("Failed to send to preview: {}", msg)
            }
        } else if preview_tx.send(PreviewMessage::Stop).is_err() {
            warn!("Failed to send to preview: stop")
        }

        state.preview_set_payload = None;
    });

    mm.register_event_handler(Event::PreviewSet, move |state, _event| {
        if state.preview_visible() {
            let payload = state.preview_set_payload();
            log::trace!("Recieved PreviewSet: {payload:?}");
            let msg = match payload {
                Some(Err(m)) => {
                    // let m = if is_empty(&m) && !help_str.lines.is_empty() {
                    //     help_str.clone()
                    // } else {
                    //     m
                    // };
                    PreviewMessage::Set(m)
                }
                None => PreviewMessage::Unset,
                Some(Ok(template)) => {
                    let item = unwrap!(state.current_raw());
                    let cmd = path_formatter(item, &template);
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
