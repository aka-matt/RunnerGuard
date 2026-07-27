//! Error types for the report crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("template rendering failed: {0}")]
    Template(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialisation failed: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("format failed: {0}")]
    Fmt(#[from] std::fmt::Error),
}
