//! Real `reqwest`-backed implementation of [`HttpClient`].

use crate::client::{HttpClient, HttpClientConfig};
use crate::error::HttpError;
use crate::request::{HttpMethod, HttpRequest, HttpResponse};
use async_trait::async_trait;
use std::time::Duration;
use tracing::{debug, warn};
use url::Url;

/// The `reqwest` client. Holds the underlying client, the policy config,
/// and an allowlist resolver.
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    inner: reqwest::Client,
    config: HttpClientConfig,
}

impl ReqwestHttpClient {
    pub fn new(config: HttpClientConfig) -> Result<Self, HttpError> {
        Self::build(config)
    }

    fn build(config: HttpClientConfig) -> Result<Self, HttpError> {
        let mut builder = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .gzip(true);

        if !config.allow_hosts.is_empty() {
            // reqwest has no native allowlist; we layer our own check in
            // [`execute`]. Surface the intent in the builder too via a
            // warning, so users who pass allow_hosts are reminded that
            // it's a RunnerGuard feature, not a reqwest one.
            debug!(
                allowlist = ?config.allow_hosts,
                "RunnerGuard HTTP allowlist is active"
            );
        }

        if let Some(extra_ca) = &config.extra_ca_file {
            let bytes = std::fs::read(extra_ca)
                .map_err(|e| HttpError::Tls(format!("read extra CA bundle: {e}")))?;
            let cert = reqwest::Certificate::from_pem(&bytes)
                .map_err(|e| HttpError::Tls(format!("parse extra CA bundle: {e}")))?;
            builder = builder.add_root_certificate(cert);
        }

        if let Some(proxy) = &config.proxy_url {
            let url = Url::parse(proxy).map_err(|e| HttpError::InvalidUrl(e.to_string()))?;
            builder = builder.proxy(
                reqwest::Proxy::all(url)
                    .map_err(|e| HttpError::Internal(format!("invalid proxy URL: {e}")))?,
            );
        }

        let inner = builder
            .build()
            .map_err(|e| HttpError::Tls(format!("reqwest builder: {e}")))?;
        Ok(Self { inner, config })
    }
}

#[async_trait]
#[allow(clippy::too_many_lines)]
impl HttpClient for ReqwestHttpClient {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        if self.config.offline {
            return Err(HttpError::Offline(request.url));
        }

        // Allowlist enforcement happens BEFORE any network I/O.
        let parsed = Url::parse(&request.url).map_err(|e| HttpError::InvalidUrl(e.to_string()))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| HttpError::InvalidUrl("missing host".to_string()))?
            .to_string();
        if !self.config.allow_hosts.is_empty()
            && !self
                .config
                .allow_hosts
                .iter()
                .any(|h| h == &host || host.ends_with(&format!(".{h}")))
        {
            return Err(HttpError::HostNotAllowed { host });
        }

        let request_id = request.request_id.clone().unwrap_or_else(|| {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            format!("rg-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
        });

        let method = match request.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Delete => reqwest::Method::DELETE,
        };

        let mut attempts: u32 = 0;
        let max_attempts = self.config.retry_count.max(1);
        loop {
            attempts += 1;
            let mut req = self.inner.request(method.clone(), &request.url);
            for (k, v) in &request.headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req = req.header("X-Request-Id", request_id.as_str());
            if let Some(body) = &request.body {
                let bytes = body.to_bytes()?;
                req = req.body(bytes);
            }
            if let Some(timeout) = request.timeout {
                req = req.timeout(timeout);
            }

            debug!(target: "runnerguard_http", attempt = attempts, url = %parsed, "HTTP request");
            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
                    warn!(target: "runnerguard_http", attempt = attempts, error = %e, "transient HTTP error");
                    if attempts >= max_attempts {
                        return Err(HttpError::RetriesExhausted { attempts });
                    }
                    sleep_with_backoff(attempts).await;
                    continue;
                }
                Err(e) if e.is_decode() => {
                    return Err(HttpError::Decode(e.to_string()));
                }
                Err(e) => {
                    // TLS errors surface as `is_connect()` false but the
                    // underlying error chain mentions TLS; we still
                    // don't retry them.
                    return Err(HttpError::Connection(e.to_string()));
                }
            };

            let status = resp.status().as_u16();
            // Capture Retry-After before we consume the response with
            // `.bytes()` — the response object is single-shot.
            let retry_after_secs: Option<u64> = if status == 429 {
                resp.headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            };
            let headers = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => return Err(HttpError::Connection(e.to_string())),
            };
            if body_bytes.len() as u64 > self.config.max_response_bytes {
                return Err(HttpError::BodyTooLarge {
                    limit: self.config.max_response_bytes,
                });
            }

            if (408..600).contains(&status) && should_retry_status(status, attempts, max_attempts) {
                if let Some(secs) = retry_after_secs {
                    // Honour Retry-After (cap at 60 s).
                    tokio::time::sleep(Duration::from_secs(secs.min(60))).await;
                }
                warn!(target: "runnerguard_http", attempt = attempts, status, "retryable HTTP status");
                if attempts >= max_attempts {
                    return Ok(HttpResponse {
                        status,
                        headers,
                        body: body_bytes.to_vec(),
                        request_id: request_id.clone(),
                        attempts,
                    });
                }
                sleep_with_backoff(attempts).await;
                continue;
            }

            return Ok(HttpResponse {
                status,
                headers,
                body: body_bytes.to_vec(),
                request_id: request_id.clone(),
                attempts,
            });
        }
    }
}

fn should_retry_status(status: u16, attempts: u32, max: u32) -> bool {
    attempts < max && matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

async fn sleep_with_backoff(attempt: u32) {
    // Exponential backoff. attempt=1 → 250 ms, attempt=2 → 500 ms, attempt=3
    // → 1 s, etc. Capped at 8 s.
    let base_ms = 250u64 * (1 << (attempt - 1).min(5));
    let ms = base_ms.min(8_000);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
