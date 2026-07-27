//! Markdown report renderer.
//!
//! Renders a [`ReportDocument`] to a single Markdown string. The
//! structure is the one defined in the implementation doc — Executive
//! Summary, Project Information, Findings (grouped by severity), and
//! so on. Every piece of user content is escaped, even though Markdown
//! doesn't really need it, so reports round-trip cleanly through
//! comment strips.

use crate::document;
use crate::error::ReportError;
use runnerguard_model::{Finding, ReportDocument, Severity};
use std::fmt::Write;

/// Render the document to a Markdown byte string.
pub fn render(doc: &ReportDocument) -> Result<Vec<u8>, ReportError> {
    let mut out = String::new();
    render_into(doc, &mut out)?;
    Ok(out.into_bytes())
}

pub fn render_into(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "# RunnerGuard Scan Report").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    render_executive_summary(doc, out)?;
    render_project_information(doc, out)?;
    render_scan_configuration(doc, out)?;
    render_rule_summary(doc, out)?;
    render_findings(doc, out)?;
    render_flow_inventory(doc, out)?;
    render_unresolved(doc, out)?;
    render_parser_diagnostics(doc, out)?;
    render_artifacts(out)?;
    render_versions(doc, out)?;
    Ok(())
}

fn render_executive_summary(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Executive Summary").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    let s = &doc.summary;
    writeln!(
        out,
        "Scan result: **{:?}**. {} finding(s) total — {} critical, {} error, {} warning, {} info.",
        s.result, s.findings_total, s.critical, s.error, s.warning, s.info
    )
    .map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_project_information(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Project Information").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    writeln!(
        out,
        "- Project name: {}",
        escape_md(&doc.metadata.project_name)
    )
    .map_err(ReportError::from)?;
    writeln!(out, "- Project id: {}", escape_md(&doc.metadata.project_id))
        .map_err(ReportError::from)?;
    writeln!(out, "- Files scanned: {}", doc.summary.files_scanned).map_err(ReportError::from)?;
    writeln!(out, "- Flows: {}", doc.summary.flow_count).map_err(ReportError::from)?;
    writeln!(out, "- Subflows: {}", doc.summary.subflow_count).map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_scan_configuration(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Scan Configuration").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    writeln!(
        out,
        "- Tool: {} {}",
        doc.metadata.tool, doc.metadata.tool_version
    )
    .map_err(ReportError::from)?;
    writeln!(out, "- Schema version: {}", doc.metadata.schema_version)
        .map_err(ReportError::from)?;
    writeln!(
        out,
        "- Generated at (unix): {}",
        doc.metadata.generated_at_unix
    )
    .map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_rule_summary(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Rule Summary").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    let rollup = document::rollup_by_rule(&doc.findings);
    if rollup.is_empty() {
        writeln!(out, "_No findings._").map_err(ReportError::from)?;
    } else {
        writeln!(out, "| Rule | Fires |").map_err(ReportError::from)?;
        writeln!(out, "|------|-------|").map_err(ReportError::from)?;
        for (rule, count) in &rollup {
            writeln!(out, "| {} | {} |", escape_md(rule), count).map_err(ReportError::from)?;
        }
    }
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_findings(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Findings").map_err(ReportError::from)?;
    let by_sev = document::group_by_severity(doc.findings.clone());
    let order = [
        Severity::Critical,
        Severity::Error,
        Severity::Warning,
        Severity::Info,
    ];
    for sev in order {
        let Some(items) = by_sev.get(&sev) else {
            continue;
        };
        writeln!(out, "### {}", sev_label(sev)).map_err(ReportError::from)?;
        writeln!(out).map_err(ReportError::from)?;
        for f in items {
            write_finding(f, out)?;
        }
    }
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn write_finding(f: &Finding, out: &mut String) -> Result<(), ReportError> {
    writeln!(
        out,
        "#### {} ({})",
        escape_md(&f.title),
        escape_md(&f.rule_id)
    )
    .map_err(ReportError::from)?;
    writeln!(out, "{}", escape_md(&f.message)).map_err(ReportError::from)?;
    if let Some(rec) = &f.recommendation {
        writeln!(out, "_Recommendation:_ {}", escape_md(rec)).map_err(ReportError::from)?;
    }
    if let Some(loc) = &f.source {
        writeln!(
            out,
            "_Location:_ `{}:{}:{}`",
            escape_md(&loc.file),
            loc.start_line,
            loc.start_column
        )
        .map_err(ReportError::from)?;
    }
    if let Some(eid) = &f.entity_id {
        writeln!(out, "_Entity:_ `{}`", escape_md(eid)).map_err(ReportError::from)?;
    }
    let evidence = serde_json::to_string(&f.evidence).unwrap_or_else(|_| "<unprintable>".into());
    writeln!(
        out,
        "<details><summary>Evidence</summary>\n\n```json\n{}\n```\n</details>",
        escape_code(&evidence)
    )
    .map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_flow_inventory(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Flow and Subflow Inventory").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    writeln!(out, "- Flows: {}", doc.summary.flow_count).map_err(ReportError::from)?;
    writeln!(out, "- Subflows: {}", doc.summary.subflow_count).map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_unresolved(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Unresolved References").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    let count: usize = doc
        .findings
        .iter()
        .filter(|f| f.rule_id.contains("reference"))
        .count();
    writeln!(out, "{} finding(s) reference resolution issues.", count)
        .map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_parser_diagnostics(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Parser Diagnostics").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    if doc.parser_diagnostics.is_empty() {
        writeln!(out, "_No parser diagnostics._").map_err(ReportError::from)?;
    } else {
        for d in &doc.parser_diagnostics {
            writeln!(
                out,
                "- [{}] {}: {}",
                escape_md(&d.code),
                escape_md(&format!("{:?}", d.stage)),
                escape_md(&d.message)
            )
            .map_err(ReportError::from)?;
        }
    }
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_artifacts(out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Generated Artifacts").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    writeln!(
        out,
        "- `report.md`, `report.html` — this document\n- `artifacts/project.json`, `artifacts/diagnostics.json`, `artifacts/flows/*.json` — machine-readable scan data"
    )
    .map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn render_versions(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    writeln!(out, "## Tool and Schema Versions").map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    writeln!(out, "- runnerguard {}", doc.metadata.tool_version).map_err(ReportError::from)?;
    writeln!(out, "- schema {}", doc.metadata.schema_version).map_err(ReportError::from)?;
    writeln!(out).map_err(ReportError::from)?;
    Ok(())
}

fn sev_label(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "Critical",
        Severity::Error => "Error",
        Severity::Warning => "Warning",
        Severity::Info => "Info",
    }
}

/// Escape user-supplied text for safe interpolation into Markdown output.
///
/// Markdown is rendered by GFM viewers that honor inline HTML, so `<`,
/// `>`, `&`, backtick, and backslash must all be neutralised in addition
/// to the table-pipe and newline that the table renderer needs.
fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '`' => out.push_str("\\`"),
            '|' => out.push_str("\\|"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push(' '),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Escape user-supplied text for safe interpolation into a fenced code
/// block. We replace any occurrence of the closing fence with a longer
/// fence so a malicious payload can't close the block early.
fn escape_code(s: &str) -> String {
    // Replace every backtick run with a backslash-escaped equivalent so
    // the user cannot close our ```json fence prematurely.
    s.replace('`', "\\`")
}
