//! Ratatui terminal UI for `RunnerGuard`.
//!
//! The TUI subscribes to the same [`ScanEvent`] stream the CLI uses; it
//! contains zero parsing, rule, or rendering logic of its own — only
//! the layout / event loop / component plumbing lives here. As with
//! the CLI, the public surface is just [`App`] plus the helper
//! [`run_cli`] for real terminals. Tests and the demo use
//! [`App::browsing`] / [`App::run`] with a precomputed outcome.

#![deny(unsafe_code)]

pub mod app;
pub mod component;
pub mod components;
pub mod error;
pub mod state;
pub mod terminal;
pub mod view;

pub use app::{App, EventSource, ScanSource, run_cli};
pub use component::{Action, Component, Event, MoveDirection, PageId, from_crossterm};
pub use error::TuiError;
pub use state::{AppState, Cycle, PanelId, ProgressCounters, Stage};
pub use terminal::TerminalGuard;
