//! Directory traversal that produces a [`ProjectFiles`] and respects the
//! configured [`Limits`].

use crate::classify::classify;
use crate::limits::Limits;
use crate::project::{ProjectFiles, SourceFile, SourceFileKind};
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("project root does not exist: {0}")]
    RootNotFound(PathBuf),
    #[error("project root is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("project root could not be canonicalised: {source}")]
    Canonicalise {
        #[source]
        source: std::io::Error,
    },
    #[error("project file count exceeds limit ({count} > {limit})")]
    TooManyFiles { count: usize, limit: usize },
}

#[derive(Debug, Clone)]
pub struct DiscoverOptions {
    pub limits: Limits,
    /// Skip MUnit XML and test resources.
    pub include_tests: bool,
}

/// Default discovery with the standard limits and `include_tests = false`.
pub fn default_project_files(root: &Path) -> Result<ProjectFiles, FsError> {
    discover(root, DiscoverOptions::default())
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            include_tests: false,
        }
    }
}

/// Walk `root` and produce a [`ProjectFiles`]. Files are returned in
/// lexicographic path order — that's our stable ordering for golden tests.
pub fn discover(root: &Path, options: DiscoverOptions) -> Result<ProjectFiles, FsError> {
    if !root.exists() {
        return Err(FsError::RootNotFound(root.to_path_buf()));
    }
    if !root.is_dir() {
        return Err(FsError::NotADirectory(root.to_path_buf()));
    }
    let canonical = root
        .canonicalize()
        .map_err(|source| FsError::Canonicalise { source })?;

    let mut files = ProjectFiles::new(canonical.clone());

    let walker = WalkDir::new(&canonical)
        .follow_links(options.limits.follow_symlinks)
        .min_depth(1)
        .into_iter();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                // We don't follow symlinks (limits.follow_symlinks defaults
                // to false), so walkdir never raises a loop error in the
                // common case. Surface the path for visibility, then skip
                // — one unreadable file should not abort a scan.
                tracing::debug!(?err, "skipping walkdir entry");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let absolute = entry.path();
        let relative = match absolute.strip_prefix(&canonical) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let relative_str = path_to_forward_slash(relative);
        if !options.include_tests && is_test_path(&relative_str) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if metadata.len() > options.limits.max_file_bytes {
            continue;
        }

        let source = classify(&relative_str, metadata.len());
        files.push(source);

        if files.len() > options.limits.max_project_files {
            return Err(FsError::TooManyFiles {
                count: files.len(),
                limit: options.limits.max_project_files,
            });
        }
    }

    files.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn path_to_forward_slash(path: &Path) -> String {
    let mut parts = Vec::new();
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if !s.is_empty() {
            parts.push(s.to_string());
        }
    }
    parts.join("/")
}

fn is_test_path(relative: &str) -> bool {
    relative.starts_with("src/test/")
}

/// Convenience for seed fixtures and small unit tests: build a
/// [`ProjectFiles`] from an in-memory list. Skips validation entirely; do
/// not use this for untrusted input.
pub fn from_list<I>(root: PathBuf, entries: I) -> ProjectFiles
where
    I: IntoIterator<Item = (String, u64)>,
{
    let mut files = ProjectFiles::new(root);
    for (path, size) in entries {
        let kind = crate::classify::classify(&path, size).kind;
        files.push(SourceFile::new(path, size, kind));
    }
    files.files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[allow(dead_code)]
const fn _ensure_kinds_match(_: SourceFileKind) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::SourceFileKind;

    #[test]
    fn rejects_missing_root() {
        let err = discover(Path::new("does/not/exist"), DiscoverOptions::default()).unwrap_err();
        assert!(matches!(err, FsError::RootNotFound(_)));
    }

    #[test]
    fn rejects_file_root() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir.xml");
        std::fs::write(&file, "<x/>").unwrap();
        let err = discover(&file, DiscoverOptions::default()).unwrap_err();
        assert!(matches!(err, FsError::NotADirectory(_)));
    }

    #[test]
    fn discovers_a_small_tree_in_lexicographic_order() {
        let dir = tempfile::tempdir().unwrap();
        let mule = dir.path().join("src").join("main").join("mule");
        std::fs::create_dir_all(&mule).unwrap();
        std::fs::write(mule.join("a.xml"), "<x/>").unwrap();
        std::fs::write(mule.join("b.xml"), "<x/>").unwrap();
        std::fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();

        let files = discover(dir.path(), DiscoverOptions::default()).unwrap();
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["pom.xml", "src/main/mule/a.xml", "src/main/mule/b.xml"]
        );
    }

    #[test]
    fn skips_test_paths_when_include_tests_false() {
        let dir = tempfile::tempdir().unwrap();
        let test_dir = dir.path().join("src").join("test").join("munit");
        std::fs::create_dir_all(&test_dir).unwrap();
        std::fs::write(test_dir.join("a.xml"), "<x/>").unwrap();
        std::fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();

        let files = discover(dir.path(), DiscoverOptions::default()).unwrap();
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["pom.xml"]);
    }

    #[test]
    fn classifies_files_under_resources() {
        let dir = tempfile::tempdir().unwrap();
        let res = dir.path().join("src").join("main").join("resources");
        std::fs::create_dir_all(&res).unwrap();
        std::fs::write(res.join("app.yaml"), "key: value").unwrap();
        std::fs::write(res.join("transform.dwl"), "%dw 2.0\n---\n1").unwrap();

        let files = discover(dir.path(), DiscoverOptions::default()).unwrap();
        assert!(files.iter().any(|f| f.kind == SourceFileKind::ResourceYaml));
        assert!(
            files
                .iter()
                .any(|f| f.kind == SourceFileKind::ResourceDataweave)
        );
    }
}
