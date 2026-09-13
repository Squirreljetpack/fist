pub mod categories;
mod categories_phf;
pub mod filetypes;
pub mod filters;
mod ft_arg;
pub mod git;
pub mod icons;

pub use categories::FileCategory;

use cba::define_when;

define_when! {
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, clap::ValueEnum)]
    pub enum When {
        Never,
        #[default]
        Auto,
        Always
    }
}
impl When {
    pub fn cycle(&mut self) {
        *self = match self {
            When::Never => When::Auto,
            When::Auto => When::Always,
            When::Always => When::Never,
        };
    }
}
