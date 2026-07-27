//! Platform-specific default config paths and parent-dir helpers.

use std::path::{Path, PathBuf};

/// Default per-user config path. Falls back to a relative path if the
/// platform directories lookup fails.
pub fn default_config_path() -> PathBuf {
    if let Some(dirs) = directories::ProjectDirs::from("invalid", "example", "runnerguard") {
        let mut path = dirs.config_dir().to_path_buf();
        path.push("config.yaml");
        return path;
    }
    PathBuf::from("./runnerguard.yaml")
}

/// Make sure `path`'s parent exists. Returns the parent path.
pub fn ensure_parent_dir(path: &Path) -> std::io::Result<PathBuf> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_is_non_empty_and_well_formed() {
        let path = default_config_path();
        assert!(!path.as_os_str().is_empty());
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("config.yaml")
        );
    }

    #[test]
    fn ensure_parent_creates_missing_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("config.yaml");
        ensure_parent_dir(&nested).unwrap();
        assert!(nested.parent().unwrap().is_dir());
    }
}
