//! Match files to actions based on matching rules.
//! Based on a RuleMatcher implementation in the standalone file rule_matcher.rs.
pub mod action;
mod config;
pub mod file_rule;
mod helpers;
pub mod rule_matcher;
use cli_boilerplate_automation::broc::tty_or_inherit;
pub use config::*;
pub mod env;
pub mod mime_helpers;

use arrayvec::ArrayVec;
use cli_boilerplate_automation::bog::BogUnwrapExt;
use cli_boilerplate_automation::{bog::BogOkExt, broc::CommandExt};
use cli_boilerplate_automation::{ebog, unwrap};
use std::process::{Command, Stdio};

use crate::cli::clap_tools::LessfilterCommand;
use crate::lessfilter::env::line_column;
use crate::lessfilter::helpers::{extract, is_header, is_metadata, show_header, show_metadata};
use crate::utils::formatter::format_path;
use crate::{
    abspath::AbsPath,
    lessfilter::{
        action::Action,
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
    }: LessfilterCommand,
    mut cfg: LessfilterConfig,
) -> i32 {
    if paths.is_empty() {
        return 2;
    }
    let mut default = cfg.rules.get(Preset::Default).clone();
    let rules = cfg.rules.get_mut(preset);
    rules.prepend(&mut default);

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

    let maybe_tty = || {
        if tty || matches!(preset, Preset::Edit) {
            tty_or_inherit()
        } else {
            Stdio::inherit()
        }
    };

    let rl = rule.len();

    for (i, action) in rule.iter().enumerate() {
        log::debug!("Action: {action:?}");

        let action_success = if let Action::Custom(s) = action {
            let Some(template) = cfg.actions.get(s) else {
                ebog!("The custom action '{s}' is not defined!");
                continue; // Note: This skip doesn't count as success/fail for execution logic
            };
            let script = format_path(template, &AbsPath::new(path.clone()));

            let mut cmd = Command::from_script(&script).with_args(args.drain(..));

            if !no_exec && i == rl {
                cmd.stdin(maybe_tty()).stdout(maybe_tty())._exec();
            }

            cmd.stdout(maybe_tty()).stdin(maybe_tty());
            cmd.status()._ebog().is_some_and(|s| s.success())
        } else if matches!(action, Action::Extract) {
            extract(path)
        } else {
            let (progs, perms) = action.to_progs(path, preset);
            let mut progs_success = true;

            // let pl = progs.iter().rposition(|x| !is_header(x) && !is_metadata(x));
            let pl = (!progs.is_empty()).then_some(progs.len() - 1);

            for (pi, mut prog) in progs.into_iter().enumerate() {
                log::trace!("prog: {prog:?}");
                // filter out headers
                let current_success = if is_header(&prog) {
                    if header.is_none() {
                        for p in &paths {
                            show_header(p)
                        }
                    }
                    true
                } else if is_metadata(&prog) {
                    paths.iter().all(|p| show_metadata(p, i == 0))
                } else {
                    // Handle singleton execution
                    if !no_exec && Some(pi) == pl && i == rl {
                        let mut cmd = Command::new(prog.remove(0))
                            .with_args(prog)
                            .with_args(&paths[1..]);
                        cmd.stdin(maybe_tty()).stdout(maybe_tty())._exec();
                    }

                    let mut cmd = Command::new(prog.remove(0));
                    cmd.args(prog)
                        .args(&paths[1..])
                        .stdin(maybe_tty())
                        .stdout(maybe_tty());

                    cmd.status()._ebog().is_some_and(|s| s.success())
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
