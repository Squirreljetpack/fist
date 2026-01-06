use std::{ffi::OsString, process::Command, sync::atomic::Ordering};

use cli_boilerplate_automation::{
    bog::BogOkExt,
    broc::{CommandExt, SHELL, tty_or_inherit},
    env_vars, prints,
};
use easy_ext::ext;
use log::{debug, info};
use matchmaker::{
    Matchmaker, efx,
    message::{Event, Interrupt},
    nucleo::{
        Indexed,
        injector::{IndexedInjector, Injector},
    },
    preview::AppendOnly,
    render::{Effect, Effects},
};

use crate::{
    aliases::MMState,
    run::{
        fspane::FsPane,
        item::PathItem,
        state::{FILTERS, RESTORE_INPUT, STACK, TEMP},
    },
};

// before reload, store a recovery method
pub fn sync_handler<'a>(
    state: &mut MMState<'a>,
    _: &Event,
) -> Effects {
    // SORTING
    // On sync:
    // Check current pane to see how to interpret global state
    // Check the global, if it's different, update and reload
    // on sort completion, create a sorted toast
    // there is no signal to the user that we are awaiting sort but that is lowpri
    // ui panel: reads the global
    // would it help to guarantee the directory didn't change here?

    // TODO: support more pane variants
    FILTERS::refilter();

    let seek = TEMP::take_prev_dir();

    if let Some(seek) = seek
        && let Some(i) = state
            .picker_ui
            .worker
            .raw_results()
            .position(|x| x.inner.path == seek)
    {
        efx![Effect::SetIndex(i as u32)]
    } else if RESTORE_INPUT
    .compare_exchange(true, false, Ordering::Acquire, Ordering::Acquire)
    .is_ok()
    // this part is exclusive to [`FsAction::Undo`] and Forward, which is when the input can be taken from stack.
    && let Some((input, index)) = STACK::get_maybe_input()
    {
        let il = input.len() as u16;
        // refreshing selections should be the responsibility of the caller
        // However, we want to keep selections on file watch events, hence the above
        efx![
            Effect::Input((input, il)),
            Effect::SetIndex(index),
            Effect::RevalidateSelectons,
            Effect::RestoreInputPromptMarker
        ]
    } else {
        efx![]
    }
}

#[ext(MMExt)]
// overrides to support static formatter
impl Matchmaker<Indexed<PathItem>, PathItem> {
    pub fn register_reload_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Reload("".into()), move |state, interrupt| {
            if let Interrupt::Reload(template) = interrupt {
                // User reload event: create a custom pane
                if !template.is_empty() {
                    if let Some(t) = state.current_raw() {
                        let script = mm_formatter(t, template);
                        log::debug!("Reloading: {script}");
                        let (shell, arg) = &*SHELL;
                        let command = (
                            OsString::from(shell),
                            vec![OsString::from(arg), script.into()],
                        );
                        let pane = FsPane::new_custom(
                            STACK::cwd().unwrap_or_default(),
                            FILTERS::visibility(),
                            command,
                        );
                        STACK::push(pane);
                    }
                } else {
                    // 1. selections should be cleared on nav change
                    // 2. For custom reload actions, the pane was already created
                    // - this signals to restore that saved input
                    // - technically we should set this only if input was saved, but the check needs to be done by sync_handler anyway

                    if STACK::has_saved_input() {
                        RESTORE_INPUT.store(true, Ordering::Release);
                    }
                }

                let injector = IndexedInjector::new(state.injector(), 0);
                STACK::populate(injector, || {});
            }
            if !RESTORE_INPUT.load(Ordering::Acquire) {
                // i.e., [`FsAction::Find`]: stay in prompt
                if state.picker_ui.results.cursor_disabled {
                    efx![Effect::Input(Default::default()), Effect::ClearSelections]
                } else {
                    efx![
                        Effect::Input(Default::default()),
                        Effect::SetIndex(0),
                        Effect::ClearSelections,
                        Effect::RestoreInputPromptMarker
                    ]
                }
            } else {
                efx!()
            }
        });
    }

    pub fn register_execute_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Execute("".into()), move |state, interrupt| {
            let Interrupt::Execute(template) = interrupt else {
                unreachable!()
            };
            let Some(t) = state.current_raw() else {
                return efx![];
            };

            if !template.is_empty() {
                let cmd = mm_formatter(t, template);

                let mut vars = state.make_env_vars();
                let preview_cmd = mm_formatter(t, state.preview_payload());
                let extra = env_vars!(
                    "FZF_PREVIEW_COMMAND" => preview_cmd,
                );
                vars.extend(extra);

                if let Some(cwd) = STACK::cwd() {
                    std::env::set_current_dir(cwd)._ebog();
                }

                if let Some(mut child) = Command::from_script(&cmd)
                    .envs(vars)
                    .stdin(tty_or_inherit())
                    ._spawn()
                {
                    match child.wait() {
                        Ok(i) => {
                            info!("Command [{cmd}] exited with {i}")
                        }
                        Err(e) => {
                            info!("Failed to wait on command [{cmd}]: {e}")
                        }
                    }
                }
            } else {
                // undecided
            };

            efx![]
        });
    }

    pub fn register_become_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Become("".into()), move |state, interrupt| {
            if let Interrupt::Become(template) = interrupt
                && !template.is_empty()
                && let Some(t) = state.current_raw()
            {
                let cmd = mm_formatter(t, template);

                let mut vars = state.make_env_vars();
                let preview_cmd = mm_formatter(t, state.preview_payload());
                let extra = env_vars!(
                    "FZF_PREVIEW_COMMAND" => preview_cmd,
                );
                vars.extend(extra);

                if let Some(cwd) = STACK::cwd() {
                    std::env::set_current_dir(cwd)._ebog();
                }

                debug!("Becoming: {cmd}");

                Command::from_script(&cmd).envs(vars)._exec();
            }
            efx![]
        });
    }

    pub fn register_print_handler_(
        &mut self,
        print_handle: AppendOnly<String>,
    ) {
        self.register_interrupt_handler(Interrupt::Print("".into()), move |state, i| {
            if let Interrupt::Print(template) = i
                && let Some(t) = state.current_raw()
            {
                let s = mm_formatter(t, template);
                if atty::is(atty::Stream::Stdout) {
                    print_handle.push(s);
                } else {
                    prints!(s);
                }
            };
            efx![]
        });
    }
}

pub fn mm_formatter(
    item: &Indexed<PathItem>,
    template: &str,
) -> String {
    crate::utils::text::path_formatter(template, &item.inner.path)
}
