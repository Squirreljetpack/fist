use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use cli_boilerplate_automation::{
    bait::TransformExt,
    bog::BogOkExt,
    bring::StrExt,
    broc::{CommandExt, SHELL, tty_or_inherit},
    env_vars, unwrap,
};
use easy_ext::ext;
use log::{debug, info, warn};
use matchmaker::{
    message::{Event, Interrupt},
    nucleo::Indexed,
    preview::AppendOnly,
};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::text_renderer_path,
    run::{
        FsMatchmaker,
        ahandlers::fs_reload,
        item::PathItem,
        pane::FsPane,
        state::{ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, STACK, TlsStore},
    },
    utils::formatter::format_path,
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

    let seek = TlsStore::take();

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
    if let Some(index) = TlsStore::take() {
        state.picker_ui.results.cursor_jump(index);
    };
}

pub fn query_handler(
    _state: &mut MMState<'_, '_>,
    _: &Event,
) {
    // rg query change is handled by rebinds
}

// ------------------------------------------------------------------------

#[ext(MMExt)]
// overrides to support static formatter
impl FsMatchmaker {
    pub fn register_reload_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Reload, move |state| {
            let template = state.payload();
            if !template.is_empty() {
                // User reload event: create a custom pane
                if let Some(t) = state.current_raw() {
                    let script = path_formatter(t, template);
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
                        false,
                    );

                    if STACK::with_current(|p| *p != pane) {
                        STACK::push(pane);
                        fs_reload(state, true)
                    } else {
                        STACK::with_current_mut(|p| *p = pane);
                        fs_reload(state, false)
                    }
                }
            }
        });
    }

    pub fn register_execute_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Execute, move |state| {
            let template = state.payload();
            if !template.is_empty() {
                let path = unwrap!(if state.picker_ui.results.cursor_disabled {
                    STACK::cwd()
                } else {
                    state.current_raw().map(|t| {
                        if TlsStore::take::<ExecuteHandlerShouldProcessParent>().is_some()
                            && let Some(p) = t.inner.path.parent()
                        {
                            AbsPath::new_unchecked(p)
                        } else {
                            t.inner.path.clone()
                        }
                    })
                });
                if execute(None, &path, state) {
                    GLOBAL::db().bump(path.is_dir(), path);
                }
            }
        });
    }

    pub fn register_become_handler_(&mut self) {
        self.register_interrupt_handler(Interrupt::Become, move |state| {
            let template = state.payload();
            if !template.is_empty()
                && let Some(p) = state.current_raw()
            {
                let cmd = path_formatter(p, template);
                let path = p.inner.path.clone();
                // lowpri: can't reliably do this as we immediately exec, tho i wonder if db can get corrupted this way;
                // GLOBAL::db().bump(path.is_dir(), path);

                let mut vars = state.make_env_vars();
                let preview_cmd = path_formatter(p, state.preview_payload());
                let extra = env_vars!(
                    "FS_PREVIEW_COMMAND" => preview_cmd,
                );
                vars.extend(extra);
                if let Some((line, col)) = state.current_raw().and_then(|item| {
                    state.picker_ui.worker.format_with(item, "3").map(|t| {
                        let x = t.as_ref().split_delim(':');
                        let line = x[0].parse::<isize>().ok();
                        let col = x[1].split_delim(':')[0].parse::<isize>().ok();
                        (line, col)
                    })
                }) && let Some(t) = line
                {
                    vars.push(("HIGHLIGHT_LINE".to_string(), t.to_string()));
                    if let Some(t) = col {
                        vars.push(("HIGHLIGHT_COLUMN".to_string(), t.to_string()));
                    }
                };

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
        default_template: Option<String>,
        output_separator: String,
    ) {
        self.register_interrupt_handler(Interrupt::Print, move |state| {
            if let Some(t) = state.current_raw() {
                let template = if state.payload().is_empty() {
                    default_template.as_ref()
                } else {
                    Some(state.payload())
                };

                let mut display = if let Some(template) = template {
                    path_formatter(t, template)
                } else {
                    t.path.to_string_lossy().into()
                };

                if atty::is(atty::Stream::Stdout) {
                    display.push_str(&output_separator);
                    print_handle.push(display);
                } else {
                    print!("{}{}", display, output_separator);
                }
            };
        });
    }
}

// ------------------------------------------------------------------------

pub fn path_formatter(
    item: &Indexed<PathItem>,
    template: &str,
) -> String {
    format_path(template, &item.inner.path)
}

fn execute(
    template: Option<&str>,
    path: &AbsPath,
    state: &mut MMState<'_, '_>,
) -> bool {
    let (template, variant) = if let Some(t) = template {
        (t, 0)
    } else if let Some(t) = state.payload().strip_prefix("\0\0\0")
        && t.len() > 1
        && let Ok(v) = t[0..1].parse::<u8>()
    {
        (&t[1..], v)
    } else {
        (state.payload().as_str(), 0u8)
    };

    let cmd = format_path(template, path);

    let mut vars = state.make_env_vars();

    let c = STACK::cwd();

    // lowpri: dow we expose fs_preview_command here?
    if STACK::in_rg() {
        if let Some((line, col)) = state.current_raw().and_then(|item| {
            state.picker_ui.worker.format_with(item, "3").map(|t| {
                let x = t.as_ref().split_delim(':');
                let line = x[0].parse::<isize>().ok();
                let col = x[1].split_delim(':')[0].parse::<isize>().ok();
                (line, col)
            })
        }) && let Some(t) = line
        {
            vars.push(("HIGHLIGHT_LINE".to_string(), t.to_string()));
            if let Some(t) = col {
                vars.push(("HIGHLIGHT_COLUMN".to_string(), t.to_string()));
            }
        };
        if let Some(p) = state.preview_ui.as_mut() {
            vars.push(("SCROLL_LINE".to_string(), p.offset().to_string()));
        }
    }
    let preview_cmd = format_path(state.preview_payload(), path);
    vars.push(("FS_PREVIEW_COMMAND".to_string(), preview_cmd));

    let mut builder = Command::from_script(&cmd);

    builder
        .envs(vars)
        .stdin(tty_or_inherit())
        .transform_if(c.is_some(), move |x| x.current_dir(c.unwrap()));

    // Apply variant-specific configurations
    match variant {
        1 => {
            // Prepare to pipe stdout to another program
            builder.stdout(Stdio::piped()).stdin(Stdio::null());
        }
        2 => {
            // full detach
            builder
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .detach();
        }
        3 => {
            // Silence but don't detach
            builder
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null());
        }
        4 => {
            // Connect everything to tty (placeholder: i dont see a use case for this)
            builder
                .stdout(maybe_tty())
                .stderr(maybe_tty())
                .stdin(maybe_tty());
        }

        _ => {} // As 0
    }

    if let Some(mut child) = builder._spawn() {
        match variant {
            1 => {
                if let Some(stdout) = child.stdout.take() {
                    let Some(mut child) = std::process::Command::new(text_renderer_path())
                        .stdin(stdout)
                        .stdout(Stdio::inherit())
                        ._spawn()
                    else {
                        warn!("Failed to spawn pager: {:?}", text_renderer_path());
                        return false;
                    };

                    match child.wait() {
                        Ok(i) => {
                            info!("Command [{cmd}] exited with {i}");
                            i.success()
                        }
                        Err(e) => {
                            info!("Failed to wait on command [{cmd}]: {e}");
                            false
                        }
                    }
                } else {
                    false
                }
            }
            2 => true,
            _ => {
                // Variant 0 and 2 logic: Just wait for the primary child
                match child.wait() {
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
        }
    } else {
        false
    }
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
