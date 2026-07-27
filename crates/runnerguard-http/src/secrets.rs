//! Secret-aware logging helpers.
//!
//! The HTTP client is a security boundary: it sees `Authorization` and
//! `X-API-Key` headers, and it sees proxy auth. None of those bytes are
//! allowed in log output, panic messages, or `anyhow` chains. The
//! helpers in this module are the only sanctioned way to log a request
//! or response.

use crate::request::HttpRequest;

const REDACTED_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "cookie",
    "set-cookie",
];

/// Redact secret-bearing headers. Returns a new `BTreeMap` so the
/// caller's map is never mutated in place (the original must keep its
/// secret values).
pub fn redact_headers(
    headers: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for (k, v) in headers {
        if REDACTED_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
            out.insert(k.clone(), "***REDACTED***".to_string());
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// Render a request for tracing. The URL is kept (so the operator can
/// confirm the right endpoint was called) but query string parameters
/// named `api_key` / `token` / `secret` are replaced with `***REDACTED***`.
pub fn format_request(request: &HttpRequest) -> String {
    let safe_url = redact_url(&request.url);
    let headers = redact_headers(&request.headers);
    format!(
        "{} {} (request_id={}) headers={:?}",
        request.method.as_str(),
        safe_url,
        request.request_id.as_deref().unwrap_or("-"),
        headers
    )
}

fn redact_url(url: &str) -> String {
    // Quick & conservative redaction: any query parameter whose name
    // suggests a secret gets replaced. This is not a full URL parser —
    // it is a logging helper.
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            let mut replaced = false;
            for (k, _v) in parsed.query_pairs() {
                let lowered = k.to_ascii_lowercase();
                if lowered.contains("key")
                    || lowered.contains("token")
                    || lowered.contains("secret")
                {
                    replaced = true;
                    break;
                }
            }
            if replaced {
                // Rebuild the URL with all query pairs redacted.
                let pairs: Vec<(String, String)> = parsed
                    .query_pairs()
                    .map(|(k, _)| {
                        let lowered = k.to_ascii_lowercase();
                        if lowered.contains("key")
                            || lowered.contains("token")
                            || lowered.contains("secret")
                        {
                            (k.to_string(), "***REDACTED***".to_string())
                        } else {
                            (k.to_string(), "***".to_string())
                        }
                    })
                    .collect();
                parsed.query_pairs_mut().clear();
                for (k, v) in pairs {
                    parsed.query_pairs_mut().append_pair(&k, &v);
                }
                parsed.to_string()
            } else {
                url.to_string()
            }
        }
        Err(_) => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn redacts_authorization_header() {
        let mut h = BTreeMap::new();
        h.insert("Authorization".to_string(), "Bearer xyz".to_string());
        h.insert("Content-Type".to_string(), "application/json".to_string());
        let r = redact_headers(&h);
        assert_eq!(r.get("Authorization").unwrap(), "***REDACTED***");
        assert_eq!(r.get("Content-Type").unwrap(), "application/json");
    }

    #[test]
    fn redacts_every_known_secret_header_case_insensitive() {
        let cases = [
            "authorization",
            "Authorization",
            "AUTHORIZATION",
            "x-api-key",
            "X-API-Key",
            "proxy-authorization",
            "cookie",
            "set-cookie",
        ];
        for c in cases {
            let mut h = BTreeMap::new();
            h.insert(c.to_string(), "value".to_string());
            assert_eq!(
                redact_headers(&h).get(c).unwrap(),
                "***REDACTED***",
                "header `{c}` was not redacted"
            );
        }
    }

    #[test]
    fn format_request_keeps_url_and_redacts_secret_query() {
        let req = HttpRequest::get("https://example.test/v1?api_key=abc&name=ok");
        let s = format_request(&req);
        assert!(s.contains("example.test"));
        assert!(!s.contains("api_key=abc"));
    }
}
