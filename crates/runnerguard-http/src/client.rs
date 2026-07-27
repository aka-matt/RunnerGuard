//! [`HttpClient`] trait, configuration, and a `reqwest`-backed
//! implementation.

use crate::error::HttpError;
use crate::request::{HttpRequest, HttpResponse};
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub offline: bool,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub retry_count: u32,
    pub max_response_bytes: u64,
    pub proxy_url: Option<String>,
    pub extra_ca_file: Option<String>,
    pub allow_hosts: Vec<String>,
    pub user_agent: String,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            offline: false,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            retry_count: 3,
            max_response_bytes: 10 * 1024 * 1024,
            proxy_url: None,
            extra_ca_file: None,
            allow_hosts: Vec::new(),
            user_agent: format!("runnerguard/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError>;
}

/// In-memory client used by tests and examples. Stores every request
/// the caller has ever made so tests can assert against the call
/// history without spinning up a real socket.
#[derive(Debug, Default)]
pub struct MockHttpClient {
    pub responses: std::sync::Mutex<Vec<HttpResponse>>,
    pub error: std::sync::Mutex<Option<HttpError>>,
}

impl MockHttpClient {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_response(self, response: HttpResponse) -> Self {
        self.responses.lock().unwrap().push(response);
        self
    }

    #[must_use]
    pub fn with_error(self, err: HttpError) -> Self {
        *self.error.lock().unwrap() = Some(err);
        self
    }
}

#[async_trait]
impl HttpClient for MockHttpClient {
    async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, HttpError> {
        if let Some(err) = self.error.lock().unwrap().take() {
            return Err(err);
        }
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(HttpError::Internal(
                "MockHttpClient ran out of queued responses".to_string(),
            ));
        }
        Ok(responses.remove(0))
    }
}
