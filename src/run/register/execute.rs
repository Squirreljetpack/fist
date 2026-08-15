use std::process::{Command, Stdio};

use cba::broc::{tty_or_inherit, CommandExt, EnvVars};
use log::{info, warn};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::text_renderer_path,
    run::{
        lua::{call_with_paths, compile_lua},
        state::{ExecuteHandlerShouldProcessParent, MENU_ACTIONS, STACK, STORE},
    },
    utils::{command::maybe_tty, formatter::format_path},
};

/// Execution mode of the `FsAction::Execute*` variants, transported through
/// `state.discriminant_payload`
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Marker discriminant for command-output copy ([`crate::run::FsAction::CopyCommand`]
    /// and `CopyCommandAsync`) on the shared `ExecuteSilent`/`ExecuteAsync`
    /// interrupts.
    Copy = 0,
    Normal = 1,
    Paged = 2,
    Tty = 3,
    Detached = 4,
    Silent = 5,
    MenuAction = 7,
    LuaCommand = 8,
    LuaCommandPaged = 9,
}

impl ExecutionMode {
    pub fn discriminant(self) -> u8 {
        self as u8
    }

    pub fn from_discriminant_for_execute(d: u8) -> Option<Self> {
        match d {
            1 => Some(Self::Normal),
            2 => Some(Self::Paged),
            3 => Some(Self::Tty),
            7 => Some(Self::MenuAction),
            8 => Some(Self::LuaCommand),
            9 => Some(Self::LuaCommandPaged),
            _ => None,
        }
    }
    pub fn from_discriminant_for_silent(d: u8) -> Option<Self> {
        match d {
            0 => Some(Self::Copy),
            3 => Some(Self::Detached),
            4 => Some(Self::Silent),
            7 => Some(Self::MenuAction),
            8 => Some(Self::LuaCommand),
            _ => None,
        }
    }
}

/// The lua command for a menu-action payload: discriminant 7/9 payloads are
/// the action key (looked up in the registered menu actions), a discriminant 8
/// payload is the command itself.
pub(super) fn menu_lua_command(
    mode: ExecutionMode,
    payload: &str,
) -> Option<String> {
    match mode {
        ExecutionMode::MenuAction | ExecutionMode::LuaCommandPaged => MENU_ACTIONS
            .get()
            .and_then(|actions| actions.get(payload).map(|a| a.command.clone())),
        ExecutionMode::LuaCommand => Some(payload.to_string()),
        _ => None,
    }
}

/// Run a menu action's lua command with the `(paths, dst)` contract: the
/// targeted paths table and an empty destination. The navigation directory
/// is passed as the third argument when present. `set_progress` has no target
/// here and is a silent no-op. The command runs in the process cwd — scripts
/// are responsible for `cd`.
pub(super) fn run_menu_lua(
    command: &str,
    paths: &[AbsPath],
    nav_cwd: Option<&AbsPath>,
) {
    let f = match compile_lua(command) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to compile menu action lua command: {e}");
            return;
        }
    };
    if let Err(e) = call_with_paths(&f, paths, "", nav_cwd, None) {
        log::error!("Menu action lua error: {e}");
    }
}

/// Run a menu action's lua command paged: stdout (from `os.execute`,
/// `io.popen`, prints, …) is piped into the pager while the script runs. The
/// command runs in the process cwd — scripts are responsible for `cd`.
/// `set_progress` has no target here and is a silent no-op.
pub(super) fn run_menu_lua_paged(
    command: &str,
    paths: &[AbsPath],
    nav_cwd: Option<&AbsPath>,
) {
    let f = match compile_lua(command) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to compile menu action lua command: {e}");
            return;
        }
    };

    // The pager is spawned BEFORE the stdout redirect so its inherited
    // stdout is the terminal (the render loop left the alt screen for the
    // execute interrupt), not the pipe it reads from.
    #[cfg(unix)]
    let pager = std::process::Command::new(text_renderer_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .env("PG_FORCE_TTY", "true")
        ._spawn();
    #[cfg(not(unix))]
    let pager: Option<std::process::Child> = None;

    let mut pager = match pager {
        Some(p) => p,
        None => {
            #[cfg(unix)]
            warn!(
                "Failed to spawn pager: {:?}; running unpaged",
                text_renderer_path()
            );
            if let Err(e) = call_with_paths(&f, paths, "", nav_cwd, None) {
                log::error!("Menu action lua error: {e}");
            }
            return;
        }
    };

    #[cfg(unix)]
    {
        let Some(pipe) = pager.stdin.take() else {
            run_menu_lua(command, paths, nav_cwd);
            return;
        };
        let redirect = match stdout_redirect::StdoutRedirect::to(&pipe) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to redirect stdout to pager: {e}; running unpaged");
                drop(pipe);
                let _ = pager.wait();
                if let Err(e) = call_with_paths(&f, paths, "", nav_cwd, None) {
                    log::error!("Menu action lua error: {e}");
                }
                return;
            }
        };

        let result = call_with_paths(&f, paths, "", nav_cwd, None);
        if let Err(e) = result {
            log::error!("Menu action lua error: {e}");
        }

        // Restore stdout, then close the pipe: EOF ends the pager.
        drop(redirect);
        drop(pipe);
        let _ = pager.wait();
    }
}

#[cfg(unix)]
mod stdout_redirect {
    use std::io;
    use std::os::fd::{AsRawFd, RawFd};

    /// Redirects fd 1 (stdout) to `target` until dropped, then restores it.
    pub struct StdoutRedirect {
        saved: RawFd,
    }

    impl StdoutRedirect {
        pub fn to(target: &impl AsRawFd) -> io::Result<Self> {
            let saved = unsafe { libc::dup(1) };
            if saved < 0 {
                return Err(io::Error::last_os_error());
            }
            if unsafe { libc::dup2(target.as_raw_fd(), 1) } < 0 {
                let e = io::Error::last_os_error();
                unsafe { libc::close(saved) };
                return Err(e);
            }
            Ok(Self { saved })
        }
    }

    impl Drop for StdoutRedirect {
        fn drop(&mut self) {
            unsafe {
                libc::dup2(self.saved, 1);
                libc::close(self.saved);
            }
        }
    }
}

/// Build the command for an execution mode. `Paged` pipes stdout (into the
/// pager), `Tty` connects everything to the tty, the silent modes use null
/// stdio and `Detached` additionally applies `.detach()`.
pub(super) fn build_exec_command(
    mode: ExecutionMode,
    cmd: &str,
    cwd: Option<AbsPath>,
    vars: EnvVars,
) -> Option<Command> {
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
        ExecutionMode::Copy => {
            builder
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
        }
        ExecutionMode::LuaCommand | ExecutionMode::MenuAction | ExecutionMode::LuaCommandPaged => {
            return None;
        }
    }

    Some(builder)
}

/// Wait for a spawned child according to its mode and report whether the
/// execution counts as successful (used for the db bump). The silent modes do
/// not wait: a successful spawn counts, child completion is unknown.
pub(super) fn wait_exec(
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
        ExecutionMode::Detached | ExecutionMode::Silent => false,
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
        _ => true,
    }
}

/// Resolve the execution target: the cwd when the cursor is disabled,
/// otherwise the current item. Honors [`ExecuteHandlerShouldProcessParent`]
/// and returns `None` instead of panicking when there is no target or parent.
pub(super) fn resolve_target(state: &MMState<'_>) -> Option<AbsPath> {
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
pub(super) fn collect_exec_env(
    state: &MMState<'_>,
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
        if let Some(p) = state.preview_ui.as_ref() {
            vars.push(("SCROLL_LINE".to_string(), p.offset().to_string()));
        }
    }
    let preview_cmd = format_path(state.preview_payload(), path);
    vars.push(("FS_PREVIEW_COMMAND".to_string(), preview_cmd));

    vars
}
