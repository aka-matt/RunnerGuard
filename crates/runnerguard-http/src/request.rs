//! Request and response types for the `RunnerGuard` HTTP client.
//!
//! These types deliberately do **not** expose [`reqwest::Request`]. Higher
//! layers talk in terms of "an HTTP request `RunnerGuard` understands",
//! and the trait implementation hides the underlying transport.

use crate::error::HttpError;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::time::Duration;

/// HTTP method used by the request. The `RunnerGuard` client only supports
/// the verbs the spec calls out (`GET`, `POST`, plus the few others the
/// implementation may need to round out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<RequestBody>,
    /// Per-request timeout. Falls back to [`HttpClientConfig::timeout`]
    /// when `None`.
    pub timeout: Option<Duration>,
    /// Optional request id for trace logs. Auto-generated when `None`.
    pub request_id: Option<String>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            headers: BTreeMap::new(),
            body: None,
            timeout: None,
            request_id: None,
        }
    }

    pub fn post(url: impl Into<String>, body: RequestBody) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: BTreeMap::new(),
            body: Some(body),
            timeout: None,
            request_id: None,
        }
    }

    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[must_use]
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
}

#[derive(Debug, Clone)]
pub enum RequestBody {
    Json(JsonValue),
    Bytes(Vec<u8>),
    Text(String),
}

impl RequestBody {
    /// Serialise the body to bytes. JSON values are serialised
    /// pretty-printed so the network log is human readable; callers that
    /// care about exact byte format should use [`RequestBody::Bytes`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, HttpError> {
        match self {
            Self::Json(v) => {
                serde_json::to_vec_pretty(v).map_err(|e| HttpError::Encode(format!("json: {e}")))
            }
            Self::Bytes(b) => Ok(b.clone()),
            Self::Text(s) => Ok(s.as_bytes().to_vec()),
        }
    }
}

impl Serialize for RequestBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // The body is never used as a *value* in a config file; we just
        // need to derive Serialize for the few types that wrap it. The
        // canonical form is bytes.
        match self {
            Self::Json(v) => v.serialize(serializer),
            Self::Bytes(_) | Self::Text(_) => serializer.serialize_str("<binary>"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
    pub request_id: String,
    /// Number of attempts (1 = first try succeeded, 2 = one retry, …).
    pub attempts: u32,
}

impl HttpResponse {
    pub fn text(&self) -> Result<&str, HttpError> {
        std::str::from_utf8(&self.body).map_err(|e| HttpError::Decode(format!("utf-8: {e}")))
    }

    pub fn json(&self) -> Result<JsonValue, HttpError> {
        serde_json::from_slice(&self.body).map_err(|e| HttpError::Decode(format!("json: {e}")))
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The codes that the client auto-retries. See the implementation
    /// doc §6.5 for the policy.
    pub fn is_retryable_status(&self) -> bool {
        matches!(self.status, 408 | 429 | 500 | 502 | 503 | 504)
    }
}
