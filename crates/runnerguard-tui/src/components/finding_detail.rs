//! Detail view for the highlighted finding.

use crate::component::{Action, Component, Event, MoveDirection};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, severity_label, severity_style, short_path, unfocused_block};
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub struct FindingDetailComponent;

impl FindingDetailComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let block = if focused {
            focused_block("Finding Detail")
        } else {
            unfocused_block("Finding Detail")
        };
        let mut lines = Vec::new();
        if let Some(f) = state.highlighted_finding() {
            lines.push(Line::from(vec![
                Span::styled(f.severity.as_str().to_string(), severity_style(f.severity)),
                Span::raw("  "),
                Span::styled(
                    f.rule_id.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  [{}]", severity_label(f.severity))),
            ]));
            lines.push(Line::from(f.title.clone()));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Message",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(f.message.clone()));
            if let Some(rec) = &f.recommendation {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Recommendation",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(rec.clone()));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Location",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            if let Some(s) = &f.source {
                lines.push(Line::from(format!(
                    "{}:{}-{}:{}",
                    short_path(&s.file, 60),
                    s.start_line,
                    s.start_column,
                    s.end_line.max(s.start_line)
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "(no source span)",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            if let Some(eid) = &f.entity_id {
                lines.push(Line::from(format!("Entity: {eid}")));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Evidence",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            let pretty = serde_json::to_string_pretty(&f.evidence)
                .unwrap_or_else(|_| "<unprintable>".to_string());
            for line in pretty.lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Cyan),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(format!(
                "origin={:?} rule_version={}",
                f.origin,
                f.rule_version.as_deref().unwrap_or("—")
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "(no finding selected)",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let widget = Paragraph::new(lines)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        frame.render_widget(widget, area);
    }
}

impl Component for FindingDetailComponent {
    fn init(&mut self) -> Result<(), TuiError> {
        Ok(())
    }
    fn handle_event(&mut self, event: &Event) -> Option<Action> {
        // The detail view shares the global `selections.finding_index`
        // with the Findings table, so accepting the same movement
        // keys here lets the user flip through issues *without*
        // bouncing back to the Findings page first. Movement is
        // intentionally only ±1 issue on this page — the user is
        // reading one finding at a time and "上一条 / 下一条" is the
        // expected semantics for both Up/Down and PageUp/PageDown.
        // See `App::apply_move` for the page-aware step size.
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
            _ => None,
        }
    }
    fn update(&mut self, _action: &Action) -> Result<(), TuiError> {
        Ok(())
    }
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        FindingDetailComponent::render_for(frame, area, &AppState::default(), false);
    }
}
