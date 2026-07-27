//! Header — title bar showing project name, current page, focus.

use crate::component::{Action, Component, Event};
use crate::error::TuiError;
use crate::state::AppState;
use crate::view::{focused_block, page_label, unfocused_block};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Debug, Default)]
pub struct HeaderComponent;

impl Component for HeaderComponent {
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
        let project = default_string();
        let page = current_page_label();
        let text = Line::from(vec![
            Span::styled(
                "RunnerGuard ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("| "),
            Span::styled(project, Style::default().fg(Color::White)),
            Span::raw(" | page: "),
            Span::styled(
                page,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let block = if area.width >= 40 {
            focused_block("Header")
        } else {
            unfocused_block("Header")
        };
        let widget = Paragraph::new(text).block(block);
        frame.render_widget(widget, area);
    }
}

/// Thread-local "currently displayed" app state for the header/footer
/// to read without taking a mutable reference. The App updates this
/// before delegating to render.
#[derive(Debug, Default)]
pub struct HeaderProjection {
    pub project_name: String,
    pub page: &'static str,
    pub focus: &'static str,
}

thread_local! {
    static HEADER_PROJECTION: std::cell::RefCell<HeaderProjection> = const { std::cell::RefCell::new(HeaderProjection {
        project_name: String::new(),
        page: "scan",
        focus: "body",
    }) };
}

pub fn set_header_projection(p: HeaderProjection) {
    HEADER_PROJECTION.with(|cell| *cell.borrow_mut() = p);
}

fn default_string() -> String {
    HEADER_PROJECTION.with(|cell| cell.borrow().project_name.clone())
}

fn current_page_label() -> &'static str {
    HEADER_PROJECTION.with(|cell| cell.borrow().page)
}

pub fn render_dynamic_header(frame: &mut Frame<'_>, area: Rect, state: &AppState, focused: bool) {
    let project = state
        .progress
        .project
        .clone()
        .unwrap_or_else(|| "<no project>".to_string());
    let stage = state.progress.stage.label();
    let page = page_label(state.current_page);
    let block = if focused {
        focused_block("Header")
    } else {
        unfocused_block("Header")
    };
    let body = vec![
        Line::from(vec![
            Span::styled(
                "RunnerGuard ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(project, Style::default().fg(Color::White)),
            Span::raw("  •  "),
            Span::styled(stage, Style::default().fg(Color::Magenta)),
            Span::raw("  •  page "),
            Span::styled(page, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(format!(
            "flows {} • rules {}/{} • findings {}",
            state.progress.parsed_flows,
            state.progress.rules_completed,
            state.progress.rules_loaded,
            state.progress.finding_count
        )),
    ];
    let widget = Paragraph::new(body).block(block);
    frame.render_widget(widget, area);
}
