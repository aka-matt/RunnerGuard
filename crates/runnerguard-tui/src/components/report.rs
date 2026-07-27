//! Report page — markdown/HTML paths and artifact list.

use crate::component::{Action, Component, Event};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, unfocused_block};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub struct ReportPageComponent;

impl ReportPageComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let block = if focused {
            focused_block("Reports")
        } else {
            unfocused_block("Reports")
        };
        let mut lines = Vec::new();
        if state.report_paths.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no reports written yet)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Reports",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in &state.report_paths {
                lines.push(Line::from(format!("  {}", path.display())));
            }
        }
        if !state.artifact_paths.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Artifacts",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            for path in state.artifact_paths.iter().take(8) {
                lines.push(Line::from(format!("  {}", path.display())));
            }
            if state.artifact_paths.len() > 8 {
                lines.push(Line::from(format!(
                    "  …and {} more",
                    state.artifact_paths.len() - 8
                )));
            }
        }
        let widget = Paragraph::new(lines).block(block);
        frame.render_widget(widget, area);
    }
}

impl Component for ReportPageComponent {
    fn init(&mut self) -> Result<(), TuiError> {
        Ok(())
    }
    fn handle_event(&mut self, _event: &Event) -> Option<Action> {
        None
    }
    fn update(&mut self, _action: &Action) -> Result<(), TuiError> {
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        ReportPageComponent::render_for(frame, area, &AppState::default(), false);
    }
}
