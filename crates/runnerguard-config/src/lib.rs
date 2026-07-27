//! YAML configuration loader with secret redaction for RunnerGuard.
//!
//! Owns the [`AppConfig`] struct, loading from a YAML file (or stdin via a
//! string), validating fields, and resolving [`SecretRef`] values against
//! the process environment.
//!
//! `config show` and any tracing output must go through [`RedactedYaml`] —
//! the raw struct never prints a secret in plaintext.

#![deny(unsafe_code)]

pub mod app_config;
pub mod default;
pub mod loader;
pub mod paths;
pub mod render;
pub mod validate;

pub use app_config::{
    AiConfig, AppConfig, LoggingConfig, NetworkConfig, ReportConfig, ScanConfig, VersionConfig,
};
pub use default::DEFAULT_CONFIG_YAML;
pub use loader::{ConfigError, ConfigSource, LoadedConfig, defaults};
pub use paths::{default_config_path, ensure_parent_dir};
pub use render::render_redacted_yaml;
pub use validate::{ValidationIssue, validate};
