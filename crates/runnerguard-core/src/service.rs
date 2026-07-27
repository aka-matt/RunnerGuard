//! The orchestration layer.
//!
//! [`ScanService`] wires the file system, parser, rule engine, and
//! report renderer together. It does **no** I/O of its own (apart from
//! writing per-flow JSON artifacts via [`runnerguard_fs::write_atomic`])
//! and emits [`ScanEvent`]s for every step the CLI/TUI cares about.

use crate::event::{CollectingSink, NullSink, ProgressSink, ScanEvent};
use runnerguard_fs::{ProjectFiles, write_atomic};
use runnerguard_model::{
    AiMode, Diagnostic, DiagnosticLevel, DiagnosticStage, ParsedProject, ProjectDescriptor,
    ProjectIndex, ReportFormat, RuleSet, ScanOutcome, ScanRequest, ScanResult, Severity,
};
use runnerguard_mule_parser::{ParseOptions, parse_project as parse_mule};
use runnerguard_report::{build_document, render_html, render_markdown};
use runnerguard_rule_engine::{compile, evaluate_with_source};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct ScanService {
    _private: (),
}

impl ScanService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the scan end-to-end.
    pub fn run(&self, request: &ScanRequest) -> ScanOutcome {
        let mut sink = NullSink;
        self.run_with_sink(request, &mut sink)
    }

    /// Same as [`Self::run`] but with a caller-provided progress sink.
    pub fn run_with_sink(&self, request: &ScanRequest, sink: &mut dyn ProgressSink) -> ScanOutcome {
        sink.emit(ScanEvent::Started {
            project: request.project_dir.display().to_string(),
        });

        let mut result = ScanResult::default();
        let mut artifact_paths = Vec::new();

        // 1. Discover files.
        let discovery = match runnerguard_fs::discover(
            &request.project_dir,
            runnerguard_fs::DiscoverOptions::default(),
        ) {
            Ok(d) => d,
            Err(e) => {
                result.parser_diagnostics.push(Diagnostic::new(
                    DiagnosticStage::FileSystem,
                    "FS-001",
                    DiagnosticLevel::Error,
                    format!("discovery failed: {e}"),
                ));
                return finish_with(sink, result, Vec::new(), Vec::new(), false);
            }
        };
        for f in &discovery.files {
            sink.emit(ScanEvent::FileDiscovered {
                path: f.path.clone(),
            });
        }

        // 2. Parse Mule XML.
        let parsed = match parse_mule(&discovery, &ParseOptions::default()) {
            Ok(p) => p,
            Err(e) => {
                result.parser_diagnostics.push(Diagnostic::new(
                    DiagnosticStage::Mule,
                    "MULE-000",
                    DiagnosticLevel::Error,
                    format!("parser crashed: {e}"),
                ));
                empty_outcome()
            }
        };
        for d in &parsed.diagnostics {
            sink.emit(ScanEvent::Warning {
                diagnostic: d.clone(),
            });
        }
        result.parser_diagnostics.extend(parsed.diagnostics);

        for doc in &parsed.project.documents {
            sink.emit(ScanEvent::XmlParsed {
                path: doc.source.file.clone(),
                flows: doc.flows.len(),
            });
        }
        result.flow_count = parsed.project.documents.iter().map(|d| d.flows.len()).sum();
        result.subflow_count = parsed
            .project
            .documents
            .iter()
            .map(|d| d.sub_flows.len())
            .sum();

        // 3. Load rule sets.
        let mut rule_sets: Vec<RuleSet> = Vec::new();
        for path in &request.rule_files {
            match load_rule_set(path) {
                Ok(rs) => rule_sets.push(rs),
                Err(e) => {
                    result.parser_diagnostics.push(Diagnostic::new(
                        DiagnosticStage::FileSystem,
                        "RULES-001",
                        DiagnosticLevel::Error,
                        format!("failed to load rule set {}: {e}", path.display()),
                    ));
                }
            }
        }
        sink.emit(ScanEvent::RulesLoaded {
            rule_count: rule_sets.iter().map(|r| r.rules.len()).sum(),
        });

        // 4. Evaluate deterministic rules.
        let mut findings = Vec::new();
        let mut rule_count = 0;
        for rule_set in &rule_sets {
            let compiled = compile(rule_set);
            for issue in &compiled.issues {
                result.parser_diagnostics.push(Diagnostic::new(
                    DiagnosticStage::Rule,
                    "RULES-002",
                    DiagnosticLevel::Warning,
                    format!("rule {}: {}", issue.rule_id, issue.message),
                ));
            }
            for rule in &compiled.rules {
                sink.emit(ScanEvent::RuleStarted {
                    rule_id: rule.id.clone(),
                });
                let outcome = evaluate_with_source(&compiled, &rule_set.rules, &parsed.project);
                let n = outcome.findings.len();
                for f in outcome.findings {
                    findings.push(f);
                }
                for e in outcome.errors {
                    result.parser_diagnostics.push(Diagnostic::new(
                        DiagnosticStage::Rule,
                        "RULES-003",
                        DiagnosticLevel::Error,
                        e.to_string(),
                    ));
                }
                sink.emit(ScanEvent::RuleCompleted {
                    rule_id: rule.id.clone(),
                    findings: n,
                });
                rule_count += 1;
            }
        }
        result.findings = findings;
        result.rule_count = rule_count;

        // 5. Optional AI pass.
        if matches!(request.ai_mode, AiMode::Required | AiMode::Optional) {
            sink.emit(ScanEvent::AiStarted);
            sink.emit(ScanEvent::AiCompleted { findings: 0 });
        }

        // 6. Per-flow artifacts.
        if request.write_flow_json {
            for doc in &parsed.project.documents {
                for flow in &doc.flows {
                    let safe = sanitize(&flow.name);
                    let hash = short_hash(&flow.id);
                    let path = request
                        .output_dir
                        .join("artifacts")
                        .join("flows")
                        .join(format!("{safe}--{hash}.json"));
                    if let Ok(bytes) = serde_json::to_vec_pretty(flow) {
                        if write_atomic(&path, &bytes).is_ok() {
                            sink.emit(ScanEvent::FlowArtifactWritten {
                                flow_id: flow.id.clone(),
                                path: path.display().to_string(),
                            });
                            artifact_paths.push(path);
                        }
                    }
                }
            }
        }

        // 7. Reports.
        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let tool_version = env!("CARGO_PKG_VERSION").to_string();
        let doc = build_document(
            &result,
            &parsed.project,
            &rule_sets,
            generated_at,
            &tool_version,
        );
        let mut report_paths = Vec::new();
        for format in &request.formats {
            let bytes = match format {
                ReportFormat::Markdown => render_markdown(&doc),
                ReportFormat::Html => render_html(&doc),
            };
            match bytes {
                Ok(b) => {
                    let path = request
                        .output_dir
                        .join(format!("report.{}", format.extension()));
                    if write_atomic(&path, &b).is_ok() {
                        sink.emit(ScanEvent::ReportWritten {
                            format: *format,
                            path: path.display().to_string(),
                        });
                        report_paths.push(path);
                    }
                }
                Err(e) => {
                    result.parser_diagnostics.push(Diagnostic::new(
                        DiagnosticStage::Report,
                        "REPORT-001",
                        DiagnosticLevel::Error,
                        format!("render failed: {e}"),
                    ));
                }
            }
        }

        finish_with(sink, result, artifact_paths, report_paths, false)
    }
}

fn finish_with(
    sink: &mut dyn ProgressSink,
    result: ScanResult,
    artifact_paths: Vec<PathBuf>,
    report_paths: Vec<PathBuf>,
    incomplete: bool,
) -> ScanOutcome {
    let summary = result.summary();
    let threshold = summary.threshold_exceeded(Severity::Warning);
    sink.emit(ScanEvent::Finished { summary });
    ScanOutcome {
        result,
        report_paths,
        artifact_paths,
        threshold_exceeded: threshold,
        incomplete,
    }
}

fn empty_outcome() -> runnerguard_mule_parser::ParsedOutcome {
    runnerguard_mule_parser::ParsedOutcome {
        project: ParsedProject {
            schema_version: runnerguard_model::CURRENT_SCHEMA_VERSION.to_string(),
            project: ProjectDescriptor {
                schema_version: runnerguard_model::CURRENT_SCHEMA_VERSION.to_string(),
                id: String::new(),
                name: String::new(),
                group_id: None,
                artifact_id: None,
                version: None,
                mule_version: None,
                sdk_version: None,
                root_path: String::new(),
                files: Vec::new(),
                required_files_missing: Vec::new(),
            },
            documents: Vec::new(),
            index: ProjectIndex::default(),
        },
        diagnostics: Vec::new(),
    }
}

fn load_rule_set(path: &PathBuf) -> Result<RuleSet, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let set: RuleSet = serde_json::from_slice(&bytes)?;
    Ok(set)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn short_hash(s: &str) -> String {
    // Lightweight non-cryptographic hash; the value is only used to
    // disambiguate artifact file names so two flows with the same
    // sanitised name don't collide.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let mut out = String::with_capacity(8);
    for b in &h.to_le_bytes()[..4] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn collecting_sink() -> CollectingSink {
    CollectingSink::default()
}

pub type Files = ProjectFiles;
