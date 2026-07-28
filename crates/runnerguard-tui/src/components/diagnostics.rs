//! Diagnostics list — parser, config, network, AI errors.

use crate::component::{Action, Component, Event, MoveDirection};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, unfocused_block};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{List, ListState};

#[derive(Debug, Default)]
pub struct DiagnosticsComponent;

impl DiagnosticsComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let block = if focused {
            focused_block("Diagnostics")
        } else {
            unfocused_block("Diagnostics")
        };
        if state.diagnostics.is_empty() {
            let paragraph = List::new(vec!["(no diagnostics)"])
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, area);
            return;
        }
        let items: Vec<String> = state
            .diagnostics
            .iter()
            .map(|d| format!("[{:?}/{:?}] {} — {}", d.level, d.stage, d.code, d.message))
            .collect();
        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
        let mut list_state = build_list_state(state, area);
        frame.render_stateful_widget(list, area, &mut list_state);
    }
}

/// Build a `ListState` that keeps `selections.diagnostic_index` in
/// the visible window. Mirrors the offset-tracking pattern used by
/// the findings table — see `findings::build_table_state` for the
/// full rationale. Without this, only the first viewport of rows was
/// ever visible because `ListState::select` does not touch the
/// offset.
fn build_list_state(state: &AppState, area: Rect) -> ListState {
    let n = state.diagnostics.len();
    let selected = if n == 0 {
        None
    } else {
        Some(state.selections.diagnostic_index.min(n - 1))
    };
    if n == 0 {
        return ListState::default();
    }
    let viewport = (area.height as usize).saturating_sub(2); // borders only
    let max_offset = n.saturating_sub(1);
    let mut offset = state.selections.diagnostic_offset.min(max_offset);
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

impl Component for DiagnosticsComponent {
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
        DiagnosticsComponent::render_for(frame, area, &AppState::default(), false);
    }
}
