use cli_boilerplate_automation::env_vars;
use log::warn;
use matchmaker::{config::PreviewerConfig, message::Event, preview::previewer::{PreviewMessage, Previewer}};
use ratatui::text::Text;

use crate::{aliases::MMState, run::{FsMatchmaker, dhandlers::mm_formatter}};

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
    mm.register_event_handler(Event::CursorChange | Event::PreviewChange, move |state: &mut MMState<'_, '_>, _| {
        if state.preview_visible() &&
        let Some(t) = state.current_raw() &&
        let m = state.preview_payload() &&
        !m.is_empty()
        {
            let cmd = mm_formatter(t, m);

            // unwrap allowed by visible
            let target = state.preview_ui.as_ref().unwrap().config.scroll.index.as_ref().and_then(|index_col| {
                state.current_raw().and_then(|item| {
                    state.picker_ui.worker.format_with(item, index_col).and_then(|t| t.parse::<isize>().ok())
                })
            }); // reset to 0 each time
            state.preview_ui.as_mut().unwrap().set_target(target.unwrap_or(0));


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
            let msg = if let Some(m) = state.preview_set_payload() {
                let m = Text::from(m);
                
                PreviewMessage::Set(m)
            } else {
                PreviewMessage::Unset
            };
            
            if tx.send(msg.clone()).is_err() {
                warn!("Failed to send: {}", msg)
            }
        }
    });
    
    previewer
}