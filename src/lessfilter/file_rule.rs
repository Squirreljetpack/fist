use cli_boilerplate_automation::broc::has;
use cli_boilerplate_automation::bs::permissions;
use globset::{Glob as GlobBuilder, GlobMatcher};
use mime_guess::{Mime, mime};
use std::ffi::OsString;
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::abspath::AbsPath;
use crate::lessfilter::TestSettings;
use crate::lessfilter::mime_helpers::Myme;
use crate::lessfilter::rule_matcher::{DefaultScore, Score, Test};

/// compiled GlobMatcher
pub type Glob = GlobMatcher;

/// Appearing on the right of the [`super::RuleMatcher`], this is tested against a path to produce a [`super::rule_matcher::Score`]
#[derive(Debug, Clone)]
pub struct FileRule {
    pub kind: FileRuleKind,
    pub invert: bool,
}

#[derive(Debug, Clone)]
pub enum FileRuleKind {
    /// Matches the file's full path
    /// Priority: 100
    Glob(Glob), // since we have ext, this is probably used to define filters on custom paths
    /// Matches extension (e.g. "rs")
    /// Priority: 1
    Ext(String),
    /// Matches if the name of any child in the dir matches this glob
    /// Priority: 50
    Child(Glob), // Higher than Mime [ Directory, _ ]
    /// [type, subtype], e.g. ["image", "png"]
    /// Priority: [10, 20]
    ///
    /// # Special cases
    /// [Text, _]: also haves charset
    /// [_, x-elf]: tries to read file headers
    Mime([String; 2]), // Higher than ext
    /// True if the specified program doesn't exist.
    /// Parsed with invert from have:prog.
    /// Score modifiers should not be set on this rule!
    NotHave(String), // The default score has the effect: have:x -> NotHave -> Min(0). !have:x -> has x -> Min(0).
}

/// This is the [`super::rule_matcher::Test::Context`] for a path
#[derive(Debug)]
pub struct FileData {
    pub path: AbsPath,
    pub children: OnceLock<Vec<OsString>>,
    // [type, subtype]
    pub mime: Myme,
    // [read, write, execute]
    pub permissions: [bool; 3],
}

impl FileData {
    #[allow(clippy::collapsible_if)]
    pub fn new(
        path: AbsPath,
        settings: &TestSettings,
    ) -> Self {
        // 2. Permissions (Read, Write, Execute)
        let permissions = permissions(&path);

        // 3. Mime Detection
        let mime = Myme::from_path(&path, settings.infer);

        Self {
            path,
            children: OnceLock::new(),
            mime,
            permissions,
        }
    }

    /// for [`FileRuleKind::Child`]
    fn children_names(&self) -> &[OsString] {
        self.children
            .get_or_init(|| {
                std::fs::read_dir(&self.path)
                    .ok()
                    .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name()).collect())
                    .unwrap_or_default()
            })
            .as_slice()
    }
}

impl Test<Path> for FileRule {
    type Context = FileData;

    fn passes(
        &self,
        item: &Path,
        data: &FileData,
    ) -> bool {
        let ok = match &self.kind {
            FileRuleKind::Glob(matcher) => matcher.is_match(&data.path),

            FileRuleKind::Ext(target_ext) => {
                if let Some(e) = item.extension().and_then(|e| e.to_str()) {
                    e.eq_ignore_ascii_case(target_ext)
                } else {
                    target_ext.is_empty()
                }
            }

            FileRuleKind::Mime([type_, subtype]) => {
                let mime = &data.mime;
                let type_ok =
                    type_.is_empty() || type_ == "*" || mime.mime.type_() == type_.as_str();
                let subtype_ok =
                    subtype.is_empty() || subtype == "*" || mime.mime.subtype() == subtype.as_str();

                let charset_text_ok = mime
                    .enc
                    .as_ref()
                    .map(|c| {
                        let c = c.as_str().to_ascii_lowercase();
                        c.contains("utf-8") || c.contains("unicode") || c.contains("ascii")
                    })
                    .unwrap_or(false);

                if type_ == "text" {
                    (type_ok && subtype_ok)
                        || ((subtype.is_empty() || subtype == "*") && charset_text_ok)
                } else {
                    type_ok && subtype_ok
                }
            }

            FileRuleKind::Child(child_glob) => data
                .children_names()
                .iter()
                .any(|child| child_glob.is_match(child)),

            FileRuleKind::NotHave(cmd) => has(cmd),
        };

        if self.invert { !ok } else { ok }
    }
}

impl DefaultScore for FileRule {
    fn default_score(&self) -> Score {
        match &self.kind {
            FileRuleKind::Glob(_) => Score::Max(100),
            FileRuleKind::Child(_) => Score::Max(50),
            FileRuleKind::Mime(_) => Score::Max(20),
            FileRuleKind::Ext(_) => Score::Max(10),
            FileRuleKind::NotHave(_) => Score::Min(0), // if don't have, set to 0, yes this is confusing when priority is given
        }
    }
}
// -------------- PARSING --------------------

#[derive(Debug, thiserror::Error)]
pub enum ParseFileRuleError {
    #[error("invalid file rule prefix")]
    InvalidPrefix,

    #[error("missing file rule prefix")]
    MissingPrefix,

    #[error("invalid mime specifier (expected type/subtype)")]
    InvalidMime,

    #[error(transparent)]
    InvalidGlob(#[from] globset::Error),
}

impl FromStr for FileRule {
    type Err = ParseFileRuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (invert, s) = match s.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, s),
        };

        let Some((kind, rest)) = s.split_once(':') else {
            return if let Some(s) = s.strip_prefix('.') {
                let kind = FileRuleKind::Ext(s.to_string());
                Ok(FileRule { kind, invert })
            } else if let Some((ty, sub)) = s.split_once('/') {
                let kind = FileRuleKind::Mime([ty.to_string(), sub.to_string()]);
                Ok(FileRule { kind, invert })
            } else {
                Err(ParseFileRuleError::InvalidPrefix)
            };
        };

        let kind = match kind {
            "glob" => FileRuleKind::Glob(GlobBuilder::new(rest)?.compile_matcher()),
            "child" => FileRuleKind::Child(GlobBuilder::new(rest)?.compile_matcher()),
            "ext" => FileRuleKind::Ext(rest.to_string()),
            "mime" => {
                let (ty, sub) = rest
                    .split_once('/')
                    .ok_or(ParseFileRuleError::InvalidMime)?;
                FileRuleKind::Mime([ty.to_string(), sub.to_string()])
            }
            "have" => {
                return Ok(FileRule {
                    kind: FileRuleKind::NotHave(rest.to_string()),
                    invert: !invert,
                });
            }
            _ => return Err(ParseFileRuleError::InvalidPrefix),
        };

        Ok(FileRule { kind, invert })
    }
}
