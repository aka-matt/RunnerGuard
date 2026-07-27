//! Shared `serde` data models for `RunnerGuard`.
//!
//! This crate has **zero I/O** — no HTTP, filesystem, or terminal code. It
//! only carries `Debug + Clone + Serialize + Deserialize + PartialEq` types
//! shared between the CLI, TUI, core orchestration layer, and every other
//! crate. All other crates depend on it; it depends on nothing else in the
//! workspace.

#![deny(unsafe_code)]

pub mod ai;
pub mod diagnostic;
pub mod finding;
pub mod project;
pub mod rule;
pub mod scan;
pub mod secret;
pub mod source;

pub use ai::{
    AiAnalysis, AiMetadata, AiRawResponse, AiSuggestion, AiTarget, AiTargetEntity,
    suggestions_to_findings,
};
pub use diagnostic::{Diagnostic, DiagnosticLevel, DiagnosticStage};
pub use finding::{Finding, FindingOrigin, Severity};
pub use project::{
    ArtifactIndex, DataWeaveBlock, FlowFacts, FlowIndexEntry, FlowRef, GlobalConfigIndex,
    MuleComponent, MuleDocument, MuleFlow, MuleFlowKind, ParsedProject, ProjectDescriptor,
    ProjectIndex, SourceFile, SourceFileKind, UnresolvedFlowRef,
};
pub use rule::{
    Condition, ConditionCombinator, FactPath, Operator, Rule, RuleDefaults, RuleSet, RuleTarget,
    RuleTargetEntity, SingleCondition,
};
pub use scan::{
    AiMode, ReportDocument, ReportFormat, ReportMetadata, ReportSummary, RuleFilter, ScanOutcome,
    ScanRequest, ScanResult, ScanResultKind, ScanSummary,
};
pub use secret::SecretRef;
pub use source::SourceSpan;

/// Marker trait for models that carry a stable `schema_version` field.
///
/// Implementing this signals that a breaking change should bump the
/// `schema_version` rather than mutate the field layout silently.
pub trait SchemaVersioned {
    fn schema_version() -> &'static str;
}

/// Sentinel schema version used by every stable model in phase 1.
pub const CURRENT_SCHEMA_VERSION: &str = "1.0";
