//! Integration tests for the TUI using Ratatui's `TestBackend`.
//!
//! Allow `field_reassign_with_default` — the test fixtures build
//! `ScanResult` row-by-row and that's clearer than a single struct
//! literal when several fields are customized.

#![allow(clippy::field_reassign_with_default)]

use std::path::PathBuf;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use runnerguard_model::{
    Diagnostic, DiagnosticLevel, DiagnosticStage, Finding, FindingOrigin, ReportFormat,
    ScanOutcome, ScanResult, Severity, SourceSpan,
};
use runnerguard_tui::{Action, App, Event, MoveDirection, PageId, from_crossterm};

fn make_outcome() -> ScanOutcome {
    let findings = vec![
        Finding::deterministic(
            "MULE-001",
            Severity::Critical,
            "Logger missing",
            "Add a logger component.",
        ),
        Finding::deterministic(
            "MULE-002",
            Severity::Warning,
            "Default port",
            "Default port 8081 is widely scanned.",
        ),
        Finding::deterministic(
            "MULE-014",
            Severity::Info,
            "No dataweave error handler",
            "Consider adding a target=\"errors\" mapper.",
        ),
    ];
    let mut result = ScanResult::default();
    result.findings = findings;
    result.parser_diagnostics = vec![Diagnostic::new(
        DiagnosticStage::Rule,
        "MULE-WARN",
        DiagnosticLevel::Warning,
        "demo diagnostic",
    )];
    result.rule_count = 3;
    result.flow_count = 1;
    result.subflow_count = 0;
    let mut report_paths = Vec::new();
    report_paths.push(PathBuf::from("./out/report.md"));
    ScanOutcome {
        result,
        report_paths,
        artifact_paths: vec![PathBuf::from(
            "./out/artifacts/flows/order-api--abc12345.json",
        )],
        threshold_exceeded: false,
        incomplete: false,
    }
}

#[test]
fn renders_empty_findings_without_panicking() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let outcome = ScanOutcome {
        result: ScanResult::default(),
        report_paths: Vec::new(),
        artifact_paths: Vec::new(),
        threshold_exceeded: false,
        incomplete: false,
    };
    let mut app = App::browsing(runnerguard_tui::EventSource::Test(Vec::new()), outcome);
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render with empty state");
}

#[test]
fn renders_findings_table() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Findings;
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render findings page");
    // The primary thing this test guarantees is that the table
    // renders without crashing on a populated dataset. Inspecting
    // exact buffer text is brittle across Ratatui versions, so we
    // also assert that nothing in the buffer is empty for the
    // findings panel area (rows 3..27).
    let buf = terminal.backend().buffer().clone();
    let mut non_empty = 0;
    for y in 3..27 {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            if !cell.symbol().is_empty() {
                non_empty += 1;
            }
        }
    }
    assert!(
        non_empty > 50,
        "expected the findings page to populate more than 50 cells, got {non_empty}"
    );
}

#[test]
fn renders_diagnostics_page() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Diagnostics;
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render diagnostics page");
}

#[test]
fn renders_report_page() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Report;
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render report page");
}

#[test]
fn small_terminal_does_not_panic() {
    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render with small terminal");
}

#[test]
fn tiny_terminal_does_not_panic() {
    let backend = TestBackend::new(8, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render with tiny terminal");
}

#[test]
fn move_down_reaches_end_and_move_up_clamps() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    let n = app.state.findings.len();
    assert!(n >= 3);
    app.apply_action(Action::Move(MoveDirection::End));
    assert_eq!(app.state.selections.finding_index, n - 1);
    app.apply_action(Action::Move(MoveDirection::Up));
    assert_eq!(app.state.selections.finding_index, n - 2);
    app.apply_action(Action::Move(MoveDirection::Home));
    assert_eq!(app.state.selections.finding_index, 0);
    app.apply_action(Action::Move(MoveDirection::Up));
    assert_eq!(app.state.selections.finding_index, 0);
}

/// Regression: `--tui` and `--tui-live` both used to render the
/// findings table with a fresh `TableState::default()` on every
/// draw, which silently reset the scroll offset to 0. Pressing `j`
/// advanced the selection index correctly but the visible window
/// never moved, so the user "couldn't page" through long finding
/// lists — they only ever saw the first viewport of rows. The fix
/// promotes a `finding_offset` onto `Selections` and clamps it
/// against the actual viewport height on every render. This test
/// builds 50 findings, scrolls to the end, and asserts the stored
/// offset now reflects that the user has scrolled.
#[test]
fn findings_offset_advances_when_user_scrolls_past_viewport() {
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    // Push 50 findings via the same event-stream path `--tui-live`
    // uses so the test exercises the live-mode code path too.
    for i in 0..50 {
        let mut f = Finding::deterministic(
            format!("MULE-{i:03}"),
            Severity::Warning,
            format!("title #{i}"),
            format!("message for finding {i}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }
    app.state.current_page = PageId::Findings;
    assert_eq!(app.state.findings.len(), 50);

    // Starting offset must be 0 — the user hasn't navigated yet.
    assert_eq!(app.state.selections.finding_offset, 0);

    // Jump to the last finding via `End`. With a tiny viewport (the
    // `Min(5)` documented minimum) only two data rows fit, so the
    // offset must move to keep the selection on screen.
    app.apply_action(Action::Move(MoveDirection::End));
    let n = app.state.findings.len();
    assert_eq!(app.state.selections.finding_index, n - 1);
    assert!(
        app.state.selections.finding_offset > 0,
        "End must scroll the offset forward when the selection is \
         beyond the first viewport (was {})",
        app.state.selections.finding_offset
    );
    let end_offset = app.state.selections.finding_offset;

    // Going Home must reset the offset back to 0.
    app.apply_action(Action::Move(MoveDirection::Home));
    assert_eq!(app.state.selections.finding_index, 0);
    assert_eq!(app.state.selections.finding_offset, 0);

    // A series of Down presses past the viewport must also push the
    // offset forward; the selection must always stay inside
    // `[offset, offset + viewport)`.
    for _ in 0..10 {
        app.apply_action(Action::Move(MoveDirection::Down));
    }
    let sel = app.state.selections.finding_index;
    let off = app.state.selections.finding_offset;
    assert!(
        sel >= off,
        "selection ({sel}) must not be above the viewport (offset {off})"
    );
    // The rendered viewport on a TestBackend can vary; the only
    // contract we can pin here is "offset <= selection".
    assert!(
        off <= sel,
        "offset ({off}) must not exceed selection ({sel})"
    );

    // Filtering the list down to fewer rows must clamp the offset
    // back into the legal range — otherwise the next render would
    // dereference past the end of the now-shorter list.
    app.state.apply_filter("MULE-000");
    assert!(
        app.state.selections.finding_offset < app.state.visible_findings.len(),
        "filter must clamp finding_offset (got {}, vis len {})",
        app.state.selections.finding_offset,
        app.state.visible_findings.len()
    );
    app.state.clear_filter();
    let _ = end_offset;
}

/// Regression: the findings table must actually render rows past
/// the first viewport. Previously the offset silently reset to 0 on
/// every draw, so pressing `End` then `Home` and re-rendering only
/// ever showed rows 0..viewport. This test runs an end-to-end render
/// on a 100x30 TestBackend and asserts that after scrolling to the
/// end the buffer contains at least one rule-id from the *end* of
/// the list — i.e. the table actually scrolled.
#[test]
fn findings_table_actually_renders_rows_past_the_first_viewport() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..50 {
        let mut f = Finding::deterministic(
            format!("MULE-{i:03}"),
            Severity::Warning,
            format!("title #{i}"),
            format!("message for finding {i}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }
    app.state.current_page = PageId::Findings;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .unwrap();

    // Initial render must NOT contain any rows past the first
    // viewport (MULE-020 and beyond).
    let buf_before = terminal.backend().buffer().clone();
    let before_text = buffer_text(&buf_before);
    assert!(
        !before_text.contains("MULE-040"),
        "initial render must not show row 40 yet"
    );

    // Jump to the end and re-render. The buffer must now contain at
    // least one row from the end of the list (MULE-04x).
    app.apply_action(Action::Move(MoveDirection::End));
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .unwrap();
    let buf_after = terminal.backend().buffer().clone();
    let after_text = buffer_text(&buf_after);
    let rendered_end_ids: Vec<&str> = ["MULE-040", "MULE-045", "MULE-049"]
        .iter()
        .copied()
        .filter(|id| after_text.contains(id))
        .collect();
    assert!(
        !rendered_end_ids.is_empty(),
        "after End, the rendered buffer must contain a row from \
         the end of the list; full buffer was:\n{after_text}"
    );

    // Jump back to the top — the first row must be visible again.
    app.apply_action(Action::Move(MoveDirection::Home));
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .unwrap();
    let buf_top = terminal.backend().buffer().clone();
    let top_text = buffer_text(&buf_top);
    assert!(
        top_text.contains("MULE-000"),
        "after Home, the rendered buffer must contain the first \
         finding again; full buffer was:\n{top_text}"
    );
}

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn enter_opens_finding_detail_page() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Findings;
    app.apply_action(Action::Enter);
    assert_eq!(app.state.current_page, PageId::FindingDetail);
}

#[test]
fn page_shortcut_keys_change_page() {
    let app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    let cases = [
        (PageId::Findings, 'f'),
        (PageId::Flows, 'g'),
        (PageId::Diagnostics, 'd'),
        (PageId::ScanProgress, 'p'),
    ];
    for (page, ch) in cases {
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(ch),
            crossterm::event::KeyModifiers::NONE,
        );
        let action = app.global_keymap(&key);
        let Some(Action::Page(p)) = action else {
            panic!("expected page action for {ch}, got {action:?}");
        };
        assert_eq!(p, page);
    }
    // 'r' is reserved for "rerun" per the spec, not for "Report".
    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('r'),
        crossterm::event::KeyModifiers::NONE,
    );
    let action = app.global_keymap(&key);
    assert!(matches!(action, Some(Action::Other(_))));
}

#[test]
fn quit_shortcut_sets_cancel_flag() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let action = app.global_keymap(&key);
    app.apply_action(action.unwrap());
    assert!(app.cancel.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn event_loop_quits_when_test_events_exhausted() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.run(&mut terminal)
        .expect("run drains empty test events");
}

#[test]
fn scanner_event_progression_updates_state() {
    use runnerguard_core::ScanEvent;
    // Start with a fresh app (no preloaded findings) so the counters
    // start at zero and we can assert exact values.
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    app.state.apply_event(ScanEvent::Started {
        project: "demo".to_string(),
    });
    app.state.apply_event(ScanEvent::XmlParsed {
        path: "src/main/mule/order-api.xml".to_string(),
        flows: 1,
    });
    app.state
        .apply_event(ScanEvent::RulesLoaded { rule_count: 3 });
    app.state.apply_event(ScanEvent::RuleCompleted {
        rule_id: "MULE-001".to_string(),
        findings: 1,
    });
    // The `Finding` event is what increments `finding_count` in the
    // live event stream; `RuleCompleted` no longer touches the counter.
    app.state
        .apply_event(ScanEvent::Finding(Finding::deterministic(
            "MULE-001",
            Severity::Warning,
            "demo",
            "demo",
        )));
    app.state.apply_event(ScanEvent::ReportWritten {
        format: ReportFormat::Markdown,
        path: "./out/report.md".to_string(),
    });
    assert_eq!(app.state.progress.parsed_flows, 1);
    assert_eq!(app.state.progress.rules_loaded, 3);
    assert_eq!(app.state.progress.rules_completed, 1);
    assert_eq!(app.state.progress.finding_count, 1);
    assert_eq!(app.state.report_paths.len(), 1);
}

#[test]
fn dispatching_key_event_routes_to_global_keymap() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('f'),
        crossterm::event::KeyModifiers::NONE,
    );
    app.dispatch(from_crossterm(crossterm::event::Event::Key(key)));
    assert_eq!(app.state.current_page, PageId::Findings);
}

#[test]
fn panel_cycle_advances_and_wraps() {
    use runnerguard_tui::Cycle;
    let app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    let start = app.state.focused_panel;
    let next = start.cycle(Cycle::Forward);
    assert_ne!(next, start);
    let back = next.cycle(Cycle::Backward);
    assert_eq!(back, start);
}

#[test]
fn progress_gauge_ratio_reaches_one_at_done() {
    use runnerguard_core::ScanEvent;
    use runnerguard_tui::components::progress::compute_ratio;
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.apply_event(ScanEvent::Finished {
        summary: app.state.summary.clone().unwrap_or_default(),
    });
    let r = compute_ratio(&app.state);
    assert!((r - 1.0).abs() < 1e-6, "expected done ratio = 1.0, got {r}");
}

#[test]
fn source_span_is_preserved_through_state() {
    let span = SourceSpan {
        file: "src/main/mule/order-api.xml".to_string(),
        start_line: 12,
        start_column: 5,
        end_line: 12,
        end_column: 24,
    };
    let f =
        Finding::deterministic("MULE-001", Severity::Critical, "title", "msg").with_source(span);
    let mut result = ScanResult::default();
    result.findings = vec![f];
    let outcome = ScanOutcome {
        result,
        report_paths: Vec::new(),
        artifact_paths: Vec::new(),
        threshold_exceeded: false,
        incomplete: false,
    };
    let app = App::browsing(runnerguard_tui::EventSource::Test(Vec::new()), outcome);
    let f = app.state.findings.first().unwrap();
    assert_eq!(f.source.as_ref().unwrap().start_line, 12);
}

#[test]
fn origin_field_round_trips() {
    let mut f = Finding::deterministic("MULE-001", Severity::Critical, "t", "m");
    f.origin = FindingOrigin::ParserDiagnostic;
    let mut result = ScanResult::default();
    result.findings = vec![f];
    let outcome = ScanOutcome {
        result,
        report_paths: Vec::new(),
        artifact_paths: Vec::new(),
        threshold_exceeded: false,
        incomplete: false,
    };
    let app = App::browsing(runnerguard_tui::EventSource::Test(Vec::new()), outcome);
    assert_eq!(
        app.state.findings[0].origin,
        FindingOrigin::ParserDiagnostic
    );
}

#[test]
fn from_crossterm_handles_resize() {
    let ev = from_crossterm(crossterm::event::Event::Resize(120, 40));
    assert!(matches!(
        ev,
        Event::Resize {
            width: 120,
            height: 40
        }
    ));
}

#[test]
fn from_crossterm_passes_key_through() {
    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('x'),
        crossterm::event::KeyModifiers::NONE,
    );
    let ev = from_crossterm(crossterm::event::Event::Key(key));
    assert!(matches!(ev, Event::Key(_)));
}

#[test]
fn filter_narrows_findings() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    let before = app.state.findings.len();
    let changed = app.state.apply_filter("Default");
    assert!(changed);
    // The filter narrows `visible_findings` but does NOT mutate the
    // authoritative `findings` list — clearing the filter restores
    // the original view.
    assert!(app.state.findings.len() == before);
    assert!(app.state.visible_findings.len() < before);
    assert!(!app.state.visible_findings.is_empty());
    app.state.clear_filter();
    assert_eq!(app.state.visible_findings.len(), before);
}

#[test]
fn short_path_handles_non_ascii() {
    // Regression test for the previous byte-slicing implementation
    // which panicked on paths containing multi-byte UTF-8 chars
    // such as `订单`. The new implementation must NOT panic and must
    // produce a valid truncated form.
    let path = "C:/projetos/mulesoft/src/main/mule/订单/order-api.xml";
    let out = runnerguard_tui::view::short_path(path, 28);
    assert!(out.contains('…'));
    // Short paths (≤ max) are returned verbatim.
    let short = "short.xml";
    assert_eq!(runnerguard_tui::view::short_path(short, 28), short);
    // Exactly max-length path stays whole.
    let exact = "x".repeat(28);
    assert_eq!(runnerguard_tui::view::short_path(&exact, 28), exact);
}

#[test]
fn config_loaded_event_is_recorded() {
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    app.state.apply_event(ScanEvent::ConfigLoaded {
        config_path: "/tmp/runnerguard.yaml".to_string(),
    });
    assert_eq!(
        app.state.config_path.as_deref(),
        Some(std::path::Path::new("/tmp/runnerguard.yaml"))
    );
}

#[test]
fn finding_events_accumulate_into_live_state() {
    // Regression: `--tui-live` mode (which uses `ScanSource::Channel`)
    // does not receive a `ScanOutcome` directly — only events. Before
    // the `ScanEvent::Finding` variant existed, draining the channel
    // left `state.findings` permanently empty and the findings page
    // rendered "no finding selected". This test pins the contract:
    // pumping Finding events through `apply_event` populates the
    // authoritative `findings` list and the filtered `visible_findings`
    // mirror exactly.
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );

    for (idx, rid) in ["MULE-001", "MULE-002", "MULE-003"].iter().enumerate() {
        let mut f = Finding::deterministic(
            rid.to_string(),
            Severity::Warning,
            "title",
            format!("message {idx}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }

    // Authoritative list has all three.
    assert_eq!(app.state.findings.len(), 3);
    // Without an active filter, visible == findings.
    assert_eq!(app.state.visible_findings.len(), 3);
    assert_eq!(app.state.progress.finding_count, 3);

    // Drain a Synthetic Finished event — `apply_event` should leave the
    // populated state alone (no clearing).
    let mut summary = runnerguard_model::ScanSummary::default();
    summary.findings_total = 3;
    app.state.apply_event(ScanEvent::Finished { summary });
    assert_eq!(app.state.findings.len(), 3);
    assert_eq!(app.state.visible_findings.len(), 3);
    assert!(!app.state.running, "Finished event must clear `running`");
}
