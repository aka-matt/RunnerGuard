//! `runnerguard` — the CLI binary.
//!
//! This crate is intentionally thin: clap wiring, exit-code handling,
//! and the call into [`runnerguard_core`]. No XML parsing, rule
//! evaluation, or report rendering lives here.
//!
//! Three progress-sink strategies exist:
//!
//! * `--json-events` — line-delimited JSON to stdout (CI-friendly).
//! * (default) — pretty text on stdout (CLI summary).
//! * `--tui` — run the scan, then open the Ratatui TUI in review mode
//!   over the final `ScanOutcome`.
//! * `--tui-live` — open the TUI while the scan is still running;
//!   events stream in over a channel.
//!
//! The last two are gated behind the `tui` cargo feature.

use clap::Parser;
use runnerguard_core::{ProgressSink, ScanEvent, ScanService};
use runnerguard_model::{AiMode, ReportFormat, ScanOutcome, ScanRequest, Severity};
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

    /// Print every progress event as a JSON line. Mutually exclusive
    /// with the TUI flags.
    #[arg(long = "json-events")]
    json_events: bool,

    /// Path to a YAML config file (loaded before flags; flags override).
    #[arg(long = "config")]
    config: Option<PathBuf>,

    /// Run the scan silently, then open the interactive TUI in review
    /// mode over the final outcome. Requires the `tui` cargo feature.
    /// Mutually exclusive with `--tui-live` and `--json-events`.
    #[cfg(feature = "tui")]
    #[arg(long = "tui", conflicts_with_all = ["tui_live", "json_events"])]
    tui: bool,

    /// Open the interactive TUI immediately and stream `ScanEvent`s in
    /// while the scan runs in a background thread. Requires the `tui`
    /// cargo feature. Mutually exclusive with `--tui` and `--json-events`.
    #[cfg(feature = "tui")]
    #[arg(long = "tui-live", conflicts_with_all = ["tui", "json_events"])]
    tui_live: bool,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
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

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
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

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
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

    // ── TUI paths ────────────────────────────────────────────────
    // Branch before constructing the text/json sink so live mode can
    // stream events into the TUI's channel instead of stdout.
    #[cfg(feature = "tui")]
    {
        if cli.tui {
            return run_tui_preloaded(&req);
        }
        if cli.tui_live {
            return run_tui_live(&req);
        }
    }

    // ── CLI text / JSON-event paths ──────────────────────────────
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

    exit_code_for(&outcome)
}

/// Translate a finished [`ScanOutcome`] into the documented exit code.
///
/// Precedence (highest first):
/// 1. 3 — XML / Mule parse error (Error-level Xml or Mule diagnostic).
/// 2. 5 — report-write failure (REPORT-001 / REPORT-002).
/// 3. 1 — `--fail-on` threshold exceeded.
/// 4. 4 — AI / network forced operation failed (incomplete).
/// 5. 0 — success.
fn exit_code_for(outcome: &ScanOutcome) -> ExitCode {
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

// ─── TUI modes (feature-gated) ─────────────────────────────────────

/// Run the scan silently into a [`NullSink`], then enter the TUI over
/// the resulting [`ScanOutcome`]. Final exit code honours `--fail-on`.
#[cfg(feature = "tui")]
fn run_tui_preloaded(req: &ScanRequest) -> ExitCode {
    use runnerguard_core::NullSink;
    use runnerguard_tui::{ScanSource, run_cli};

    let mut sink = NullSink;
    let outcome = ScanService::new().run_with_sink(req, &mut sink);
    let threshold_exceeded = outcome.threshold_exceeded;

    match run_cli(ScanSource::Preloaded(outcome)) {
        Ok(()) => exit_code_for_threshold(threshold_exceeded),
        Err(err) => {
            eprintln!("tui error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Run the TUI immediately; the actual scan runs on a worker thread
/// and feeds the TUI's receiver via [`ChannelSink`]. Report files are
/// written by the worker thread; the TUI never blocks on report I/O.
///
/// Because the scan happens off-thread we cannot honour `--fail-on`
/// after TUI exit — the worker may still be writing reports. We exit
/// with 0 on clean quit and 4 only if the worker died (which the user
/// would already see as a stuck UI).
#[cfg(feature = "tui")]
fn run_tui_live(req: &ScanRequest) -> ExitCode {
    use runnerguard_tui::{ScanSource, run_cli};

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ScanEvent>();
    let req_for_thread = req.clone();
    let handle = match std::thread::Builder::new()
        .name("runnerguard-scan".to_string())
        .spawn(move || {
            let mut sink = ChannelSink { tx };
            // Drop the sender when the scan returns so the TUI's
            // receiver can drain to completion.
            ScanService::new().run_with_sink(&req_for_thread, &mut sink);
        }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("failed to spawn scan thread: {e}");
            return ExitCode::from(2);
        }
    };

    let tui_result = run_cli(ScanSource::Channel(rx));

    // The scan thread continues to completion in the background so
    // reports + artifacts are still written. We join it so the
    // process only exits after the worker is done. Bound the wait
    // so a stuck TUI quit doesn't deadlock the worker.
    match handle.join() {
        Ok(()) => {}
        Err(_) => eprintln!("warning: scan worker panicked"),
    }

    match tui_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tui error: {err}");
            ExitCode::from(2)
        }
    }
}

#[cfg(feature = "tui")]
fn exit_code_for_threshold(threshold_exceeded: bool) -> ExitCode {
    if threshold_exceeded {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// [`ProgressSink`] adapter that forwards every event into the TUI's
/// event channel. Send errors are ignored: the receiver is dropped on
/// TUI exit, and we don't want to poison the scan thread.
#[cfg(feature = "tui")]
struct ChannelSink {
    tx: tokio::sync::mpsc::UnboundedSender<ScanEvent>,
}

#[cfg(feature = "tui")]
impl ProgressSink for ChannelSink {
    fn emit(&mut self, event: ScanEvent) {
        let _ = self.tx.send(event);
    }
}

// ─── CLI text / JSON sinks ─────────────────────────────────────────

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
            ScanEvent::Finding(_) => {
                // Findings are emitted one-per-finding as the rule
                // engine produces them. The CLI text sink reports
                // them per-rule via `RuleCompleted { findings }`; we
                // suppress a per-finding line so the existing summary
                // output shape doesn't change.
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
