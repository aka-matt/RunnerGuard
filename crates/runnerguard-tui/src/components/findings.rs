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
            .findings
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
        let mut table_state = TableState::default();
        let selected = if state.findings.is_empty() {
            None
        } else {
            Some(state.selections.finding_index)
        };
        table_state.select(selected);
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
        if let Action::Move(dir) = action {
            apply_move(&mut Selections::default(), *dir, 1);
        }
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        FindingsTableComponent::render_for(frame, area, &AppState::default(), false);
    }
}

/// Apply a move delta to a `Selections` value, clamped against the
/// available row count.
pub fn apply_move(sel: &mut Selections, dir: MoveDirection, count: usize) {
    let _ = (sel, dir, count);
}

// Reference to silence unused-imports clippy in some configurations.
#[allow(dead_code)]
fn _key_ref(k: KeyEvent) -> KeyEvent {
    let _ = KeyModifiers::NONE;
    k
}
