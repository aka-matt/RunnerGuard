//! Component trait and event / action primitives shared by all TUI panels.

use crate::error::TuiError;
use crossterm::event::Event as CtEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Events the [`App`](crate::app::App) forwards into the focused
/// component. Each variant corresponds to a concrete key/mouse event the
/// terminal produced; [`Action::Other`] carries anything not yet
/// recognised so components can decide for themselves.
#[derive(Debug, Clone)]
pub enum Event {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize { width: u16, height: u16 },
    Tick,
}

/// High-level actions raised by components in response to events. The
/// app applies them across state and may schedule page changes, focus
/// moves, or scan reruns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move focus to the next / previous panel within the current page.
    NextPanel,
    PrevPanel,
    /// Switch to a specific page by name.
    Page(PageId),
    /// Quit the TUI.
    Quit,
    /// Move the selection within the focused panel.
    Move(MoveDirection),
    /// Confirm / enter the highlighted item.
    Enter,
    /// Clear an in-panel filter.
    ClearFilter,
    /// Component-opaque payload (e.g. toggling a sub-view).
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// The fixed set of pages shown in the title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageId {
    #[default]
    ScanProgress,
    Findings,
    FindingDetail,
    Flows,
    Diagnostics,
    Report,
}

impl PageId {
    pub const ALL: &'static [PageId] = &[
        Self::ScanProgress,
        Self::Findings,
        Self::FindingDetail,
        Self::Flows,
        Self::Diagnostics,
        Self::Report,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ScanProgress => "Scan",
            Self::Findings => "Findings",
            Self::FindingDetail => "Detail",
            Self::Flows => "Flows",
            Self::Diagnostics => "Diagnostics",
            Self::Report => "Report",
        }
    }
}

/// Trait every panel implements. The lifecycle is:
///   * `init` — one-shot setup (counters, internal state).
///   * `handle_event` — turn a raw event into either an inline `update`
///     or an outbound `Action`.
///   * `update` — apply a previously-decided action.
///   * `render` — paint into the given area.
///
/// None of these methods return a Result that expects the caller to
/// recover; errors during `init`/`update` go to `init` of the
/// [`App`](crate::app::App) and quit the TUI.
pub trait Component {
    fn init(&mut self) -> Result<(), TuiError>;

    /// Inspect the event; if the component wants to consume it,
    /// return the action it decided on. Returning `None` means "ignore".
    fn handle_event(&mut self, event: &Event) -> Option<Action>;

    /// Apply an action that was decided either locally or by another
    /// component / the app shell.
    fn update(&mut self, action: &Action) -> Result<(), TuiError>;

    /// Paint the component. Implementations must not panic on small
    /// areas — fall back to a minimal layout when `area` is too small.
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);
}

/// Translate a raw crossterm event into our [`Event`] enum; unknown
/// variant types become [`Event::Tick`] so the caller still has
/// something to dispatch.
#[must_use]
pub fn from_crossterm(event: CtEvent) -> Event {
    match event {
        CtEvent::Key(k) => Event::Key(k),
        CtEvent::Mouse(m) => Event::Mouse(m),
        CtEvent::Resize(w, h) => Event::Resize {
            width: w,
            height: h,
        },
        CtEvent::FocusGained | CtEvent::FocusLost | CtEvent::Paste(_) => Event::Tick,
    }
}
