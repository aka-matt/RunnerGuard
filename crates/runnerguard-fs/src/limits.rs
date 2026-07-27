//! Safety limits applied during discovery. Defaults follow the design doc.

use serde::{Deserialize, Serialize};

/// Resource limits enforced by the file-system and parser layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// Maximum size of a single file we'll read into memory.
    pub max_file_bytes: u64,
    /// Maximum total number of files we'll keep under management.
    pub max_project_files: usize,
    /// Maximum nesting depth of an XML document (parser).
    pub max_xml_depth: usize,
    /// Maximum size of an HTTP response body (network layer).
    pub max_http_response_bytes: u64,
    /// Whether to follow symbolic links when walking the project tree.
    pub follow_symlinks: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024,
            max_project_files: 50_000,
            max_xml_depth: 256,
            max_http_response_bytes: 10 * 1024 * 1024,
            follow_symlinks: false,
        }
    }
}
