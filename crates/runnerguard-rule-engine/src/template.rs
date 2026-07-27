//! Template variable substitution for rule messages and recommendations.
//!
//! The rule DSL supports a closed set of `{{ variable }}` placeholders.
//! Any other name is left untouched — that lets us surface a typo in a
//! report rather than silently dropping the marker.

use crate::context::{EvalContext, EvalTarget};
use runnerguard_model::SourceSpan;
use serde_json::Value;

/// Resolve every supported placeholder against the evaluation context
/// plus the operator's actual/expected values.
pub fn render(
    template: &str,
    ctx: &EvalContext<'_>,
    actual: Option<&Value>,
    expected: Option<&Value>,
) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"{{") {
            if let Some(end) = find_close(&bytes[i + 2..]) {
                let name = &template[i + 2..i + 2 + end];
                let trimmed = name.trim();
                match lookup(trimmed, ctx, actual, expected) {
                    Some(v) => out.push_str(&v),
                    None => {
                        // Unknown placeholder — keep it verbatim so the
                        // typo is visible in the rendered output.
                        out.push_str(&template[i..i + 2 + end + 2]);
                    }
                }
                i += 2 + end + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn find_close(bytes: &[u8]) -> Option<usize> {
    let mut depth = 1;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if &bytes[i..i + 2] == b"{{" {
            depth += 1;
            i += 2;
        } else if &bytes[i..i + 2] == b"}}" {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

fn lookup(
    name: &str,
    ctx: &EvalContext<'_>,
    actual: Option<&Value>,
    expected: Option<&Value>,
) -> Option<String> {
    match name {
        "project.name" => Some(ctx.project.project.name.clone()),
        "project.id" => Some(ctx.project.project.id.clone()),
        "project.root" => Some(ctx.project.project.root_path.clone()),
        "file.path" => match &ctx.target {
            EvalTarget::File(f) => Some(f.path.clone()),
            _ => Some(source_file(ctx)),
        },
        "flow.id" => flow_id(ctx),
        "flow.name" => flow_name(ctx),
        "flow.kind" => flow_kind_str(ctx),
        "component.qualified-name" => component_qname(ctx),
        "component.local-name" => component_lname(ctx),
        "component.id" => component_id(ctx),
        "actual" => actual.map(value_to_string),
        "expected" => expected.map(value_to_string),
        "source.file" => Some(source_file(ctx)),
        "source.line" | "source.start-line" => Some(source_line(ctx).to_string()),
        "source.column" | "source.start-column" => Some(source_column(ctx).to_string()),
        _ => None,
    }
}

fn source_file(ctx: &EvalContext<'_>) -> String {
    span(ctx).map(|s| s.file).unwrap_or_default()
}
fn source_line(ctx: &EvalContext<'_>) -> u32 {
    span(ctx).map(|s| s.start_line).unwrap_or(0)
}
fn source_column(ctx: &EvalContext<'_>) -> u32 {
    span(ctx).map(|s| s.start_column).unwrap_or(0)
}

fn span(ctx: &EvalContext<'_>) -> Option<SourceSpan> {
    match &ctx.target {
        EvalTarget::Project(_) => None,
        EvalTarget::File(_) => None,
        EvalTarget::Flow(f) => Some(f.source.clone()),
        EvalTarget::Component(c) => Some(c.source.clone()),
        EvalTarget::PropertyRef { .. } => None,
        EvalTarget::FlowRef(r) => Some(r.source.clone()),
        EvalTarget::DataweaveBlock(b) => Some(b.source.clone()),
    }
}

fn flow_id(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Flow(f) => Some(f.id.clone()),
        EvalTarget::FlowRef(r) => Some(format!("reference:{}", r.target)),
        _ => None,
    }
}

fn flow_name(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Flow(f) => Some(f.name.clone()),
        EvalTarget::PropertyRef { flow_name, .. } => Some(flow_name.clone()),
        _ => None,
    }
}

fn flow_kind_str(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Flow(f) => Some(f.kind.as_str().to_string()),
        _ => None,
    }
}

fn component_qname(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Component(c) => Some(c.qualified_name.clone()),
        _ => None,
    }
}

fn component_lname(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Component(c) => Some(c.local_name.clone()),
        _ => None,
    }
}

fn component_id(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Component(c) => Some(c.id.clone()),
        _ => None,
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
