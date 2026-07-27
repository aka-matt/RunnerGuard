//! Source location types used by every diagnostic and finding.

use serde::{Deserialize, Serialize};

/// A half-open source location: `file:start_line:start_column` to
/// `end_line:end_column`. Line and column numbers are 1-based to match what
/// editors and CI tooling display.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Repository-relative file path (forward-slash separated for stability).
    pub file: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl SourceSpan {
    /// Build a span that points at a single point (start == end) on
    /// `line:column`. Useful for "the secret-looking thing starts here" style
    /// findings.
    pub fn point(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            start_line: line,
            start_column: column,
            end_line: line,
            end_column: column,
        }
    }

    /// Path/line/column string for logs and reports.
    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.file, self.start_line, self.start_column)
    }
}
