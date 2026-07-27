//! Build [`Finding`] objects from rule-evaluation results.
//!
//! Every finding follows the same shape: rule id, severity, evidence
//! with operator + actual + expected, optional source location. The
//! engine never emits findings with `origin != deterministic-rule`.

use crate::compile::CompiledRule;
use crate::context::{EvalContext, EvalTarget};
use crate::operator;
use runnerguard_model::{Finding, FindingOrigin, MuleFlowKind, Severity};
use serde_json::json;

/// Construct a Finding from a compiled rule and the context that fired.
pub fn build(
    rule: &CompiledRule,
    ctx: &EvalContext<'_>,
    operator: runnerguard_model::Operator,
    actual: serde_json::Value,
    expected: Option<serde_json::Value>,
) -> Finding {
    let message = crate::template::render(&rule.message, ctx, Some(&actual), expected.as_ref());
    let recommendation = rule
        .recommendation
        .as_ref()
        .map(|t| crate::template::render(t, ctx, Some(&actual), expected.as_ref()));
    let entity_id = entity_id(ctx);
    let source = source_location(ctx);
    let evidence = json!({
        "operator": operator.as_str(),
        "actual": actual,
        "expected": expected,
    });
    Finding {
        rule_id: rule.id.clone(),
        severity: rule.severity,
        title: rule.title.clone(),
        message,
        recommendation,
        entity_id,
        source,
        evidence,
        origin: FindingOrigin::DeterministicRule,
        rule_version: Some(rule.version.clone()),
    }
}

/// Convenience for tests that don't care about evidence details.
pub fn bare(rule_id: &str, severity: Severity, message: &str) -> Finding {
    Finding::deterministic(rule_id, severity, rule_id, message)
}

fn entity_id(ctx: &EvalContext<'_>) -> Option<String> {
    match &ctx.target {
        EvalTarget::Project(p) => Some(p.id.clone()),
        EvalTarget::File(f) => Some(format!("file:{}", f.path)),
        EvalTarget::Flow(f) => Some(f.id.clone()),
        EvalTarget::Component(c) => Some(c.id.clone()),
        EvalTarget::PropertyRef { flow_name, value } => {
            Some(format!("property:{flow_name}:{value}"))
        }
        EvalTarget::FlowRef(r) => Some(format!("reference:{}", r.target)),
        EvalTarget::DataweaveBlock(b) => Some(format!("dataweave:{}", b.content_hash)),
    }
}

fn source_location(ctx: &EvalContext<'_>) -> Option<runnerguard_model::SourceSpan> {
    match &ctx.target {
        EvalTarget::Flow(f) => Some(f.source.clone()),
        EvalTarget::Component(c) => Some(c.source.clone()),
        EvalTarget::FlowRef(r) => Some(r.source.clone()),
        EvalTarget::DataweaveBlock(b) => Some(b.source.clone()),
        _ => None,
    }
}

#[allow(dead_code)]
fn flow_kind_label(ctx: &EvalContext<'_>) -> Option<&'static str> {
    if let EvalTarget::Flow(f) = &ctx.target {
        Some(match f.kind {
            MuleFlowKind::Flow => "flow",
            MuleFlowKind::SubFlow => "sub-flow",
        })
    } else {
        None
    }
}

#[allow(dead_code)]
fn operator_name(op: runnerguard_model::Operator) -> &'static str {
    operator::operator_label(op)
}
