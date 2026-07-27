//! Error types raised by the JSON codec and schema validator.

use std::path::PathBuf;
use thiserror::Error;

/// All errors produced by the JSON codec and validator.
#[derive(Debug, Error)]
pub enum JsonError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("UTF-8 decode error in {path}: {source}")]
    Utf8 {
        path: PathBuf,
        #[source]
        source: std::str::Utf8Error,
    },

    #[error("JSON syntax error in {path}: {source}")]
    Syntax {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("JSON Schema validation failed for {path}: {count} violation(s)")]
    Schema {
        path: PathBuf,
        count: usize,
        #[source]
        source: Box<JsonError>,
    },

    #[error("Deserialisation failed for {path}: {source}")]
    Deserialise {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// One violation emitted by [`SchemaValidator`]. The validator must surface
/// *all* violations, not fail-fast — that's how rule files can report every
/// broken rule in one pass.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaViolation {
    /// JSON Pointer (`/rules/3/op`) into the validated document, if
    /// available.
    pub pointer: Option<String>,
    pub message: String,
}

impl std::fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.pointer {
            Some(p) => write!(f, "{p}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}
