//! AI provider abstraction, prompt templates, and response parsing
//! for `RunnerGuard`.
//!
//! Public surface:
//!
//! * [`AiProvider`] / [`OpenAiCompatibleProvider`] — talk to any
//!   OpenAI-compatible chat completions endpoint.
//! * [`prompt::load_bundled`] — load `prompts/review.md` and
//!   `prompts/format-repair.md` from disk.
//! * [`service::AiService`] — orchestrate the full
//!   prompt → provider → repair → findings flow.
//! * [`response::parse_response`] — JSON extract, schema validate, one
//!   repair pass.
//!
//! ## Safety properties
//!
//! * API keys are `SecretRef`s. They are resolved into HTTP headers and
//!   never written into prompts, logs, or `anyhow` chains.
//! * Untrusted user data is wrapped in `<<<UNTRUSTED>>>` markers and
//!   sanitised before substitution so the markers can't be closed early.
//! * AI failures never invalidate the deterministic report — the
//!   caller receives a diagnostic, not a hard error.
//!
//! See `implementation_docs/RunnerGuard__实施文档.md` §6.8 for the
//! full contract.

pub mod error;
pub mod prompt;
pub mod provider;
pub mod response;
pub mod secret;
pub mod service;

pub use error::AiError;
pub use provider::{AiCallResult, AiProvider, AiRequest, OpenAiCompatibleProvider};
pub use response::{
    ParsedAiPayload, detect_injection, extract_json_object, parse_response,
    parse_response_with_meta, validate_against_schema,
};
pub use service::{AiService, AiServiceResult};
