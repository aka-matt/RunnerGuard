//! Orchestration layer for RunnerGuard.
//!
//! The crate wires the file system, parser, rule engine, and report
//! renderer into a single [`ScanService`] that takes a [`ScanRequest`]
//! and returns a [`ScanOutcome`]. It also defines the [`ScanEvent`]
//! stream consumed by both the CLI and the TUI.

#![deny(unsafe_code)]

pub mod event;
pub mod service;

pub use event::{CollectingSink, NullSink, ProgressSink, ScanEvent};
pub use runnerguard_model::{ReportFormat, ScanOutcome, ScanRequest, ScanResult, ScanSummary};
pub use service::{Files, ScanService, collecting_sink};
