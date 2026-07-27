//! AI provider trait and the OpenAI-compatible implementation.

use crate::error::AiError;
use crate::prompt::RenderedPrompt;
use crate::response::ParsedAiPayload;
use async_trait::async_trait;
use runnerguard_http::{HttpClient, HttpRequest, HttpResponse, RequestBody};
use runnerguard_model::AiMetadata;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A request the caller hands to a provider.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system_prompt: String,
    pub user_prompt: String,
    pub response_schema: String,
    /// Optional model override; otherwise the provider default is used.
    pub model: Option<String>,
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn analyze(&self, request: AiRequest) -> Result<AiCallResult, AiError>;
}

/// What the trait returns. The crate-level [`AiRawResponse`] is built
/// by [`parse_response`]; this struct carries the raw text + transport
/// metadata so callers can decide what to do with a malformed payload.
#[derive(Debug, Clone)]
pub struct AiCallResult {
    pub raw_text: String,
    pub metadata: AiMetadata,
}

/// An OpenAI-compatible provider. Tested against the standard
/// `/v1/chat/completions` schema; any provider that mirrors that shape
/// works (Azure, vLLM, Ollama, …).
pub struct OpenAiCompatibleProvider {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<runnerguard_model::SecretRef>,
    pub http: Arc<dyn HttpClient>,
    pub timeout: Duration,
}

impl std::fmt::Debug for OpenAiCompatibleProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatibleProvider")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &self.api_key)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl OpenAiCompatibleProvider {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<runnerguard_model::SecretRef>,
        http: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            model: model.into(),
            api_key,
            http,
            timeout: Duration::from_secs(60),
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    async fn analyze(&self, request: AiRequest) -> Result<AiCallResult, AiError> {
        let api_key = match &self.api_key {
            Some(secret) => Some(crate::secret::resolve(secret)?),
            None => None,
        };

        let model = request.model.clone().unwrap_or_else(|| self.model.clone());
        let payload = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": request.system_prompt},
                {"role": "user", "content": request.user_prompt},
            ],
            "response_format": {"type": "json_object"},
            "temperature": 0.1,
            "max_tokens": 2048,
        });

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut req =
            HttpRequest::post(&url, RequestBody::Json(payload)).with_timeout(self.timeout);
        if let Some(key) = &api_key {
            req.headers
                .insert("Authorization".to_string(), format!("Bearer {key}"));
        }
        let started = Instant::now();
        let response: HttpResponse = self.http.execute(req).await?;
        let latency_ms = started.elapsed().as_millis() as u64;

        let raw_text = String::from_utf8(response.body.clone())
            .map_err(|e| AiError::Internal(format!("response was not valid UTF-8: {e}")))?;

        let (model, prompt_tokens, completion_tokens, total_tokens) =
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&response.body) {
                let model = value
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string());
                let usage = value.get("usage");
                (
                    model,
                    usage
                        .and_then(|u| u.get("prompt_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("completion_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("total_tokens"))
                        .and_then(|v| v.as_u64()),
                )
            } else {
                (None, None, None, None)
            };
        let metadata = AiMetadata {
            request_id: Some(response.request_id.clone()),
            model,
            latency_ms: Some(latency_ms),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            extra: Default::default(),
        };

        Ok(AiCallResult { raw_text, metadata })
    }
}

/// Helpers shared between the live provider and tests.
impl AiRequest {
    pub fn new(
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
        response_schema: impl Into<String>,
    ) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            user_prompt: user_prompt.into(),
            response_schema: response_schema.into(),
            model: None,
        }
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Build an [`AiRequest`] from already-rendered [`RenderedPrompt`]
    /// parts. The caller passes the system-prompt and user-prompt
    /// fragments separately.
    pub fn from_rendered(system: &RenderedPrompt, user: &RenderedPrompt, schema: &str) -> Self {
        Self::new(
            system.body().to_string(),
            user.body().to_string(),
            schema.to_string(),
        )
    }

    /// Extract the model's text content from a parsed OpenAI payload.
    /// Returns [`AiError::JsonNotFound`] when the response is malformed.
    pub fn extract_text(parsed: &ParsedAiPayload) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(&parsed.raw).ok()?;
        // OpenAI shape: choices[0].message.content
        if let Some(s) = v
            .pointer("/choices/0/message/content")
            .and_then(|x| x.as_str())
        {
            return Some(s.to_string());
        }
        // Ollama / other shape: response
        if let Some(s) = v.pointer("/response").and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
        None
    }
}
