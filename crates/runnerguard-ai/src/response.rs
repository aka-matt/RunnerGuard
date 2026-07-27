//! Response parsing, JSON extraction, schema validation, and the
//! single repair pass.

use crate::error::AiError;
use crate::prompt::repair_template;
use crate::provider::{AiProvider, AiRequest};
use regex::Regex;
use runnerguard_json::{DefaultSchemaValidator, SchemaValidator};
use runnerguard_model::{AiAnalysis, AiMetadata};
use serde_json::Value;
use std::sync::OnceLock;

/// Wrapper for an AI response that has been parsed into a JSON value
/// but not yet validated against the AI response schema.
#[derive(Debug, Clone)]
pub struct ParsedAiPayload {
    pub raw: String,
    pub json: Value,
}

/// Try to extract a single JSON object from `text`.
///
/// Strategy (in order):
/// 1. The whole text is a JSON object.
/// 2. A `json` code fence.
/// 3. The first balanced `{...}` block.
///
/// Markdown backticks / fences are stripped before scanning. The
/// extraction never panics; on failure we return [`AiError::JsonNotFound`].
pub fn extract_json_object(text: &str) -> Result<Value, AiError> {
    let trimmed = text.trim();

    // 1. The whole payload is JSON.
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        if v.is_object() {
            return Ok(v);
        }
    }

    // 2. A ```json ... ``` fenced block.
    if let Some(inner) = first_code_fence(trimmed, "json") {
        if let Ok(v) = serde_json::from_str::<Value>(&inner) {
            if v.is_object() {
                return Ok(v);
            }
        }
    }

    // 3. The first balanced object.
    if let Some(obj_text) = first_balanced_object(trimmed) {
        if let Ok(v) = serde_json::from_str::<Value>(&obj_text) {
            if v.is_object() {
                return Ok(v);
            }
        }
    }

    Err(AiError::JsonNotFound)
}

fn first_code_fence(text: &str, lang: &str) -> Option<String> {
    let pat = format!("```{lang}");
    let start = text.find(&pat)?;
    let after = &text[start + pat.len()..];
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

fn first_balanced_object(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(text[s..=i].to_string());
                }
            }
        }
    }
    None
}

/// Validate `payload.json` against the bundled `ai-response.schema.json`.
/// The schema is the contract — providers that deviate are not accepted.
pub fn validate_against_schema(payload: &Value, schema: &str) -> Result<AiAnalysis, AiError> {
    let schema_value: Value = serde_json::from_str(schema)
        .map_err(|e| AiError::Internal(format!("schema parse: {e}")))?;
    let validator = DefaultSchemaValidator;
    let errors = validator.validate(&schema_value, payload);
    if let Err(violations) = errors {
        let formatted: Vec<String> = violations
            .into_iter()
            .map(|v| {
                format!(
                    "{} (path {})",
                    v.message,
                    v.pointer.unwrap_or_else(|| "<root>".to_string())
                )
            })
            .collect();
        return Err(AiError::SchemaValidation(formatted.join("; ")));
    }
    serde_json::from_value::<AiAnalysis>(payload.clone())
        .map_err(|e| AiError::SchemaValidation(format!("shape mismatch: {e}")))
}

/// Parse + validate an AI response. On schema failure, optionally sends
/// **one** repair request through the provider.
pub async fn parse_response(
    provider: &dyn AiProvider,
    raw: &str,
    schema: &str,
    request: &AiRequest,
) -> Result<(AiAnalysis, AiMetadata), AiError> {
    parse_response_with_meta(provider, raw, schema, request, AiMetadata::default()).await
}

/// Like [`parse_response`] but takes pre-collected metadata (model /
/// latency / token hints) from the caller. When the response validates
/// on the first try, this metadata is returned as-is; on a repair pass
/// the repair metadata takes over.
pub async fn parse_response_with_meta(
    provider: &dyn AiProvider,
    raw: &str,
    schema: &str,
    request: &AiRequest,
    initial_meta: AiMetadata,
) -> Result<(AiAnalysis, AiMetadata), AiError> {
    let payload = match extract_json_object(raw) {
        Ok(v) => ParsedAiPayload {
            raw: raw.to_string(),
            json: v,
        },
        Err(_) => return Err(AiError::JsonNotFound),
    };
    match validate_against_schema(&payload.json, schema) {
        Ok(analysis) => Ok((analysis, initial_meta)),
        Err(_) => {
            // One repair pass — if the bundled repair template wasn't
            // loaded, skip the repair and surface the original error.
            let repair = match build_repair_request(request, &payload.raw, schema) {
                Ok(r) => r,
                Err(_) => {
                    return Err(AiError::SchemaValidation(
                        "schema validation failed and repair template not loaded".to_string(),
                    ));
                }
            };
            let call = provider.analyze(repair).await?;
            let v = extract_json_object(&call.raw_text)?;
            let parsed = validate_against_schema(&v, schema)
                .map_err(|e| AiError::RepairFailed(e.to_string()))?;
            let mut meta = call.metadata;
            meta.extra
                .insert("repaired".to_string(), serde_json::Value::Bool(true));
            Ok((parsed, meta))
        }
    }
}

fn build_repair_request(
    original: &AiRequest,
    previous: &str,
    schema: &str,
) -> Result<AiRequest, AiError> {
    let template = repair_template().ok_or_else(|| {
        AiError::PromptMissing(
            "format-repair".to_string(),
            "load_bundled was not called".to_string(),
        )
    })?;
    let rendered = template.render(&serde_json::json!({
        "response_schema": schema,
        "previous_response": previous,
    }))?;
    Ok(AiRequest {
        system_prompt: rendered.body().to_string(),
        user_prompt: String::new(),
        response_schema: original.response_schema.clone(),
        model: original.model.clone(),
    })
}

/// Detect likely prompt injection: the response includes the markers
/// outside an untrusted block, or attempts to escalate ("ignore
/// previous", "system:", etc.).
pub fn detect_injection(text: &str) -> Option<String> {
    let markers = [
        "<<<UNTRUSTED>>>",
        "<<<END_UNTRUSTED>>>",
        "ignore previous instructions",
        "ignore all previous",
        "system:",
    ];
    for m in markers {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(&format!("(?i){m}")).unwrap());
        if re.is_match(text) {
            // The marker itself inside a fenced code block is fine if
            // we found a JSON object — but if it's appearing in the
            // raw response outside JSON, that's suspicious.
            if m.starts_with("<<<") {
                return Some(format!("leaked untrusted marker: {m}"));
            }
            return Some(format!("possible instruction override: {m}"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pure_json() {
        let v = extract_json_object("{\"a\":1}").unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn extract_from_markdown_fence() {
        let text = "Some prose\n```json\n{\"a\":1,\"b\":[2,3]}\n```\nmore";
        let v = extract_json_object(text).unwrap();
        assert_eq!(v["b"][1], 3);
    }

    #[test]
    fn extract_from_prose() {
        let text = "The answer is here: {\"a\":42} enjoy!";
        let v = extract_json_object(text).unwrap();
        assert_eq!(v["a"], 42);
    }

    #[test]
    fn extract_fails_on_garbage() {
        assert!(extract_json_object("not json at all").is_err());
    }

    #[test]
    fn detect_marker_leak() {
        let s = "ok\n<<<UNTRUSTED>>>\n";
        let detected = detect_injection(s);
        assert!(detected.is_some());
    }
}
