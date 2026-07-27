//! Scan request/result/report types — the contracts that bind the CLI, TUI,
//! and core orchestration layer together.

use crate::diagnostic::Diagnostic;
use crate::finding::{Finding, Severity};
use crate::rule::{Rule, RuleSet};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct ScanRequest {
    pub project_dir: PathBuf,
    pub rule_files: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub formats: Vec<ReportFormat>,
    pub include_tests: bool,
    pub write_flow_json: bool,
    pub ai_mode: AiMode,
    pub fail_on: Severity,
    pub rule_filter: RuleFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportFormat {
    Markdown,
    Html,
}

impl ReportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiMode {
    Disabled,
    Optional,
    Required,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleFilter {
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub rules: Vec<String>,
    pub disabled_rules: Vec<String>,
    pub severity_overrides: Vec<(String, Severity)>,
}

impl RuleFilter {
    /// Returns true when `rule` survives the active filter.
    pub fn accepts(&self, rule: &Rule) -> bool {
        if !rule.enabled || self.disabled_rules.iter().any(|id| id == &rule.id) {
            return false;
        }
        if !self.rules.is_empty() && !self.rules.iter().any(|id| id == &rule.id) {
            return false;
        }
        if !self.tags.is_empty() && !rule.tags.iter().any(|t| self.tags.contains(t)) {
            return false;
        }
        if self.exclude_tags.iter().any(|t| rule.tags.contains(t)) {
            return false;
        }
        true
    }

    /// Apply any severity overrides to `rule`, returning a (possibly
    /// downgraded/upgraded) clone.
    pub fn apply_overrides(&self, rule: &Rule) -> Rule {
        let mut cloned = rule.clone();
        for (id, sev) in &self.severity_overrides {
            if id == &rule.id {
                cloned.severity = *sev;
            }
        }
        cloned
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanResult {
    pub findings: Vec<Finding>,
    pub parser_diagnostics: Vec<Diagnostic>,
    pub flow_count: usize,
    pub subflow_count: usize,
    pub rule_count: usize,
}

impl ScanResult {
    pub fn summary(&self) -> ScanSummary {
        let mut summary = ScanSummary::default();
        for finding in &self.findings {
            match finding.severity {
                Severity::Critical => summary.critical += 1,
                Severity::Error => summary.error += 1,
                Severity::Warning => summary.warning += 1,
                Severity::Info => summary.info += 1,
            }
        }
        summary.findings_total = self.findings.len();
        summary.flow_count = self.flow_count;
        summary.subflow_count = self.subflow_count;
        summary.rule_count = self.rule_count;
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScanSummary {
    pub findings_total: usize,
    pub critical: usize,
    pub error: usize,
    pub warning: usize,
    pub info: usize,
    pub flow_count: usize,
    pub subflow_count: usize,
    pub rule_count: usize,
}

impl ScanSummary {
    pub fn threshold_exceeded(&self, fail_on: Severity) -> bool {
        let rank = fail_on.rank();
        if rank <= Severity::Info.rank() {
            self.info > 0
        } else if rank == Severity::Warning.rank() {
            self.warning > 0
        } else if rank == Severity::Error.rank() {
            self.error > 0
        } else {
            self.critical > 0
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanOutcome {
    pub result: ScanResult,
    pub report_paths: Vec<PathBuf>,
    pub artifact_paths: Vec<PathBuf>,
    pub threshold_exceeded: bool,
    pub incomplete: bool,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReportDocument {
    pub metadata: ReportMetadata,
    pub summary: ReportSummary,
    pub findings: Vec<Finding>,
    pub parser_diagnostics: Vec<Diagnostic>,
    pub rule_sets: Vec<RuleSet>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReportMetadata {
    pub tool: String,
    pub tool_version: String,
    pub schema_version: String,
    pub generated_at_unix: i64,
    pub project_id: String,
    pub project_name: String,
}

impl Default for ReportMetadata {
    fn default() -> Self {
        Self {
            tool: "runnerguard".to_string(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: crate::CURRENT_SCHEMA_VERSION.to_string(),
            generated_at_unix: 0,
            project_id: String::new(),
            project_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReportSummary {
    pub findings_total: usize,
    pub critical: usize,
    pub error: usize,
    pub warning: usize,
    pub info: usize,
    pub flow_count: usize,
    pub subflow_count: usize,
    pub rule_count: usize,
    pub files_scanned: usize,
    pub result: ScanResultKind,
}

impl Default for ReportSummary {
    fn default() -> Self {
        Self {
            findings_total: 0,
            critical: 0,
            error: 0,
            warning: 0,
            info: 0,
            flow_count: 0,
            subflow_count: 0,
            rule_count: 0,
            files_scanned: 0,
            result: ScanResultKind::Passed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanResultKind {
    Passed,
    Failed,
    Incomplete,
}
