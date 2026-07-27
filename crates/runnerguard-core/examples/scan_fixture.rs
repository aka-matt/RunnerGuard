//! Runs a scan against a project on disk and prints the summary.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-core --example scan_fixture -- \
//!   ./fixtures/mule-project-basic ./rules/basic.json ./tmp/scan
//! ```

use runnerguard_core::{CollectingSink, ScanEvent, ScanService};
use runnerguard_model::{AiMode, ReportFormat, ScanRequest, Severity};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let project = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./fixtures/mule-project-basic"));
    let rules = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./rules/basic.json"));
    let out = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./tmp/scan"));

    let req = ScanRequest {
        project_dir: project,
        rule_files: vec![rules],
        output_dir: out.clone(),
        formats: vec![ReportFormat::Markdown, ReportFormat::Html],
        include_tests: false,
        write_flow_json: true,
        ai_mode: AiMode::Disabled,
        fail_on: Severity::Warning,
        rule_filter: Default::default(),
    };

    let mut sink = CollectingSink::default();
    let outcome = ScanService::new().run_with_sink(&req, &mut sink);

    let event_count = sink.events.len();
    let started = sink
        .events
        .iter()
        .filter(|e| matches!(e, ScanEvent::Started { .. }))
        .count();
    let finished = sink
        .events
        .iter()
        .filter(|e| matches!(e, ScanEvent::Finished { .. }))
        .count();
    println!(
        "scan emitted {} events (started={}, finished={})",
        event_count, started, finished
    );
    println!(
        "found {} finding(s), wrote {} report(s) and {} artifact(s)",
        outcome.result.findings.len(),
        outcome.report_paths.len(),
        outcome.artifact_paths.len()
    );
    Ok(())
}
