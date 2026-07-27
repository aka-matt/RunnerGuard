//! Top-level entry point: take a compiled rule set and a parsed project,
//! produce findings.

use crate::compile::{CompiledRule, CompiledRuleSet};
use crate::condition;
use crate::context::{EvalContext, EvalTarget};
use crate::error::EngineError;
use crate::target;
use runnerguard_model::{Condition, Finding, FindingOrigin, ParsedProject, Rule};
use serde_json::Value;

/// Result of one evaluation pass.
#[derive(Debug, Clone, Default)]
pub struct EvaluationResult {
    pub findings: Vec<Finding>,
    pub evaluated_rules: usize,
    pub skipped_rules: usize,
    pub errors: Vec<EngineError>,
}

/// Evaluate every rule in `set` against `project`. Determinism comes
/// from sorting candidates and from the fact that fact resolution and
/// operators are pure functions of their inputs.
pub fn evaluate(set: &CompiledRuleSet, project: &ParsedProject) -> EvaluationResult {
    evaluate_with_source(set, &[], project)
}

/// Public helper: evaluate with the original rules (so `when` is
/// available, and findings carry the real rule id).
pub fn evaluate_with_source(
    set: &CompiledRuleSet,
    source: &[Rule],
    project: &ParsedProject,
) -> EvaluationResult {
    let mut out = EvaluationResult::default();
    for rule in set.rules.iter() {
        let original = source.iter().find(|r| r.id == rule.id);
        match evaluate_rule(rule, original, project) {
            Ok(findings) => {
                out.findings.extend(findings);
                out.evaluated_rules += 1;
            }
            Err(e) => {
                tracing::debug!(rule_id = %rule.id, error = %e, "rule evaluation failed");
                out.errors.push(e);
                out.skipped_rules += 1;
            }
        }
    }
    sort(&mut out.findings);
    out
}

fn evaluate_rule(
    rule: &CompiledRule,
    original: Option<&Rule>,
    project: &ParsedProject,
) -> Result<Vec<Finding>, EngineError> {
    if !rule.enabled_from(original) {
        return Ok(Vec::new());
    }
    let mut candidates = target::candidates(project, &rule.target);
    target::sort(&mut candidates);

    let mut out = Vec::new();
    for cand in candidates {
        let ctx = EvalContext::new(project, cand.target.clone());
        if let Some(m) = target::match_clause(&rule.target) {
            if !condition::evaluate(&ctx, m)? {
                continue;
            }
        }
        if let Some(orig) = original {
            if let Some(when) = &orig.when {
                if !condition::evaluate(&ctx, when)? {
                    continue;
                }
            }
        }
        out.extend(check_assert(rule, &ctx, &rule.assert)?);
    }
    Ok(out)
}

fn check_assert(
    rule: &CompiledRule,
    ctx: &EvalContext<'_>,
    cond: &Condition,
) -> Result<Vec<Finding>, EngineError> {
    let mut findings = Vec::new();
    visit(rule, cond, ctx, &mut findings)?;
    Ok(findings)
}

fn visit(
    rule: &CompiledRule,
    cond: &Condition,
    ctx: &EvalContext<'_>,
    findings: &mut Vec<Finding>,
) -> Result<(), EngineError> {
    match cond {
        Condition::Single(s) => {
            // The resolver needs to know the expected value to produce
            // a meaningful `actual` for facts that depend on it (e.g.
            // `project.required-files`). Set it on the context, resolve,
            // then clear.
            let previous_expected = ctx.current_expected.replace(s.value.clone());
            let result: Result<(), EngineError> = if condition::evaluate(ctx, cond)? {
                Ok(())
            } else {
                let actual = crate::facts::resolve(ctx, &s.fact);
                let expected = s.value.clone();
                let finding = Finding {
                    rule_id: rule.id.clone(),
                    severity: rule.severity,
                    title: rule.title.clone(),
                    message: crate::template::render(
                        &rule.message,
                        ctx,
                        Some(&actual),
                        expected.as_ref(),
                    ),
                    recommendation: rule
                        .recommendation
                        .as_ref()
                        .map(|t| crate::template::render(t, ctx, Some(&actual), expected.as_ref())),
                    entity_id: entity_id(ctx),
                    source: source_location(ctx),
                    evidence: serde_json::json!({
                        "operator": s.op.as_str(),
                        "actual": actual,
                        "expected": expected,
                    }),
                    origin: FindingOrigin::DeterministicRule,
                    rule_version: Some(rule.version.clone()),
                };
                findings.push(finding);
                Ok(())
            };
            ctx.current_expected.replace(previous_expected);
            result
        }
        Condition::Combinator(c) => match c {
            runnerguard_model::ConditionCombinator::All { all } => {
                for child in all {
                    visit(rule, child, ctx, findings)?;
                }
                Ok(())
            }
            runnerguard_model::ConditionCombinator::Any { any } => {
                for child in any {
                    if condition::evaluate(ctx, child)? {
                        return Ok(());
                    }
                }
                let finding = Finding {
                    rule_id: rule.id.clone(),
                    severity: rule.severity,
                    title: rule.title.clone(),
                    message: format!("any branch of rule `{}` failed", rule.id),
                    recommendation: rule.recommendation.clone(),
                    entity_id: entity_id(ctx),
                    source: source_location(ctx),
                    evidence: serde_json::json!({"operator": "any"}),
                    origin: FindingOrigin::DeterministicRule,
                    rule_version: Some(rule.version.clone()),
                };
                findings.push(finding);
                Ok(())
            }
            runnerguard_model::ConditionCombinator::Not { not } => {
                if condition::evaluate(ctx, not)? {
                    let finding = Finding {
                        rule_id: rule.id.clone(),
                        severity: rule.severity,
                        title: rule.title.clone(),
                        message: format!("not branch of rule `{}` unexpectedly matched", rule.id),
                        recommendation: rule.recommendation.clone(),
                        entity_id: entity_id(ctx),
                        source: source_location(ctx),
                        evidence: serde_json::json!({"operator": "not"}),
                        origin: FindingOrigin::DeterministicRule,
                        rule_version: Some(rule.version.clone()),
                    };
                    findings.push(finding);
                }
                Ok(())
            }
        },
    }
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

fn sort(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| source_file(a).cmp(&source_file(b)))
            .then_with(|| source_line(a).cmp(&source_line(b)))
            .then_with(|| entity_id_str(a).cmp(&entity_id_str(b)))
    });
}

fn source_file(f: &Finding) -> String {
    f.source
        .as_ref()
        .map(|s| s.file.clone())
        .unwrap_or_default()
}
fn source_line(f: &Finding) -> u32 {
    f.source.as_ref().map(|s| s.start_line).unwrap_or(0)
}
fn entity_id_str(f: &Finding) -> String {
    f.entity_id.clone().unwrap_or_default()
}

#[allow(dead_code)]
fn evidence_of(f: &Finding) -> &Value {
    &f.evidence
}

trait RuleMeta {
    fn enabled_from(&self, original: Option<&Rule>) -> bool;
}

impl RuleMeta for CompiledRule {
    fn enabled_from(&self, original: Option<&Rule>) -> bool {
        original.map(|r| r.enabled).unwrap_or(true)
    }
}
