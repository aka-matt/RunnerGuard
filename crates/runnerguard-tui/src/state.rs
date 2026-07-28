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
    /// The authoritative findings list (never filtered).
    pub findings: Vec<Finding>,
    /// Filtered view of `findings` when a filter is active.
    pub visible_findings: Vec<Finding>,
    pub diagnostics: Vec<Diagnostic>,
    pub report_paths: Vec<PathBuf>,
    pub artifact_paths: Vec<PathBuf>,
    pub summary: Option<ScanSummary>,
    pub current_page: PageId,
    pub focused_panel: PanelId,
    pub selections: Selections,
    /// The currently applied findings filter pattern. `None` means
    /// "show everything". Updated by [`crate::app::App::apply_action`]
    /// when the user commits a pattern from filter-input mode.
    pub filter: Option<String>,
    /// True while the user is typing a new filter pattern via the
    /// `/` affordance. While this is set, key events are routed to
    /// the filter-input machinery instead of the focused component.
    pub filter_mode: bool,
    /// In-progress draft pattern while [`Self::filter_mode`] is true.
    /// Pre-filled from [`Self::filter`] when the user enters the
    /// mode so they can edit the existing pattern instead of
    /// starting from scratch.
    pub filter_input: String,
    pub running: bool,
    pub config_path: Option<PathBuf>,
    /// Number of data rows the findings table could show on its
    /// most recent render. Refreshed by `App::render_body` from the
    /// actual layout chunk so `scroll_findings_offset_into_view` can
    /// walk the highlight through every visible row instead of
    /// pinning it to a constant "PageDown-step" slot. `0` means no
    /// render has happened yet.
    pub findings_viewport_rows: usize,
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
///
/// Each list-style panel tracks both the currently-selected row and
/// the scroll offset of its first visible row. The two are kept in
/// sync by [`App::apply_move`](crate::app::App::apply_move) — the
/// selection moves under the user's finger, and the offset is
/// adjusted so the selection stays inside the viewport. Storing both
/// on the `AppState` instead of inside the ratatui widget means the
/// scroll position survives every render (previously each component
/// built a fresh `TableState::default()` on every frame, dropping
/// the offset the widget had silently adjusted the previous frame).
#[derive(Debug, Default, Clone)]
pub struct Selections {
    pub finding_index: usize,
    /// Scroll offset of the findings table — the index of the first
    /// visible row. Adjusted by [`crate::app::App::apply_move`] so
    /// the highlighted row always lies inside the viewport.
    pub finding_offset: usize,
    pub flow_index: usize,
    /// Scroll offset of the flow tree.
    pub flow_offset: usize,
    pub diagnostic_index: usize,
    /// Scroll offset of the diagnostics list.
    pub diagnostic_offset: usize,
    pub report_index: usize,
    /// Scroll offset of the report page.
    pub report_offset: usize,
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
            ScanEvent::ConfigLoaded { config_path } => {
                self.config_path = Some(PathBuf::from(&config_path));
                self.progress.last_event = Some(format!("config loaded: {config_path}"));
            }
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
            ScanEvent::Finding(finding) => {
                // Live-mode (`--tui-live`) only has the event stream —
                // the `ScanOutcome` is not handed to the App directly.
                // We push each finding into the authoritative list and
                // refresh the filter view so the user sees results as
                // they arrive.
                self.findings.push(finding);
                self.progress.finding_count = self.findings.len();
                self.recompute_visible();
                // Keep the cursor on the most recent finding when a
                // filter is active; otherwise let the user explore.
                if self.filter.is_some() && !self.visible_findings.is_empty() {
                    self.selections.finding_index = self.visible_findings.len() - 1;
                }
            }
            ScanEvent::RuleCompleted { rule_id, findings } => {
                self.progress.rules_completed += 1;
                // `finding_count` is owned by the `Finding` event path
                // for live mode and by `set_findings` for preloaded
                // mode; only the bare CLI text path relies on this
                // counter being incremented. Do not double-count here.
                self.progress.last_event =
                    Some(format!("rule {rule_id} produced {findings} finding(s)"));
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
        self.recompute_visible();
        if self.selections.finding_index >= self.visible_findings.len()
            && !self.visible_findings.is_empty()
        {
            self.selections.finding_index = self.visible_findings.len() - 1;
        }
    }

    /// Apply an optional regex filter on the cached findings list.
    /// Returns true if the visibility of any row changed.
    ///
    /// The filter narrows `visible_findings` but **never** mutates the
    /// authoritative `findings` list — clearing the filter restores
    /// the original view.
    pub fn apply_filter(&mut self, pattern: &str) -> bool {
        use regex::Regex;
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let before = self.visible_findings.len();
        self.filter = Some(pattern.to_string());
        self.recompute_visible_with(&regex, pattern);
        // Clamp selection into the new view.
        if self.selections.finding_index >= self.visible_findings.len() {
            self.selections.finding_index = self.visible_findings.len().saturating_sub(1);
        }
        // Clamp the scroll offset into the same range so the next
        // render doesn't dereference past the end of the now-shorter
        // list — that was the secondary symptom of the same offset
        // bug as the "can only see one issue" complaint.
        if self.selections.finding_offset >= self.visible_findings.len() {
            self.selections.finding_offset = self.visible_findings.len().saturating_sub(1);
        }
        before != self.visible_findings.len()
    }

    /// Clear any active filter and restore the full findings view.
    pub fn clear_filter(&mut self) {
        self.filter = None;
        self.visible_findings = self.findings.clone();
    }

    fn recompute_visible(&mut self) {
        if let Some(pat) = self.filter.clone() {
            if let Ok(regex) = regex::Regex::new(&pat) {
                self.recompute_visible_with(&regex, &pat);
                return;
            }
        }
        self.visible_findings = self.findings.clone();
    }

    fn recompute_visible_with(&mut self, regex: &regex::Regex, pattern: &str) {
        self.visible_findings = self
            .findings
            .iter()
            .filter(|f| {
                f.rule_id.contains(pattern)
                    || f.message.contains(pattern)
                    || f.entity_id.as_deref().unwrap_or("").contains(pattern)
                    || regex.is_match(&f.rule_id)
                    || regex.is_match(&f.message)
            })
            .cloned()
            .collect();
    }

    pub fn highlighted_finding(&self) -> Option<&Finding> {
        self.visible_findings.get(self.selections.finding_index)
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
        ScanEvent::Finding(f) => format!("finding: {} ({})", f.rule_id, f.severity.as_str()),
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
