//! Shared application state that every component reads from / mutates.

use crate::component::PageId;
use runnerguard_core::ScanEvent;
use runnerguard_model::{Diagnostic, Finding, ScanSummary};
use std::path::PathBuf;

/// Progress counters derived from the [`ScanEvent`] stream.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProgressCounters {
    pub project: Option<String>,
    pub stage: Stage,
    pub current_file: Option<String>,
    pub parsed_flows: usize,
    pub rules_loaded: usize,
    pub rules_completed: usize,
    pub finding_count: usize,
    pub ai_findings: usize,
    pub last_event: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Discovery,
    Parse,
    Rules,
    Ai,
    Reports,
    Done,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Discovery => "discover",
            Self::Parse => "parse",
            Self::Rules => "rules",
            Self::Ai => "ai",
            Self::Reports => "reports",
            Self::Done => "done",
        }
    }
}

/// Whole-app state. Components hold a `&mut AppState` and mutate it.
#[derive(Debug, Default)]
pub struct AppState {
    pub progress: ProgressCounters,
    pub findings: Vec<Finding>,
    pub diagnostics: Vec<Diagnostic>,
    pub report_paths: Vec<PathBuf>,
    pub artifact_paths: Vec<PathBuf>,
    pub summary: Option<ScanSummary>,
    pub current_page: PageId,
    pub focused_panel: PanelId,
    pub selections: Selections,
    pub filter: Option<String>,
    pub running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelId {
    Header,
    #[default]
    Body,
    Footer,
}

impl PanelId {
    pub const ALL: &'static [PanelId] = &[Self::Header, Self::Body, Self::Footer];
    pub fn cycle(self, dir: Cycle) -> Self {
        let idx = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        let n = Self::ALL.len() as i32;
        let mut next = idx as i32 + dir.step();
        if next < 0 {
            next = n - 1;
        } else if next >= n {
            next = 0;
        }
        Self::ALL[next as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cycle {
    Forward,
    Backward,
}

impl Cycle {
    pub fn step(self) -> i32 {
        match self {
            Self::Forward => 1,
            Self::Backward => -1,
        }
    }
}

/// Per-component selections.
#[derive(Debug, Default, Clone)]
pub struct Selections {
    pub finding_index: usize,
    pub flow_index: usize,
    pub diagnostic_index: usize,
    pub report_index: usize,
}

impl AppState {
    /// Apply a single [`ScanEvent`] to the app state. Used by the
    /// app's main loop and by tests.
    pub fn apply_event(&mut self, event: ScanEvent) {
        self.progress.last_event = Some(event_summary(&event));
        match event {
            ScanEvent::Started { project } => {
                self.running = true;
                self.progress.project = Some(project);
                self.progress.stage = Stage::Discovery;
            }
            ScanEvent::ConfigLoaded { .. } => {}
            ScanEvent::FileDiscovered { path } => {
                self.progress.current_file = Some(path);
                self.progress.stage = Stage::Discovery;
            }
            ScanEvent::RulesLoaded { rule_count } => {
                self.progress.rules_loaded = rule_count;
                self.progress.stage = Stage::Rules;
            }
            ScanEvent::XmlParsed { flows, .. } => {
                self.progress.parsed_flows += flows;
                self.progress.stage = Stage::Parse;
            }
            ScanEvent::FlowArtifactWritten { path, .. } => {
                self.artifact_paths.push(PathBuf::from(path));
            }
            ScanEvent::RuleStarted { .. } => {
                self.progress.stage = Stage::Rules;
            }
            ScanEvent::RuleCompleted {
                rule_id: _,
                findings,
            } => {
                self.progress.rules_completed += 1;
                self.progress.finding_count += findings;
            }
            ScanEvent::AiStarted => {
                self.progress.stage = Stage::Ai;
            }
            ScanEvent::AiCompleted { findings } => {
                self.progress.ai_findings = findings;
            }
            ScanEvent::ReportWritten { format, path } => {
                self.progress.stage = Stage::Reports;
                if matches!(format, runnerguard_model::ReportFormat::Markdown)
                    || matches!(format, runnerguard_model::ReportFormat::Html)
                {
                    self.report_paths.push(PathBuf::from(path));
                }
            }
            ScanEvent::Warning { diagnostic } => {
                self.diagnostics.push(diagnostic);
            }
            ScanEvent::Finished { summary } => {
                self.running = false;
                self.progress.stage = Stage::Done;
                self.summary = Some(summary);
            }
        }
    }

    /// Replace the cached findings list. The TUI typically receives this
    /// once the scan finishes.
    pub fn set_findings(&mut self, findings: Vec<Finding>) {
        self.findings = findings;
        self.progress.finding_count = self.findings.len();
        if self.selections.finding_index >= self.findings.len() && !self.findings.is_empty() {
            self.selections.finding_index = self.findings.len() - 1;
        }
    }

    /// Apply an optional regex filter on the cached findings list.
    /// Returns true if the visibility of any row changed.
    pub fn apply_filter(&mut self, pattern: &str) -> bool {
        use regex::Regex;
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let before = self.findings.len();
        self.findings.retain(|f| {
            f.rule_id.contains(pattern)
                || f.message.contains(pattern)
                || f.entity_id.as_deref().unwrap_or("").contains(pattern)
                || regex.is_match(&f.rule_id)
                || regex.is_match(&f.message)
        });
        before != self.findings.len()
    }

    pub fn highlighted_finding(&self) -> Option<&Finding> {
        self.findings.get(self.selections.finding_index)
    }
}

fn event_summary(event: &ScanEvent) -> String {
    match event {
        ScanEvent::Started { project } => format!("started: {project}"),
        ScanEvent::ConfigLoaded { config_path } => {
            format!("config: {config_path}")
        }
        ScanEvent::FileDiscovered { path } => format!("file: {path}"),
        ScanEvent::RulesLoaded { rule_count } => format!("rules: {rule_count}"),
        ScanEvent::XmlParsed { path, flows } => {
            format!("parsed {flows} flow(s): {path}")
        }
        ScanEvent::FlowArtifactWritten { flow_id, path } => {
            format!("artifact {flow_id}: {path}")
        }
        ScanEvent::RuleStarted { rule_id } => format!("rule start: {rule_id}"),
        ScanEvent::RuleCompleted { rule_id, findings } => {
            format!("rule {rule_id} => {findings} finding(s)")
        }
        ScanEvent::AiStarted => "ai started".to_string(),
        ScanEvent::AiCompleted { findings } => format!("ai done: {findings}"),
        ScanEvent::ReportWritten { format, path } => {
            format!("report {:?}: {path}", format)
        }
        ScanEvent::Warning { diagnostic } => format!("warning: {}", diagnostic.message),
        ScanEvent::Finished { .. } => "finished".to_string(),
    }
}
