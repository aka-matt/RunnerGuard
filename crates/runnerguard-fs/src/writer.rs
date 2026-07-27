//! Atomic file writes: write to a sibling temp file, fsync, then rename.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("could not create parent directory for {path}: {source}")]
    Mkdir {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not create temp file for {path}: {source}")]
    CreateTemp {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write to temp file for {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not fsync temp file for {path}: {source}")]
    Sync {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not rename temp file to {path}: {source}")]
    Rename {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not clean up temp file {temp}: {source}")]
    Cleanup {
        temp: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Atomically write `content` to `path`. The destination either ends up
/// containing the entire new content, or it is unchanged. A best-effort
/// cleanup removes the temp file on failure.
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<(), WriteError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| WriteError::Mkdir {
                path: path.to_path_buf(),
                source,
            })?;
        }
    }

    let temp = match path.file_name() {
        Some(name) => {
            let mut s = name.to_os_string();
            s.push(".tmp");
            path.with_file_name(s)
        }
        None => path.with_extension("tmp"),
    };

    let write_result = (|| -> Result<(), WriteError> {
        let mut file = File::create(&temp).map_err(|source| WriteError::CreateTemp {
            path: path.to_path_buf(),
            source,
        })?;
        file.write_all(content)
            .map_err(|source| WriteError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        file.sync_all().map_err(|source| WriteError::Sync {
            path: path.to_path_buf(),
            source,
        })?;
        drop(file);
        fs::rename(&temp, path).map_err(|source| WriteError::Rename {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp).map_err(|source| WriteError::Cleanup {
            temp: temp.clone(),
            source,
        });
    }

    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_file_and_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.md");
        write_atomic(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn cleanup_runs_when_rename_fails() {
        // Force rename failure by making the destination a directory.
        let dir = tempfile::tempdir().unwrap();
        let blocking_dir = dir.path().join("blocker");
        fs::create_dir(&blocking_dir).unwrap();
        let result = write_atomic(&blocking_dir, b"x");
        assert!(result.is_err());
        // temp file should be gone
        let entries: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(entries.is_empty(), "expected no temp leftovers");
    }
}
