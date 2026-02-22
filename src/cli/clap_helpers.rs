use clap::{Args, ValueEnum};

use fist_types::When;

#[derive(Debug, ValueEnum, Default, Clone, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum ClapStyleSetting {
    Icons,
    Colors,
    None,
    All,
    #[default]
    Auto,
}

#[derive(Debug, Default, Clone, Args)]
pub struct CaseArgs {
    #[arg(
        short = 'i',
        long = "ignore-case",
        overrides_with_all = ["case_sensitive", "smart_case"]
    )]
    ignore_case: bool,

    #[arg(
        short = 's',
        long = "case-sensitive",
        overrides_with_all = ["ignore_case", "smart_case"]
    )]
    case_sensitive: bool,

    #[arg(
        short = 'S',
        long = "smart-case",
        overrides_with_all = ["ignore_case", "case_sensitive"]
    )]
    smart_case: bool,
}

impl CaseArgs {
    pub fn resolve(&self) -> When {
        if self.smart_case {
            When::Auto
        } else if self.ignore_case {
            When::Never
        } else if self.case_sensitive {
            When::Always
        } else {
            When::Auto
        }
    }
}

#[derive(Args, Debug, Default, Clone)]
pub struct ContextArgs {
    /// Show NUM lines after each match.
    #[arg(short = 'A', long = "after-context", value_name = "NUM")]
    pub after: Option<usize>,

    /// Show NUM lines before each match.
    #[arg(short = 'B', long = "before-context", value_name = "NUM")]
    pub before: Option<usize>,

    /// Show NUM lines before and after each match.
    #[arg(short = 'C', long = "context", value_name = "NUM")]
    pub context: Option<usize>,
}

impl ContextArgs {
    pub fn resolve(self) -> [usize; 2] {
        let base = self.context.unwrap_or(0);

        let before = self.before.unwrap_or(base);
        let after = self.after.unwrap_or(base);

        [before, after]
    }
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ListMode {
    #[value(name = "_")]
    Default,
    All,
}
