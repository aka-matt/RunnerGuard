//! Parser entry point. The trait here lets `runnerguard-core` swap in a
//! mock during tests.

use crate::options::ParseOptions;
use runnerguard_fs::ProjectFiles;
use runnerguard_model::{Diagnostic, ParsedProject};

pub trait MuleParser: Send + Sync {
    fn parse_project(
        &self,
        files: &ProjectFiles,
        options: &ParseOptions,
    ) -> Result<ParsedProject, ParseError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("project has no XML to parse")]
    NoXml,
    #[error("too many parse failures: {count}")]
    TooManyFailures { count: usize },
}

/// Outcome of a parse: even on per-file failures, the parser still emits
/// whatever it could produce.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedOutcome {
    pub project: ParsedProject,
    pub diagnostics: Vec<Diagnostic>,
}
