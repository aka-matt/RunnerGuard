//! Filesystem traversal, file classification, and atomic writes for
//! RunnerGuard.
//!
//! This crate owns the *physical* side of project discovery: walking the
//! directory, classifying files, enforcing size/depth limits, and writing
//! report artifacts without ever leaving a half-written file behind.
//!
//! It does **not** parse any Mule XML or YAML — that's the parser / config
//! crates' job. It only decides what's on disk.

#![deny(unsafe_code)]

pub mod classify;
pub mod discovery;
pub mod limits;
pub mod project;
pub mod writer;

pub use discovery::{DiscoverOptions, default_project_files, discover};
pub use limits::Limits;
pub use project::ProjectFiles;
pub use runnerguard_model::{SourceFile, SourceFileKind};
pub use writer::write_atomic;
