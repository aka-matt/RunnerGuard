//! Project summary panel — top-level counts and metadata.

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
pub struct ProjectSummaryComponent;

impl ProjectSummaryComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let mut lines = Vec::new();
        let project = state
            .progress
            .project
            .clone()
            .unwrap_or_else(|| "—".to_string());
        lines.push(Line::from(vec![
            Span::styled("Project: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(project, Style::default().fg(Color::Cyan)),
        ]));
        if let Some(summary) = &state.summary {
            lines.push(Line::from(format!(
                "rule_count={} • flows={} • subflows={}",
                summary.rule_count, summary.flow_count, summary.subflow_count
            )));
            lines.push(Line::from(format!(
                "findings: total={} info={} warning={} error={} critical={}",
                summary.findings_total,
                summary.info,
                summary.warning,
                summary.error,
                summary.critical
            )));
            lines.push(Line::from(format!(
                "diagnostics: {} • threshold_exceeded={}",
                state.diagnostics.len(),
                summary.threshold_exceeded(runnerguard_model::Severity::Warning)
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "(scan in progress or not yet completed)",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let block = if focused {
            focused_block("Project")
        } else {
            unfocused_block("Project")
        };
        let widget = Paragraph::new(lines).block(block);
        frame.render_widget(widget, area);
    }
}

impl Component for ProjectSummaryComponent {
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
        ProjectSummaryComponent::render_for(frame, area, &AppState::default(), false);
    }
}
