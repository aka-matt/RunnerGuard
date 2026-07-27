//! Generic, AI-agnostic HTTP client for `RunnerGuard`.
//!
//! The trait lives here; the AI crate consumes it. The reqwest-backed
//! implementation lives in [`reqwest_client`] and a deterministic
//! mock lives in [`client::MockHttpClient`].

pub mod client;
pub mod error;
pub mod request;
pub mod reqwest_client;
pub mod secrets;

pub use client::{HttpClient, HttpClientConfig, MockHttpClient};
pub use error::HttpError;
pub use request::{HttpMethod, HttpRequest, HttpResponse, RequestBody};
pub use reqwest_client::ReqwestHttpClient;
pub use secrets::{format_request, redact_headers};
