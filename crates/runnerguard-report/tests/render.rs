//! Integration tests for the report renderers.

use runnerguard_model::{
    Diagnostic, DiagnosticLevel, DiagnosticStage, Finding, FindingOrigin, ReportDocument,
    ReportMetadata, ReportSummary, ScanResultKind, Severity, SourceSpan,
};
use runnerguard_report::{render_html, render_markdown};

fn sample_doc() -> ReportDocument {
    let finding = Finding {
        rule_id: "RG-TEST-001".to_string(),
        severity: Severity::Warning,
        title: "Sample warning".to_string(),
        message: "This is <the> sample & message.".to_string(),
        recommendation: Some("Fix it.".to_string()),
        entity_id: Some("flow:demo".to_string()),
        source: Some(SourceSpan::point("src/main/mule/demo.xml", 12, 4)),
        evidence: serde_json::json!({"operator": "equals", "actual": "x", "expected": "y"}),
        origin: FindingOrigin::DeterministicRule,
        rule_version: Some("1.0.0".to_string()),
    };
    let diagnostic = Diagnostic::new(
        DiagnosticStage::Mule,
        "MULE-001",
        DiagnosticLevel::Error,
        "missing mule",
    );
    ReportDocument {
        metadata: ReportMetadata {
            tool: "runnerguard".to_string(),
            tool_version: "0.1.0".to_string(),
            schema_version: "1.0".to_string(),
            generated_at_unix: 1_700_000_000,
            project_id: "project:demo".to_string(),
            project_name: "demo project".to_string(),
        },
        summary: ReportSummary {
            findings_total: 1,
            critical: 0,
            error: 0,
            warning: 1,
            info: 0,
            flow_count: 1,
            subflow_count: 0,
            rule_count: 1,
            files_scanned: 3,
            result: ScanResultKind::Passed,
        },
        findings: vec![finding],
        parser_diagnostics: vec![diagnostic],
        rule_sets: vec![],
    }
}

#[test]
fn markdown_renders_all_sections() {
    let doc = sample_doc();
    let bytes = render_markdown(&doc).unwrap();
    let out = String::from_utf8(bytes).unwrap();
    assert!(out.contains("# RunnerGuard Scan Report"));
    assert!(out.contains("## Executive Summary"));
    assert!(out.contains("## Project Information"));
    assert!(out.contains("## Scan Configuration"));
    assert!(out.contains("## Rule Summary"));
    assert!(out.contains("## Findings"));
    assert!(out.contains("### Warning"));
    assert!(out.contains("## Flow and Subflow Inventory"));
    assert!(out.contains("## Unresolved References"));
    assert!(out.contains("## Parser Diagnostics"));
    assert!(out.contains("## Generated Artifacts"));
    assert!(out.contains("## Tool and Schema Versions"));
    assert!(out.contains("RG-TEST-001"));
    // HTML special chars must be escaped — GFM renderers honour inline
    // HTML, so emitting a raw `<` lets an attacker inject script tags
    // through any user-controlled field (rule title, message, file path).
    assert!(out.contains("This is &lt;the&gt; sample &amp; message"));
}

#[test]
fn html_renders_and_escapes_user_content() {
    let doc = sample_doc();
    let bytes = render_html(&doc).unwrap();
    let out = String::from_utf8(bytes).unwrap();
    assert!(out.contains("<!doctype html>"));
    assert!(out.contains("RunnerGuard Scan Report"));
    assert!(out.contains("&lt;the&gt;"));
    assert!(out.contains("&amp;"));
    // No raw <the> outside of HTML elements.
    let raw = out.find("<the>").unwrap_or(usize::MAX);
    assert!(
        raw > out.find("</header>").unwrap_or(0),
        "<the> must be escaped"
    );
    // Embedded CSS is present.
    assert!(out.contains(":root"));
    // Filter controls exist.
    assert!(out.contains("flt-text"));
    assert!(out.contains("flt-severity"));
}

#[test]
fn empty_findings_render_without_panicking() {
    let mut doc = sample_doc();
    doc.findings.clear();
    doc.summary.findings_total = 0;
    doc.summary.warning = 0;
    let md = String::from_utf8(render_markdown(&doc).unwrap()).unwrap();
    assert!(md.contains("_No findings._"));
    let html = String::from_utf8(render_html(&doc).unwrap()).unwrap();
    assert!(html.contains("All severities"));
}

#[test]
fn findings_are_sorted_severity_first() {
    let mut doc = sample_doc();
    doc.findings = vec![
        Finding {
            rule_id: "RG-A-INFO".to_string(),
            severity: Severity::Info,
            title: "info".into(),
            message: "msg".into(),
            recommendation: None,
            entity_id: None,
            source: None,
            evidence: serde_json::json!({}),
            origin: FindingOrigin::DeterministicRule,
            rule_version: None,
        },
        Finding {
            rule_id: "RG-A-CRIT".to_string(),
            severity: Severity::Critical,
            title: "crit".into(),
            message: "msg".into(),
            recommendation: None,
            entity_id: None,
            source: None,
            evidence: serde_json::json!({}),
            origin: FindingOrigin::DeterministicRule,
            rule_version: None,
        },
    ];
    doc.summary.findings_total = 2;
    doc.summary.critical = 1;
    doc.summary.info = 1;
    let md = String::from_utf8(render_markdown(&doc).unwrap()).unwrap();
    let crit_pos = md.find("RG-A-CRIT").unwrap();
    let info_pos = md.find("RG-A-INFO").unwrap();
    assert!(crit_pos < info_pos, "critical must appear before info");
}
