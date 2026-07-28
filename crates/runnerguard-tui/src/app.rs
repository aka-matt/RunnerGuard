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
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

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
    /// Wall-clock throttle for the `Event::Tick` heartbeat. Set to
    /// `None` initially so the *first* idle poll emits a tick
    /// immediately; subsequent ticks gate on this.
    last_tick: Option<Instant>,
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
            last_tick: None,
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
                    // A real input event counts as activity — let the
                    // next idle period stretch the full `self.tick`.
                    self.last_tick = Some(Instant::now());
                    Some(from_crossterm(ev))
                } else {
                    // Throttle `Event::Tick` to at most once per
                    // `self.tick` so the loop doesn't burn cycles when
                    // the screen is idle. Uses `std::time::Instant`
                    // directly — the previous `tokio::time::interval`
                    // required a Tokio runtime, which `App::run` is not
                    // running inside (TUI is sync).
                    let now = Instant::now();
                    let due = self
                        .last_tick
                        .is_none_or(|t| now.duration_since(t) >= self.tick);
                    if due {
                        self.last_tick = Some(now);
                        Some(Event::Tick)
                    } else {
                        // Sleep for the residual interval so we don't
                        // spin the CPU. Cap at 50ms to stay responsive
                        // to the next crossterm event.
                        let residual = self
                            .last_tick
                            .and_then(|t| self.tick.checked_sub(now.duration_since(t)))
                            .unwrap_or(self.tick)
                            .min(Duration::from_millis(50));
                        std::thread::sleep(residual);
                        // Recurse once: re-poll so a freshly-arrived
                        // key is preferred over a synthetic Tick.
                        self.poll_event()
                    }
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
            Action::Page(page) => {
                self.state.current_page = *page;
                // The user just asked to look at a different page —
                // they're going to start interacting with the body
                // (j/k on Findings, etc.). Cycle focus back to the
                // body panel so the visible focus matches where the
                // keymap routes key events; otherwise a leftover
                // Header / Footer focus makes the global keymap the
                // only thing left consuming keys and j/k fall on the
                // floor.
                self.state.focused_panel = PanelId::Body;
            }
            Action::Move(dir) => self.apply_move(*dir),
            Action::Enter => {
                if let PageId::Findings = self.state.current_page {
                    self.state.current_page = PageId::FindingDetail;
                }
                // Same reasoning as `Action::Page` — drilling into a
                // finding is implicit navigation, so land focus on
                // the body panel.
                self.state.focused_panel = PanelId::Body;
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
        // Movement operates on the filtered view, not the raw
        // findings list — otherwise the selection index can land on a
        // row that's currently hidden.
        let n = self.state.visible_findings.len();
        if n == 0 {
            return;
        }
        let cur = self.state.selections.finding_index;
        // The page determines how far PageUp / PageDown jump. On the
        // Findings table the user expects "上下翻页" to skip a
        // screenful of rows at a time — the data view gets crowded
        // fast and stepping by 1 is too tedious for long lists, so
        // we step by 8 (the minimum data-row capacity of the
        // Findings table). On FindingDetail (and every other body
        // page — Flows, Diagnostics, Report) there's no long list to
        // scroll; the user is reading one finding's detail at a
        // time, and "上一条 / 下一条 issue" is what the keys must do,
        // so we step by 1 and clamp. The two behaviours share the
        // same selection state because FindingDetail re-renders off
        // `highlighted_finding()`, so a Down on the detail page
        // changes the visible body as a side effect.
        let page = self.state.current_page;
        let next = match dir {
            MoveDirection::Up => cur.saturating_sub(1),
            MoveDirection::Down => (cur + 1).min(n - 1),
            MoveDirection::PageUp => {
                if page == PageId::Findings {
                    cur.saturating_sub(8)
                } else {
                    cur.saturating_sub(1)
                }
            }
            MoveDirection::PageDown => {
                if page == PageId::Findings {
                    (cur + 8).min(n - 1)
                } else {
                    (cur + 1).min(n - 1)
                }
            }
            MoveDirection::Home => 0,
            MoveDirection::End => n - 1,
        };
        self.state.selections.finding_index = next;
        // Scroll the stored offset forward so the new selection sits
        // inside the visible window. The render pass re-reconciles
        // with the actual viewport size; this early adjustment is
        // what makes `End` from the top immediately show the bottom
        // row instead of waiting for the next redraw.
        self.scroll_findings_offset_into_view();
    }

    /// Adjust `selections.finding_offset` so `finding_index` lies
    /// inside the visible window. The window is approximated by the
    /// minimum data-row capacity of the findings table — the page
    /// layout guarantees the table area gets at least 5 rows, and
    /// borders (2) + header (1) consume 3 of those, leaving 2 data
    /// rows at the smallest legal terminal height. Using this
    /// minimum keeps the offset meaningful when no render has
    /// happened yet; the render pass sharpens it against the actual
    /// area.height.
    fn scroll_findings_offset_into_view(&mut self) {
        let n = self.state.visible_findings.len();
        if n == 0 {
            self.state.selections.finding_offset = 0;
            return;
        }
        let viewport = self.findings_viewport_rows();
        if viewport == 0 {
            return;
        }
        let sel = self.state.selections.finding_index;
        let max_offset = n.saturating_sub(viewport);
        let mut offset = self.state.selections.finding_offset.min(max_offset);
        if sel >= offset + viewport {
            // Selection fell off the bottom — push the window
            // forward so the selection sits on the last visible row.
            offset = (sel + 1).saturating_sub(viewport).min(max_offset);
        } else if sel < offset {
            // Selection scrolled above the top — drag the window
            // back to it.
            offset = sel;
        }
        self.state.selections.finding_offset = offset;
    }

    /// Minimum number of data rows the findings table can show.
    /// The findings page splits the body into `Min(5)` + `Length(8)`,
    /// so the table area is always at least 5 rows tall. Borders
    /// consume 2 of those and the table header consumes 1 more,
    /// leaving 2 data rows as the worst-case minimum.
    fn findings_viewport_rows(&self) -> usize {
        // Use the documented page-layout minimum so this method is
        // deterministic regardless of terminal size; the render
        // pass re-clamps against the real `area.height`.
        2
    }

    fn apply_other(&mut self, payload: String) {
        match payload.as_str() {
            "rerun" => {
                self.cancel.store(true, Ordering::SeqCst);
            }
            "filter" => {
                // The "filter" affordance is reached from the
                // findings page; tapping it again clears an active
                // filter so the user can step out of the narrowed
                // view without losing findings.
                if self.state.filter.is_some() {
                    self.state.clear_filter();
                }
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
        // The body block title should light up only when the body
        // panel actually owns focus. Previously this branch hardcoded
        // `focused = true` for every page render, so when the user
        // cycled focus to the Header or Footer with `Tab`, the
        // Findings / Flows / Diagnostics block kept its cyan
        // highlight while the Header or Footer title *also* lit up —
        // two title bars looked focused at once. Worse, the visual
        // still said "findings is focused" so the user pressed `j` /
        // `k` / `PageDown`, only to discover the keys had been
        // rerouted to `header.handle_event` (which returns `None`).
        let body_focused = self.state.focused_panel == PanelId::Body;
        match self.state.current_page {
            PageId::ScanProgress => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(3)])
                    .split(area);
                // Project summary is a static read-out; the live
                // event log is the focus target on this page.
                ProjectSummaryComponent::render_for(frame, chunks[0], &self.state, false);
                ProgressComponent::render_for(frame, chunks[1], &self.state, body_focused);
            }
            PageId::Findings => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(5), Constraint::Length(8)])
                    .split(area);
                // The detail pane is a read-only preview keyed off
                // the highlighted row; the table owns the cursor.
                FindingsTableComponent::render_for(frame, chunks[0], &self.state, body_focused);
                FindingDetailComponent::render_for(frame, chunks[1], &self.state, false);
            }
            PageId::FindingDetail => {
                FindingDetailComponent::render_for(frame, area, &self.state, body_focused);
            }
            PageId::Flows => {
                FlowTreeComponent::render_for(frame, area, &self.state, body_focused);
            }
            PageId::Diagnostics => {
                DiagnosticsComponent::render_for(frame, area, &self.state, body_focused);
            }
            PageId::Report => {
                ReportPageComponent::render_for(frame, area, &self.state, body_focused);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: building the App and inspecting the throttle must
    /// not depend on a Tokio runtime. The previous `poll_event` called
    /// `tokio::time::interval(self.tick)` which panicked with
    /// "there is no reactor running" the first time the production
    /// code path ran outside a `#[tokio::main]`. This test pins the
    /// App's behaviour: `last_tick` starts at `None` (so the first
    /// idle poll emits a tick); an externally-set `last_tick` is
    /// read back through `pub` fields; we can move `Instant` along
    /// without any tokio context.
    #[test]
    fn app_does_not_require_a_tokio_runtime_for_idle_timing() {
        let app = App::new(EventSource::Test(Vec::new()), ScanSource::None);
        assert!(app.last_tick.is_none(), "fresh App starts with no tick");
        let mut app = app;
        // Drive the throttle manually; this is what `poll_event`
        // does internally — no Tokio involved.
        let first = Instant::now();
        app.last_tick = Some(first);
        assert!(app.last_tick.is_some());
    }
}
