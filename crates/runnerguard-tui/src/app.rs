//! The main [`App`] that owns the components, the event loop, and the
//! cancellation token.

use crate::component::{Action, Component, Event, MoveDirection, PageId, from_crossterm};
use crate::components::diagnostics::DiagnosticsComponent;
use crate::components::finding_detail::FindingDetailComponent;
use crate::components::findings::FindingsTableComponent;
use crate::components::flow_tree::FlowTreeComponent;
use crate::components::footer::FooterHelpComponent;
use crate::components::header::HeaderComponent;
use crate::components::progress::ProgressComponent;
use crate::components::project_summary::ProjectSummaryComponent;
use crate::components::report::ReportPageComponent;
use crate::error::TuiError;
use crate::state::{AppState, Cycle, PanelId};
use crate::terminal::TerminalGuard;
use crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::interval;

/// Where the App receives events from. Useful for testing — tests
/// pass a `Vec<Event>` rather than polling stdin.
pub enum EventSource {
    /// Poll the real terminal via crossterm.
    Terminal,
    /// Use a pre-loaded vector of events; the TUI runs through them
    /// sequentially and exits when the list is exhausted.
    Test(Vec<Event>),
}

impl std::fmt::Debug for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal => f.debug_tuple("Terminal").finish(),
            Self::Test(v) => f.debug_tuple("Test").field(&v.len()).finish(),
        }
    }
}

/// Where the App receives scan events from. Most production code uses
/// `Scan` which spawns a background tokio task running `ScanService`.
pub enum ScanSource {
    /// A precomputed [`ScanOutcome`] — use this in tests and the demo.
    Preloaded(runnerguard_model::ScanOutcome),
    /// A receiver from a channel; the TUI drains it while it runs.
    Channel(UnboundedReceiver<runnerguard_core::ScanEvent>),
    /// No scan is being run; the TUI is in "browse this outcome" mode.
    None,
}

impl std::fmt::Debug for ScanSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preloaded(_) => f.debug_tuple("Preloaded").finish(),
            Self::Channel(_) => f.debug_tuple("Channel").finish(),
            Self::None => f.debug_tuple("None").finish(),
        }
    }
}

/// Top-level application state.
pub struct App {
    pub state: AppState,
    pub header: HeaderComponent,
    pub summary: ProjectSummaryComponent,
    pub progress: ProgressComponent,
    pub findings: FindingsTableComponent,
    pub finding_detail: FindingDetailComponent,
    pub flow_tree: FlowTreeComponent,
    pub diagnostics: DiagnosticsComponent,
    pub footer: FooterHelpComponent,
    pub report: ReportPageComponent,
    pub cancel: Arc<AtomicBool>,
    events: EventSource,
    scan_rx: Option<UnboundedReceiver<runnerguard_core::ScanEvent>>,
    preloaded: Option<runnerguard_model::ScanOutcome>,
    tick: Duration,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("state", &self.state)
            .field("events", &self.events)
            .finish()
    }
}

impl App {
    /// Build a new App. `events` decides where input comes from;
    /// `scan` decides whether and how a scan is feeding it.
    #[must_use]
    pub fn new(events: EventSource, scan: ScanSource) -> Self {
        let (scan_rx, preloaded) = match scan {
            ScanSource::Channel(rx) => (Some(rx), None),
            ScanSource::Preloaded(outcome) => (None, Some(outcome)),
            ScanSource::None => (None, None),
        };
        Self {
            state: AppState::default(),
            header: HeaderComponent,
            summary: ProjectSummaryComponent,
            progress: ProgressComponent,
            findings: FindingsTableComponent,
            finding_detail: FindingDetailComponent,
            flow_tree: FlowTreeComponent,
            diagnostics: DiagnosticsComponent,
            footer: FooterHelpComponent,
            report: ReportPageComponent,
            cancel: Arc::new(AtomicBool::new(false)),
            events,
            scan_rx,
            preloaded,
            tick: Duration::from_millis(250),
        }
    }

    /// Build an App that will run a real scan as soon as [`Self::run`]
    /// is called. The TUI is then asked to manage the scan lifecycle.
    #[must_use]
    pub fn with_scan(
        events: EventSource,
        scan_rx: UnboundedReceiver<runnerguard_core::ScanEvent>,
    ) -> Self {
        Self::new(events, ScanSource::Channel(scan_rx))
    }

    /// Build an App that browses a precomputed outcome (tests + demo).
    /// The outcome is immediately folded into state so callers can
    /// inspect `state.findings` / `state.summary` without first running
    /// the event loop.
    #[must_use]
    pub fn browsing(events: EventSource, outcome: runnerguard_model::ScanOutcome) -> Self {
        let mut app = Self::new(events, ScanSource::Preloaded(outcome));
        app.bootstrap_preloaded();
        app
    }

    /// Tick interval — exposed so tests can speed it up.
    #[must_use]
    pub fn with_tick(mut self, tick: Duration) -> Self {
        self.tick = tick;
        self
    }

    /// Main entry point. Takes the [`TerminalGuard`], runs the event
    /// loop, and returns once the user quits.
    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), TuiError> {
        let _ = self.tick; // suppress unused warning when run implementation is sync
        self.bootstrap_preloaded();
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                break;
            }
            self.drain_scan_events();
            terminal.draw(|frame| self.render(frame, frame.area()))?;
            let event_opt = self.poll_event();
            if let Some(ev) = event_opt {
                self.dispatch(ev);
            }
            if matches!(self.events, EventSource::Test(ref v) if v.is_empty()) {
                break;
            }
        }
        Ok(())
    }

    fn bootstrap_preloaded(&mut self) {
        if let Some(outcome) = self.preloaded.take() {
            self.state.set_findings(outcome.result.findings.clone());
            self.state.diagnostics = outcome.result.parser_diagnostics.clone();
            self.state.report_paths = outcome.report_paths.clone();
            self.state.artifact_paths = outcome.artifact_paths.clone();
            self.state.summary = Some(outcome.result.summary());
            self.state.progress.finding_count = outcome.result.findings.len();
            self.state.progress.rules_loaded = outcome.result.rule_count;
            self.state.progress.rules_completed = outcome.result.rule_count;
            self.state.progress.stage = crate::state::Stage::Done;
            self.state.running = false;
        }
    }

    fn drain_scan_events(&mut self) {
        let Some(rx) = self.scan_rx.as_mut() else {
            return;
        };
        while let Ok(event) = rx.try_recv() {
            self.state.apply_event(event);
        }
        if !self.state.running {
            self.scan_rx = None;
        }
    }

    fn poll_event(&mut self) -> Option<Event> {
        match &mut self.events {
            EventSource::Terminal => {
                if event::poll(Duration::from_millis(50)).ok()? {
                    let ev = event::read().ok()?;
                    Some(from_crossterm(ev))
                } else {
                    let _ = interval(self.tick);
                    Some(Event::Tick)
                }
            }
            EventSource::Test(vec) => vec.pop(),
        }
    }

    /// Dispatch one event through the global keymap and the focused
    /// component. Public for tests.
    pub fn dispatch(&mut self, event: Event) {
        if let Event::Key(k) = &event {
            if let Some(action) = self.global_keymap(k) {
                self.apply_action(action);
                return;
            }
        }
        let action = match self.state.focused_panel {
            PanelId::Header => self.header.handle_event(&event),
            PanelId::Body => self.body_handle_event(&event),
            PanelId::Footer => self.footer.handle_event(&event),
        };
        if let Some(action) = action {
            self.apply_action(action);
        }
    }

    fn body_handle_event(&mut self, event: &Event) -> Option<Action> {
        match self.state.current_page {
            PageId::ScanProgress => self.progress.handle_event(event),
            PageId::Findings => self.findings.handle_event(event),
            PageId::FindingDetail => self.finding_detail.handle_event(event),
            PageId::Flows => self.flow_tree.handle_event(event),
            PageId::Diagnostics => self.diagnostics.handle_event(event),
            PageId::Report => self.report.handle_event(event),
        }
    }

    /// The global keymap. Public for tests so they can assert what
    /// each key produces without sending a real event.
    pub fn global_keymap(&self, key: &KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Some(Action::Quit),
            KeyCode::Tab => Some(Action::NextPanel),
            KeyCode::BackTab => Some(Action::PrevPanel),
            KeyCode::Char('f') => Some(Action::Page(PageId::Findings)),
            KeyCode::Char('p') => Some(Action::Page(PageId::ScanProgress)),
            KeyCode::Char('g') => Some(Action::Page(PageId::Flows)),
            KeyCode::Char('d') => Some(Action::Page(PageId::Diagnostics)),
            KeyCode::Char('r') => Some(Action::Other("rerun".to_string())),
            KeyCode::Char('/') => Some(Action::Other("filter".to_string())),
            KeyCode::Enter => Some(Action::Page(PageId::FindingDetail)),
            KeyCode::Char('?') => Some(Action::Other("help".to_string())),
            _ => None,
        }
    }

    /// Apply a single action to the App. Public so tests can drive
    /// the state machine directly without going through the event loop.
    pub fn apply_action(&mut self, action: Action) {
        match &action {
            Action::Quit => {
                self.cancel.store(true, Ordering::SeqCst);
            }
            Action::NextPanel => {
                self.state.focused_panel = self.state.focused_panel.cycle(Cycle::Forward);
            }
            Action::PrevPanel => {
                self.state.focused_panel = self.state.focused_panel.cycle(Cycle::Backward);
            }
            Action::Page(page) => self.state.current_page = *page,
            Action::Move(dir) => self.apply_move(*dir),
            Action::Enter => {
                if let PageId::Findings = self.state.current_page {
                    self.state.current_page = PageId::FindingDetail;
                }
            }
            Action::ClearFilter => {
                self.state.filter = None;
            }
            Action::Other(payload) => self.apply_other(payload.clone()),
        }
        let _ = self.findings.update(&action);
        let _ = self.finding_detail.update(&action);
        let _ = self.flow_tree.update(&action);
        let _ = self.diagnostics.update(&action);
        let _ = self.footer.update(&action);
        let _ = self.report.update(&action);
        let _ = self.progress.update(&action);
        let _ = self.summary.update(&action);
        let _ = self.header.update(&action);
    }

    fn apply_move(&mut self, dir: MoveDirection) {
        let n = self.state.findings.len();
        if n == 0 {
            return;
        }
        let cur = self.state.selections.finding_index;
        let next = match dir {
            MoveDirection::Up => cur.saturating_sub(1),
            MoveDirection::Down => (cur + 1).min(n - 1),
            MoveDirection::PageUp => cur.saturating_sub(8),
            MoveDirection::PageDown => (cur + 8).min(n - 1),
            MoveDirection::Home => 0,
            MoveDirection::End => n - 1,
        };
        self.state.selections.finding_index = next;
    }

    fn apply_other(&mut self, payload: String) {
        match payload.as_str() {
            "rerun" => {
                self.cancel.store(true, Ordering::SeqCst);
            }
            "filter" => {
                self.state.filter = Some(String::new());
            }
            "help" => {
                self.state.current_page = PageId::ScanProgress;
            }
            _ => {}
        }
    }

    /// Render the entire frame. Public so that the demo / tests can
    /// draw without going through the event loop.
    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);
        let header_focused = self.state.focused_panel == PanelId::Header;
        let footer_focused = self.state.focused_panel == PanelId::Footer;
        crate::components::header::render_dynamic_header(
            frame,
            outer[0],
            &self.state,
            header_focused,
        );
        self.render_body(frame, outer[1]);
        FooterHelpComponent::render_for(frame, outer[2], &self.state, footer_focused);
    }

    fn render_body(&mut self, frame: &mut Frame<'_>, area: Rect) {
        match self.state.current_page {
            PageId::ScanProgress => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(3)])
                    .split(area);
                ProjectSummaryComponent::render_for(frame, chunks[0], &self.state, false);
                ProgressComponent::render_for(frame, chunks[1], &self.state, false);
            }
            PageId::Findings => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(5), Constraint::Length(8)])
                    .split(area);
                FindingsTableComponent::render_for(frame, chunks[0], &self.state, true);
                FindingDetailComponent::render_for(frame, chunks[1], &self.state, false);
            }
            PageId::FindingDetail => {
                FindingDetailComponent::render_for(frame, area, &self.state, true);
            }
            PageId::Flows => {
                FlowTreeComponent::render_for(frame, area, &self.state, true);
            }
            PageId::Diagnostics => {
                DiagnosticsComponent::render_for(frame, area, &self.state, true);
            }
            PageId::Report => {
                ReportPageComponent::render_for(frame, area, &self.state, true);
            }
        }
    }
}

/// Convenience: build a `TerminalGuard` + `App` and run them on the
/// real terminal. Returns once the user quits.
pub fn run_cli(scan: ScanSource) -> Result<(), TuiError> {
    let mut guard = TerminalGuard::enter()?;
    let mut app = App::new(EventSource::Terminal, scan);
    app.run(guard.terminal_mut())?;
    guard.dispose()?;
    Ok(())
}
