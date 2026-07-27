//! Report rendering for RunnerGuard.
//!
//! Takes a [`ReportDocument`] and emits Markdown or HTML. The crate is
//! deliberately stateless — no I/O. Callers write the bytes to disk.
//!
//! Markdown is the canonical wire format; HTML is the human-friendly
//! version with embedded CSS and a tiny inline script for filtering.

#![deny(unsafe_code)]

pub mod document;
pub mod error;
pub mod html;
pub mod markdown;

pub use document::{build_document, group_by_severity, rollup_by_rule, sort_findings};
pub use error::ReportError;
pub use html::render as render_html;
pub use markdown::render as render_markdown;

use runnerguard_model::ReportDocument;

/// A pluggable report renderer.
pub trait ReportRenderer {
    fn format(&self) -> ReportFormatLabel;
    fn render(&self, document: &ReportDocument) -> Result<Vec<u8>, ReportError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportFormatLabel {
    Markdown,
    Html,
}

pub struct MarkdownRenderer;
impl ReportRenderer for MarkdownRenderer {
    fn format(&self) -> ReportFormatLabel {
        ReportFormatLabel::Markdown
    }
    fn render(&self, doc: &ReportDocument) -> Result<Vec<u8>, ReportError> {
        markdown::render(doc)
    }
}

pub struct HtmlRenderer;
impl ReportRenderer for HtmlRenderer {
    fn format(&self) -> ReportFormatLabel {
        ReportFormatLabel::Html
    }
    fn render(&self, doc: &ReportDocument) -> Result<Vec<u8>, ReportError> {
        html::render(doc)
    }
}
