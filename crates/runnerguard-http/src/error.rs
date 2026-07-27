//! HTTP-layer error type.
//!
//! Everything that can fail a request collapses into one variant. Higher
//! layers (the AI crate, the network rule loader) translate this into a
//! cross-cutting [`runnerguard_model::Diagnostic`] — they do not
//! re-encode the error.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("offline mode is enabled; refusing to make a network request to {0}")]
    Offline(String),
    #[error("target host `{host}` is not in the configured allowlist")]
    HostNotAllowed { host: String },
    #[error("URL is invalid: {0}")]
    InvalidUrl(String),
    #[error("response body exceeded the {limit}-byte limit")]
    BodyTooLarge { limit: u64 },
    #[error("HTTP status {status}: {message}")]
    Status { status: u16, message: String },
    #[error("connection error: {0}")]
    Connection(String),
    #[error("request body could not be serialised: {0}")]
    Encode(String),
    #[error("response body could not be decoded: {0}")]
    Decode(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("retries exhausted after {attempts} attempts")]
    RetriesExhausted { attempts: u32 },
    #[error("internal error: {0}")]
    Internal(String),
}

impl HttpError {
    /// True for errors the caller might be able to recover from by
    /// changing the request or the environment (e.g. the user can fix
    /// their API key). False for "the request is malformed" type errors.
    pub fn is_user_actionable(&self) -> bool {
        matches!(
            self,
            Self::HostNotAllowed { .. }
                | Self::InvalidUrl(_)
                | Self::Status { .. }
                | Self::Offline(_)
        )
    }
}
