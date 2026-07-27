//! TUI-local error type.

use thiserror::Error;

/// Errors the TUI may surface. Anything marked `Fatal` is propagated
/// back to the caller of [`run`](crate::app::App::run); `Recoverable`
/// errors are added to diagnostics and the TUI keeps running.
#[derive(Debug, Error)]
pub enum TuiError {
    #[error("terminal setup failed: {0}")]
    Terminal(String),
    #[error("event polling failed: {0}")]
    EventSource(String),
    #[error("component state invalid: {0}")]
    Component(String),
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),
}

impl TuiError {
    /// True if the failure means we cannot keep the TUI alive.
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Terminal(_) | Self::Io(_) | Self::EventSource(_))
    }
}
