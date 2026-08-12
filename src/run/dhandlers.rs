use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use cba::{
    bog::BogOkExt,
    broc::{CommandExt, EnvVars, SHELL, tty_or_inherit},
    env_vars,
};
use easy_ext::ext;
use log::{debug, info, warn};
use matchmaker::{
    message::{Event, Interrupt},
    preview::AppendOnly,
};
use tokio::io::AsyncReadExt;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::text_renderer_path,
    clipboard,
    run::{
        FsMatchmaker,
        ahandlers::fs_reload,
        item::PathItem,
        pane::FsPane,
        selection,
        state::{ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, STACK, STORE, TASKS, sort},
    },
    utils::formatter::format_path,
};
use fist_types::filters::SortOrder;

// ------------------------------------------------------------------------
// Execution-mode transport
// ------------------------------------------------------------------------

/// Marker discriminant for command-output copy ([`crate::run::FsAction::CopyCommand`]
/// and `CopyCommandAsync`) on the shared `ExecuteSilent`/`ExecuteAsync`
/// interrupts. It means "command-output copy" — it never selects a clipboard
/// backend; the backend is fixed at clipboard initialization.
pub const COPY_COMMAND: u8 = 0;

/// Execution mode of the `FsAction::Execute*` variants, transported through
/// `state.discriminant_payload` (the old `\0\0\0<digit>` payload prefix is
/// gone).
///
/// `Normal`'s value `0` is scoped to `Interrupt::Execute`, so it cannot
/// collide with [`COPY_COMMAND`] on the silent/async interrupts: fist only
/// sends `Detached`/`Silent` there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Normal = 0,
    Paged = 1,
    Tty = 2,
    Detached = 3,
    Silent = 4,
}

impl ExecutionMode {
    pub fn discriminant(self) -> u8 {
        self as u8
    }

    pub fn from_discriminant(d: u8) -> Option<Self> {
        match d {
            0 => Some(Self::Normal),
            1 => Some(Self::Paged),
            2 => Some(Self::Tty),
            3 => Some(Self::Detached),
            4 => Some(Self::Silent),
            _ => None,
        }
    }

    /// The silent modes live on `Interrupt::ExecuteSilent`; all of them spawn
    /// without waiting, and only `Detached` additionally applies `.detach()`.
    pub fn is_silent(self) -> bool {
        matches!(self, Self::Detached | Self::Silent)
    }
}

// ------------------------------------------------------------------------

//  Apply recovery methods which depend on all entries being present.
/// Rehydrates the selections that [`crate::run::ahandlers::fs_reload`] snapshotted as path hashes once the
/// fresh listing has landed.
pub fn sync_handler(
    state: &mut MMState<'_, '_>,
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
    _state: &mut MMState<'_, '_>,
    _: &Event,
) {
    // rg query change is handled by rebinds
}

// ------------------------------------------------------------------------
// Execution helpers
// ------------------------------------------------------------------------

/// Resolve the execution target: the cwd when the cursor is disabled,
/// otherwise the current item. Honors [`ExecuteHandlerShouldProcessParent`]
/// and returns `None` instead of panicking when there is no target or parent.
fn resolve_target(state: &mut MMState<'_, '_>) -> Option<AbsPath> {
    if state.picker_ui.results.cursor_disabled() {
        STACK::cwd()
    } else {
        let item = state.current_raw()?;
        if STORE::take::<ExecuteHandlerShouldProcessParent>().is_some() {
            item.path.parent().map(AbsPath::new_unchecked)
        } else {
            Some(item.path.clone())
        }
    }
}

/// Collect the filesystem environment for execution commands: the formatted
/// preview command plus rg highlight/scroll positions when in an rg pane.
/// Capture before spawning any task.
fn collect_exec_env(
    state: &mut MMState<'_, '_>,
    path: &AbsPath,
) -> EnvVars {
    let mut vars = state.make_env_vars();

    if STACK::in_rg() {
        if let Some(item) = state.current_raw() {
            let (line, col) = item.loc();
            vars.push(("HIGHLIGHT_LINE".to_string(), line.to_string()));
            if col != 0 {
                vars.push(("HIGHLIGHT_COLUMN".to_string(), col.to_string()));
            }
        };
        if let Some(p) = state.preview_ui.as_mut() {
            vars.push(("SCROLL_LINE".to_string(), p.offset().to_string()));
        }
    }
    let preview_cmd = format_path(state.preview_payload(), path);
    vars.push(("FS_PREVIEW_COMMAND".to_string(), preview_cmd));

    vars
}

/// Build the command for an execution mode. `Paged` pipes stdout (into the
/// pager), `Tty` connects everything to the tty, the silent modes use null
/// stdio and `Detached` additionally applies `.detach()`.
fn build_exec_command(
    mode: ExecutionMode,
    cmd: &str,
    cwd: Option<AbsPath>,
    vars: EnvVars,
) -> Command {
    let mut builder = Command::from_script(cmd, &[]);
    builder.envs(vars).stdin(tty_or_inherit());
    if let Some(c) = cwd {
        builder.current_dir(c);
    }

    match mode {
        ExecutionMode::Normal => {}
        ExecutionMode::Paged => {
            // prepare to pipe stdout to the pager
            builder.stdout(Stdio::piped()).stdin(Stdio::null());
        }
        ExecutionMode::Tty => {
            builder
                .stdout(maybe_tty())
                .stderr(maybe_tty())
                .stdin(maybe_tty());
        }
        ExecutionMode::Detached => {
            builder
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .detach();
        }
        ExecutionMode::Silent => {
            // silent but not detached
            builder
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null());
        }
    }

    builder
}

/// Wait for a spawned child according to its mode and report whether the
/// execution counts as successful (used for the db bump). The silent modes do
/// not wait: a successful spawn counts, child completion is unknown.
fn wait_exec(
    mode: ExecutionMode,
    cmd: &str,
    mut child: std::process::Child,
) -> bool {
    match mode {
        ExecutionMode::Paged => {
            let Some(stdout) = child.stdout.take() else {
                return false;
            };
            let Some(mut pager) = std::process::Command::new(text_renderer_path())
                .stdin(stdout)
                .stdout(Stdio::inherit())
                .env("PG_FORCE_TTY", "true")
                ._spawn()
            else {
                warn!("Failed to spawn pager: {:?}", text_renderer_path());
                return false;
            };

            match pager.wait() {
                Ok(i) => {
                    info!("Command [{cmd}] exited with {i}");
                    i.success()
                }
                Err(e) => {
                    info!("Failed to wait on command [{cmd}]: {e}");
                    false
                }
            }
        }
        ExecutionMode::Detached | ExecutionMode::Silent => true,
        ExecutionMode::Normal | ExecutionMode::Tty => match child.wait() {
            Ok(i) => {
                info!("Command [{cmd}] exited with {i}");
                i.success()
            }
            Err(e) => {
                info!("Failed to wait on command [{cmd}]: {e}");
                false
            }
        },
    }
}

/// [`tokio::process::Command`] counterpart of `cba`'s `Command::from_script`,
/// always using the default [`SHELL`] (fist has no configurable shell).
fn tokio_from_script(script: &str) -> tokio::process::Command {
    let (def_sh, def_arg) = &*SHELL;

    let mut cmd = tokio::process::Command::new(def_sh);
    cmd.arg(def_arg);
    cmd.arg(script);

    #[cfg(unix)]
    cmd.arg("");

    cmd
}

/// `[tui].copy_trailing_newline` policy for command-output copy: when false,
/// trim one trailing newline and a preceding `\r`.
pub(crate) fn apply_newline_policy(
    text: &mut String,
    copy_trailing_newline: bool,
) {
    if !copy_trailing_newline && text.ends_with('\n') {
        text.pop();

        if text.ends_with('\r') {
            text.pop();
        }
    }
}

// ------------------------------------------------------------------------

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
            let template = state.payload().clone();
            if template.is_empty() {
                return;
            }

            // every discriminant on Interrupt::Execute is a fist ExecutionMode
            let mode = match state.discriminant_payload.take() {
                Some(d) => ExecutionMode::from_discriminant(d).unwrap_or_else(|| {
                    log::error!("Unknown execution discriminant on Execute: {d}");
                    ExecutionMode::Normal
                }),
                None => ExecutionMode::Normal,
            };

            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(&template, &path);
            if cmd.is_empty() {
                warn!("Empty formatted command for Execute");
                return;
            }

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            let Some(child) = build_exec_command(mode, &cmd, cwd, vars)._spawn() else {
                return; // spawn failure already logged
            };
            if wait_exec(mode, &cmd, child) {
                GLOBAL::db().bump_path(path.is_dir(), path);
            }
        });
    }

    /// Handles `Interrupt::ExecuteSilent`: the fist silent modes
    /// (`ExecDetached`/`ExecSilent`) and the core `Action::ExecuteSilent`
    /// (no discriminant). All silent actions spawn without waiting; only
    /// `ExecDetached` applies `.detach()`.
    ///
    /// The [`COPY_COMMAND`] discriminant is left untouched for
    /// [`MMExt::register_copy_command_handler`].
    pub fn register_execute_silent_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::ExecuteSilent, move |state| {
            let mode = match state.discriminant_payload {
                Some(COPY_COMMAND) => return, // owned by the copy handler
                Some(d) => {
                    let Some(mode) = ExecutionMode::from_discriminant(d) else {
                        // not ours: leave it for the next handler
                        return;
                    };
                    state.discriminant_payload.take();
                    if !mode.is_silent() {
                        log::error!("Execution mode {mode:?} belongs to Interrupt::Execute");
                        return;
                    }
                    mode
                }
                None => {
                    // plain core Action::ExecuteSilent: matchmaker's silent
                    // behavior — spawn without waiting
                    let template = state.payload().clone();
                    if template.is_empty() {
                        return;
                    }
                    let Some(path) = resolve_target(state) else {
                        return;
                    };
                    let cmd = format_path(&template, &path);
                    if cmd.is_empty() {
                        warn!("Empty formatted command for ExecuteSilent");
                        return;
                    }
                    let cwd = STACK::cwd();
                    let vars = collect_exec_env(state, &path);

                    let mut builder = Command::from_script(&cmd, &[]);
                    builder.envs(vars).stdin(tty_or_inherit());
                    if let Some(c) = cwd {
                        builder.current_dir(c);
                    }
                    builder._spawn(); // spawn failure already logged
                    return;
                }
            };

            let template = state.payload().clone();
            if template.is_empty() {
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(&template, &path);
            if cmd.is_empty() {
                warn!("Empty formatted command for {mode:?}");
                return;
            }
            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            let mut builder = build_exec_command(mode, &cmd, cwd, vars);
            builder._spawn(); // spawn failure already logged
        });
    }

    /// Handles the core `Action::ExecuteAsync`/`Action::ExecuteThen`, which
    /// the render loop encodes as `2 * id` (continue regardless of exit
    /// status) / `2 * id + 1` (continue only on success) with `id >= 1`, so
    /// continuation payloads are always `>= 2`. [`COPY_COMMAND`] is left
    /// untouched for [`MMExt::register_copy_command_handler`].
    pub fn register_execute_async_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::ExecuteAsync, move |state| {
            let Some(payload) = state.discriminant_payload else {
                return;
            };
            if payload < 2 {
                return; // COPY_COMMAND (0) is owned by the copy handler
            }
            state.discriminant_payload.take();

            let id = payload / 2;
            let require_success = payload % 2 == 1;
            // move the continuation closure out of the state before spawning
            let closure_opt = state.take_actions(id);

            let template = state.payload().clone();
            if template.is_empty() {
                warn!("Empty payload for ExecuteAsync");
                return;
            }
            let Some(path) = resolve_target(state) else {
                warn!("No execution target for ExecuteAsync");
                return;
            };
            let cmd = format_path(&template, &path);
            if cmd.is_empty() {
                warn!("Empty formatted command for ExecuteAsync");
                return;
            }
            let Some(closure) = closure_opt else {
                log::error!("No continuation slot for ExecuteAsync discriminant {payload}");
                return;
            };

            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

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

    /// Handles command-output copy: [`crate::run::FsAction::CopyCommand`]
    /// (synchronous, on `ExecuteSilent`) and `CopyCommandAsync` (background,
    /// on `ExecuteAsync`), both marked [`COPY_COMMAND`]. Captures stdout
    /// separately from stderr, waits for the command, applies the
    /// `[tui].copy_trailing_newline` policy and copies any non-empty output
    /// through the initialized backend. There is no `CLIPcmd` fallback; the
    /// async branch creates no action-batch continuation.
    pub fn register_copy_command_handler(&mut self, copy_trailing_newline: bool) {
        self.register_interrupt_handler(Interrupt::ExecuteSilent, move |state| {
            if state.discriminant_payload != Some(COPY_COMMAND) {
                return;
            }
            state.discriminant_payload.take();

            let template = state.payload().clone();
            if template.is_empty() {
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(&template, &path);
            if cmd.is_empty() {
                warn!("Empty formatted command for CopyCommand");
                return;
            }
            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            // synchronous copy: stdout must be available before copying
            let mut builder = Command::from_script(&cmd, &[]);
            builder
                .envs(vars)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(c) = cwd {
                builder.current_dir(c);
            }

            let output = match builder.output() {
                Ok(o) => o,
                Err(e) => {
                    warn!("Failed to spawn copy command [{cmd}]: {e}");
                    return;
                }
            };
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                info!(
                    "Copy command [{cmd}] exited with {}; copying stdout anyway. stderr: {}",
                    output.status,
                    stderr.trim()
                );
            }

            let Ok(mut text) = String::from_utf8(output.stdout) else {
                warn!("Copy command [{cmd}] produced non-UTF8 stdout");
                return;
            };
            apply_newline_policy(&mut text, copy_trailing_newline);
            if !text.is_empty() {
                clipboard::copy_text(text, true);
            }
        });

        self.register_interrupt_handler(Interrupt::ExecuteAsync, move |state| {
            if state.discriminant_payload != Some(COPY_COMMAND) {
                return;
            }
            state.discriminant_payload.take();

            let template = state.payload().clone();
            if template.is_empty() {
                return;
            }
            let Some(path) = resolve_target(state) else {
                return;
            };
            let cmd = format_path(&template, &path);
            if cmd.is_empty() {
                warn!("Empty formatted command for CopyCommandAsync");
                return;
            }
            let cwd = STACK::cwd();
            let vars = collect_exec_env(state, &path);

            // background copy: capture stdout without blocking the render thread
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

                apply_newline_policy(&mut text, copy_trailing_newline);
                if !text.is_empty() {
                    clipboard::copy_text(text, true);
                }
            });
        });
    }

    pub fn register_become_handler(&mut self) {
        self.register_interrupt_handler(Interrupt::Become, move |state| {
            let template = state.payload();
            if !template.is_empty()
                && let Some(p) = state.current_raw()
            {
                let cmd = path_formatter(p, template);
                if cmd.is_empty() {
                    warn!("Empty formatted command for Become");
                    return;
                }
                // lowpri: can't reliably do this as we immediately exec, tho i wonder if db can get corrupted this way;
                // GLOBAL::db().bump(path.is_dir(), path);

                let mut vars = state.make_env_vars();
                let preview_cmd = path_formatter(p, state.preview_payload());
                let extra = env_vars!(
                    "FS_PREVIEW_COMMAND" => preview_cmd,
                );
                vars.extend(extra);
                if STACK::in_rg() {
                    let (line, col) = p.loc();
                    vars.push(("HIGHLIGHT_LINE".to_string(), line.to_string()));
                    if col != 0 {
                        vars.push(("HIGHLIGHT_COLUMN".to_string(), col.to_string()));
                    }
                };

                if let Some(cwd) = STACK::cwd() {
                    std::env::set_current_dir(cwd)._ebog();
                }

                debug!("Becoming: {cmd}");

                Command::from_script(&cmd, &[]).envs(vars)._exec();
            }
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

// ------------------------------------------------------------------------

pub fn path_formatter(
    item: &PathItem,
    template: &str,
) -> String {
    format_path(template, &item.path)
}

fn maybe_tty() -> Stdio {
    if let Ok(mut tty) = std::fs::File::open("/dev/tty") {
        let _ = std::io::Write::flush(&mut tty); // does nothing but seems logical
        Stdio::from(tty)
    } else {
        log::error!("Failed to open /dev/tty");
        Stdio::inherit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_mode_round_trips_all_discriminants() {
        for mode in [
            ExecutionMode::Normal,
            ExecutionMode::Paged,
            ExecutionMode::Tty,
            ExecutionMode::Detached,
            ExecutionMode::Silent,
        ] {
            assert_eq!(
                ExecutionMode::from_discriminant(mode.discriminant()),
                Some(mode)
            );
        }
        assert_eq!(ExecutionMode::from_discriminant(5), None);
        assert_eq!(ExecutionMode::from_discriminant(u8::MAX), None);
    }

    #[test]
    fn copy_command_marker_classification() {
        // on ExecuteSilent/ExecuteAsync, only Detached/Silent are execution
        // modes; COPY_COMMAND (0) identifies command-output copy and the
        // async continuation payloads start at 2
        assert_eq!(COPY_COMMAND, 0);
        assert!(
            !ExecutionMode::from_discriminant(COPY_COMMAND)
                .unwrap()
                .is_silent()
        );
        assert!(ExecutionMode::Detached.is_silent());
        assert!(ExecutionMode::Silent.is_silent());
        assert!(!ExecutionMode::Normal.is_silent());
        assert!(!ExecutionMode::Paged.is_silent());
        assert!(!ExecutionMode::Tty.is_silent());
    }

    #[test]
    fn newline_policy_trims_when_disabled() {
        let mut text = "hello\r\n".to_string();
        apply_newline_policy(&mut text, false);
        assert_eq!(text, "hello");

        let mut text = "hello\n".to_string();
        apply_newline_policy(&mut text, false);
        assert_eq!(text, "hello");

        // only one trailing newline (and its preceding \r) is trimmed
        let mut text = "a\n\n".to_string();
        apply_newline_policy(&mut text, false);
        assert_eq!(text, "a\n");

        let mut text = "hello".to_string();
        apply_newline_policy(&mut text, false);
        assert_eq!(text, "hello");
    }

    #[test]
    fn newline_policy_keeps_output_when_enabled() {
        let mut text = "hello\r\n".to_string();
        apply_newline_policy(&mut text, true);
        assert_eq!(text, "hello\r\n");
    }
}
