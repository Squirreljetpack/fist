use std::process::{Command, Stdio};

use cba::broc::{CommandExt, EnvVars, tty_or_inherit};
use log::{info, warn};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::text_renderer_path,
    run::state::{ExecuteHandlerShouldProcessParent, STACK, STORE},
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
            _ => None,
        }
    }
    pub fn from_discriminant_for_silent(d: u8) -> Option<Self> {
        match d {
            0 => Some(Self::Copy),
            3 => Some(Self::Detached),
            4 => Some(Self::Silent),
            _ => None,
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
        ExecutionMode::LuaCommand | ExecutionMode::MenuAction => return None,
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
pub(super) fn resolve_target(state: &MMState<'_, '_>) -> Option<AbsPath> {
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
    state: &MMState<'_, '_>,
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
