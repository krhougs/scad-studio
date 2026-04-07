use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    Viewer,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentKey {
    pub kind: DocumentKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDescriptor {
    pub key: DocumentKey,
    pub base_title: String,
}

impl DocumentDescriptor {
    pub fn new(kind: DocumentKind, path: PathBuf) -> Self {
        let path = normalize_path(path);
        Self {
            base_title: file_title(&path),
            key: DocumentKey { kind, path },
        }
    }

    pub fn path(&self) -> &Path {
        &self.key.path
    }
}

fn normalize_path(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

fn file_title(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Document")
        .to_owned()
}
