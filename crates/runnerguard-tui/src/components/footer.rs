//! Footer — keybinding hint + report paths summary, or the
//! filter-input bar while the user is editing a pattern.

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
pub struct FooterHelpComponent;

impl FooterHelpComponent {
    pub fn render_for(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
        // While the user is editing a filter pattern the footer
        // becomes a single-line input bar — the global help text
        // would be confusing noise while they're trying to type.
        // The block title changes too so the panel reads as
        // "Filter" rather than "Help".
        if state.filter_mode {
            Self::render_filter_prompt(frame, area, &state.filter_input);
            return;
        }
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

    /// Render the in-progress filter input. The block is always
    /// styled as focused (cyan) because the user is actively typing
    /// into it; the title reflects that the bar *is* the filter
    /// input, not a status line.
    fn render_filter_prompt(frame: &mut Frame<'_>, area: Rect, draft: &str) {
        let block = focused_block("Filter");
        let line = Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                // The trailing underscore is a cheap cursor caret;
                // the prompt is rebuilt on every render so the caret
                // tracks the actual draft length without flicker.
                format!("{draft}_"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "Enter apply • Esc cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        let widget = Paragraph::new(vec![line]).block(block);
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
