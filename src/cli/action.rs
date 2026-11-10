//! `fs :tool action <name> [--dest <dir>] [--] [paths]` — run one menu action
//! from the shell.
//!
//! The name is the menu label, prefix included (`image: to png`, `flatten`).
//! Actions that write somewhere need `--dest <dir>`; the script receives it as
//! its `dst` argument exactly like a menu run. Options must precede the paths,
//! and `--` separates a path that looks like an option. Toasts print to stderr
//! since no UI is showing them.

use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicU8,
};

use crate::{
    abspath::AbsPath,
    lua::{execute, load_script, set_toasts_to_stderr},
    menu::{MenuActions, MenuStrategy},
};

pub struct Args {
    pub name: String,
    pub dest: Option<PathBuf>,
    pub paths: Vec<PathBuf>,
}

/// Run the named action, returning a process exit code.
pub fn run(
    actions: &Path,
    actions_dir: &Path,
    args: Args,
) -> i32 {
    let (actions, errors) = MenuActions::load_all(actions, actions_dir, true);
    for e in errors {
        eprintln!("warning: {e}");
    }

    let found = actions
        .iter()
        .find(|(key, action)| action.display_name(key) == args.name)
        // a bare key is accepted too, since prefixes are only a display detail
        .or_else(|| actions.iter().find(|(key, _)| **key == args.name));
    let Some((key, action)) = found else {
        let mut names: Vec<String> = actions
            .iter()
            .map(|(key, action)| action.display_name(key))
            .collect();
        names.sort();
        eprintln!("error: no menu action {:?}", args.name);
        if !names.is_empty() {
            eprintln!("available actions: {}", names.join(", "));
            eprintln!("(gated actions are hidden here; `fs :tool check` lists them)");
        }
        return 1;
    };

    let dest = match args.dest {
        Some(d) => {
            let d = AbsPath::new(d);
            if !AsRef::<Path>::as_ref(&d).is_dir() {
                eprintln!("error: --dest {d} is not a directory");
                return 1;
            }
            Some(d)
        }
        None if action.requires_dest => {
            eprintln!(
                "error: action {:?} writes to a destination; pass --dest <dir>",
                action.display_name(key)
            );
            return 1;
        }
        None => None,
    };

    let paths: Vec<AbsPath> = if args.paths.is_empty() {
        vec![AbsPath::initial()]
    } else {
        args.paths.into_iter().map(AbsPath::new).collect()
    };

    let Some(source) = load_script(&action.command, Some(actions_dir)) else {
        eprintln!(
            "error: no command script for {:?}",
            action.display_name(key)
        );
        return 1;
    };

    set_toasts_to_stderr(true);
    let dst = dest.map(|d| d.to_string()).unwrap_or_default();
    let nav_cwd = AbsPath::initial();
    // only the strategies that own the terminal can prompt
    let interactive = matches!(action.strategy, MenuStrategy::Exec | MenuStrategy::ExecWait);
    // queued rows drive a progress cell, so `set_progress` must exist for the
    // strategies that would have one in the UI
    let queued = matches!(
        action.strategy,
        MenuStrategy::Queue | MenuStrategy::QueueBatch(_) | MenuStrategy::Enqueue
    );
    let progress = AtomicU8::new(0);
    let progress = queued.then_some(&progress);
    match execute(&source, &paths, &dst, Some(&nav_cwd), progress, interactive) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            exit_code(&e).unwrap_or(1)
        }
    }
}

/// Recover the status from the runtime error the sandboxed `os.exit` raises,
/// so `fs :tool action` exits with the script's own code.
fn exit_code(error: &str) -> Option<i32> {
    let start = error.rfind("os.exit(")? + "os.exit(".len();
    let end = error[start..].find(')')? + start;
    error[start..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::exit_code;

    #[test]
    fn os_exit_code_is_recovered() {
        assert_eq!(exit_code("runtime error: os.exit(3)"), Some(3));
        assert_eq!(exit_code("os.exit(0)"), Some(0));
        assert_eq!(exit_code("runtime error: boom"), None);
    }
}
