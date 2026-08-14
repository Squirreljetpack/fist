use std::{ffi::OsString, path::Path, str::FromStr, sync::OnceLock};

use globset::{Glob as GlobBuilder, GlobMatcher};

use cba::{broc::has, bs::permissions, wbog};

use fist_types::{categories::FileCategory, filetypes::FileType};

use crate::{
    abspath::AbsPath,
    lessfilter::{
        mime_helpers::{detect_encoding, is_native, Myme},
        rule_matcher::{DefaultScore, Score, Test},
        Categories, LessfilterSettings, MimeString,
    },
};

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
    Glob {
        /// The original pattern, kept for lossless round-trip serialization.
        pattern: String,
        matcher: Glob,
    }, // since we have ext, this is probably used to define filters on custom paths
    /// Matches extension (e.g. "rs")
    /// Priority: 1
    Ext(String),
    /// Matches if the name of any child in the dir matches this glob
    /// Priority: 50
    Child {
        /// The original pattern, kept for lossless round-trip serialization.
        pattern: String,
        matcher: Glob,
    }, // Higher than Mime [ Directory, _ ]
    /// [type, subtype], e.g. ["image", "png"]
    /// Priority: [10, 20]
    ///
    /// # Special cases
    /// [Text, _]: also haves charset
    /// [_, x-elf]: tries to read file headers
    Mime(MimeString), // Higher than ext

    /// If the given key is the name of a [`FileCategory`], checks if the file matches it.
    /// Otherwise, check if the file's mime is contained in the user-defined [`Categories`] table under the given key.
    Cat(String), // Higher than ext
    /// True if the specified program doesn't exist.
    /// Parsed with invert from have:prog.
    /// Score modifiers should not be set on this rule!
    Have(String), // The default score has the effect: have:x -> NotHave -> Min(0). !have:x -> has x -> Min(0).

    /// Check if the file matches a known [`FileType`]
    /// A few additional broad file types are supported:
    /// - Text
    FileType(OverloadedFileType),
    /// Platform-specific application bundle/launcher/executable.
    Application,
    /// Always matches; parsed from the string `"*"`.
    Any,
    /// True if the path is inside a git work tree; parsed from the string
    /// `"git"`. Directories are checked as-is, files via their parent.
    Git,
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
                let dir = if self.path.is_dir() {
                    self.path.as_path()
                } else {
                    self.path.parent().unwrap_or(&self.path)
                };
                std::fs::read_dir(dir)
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
            FileRuleKind::Glob { matcher, .. } => matcher.is_match(&data.path),

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

            FileRuleKind::Child { matcher, .. } => data
                .children_names()
                .iter()
                .any(|child| matcher.is_match(child)),

            FileRuleKind::FileType(ft) => match ft {
                OverloadedFileType::Ft(ft) => ft == &data.ft,
                OverloadedFileType::Text => {
                    data.mime.kind.as_ref().is_some_and(|x| x.is_text())
                        || detect_encoding(item).as_deref().is_some_and(is_native)
                } // this computed for each test instead of being cached
            },

            FileRuleKind::Application => is_application_path(item),

            FileRuleKind::Have(cmd) => has(cmd),

            FileRuleKind::Any => true,

            FileRuleKind::Git => {
                // `git -C` chdirs, so directories are checked as-is and files
                // via their parent. The exit status alone is not enough: a
                // `.git` entry prints `false` with exit 0.
                let dir = if item.is_dir() {
                    item
                } else {
                    item.parent().unwrap_or(item)
                };
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(dir)
                    .args(["rev-parse", "--is-inside-work-tree"])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .output()
                    .ok()
                    .is_some_and(|out| {
                        out.status.success()
                            && String::from_utf8_lossy(&out.stdout).trim() == "true"
                    })
            }
        };
        if ok {
            log::trace!("{self:?} passed")
        }

        if self.invert {
            !ok
        } else {
            ok
        }
    }
}

impl DefaultScore for FileRule {
    fn default_score(&self) -> Score {
        match &self.kind {
            FileRuleKind::Glob { .. } => Score::Max(50),
            FileRuleKind::Child { .. } => Score::Max(50),
            FileRuleKind::Ext(_) => Score::Max(30),
            FileRuleKind::Mime(_) => Score::Max(20),
            FileRuleKind::Cat(_) => Score::Max(20),
            FileRuleKind::Have(_) => Score::Req,
            FileRuleKind::FileType(_) => Score::Req,
            FileRuleKind::Application => Score::Max(60),
            FileRuleKind::Any => Score::Max(0),
            FileRuleKind::Git => Score::Req,
        }
    }
}

pub fn is_application_path(path: &Path) -> bool {
    if cfg!(target_os = "macos") {
        return path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".app") || name.ends_with(".Application"));
    }

    if cfg!(target_os = "linux") {
        return path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".desktop") || name.ends_with(".AppImage"));
    }

    if cfg!(target_os = "windows") {
        return path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    }

    false
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
            } else if s == "*" {
                let kind = FileRuleKind::Any;
                Ok(FileRule { kind, invert })
            } else if s.eq_ignore_ascii_case("git") {
                let kind = FileRuleKind::Git;
                Ok(FileRule { kind, invert })
            } else if s.eq_ignore_ascii_case("application") || s.eq_ignore_ascii_case("app") {
                let kind = FileRuleKind::Application;
                Ok(FileRule { kind, invert })
            } else if let Ok(mime) = s.parse() {
                let kind = FileRuleKind::Mime(mime);
                Ok(FileRule { kind, invert })
            } else {
                Err(ParseFileRuleError::InvalidPrefix(s.to_string()))
            };
        };

        let kind = match kind {
            "glob" => FileRuleKind::Glob {
                pattern: rest.to_string(),
                matcher: GlobBuilder::new(rest)?.compile_matcher(),
            },
            "child" => FileRuleKind::Child {
                pattern: rest.to_string(),
                matcher: GlobBuilder::new(rest)?.compile_matcher(),
            },
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

impl std::fmt::Display for FileRule {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let invert = if self.invert { "!" } else { "" };
        match &self.kind {
            FileRuleKind::Glob { pattern, .. } => write!(f, "{invert}glob:{pattern}"),
            FileRuleKind::Ext(e) => write!(f, "{invert}ext:{e}"),
            FileRuleKind::Child { pattern, .. } => write!(f, "{invert}child:{pattern}"),
            FileRuleKind::Mime(m) => write!(f, "{invert}mime:{m}"),
            FileRuleKind::Cat(c) => write!(f, "{invert}cat:{c}"),
            FileRuleKind::Have(h) => write!(f, "{invert}have:{h}"),
            FileRuleKind::FileType(ft) => write!(f, "{invert}type:{ft}"),
            FileRuleKind::Application => write!(f, "{invert}application"),
            FileRuleKind::Any => write!(f, "{invert}*"),
            FileRuleKind::Git => write!(f, "{invert}git"),
        }
    }
}

impl std::fmt::Display for OverloadedFileType {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Ft(ft) => write!(f, "{ft}"),
            Self::Text => write!(f, "text"),
        }
    }
}

// -------------- SERDE ----------------

use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for FileRule {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for FileRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lessfilter::{Categories, InferMode, LessfilterSettings};
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    fn rule(s: &str) -> FileRule {
        s.parse().expect("rule should parse")
    }

    /// FileData for `path` with default settings and no custom categories.
    fn file_data<'a>(
        path: &Path,
        categories: &'a Categories,
    ) -> FileData<'a> {
        FileData::new(
            AbsPath::new(path.to_path_buf()),
            &LessfilterSettings::default(),
            categories,
        )
    }

    #[test]
    fn rule_strings_parse_and_roundtrip() {
        // one parse + display round-trip per rule kind
        for s in [
            "glob:*.rs",
            "child:src",
            "ext:rs",
            "mime:image/*",
            "mime:*/*",
            "cat:document",
            "have:sqlite3",
            "type:f",
            "type:d",
            "type:l",
            "type:x",
            "type:text",
            "application",
            "*",
            "git",
        ] {
            let parsed = s.parse::<FileRule>().expect(s);
            assert_eq!(parsed.to_string(), s, "round-trip failed for {s}");
        }

        // `app` is an alias for `application`, canonicalized on display
        let app = "app".parse::<FileRule>().unwrap();
        assert!(matches!(app.kind, FileRuleKind::Application));
        assert_eq!(app.to_string(), "application");

        // a bare `.ext` is shorthand for `ext:ext`, canonicalized on display
        let md = ".md".parse::<FileRule>().unwrap();
        assert!(matches!(md.kind, FileRuleKind::Ext(_)));
        assert_eq!(md.to_string(), "ext:md");
    }

    #[test]
    fn bang_prefix_inverts_a_rule() {
        let inverted = rule("!ext:rs");
        assert!(inverted.invert);
        assert_eq!(inverted.to_string(), "!ext:rs");
        assert!(!rule("ext:rs").invert);
    }

    #[test]
    fn invalid_rule_strings_are_rejected() {
        // type:file/type:directory are not strum serializes — only type:f,
        // type:d, ... parse (a regression test for the doc bug)
        for s in [
            "type:file",
            "type:directory",
            "mime:no-slash",
            "bogus:x",
            "type:",
            "not-a-rule",
        ] {
            assert!(s.parse::<FileRule>().is_err(), "{s} should be rejected");
        }
    }

    #[test]
    fn file_type_rules_match_what_they_name() {
        let dir = tempdir().unwrap();
        let categories = Categories::default();

        let file = dir.path().join("note.txt");
        File::create(&file).unwrap().write_all(b"hello").unwrap();
        let data = file_data(&file, &categories);
        assert!(rule("type:f").passes(&file, &data));
        assert!(!rule("type:d").passes(&file, &data));
        assert!(!rule("type:l").passes(&file, &data));
        assert!(rule("type:text").passes(&file, &data));

        let data = file_data(dir.path(), &categories);
        assert!(rule("type:d").passes(dir.path(), &data));
        assert!(!rule("type:f").passes(dir.path(), &data));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let exe = dir.path().join("run.sh");
            File::create(&exe)
                .unwrap()
                .write_all(b"#!/bin/sh\n")
                .unwrap();
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
            let data = file_data(&exe, &categories);
            // the executable bit classifies the file as type:x, not type:f
            assert!(rule("type:x").passes(&exe, &data));
            assert!(!rule("type:f").passes(&exe, &data));

            let link = dir.path().join("dir-link");
            std::os::unix::fs::symlink(dir.path(), &link).unwrap();
            let data = file_data(&link, &categories);
            assert!(rule("type:l").passes(&link, &data));
        }
    }

    #[test]
    fn ext_glob_and_child_rules() {
        let dir = tempdir().unwrap();
        let categories = Categories::default();

        let rs = dir.path().join("main.rs");
        File::create(&rs)
            .unwrap()
            .write_all(b"fn main() {}")
            .unwrap();
        let data = file_data(&rs, &categories);
        assert!(rule("ext:rs").passes(&rs, &data));
        assert!(!rule("ext:py").passes(&rs, &data));
        assert!(rule("glob:*.rs").passes(&rs, &data));
        assert!(!rule("glob:*.py").passes(&rs, &data));
        assert!(rule("!ext:py").passes(&rs, &data));
        assert!(rule("!glob:*.py").passes(&rs, &data));

        // child: a directory whose child matches the glob...
        let project = dir.path().join("project");
        fs::create_dir_all(project.join("src")).unwrap();
        let data = file_data(&project, &categories);
        assert!(rule("child:src").passes(&project, &data));

        // ...and for a file, the glob is matched against its siblings
        let readme = project.join("readme.md");
        File::create(&readme).unwrap().write_all(b"# hi").unwrap();
        let data = file_data(&readme, &categories);
        assert!(rule("child:src").passes(&readme, &data));
        assert!(!rule("child:dist").passes(&readme, &data));
    }

    #[test]
    fn mime_rules_match_by_magic_and_extension() {
        let dir = tempdir().unwrap();
        let categories = Categories::default();

        // a real png header is detected from content (FileFormat mode)
        let png = dir.path().join("magic.png");
        File::create(&png)
            .unwrap()
            .write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
            .unwrap();
        let data = file_data(&png, &categories);
        assert!(rule("mime:image/png").passes(&png, &data));
        assert!(rule("mime:image/*").passes(&png, &data));

        // Guess mode: an empty file with a known extension falls back to
        // the extension-based guess (FileFormat mode would classify it as
        // application/octet-stream, having no content to inspect)
        let empty = dir.path().join("empty.png");
        File::create(&empty).unwrap();
        let settings = LessfilterSettings {
            infer: InferMode::Guess,
            ..Default::default()
        };
        let data = FileData::new(AbsPath::new(empty.clone()), &settings, &categories);
        assert!(rule("mime:image/png").passes(&empty, &data));

        // mime:image/* is a wildcard: the type must match, the subtype may differ
        let txt = dir.path().join("note.txt");
        File::create(&txt).unwrap().write_all(b"hello").unwrap();
        let data = file_data(&txt, &categories);
        assert!(!rule("mime:image/*").passes(&txt, &data));
        assert!(rule("mime:text/*").passes(&txt, &data));
    }

    #[test]
    fn have_rule_checks_program_existence() {
        let dir = tempdir().unwrap();
        let categories = Categories::default();
        let path = dir.path();
        let data = file_data(path, &categories);

        // `sh` is on every unix; the second name is not a real program
        #[cfg(unix)]
        assert!(rule("have:sh").passes(path, &data));
        assert!(!rule("have:definitely-not-a-real-program-xyz").passes(path, &data));
    }

    #[test]
    fn any_and_git_rules() {
        let dir = tempdir().unwrap();
        let categories = Categories::default();

        let file = dir.path().join("whatever.xyz");
        File::create(&file).unwrap().write_all(b"data").unwrap();
        let data = file_data(&file, &categories);
        assert!(rule("*").passes(&file, &data));
        assert!(!rule("!*").passes(&file, &data));

        // a plain tempdir is not a git work tree
        let data = file_data(dir.path(), &categories);
        assert!(!rule("git").passes(dir.path(), &data));

        // the repo root (where these tests run from) is one
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
        let data = file_data(repo, &categories);
        assert!(rule("git").passes(repo, &data));
    }
}
