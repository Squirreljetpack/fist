//! `fs :tool check` — non-interactive configuration validation.
//!
//! Parses every config file (config, mm, lessfilter, actions + the actions
//! folder), compiles every menu-action lua command against a scratch VM,
//! and reports warnings (key case-collisions, misused `requires_dest`,
//! duplicate aliases, empty `seq` conditions) and errors (parse failures,
//! compile failures, `@file` read failures, bindings referencing missing
//! action keys). Exits non-zero when any error is found.

use std::path::Path;

use cba::prints;
use matchmaker::action::Action;

use crate::{
    config::Config,
    display::display_menu_actions,
    lessfilter::LessfilterConfig,
    lua::{check_compiles, load_script},
    menu::{MenuActions, MenuStrategy},
    run::{
        FsAction,
        mm_config::{MMConfig, get_mm_binds},
        queue::{QueueSelector, validate_queue_kind},
    },
};

pub struct CheckResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// The merged menu actions (empty when loading failed).
    pub actions: MenuActions,
}

impl CheckResult {
    fn ok() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            actions: MenuActions::default(),
        }
    }
    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

/// Run the checks over the given config paths (the same files the app
/// would load). Prints a plain report to stdout, followed by the installed
/// menu actions (see [`crate::display::display_menu_actions`] for the
/// verbosity-dependent detail), and returns the exit code.
pub fn run(
    config: &Path,
    mm: &Path,
    lessfilter: &Path,
    actions: &Path,
    actions_dir: &Path,
    verbosity: u8,
) -> i32 {
    let result = check(config, mm, lessfilter, actions, actions_dir);
    for e in &result.errors {
        println!("error: {e}");
    }
    for w in &result.warnings {
        println!("warning: {w}");
    }
    if result.errors.is_empty() {
        println!(
            "check ok: {} error(s), {} warning(s)",
            result.errors.len(),
            result.warnings.len()
        );
    } else {
        println!(
            "check failed: {} error(s), {} warning(s)",
            result.errors.len(),
            result.warnings.len()
        );
    }
    let listing = display_menu_actions(&result.actions, verbosity);
    if !listing.is_empty() {
        println!();
        prints!(listing);
    }
    if result.errors.is_empty() { 0 } else { 1 }
}

fn check(
    config: &Path,
    mm: &Path,
    lessfilter: &Path,
    actions: &Path,
    actions_dir: &Path,
) -> CheckResult {
    let mut r = CheckResult::ok();

    // 1. parse every config file; a missing file is only a warning since
    //    the app silently falls back to its defaults in that case
    if config.is_file() {
        match read_parse::<Config>(config) {
            Ok(_) => {}
            Err(e) => r.error(e),
        }
    } else {
        r.warn(format!(
            "config file not found: {}; falling back to default config",
            config.display()
        ));
    }
    if mm.is_file() {
        if let Err(e) = read_parse::<MMConfig>(mm) {
            r.error(e);
        }
    } else {
        r.warn(format!(
            "binds file not found: {}; falling back to default binds",
            mm.display()
        ));
    }
    if lessfilter.is_file() {
        if let Err(e) = read_parse::<LessfilterConfig>(lessfilter) {
            r.error(e);
        }
    } else {
        r.warn(format!(
            "lessfilter file not found: {}; falling back to default rules",
            lessfilter.display()
        ));
    }
    // lessfilter presets carry shell templates, not lua commands

    let actions_map = match MenuActions::load_all(actions, actions_dir) {
        Ok(a) => a,
        Err(e) => {
            r.error(e);
            return r;
        }
    };
    r.actions = actions_map.clone();

    // 2. compile every menu-action command (with @file resolution against
    //    the actions folder, exactly as execution does)
    for (key, action) in actions_map.iter() {
        let Some(source) = load_script(&action.command, Some(actions_dir)) else {
            r.error(format!(
                "action {key:?}: failed to load command {:?}",
                action.command
            ));
            continue;
        };
        if let Err(e) = check_compiles(&source) {
            r.error(format!("action {key:?}: command does not compile: {e}"));
        }
    }

    // 5. key collisions: action keys differing only in case from each other
    //    (queue kind matching is exact-case, so `ExecuteQueue(key)` selects
    //    by the exact spelling; collisions with the builtin kinds are
    //    rejected at parse time as reserved keys)
    let mut seen: Vec<&str> = Vec::new();
    for key in actions_map.keys() {
        if let Some(other) = seen.iter().find(|k| k.eq_ignore_ascii_case(key)) {
            r.warn(format!(
                "action key {key:?} differs only in case from {other:?}: \
                 queue selection is case-sensitive"
            ));
        }
        seen.push(key);
    }

    // 6. bindings referencing missing action keys (queue selectors only:
    //    ExecuteQueue/ClearQueue payloads are menu-action keys, Enqueue is queue kind)
    if mm.is_file() {
        let (binds, _help) = get_mm_binds(mm);
        for bound in binds.values() {
            for action in &bound.0 {
                let Action::Custom(fs_action) = action else {
                    continue;
                };
                let selector = match fs_action {
                    FsAction::ExecuteQueue(sel) => Some(sel),
                    FsAction::ClearQueue(sel, _) => Some(sel),
                    _ => None,
                };
                if let Some(QueueSelector::Kind(kind)) = selector {
                    if let Err(err) = validate_queue_kind(kind, Some(&actions_map)) {
                        r.error(format!("bind references {err}"));
                    }
                }
                if let FsAction::Enqueue(kind) = fs_action {
                    if let Err(err) = validate_queue_kind(kind, Some(&actions_map)) {
                        r.error(format!("bind references {err}"));
                    }
                }
            }
        }
    }

    // 7. requires_dest on non-queue strategies has no effect
    for (key, action) in actions_map.iter() {
        if action.requires_dest
            && !matches!(
                action.strategy,
                MenuStrategy::Queue | MenuStrategy::QueueBatch(_)
            )
        {
            r.warn(format!(
                "action {key:?}: requires_dest = true has no effect on the {:?} strategy",
                action.strategy
            ));
        }
    }

    // 8. alias collisions
    let mut aliases: Vec<(&str, &str)> = Vec::new();
    for (key, action) in actions_map.iter() {
        if let Some(alias) = action.alias.as_deref() {
            if let Some((other, _)) = aliases.iter().find(|(_, a)| *a == alias) {
                r.warn(format!(
                    "action {key:?} reuses alias {alias:?} already used by {other:?}"
                ));
            } else {
                aliases.push((key, alias));
            }
        }
    }

    // 9. empty seq conditions never fire intentionally
    for (key, action) in actions_map.iter() {
        for condition in &action.condition {
            if matches!(condition, crate::menu::MenuCondition::Seq(v) if v.is_empty()) {
                r.warn(format!(
                    "action {key:?}: an empty seq condition is almost certainly a mistake"
                ));
            }
        }
    }

    r
}

fn read_parse<T>(path: &Path) -> Result<T, String>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&content).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    /// A temp config tree with a clean actions file.
    fn setup(
        actions: &str,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        let mm = dir.path().join("mm.toml");
        let lessfilter = dir.path().join("lessfilter.toml");
        let actions_path = dir.path().join("actions.toml");
        let actions_dir = dir.path().join("actions");
        write(&config, "");
        write(&mm, "");
        write(&lessfilter, "");
        write(&actions_path, actions);
        (dir, config, mm, lessfilter, actions_path, actions_dir)
    }

    #[test]
    fn shipped_actions_parse() {
        let toml_str = include_str!("../../assets/config/actions.toml");
        let actions: MenuActions = toml::from_str(toml_str).unwrap();
        assert_eq!(actions.len(), 1);
        assert!(actions["chmod +x"].command.contains("chmod "));
    }

    #[test]
    fn check_clean_config() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup("");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(r.errors.is_empty(), "{}", r.errors.join("; "));
        assert!(r.warnings.is_empty(), "{}", r.warnings.join("; "));
    }

    #[test]
    fn check_compile_error_reported() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) =
            setup("[\"bad\"]\ncommand = \"this is not lua (\"\n");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(
            r.errors.iter().any(|e| e.contains("bad")),
            "{}",
            r.errors.join("; ")
        );
    }

    #[test]
    fn check_missing_at_file_reported() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) =
            setup("[\"missing\"]\ncommand = \"@nope.lua\"\n");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(
            r.errors.iter().any(|e| e.contains("nope.lua")),
            "{}",
            r.errors.join("; ")
        );
    }

    #[test]
    fn check_parse_error_reported() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup("not toml [");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(!r.errors.is_empty());
    }

    #[test]
    fn check_bind_references_missing_key() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup("");
        write(&mm, "[binds]\nctrl-alt-p = \"ExecuteQueue(nope)\"\n");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(
            r.errors.iter().any(|e| e.contains("nope")),
            "{}",
            r.errors.join("; ")
        );
    }

    #[test]
    fn check_bind_enqueue_validation() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup(
            r#"
            [zip]
            command = "print('zip')"
            strategy = "Queue"
            "#,
        );
        // Valid custom and builtin kinds
        write(
            &mm,
            "[binds]\nctrl-z = \"Enqueue(zip)\"\nctrl-c = \"Enqueue(copy)\"\n",
        );
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(r.errors.is_empty(), "{}", r.errors.join("; "));

        // Unknown custom kind
        write(&mm, "[binds]\nctrl-z = \"Enqueue(nope)\"\n");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(
            r.errors.iter().any(|e| e.contains("nope")),
            "{}",
            r.errors.join("; ")
        );

        // Reserved selector keywords are rejected
        write(&mm, "[binds]\nctrl-z = \"Enqueue(all)\"\n");
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(
            r.errors.iter().any(|e| e.contains("all")),
            "{}",
            r.errors.join("; ")
        );
    }

    #[test]
    fn check_warnings() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup(
            r#"
["Git"]
alias = "g"
command = "return 1"
strategy = "Execute"
requires_dest = true

["git"]
alias = "g"
command = "return 1"
condition = []
"#,
        );
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        let joined = r.warnings.join("; ");
        assert!(joined.contains("case"), "{joined}");
        assert!(joined.contains("requires_dest"), "{joined}");
        assert!(joined.contains("alias"), "{joined}");
        assert!(joined.contains("seq"), "{joined}");
    }

    #[test]
    fn check_missing_files_are_warnings() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup("");
        fs::remove_file(&config).unwrap();
        fs::remove_file(&mm).unwrap();
        fs::remove_file(&lessfilter).unwrap();

        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(r.errors.is_empty(), "{}", r.errors.join("; "));
        let joined = r.warnings.join("; ");
        assert!(joined.contains("config file not found"), "{joined}");
        assert!(joined.contains("binds file not found"), "{joined}");
        assert!(joined.contains("lessfilter file not found"), "{joined}");
        assert_eq!(
            run(&config, &mm, &lessfilter, &actions_path, &actions_dir, 4),
            0
        );
    }

    #[test]
    fn listing_verbosity_tiers() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup(
            r#"
["zip"]
alias = "z"
command = "print('zip')"
strategy = "Queue"
requires_dest = true
condition = { selected = "active", condition = "glob:*.rs" }
"#,
        );
        let r = check(&config, &mm, &lessfilter, &actions_path, &actions_dir);
        assert!(r.errors.is_empty(), "{}", r.errors.join("; "));

        // verbosity 4: names only
        let names = display_menu_actions(&r.actions, 4);
        assert!(names.contains("installed menu actions (1):"), "{names}");
        assert!(names.contains("zip"), "{names}");
        assert!(!names.contains("strategy"), "{names}");

        // verbosity 5: everything but the command
        let detail = display_menu_actions(&r.actions, 5);
        assert!(detail.contains("[zip]"), "{detail}");
        assert!(detail.contains("alias = \"z\""), "{detail}");
        assert!(detail.contains("strategy = \"Queue\""), "{detail}");
        assert!(detail.contains("requires_dest = true"), "{detail}");
        assert!(detail.contains("condition"), "{detail}");
        assert!(!detail.contains("command"), "{detail}");

        // verbosity 6: everything including the command
        let full = display_menu_actions(&r.actions, 6);
        assert!(full.contains("command = \"print('zip')\""), "{full}");
    }

    #[test]
    fn check_run_exit_codes() {
        let (_d, config, mm, lessfilter, actions_path, actions_dir) = setup("");
        assert_eq!(
            run(&config, &mm, &lessfilter, &actions_path, &actions_dir, 4),
            0
        );

        let (_d, config, mm, lessfilter, actions_path, actions_dir) =
            setup("[\"bad\"]\ncommand = \"not lua (\"\n");
        assert_eq!(
            run(&config, &mm, &lessfilter, &actions_path, &actions_dir, 4),
            1
        );
    }
}
