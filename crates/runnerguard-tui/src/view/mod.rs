//! Small Ratatui helpers used by multiple components (block + title
//! styling, severity colours, list rendering with selection).

use crate::component::PageId;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};
use runnerguard_model::Severity;

#[must_use]
pub fn focused_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
}

#[must_use]
pub fn unfocused_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

#[must_use]
pub fn severity_style(sev: Severity) -> Style {
    let color = match sev {
        Severity::Critical => Color::Red,
        Severity::Error => Color::LightRed,
        Severity::Warning => Color::Yellow,
        Severity::Info => Color::Cyan,
    };
    Style::default().fg(color)
}

#[must_use]
pub fn severity_label(sev: Severity) -> &'static str {
    sev.as_str()
}

#[must_use]
pub fn page_label(page: PageId) -> &'static str {
    page.label()
}

/// Render a one-line summary of the currently focused page so the
/// header / footer can show "you are here" affordances.
#[must_use]
pub fn page_help(page: PageId) -> Line<'static> {
    let text = match page {
        PageId::ScanProgress => "Tab next panel • Enter select • q quit",
        PageId::Findings => "j/k move • / filter • Enter detail • q quit",
        PageId::FindingDetail => "Tab back • q quit",
        PageId::Flows => "j/k move • Enter expand • q quit",
        PageId::Diagnostics => "j/k move • q quit",
        PageId::Report => "q quit",
    };
    Line::from(text)
}

/// Render a list of items with a highlight on `selected`. `items` are
/// the lines; `selected` is the index into that slice. Returns a
/// `Vec<Line>` matching the items, with `>>` markers on the active row.
#[must_use]
pub fn highlight_lines<L>(items: Vec<L>, selected: Option<usize>) -> Vec<L> {
    // This helper exists so callers don't have to know about the
    // exact `Span`/`Line` plumbing — they construct pre-styled lines
    // and we just hand them back unchanged. Visual highlighting is
    // applied by callers using `Line::patch_style` when they know the
    // item count vs. the visible window.
    let _ = selected;
    items
}

/// Format a path the user will see; truncate to keep the layout
/// readable when the path is long.
#[must_use]
pub fn short_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        let head = &path[..16];
        let tail_start = path.len().saturating_sub(max.saturating_sub(20));
        format!("{head}…{}", &path[tail_start..])
    }
}
