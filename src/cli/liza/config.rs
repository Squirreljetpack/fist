use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewMode {
    Nav,
    Git,
    Dirs,
    Flatten,
    Tree,
    Recent,
}

#[derive(Debug, Clone, Default)]
pub struct LizaConfig {
    pub show_help: bool,
    pub verbose: bool,
    pub view_mode: Option<ViewMode>,

    // Filters & views
    pub all: bool,
    pub git_ignore: bool,
    pub tree_depth: Option<usize>,
    pub unbounded_tree: bool,
    pub pretty: bool,
    pub one_line: bool,
    pub header: bool,
    pub no_header: bool,
    pub git_status: bool,

    // Columns
    pub show_mtime: bool,
    pub show_octal: bool,
    pub show_time: bool,
    pub show_size: bool,
    pub show_clean_long: bool,
    pub show_extensive: bool,

    // Column exclusion tracking for eza long mode
    pub no_filesize: bool,
    pub no_user: bool,
    pub no_permissions: bool,
    pub no_time: bool,

    // Raw passthrough flags starting with '-'
    pub passthrough_args: Vec<OsString>,

    // Target paths
    pub paths: Vec<PathBuf>,
}
