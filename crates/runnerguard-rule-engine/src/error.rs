//! Error types emitted by the rule engine.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum EngineError {
    #[error("rule compilation failed for `{rule_id}`: {message}")]
    Compile { rule_id: String, message: String },
    #[error("rule evaluation failed for `{rule_id}`: {message}")]
    Evaluate { rule_id: String, message: String },
}

/// Non-fatal issues surfaced during compilation. The engine returns these
/// in `EvaluationResult::warnings` so callers can show them in reports.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileIssue {
    pub rule_id: String,
    pub message: String,
}
