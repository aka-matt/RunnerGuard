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

/// Header / query / path names that suggest a secret. Matched as a
/// whole word — substrings like "monkey" or "betoken" do NOT match.
const SECRET_NAMES: &[&str] = &[
    "key",
    "api_key",
    "apikey",
    "token",
    "access_token",
    "refresh_token",
    "tokens",
    "secret",
    "client_secret",
    "password",
    "passwd",
    "pwd",
    "auth",
    "signature",
    "sig",
    "private",
];

fn is_secret_name(name: &str) -> bool {
    // Whole-word match against SECRET_NAMES. Avoids false positives
    // like `monkey`, `betoken`, `keywords`.
    SECRET_NAMES.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// Redact secret-bearing headers. Returns a new `BTreeMap` so the
/// caller's map is never mutated in place (the original must keep its
/// secret values).
pub fn redact_headers(
    headers: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for (k, v) in headers {
        let lowered = k.to_ascii_lowercase();
        if REDACTED_HEADERS.contains(&lowered.as_str())
            || (lowered.starts_with("x-") && is_secret_name(&lowered[2..]))
        {
            out.insert(k.clone(), "***REDACTED***".to_string());
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// Render a request for tracing. The URL is kept (so the operator can
/// confirm the right endpoint was called) but query string parameters
/// AND path segments whose names suggest a secret are replaced with
/// `***REDACTED***`. The previous implementation only redacted query
/// params and used a substring match that produced false positives
/// (e.g. `monkey`, `betoken`).
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
    // Conservative redaction: any query parameter whose name matches
    // a secret-name token gets its value replaced. Other parameters
    // are preserved verbatim so logs remain useful for debugging.
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            // Redact query parameters that look like secrets.
            let mut secret_keys: Vec<String> = Vec::new();
            for (k, _v) in parsed.query_pairs() {
                if is_secret_name(&k) {
                    secret_keys.push(k.into_owned());
                }
            }
            if !secret_keys.is_empty() {
                let pairs: Vec<(String, String)> = parsed
                    .query_pairs()
                    .map(|(k, v)| {
                        if secret_keys.iter().any(|s| s == k.as_ref()) {
                            (k.into_owned(), "***REDACTED***".to_string())
                        } else {
                            (k.into_owned(), v.into_owned())
                        }
                    })
                    .collect();
                parsed.query_pairs_mut().clear();
                for (k, v) in pairs {
                    parsed.query_pairs_mut().append_pair(&k, &v);
                }
            }
            // Redact path segments that look like secrets. Anything
            // after a path segment that matches a secret-name token
            // gets replaced (e.g. `/users/<id>/tokens/<value>`).
            redact_path_segments(&mut parsed);
            parsed.to_string()
        }
        Err(_) => url.to_string(),
    }
}

fn redact_path_segments(parsed: &mut url::Url) {
    let path = parsed.path().to_string();
    if path.is_empty() || path == "/" {
        return;
    }
    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() {
        return;
    }
    let mut redacted = Vec::with_capacity(segments.len());
    for (i, seg) in segments.iter().enumerate() {
        // A segment is a secret if it OR the immediately previous segment
        // matches a secret name (covers `/tokens/<value>` style).
        let prev_is_secret = i
            .checked_sub(1)
            .and_then(|j| segments.get(j).map(|s| is_secret_name(s)))
            .unwrap_or(false);
        if prev_is_secret || is_secret_name(seg) {
            redacted.push("***REDACTED***".to_string());
        } else {
            redacted.push(seg.to_string());
        }
    }
    let new_path = format!("/{}", redacted.join("/"));
    parsed.set_path(&new_path);
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

    #[test]
    fn does_not_redact_unrelated_substring_params() {
        // Previous substring match redacted `monkey` and `betoken`;
        // the whole-word match keeps these readable for debugging.
        let req = HttpRequest::get("https://example.test/v1?monkey=bobo&betoken=ok&name=alice");
        let s = format_request(&req);
        assert!(s.contains("monkey=bobo"));
        assert!(s.contains("betoken=ok"));
        assert!(s.contains("name=alice"));
    }

    #[test]
    fn redacts_token_followed_by_value_in_path() {
        // /tokens/<value> in the path should mask the value, even
        // though no query params are present.
        let req = HttpRequest::get("https://example.test/v1/users/u1/tokens/abc123def");
        let s = format_request(&req);
        assert!(!s.contains("abc123def"), "path token leaked: {s}");
        assert!(s.contains("***REDACTED***"));
    }

    #[test]
    fn redacts_secret_password_query() {
        let req = HttpRequest::get("https://example.test/?password=hunter2&page=1");
        let s = format_request(&req);
        assert!(!s.contains("hunter2"));
        assert!(s.contains("page=1"));
    }

    #[test]
    fn redacts_x_secret_headers() {
        let mut h = BTreeMap::new();
        h.insert("X-Token".to_string(), "t0p-s3cret".to_string());
        h.insert("X-Monkey".to_string(), "abc".to_string());
        let r = redact_headers(&h);
        assert_eq!(r.get("X-Token").unwrap(), "***REDACTED***");
        assert_eq!(r.get("X-Monkey").unwrap(), "abc");
    }
}
