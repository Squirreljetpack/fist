use cli_boilerplate_automation::broc::has;
use cli_boilerplate_automation::bs::permissions;
use cli_boilerplate_automation::{unwrap, wbog};
use globset::{Glob as GlobBuilder, GlobMatcher};
use mime_guess::{Mime, mime};
use std::ffi::OsString;
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;

use crate::abspath::AbsPath;
use crate::lessfilter::mime_helpers::{Myme, detect_encoding, is_native};
use crate::lessfilter::rule_matcher::{DefaultScore, Score, Test};
use crate::lessfilter::{Categories, LessfilterSettings, MimeString};
use crate::utils::categories::FileCategory;
use crate::utils::filetypes::FileType;

/// compiled GlobMatcher
pub type Glob = GlobMatcher;

/// Appearing on the right of the [`super::RuleMatcher`], this is tested against a path to produce a [`super::rule_matcher::Score`]
#[derive(Debug, Clone)]
pub struct FileRule {
    pub kind: FileRuleKind,
    pub invert: bool,
}

impl From<FileRuleKind> for FileRule {
    fn from(kind: FileRuleKind) -> Self {
        Self {
            kind,
            invert: false,
        }
    }
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
    Mime(MimeString), // Higher than ext

    Cat(String), // Higher than ext
    /// True if the specified program doesn't exist.
    /// Parsed with invert from have:prog.
    /// Score modifiers should not be set on this rule!
    Have(String), // The default score has the effect: have:x -> NotHave -> Min(0). !have:x -> has x -> Min(0).

    FileType(OverloadedFileType),
}

/// Overloads FileType to add a Text variant, which is matched on all native text (utf-8/utf-16).
#[derive(Debug, Clone)]
pub enum OverloadedFileType {
    Ft(FileType),
    Text,
}

/// This is the [`super::rule_matcher::Test::Context`] for a path
#[derive(Debug)]
pub struct FileData<'a> {
    pub path: AbsPath,
    pub children: OnceLock<Vec<OsString>>,
    pub mime: Myme,
    /// [read, write, execute]
    pub permissions: [bool; 3],
    pub ft: FileType,
    pub categories: &'a Categories,
}

impl<'a> FileData<'a> {
    #[allow(clippy::collapsible_if)]
    pub fn new(
        path: AbsPath,
        settings: &LessfilterSettings,
        categories: &'a Categories,
    ) -> Self {
        // 1. Permissions (Read, Write, Execute)
        let permissions = permissions(&path);
        let ft = FileType::get(&path);

        // 2. Mime Detection
        let mime = if matches!(
            ft,
            FileType::File | FileType::Directory | FileType::Executable | FileType::Symlink
        ) {
            Myme::from_path(&path, settings.infer)
        } else {
            Myme::default()
        };

        Self {
            path,
            children: OnceLock::new(),
            mime,
            ft,
            permissions,
            categories,
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
    type Context<'a> = FileData<'a>;

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

            FileRuleKind::Mime(mime_) => {
                if let Some(mime) = &data.mime.mime {
                    mime_.matches_type(mime.type_().as_str())
                        && mime_.matches_subtype(mime.subtype().as_str())
                } else {
                    mime_.matches_any()
                }
            }

            FileRuleKind::Cat(s) => {
                if let Ok(kind) = s.parse::<FileCategory>() {
                    return data.mime.kind == Some(kind);
                };

                let Myme {
                    mime: Some(mime), ..
                } = &data.mime
                else {
                    return false;
                };
                if let Some(mimes) = data.categories.get(s) {
                    mimes.iter().any(|m| m.equal(mime))
                } else {
                    wbog!("Invalid file rule: No category named {s}.");
                    false
                }
            }

            FileRuleKind::Child(child_glob) => data
                .children_names()
                .iter()
                .any(|child| child_glob.is_match(child)),

            FileRuleKind::FileType(ft) => match ft {
                OverloadedFileType::Ft(ft) => ft == &data.ft,
                OverloadedFileType::Text => detect_encoding(item).as_deref().is_some_and(is_native), // this computed for each test instead of being cached
            },

            FileRuleKind::Have(cmd) => has(cmd),
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
            FileRuleKind::Cat(_) => Score::Max(20),
            FileRuleKind::Ext(_) => Score::Max(10),
            FileRuleKind::Have(_) => Score::Req,
            FileRuleKind::FileType(_) => Score::Req,
        }
    }
}
// -------------- PARSING --------------------

#[derive(Debug, thiserror::Error)]
pub enum ParseFileRuleError {
    #[error("invalid file rule prefix: {0}")]
    InvalidPrefix(String),

    #[error("missing file rule prefix")]
    MissingPrefix,

    #[error("invalid mime specifier (expected type/subtype)")]
    InvalidMime,

    #[error("invalid filetype specifier: {0}")]
    InvalidFileType(#[from] strum::ParseError),

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
            } else if let Ok(mime) = s.parse() {
                let kind = FileRuleKind::Mime(mime);
                Ok(FileRule { kind, invert })
            } else {
                Err(ParseFileRuleError::InvalidPrefix(s.to_string()))
            };
        };

        let kind = match kind {
            "glob" => FileRuleKind::Glob(GlobBuilder::new(rest)?.compile_matcher()),
            "child" => FileRuleKind::Child(GlobBuilder::new(rest)?.compile_matcher()),
            "ext" => FileRuleKind::Ext(rest.to_string()),
            "mime" => FileRuleKind::Mime(rest.parse()?),
            "have" => {
                return Ok(FileRule {
                    kind: FileRuleKind::Have(rest.to_string()),
                    invert,
                });
            }
            "cat" | "category" => {
                return Ok(FileRule {
                    kind: FileRuleKind::Cat(rest.to_string()),
                    invert,
                });
            }
            "type" => {
                let ft = match rest {
                    "text" => OverloadedFileType::Text,
                    _ => OverloadedFileType::Ft(rest.parse()?),
                };
                return Ok(FileRule {
                    kind: FileRuleKind::FileType(ft),
                    invert,
                });
            }
            _ => return Err(ParseFileRuleError::InvalidPrefix(kind.to_string())),
        };

        Ok(FileRule { kind, invert })
    }
}
