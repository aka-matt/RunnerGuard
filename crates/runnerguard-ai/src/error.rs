//! AI-layer error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("provider `{0}` is not configured")]
    ProviderNotConfigured(String),
    #[error("prompt template `{0}` could not be loaded: {1}")]
    PromptMissing(String, String),
    #[error("prompt template `{0}` failed to render: {1}")]
    PromptRender(String, String),
    #[error("response did not contain a parseable JSON object")]
    JsonNotFound,
    #[error("response JSON failed schema validation: {0}")]
    SchemaValidation(String),
    #[error("HTTP transport error: {0}")]
    Http(#[from] runnerguard_http::HttpError),
    #[error("AI repair pass failed: {0}")]
    RepairFailed(String),
    #[error("prompt injection attempt detected: {0}")]
    PromptInjection(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AiError {
    /// AI failure must never invalidate the deterministic report. The
    /// caller translates an [`AiError`] into a diagnostic and continues.
    pub fn is_user_actionable(&self) -> bool {
        matches!(
            self,
            Self::ProviderNotConfigured(_) | Self::PromptMissing(..)
        )
    }
}
