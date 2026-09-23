use cba::bath::PathExt;
use std::path::Path;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, strum_macros::Display, PartialEq, Eq, clap::ValueEnum)]
#[cfg_attr(
    feature = "serde",
    derive(
        serde::Serialize,
        serde::Deserialize,
        strum_macros::EnumIter,
        strum_macros::IntoStaticStr,
    )
)]
#[strum(serialize_all = "lowercase")]
pub enum SortOrder {
    name,
    mtime,
    atime,
    size,
    #[default]
    none,
}

impl SortOrder {
    pub fn cycle(&mut self) {
        *self = match self {
            SortOrder::name => SortOrder::mtime,
            SortOrder::mtime => SortOrder::atime,
            SortOrder::atime => SortOrder::size,
            SortOrder::size => SortOrder::none,
            SortOrder::none => SortOrder::name,
        };
    }

    /// Display label for prompts/overlays.
    /// In db panes (files/folders/apps), `none` means frecency and the
    /// other variants map to their SQL orderings.
    pub fn label(
        &self,
        db: bool,
    ) -> &'static str {
        if db {
            match self {
                SortOrder::name => "name",
                SortOrder::mtime => "none",
                SortOrder::atime => "atime",
                SortOrder::size => "count",
                SortOrder::none => "frecency",
            }
        } else {
            match self {
                SortOrder::name => "name",
                SortOrder::mtime => "mtime",
                SortOrder::atime => "atime",
                SortOrder::size => "size",
                SortOrder::none => "none",
            }
        }
    }
}

// ------------------------------------------------------------
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    /// show hidden files and folders
    pub hidden: bool,
    /// show hidden files only.
    /// When combined with dir or files, the effect is inclusive: hidden or a file, hidden or a directory.
    pub hidden_only: bool,
    /// HIDE ignored files
    pub ignore: bool,
    /// show all
    all: bool,

    /// only show directories
    pub dirs: bool,
    /// show only files
    pub files: bool,

    /// Don't follow symlinks (tui only).
    pub no_follow: bool,
}

impl Visibility {
    pub const DEFAULT: Self = Self {
        all: false,
        no_follow: false,
        hidden: false,
        hidden_only: false,
        ignore: false,
        dirs: false,
        files: false,
    };

    pub fn enable_hidden_if_empty_otherwise(
        mut self,
        cwd: &Path,
        modify_if: bool,
    ) -> Self {
        if !self.hidden && modify_if {
            // automatic flag set for directories with only hidden files
            let only_hidden = std::fs::read_dir(cwd)
                .map(|mut entries| {
                    entries.all(|entry| {
                        entry
                            .map(|e| {
                                e.file_name()
                                    .to_str()
                                    .map(|s| s.starts_with('.'))
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if only_hidden {
                self.hidden = true
            }
        }
        self
    }

    // note: does rust know to get all these metadata checks in one go?
    pub fn post_nav_filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            return path.is_hidden()
                || if self.dirs {
                    path.is_dir()
                } else if self.files {
                    path.is_file()
                } else {
                    false
                };
        } else if !self.hidden {
            push &= !path.is_hidden()
        }

        if self.dirs {
            push &= path.is_dir()
        } else if self.files {
            push &= path.is_file()
        }

        push
    }

    /// applies the full visibility filter (notes: checks exists(), ignore not implemented)
    pub fn filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;
        if !self.all {
            push &= path.exists()
        }

        if self.hidden_only {
            return path.is_hidden()
                || if self.dirs {
                    path.is_dir()
                } else if self.files {
                    path.is_file()
                } else {
                    false
                };
        } else if !self.hidden {
            push &= !path.is_hidden()
        }

        if self.dirs {
            push &= path.is_dir()
        } else if self.files {
            push &= path.is_file()
        }

        if self.ignore {
            // lowpri: todo
        }

        push
    }

    pub fn post_fd_filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            push = path.is_hidden()
                || if self.dirs {
                    path.is_dir()
                } else if self.files {
                    path.is_file()
                } else {
                    false
                };
        };

        push
    }

    pub fn is_default(&self) -> bool {
        *self == Self::DEFAULT
    }

    pub fn all(&self) -> bool {
        self.all
    }
    pub fn set_all(
        &mut self,
        all: bool,
    ) {
        if all {
            *self = Visibility {
                all: true,
                dirs: self.dirs,
                no_follow: self.no_follow,
                ..Default::default()
            }
        } else {
            self.all = false;
        }
    }
    pub fn toggle_all(&mut self) {
        if self.all() {
            self.set_all(false)
        } else {
            self.set_all(true)
        }
    }
    pub fn set_default(&mut self) {
        *self = Visibility {
            dirs: self.dirs,
            ..Default::default()
        }
    }
    pub fn include_hidden(&self) -> bool {
        self.hidden || self.hidden_only
    }

    pub fn validated(mut self) -> Self {
        if self.all {
            self.set_all(true);
        }
        if self.dirs {
            self.files = false
        } else if self.files {
            self.dirs = false
        }
        self
    }
    // fn set_depth(&mut self, depth: usize) {
    //     self.depth = depth.max(1)
    // }
    // fn depth(&self) -> usize {
    //     self.depth
    // }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::Args)]
pub struct PartialVisibility {
    /// Show hidden files and folders
    #[arg(short = 'h', overrides_with = "no_hidden")]
    pub hidden: bool,

    /// Hide hidden files and folders
    #[arg(short = 'H', overrides_with = "hidden")]
    pub no_hidden: bool,

    /// Show ignored files
    #[arg(short = 'i', overrides_with = "ignore")]
    pub no_ignore: bool,

    /// Hide ignored files
    #[arg(short = 'I', overrides_with = "no_ignore")]
    pub ignore: bool,

    #[arg(short = 'u', overrides_with = "no_unrestricted")]
    pub unrestricted: bool,

    #[arg(short = 'U', overrides_with = "unrestricted")]
    pub no_unrestricted: bool,

    /// Only show directories
    #[arg(short = 'F', overrides_with = "files")]
    pub dirs: bool,

    /// Show only files
    #[arg(short = 'f', overrides_with = "dirs")]
    pub files: bool,

    /// Don't follow symlinks (tui only).
    #[arg(skip)]
    pub no_follow: Option<bool>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for PartialVisibility {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        if let Some(v) = self.hidden() {
            map.serialize_entry("hidden", &v)?;
        }
        if let Some(v) = self.ignore() {
            map.serialize_entry("ignore", &v)?;
        }
        if let Some(v) = self.unrestricted() {
            map.serialize_entry("unrestricted", &v)?;
        }
        if let Some(v) = self.files() {
            map.serialize_entry("files", &v)?;
        }
        if let Some(nf) = self.no_follow {
            map.serialize_entry("follow", &!nf)?;
        }
        map.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PartialVisibility {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = PartialVisibility;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                f.write_str("a visibility filter map")
            }

            fn visit_map<M>(
                self,
                mut access: M,
            ) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut pv = PartialVisibility::default();
                while let Some(key) = access.next_key::<String>()? {
                    match key.as_str() {
                        "hidden" => pv.set_hidden(access.next_value()?),
                        "ignore" => pv.set_ignore(access.next_value()?),
                        "unrestricted" => pv.set_unrestricted(access.next_value()?),
                        "files" => pv.set_files(access.next_value()?),
                        "follow" => {
                            let follow: bool = access.next_value()?;
                            pv.no_follow = Some(!follow);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(&key, FIELDS));
                        }
                    }
                }
                Ok(pv)
            }
        }

        const FIELDS: &[&str] = &["hidden", "ignore", "unrestricted", "files", "follow"];
        deserializer.deserialize_struct("PartialVisibility", FIELDS, Visitor)
    }
}

impl PartialVisibility {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn hidden(&self) -> Option<bool> {
        if self.hidden {
            Some(true)
        } else if self.no_hidden {
            Some(false)
        } else {
            None
        }
    }

    pub fn ignore(&self) -> Option<bool> {
        if self.ignore {
            Some(true)
        } else if self.no_ignore {
            Some(false)
        } else {
            None
        }
    }

    pub fn unrestricted(&self) -> Option<bool> {
        if self.unrestricted {
            Some(true)
        } else if self.no_unrestricted {
            Some(false)
        } else {
            None
        }
    }

    pub fn files(&self) -> Option<bool> {
        if self.files {
            Some(true)
        } else if self.dirs {
            Some(false)
        } else {
            None
        }
    }

    pub fn set_hidden(
        &mut self,
        val: bool,
    ) {
        self.hidden = val;
        self.no_hidden = !val;
    }

    pub fn set_ignore(
        &mut self,
        val: bool,
    ) {
        self.ignore = val;
        self.no_ignore = !val;
    }

    pub fn set_unrestricted(
        &mut self,
        val: bool,
    ) {
        self.unrestricted = val;
        self.no_unrestricted = !val;
    }

    pub fn set_files(
        &mut self,
        val: bool,
    ) {
        self.files = val;
        self.dirs = !val;
    }

    pub fn into_resolved(
        mut self,
        cfg: Option<Self>,
        smart_git: bool,
    ) -> Visibility {
        let mut vis = Visibility::default();

        if self.is_default() {
            if let Some(cfg) = cfg {
                vis.apply(cfg);
            } else if smart_git && super::git::in_git_repo(std::env::current_dir().ok()) {
                vis.hidden = true;
                vis.ignore = true;
            };
        } else {
            if self.hidden().is_none() && self.ignore().is_none() {
                // Config specifies a default cfg (for the pane)
                if let Some(cfg) = cfg {
                    if let Some(h) = cfg.hidden() {
                        self.set_hidden(h);
                    }
                    if let Some(i) = cfg.ignore() {
                        self.set_ignore(i);
                    }
                // automatic flag set for git repo
                } else if smart_git && super::git::in_git_repo(std::env::current_dir().ok()) {
                    vis.hidden = true;
                    vis.ignore = true;
                }
            }
            vis.apply(self);
        }
        vis
    }
}

impl Visibility {
    pub fn apply(
        &mut self,
        patch: PartialVisibility,
    ) {
        if let Some(v) = patch.hidden() {
            self.hidden = v;
        }
        if let Some(v) = patch.ignore() {
            self.ignore = v;
        }
        if let Some(v) = patch.unrestricted() {
            self.all = v;
        }
        if let Some(v) = patch.files() {
            if v {
                self.files = true;
                self.dirs = false;
            } else {
                self.files = false;
                self.dirs = true;
            }
        }
        if let Some(v) = patch.no_follow {
            self.no_follow = v;
        }
        *self = self.validated();
    }
}
