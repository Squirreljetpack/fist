use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileMetadata {
    pub permissions: Option<String>,
    pub size: Option<String>,
    pub btime: Option<String>,
    pub mtime: Option<String>,
    pub atime: Option<String>,
    pub extra: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub metadata: Option<FileMetadata>,
}

impl FileEntry {
    pub fn new(
        name: impl Into<String>,
        is_dir: bool,
    ) -> Self {
        Self {
            name: name.into(),
            is_dir,
            metadata: None,
        }
    }

    pub fn with_metadata(
        name: impl Into<String>,
        is_dir: bool,
        metadata: FileMetadata,
    ) -> Self {
        Self {
            name: name.into(),
            is_dir,
            metadata: Some(metadata),
        }
    }
}

impl Display for FileEntry {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> fmt::Result {
        let mut metas = Vec::new();
        if let Some(m) = &self.metadata {
            if let Some(p) = &m.permissions {
                metas.push(p.as_str());
            }
            if let Some(s) = &m.size {
                metas.push(s.as_str());
            }
            if let Some(b) = &m.btime {
                metas.push(b.as_str());
            }
            if let Some(t) = &m.mtime {
                metas.push(t.as_str());
            }
            if let Some(a) = &m.atime {
                metas.push(a.as_str());
            }
            if let Some(e) = &m.extra {
                metas.push(e.as_str());
            }
        }

        if metas.is_empty() {
            write!(f, "{}", self.name)
        } else {
            write!(f, "({}) {}", metas.join(" | "), self.name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_without_metadata() {
        let entry = FileEntry::new("script.sh", false);
        assert_eq!(entry.to_string(), "script.sh");
    }

    #[test]
    fn test_display_with_metadata() {
        let meta = FileMetadata {
            size: Some("4.2K".into()),
            mtime: Some("2026-08-22".into()),
            ..Default::default()
        };
        let entry = FileEntry::with_metadata("filename.sh", false, meta);
        assert_eq!(entry.to_string(), "(4.2K | 2026-08-22) filename.sh");
    }

    #[test]
    fn test_display_with_all_metadata() {
        let meta = FileMetadata {
            permissions: Some("0755".into()),
            size: Some("12.0M".into()),
            btime: None,
            mtime: Some("12:00".into()),
            atime: None,
            extra: Some("dir".into()),
        };
        let entry = FileEntry::with_metadata("my_dir", true, meta);
        assert_eq!(entry.to_string(), "(0755 | 12.0M | 12:00 | dir) my_dir");
    }
}
