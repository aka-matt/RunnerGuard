//! YAML loading + parse errors.

use crate::app_config::AppConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config file {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("YAML in {path} is not valid: {source}")]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: serde_yaml_ng::Error,
    },
    #[error("unsupported config version {found}; this build supports versions up to {supported}")]
    UnsupportedVersion { found: u32, supported: u32 },
}

/// Result of loading: the parsed config plus the source it came from (so
/// `config show` can label the output).
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    pub source: ConfigSource,
    pub config: AppConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    Path(std::path::PathBuf),
    Stdin,
    Default,
}

const SUPPORTED_VERSION: u32 = 1;

/// Load a config from a YAML file.
pub fn load_from_path(path: &std::path::Path) -> Result<LoadedConfig, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut config: AppConfig =
        serde_yaml_ng::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    if config.version > SUPPORTED_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: config.version,
            supported: SUPPORTED_VERSION,
        });
    }
    if config.version == 0 {
        config.version = SUPPORTED_VERSION;
    }
    Ok(LoadedConfig {
        source: ConfigSource::Path(path.to_path_buf()),
        config,
    })
}

/// Load a config from an in-memory YAML string. Used by tests and by the
/// `--config -` stdin path.
pub fn load_from_str(text: &str, source: ConfigSource) -> Result<LoadedConfig, ConfigError> {
    let mut config: AppConfig =
        serde_yaml_ng::from_str(text).map_err(|source| ConfigError::Parse {
            path: std::path::PathBuf::from("<inline>"),
            source,
        })?;
    if config.version > SUPPORTED_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: config.version,
            supported: SUPPORTED_VERSION,
        });
    }
    if config.version == 0 {
        config.version = SUPPORTED_VERSION;
    }
    Ok(LoadedConfig { source, config })
}

/// Return the built-in default config. Useful for `runnerguard config init`
/// to seed a file.
pub fn defaults() -> AppConfig {
    AppConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_minimal_yaml() {
        let yaml = "version: 1\nscan:\n  fail_on: warning\n";
        let loaded = load_from_str(yaml, ConfigSource::Stdin).unwrap();
        assert_eq!(
            loaded.config.scan.fail_on,
            runnerguard_model::Severity::Warning
        );
    }

    #[test]
    fn rejects_unsupported_version() {
        let yaml = "version: 99\n";
        let err = load_from_str(yaml, ConfigSource::Stdin).unwrap_err();
        matches!(err, ConfigError::UnsupportedVersion { found: 99, .. });
    }

    #[test]
    fn rejects_malformed_yaml() {
        let yaml = "scan: [this is not valid\n";
        let err = load_from_str(yaml, ConfigSource::Stdin).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }
}
