//! `runnerguard` — the CLI binary.
//!
//! This crate is intentionally thin: clap wiring, exit-code handling,
//! and the call into [`runnerguard_core`]. No XML parsing, rule
//! evaluation, or report rendering lives here.

use clap::{Parser, ValueEnum};
use runnerguard_core::{ProgressSink, ScanEvent, ScanService};
use runnerguard_model::{AiMode, ReportFormat, ScanRequest, Severity};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "runnerguard",
    about = "Static analysis for MuleSoft 4 projects",
    version
)]
struct Cli {
    /// Path to the MuleSoft project to scan.
    #[arg(value_name = "PROJECT")]
    project: PathBuf,

    /// One or more rule-set JSON files to evaluate.
    #[arg(short = 'r', long = "rule", value_name = "RULES")]
    rules: Vec<PathBuf>,

    /// Directory to write reports and artifacts into.
    #[arg(short = 'o', long = "output", default_value = "./runnerguard-out")]
    output: PathBuf,

    /// Report formats to produce. Defaults to Markdown + HTML.
    #[arg(long = "format", value_enum, default_values_t = [ReportFormatArg::Markdown, ReportFormatArg::Html])]
    format: Vec<ReportFormatArg>,

    /// Disable writing per-flow JSON artifacts.
    #[arg(long = "no-flow-json")]
    no_flow_json: bool,

    /// Fail (exit 1) when findings reach or exceed this severity.
    #[arg(long = "fail-on", value_enum, default_value_t = SeverityArg::Warning)]
    fail_on: SeverityArg,

    /// AI analysis mode.
    #[arg(long = "ai", value_enum, default_value_t = AiArg::Disabled)]
    ai: AiArg,

    /// Print every progress event as a JSON line.
    #[arg(long = "json-events")]
    json_events: bool,

    /// Path to a YAML config file (loaded before flags; flags override).
    #[arg(long = "config")]
    config: Option<PathBuf>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ReportFormatArg {
    Markdown,
    Html,
}

impl From<ReportFormatArg> for ReportFormat {
    fn from(value: ReportFormatArg) -> Self {
        match value {
            ReportFormatArg::Markdown => ReportFormat::Markdown,
            ReportFormatArg::Html => ReportFormat::Html,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum SeverityArg {
    Info,
    Warning,
    Error,
    Critical,
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Info => Severity::Info,
            SeverityArg::Warning => Severity::Warning,
            SeverityArg::Error => Severity::Error,
            SeverityArg::Critical => Severity::Critical,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum AiArg {
    Disabled,
    Optional,
    Required,
}

impl From<AiArg> for AiMode {
    fn from(value: AiArg) -> Self {
        match value {
            AiArg::Disabled => AiMode::Disabled,
            AiArg::Optional => AiMode::Optional,
            AiArg::Required => AiMode::Required,
        }
    }
}

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    let formats: Vec<ReportFormat> = cli.format.iter().copied().map(Into::into).collect();

    let req = ScanRequest {
        project_dir: cli.project.clone(),
        rule_files: cli.rules.clone(),
        output_dir: cli.output.clone(),
        formats,
        include_tests: false,
        write_flow_json: !cli.no_flow_json,
        ai_mode: cli.ai.into(),
        fail_on: cli.fail_on.into(),
        rule_filter: Default::default(),
    };

    let mut sink: Box<dyn ProgressSink> = if cli.json_events {
        Box::new(JsonEventSink)
    } else {
        Box::new(CliTextSink)
    };

    let outcome = ScanService::new().run_with_sink(&req, sink.as_mut());

    let summary = outcome.result.summary();
    if !cli.json_events {
        println!(
            "Found {} finding(s); wrote {} report(s) and {} artifact(s).",
            summary.findings_total,
            outcome.report_paths.len(),
            outcome.artifact_paths.len()
        );
    }

    // Exit-code precedence:
    //   3 — XML / Mule parse error
    //   5 — report-write failure (see REPORT-001 diagnostics)
    //   1 — threshold exceeded
    //   4 — forced AI / network operation failed
    //   0 — success
    let mut report_write_failed = false;
    for d in &outcome.result.parser_diagnostics {
        if d.stage == runnerguard_model::DiagnosticStage::Report
            && matches!(d.level, runnerguard_model::DiagnosticLevel::Error)
        {
            report_write_failed = true;
            break;
        }
    }
    if outcome.result.parser_diagnostics.iter().any(|d| {
        matches!(
            d.stage,
            runnerguard_model::DiagnosticStage::Xml | runnerguard_model::DiagnosticStage::Mule
        ) && matches!(d.level, runnerguard_model::DiagnosticLevel::Error)
    }) {
        return ExitCode::from(3);
    }
    if report_write_failed {
        return ExitCode::from(5);
    }
    if outcome.threshold_exceeded {
        return ExitCode::from(1);
    }
    if outcome.incomplete {
        return ExitCode::from(4);
    }
    ExitCode::SUCCESS
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_target(false)
        .try_init();
}

#[derive(Default)]
struct CliTextSink;

impl ProgressSink for CliTextSink {
    fn emit(&mut self, event: ScanEvent) {
        match event {
            ScanEvent::Started { project } => {
                println!("→ Scanning {project}");
            }
            ScanEvent::FileDiscovered { path } => {
                println!("  found {path}");
            }
            ScanEvent::XmlParsed { path, flows } => {
                println!("  parsed {path} ({flows} flow(s))");
            }
            ScanEvent::RuleStarted { rule_id } => {
                println!("  • evaluating {rule_id}");
            }
            ScanEvent::RuleCompleted { rule_id, findings } => {
                if findings > 0 {
                    println!("  ✗ {rule_id}: {findings} finding(s)");
                }
            }
            ScanEvent::Warning { diagnostic } => {
                eprintln!("  ! [{}] {}", diagnostic.code, diagnostic.message);
            }
            ScanEvent::ReportWritten { format, path } => {
                println!("  ✓ wrote {path} ({format:?})");
            }
            ScanEvent::AiStarted => println!("  → invoking AI"),
            ScanEvent::AiCompleted { findings } => {
                println!("  ✓ AI added {findings} suggestion(s)")
            }
            ScanEvent::FlowArtifactWritten { flow_id, path } => {
                println!("    ↳ wrote artifact for {flow_id} → {path}")
            }
            ScanEvent::RulesLoaded { rule_count } => {
                println!("  loaded {rule_count} rule(s)")
            }
            ScanEvent::ConfigLoaded { config_path } => {
                println!("  config: {config_path}")
            }
            ScanEvent::Finished { summary } => {
                println!(
                    "  result: {} finding(s) ({} critical / {} error / {} warning / {} info)",
                    summary.findings_total,
                    summary.critical,
                    summary.error,
                    summary.warning,
                    summary.info,
                );
            }
        }
    }
}

#[derive(Default)]
struct JsonEventSink;

impl ProgressSink for JsonEventSink {
    fn emit(&mut self, event: ScanEvent) {
        if let Ok(line) = serde_json::to_string(&event) {
            println!("{line}");
        }
    }
}
