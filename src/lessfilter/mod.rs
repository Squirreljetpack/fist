#![allow(unused)]

pub mod action;
mod config;
pub mod file_rule;
mod helpers;
pub mod rule_matcher;
use cli_boilerplate_automation::broc::tty_or_inherit;
pub use config::*;
pub mod mime_helpers;
use arrayvec::ArrayVec;
use cli_boilerplate_automation::bait::OptionExt;
use cli_boilerplate_automation::bog::BogUnwrapExt;
use cli_boilerplate_automation::{bog::BogOkExt, broc::CommandExt};
use cli_boilerplate_automation::{ebog, else_default};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{path::PathBuf, process::exit};

use crate::cli::tool_types::LessfilterCommand;
use crate::lessfilter::helpers::{header_viewer, is_header, show_header};
use crate::utils::text::path_formatter;
use crate::{
    abspath::AbsPath,
    config::Config,
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
    let default = cfg.rules.get(Preset::Default).clone();
    let rules = cfg.rules.get_mut(preset);
    rules.prepend(default);

    let mut succeeded = false;

    for path in paths {
        let apath = AbsPath::new(path.clone());
        let data = FileData::new(apath.clone(), &cfg.test);
        log::debug!("file data: {data:?}");

        let rule = else_default!(rules.get_best_match(&path, data).ebog(format!("No rule for {}", path.to_string_lossy())); !);
        if rule.is_empty() {
            continue;
        }

        // show header
        if header == Some(true) {
            show_header(&path);
            succeeded = true;
        }
        log::debug!("rule found: {rule:?}");
        for action in rule.iter() {
            log::debug!("Action: {action:?}");
            if let Action::Custom(s) = action {
                let Some(template) = cfg.actions.get(s) else {
                    ebog!("The custom action '{s}' is not defined!");
                    continue;
                };
                let script = path_formatter(template, &AbsPath::new(path.clone()));
                let mut cmd = Command::from_script(&script);
                cmd.stdout(if matches!(preset, Preset::Edit) {
                    tty_or_inherit()
                } else {
                    Stdio::inherit()
                });
                succeeded |= cmd.status()._ebog().is_some_and(|s| s.success())
            } else {
                let (progs, perms) = action.to_progs(&path, preset);
                for mut prog in progs {
                    // filter out headers
                    if is_header(&prog) {
                        if header.is_some() {
                            show_header(&path);
                            continue;
                        } else {
                            succeeded = true
                        }
                    }

                    log::debug!("Executing: {prog:?}");
                    let mut cmd = Command::new(prog.remove(0));
                    cmd.args(prog).stdout(if matches!(preset, Preset::Edit) {
                        tty_or_inherit()
                    } else {
                        Stdio::inherit()
                    });

                    succeeded |= cmd.status()._ebog().is_some_and(|s| s.success())
                }
            }
        }
    }

    if succeeded { exit(0) } else { exit(1) }
}

//-------------------------
/// Struct representation of RulesConfig
#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    pub preview: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub display: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub extended: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub info: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub open: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub alternate: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub edit: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
    pub default: RuleMatcher<FileRule, ArrayVec<Action, 10>>,
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
    ) -> &RuleMatcher<FileRule, ArrayVec<Action, 10>> {
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
    ) -> &mut RuleMatcher<FileRule, ArrayVec<Action, 10>> {
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
