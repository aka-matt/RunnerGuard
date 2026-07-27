//! Demonstrates the HTTP client without touching the network.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p runnerguard-http --example mock_get
//! ```
//!
//! The example queues a fake response into the mock client and prints it
//! back. Useful for sandboxed environments.

use runnerguard_http::{HttpClient, HttpRequest, HttpResponse, MockHttpClient};
use std::collections::BTreeMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut headers = BTreeMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let body = serde_json::json!({
        "id": "demo",
        "ok": true,
    });
    let bytes = serde_json::to_vec(&body)?;

    let response = HttpResponse {
        status: 200,
        headers,
        body: bytes,
        request_id: "demo-1".to_string(),
        attempts: 1,
    };

    let client = MockHttpClient::new().with_response(response);
    let req = HttpRequest::get("https://example.test/v1/health");

    let resp = client.execute(req).await?;
    println!(
        "status={} attempts={} request_id={}",
        resp.status, resp.attempts, resp.request_id
    );
    println!("body = {}", std::str::from_utf8(&resp.body)?);

    // Show that the redaction helper does not leak headers.
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert("Authorization".to_string(), "Bearer SECRET".to_string());
    let req2 =
        HttpRequest::get("https://example.test/v1/x").with_header("Authorization", "Bearer SECRET");
    let log_line = runnerguard_http::format_request(&req2);
    assert!(!log_line.contains("SECRET"));
    assert!(log_line.contains("***REDACTED***"));
    println!("log line: {log_line}");
    let _ = secret_headers; // keep BTreeMap import happy

    Ok(())
}
