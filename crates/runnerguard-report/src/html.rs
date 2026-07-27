//! HTML report renderer.
//!
//! Produces a single self-contained HTML file with embedded CSS, no
//! external CDN, and a small inline script for filtering. Every user
//! content node is HTML-escaped before insertion — the renderer never
//! trusts the strings it gets.

use crate::document;
use crate::error::ReportError;
use runnerguard_model::{Finding, ReportDocument, Severity};

const STYLE: &str = r#"
:root {
  color-scheme: light dark;
  --bg: #f7f7f9;
  --fg: #1a1a1a;
  --muted: #666;
  --accent: #234;
  --critical: #b00020;
  --error: #c9302c;
  --warning: #b88600;
  --info: #2e6da4;
  --rule: #555;
  --border: #e0e0e0;
  --code-bg: #f0f0f0;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1d1f23;
    --fg: #e5e5e5;
    --muted: #aaa;
    --accent: #cbd3e0;
    --critical: #ff6b6b;
    --error: #f08080;
    --warning: #f4c75c;
    --info: #74b9ff;
    --rule: #bbb;
    --border: #333;
    --code-bg: #2a2c30;
  }
}
* { box-sizing: border-box; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  line-height: 1.5;
}
header {
  padding: 1.5rem 2rem;
  border-bottom: 1px solid var(--border);
}
header h1 { margin: 0 0 0.25rem 0; }
header .meta { color: var(--muted); font-size: 0.9rem; }
main { padding: 1rem 2rem 4rem; max-width: 1100px; margin: 0 auto; }
section { margin: 2rem 0; }
section h2 { border-bottom: 1px solid var(--border); padding-bottom: 0.25rem; }
.controls {
  display: flex; gap: 0.75rem; flex-wrap: wrap; margin: 0.75rem 0;
}
.controls input, .controls select {
  padding: 0.35rem 0.5rem;
  font-size: 0.9rem;
  border: 1px solid var(--border);
  background: var(--bg); color: var(--fg); border-radius: 4px;
}
table { width: 100%; border-collapse: collapse; }
th, td { text-align: left; padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border); vertical-align: top; }
th { font-size: 0.85rem; color: var(--muted); }
.finding { border-left: 4px solid var(--rule); padding: 0.5rem 0.75rem; margin: 0.5rem 0; background: rgba(0,0,0,0.02); }
.finding.critical { border-left-color: var(--critical); }
.finding.error    { border-left-color: var(--error); }
.finding.warning  { border-left-color: var(--warning); }
.finding.info     { border-left-color: var(--info); }
.finding .title { font-weight: 600; }
.finding .rule { color: var(--rule); font-size: 0.85rem; margin-left: 0.5rem; }
.finding pre { background: var(--code-bg); padding: 0.5rem; border-radius: 4px; overflow: auto; font-size: 0.8rem; }
details summary { cursor: pointer; color: var(--accent); font-size: 0.85rem; }
.pill { display: inline-block; padding: 0.05rem 0.5rem; border-radius: 999px; font-size: 0.75rem; color: white; }
.pill.critical { background: var(--critical); }
.pill.error    { background: var(--error); }
.pill.warning  { background: var(--warning); }
.pill.info     { background: var(--info); }
.kv { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; }
.kv dt { color: var(--muted); }
"#;

pub fn render(doc: &ReportDocument) -> Result<Vec<u8>, ReportError> {
    let mut out = String::new();
    render_into(doc, &mut out)?;
    Ok(out.into_bytes())
}

pub fn render_into(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>RunnerGuard Scan Report</title>\n");
    out.push_str("<style>\n");
    out.push_str(STYLE);
    out.push_str("\n</style>\n");
    out.push_str("</head>\n<body>\n");

    render_header(doc, out)?;
    out.push_str("<main>\n");
    render_summary(doc, out)?;
    render_findings(doc, out)?;
    render_diagnostics(doc, out)?;
    render_artifacts(out)?;
    render_versions(doc, out)?;
    out.push_str("</main>\n");

    out.push_str("<script>\n");
    out.push_str(JS);
    out.push_str("\n</script>\n");

    out.push_str("</body>\n</html>\n");
    Ok(())
}

fn render_header(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<header><h1>RunnerGuard Scan Report</h1>\n");
    out.push_str("<div class=\"meta\">");
    out.push_str(&format!(
        "<span>Project: <strong>{}</strong></span> &middot; <span>Tool: {} {}</span> &middot; <span>Schema: {}</span>",
        escape(&doc.metadata.project_name),
        escape(&doc.metadata.tool),
        escape(&doc.metadata.tool_version),
        escape(&doc.metadata.schema_version),
    ));
    out.push_str("</div></header>\n");
    Ok(())
}

fn render_summary(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<section><h2>Executive Summary</h2>\n");
    let s = &doc.summary;
    out.push_str(&format!(
        "<p>Result: <strong>{:?}</strong>. {} finding(s) — <span class=\"pill critical\">{}</span> <span class=\"pill error\">{}</span> <span class=\"pill warning\">{}</span> <span class=\"pill info\">{}</span></p>\n",
        s.result, s.findings_total, s.critical, s.error, s.warning, s.info
    ));
    out.push_str(&format!(
        "<dl class=\"kv\"><dt>Files scanned</dt><dd>{}</dd><dt>Flows</dt><dd>{}</dd><dt>Subflows</dt><dd>{}</dd><dt>Rules</dt><dd>{}</dd></dl>\n",
        s.files_scanned, s.flow_count, s.subflow_count, s.rule_count
    ));
    out.push_str("</section>\n");
    Ok(())
}

fn render_findings(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<section><h2>Findings</h2>\n");
    out.push_str("<div class=\"controls\">\n");
    out.push_str(
        "<input type=\"search\" id=\"flt-text\" placeholder=\"Filter by text, rule id, file…\">\n",
    );
    out.push_str("<select id=\"flt-severity\"><option value=\"\">All severities</option>");
    for sev in [
        Severity::Critical,
        Severity::Error,
        Severity::Warning,
        Severity::Info,
    ] {
        out.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            sev.as_str(),
            sev_label(sev)
        ));
    }
    out.push_str("</select>\n");
    out.push_str("</div>\n");

    out.push_str("<div id=\"findings-list\">\n");
    let grouped = document::group_by_severity(doc.findings.clone());
    let order = [
        Severity::Critical,
        Severity::Error,
        Severity::Warning,
        Severity::Info,
    ];
    for sev in order {
        let Some(items) = grouped.get(&sev) else {
            continue;
        };
        out.push_str(&format!(
            "<h3 class=\"sev-{}\">{}</h3>\n",
            sev.as_str(),
            sev_label(sev)
        ));
        for f in items {
            render_finding_card(f, out);
        }
    }
    out.push_str("</div>\n</section>\n");
    Ok(())
}

fn render_finding_card(f: &Finding, out: &mut String) {
    let sev_class = f.severity.as_str();
    out.push_str(&format!(
        "<article class=\"finding {}\" data-severity=\"{}\" data-text=\"{}\">\n",
        sev_class,
        sev_class,
        escape(&format!(
            "{} {} {}",
            f.rule_id,
            f.title,
            f.source
                .as_ref()
                .map(|s| s.file.clone())
                .unwrap_or_default()
        )),
    ));
    out.push_str(&format!(
        "<div class=\"title\">{} <span class=\"rule\">{}</span></div>\n",
        escape(&f.title),
        escape(&f.rule_id),
    ));
    out.push_str(&format!(
        "<div class=\"message\">{}</div>\n",
        escape(&f.message)
    ));
    if let Some(rec) = &f.recommendation {
        out.push_str(&format!(
            "<div class=\"recommendation\"><em>Recommendation:</em> {}</div>\n",
            escape(rec)
        ));
    }
    if let Some(loc) = &f.source {
        out.push_str(&format!(
            "<div class=\"location\">at <code>{}:{}:{}</code></div>\n",
            escape(&loc.file),
            loc.start_line,
            loc.start_column
        ));
    }
    let evidence = serde_json::to_string(&f.evidence).unwrap_or_default();
    out.push_str("<details><summary>Evidence</summary>\n");
    out.push_str(&format!("<pre>{}</pre>\n", escape(&evidence)));
    out.push_str("</details>\n");
    out.push_str("</article>\n");
}

fn render_diagnostics(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<section><h2>Parser Diagnostics</h2>\n");
    if doc.parser_diagnostics.is_empty() {
        out.push_str("<p><em>No parser diagnostics.</em></p>\n");
    } else {
        out.push_str(
            "<table><thead><tr><th>Code</th><th>Stage</th><th>Message</th></tr></thead><tbody>\n",
        );
        for d in &doc.parser_diagnostics {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>\n",
                escape(&d.code),
                escape(&format!("{:?}", d.stage)),
                escape(&d.message),
            ));
        }
        out.push_str("</tbody></table>\n");
    }
    out.push_str("</section>\n");
    Ok(())
}

fn render_artifacts(out: &mut String) -> Result<(), ReportError> {
    out.push_str("<section><h2>Generated Artifacts</h2>\n<ul>\n");
    out.push_str("<li><code>report.md</code>, <code>report.html</code> — this report</li>\n");
    out.push_str("<li><code>artifacts/project.json</code>, <code>artifacts/diagnostics.json</code>, <code>artifacts/flows/*.json</code></li>\n");
    out.push_str("</ul>\n</section>\n");
    Ok(())
}

fn render_versions(doc: &ReportDocument, out: &mut String) -> Result<(), ReportError> {
    out.push_str("<section><h2>Tool and Schema Versions</h2>\n");
    out.push_str(&format!(
        "<p>runnerguard {} &middot; schema {}</p>\n",
        escape(&doc.metadata.tool_version),
        escape(&doc.metadata.schema_version),
    ));
    out.push_str("</section>\n");
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

/// HTML-escape every character that has a special meaning in HTML.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const JS: &str = r#"
(function() {
  var text = document.getElementById('flt-text');
  var sev = document.getElementById('flt-severity');
  var list = document.getElementById('findings-list');
  if (!text || !sev || !list) return;
  function apply() {
    var t = (text.value || '').toLowerCase();
    var s = sev.value || '';
    var items = list.querySelectorAll('.finding');
    Array.prototype.forEach.call(items, function(el) {
      var textMatch = !t || (el.getAttribute('data-text') || '').toLowerCase().indexOf(t) !== -1;
      var sevMatch = !s || el.getAttribute('data-severity') === s;
      el.style.display = (textMatch && sevMatch) ? '' : 'none';
    });
  }
  text.addEventListener('input', apply);
  sev.addEventListener('change', apply);
})();
"#;
