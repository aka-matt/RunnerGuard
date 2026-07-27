//! High-level orchestration: load templates, build the request, parse
//! the response, convert into findings.

use crate::error::AiError;
use crate::prompt::{RenderedPrompt, review_template};
use crate::provider::{AiProvider, AiRequest};
use crate::response::{detect_injection, parse_response};
use runnerguard_model::{AiAnalysis, AiMetadata, AiRawResponse, Finding, ParsedProject};
use serde_json::json;
use std::sync::Arc;

/// The result of an AI-augmented scan. The caller decides whether to
/// surface the diagnostic and/or merge the suggestions into the
/// deterministic findings list.
#[derive(Debug, Clone)]
pub struct AiServiceResult {
    pub analysis: Option<AiAnalysis>,
    pub findings: Vec<Finding>,
    pub metadata: AiMetadata,
    pub diagnostics: Vec<String>,
}

pub struct AiService {
    pub provider: Arc<dyn AiProvider>,
    pub schema: String,
}

impl std::fmt::Debug for AiService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiService")
            .field("provider", &"<dyn AiProvider>")
            .field("schema_bytes", &self.schema.len())
            .finish()
    }
}

impl AiService {
    pub fn new(provider: Arc<dyn AiProvider>, schema: impl Into<String>) -> Self {
        Self {
            provider,
            schema: schema.into(),
        }
    }

    /// Render both prompts, send to the provider, parse + repair, and
    /// turn suggestions into findings.
    pub async fn review(
        &self,
        project: &ParsedProject,
        deterministic_findings: &[Finding],
    ) -> AiServiceResult {
        let mut diagnostics = Vec::new();
        let rendered = match build_prompts(project, deterministic_findings) {
            Ok(r) => r,
            Err(e) => {
                diagnostics.push(format!("prompt render failed: {e}"));
                return AiServiceResult {
                    analysis: None,
                    findings: Vec::new(),
                    metadata: AiMetadata::default(),
                    diagnostics,
                };
            }
        };

        let request = AiRequest::from_rendered(&rendered.system, &rendered.user, &self.schema);
        let raw = match self.provider.analyze(request.clone()).await {
            Ok(r) => r,
            Err(e) => {
                diagnostics.push(format!("provider call failed: {e}"));
                return AiServiceResult {
                    analysis: None,
                    findings: Vec::new(),
                    metadata: AiMetadata::default(),
                    diagnostics,
                };
            }
        };

        if let Some(injection) = detect_injection(&raw.raw_text) {
            diagnostics.push(format!(
                "possible prompt injection in AI response: {injection}"
            ));
        }

        match parse_response(
            self.provider.as_ref(),
            &raw.raw_text,
            &self.schema,
            &request,
        )
        .await
        {
            Ok((analysis, metadata)) => {
                let mut merged = metadata;
                merged.model = merged.model.or(raw.metadata.model);
                merged.latency_ms = merged.latency_ms.or(raw.metadata.latency_ms);
                merged.request_id = merged.request_id.or(raw.metadata.request_id);
                merged.prompt_tokens = merged.prompt_tokens.or(raw.metadata.prompt_tokens);
                merged.completion_tokens =
                    merged.completion_tokens.or(raw.metadata.completion_tokens);
                merged.total_tokens = merged.total_tokens.or(raw.metadata.total_tokens);
                let findings = runnerguard_model::suggestions_to_findings(analysis.clone());
                AiServiceResult {
                    analysis: Some(analysis),
                    findings,
                    metadata: merged,
                    diagnostics,
                }
            }
            Err(e) => {
                diagnostics.push(format!("response parse failed: {e}"));
                AiServiceResult {
                    analysis: None,
                    findings: Vec::new(),
                    metadata: raw.metadata,
                    diagnostics,
                }
            }
        }
    }
}

struct RenderedPair {
    system: RenderedPrompt,
    user: RenderedPrompt,
}

/// Render the system + user prompt for a project review.
fn build_prompts(
    project: &ParsedProject,
    deterministic: &[Finding],
) -> Result<RenderedPair, AiError> {
    let snapshot = build_snapshot(project);
    let det_text = deterministic
        .iter()
        .map(|f| format!("- [{}] {}: {}", f.severity.as_str(), f.rule_id, f.message))
        .collect::<Vec<_>>()
        .join("\n");

    let vars = json!({
        "project": {
            "id": project.project.id,
            "name": project.project.name,
            "mule_version": project.project.mule_version,
            "flow_count": project.index.flows.len(),
            "subflow_count": project.index.sub_flows.len(),
        },
        "deterministic_findings": det_text,
        "project_snapshot": snapshot,
    });

    // The bundled template has a single body; we split it at the
    // "## System prompt" / "## User prompt template" markers so the
    // caller sees two [`RenderedPrompt`] values.
    let template = review_template().ok_or_else(|| {
        crate::error::AiError::PromptMissing(
            "review".to_string(),
            "load_bundled was not called".to_string(),
        )
    })?;
    let rendered = template.render(&vars)?;
    let (sys, usr) = split_rendered(&rendered);
    Ok(RenderedPair {
        system: RenderedPrompt {
            name: format!("{}-system", rendered.name()),
            body: sys.to_string(),
        },
        user: RenderedPrompt {
            name: format!("{}-user", rendered.name()),
            body: usr.to_string(),
        },
    })
}

fn split_rendered(p: &RenderedPrompt) -> (String, String) {
    let body = p.body();
    let sys_marker = "## System prompt";
    let usr_marker = "## User prompt template";
    let sys_start = body.find(sys_marker).unwrap_or(0);
    let usr_start = body.find(usr_marker).unwrap_or(body.len());
    if sys_start < usr_start {
        (
            body[sys_start..usr_start].trim().to_string(),
            body[usr_start..].trim().to_string(),
        )
    } else {
        (body.to_string(), String::new())
    }
}

/// Build the redacted snapshot we'll embed inside `<<<UNTRUSTED>>>`.
fn build_snapshot(project: &ParsedProject) -> String {
    let mut out = serde_json::Map::new();
    out.insert("id".to_string(), json!(project.project.id));
    out.insert("name".to_string(), json!(project.project.name));
    out.insert(
        "mule_version".to_string(),
        json!(project.project.mule_version),
    );
    out.insert("flow_count".to_string(), json!(project.index.flows.len()));
    out.insert(
        "subflow_count".to_string(),
        json!(project.index.sub_flows.len()),
    );
    let mut flows = Vec::new();
    for entry in &project.index.flows {
        let doc = project.documents.iter().find(|d| d.id == entry.document_id);
        let mut comps = Vec::new();
        if let Some(d) = doc {
            if let Some(flow) = d.flows.iter().find(|f| f.id == entry.flow_id) {
                for c in &flow.components {
                    collect_components(c, &mut comps);
                }
            }
        }
        flows.push(json!({
            "id": entry.flow_id,
            "name": entry.name,
            "file": entry.source.file,
            "components": comps,
        }));
    }
    out.insert("flows".to_string(), json!(flows));
    serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".to_string())
}

fn collect_components(comp: &runnerguard_model::MuleComponent, out: &mut Vec<serde_json::Value>) {
    let mut entry = serde_json::Map::new();
    entry.insert("namespace".to_string(), json!(comp.namespace_uri));
    entry.insert("local_name".to_string(), json!(comp.local_name));
    entry.insert("qualified_name".to_string(), json!(comp.qualified_name));
    if let Some(value) = comp.attributes.get("config-ref") {
        entry.insert(
            "config_ref".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = comp.attributes.get("name") {
        entry.insert(
            "component_name".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    out.push(serde_json::Value::Object(entry));
    for c in &comp.children {
        collect_components(c, out);
    }
}

// Re-export the wrapped raw response for tests/examples.
pub fn raw_response(text: String, analysis: AiAnalysis, metadata: AiMetadata) -> AiRawResponse {
    AiRawResponse {
        raw_text: text,
        parsed: analysis,
        metadata,
    }
}
