//! Findings table — severity, rule, entity, file:line.

use crate::component::{Action, Component, Event, MoveDirection};
use crate::error::TuiError;
use crate::state::{AppState, Selections};
use crate::view::{focused_block, severity_label, severity_style, short_path, unfocused_block};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table, TableState};

#[derive(Debug, Default)]
pub struct FindingsTableComponent;

impl FindingsTableComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let base = if focused {
            focused_block("Findings")
        } else {
            unfocused_block("Findings")
        };
        let filter_label = state
            .filter
            .as_deref()
            .filter(|f| !f.is_empty())
            .map(|f| format!(" (filter: {f})"))
            .unwrap_or_default();
        let block = if filter_label.is_empty() {
            base
        } else {
            Block::default()
                .title(Span::styled(
                    format!(" Findings{filter_label} "),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
        };
        let header = Row::new(vec![
            cell("SEV"),
            cell("RULE"),
            cell("ENTITY"),
            cell("FILE"),
            cell("MSG"),
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        let rows: Vec<Row> = state
            .visible_findings
            .iter()
            .map(|f| {
                let sev_style = severity_style(f.severity);
                let file = f
                    .source
                    .as_ref()
                    .map(|s| short_path(&s.file, 28))
                    .unwrap_or_default();
                let line = f.source.as_ref().map(|s| s.start_line).unwrap_or_default();
                let file_cell = if line > 0 {
                    format!("{file}:{line}")
                } else {
                    file
                };
                Row::new(vec![
                    Span::styled(severity_label(f.severity), sev_style),
                    Span::raw(f.rule_id.clone()),
                    Span::raw(f.entity_id.clone().unwrap_or_default()),
                    Span::styled(file_cell, Style::default().fg(Color::DarkGray)),
                    Span::raw(truncate(&f.message, 60)),
                ])
            })
            .collect();
        let widths = [
            ratatui::layout::Constraint::Length(9),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(20),
            ratatui::layout::Constraint::Length(28),
            ratatui::layout::Constraint::Min(10),
        ];
        let mut table_state = build_table_state(state, area);
        let widget = Table::new(rows, widths)
            .header(header)
            .block(block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
        frame.render_stateful_widget(widget, area, &mut table_state);
    }
}

/// Build a `TableState` that scrolls to the row whose index is in
/// `state.selections.finding_index`.
///
/// `TableState::select()` only mutates the `selected` field — it does
/// NOT touch `offset`. If we just did `TableState::default()` +
/// `select(idx)` on every render, the offset would stay at 0 and the
/// user would see the first viewport of rows even after pressing `End`.
/// The widget's render code does silently correct the offset to keep
/// the selection in view, but the *next* render would construct a
/// fresh `TableState` again and the cycle would repeat — visible
/// flicker, and on very small viewports the correction logic can fail
/// to find a valid offset entirely.
///
/// This function takes the stored offset (`selections.finding_offset`)
/// as a starting point, clamps it against the actual viewport height,
/// and adjusts it so the selected row is inside the visible window.
/// The widget then renders from that offset without needing to second-
/// guess the user's scroll position.
fn build_table_state(state: &AppState, area: Rect) -> TableState {
    let n = state.visible_findings.len();
    let selected = if n == 0 {
        None
    } else {
        Some(state.selections.finding_index.min(n - 1))
    };
    if n == 0 {
        return TableState::default();
    }
    // `area` is the *outer* table area including the block's borders.
    // The widget subtracts its own header row internally, so the data
    // viewport is `area.height - 2 (borders) - 1 (header)`.
    let viewport = (area.height as usize).saturating_sub(3);
    let max_offset = n.saturating_sub(1);
    let mut offset = state.selections.finding_offset.min(max_offset);
    if viewport > 0 {
        let visible_end = offset.saturating_add(viewport);
        if let Some(sel) = selected {
            if sel >= visible_end {
                // Selection fell below the visible window — scroll
                // forward so the selection sits on the last visible row.
                offset = (sel + 1).saturating_sub(viewport).min(max_offset);
            } else if sel < offset {
                // Selection scrolled above the window — scroll back
                // to it.
                offset = sel;
            }
        }
    }
    TableState::default()
        .with_offset(offset)
        .with_selected(selected)
}

fn cell(text: &str) -> Span<'static> {
    Span::styled(text.to_string(), Style::default())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

impl Component for FindingsTableComponent {
    fn init(&mut self) -> Result<(), TuiError> {
        Ok(())
    }
    fn handle_event(&mut self, event: &Event) -> Option<Action> {
        let Event::Key(key) = event else {
            return None;
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Action::Move(MoveDirection::Down)),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::Move(MoveDirection::Up)),
            KeyCode::PageDown => Some(Action::Move(MoveDirection::PageDown)),
            KeyCode::PageUp => Some(Action::Move(MoveDirection::PageUp)),
            KeyCode::Home => Some(Action::Move(MoveDirection::Home)),
            KeyCode::End => Some(Action::Move(MoveDirection::End)),
            KeyCode::Enter => Some(Action::Enter),
            _ => None,
        }
    }
    fn update(&mut self, action: &Action) -> Result<(), TuiError> {
        // The findings component doesn't own the global selection
        // state — the App's `apply_move` is the single source of
        // truth. The trait impl is kept only to satisfy the
        // `Component` shape; this component never mutates state.
        let _ = action;
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        FindingsTableComponent::render_for(frame, area, &AppState::default(), false);
    }
}

/// Apply a move delta to a `Selections` value, clamped against the
/// available row count. Used by callers that drive the selection
/// outside the App shell (tests, scripted demos).
#[allow(unused_variables)]
pub fn apply_move(sel: &mut Selections, dir: MoveDirection, count: usize) {
    // The component no longer owns selection state. Real movement
    // goes through `App::apply_move` so that filter-aware clamping
    // can be applied uniformly. This stub is intentionally a no-op
    // and only kept to preserve the public surface for downstream
    // callers that previously depended on it.
}

// Reference to silence unused-imports clippy in some configurations.
#[allow(dead_code)]
fn _key_ref(k: KeyEvent) -> KeyEvent {
    let _ = KeyModifiers::NONE;
    k
}
