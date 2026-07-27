//! Footer — keybinding hint + report paths summary.

use crate::component::{Action, Component, Event};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, unfocused_block};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub struct FooterHelpComponent;

impl FooterHelpComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        let block = if focused {
            focused_block("Help")
        } else {
            unfocused_block("Help")
        };
        let page_hint = match state.current_page {
            crate::component::PageId::ScanProgress => "p progress",
            crate::component::PageId::Findings => "f findings",
            crate::component::PageId::FindingDetail => "Enter detail",
            crate::component::PageId::Flows => "g flows",
            crate::component::PageId::Diagnostics => "d diagnostics",
            crate::component::PageId::Report => "r rerun",
        };
        let mut lines = vec![Line::from(vec![
            Span::styled("Keys: ", Style::default().fg(Color::Yellow)),
            Span::raw("q/Esc quit • Tab/Shift+Tab cycle • j/k move • "),
            Span::styled(page_hint, Style::default().fg(Color::Cyan)),
            Span::raw(" • / filter • ? help"),
        ])];
        if !state.report_paths.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Reports:",
                Style::default().fg(Color::DarkGray),
            )]));
            for path in state.report_paths.iter().take(2) {
                lines.push(Line::from(Span::styled(
                    format!("  {}", path.display()),
                    Style::default().fg(Color::White),
                )));
            }
        }
        let widget = Paragraph::new(lines).block(block);
        frame.render_widget(widget, area);
    }
}

impl Component for FooterHelpComponent {
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
        FooterHelpComponent::render_for(frame, area, &AppState::default(), false);
    }
}
