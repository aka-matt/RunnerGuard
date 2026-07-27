//! AI-layer types shared between `runnerguard-ai` and downstream crates.
//!
//! The types in this module are the *contract* the AI crate exposes.
//! They are deliberately small and serialisable so the response payload
//! can be validated against `schemas/ai-response.schema.json` without
//! reaching into private types.

use crate::finding::Finding;
use crate::source::SourceSpan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kind of entity a suggestion targets. Matches the JSON Schema
/// `suggestion.target.entity` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiTargetEntity {
    Project,
    File,
    Flow,
    Subflow,
    Component,
    PropertyReference,
    FlowReference,
    DataweaveBlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiTarget {
    pub entity: AiTargetEntity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub id: String,
    pub title: String,
    pub severity: crate::finding::Severity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
    pub target: AiTarget,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub evidence: BTreeMap<String, serde_json::Value>,
}

/// What the AI crate hands back to `runnerguard-core` after parsing
/// and validating a provider response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiAnalysis {
    pub schema_version: String,
    pub summary: String,
    pub suggestions: Vec<AiSuggestion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Convert an [`AiAnalysis`] into a list of [`Finding`]s with
/// `origin = AiSuggestion`. The mapping preserves severity and
/// attaches the suggestion's `target` as `entity_id`.
pub fn suggestions_to_findings(analysis: AiAnalysis) -> Vec<Finding> {
    let model = analysis.model.clone();
    analysis
        .suggestions
        .into_iter()
        .map(|s| {
            let entity_id = match (&s.target.entity, &s.target.name) {
                (AiTargetEntity::Flow, Some(name)) => Some(format!("flow:{name}")),
                (AiTargetEntity::Subflow, Some(name)) => Some(format!("subflow:{name}")),
                (AiTargetEntity::Component, Some(name)) => Some(format!("component:{name}")),
                (entity, name) => name.clone().map(|n| format!("{entity:?}:{n}")),
            };
            let source = s.target.file.as_ref().map(|file| SourceSpan {
                file: file.clone(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
            });
            Finding {
                rule_id: s.id,
                severity: s.severity,
                title: s.title,
                message: s.message,
                recommendation: s.recommendation,
                entity_id,
                source,
                evidence: serde_json::to_value(&s.evidence).unwrap_or(serde_json::Value::Null),
                origin: crate::finding::FindingOrigin::AiSuggestion,
                rule_version: model.clone(),
            }
        })
        .collect()
}

/// Token / cost / latency metadata captured from the provider response.
/// Kept as a free-form BTreeMap so we don't lock in a vendor schema —
/// the caller decides which keys they care about.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AiMetadata {
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub latency_ms: Option<u64>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// What `AiProvider::analyze` returns. The raw text and metadata are
/// kept so callers can record them in diagnostics / logs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRawResponse {
    pub raw_text: String,
    pub parsed: AiAnalysis,
    pub metadata: AiMetadata,
}
