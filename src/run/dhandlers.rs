use std::{ffi::OsString, process::Command};

use cli_boilerplate_automation::{
    bog::BogOkExt,
    broc::{CommandExt, SHELL, tty_or_inherit},
    else_default, env_vars, prints,
};
use easy_ext::ext;
use log::{debug, info};
use matchmaker::{
    Matchmaker,
    message::{Event, Interrupt},
    nucleo::Indexed,
    preview::AppendOnly,
};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        ahandler::fs_reload,
        item::PathItem,
        pane::FsPane,
        state::{FILTERS, STACK, TEMP},
    },
};

// before reload, store a recovery method
pub fn sync_handler(
    state: &mut MMState<'_, '_>,
    _: &Event,
) {
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

    // reload saved state
    if let Some(seek) = seek
        && let Some(i) = state
            .picker_ui
            .worker
            .raw_results()
            .position(|x| x.inner.path == seek)
    {
        state.picker_ui.results.cursor_jump(i as u32);
    } else
    // this part is exclusive to [`FsAction::Undo`], Forward and watcher reload.
    if let Some(index) = TEMP::take_stashed_index() {
        state.picker_ui.results.cursor_jump(index);
    };
}

#[ext(MMExt)]
// overrides to support static formatter
impl Matchmaker<Indexed<PathItem>, PathItem> {
    pub fn register_reload_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Reload("".into()), move |state, interrupt| {
            if let Interrupt::Reload(template) = interrupt {
                // User reload event: create a custom pane
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

                    fs_reload(state)
                }
            }
        });
    }

    pub fn register_execute_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Execute("".into()), move |state, interrupt| {
            let Interrupt::Execute(template) = interrupt else {
                unreachable!()
            };
            if !template.is_empty() {
                let path = else_default!(if state.picker_ui.results.cursor_disabled {
                    STACK::cwd()
                } else {
                    state.current_raw().map(|t| t.inner.path.clone())
                });
                fs_execute(template, &path, state);
            }
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
        });
    }
}

pub fn mm_formatter(
    item: &Indexed<PathItem>,
    template: &str,
) -> String {
    crate::utils::text::path_formatter(template, &item.inner.path)
}

pub fn fs_execute(
    template: &str,
    path: &AbsPath,
    state: &MMState<'_, '_>,
) {
    let cmd = crate::utils::text::path_formatter(template, path);

    let vars = state.make_env_vars();

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
}
