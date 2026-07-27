//! Aggregation and sorting logic that turns a [`ScanResult`] into a
//! stable [`ReportDocument`]. Determinism matters: two scans over the
//! same inputs must produce identical bytes.

use runnerguard_model::{
    Diagnostic, Finding, ParsedProject, ReportDocument, ReportMetadata, ReportSummary, RuleSet,
    ScanResult, ScanResultKind, Severity,
};
use std::collections::BTreeMap;

/// Build a report document from the inputs the engine produces. The
/// document is the only thing renderers see — they do not touch the
/// raw engine types.
pub fn build_document(
    result: &ScanResult,
    project: &ParsedProject,
    rule_sets: &[RuleSet],
    generated_at_unix: i64,
    tool_version: &str,
) -> ReportDocument {
    let findings = sort_findings(&result.findings);
    let summary = build_summary(result, project, findings.first().map(|f| f.severity));
    ReportDocument {
        metadata: ReportMetadata {
            tool: "runnerguard".to_string(),
            tool_version: tool_version.to_string(),
            schema_version: runnerguard_model::CURRENT_SCHEMA_VERSION.to_string(),
            generated_at_unix,
            project_id: project.project.id.clone(),
            project_name: project.project.name.clone(),
        },
        summary,
        findings,
        parser_diagnostics: result.parser_diagnostics.clone(),
        rule_sets: rule_sets.to_vec(),
    }
}

fn build_summary(
    result: &ScanResult,
    project: &ParsedProject,
    top: Option<Severity>,
) -> ReportSummary {
    let mut summary = ReportSummary::default();
    for f in &result.findings {
        match f.severity {
            Severity::Critical => summary.critical += 1,
            Severity::Error => summary.error += 1,
            Severity::Warning => summary.warning += 1,
            Severity::Info => summary.info += 1,
        }
    }
    summary.findings_total = result.findings.len();
    summary.flow_count = result.flow_count;
    summary.subflow_count = result.subflow_count;
    summary.rule_count = result.rule_count;
    summary.files_scanned = project_file_count(project);
    summary.result = classify(top);
    summary
}

fn classify(top: Option<Severity>) -> ScanResultKind {
    match top {
        Some(Severity::Critical) | Some(Severity::Error) => ScanResultKind::Failed,
        Some(Severity::Warning) => ScanResultKind::Passed,
        _ => ScanResultKind::Passed,
    }
}

fn project_file_count(project: &ParsedProject) -> usize {
    project.project.files.len()
}

/// Stable sort by severity desc → rule id → file → line → entity id.
pub fn sort_findings(findings: &[Finding]) -> Vec<Finding> {
    let mut sorted = findings.to_vec();
    sorted.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| source_file(a).cmp(&source_file(b)))
            .then_with(|| source_line(a).cmp(&source_line(b)))
            .then_with(|| entity_id(a).cmp(&entity_id(b)))
    });
    sorted
}

fn source_file(f: &Finding) -> String {
    f.source
        .as_ref()
        .map(|s| s.file.clone())
        .unwrap_or_default()
}
fn source_line(f: &Finding) -> u32 {
    f.source.as_ref().map(|s| s.start_line).unwrap_or(0)
}
fn entity_id(f: &Finding) -> String {
    f.entity_id.clone().unwrap_or_default()
}

/// Group findings by rule id so reports can show "this rule fired N
/// times" roll-ups.
pub fn rollup_by_rule(findings: &[Finding]) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for f in findings {
        *out.entry(f.rule_id.clone()).or_insert(0) += 1;
    }
    out
}

/// Group findings by severity in the canonical Critical / Error /
/// Warning / Info order.
pub fn group_by_severity(findings: Vec<Finding>) -> BTreeMap<Severity, Vec<Finding>> {
    let mut out: BTreeMap<Severity, Vec<Finding>> = BTreeMap::new();
    for f in findings {
        out.entry(f.severity).or_default().push(f);
    }
    out
}

#[allow(dead_code)]
fn diagnostic_codes(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.code.clone()).collect()
}
