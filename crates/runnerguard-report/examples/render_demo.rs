//! Renders both Markdown and HTML report files from a small in-memory
//! document. Useful for eyeballing the renderer during development.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-report --example render_demo -- ./tmp/render
//! ```

use runnerguard_model::{
    Finding, FindingOrigin, ReportDocument, ReportMetadata, ReportSummary, Severity,
};
use runnerguard_report::{render_html, render_markdown};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./tmp/render"));
    std::fs::create_dir_all(&out_dir)?;

    let doc = ReportDocument {
        metadata: ReportMetadata {
            tool: "runnerguard".to_string(),
            tool_version: "0.1.0".to_string(),
            schema_version: "1.0".to_string(),
            generated_at_unix: 1_700_000_000,
            project_id: "project:demo".to_string(),
            project_name: "demo".to_string(),
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
            files_scanned: 1,
            result: runnerguard_model::ScanResultKind::Passed,
        },
        findings: vec![Finding {
            rule_id: "RG-DEMO-001".to_string(),
            severity: Severity::Warning,
            title: "Demo finding".to_string(),
            message: "Use https instead of http.".to_string(),
            recommendation: Some("Switch the listener to HTTPS.".to_string()),
            entity_id: Some("flow:demo".to_string()),
            source: Some(runnerguard_model::SourceSpan::point(
                "src/main/mule/demo.xml",
                4,
                12,
            )),
            evidence: serde_json::json!({"operator": "equals", "actual": "http", "expected": "https"}),
            origin: FindingOrigin::DeterministicRule,
            rule_version: Some("1.0.0".to_string()),
        }],
        parser_diagnostics: vec![],
        rule_sets: vec![],
    };

    let md_path = out_dir.join("report.md");
    let html_path = out_dir.join("report.html");
    std::fs::write(&md_path, render_markdown(&doc)?)?;
    std::fs::write(&html_path, render_html(&doc)?)?;
    println!("Wrote {} and {}", md_path.display(), html_path.display());
    Ok(())
}
