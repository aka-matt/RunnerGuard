//! Findings: the user-visible result of a scan.

use crate::source::SourceSpan;
use serde::{Deserialize, Serialize};

/// Severity ranking. Higher `rank()` = more severe.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
    Critical,
}

impl Severity {
    pub fn rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Error => 2,
            Self::Critical => 3,
        }
    }

    pub fn from_rank(rank: u8) -> Option<Self> {
        match rank {
            0 => Some(Self::Info),
            1 => Some(Self::Warning),
            2 => Some(Self::Error),
            3 => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

/// Where a finding came from. AI findings are never allowed to displace
/// deterministic ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingOrigin {
    DeterministicRule,
    AiSuggestion,
    ParserDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
    pub evidence: serde_json::Value,
    pub origin: FindingOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_version: Option<String>,
}

impl Finding {
    /// Construct a deterministic-rule finding. This is the path taken by the
    /// rule engine on every evaluation.
    pub fn deterministic(
        rule_id: impl Into<String>,
        severity: Severity,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            title: title.into(),
            message: message.into(),
            recommendation: None,
            entity_id: None,
            source: None,
            evidence: serde_json::Value::Null,
            origin: FindingOrigin::DeterministicRule,
            rule_version: None,
        }
    }

    /// Add a recommendation in builder style.
    #[must_use]
    pub fn with_recommendation(mut self, recommendation: impl Into<String>) -> Self {
        self.recommendation = Some(recommendation.into());
        self
    }

    /// Add a source location in builder style.
    #[must_use]
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    /// Add an entity ID in builder style.
    #[must_use]
    pub fn with_entity(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_id = Some(entity_id.into());
        self
    }

    /// Add structured evidence in builder style.
    #[must_use]
    pub fn with_evidence(mut self, evidence: serde_json::Value) -> Self {
        self.evidence = evidence;
        self
    }

    /// Add a rule version in builder style.
    #[must_use]
    pub fn with_rule_version(mut self, version: impl Into<String>) -> Self {
        self.rule_version = Some(version.into());
        self
    }
}
