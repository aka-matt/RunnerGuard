//! Progress events emitted by [`ScanService`].
//!
//! The CLI and TUI both consume this stream. The events are the only
//! way core talks to the outside world during a scan — no `println!`,
//! no terminal escapes, no log macros that depend on terminal state.

use runnerguard_model::{Diagnostic, ReportFormat, ScanSummary};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ScanEvent {
    Started { project: String },
    ConfigLoaded { config_path: String },
    FileDiscovered { path: String },
    RulesLoaded { rule_count: usize },
    XmlParsed { path: String, flows: usize },
    FlowArtifactWritten { flow_id: String, path: String },
    RuleStarted { rule_id: String },
    RuleCompleted { rule_id: String, findings: usize },
    AiStarted,
    AiCompleted { findings: usize },
    ReportWritten { format: ReportFormat, path: String },
    Warning { diagnostic: Diagnostic },
    Finished { summary: ScanSummary },
}

/// Anything that can receive scan events. The CLI sink writes text, the
/// TUI sink pushes to a Ratatui-bound channel.
pub trait ProgressSink: Send {
    fn emit(&mut self, event: ScanEvent);
}

/// A no-op sink used in tests and when the caller doesn't care about
/// progress (e.g. running headless inside CI).
#[derive(Debug, Default)]
pub struct NullSink;
impl ProgressSink for NullSink {
    fn emit(&mut self, _event: ScanEvent) {}
}

/// A sink that collects every event into a `Vec` for later inspection.
#[derive(Debug, Default)]
pub struct CollectingSink {
    pub events: Vec<ScanEvent>,
}
impl ProgressSink for CollectingSink {
    fn emit(&mut self, event: ScanEvent) {
        self.events.push(event);
    }
}
