//! Flow tree — XML file, flow, subflow, components.

use crate::component::{Action, Component, Event, MoveDirection};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, unfocused_block};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListState, Paragraph};

#[derive(Debug, Default)]
pub struct FlowTreeComponent;

impl FlowTreeComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let block = if focused {
            focused_block("Flows")
        } else {
            unfocused_block("Flows")
        };
        if area.width < 20 || area.height < 3 {
            let paragraph = Paragraph::new("(terminal too small)")
                .block(block)
                .style(Style::default());
            frame.render_widget(paragraph, area);
            return;
        }
        // Walk the project's flow index into a flat string tree so
        // we don't depend on a real parser here. The app wires the
        // actual ParsedProject in.
        let lines = state
            .summary
            .as_ref()
            .map(|s| {
                vec![
                    Line::from(Span::styled(
                        format!("flows: {}", s.flow_count),
                        Style::default().fg(Color::Cyan),
                    )),
                    Line::from(format!("subflows: {}", s.subflow_count)),
                    Line::from(format!("findings: {}", s.findings_total)),
                ]
            })
            .unwrap_or_else(|| {
                vec![Line::from(Span::styled(
                    "(scan summary not yet available)",
                    Style::default().fg(Color::DarkGray),
                ))]
            });
        let items: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol(">> ");
        let mut list_state = build_list_state(state, area);
        frame.render_stateful_widget(list, area, &mut list_state);
        // Keep the line var alive for clarity in the helper:
        let _ = lines;
    }
}

/// Build a `ListState` that scrolls to keep `selections.flow_index`
/// in the visible window. See `findings::build_table_state` for the
/// rationale — without explicit offset tracking the user can only see
/// the first viewport of rows.
fn build_list_state(state: &AppState, area: Rect) -> ListState {
    let n = state.artifact_paths.len();
    let selected = if n == 0 {
        None
    } else {
        Some(state.selections.flow_index.min(n - 1))
    };
    if n == 0 {
        return ListState::default();
    }
    let viewport = (area.height as usize).saturating_sub(2);
    let max_offset = n.saturating_sub(1);
    let mut offset = state.selections.flow_offset.min(max_offset);
    if viewport > 0 {
        let visible_end = offset.saturating_add(viewport);
        if let Some(sel) = selected {
            if sel >= visible_end {
                offset = (sel + 1).saturating_sub(viewport).min(max_offset);
            } else if sel < offset {
                offset = sel;
            }
        }
    }
    ListState::default()
        .with_offset(offset)
        .with_selected(selected)
}

impl Component for FlowTreeComponent {
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
            _ => None,
        }
    }
    fn update(&mut self, _action: &Action) -> Result<(), TuiError> {
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        FlowTreeComponent::render_for(frame, area, &AppState::default(), false);
    }
}

/// Sub-tree formatter; takes a Vec of (depth, label) rows and returns
/// the styled lines for the tree display.
pub fn tree_lines(rows: &[(u8, String)]) -> Vec<Line<'static>> {
    rows.iter()
        .map(|(depth, label)| {
            let indent = "  ".repeat(*depth as usize);
            Line::from(format!("{indent}{label}"))
        })
        .collect()
}

#[allow(dead_code)]
fn k(x: KeyEvent) -> KeyEvent {
    x
}
