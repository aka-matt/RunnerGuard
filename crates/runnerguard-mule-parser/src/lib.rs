//! MuleSoft XML parser producing per-flow JSON for RunnerGuard.
//!
//! This is the heart of the static analysis pipeline. It takes a
//! [`ProjectFiles`] from `runnerguard-fs`, parses each Mule XML document
//! namespace-aware, and produces a [`ParsedProject`] containing:
//!
//! - one [`MuleDocument`] per source XML file;
//! - one [`MuleFlow`] per `flow` and `sub-flow` element;
//! - pre-computed [`FlowFacts`] the rule engine reads;
//! - a project-wide [`ProjectIndex`] used for reference resolution.
//!
//! Per-file failures become [`Diagnostic`]s, never panics. External
//! entities are not resolved; DTD declarations are ignored.

#![deny(unsafe_code)]

pub mod facts;
pub mod index;
pub mod options;
pub mod parser;
pub mod project;
pub mod reference;

pub use options::ParseOptions;
pub use parser::parse_project;
pub use project::{MuleParser, ParseError, ParsedOutcome};
