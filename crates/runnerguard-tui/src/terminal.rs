//! Terminal lifecycle management.
//!
//! Owning the [`TerminalGuard`] is the only safe way to use the TUI:
//! the guard holds the [`Terminal`] in alternate screen + raw mode and
//! restores it on drop / panic.

use crate::error::TuiError;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{ExecutableCommand, event::DisableMouseCapture, event::EnableMouseCapture};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};
use std::sync::OnceLock;

/// Type alias for the concrete terminal type the TUI uses. Tests swap
/// this for a `TestBackend`.
pub type CtTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Guard that owns the `Terminal` and restores it on drop or panic.
pub struct TerminalGuard {
    terminal: Option<CtTerminal>,
}

impl std::fmt::Debug for TerminalGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalGuard")
            .field("active", &self.terminal.is_some())
            .finish()
    }
}

impl TerminalGuard {
    /// Enter the alternate screen, turn on raw mode, enable mouse
    /// capture, and install the panic hook. The panic hook restores the
    /// terminal BEFORE the runtime unwinds so a panic in a component
    /// doesn't leave the user staring at an empty / scrambled screen.
    pub fn enter() -> Result<Self, TuiError> {
        let mut stdout = stdout();
        enable_raw_mode().map_err(|e| TuiError::Terminal(e.to_string()))?;
        stdout
            .execute(EnterAlternateScreen)
            .map_err(|e| TuiError::Terminal(e.to_string()))?;
        stdout
            .execute(EnableMouseCapture)
            .map_err(|e| TuiError::Terminal(e.to_string()))?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).map_err(|e| TuiError::Terminal(e.to_string()))?;

        install_panic_hook();

        Ok(Self {
            terminal: Some(terminal),
        })
    }

    /// Borrow the terminal mutably. Panics if the guard was already
    /// disposed — that's a programmer error (trying to draw after
    /// leaving alternate screen).
    #[must_use]
    pub fn terminal_mut(&mut self) -> &mut CtTerminal {
        self.terminal
            .as_mut()
            .expect("TerminalGuard used after dispose")
    }

    /// Drop alternate screen + raw mode + mouse capture. Equivalent to
    /// relying on `Drop`, but lets the caller handle a failure (e.g. in
    /// tests where we don't care about restoration errors).
    pub fn dispose(&mut self) -> Result<(), TuiError> {
        if let Some(mut term) = self.terminal.take() {
            // Best-effort leave-alternate-screen; even on failure we
            // continue to restore raw mode + mouse capture below.
            let _ = term.show_cursor();
        }
        let mut stdout = stdout();
        let _ = stdout.execute(DisableMouseCapture);
        let _ = stdout.execute(LeaveAlternateScreen);
        disable_raw_mode().map_err(|e| TuiError::Terminal(e.to_string()))?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}

/// Guard returned by [`TerminalGuard::enter`] that has been disposed
/// explicitly — used by the `app.run` loop to ensure we still restore
/// when a panic propagates past the TUI layer.
fn install_panic_hook() {
    static HOOK: OnceLock<()> = OnceLock::new();
    HOOK.get_or_init(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Best-effort restore; if it fails there's nothing we can
            // do besides letting the original panic carry on.
            let mut stdout = stdout();
            let _ = stdout.execute(DisableMouseCapture);
            let _ = stdout.execute(LeaveAlternateScreen);
            let _ = disable_raw_mode();
            prev(info);
        }));
    });
}
