#![allow(unused)]

pub mod categories;
mod categories_phf;
pub mod filetypes;
pub mod filters;
mod ft_arg;
pub mod icons;

pub use categories::FileCategory;

use cli_boilerplate_automation::define_when;
define_when! {
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
    pub enum When {
        #[cfg_attr(feature = "serde", serde(alias = "false", alias = "never"))]
        Never,
        #[default]
        #[cfg_attr(feature = "serde", serde(alias = "auto"))]
        Auto,
        #[cfg_attr(feature = "serde", serde(alias = "true", alias = "always"))]
        Always
    }
}
