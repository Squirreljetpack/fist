//! Match files to actions based on matching rules.
//! Based on a RuleMatcher implementation in the standalone file rule_matcher.rs.
pub mod action;
mod application_helper;
mod config;
pub mod file_rule;
mod helpers;
pub mod rule_matcher;
pub use config::*;
pub use helpers::env_bat_opts;
pub mod env;
pub mod mime_helpers;

use arrayvec::ArrayVec;
use cba::bog::BogUnwrapExt;
use cba::{_trace, ebog, unwrap};
use cba::{bog::BogOkExt, broc::CommandExt};
use std::path::PathBuf;
use std::process::Command;

use crate::cli::clap_tools::LessfilterCommand;
use crate::lessfilter::env::line_column;
use crate::lessfilter::helpers::{extract, show_header, show_simple_metadata};use crate::utils::formatter::format_path;
use crate::pager;
use crate::{
    abspath::AbsPath,
    lessfilter::{
        action::{Action, CommandStrategy},
        file_rule::{FileData, FileRule},
        rule_matcher::RuleMatcher,
    },
};

// todo: if not read perm, add sudo
//

// this runs multiple commands, so it's more convenient for execute and preview to re-invoke this through :tools,
// ..although image protocols may necessitate adjustments
pub fn handle(
    LessfilterCommand {
        preset,
        header,
        paths,
        mut args,
        no_exec,
        tty,
        diagnose,
    }: LessfilterCommand,
    mut cfg: LessfilterConfig,
) -> i32 {
    if paths.is_empty() {
        return 2;
    }

    if diagnose {
        return run_diagnose(preset, paths, cfg);
    }

    let mut default = cfg.rules.get(Preset::Default).clone();
    let rules = cfg.rules.get_mut(preset);
    rules.append(&mut default);

    let mut any_file_succeeded = false;

    line_column::init_from_env();

    let path = &paths[0];
    let apath = AbsPath::new(path.clone());
    let data = FileData::new(apath.clone(), &cfg.settings, &cfg.categories);
    log::debug!("file data: {data:?}");

    let ActionEntry { rule, execution } = unwrap!(
        rules.get_best_match(path, data)
        .ebog(format!("No rule for {}", path.to_string_lossy()));
        2
    );

    if rule.is_empty() {
        return 2;
    }

    // show header
    if header == Some(true) {
        show_header(path);
        any_file_succeeded = true;
    }
    log::debug!("rule found: {rule:?}");

    let rl = rule.len().saturating_sub(1);

    for (i, action) in rule.iter().enumerate() {
        log::debug!("Action: {action:?}");

        let action_success = if let Action::Custom(s) = action {
            let Some(template) = cfg.actions.get(s) else {
                ebog!("The custom action '{s}' is not defined!");
                continue; // Note: This skip doesn't count as success/fail for execution logic
            };
            let script = format_path(template, &AbsPath::new(path.clone()));

            let mut cmd = Command::from_script(&script, &[]).with_args(args.drain(..));
            log::trace!("spawning custom: {script}");

            if !no_exec && i == rl {
                cmd._exec();
            }

            cmd.status()._ebog().is_some_and(|s| s.success())
        } else if matches!(action, Action::Extract) {
            extract(path)
        } else {
            let (strategies, perms) = action.to_progs(path, preset);
            let mut progs_success = true;

            let pl = (!strategies.is_empty()).then_some(strategies.len() - 1);

            for (pi, strategy) in strategies.into_iter().enumerate() {
                _trace!(strategy);
                let current_success = match strategy {
                    CommandStrategy::Header => {
                        if header.is_none() {
                            for p in &paths {
                                show_header(p)
                            }
                        }
                        true
                    }
                    CommandStrategy::Metadata(p) => {
                        paths.iter().all(|path| show_simple_metadata(path, pi == 0))
                    }
                    CommandStrategy::Pager(p) => {
                        // in-process render replacing the renderer subprocess:
                        // the singleton-exec optimization is lost for this path.
                        paths.iter().all(|path| {
                            matches!(pager::render_text(path, env_bat_opts()), Ok(true))
                        })
                    }
                    // an empty command line renders nothing; not a failure
                    CommandStrategy::None => true,

                    CommandStrategy::Prog(prog, args) => {
                        if !no_exec && Some(pi) == pl && i == rl {
                            let mut cmd = Command::new(prog);
                            cmd.args(args).args(&paths[1..])._exec();
                        }

                        let mut cmd = Command::new(prog);
                        cmd.args(args).args(&paths[1..]);
                        cmd.status()._ebog().is_some_and(|s| s.success())
                    }
                };

                if !current_success {
                    progs_success = false;
                    if cfg.settings.early_exit {
                        break;
                    }
                }
            }
            progs_success
        };

        any_file_succeeded |= action_success;

        match execution {
            ActionExecution::Abort if !action_success => {
                log::debug!("Stopped due to Execution=Abort.");
                break;
            }
            ActionExecution::Until if action_success => {
                log::debug!("Stopped due to Execution=Until.");
                break;
            }
            _ => {
                // ActionExecution::All continues regardless
            }
        }
    }

    if any_file_succeeded { 0 } else { 1 }
}

/// `--diagnose`: for each path, print the detected file data, the winning
/// rule with its score, and the commands that would run — without executing
/// anything.
fn run_diagnose(
    preset: Preset,
    paths: Vec<PathBuf>,
    mut cfg: LessfilterConfig,
) -> i32 {
    use cba::prints;

    let mut default = cfg.rules.get(Preset::Default).clone();
    let rules = cfg.rules.get_mut(preset);
    rules.append(&mut default);

    for path in &paths {
        let apath = AbsPath::new(path.clone());
        let data = FileData::new(apath.clone(), &cfg.settings, &cfg.categories);

        prints!(
            format!("{}", path.display());
            format!(
                "  mime: {}  kind: {}  filetype: {}  perms: {}{}{}",
                data.mime
                    .mime
                    .as_ref()
                    .map(|m| m.to_string())
                    .unwrap_or_default(),
                data.mime
                    .kind
                    .as_ref()
                    .map(|k| k.to_string())
                    .unwrap_or_default(),
                data.ft,
                if data.permissions[0] { 'r' } else { '-' },
                if data.permissions[1] { 'w' } else { '-' },
                if data.permissions[2] { 'x' } else { '-' },
            );
            format!("  preset: {preset}")
        );

        match rules.get_best_match_with_score(path, data) {
            Some((entry, score, rule)) => {
                prints!(format!("  matched rule (score {score}):"));
                for (score_part, rule_part) in rule {
                    prints!(format!("    {}", score_part.format(rule_part)));
                }
                prints!(format!("  actions:"));
                for action in entry.rule.iter() {
                    let commands = if let Action::Custom(s) = action {
                        let Some(template) = cfg.actions.get(s) else {
                            prints!(format!("    (custom action '{s}' is not defined)"));
                            continue;
                        };
                        vec![format_path(template, &apath)]
                    } else {
                        action
                            .to_progs(path, preset)
                            .0
                            .into_iter()
                            .map(|strategy| match strategy {
                                CommandStrategy::Header => "<header>".to_string(),
                                CommandStrategy::Metadata(p) => {
                                    format!("<metadata {}>", p.display())
                                }
                                CommandStrategy::Pager(p) => {
                                    format!("<pager {}>", p.display())
                                }
                                CommandStrategy::None => "<none>".to_string(),

                                CommandStrategy::Prog(prog, args) => std::iter::once(prog)
                                    .chain(args)
                                    .map(|part| part.to_string_lossy().into_owned())
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            })
                            .collect()
                    };
                    for command in commands {
                        prints!(format!("    {command}"));
                    }
                }
            }
            None => prints!(format!("  no rule matched")),
        }

        prints!("");
    }

    0
}

//-------------------------

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub enum ActionExecution {
    /// Stop execution on failure
    Abort,

    #[default]

    /// Execute all actions
    All,

    /// Stop execution on success
    Until,
}

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, serde::Serialize)]
pub struct ActionEntry {
    rule: ArrayVec<Action, 10>,
    execution: ActionExecution,
}

pub type RulePreset = RuleMatcher<FileRule, ActionEntry>;
/// Struct representation of RulesConfig
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    pub preview: RulePreset,
    pub display: RulePreset,
    pub extended: RulePreset,
    pub info: RulePreset,
    pub open: RulePreset,
    pub alternate: RulePreset,
    pub alternate2: RulePreset,
    pub edit: RulePreset,
    pub default: RulePreset,
}

/// Default impl
impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            preview: RuleMatcher::new(),
            display: RuleMatcher::new(),
            extended: RuleMatcher::new(),
            info: RuleMatcher::new(),
            open: RuleMatcher::new(),
            alternate: RuleMatcher::new(),
            alternate2: RuleMatcher::new(),
            edit: RuleMatcher::new(),
            default: RuleMatcher::new(),
        }
    }
}

impl RulesConfig {
    /// Getter by Preset enum
    pub fn get(
        &self,
        preset: Preset,
    ) -> &RulePreset {
        match preset {
            Preset::Preview => &self.preview,
            Preset::Display => &self.display,
            Preset::Extended => &self.extended,
            Preset::Info => &self.info,
            Preset::Open => &self.open,
            Preset::Alternate => &self.alternate,
            Preset::Alternate2 => &self.alternate2,
            Preset::Edit => &self.edit,
            Preset::Default => &self.default,
        }
    }

    /// Mutable getter
    pub fn get_mut(
        &mut self,
        preset: Preset,
    ) -> &mut RulePreset {
        match preset {
            Preset::Preview => &mut self.preview,
            Preset::Display => &mut self.display,
            Preset::Extended => &mut self.extended,
            Preset::Info => &mut self.info,
            Preset::Open => &mut self.open,
            Preset::Alternate => &mut self.alternate,
            Preset::Alternate2 => &mut self.alternate2,
            Preset::Edit => &mut self.edit,
            Preset::Default => &mut self.default,
        }
    }
}

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------------

use serde::Deserialize;

impl<'de> Deserialize<'de> for ActionEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Action(ArrayVec<Action, 10>),
            Full {
                kind: ArrayVec<Action, 10>,
                #[serde(default)]
                execution: ActionExecution,
            },
        }

        match Repr::deserialize(deserializer)? {
            Repr::Action(kind) => Ok(ActionEntry {
                rule: kind,
                execution: ActionExecution::All,
            }),
            Repr::Full { kind, execution } => Ok(ActionEntry {
                rule: kind,
                execution,
            }),
        }
    }
}
