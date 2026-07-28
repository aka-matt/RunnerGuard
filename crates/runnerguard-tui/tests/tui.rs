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

/// Regression: on the Findings *table* page, `PageUp` / `PageDown`
/// step by 8 rows at a time. The Findings list grows large enough
/// that ±1 navigation is tedious, so the keys behave like a
/// screenful jump — same as the very first incarnation of the TUI.
/// On every *other* body page (FindingDetail, Flows, Diagnostics,
/// Report) the same keys step by 1 issue because the user is
/// reading one item at a time and "上一条 / 下一条" is the only
/// useful semantics. This test pins the Findings page contract.
#[test]
fn findings_page_page_keys_step_by_eight_rows() {
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..30 {
        let mut f = Finding::deterministic(
            format!("MULE-{i:03}"),
            Severity::Warning,
            format!("title #{i}"),
            format!("message {i}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }
    app.state.current_page = PageId::Findings;
    let n = app.state.findings.len();
    assert_eq!(n, 30);

    // From the top, one PageDown must land on row 8 — not 1 (Down)
    // and not 19 / 24 (the old "no-op alias" experiment). The 8-row
    // step is the contract.
    app.apply_action(Action::Move(MoveDirection::Home));
    assert_eq!(app.state.selections.finding_index, 0);
    app.apply_action(Action::Move(MoveDirection::PageDown));
    assert_eq!(
        app.state.selections.finding_index, 8,
        "Findings page: PageDown must step 8 rows (got {})",
        app.state.selections.finding_index
    );

    // Two more presses advance by another 16 — landing on row 24.
    app.apply_action(Action::Move(MoveDirection::PageDown));
    app.apply_action(Action::Move(MoveDirection::PageDown));
    assert_eq!(
        app.state.selections.finding_index, 24,
        "Findings page: each PageDown must add 8 (got {})",
        app.state.selections.finding_index
    );

    // PageDown above the list end clamps to n - 1.
    app.apply_action(Action::Move(MoveDirection::PageDown));
    assert_eq!(
        app.state.selections.finding_index,
        n - 1,
        "Findings page: PageDown must clamp at the bottom"
    );

    // PageUp mirrors: from the clamped `n - 1` (= 29) row PageUp
    // lands on 21 (= 29 - 8). The earlier count of "16" was a
    // misread of the sequence — the third PageDown above had
    // already clamped to the bottom, so the upward step starts
    // from there, not from row 24.
    app.apply_action(Action::Move(MoveDirection::PageUp));
    assert_eq!(
        app.state.selections.finding_index, 21,
        "Findings page: PageUp must step 8 rows back (got {})",
        app.state.selections.finding_index
    );

    // PageUp clamps at 0 — the user can spam the key without
    // wrapping or panicking.
    app.apply_action(Action::Move(MoveDirection::Home));
    for _ in 0..3 {
        app.apply_action(Action::Move(MoveDirection::PageUp));
    }
    assert_eq!(
        app.state.selections.finding_index, 0,
        "Findings page: PageUp must clamp at the top"
    );
}

/// Regression: crossterm 0.28 fires both `KeyEventKind::Press` and
/// `KeyEventKind::Release` for a single physical key tap when the
/// host enables keyboard-enhancement mode (Windows Terminal, most
/// ConPTY-based terminals, xterm with `modifyOtherKeys`, etc.).
/// The previous `from_crossterm` forwarded both events into the
/// dispatcher unchanged, so the user's press of `j` on the
/// FindingDetail page advanced `finding_index` by 2 issues — the
/// `Release` did an extra `Action::Move(Down)` after the `Press`
/// already moved it. This test pins the contract: a Press + Release
/// pair on the same key must advance the index by exactly 1 issue.
#[test]
fn key_release_event_does_not_advance_finding_index_a_second_time() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Findings;
    app.apply_action(Action::Enter);
    assert_eq!(app.state.current_page, PageId::FindingDetail);
    assert_eq!(app.state.selections.finding_index, 0);

    let press =
        KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Press);
    app.dispatch(from_crossterm(crossterm::event::Event::Key(press)));
    let after_press = app.state.selections.finding_index;
    assert_eq!(
        after_press, 1,
        "Press of `j` must advance by exactly 1 (got {after_press})"
    );

    let release = KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    app.dispatch(from_crossterm(crossterm::event::Event::Key(release)));
    let after_release = app.state.selections.finding_index;
    assert_eq!(
        after_release, 1,
        "Release of `j` must NOT advance a second time (got {after_release})"
    );

    // Belt-and-braces: a third physical tap (Press + Release) on
    // the same key must advance by exactly 1 more issue, not 2.
    let press =
        KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Press);
    app.dispatch(from_crossterm(crossterm::event::Event::Key(press)));
    let release = KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    app.dispatch(from_crossterm(crossterm::event::Event::Key(release)));
    assert_eq!(
        app.state.selections.finding_index, 2,
        "one more press+release pair must advance by exactly 1 (got {})",
        app.state.selections.finding_index
    );
}

/// Regression: on the FindingDetail page, `PageUp` / `PageDown`
/// step by exactly one issue — "上一条 / 下一条 issue". This is the
/// page-aware counterpart to `findings_page_page_keys_step_by_eight_rows`.
/// Up / Down behave identically (±1); they're here for symmetry.
#[test]
fn finding_detail_page_page_keys_step_by_one_issue() {
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..20 {
        let mut f = Finding::deterministic(
            format!("MULE-{i:03}"),
            Severity::Warning,
            format!("title #{i}"),
            format!("message {i}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }
    // Reach the detail page via the same `Enter` key the user
    // presses on the Findings page; this also exercises the focus-
    // reset path.
    app.state.current_page = PageId::Findings;
    app.apply_action(Action::Enter);
    assert_eq!(app.state.current_page, PageId::FindingDetail);
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);
    let n = app.state.findings.len();
    assert_eq!(n, 20);

    app.apply_action(Action::Move(MoveDirection::Home));
    assert_eq!(app.state.selections.finding_index, 0);

    // PageDown on the detail page must move ±1, not ±8 — pressing
    // it three times from row 0 must land on row 3, never row 24.
    app.apply_action(Action::Move(MoveDirection::PageDown));
    app.apply_action(Action::Move(MoveDirection::PageDown));
    app.apply_action(Action::Move(MoveDirection::PageDown));
    assert_eq!(
        app.state.selections.finding_index, 3,
        "FindingDetail: PageDown must step 1 row (got {})",
        app.state.selections.finding_index
    );

    // Re-rendering the detail page must show the new finding's
    // title — proving the page is reading `highlighted_finding()`
    // off the updated selection rather than a snapshot.
    let backend = ratatui::backend::TestBackend::new(120, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let body_text = buffer_text(&buf);
    assert!(
        body_text.contains("MULE-003"),
        "after PageDown x3 on the detail page, MULE-003 must be the \
         highlighted finding; full buffer was:\n{body_text}"
    );

    // Up mirrors Down.
    app.apply_action(Action::Move(MoveDirection::PageUp));
    app.apply_action(Action::Move(MoveDirection::PageUp));
    assert_eq!(
        app.state.selections.finding_index, 1,
        "FindingDetail: PageUp must step 1 row back (got {})",
        app.state.selections.finding_index
    );

    // Both endpoints clamp.
    app.apply_action(Action::Move(MoveDirection::Home));
    for _ in 0..5 {
        app.apply_action(Action::Move(MoveDirection::PageUp));
    }
    assert_eq!(
        app.state.selections.finding_index, 0,
        "FindingDetail: PageUp must clamp at the top"
    );
    app.apply_action(Action::Move(MoveDirection::End));
    for _ in 0..5 {
        app.apply_action(Action::Move(MoveDirection::PageDown));
    }
    assert_eq!(
        app.state.selections.finding_index,
        n - 1,
        "FindingDetail: PageDown must clamp at the bottom"
    );

    // Key parity: PageUp/Down and Up/Down behave identically on
    // the detail page (a step is a step is a step).
    app.apply_action(Action::Move(MoveDirection::Home));
    app.apply_action(Action::Move(MoveDirection::Up));
    let up_step = app.state.selections.finding_index;
    app.apply_action(Action::Move(MoveDirection::Home));
    app.apply_action(Action::Move(MoveDirection::PageUp));
    let paged_step = app.state.selections.finding_index;
    assert_eq!(
        up_step, paged_step,
        "FindingDetail: PageUp must equal Up (got {up_step} vs {paged_step})"
    );
}

/// Regression: after cycling focus to the Header or Footer with
/// `Tab`, the body block title must lose its focused highlight so
/// exactly one panel (the actually-focused one) lights up at any
/// time. Previously `render_body` hardcoded `focused = true` for
/// every page render, so the Findings / Flows / Diagnostics block
/// kept its cyan background regardless of where the focus actually
/// was — the user saw two title bars highlighted at once and
/// pressed `j`, only to discover the keys had been routed to the
/// static Header / Footer.
#[test]
fn tab_cycles_focus_and_only_one_panel_highlights_at_a_time() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Color;
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Findings;

    // Walk every panel in turn. The default starting panel is
    // `Body`; one `Tab` press cycles to `Footer`, then `Header`,
    // then wraps back to `Body`, then `Footer` again.
    let cycle = [
        runnerguard_tui::PanelId::Footer,
        runnerguard_tui::PanelId::Header,
        runnerguard_tui::PanelId::Body,
        runnerguard_tui::PanelId::Footer,
    ];
    for expected_panel in cycle {
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.dispatch(from_crossterm(crossterm::event::Event::Key(key)));
        assert_eq!(
            app.state.focused_panel, expected_panel,
            "Tab must cycle panels in order"
        );

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| app.render(frame, frame.area()))
            .expect("render must not panic while cycling focus");

        // Count the cells whose background is the focused cyan.
        // Exactly one block title per frame should light up cyan;
        // earlier code lit the body title *every* frame, so the
        // count was always ≥ 2 for the Findings page.
        let buf = terminal.backend().buffer().clone();
        let mut focused_cells = 0usize;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                if cell.style().bg == Some(Color::Cyan) {
                    focused_cells += 1;
                }
            }
        }
        // The block title is rendered as a single styled line; allow
        // a small slack for any other cyan accents (e.g. the
        // header's "RunnerGuard" span on the same row) but the
        // property we care about is: the *body* block must NOT
        // contribute additional focused cells when focus has moved
        // away.
        assert!(
            focused_cells > 0,
            "at least one focused cell must exist (panel = {expected_panel:?})"
        );
        // Findings block title lives on row 3 (header height 3).
        // When Body is NOT focused, that row must not carry a
        // cyan-background title.
        if expected_panel != runnerguard_tui::PanelId::Body {
            let findings_title_cell = &buf[(0, 3)];
            assert_ne!(
                findings_title_cell.style().bg,
                Some(Color::Cyan),
                "Findings block title must NOT be cyan when focus is \
                 on {expected_panel:?}; this is the double-highlight bug."
            );
        }
    }
}

/// Regression: with focus on Header or Footer, the body keymap
/// must not move the finding selection. The bug looked like "j /
/// k / PageDown don't work after pressing Tab" — the user-visible
/// cue was the still-focused-looking Findings block, the actual
/// cause was that Tab had rerouted the keys to a no-op handler.
#[test]
fn keys_routed_to_header_or_footer_do_not_move_finding_selection() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    app.state.current_page = PageId::Findings;
    let baseline = app.state.selections.finding_index;

    // Cycle once: Body → Footer.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::NONE,
    ))));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Footer);
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
    ))));
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::PageDown,
        KeyModifiers::NONE,
    ))));
    assert_eq!(
        app.state.selections.finding_index, baseline,
        "j / PageDown on Footer must NOT advance the finding selection"
    );

    // Cycle once more: Footer → Header.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::NONE,
    ))));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Header);
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Char('k'),
        KeyModifiers::NONE,
    ))));
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::PageUp,
        KeyModifiers::NONE,
    ))));
    assert_eq!(
        app.state.selections.finding_index, baseline,
        "k / PageUp on Header must NOT advance the finding selection"
    );

    // Cycle back: Header → Body. `j` must now move the cursor — this
    // is the half the user actually wanted.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Tab,
        KeyModifiers::NONE,
    ))));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);
    app.dispatch(from_crossterm(crossterm::event::Event::Key(KeyEvent::new(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
    ))));
    assert_eq!(
        app.state.selections.finding_index,
        baseline + 1,
        "j on Body must advance the finding selection"
    );
}

/// Regression: pressing `f` / `g` / `d` (and `Enter` on the
/// Findings page) navigates to a new page — focus must land on the
/// Body panel so the user can immediately use `j`/`k`/`PageDown`.
/// Previously the page change left `focused_panel` wherever it was,
/// so a leftover Header / Footer focus silently disabled the body
/// keymap.
#[test]
fn page_shortcut_resets_focus_to_body() {
    let mut app = App::browsing(
        runnerguard_tui::EventSource::Test(Vec::new()),
        make_outcome(),
    );
    // Park focus somewhere inconvenient, then navigate.
    app.state.focused_panel = runnerguard_tui::PanelId::Header;
    app.apply_action(Action::Page(PageId::Findings));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);

    app.state.focused_panel = runnerguard_tui::PanelId::Footer;
    app.apply_action(Action::Page(PageId::Flows));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);

    app.state.focused_panel = runnerguard_tui::PanelId::Header;
    app.apply_action(Action::Page(PageId::Diagnostics));
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);

    // Enter from Findings opens FindingDetail and also lands on Body.
    app.state.current_page = PageId::Findings;
    app.state.focused_panel = runnerguard_tui::PanelId::Footer;
    app.apply_action(Action::Enter);
    assert_eq!(app.state.current_page, PageId::FindingDetail);
    assert_eq!(app.state.focused_panel, runnerguard_tui::PanelId::Body);
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

/// Regression: the `>>` indicator on the Findings page must be able
/// to land on **more than one** visible row, not stay pinned to a
/// fixed slot while the data scrolls past it.
///
/// The previous implementation of `scroll_findings_offset_into_view`
/// approximated the viewport with the page-layout minimum (2 data
/// rows). That made `offset = sel + 1 - viewport` always yield
/// `sel - offset = 1`, so the highlight was glued to the second
/// visible row forever — pressing `j` did advance the selection but
/// the visible rows simply slid past underneath the same highlight.
/// The user observed: "the selection indicator `>>` can only point
/// to the first and second item; it's the list that's scrolling".
///
/// Contract for this test: walking the first 8 selections (rows
/// 0..=7) must visit all 8 distinct visible slots. With the old
/// buggy `viewport = 2` formula the only slots the user ever saw
/// were `0` (start) and `1` (after the first press); with the fix
/// the highlight reaches every visible row in the first viewport.
#[test]
fn findings_highlight_escapes_the_second_row_slot() {
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    // 50 findings — long enough that scrolling has to happen well
    // before the user reaches the end.
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

    // Sanity check: the first Down advances the selection by one
    // and leaves the highlight at visible position 1 — i.e. the
    // SAME slot the user complains about. The next six presses
    // must move it past that slot, proving the bug is gone.
    app.apply_action(Action::Move(MoveDirection::Home));
    assert_eq!(app.state.selections.finding_index, 0);
    assert_eq!(app.state.selections.finding_offset, 0);

    let mut visited = std::collections::BTreeSet::new();
    // Record the visible position before AND after each Down so the
    // 8 selection states (rows 0..=7) are all represented.
    visited.insert(app.state.selections.finding_index - app.state.selections.finding_offset);
    for _ in 0..7 {
        app.apply_action(Action::Move(MoveDirection::Down));
        let sel = app.state.selections.finding_index;
        let off = app.state.selections.finding_offset;
        assert!(
            sel >= off,
            "selection ({sel}) must never be above the viewport (offset {off})",
        );
        visited.insert(sel - off);
    }
    assert_eq!(
        visited.len(),
        8,
        "highlight must visit 8 distinct visible slots in the first \
         viewport, visited {visited:?}",
    );
    // The buggy viewport=2 formula only ever let the highlight sit
    // on visible slot 0 (start) or 1 (after one press). The fix
    // must let it reach slot 7 — i.e. the last visible row.
    assert!(
        visited.contains(&7),
        "highlight must reach the last visible slot (7) of the \
         first viewport; visited {visited:?}",
    );
    // And it must NOT collapse to a single slot — that would
    // reproduce the user's "only first and second item" complaint.
    assert!(
        visited.len() > 2,
        "highlight must visit more than 2 slots; visited {visited:?}",
    );
}

/// Regression: on a **tall** terminal the findings table can show
/// many more rows than the PageDown step (8). The first attempt at
/// fixing the "highlight stuck at row 8" bug used
/// `FINDINGS_VIRTUAL_VIEWPORT = 8` everywhere, which fixed the
/// small-terminal case but stopped the highlight at row 8 on
/// tall terminals — the user observed "在 `findings` 没有到底之前
/// `>>` 只能最多指到第8行". The current fix caches the real
/// `area.height` after each render and uses it in the scroll
/// formula, so the highlight walks through every visible row.
///
/// This test renders the app onto a 120×40 TestBackend (enough
/// vertical room for ~23 data rows in the findings table), drives a
/// sequence of Down presses, and asserts the highlight can reach
/// row 15 — a position that's impossible if the formula is still
/// hardcoded to viewport=8.
#[test]
fn findings_highlight_walks_every_visible_row_on_a_tall_terminal() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use runnerguard_core::ScanEvent;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..60 {
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

    // First render — populates `AppState::findings_viewport_rows`
    // from the actual layout chunk. With 120×40 the body splits
    // 3/34/3 (header/body/footer), then Findings splits 26/8, so
    // the table gets 26 rows of which 23 are visible data rows.
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("initial render");
    let cached_viewport = app.state.findings_viewport_rows;
    assert!(
        cached_viewport > 8,
        "a 120x40 terminal must expose more than 8 data rows; got \
         {cached_viewport}",
    );

    // Walk past the old viewport=8 threshold. With the bug the
    // highlight would freeze at row 8; with the fix it should keep
    // moving down through every row in the cached viewport.
    let target_row = cached_viewport.saturating_sub(1);
    app.apply_action(Action::Move(MoveDirection::Home));
    for _ in 0..target_row {
        app.apply_action(Action::Move(MoveDirection::Down));
    }
    let sel = app.state.selections.finding_index;
    let off = app.state.selections.finding_offset;
    assert_eq!(
        sel, target_row,
        "Down presses must advance selection by one each time",
    );
    assert_eq!(
        sel - off,
        target_row,
        "highlight must sit on row {target_row} (sel - offset); \
         with the viewport=8 bug it would be stuck at 7",
    );
    assert!(
        target_row >= 15,
        "this test only catches the tall-terminal bug when the \
         actual viewport is at least 15 rows; got {target_row}",
    );
}

/// Regression for the `/` filter affordance. Until this change,
/// `Action::Other("filter")` in `App::apply_other` was a clear-only
/// toggle / no-op: pressing `/` on the Findings page either cleared
/// an existing filter or did nothing visible, with no input mode,
/// no prompt, and no way to actually type a pattern. The fix wires
/// a real input mode that captures characters, applies the pattern
/// on Enter, and cancels cleanly on Esc.
#[test]
fn filter_input_mode_captures_types_and_commits() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..20 {
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
    let pre_filter_visible = app.state.visible_findings.len();
    assert_eq!(pre_filter_visible, 20);

    // Pressing `/` on Findings must enter filter mode and pre-fill
    // the draft with the (currently empty) filter pattern.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(
        app.state.filter_mode,
        "/ must enter filter mode on Findings"
    );
    assert_eq!(
        app.state.filter_input, "",
        "draft must start empty when no filter is active",
    );

    // Each character press appends to the draft. We use the
    // dispatcher end-to-end so the test pins the public contract
    // rather than poking `apply_action` directly.
    for ch in "MULE-001".chars() {
        app.dispatch(from_crossterm(crossterm::event::Event::Key(
            KeyEvent::new_with_kind(KeyCode::Char(ch), KeyModifiers::NONE, KeyEventKind::Press),
        )));
    }
    assert_eq!(app.state.filter_input, "MULE-001");
    // The applied filter must still be `None` until Enter commits.
    assert!(
        app.state.filter.is_none(),
        "Enter alone must apply the filter"
    );
    assert_eq!(
        app.state.visible_findings.len(),
        pre_filter_visible,
        "visible_findings must not narrow until Enter",
    );

    // Backspace pops one character.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert_eq!(app.state.filter_input, "MULE-00");

    // Enter commits and exits filter mode.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(!app.state.filter_mode, "Enter must exit filter mode");
    assert!(app.state.filter_input.is_empty());
    let applied = app.state.filter.as_deref().unwrap_or("");
    assert_eq!(applied, "MULE-00");
    // The filtered view should be smaller than the original list —
    // `MULE-00*` matches exactly MULE-000 and MULE-001..MULE-009
    // (ten findings), so visible_findings.len() must be < 20.
    assert!(
        app.state.visible_findings.len() < pre_filter_visible,
        "applying MULE-00 must narrow visible_findings (was {}, now {})",
        pre_filter_visible,
        app.state.visible_findings.len(),
    );
}

/// `Esc` while editing the filter must cancel without touching the
/// currently-applied filter. This guards against the previous
/// "clear-only toggle" behaviour, which lost the existing filter
/// when the user pressed `/` then `Esc`.
#[test]
fn filter_input_esc_keeps_existing_filter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..10 {
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
    // Seed a filter directly via the public state API.
    app.state.apply_filter("MULE-00");
    let pre_filter = app.state.filter.clone();
    let pre_visible = app.state.visible_findings.len();
    assert_eq!(pre_filter.as_deref(), Some("MULE-00"));

    // Enter filter mode — draft must be pre-filled with the current
    // filter so the user can edit in place.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(app.state.filter_mode);
    assert_eq!(
        app.state.filter_input, "MULE-00",
        "entering filter mode must pre-fill the draft with the active filter",
    );

    // Type something the user might change their mind about.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('X'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert_eq!(app.state.filter_input, "MULE-00X");

    // Esc cancels: filter mode off, draft cleared, but the *applied*
    // filter must still be "MULE-00" so the user's view is preserved.
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(!app.state.filter_mode);
    assert!(app.state.filter_input.is_empty());
    assert_eq!(app.state.filter.as_deref(), Some("MULE-00"));
    assert_eq!(app.state.visible_findings.len(), pre_visible);
}

/// Entering filter mode and committing an empty pattern must clear
/// any existing filter. This makes `Enter` on an empty draft a
/// shortcut for "show me everything again" without needing Esc +
/// then a separate `/` toggle.
#[test]
fn filter_input_empty_enter_clears_filter() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..10 {
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
    app.state.apply_filter("MULE-00");
    assert!(app.state.filter.is_some());
    let total = app.state.findings.len();

    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(app.state.filter_mode);
    // Backspace enough times to clear the pre-filled draft.
    for _ in 0.."MULE-00".len() {
        app.dispatch(from_crossterm(crossterm::event::Event::Key(
            KeyEvent::new_with_kind(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Press),
        )));
    }
    assert!(app.state.filter_input.is_empty());

    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(
        app.state.filter.is_none(),
        "empty Enter must clear the filter"
    );
    assert_eq!(
        app.state.visible_findings.len(),
        total,
        "clearing the filter must restore the full list",
    );
}

/// `/` on a non-Findings page must be ignored — the user would
/// have nothing to filter there, and entering filter mode without
/// a visible list is a dead end.
#[test]
fn filter_input_slash_only_works_on_findings_page() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..5 {
        let mut f = Finding::deterministic(
            format!("MULE-{i:03}"),
            Severity::Warning,
            format!("title #{i}"),
            format!("message for finding {i}"),
        );
        f.origin = FindingOrigin::DeterministicRule;
        app.state.apply_event(ScanEvent::Finding(f));
    }
    app.state.current_page = PageId::Diagnostics;
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(
        !app.state.filter_mode,
        "`/` on Diagnostics must not enter filter mode",
    );
    // Sanity: it works on Findings.
    app.state.current_page = PageId::Findings;
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(
        app.state.filter_mode,
        "`/` on Findings must enter filter mode",
    );
}

/// After exiting filter mode, `j` / `k` navigation must work again
/// — otherwise the user could type a filter and then be stuck with
/// no way to move through the resulting list.
#[test]
fn filter_input_j_k_navigation_works_after_exit() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use runnerguard_core::ScanEvent;
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..10 {
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
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('/'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert!(!app.state.filter_mode);
    let before = app.state.selections.finding_index;
    app.dispatch(from_crossterm(crossterm::event::Event::Key(
        KeyEvent::new_with_kind(KeyCode::Char('j'), KeyModifiers::NONE, KeyEventKind::Press),
    )));
    assert_eq!(
        app.state.selections.finding_index,
        before + 1,
        "j must move the selection again once filter mode is exited",
    );
}

/// The footer must render the filter prompt (not the static help
/// text) while filter mode is on. This guards against future
/// refactors that accidentally let the default footer leak through.
#[test]
fn footer_renders_filter_prompt_in_filter_mode() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use runnerguard_core::ScanEvent;
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(
        runnerguard_tui::EventSource::Test(Vec::new()),
        runnerguard_tui::ScanSource::None,
    );
    for i in 0..5 {
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
    app.state.filter_mode = true;
    app.state.filter_input = "MULE".to_string();
    terminal
        .draw(|frame| app.render(frame, frame.area()))
        .expect("render with filter mode on");
    let buf = terminal.backend().buffer().clone();
    // The footer area is the last 3 rows (outer layout).
    let footer_y = buf.area.height.saturating_sub(2);
    let line_text: String = (0..buf.area.width)
        .map(|x| buf[(x, footer_y)].symbol().to_string())
        .collect();
    assert!(
        line_text.contains("Filter:"),
        "footer must show the Filter prompt in filter mode, got: {line_text:?}",
    );
    assert!(
        line_text.contains("MULE_"),
        "footer prompt must include the draft + caret, got: {line_text:?}",
    );
}
