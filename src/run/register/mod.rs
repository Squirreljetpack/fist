mod execute;
pub use execute::ExecutionMode;
use execute::*;

use std::{ffi::OsString, process::Stdio};

use cba::{
    bait::ResultExt,
    broc::{CommandExt, SHELL},
};
use easy_ext::ext;
use log::{info, warn};
use matchmaker::{
    message::{Event, Interrupt},
    preview::AppendOnly,
};
use tokio::io::AsyncReadExt;

use crate::{
    aliases::MMState,
    clipboard,
    run::{
        FsMatchmaker,
        ahandlers::fs_reload,
        item::PathItem,
        pane::FsPane,
        selection,
        state::{FILTERS, GLOBAL, MenuCommandPaths, STACK, STORE, TASKS, sort},
    },
    utils::{command::tokio_from_script, formatter::format_path},
};
use fist_types::filters::SortOrder;

//  Apply recovery methods which depend on all entries being present.
/// Rehydrates the selections that [`crate::run::ahandlers::fs_reload`] snapshotted as path hashes once the
/// fresh listing has landed.
pub fn sync_handler(
    state: &mut MMState<'_,>,
    _: &Event,
) {
    // reload saved state
    if let Some(seek) = STORE::take()
        && let Some(i) = state
            .picker_ui
            .worker
            .matched_results()
            .position(|x| x.path == seek)
    {
        state.picker_ui.results.cursor_jump(i as u32);
        STORE::take::<u32>();
    } else
    // this part is exclusive to [`FsAction::Undo`], Forward and watcher reload.
    if let Some(index) = STORE::take() {
        state.picker_ui.results.cursor_jump(index);
    };

    // peek: only refill once the pane has finished populating
    let ready = STORE::with::<selection::PendingSelections, _>(|pending| {
        !pending.0.is_empty() && STACK::with_current(FsPane::is_complete)
    })
    .unwrap_or(false);
    if !ready {
        return;
    }

    if let Some(selection::PendingSelections(hashes)) =
        STORE::take::<selection::PendingSelections>()
    {
        let items = state.picker_ui.worker.nucleo.items();
        let indices = selection::rehydrate(&hashes, items.iter());
        state.picker_ui.selector.clear();
        state.picker_ui.selector.extend(indices);
    }

    // the pane has finished populating: for a size sort, wait for the dir
    // sizes (added during populate) and let ReSort apply + resort them
    if sort::get_sort().order == SortOrder::size {
        sort::wait_sizes_then_resort();
    }
}

pub fn query_handler(
    _state: &mut MMState<'_,>,
    _: &Event,
) {
    // rg query change is handled by rebinds
}

#[ext(MMExt)]
// overrides to support static formatter
impl FsMatchmaker {
    pub fn register_reload_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::Reload, move |state| {
            let template = state.payload();
            if !template.is_empty() {
                // User reload event: create a custom pane
                if let Some(item) = state.current_raw() {
                    let script = path_formatter(item, template);
                    log::debug!("Reloading: {script}");
                    let (shell, arg) = &*SHELL;
                    let command = (
                        OsString::from(shell),
                        vec![OsString::from(arg), script.into()],
                    );
                    let pane = FsPane::new_custom(
                        STACK::_cwd(),
                        FILTERS::visibility(),
                        Some(command),
                        false,
                        fist_types::filters::SortOrder::none,
                        None,
                        None,
                        None,
                    );

                    if STACK::with_current(|p| *p != pane) {
                        STACK::push(pane);
                        fs_reload(state, true, false)
                    } else {
                        STACK::with_current_mut(|p| *p = pane);
                        fs_reload(state, false, false)
                    }
                }
            }
        });
    }

    /// Handles `Interrupt::Execute`: the fist execution modes (`Execute`,
    /// `ExecPaged`, `ExecTTY`) and bare templates from e.g. `FsAction::Advance`
    /// (no discriminant = [`ExecutionMode::Normal`]). Waits for the child.
    pub fn register_execute_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::Execute, move |state| {
            if state.payload().is_empty() {
                return;
            }
            let mode = match state
                .discriminant_payload
                .take()
                .and_then(ExecutionMode::from_discriminant_for_execute)
            {
                Some(x) => x,
                _ => ExecutionMode::Normal,
            };
            // menu action execution (discriminants 7/8/9): the payload is not a
            // command template — it is the action key (7/9) or the command
            // itself (8). The targeted paths were stashed at menu activation.
            if let Some(lua_cmd) = menu_lua_command(mode, state.payload()) {
                let Some(paths) = STORE::take::<MenuCommandPaths>() else {
                    warn!("Menu action executed without targeted paths");
                    return;
                };
                if mode == ExecutionMode::LuaCommandPaged {
                    run_menu_lua_paged(&lua_cmd, &paths);
                } else {
                    run_menu_lua(&lua_cmd, &paths);
                }
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(state.payload(), &path);
            if cmd.is_empty() {
                warn!("Empty formatted command");
                return;
            }

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            let Some(child) = build_exec_command(mode, &cmd, cwd, vars).unwrap()._spawn() else {
                return;
            };
            if wait_exec(mode, &cmd, child) {
                GLOBAL::db().bump_path(path.is_dir(), path);
            }
        });
    }

    pub fn register_execute_silent_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::ExecuteSilent, move |state| {
            if state.payload().is_empty() {
                return;
            }
            let mode = match state
                .discriminant_payload
                .take()
                .and_then(ExecutionMode::from_discriminant_for_silent)
            {
                Some(x) => x,
                _ => ExecutionMode::Silent,
            };
            // menu action execution (discriminants 7/8): the payload is not a
            // command template — it is the action key or the command itself.
            // Fired without waiting, like the other silent modes. The targeted
            // paths are taken here on the main thread (STORE is thread-local).
            if let Some(lua_cmd) = menu_lua_command(mode, state.payload()) {
                let Some(paths) = STORE::take::<MenuCommandPaths>() else {
                    warn!("Menu action executed without targeted paths");
                    return;
                };
                TASKS::spawn_blocking(move || run_menu_lua(&lua_cmd, &paths));
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(state.payload(), &path);
            if cmd.is_empty() {
                warn!("Empty formatted command");
                return;
            }

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            match build_exec_command(mode, &cmd, cwd, vars) {
                Some(mut c) => {
                    if mode == ExecutionMode::Copy {
                        if let Some(contents) = c.read_to_string()._elog()
                            && !contents.is_empty()
                        {
                            clipboard::copy_text(contents, true);
                        };
                    } else if let Some(child) = c._spawn() {
                        if wait_exec(mode, &cmd, child) {
                            GLOBAL::db().bump_path(path.is_dir(), path);
                        }
                    };
                }
                None => {}
            }
        });
    }

    /// Handles the core `Action::ExecuteAsync`/`Action::ExecuteThen`, which
    /// the render loop encodes as `2 * id` (continue regardless of exit
    /// status) / `2 * id + 1` (continue only on success) with `id >= 1`, so
    /// continuation payloads are always `>= 2`. [`COPY_COMMAND`] is left
    /// untouched for [`MMExt::register_copy_command_handler`].
    pub fn register_execute_async_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::ExecuteAsync, move |state| {
            if state.payload().is_empty() {
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let Some(payload) = state.discriminant_payload.take() else {
                return;
            };
            let cmd = format_path(state.payload(), &path);
            if cmd.is_empty() {
                warn!("Empty formatted command");
                return;
            }

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            if payload < 2 {
                TASKS::spawn(async move {
                    let mut builder = tokio_from_script(&cmd);
                    builder
                        .envs(vars)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null());
                    if let Some(c) = cwd {
                        builder.current_dir(c);
                    }

                    let mut child = match builder.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            warn!("Failed to spawn copy command [{cmd}]: {e}");
                            return;
                        }
                    };

                    let mut text = String::new();
                    if let Some(mut stdout) = child.stdout.take()
                        && let Err(e) = stdout.read_to_string(&mut text).await
                    {
                        warn!("Failed to read copy command stdout [{cmd}]: {e}");
                    }

                    match child.wait().await {
                        Ok(s) if !s.success() => {
                            info!("Copy command [{cmd}] exited with {s}; copying stdout anyway");
                        }
                        Err(e) => {
                            warn!("Failed to wait on copy command [{cmd}]: {e}");
                        }
                        _ => {}
                    }

                    if !text.is_empty() {
                        clipboard::copy_text(text, true);
                    }
                });
                return;
            }

            let id = payload / 2;
            let require_success = payload % 2 == 1;
            // move the continuation closure out of the state before spawning
            let closure_opt = state.take_actions(id);

            let Some(closure) = closure_opt else {
                log::error!("No continuation slot for ExecuteAsync discriminant {payload}");
                return;
            };

            TASKS::spawn(async move {
                let mut builder = tokio_from_script(&cmd);
                builder
                    .envs(vars)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                if let Some(c) = cwd {
                    builder.current_dir(c);
                }

                let mut child = match builder.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Failed to spawn async command [{cmd}]: {e}");
                        return;
                    }
                };

                match child.wait().await {
                    Ok(s) => {
                        info!("Async command [{cmd}] exited with {s}");
                        if !require_success || s.success() {
                            closure();
                        }
                    }
                    Err(e) => {
                        warn!("Failed to wait on async command [{cmd}]: {e}");
                    }
                }
            });
        });
    }

    pub fn register_become_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::Become, move |state| {
            if state.payload().is_empty() {
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(state.payload(), &path);
            if cmd.is_empty() {
                warn!("Empty formatted command");
                return;
            }

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            build_exec_command(ExecutionMode::Normal, &cmd, cwd, vars)
                .unwrap()
                ._exec()
        });
    }

    pub fn register_print_handler(
        &mut self,
        print_handle: AppendOnly<String>,
        default_template: Option<String>,
        output_sep: String,
    ) {
        self.register_interrupt_handler(Interrupt::Print, move |state| {
            if let Some(t) = state.current_raw() {
                let template = if state.payload().is_empty() {
                    default_template.as_deref()
                } else {
                    Some(state.payload().as_str())
                };

                emit_print(&print_handle, t, template, &output_sep);
            };
        });
    }
}

// ------------------------------------------------------------------------

/// Format one item per `template` (falling back to the raw path) and emit:
/// tty → `print_handle` (flushed post-pick), piped → stdout directly.
///
/// Shared by the [`Interrupt::Print`] handler ([`MMExt::register_print_handler`])
/// and the accept hook built in [`crate::run::start::make_mm`].
pub fn emit_print(
    print_handle: &AppendOnly<String>,
    item: &PathItem,
    template: Option<&str>,
    output_sep: &str,
) {
    let mut display = if let Some(template) = template {
        path_formatter(item, template)
    } else {
        item.path.to_string_lossy().into()
    };

    if atty::is(atty::Stream::Stdout) {
        display.push_str(output_sep);
        print_handle.push(display);
    } else {
        print!("{}{}", display, output_sep);
    }
}

pub fn path_formatter(
    item: &PathItem,
    template: &str,
) -> String {
    format_path(template, &item.path)
}
