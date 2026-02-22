//! Match files to actions based on matching rules.
//! Based on a RuleMatcher implementation in the standalone file rule_matcher.rs.
pub mod action;
mod config;
pub mod file_rule;
mod helpers;
pub mod rule_matcher;
use cli_boilerplate_automation::broc::tty_or_inherit;
pub use config::*;

pub mod mime_helpers;

use arrayvec::ArrayVec;
use cli_boilerplate_automation::bog::BogUnwrapExt;
use cli_boilerplate_automation::{bog::BogOkExt, broc::CommandExt};
use cli_boilerplate_automation::{ebog, unwrap};
use std::process::exit;
use std::process::{Command, Stdio};

use crate::cli::clap_tools::LessfilterCommand;
use crate::lessfilter::helpers::{extract, is_header, is_metadata, show_header, show_metadata};
use crate::utils::text::path_formatter;
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
    }: LessfilterCommand,
    mut cfg: LessfilterConfig,
) -> ! {
    let mut default = cfg.rules.get(Preset::Default).clone();
    let rules = cfg.rules.get_mut(preset);
    rules.prepend(&mut default);

    let mut any_file_succeeded = false;

    let mut singleton = paths.len() == 1;

    for path in paths {
        let apath = AbsPath::new(path.clone());
        let data = FileData::new(apath.clone(), &cfg.settings, &cfg.categories);
        log::debug!("file data: {data:?}");

        let rule = unwrap!(rules.get_best_match(&path, data).ebog(format!("No rule for {}", path.to_string_lossy())); continue);
        if rule.is_empty() {
            continue;
        }
        singleton &= rule.len() == 1;

        // show header
        if header == Some(true) {
            show_header(&path);
            any_file_succeeded = true;
        }
        log::debug!("rule found: {rule:?}");

        let maybe_tty = || {
            if matches!(preset, Preset::Edit) {
                tty_or_inherit()
            } else {
                Stdio::inherit()
            }
        };

        for (i, action) in rule.iter().enumerate() {
            log::debug!("Action: {action:?}");

            let action_success = if let Action::Custom(s) = action {
                let Some(template) = cfg.actions.get(s) else {
                    ebog!("The custom action '{s}' is not defined!");
                    continue;
                };
                let script = path_formatter(template, &AbsPath::new(path.clone()));
                let mut cmd = Command::from_script(&script);
                cmd.stdout(maybe_tty()).stdin(maybe_tty());

                cmd.status()._ebog().is_some_and(|s| s.success())
            } else if matches!(action, Action::Extract) {
                extract(&path)
            } else {
                let (progs, perms) = action.to_progs(&path, preset);
                singleton &= progs.len() == 1;

                let all_progs_succeeded = true;

                progs.into_iter().all(|mut prog| {
                    // filter out headers
                    if is_header(&prog) {
                        if header.is_none() {
                            show_header(&path);
                        }
                        true
                    } else if is_metadata(&prog) {
                        show_metadata(&path, i == 0)
                    } else {
                        log::debug!("Executing: {prog:?}");
                        if singleton {
                            let mut cmd = Command::new(prog.remove(0)).with_args(prog);
                            cmd.stdin(maybe_tty()).stdout(maybe_tty())._exec();
                        }
                        let mut cmd = Command::new(prog.remove(0));
                        cmd.args(prog).stdin(maybe_tty()).stdout(maybe_tty());

                        !cmd.status()._ebog().is_some_and(|s| s.success())
                    }
                })
            };

            any_file_succeeded |= action_success;
            if action_success && cfg.settings.early_exit {
                break;
            }
        }
    }

    if any_file_succeeded { exit(0) } else { exit(1) }
}

//-------------------------

pub type RulePreset = RuleMatcher<FileRule, ArrayVec<Action, 10>>;
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
            Preset::Edit => &mut self.edit,
            Preset::Default => &mut self.default,
        }
    }
}

#[cfg(test)]
mod tests;
