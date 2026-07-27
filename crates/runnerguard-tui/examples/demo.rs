//! `cargo run -p runnerguard-tui --example demo`
//!
//! Builds a synthetic `ScanOutcome` and shows the TUI browsing it on
//! the real terminal. Use this to eyeball the layout / keybindings
//! without needing a real Mule project at hand.

use runnerguard_model::{
    Diagnostic, DiagnosticLevel, DiagnosticStage, Finding, ScanOutcome, ScanResult, Severity,
    SourceSpan,
};
use runnerguard_tui::{App, EventSource, ScanSource, run_cli};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let outcome = synthetic_outcome();
    run_cli(ScanSource::Preloaded(outcome))?;
    Ok(())
}

/// Build a synthetic `ScanOutcome` so the demo can render without
/// scanning a real Mule project.
#[must_use]
pub fn synthetic_outcome() -> ScanOutcome {
    let findings = vec![
        Finding::deterministic(
            "MULE-001",
            Severity::Critical,
            "Logger missing",
            "Flow is missing a Logger component — production traffic won't be logged.",
        )
        .with_recommendation("Add a <logger/> component early in the flow.")
        .with_entity("flow:order-api")
        .with_source(SourceSpan {
            file: "src/main/mule/order-api.xml".to_string(),
            start_line: 12,
            start_column: 5,
            end_line: 12,
            end_column: 24,
        }),
        Finding::deterministic(
            "MULE-002",
            Severity::Warning,
            "Http Listener uses default port",
            "Default port 8081 is reached by automated scanners; pick a port outside the public range.",
        )
        .with_entity("flow:order-api"),
        Finding::deterministic(
            "MULE-014",
            Severity::Info,
            "No DataWeave error handler",
            "Consider adding a target=\"errors\" mapper to capture failures.",
        )
        .with_entity("flow:order-api"),
    ];
    let diagnostics = vec![Diagnostic::new(
        DiagnosticStage::Rule,
        "MULE-WARN",
        DiagnosticLevel::Warning,
        "demo diagnostic: component config-ref 'orders-db' was not resolved.",
    )];
    let result = ScanResult {
        findings,
        parser_diagnostics: diagnostics,
        rule_count: 3,
        flow_count: 1,
        subflow_count: 0,
    };
    ScanOutcome {
        result,
        report_paths: vec![
            PathBuf::from("./reports/report.md"),
            PathBuf::from("./reports/report.html"),
        ],
        artifact_paths: vec![PathBuf::from(
            "./reports/artifacts/flows/order-api--abc12345.json",
        )],
        threshold_exceeded: false,
        incomplete: false,
    }
}

/// Programmatic helper used by `App::browsing` callers.
#[allow(dead_code)]
fn scaffolding() -> App {
    App::browsing(EventSource::Test(Vec::new()), synthetic_outcome())
}
