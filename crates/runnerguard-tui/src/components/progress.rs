//! Progress panel — file / flow / rule / finding counters, last event.

use crate::component::{Action, Component, Event};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, unfocused_block};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};

#[derive(Debug, Default)]
pub struct ProgressComponent;

impl ProgressComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        // Use the top half of the area for a gauge; bottom half for the
        // counters and the last event line.
        let chunks = if area.height >= 6 {
            vec![
                Rect::new(area.x, area.y, area.width, 3),
                Rect::new(area.x, area.y + 3, area.width, area.height - 3),
            ]
        } else {
            vec![area]
        };
        let block = if focused {
            focused_block("Progress")
        } else {
            unfocused_block("Progress")
        };
        let ratio = compute_ratio(state);
        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(ratio.into())
            .label(format!("{}%", (ratio * 100.0) as u16))
            .block(block.clone());
        if chunks.len() == 2 {
            frame.render_widget(gauge, chunks[0]);
        } else {
            frame.render_widget(gauge, area);
            return;
        }
        let stage = state.progress.stage.label();
        let project = state
            .progress
            .project
            .clone()
            .unwrap_or_else(|| "—".to_string());
        let last = state
            .progress
            .last_event
            .clone()
            .unwrap_or_else(|| "(no events yet)".to_string());
        let body = vec![
            Line::from(vec![
                Span::styled("Stage: ", Style::default().fg(Color::DarkGray)),
                Span::styled(stage, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(format!(
                "Project: {project} • flows {} • subflows {}",
                state.progress.parsed_flows, 0
            )),
            Line::from(format!(
                "Rules {}/{} • findings {} • ai {}",
                state.progress.rules_completed,
                state.progress.rules_loaded,
                state.progress.finding_count,
                state.progress.ai_findings
            )),
            Line::from(vec![
                Span::styled("Last: ", Style::default().fg(Color::DarkGray)),
                Span::styled(last, Style::default().fg(Color::White)),
            ]),
        ];
        let widget = Paragraph::new(body).block(block);
        frame.render_widget(widget, chunks[1]);
    }
}

impl Component for ProgressComponent {
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
        ProgressComponent::render_for(frame, area, &AppState::default(), false);
    }
}

/// Compute a 0.0..=1.0 ratio for the progress gauge. Pure heuristic —
/// the document never promises an exact percentage; we just want a
/// look-the-user-can-read bar.
#[must_use]
pub fn compute_ratio(state: &AppState) -> f64 {
    use crate::state::Stage;
    match state.progress.stage {
        Stage::Idle => 0.0,
        Stage::Discovery => 0.1,
        Stage::Parse => {
            let target = (state.progress.rules_loaded as f64).max(1.0);
            (state.progress.parsed_flows as f64 / (target * 4.0)).clamp(0.1, 0.4)
        }
        Stage::Rules => {
            let total = (state.progress.rules_loaded as f64).max(1.0);
            (0.4 + 0.4 * (state.progress.rules_completed as f64 / total)).clamp(0.4, 0.8)
        }
        Stage::Ai => 0.85,
        Stage::Reports => 0.95,
        Stage::Done => 1.0,
    }
}
