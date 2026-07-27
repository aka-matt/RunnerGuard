//! Cross-cutting diagnostic type used by parsers, the config layer, the
//! network layer, and AI integration.

use crate::source::SourceSpan;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticLevel {
    Note,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticStage {
    Config,
    FileSystem,
    Json,
    Xml,
    Mule,
    Rule,
    Http,
    Ai,
    Report,
    Tui,
}

impl DiagnosticStage {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Config => "CFG",
            Self::FileSystem => "FS",
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Mule => "MULE",
            Self::Rule => "RULE",
            Self::Http => "HTTP",
            Self::Ai => "AI",
            Self::Report => "RPT",
            Self::Tui => "TUI",
        }
    }
}

/// A non-fatal, machine-readable diagnostic emitted by any stage. Used both
/// for the parser's per-file failures and for non-fatal AI / HTTP errors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable error code, e.g. `XML-001` for invalid XML. Stage prefix is
    /// derived from `stage`; the numeric suffix is set by the emitter.
    pub code: String,
    pub level: DiagnosticLevel,
    pub stage: DiagnosticStage,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<String>,
}

impl Diagnostic {
    pub fn new(
        stage: DiagnosticStage,
        code: impl Into<String>,
        level: DiagnosticLevel,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            level,
            stage,
            message: message.into(),
            help: None,
            source: None,
            causes: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: SourceSpan) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.causes.push(cause.into());
        self
    }
}
