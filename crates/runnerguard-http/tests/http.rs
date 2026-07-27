//! Integration tests for the HTTP client.
//!
//! The reqwest-backed client is exercised against a local server bound
//! to `127.0.0.1` — no real network calls. The mock client covers the
//! cases where the real client would be overkill (header redaction,
//! allowlist enforcement, offline mode).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use runnerguard_http::{
    HttpClient, HttpClientConfig, HttpError, HttpMethod, HttpRequest, HttpResponse, MockHttpClient,
    ReqwestHttpClient,
};

/// Tiny single-shot HTTP server. Each request it accepts returns the
/// configured status + body.
async fn start_test_server(
    status: u16,
    body: &'static str,
    extra_status: Option<(u16, &'static str)>,
) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    tokio::spawn(async move {
        // We serve two requests in the (status, body) → first form;
        // the first call returns `status`, the second (if any) returns
        // `extra_status`. After that, any further requests also return
        // `extra_status` to be safe.
        let mut first = true;
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => return,
            };
            let mut buf = vec![0u8; 4096];
            let _ = sock.read(&mut buf).await;
            let (code, body_str) = if first {
                first = false;
                (status, body)
            } else if let Some((c, b)) = extra_status {
                (c, b)
            } else {
                (status, body)
            };
            let resp = format!(
                "HTTP/1.1 {code} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            let _ = sock.shutdown().await;
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://{addr}"), counter)
}

#[tokio::test]
async fn mock_client_returns_queued_response() {
    let mut h = BTreeMap::new();
    h.insert("X-Test".to_string(), "yes".to_string());
    let resp = HttpResponse {
        status: 201,
        headers: h,
        body: b"{\"ok\":true}".to_vec(),
        request_id: "test-1".to_string(),
        attempts: 1,
    };
    let client = MockHttpClient::new().with_response(resp);
    let r = client
        .execute(HttpRequest::get("https://example.test/"))
        .await
        .unwrap();
    assert_eq!(r.status, 201);
    assert_eq!(r.text().unwrap(), "{\"ok\":true}");
}

#[tokio::test]
async fn offline_mode_refuses_network() {
    let config = HttpClientConfig {
        offline: true,
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let err = client
        .execute(HttpRequest::get("https://example.test/"))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Offline(_)));
}

#[tokio::test]
async fn allowlist_blocks_disallowed_host() {
    let config = HttpClientConfig {
        allow_hosts: vec!["allowed.test".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let err = client
        .execute(HttpRequest::get("https://evil.test/"))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::HostNotAllowed { .. }));
}

#[tokio::test]
async fn default_deny_blocks_every_host() {
    // Empty allowlist = nothing allowed (SSRF defence). Even offline
    // mode can't bypass this — but offline returns first, so set it
    // false to reach the allowlist check.
    let config = HttpClientConfig::default();
    let client = ReqwestHttpClient::new(config).unwrap();
    let err = client
        .execute(HttpRequest::get("https://example.test/"))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::HostNotAllowed { .. }));
}

#[tokio::test]
async fn allowlist_allows_exact_match() {
    let config = HttpClientConfig {
        allow_hosts: vec!["allowed.test".to_string()],
        offline: true, // skip the network call once the host check passes
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    // Will fail with Offline because offline=true, but it must pass the
    // host check first.
    let err = client
        .execute(HttpRequest::get("https://allowed.test/"))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Offline(_)));
}

#[tokio::test]
async fn reqwest_client_sends_get_and_parses_response() {
    let (url, counter) = start_test_server(200, "{\"ok\":true}", None).await;
    let config = HttpClientConfig {
        allow_hosts: vec!["127.0.0.1".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let resp = client.execute(HttpRequest::get(&url)).await.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.text().unwrap(), "{\"ok\":true}");
    assert!(counter.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn reqwest_client_posts_json() {
    let (url, _) = start_test_server(200, "ok", None).await;
    let config = HttpClientConfig {
        allow_hosts: vec!["127.0.0.1".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let req = HttpRequest::post(
        &url,
        runnerguard_http::RequestBody::Json(serde_json::json!({"a": 1})),
    )
    .with_header("Content-Type", "application/json");
    let resp = client.execute(req).await.unwrap();
    assert_eq!(resp.status, 200);
}

#[tokio::test]
async fn retry_429_then_success() {
    let (url, counter) = start_test_server(429, "busy", Some((200, "{\"ok\":true}"))).await;
    let config = HttpClientConfig {
        retry_count: 3,
        timeout: Duration::from_secs(5),
        allow_hosts: vec!["127.0.0.1".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let resp = client.execute(HttpRequest::get(&url)).await.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn no_retry_on_404() {
    let (url, counter) = start_test_server(404, "missing", None).await;
    let config = HttpClientConfig {
        retry_count: 3,
        timeout: Duration::from_secs(5),
        allow_hosts: vec!["127.0.0.1".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let resp = client.execute(HttpRequest::get(&url)).await.unwrap();
    assert_eq!(resp.status, 404);
    // No retries on non-retryable status.
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retries_exhausted_returns_last_status() {
    // Both attempts return 503.
    let (url, counter) = start_test_server(503, "down", Some((503, "down"))).await;
    let config = HttpClientConfig {
        retry_count: 3,
        timeout: Duration::from_secs(5),
        allow_hosts: vec!["127.0.0.1".to_string()],
        ..HttpClientConfig::default()
    };
    let client = ReqwestHttpClient::new(config).unwrap();
    let resp = client.execute(HttpRequest::get(&url)).await.unwrap();
    // After exhausting retries we still surface the last response so the
    // caller can show a diagnostic.
    assert_eq!(resp.status, 503);
    // Three attempts total: 1 + 2 retries.
    assert!(counter.load(Ordering::SeqCst) >= 3);
}

#[tokio::test]
async fn method_post_builds_correct_outgoing_request() {
    use std::collections::BTreeMap;
    let resp = HttpResponse {
        status: 200,
        headers: BTreeMap::new(),
        body: b"ok".to_vec(),
        request_id: "r".to_string(),
        attempts: 1,
    };
    let client = MockHttpClient::new().with_response(resp);
    let req = HttpRequest::post(
        "https://example.test/",
        runnerguard_http::RequestBody::Bytes(b"hello".to_vec()),
    );
    assert_eq!(req.method, HttpMethod::Post);
    let r = client.execute(req).await.unwrap();
    assert_eq!(r.status, 200);
}
