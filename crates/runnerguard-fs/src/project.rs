//! Project files collection and the source-file descriptor type.
//!
//! `SourceFile` and `SourceFileKind` are the canonical types defined in
//! `runnerguard-model`; we re-export them here so callers can depend on
//! just this crate without reaching into the model crate for a type they
//! usually want alongside `ProjectFiles`.

pub use runnerguard_model::{SourceFile, SourceFileKind};

/// Files discovered under a Mule project, plus descriptors of where they
/// came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectFiles {
    pub root: std::path::PathBuf,
    pub files: Vec<SourceFile>,
}

impl ProjectFiles {
    pub fn new(root: std::path::PathBuf) -> Self {
        Self {
            root,
            files: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
    }

    pub fn push(&mut self, file: SourceFile) {
        self.files.push(file);
    }

    pub fn of_kind(&self, kind: SourceFileKind) -> impl Iterator<Item = &SourceFile> {
        self.files.iter().filter(move |f| f.kind == kind)
    }
}
