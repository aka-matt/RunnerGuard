//! Compile a [`RuleSet`] into a [`CompiledRuleSet`].
//!
//! Compilation performs all validation that can be done without the
//! parsed project: fact-path whitelisting, regex pre-compilation for
//! `matches`, and required-value checks. The output is what
//! [`engine::evaluate`] consumes.

use crate::condition;
use crate::error::{CompileIssue, EngineError};
use crate::path;
use runnerguard_model::{FactPath, Operator, Rule, RuleSet, SingleCondition};
use std::collections::BTreeSet;

/// Compiled view of one rule.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub severity: runnerguard_model::Severity,
    pub tags: Vec<String>,
    pub target: runnerguard_model::RuleTarget,
    pub assert: runnerguard_model::Condition,
    pub message: String,
    pub recommendation: Option<String>,
    pub version: String,
    pub when_facts: Vec<FactPath>,
    pub assert_facts: Vec<FactPath>,
}

/// Compiled rule set, ready for evaluation.
#[derive(Debug, Clone)]
pub struct CompiledRuleSet {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub rules: Vec<CompiledRule>,
    pub issues: Vec<CompileIssue>,
}

/// Compile a rule set. Compilation **never** aborts on the first error —
/// every problem is collected so the caller can show them all at once.
pub fn compile(set: &RuleSet) -> CompiledRuleSet {
    path::warm();
    let mut rules = Vec::new();
    let mut issues = Vec::new();
    for rule in &set.rules {
        match compile_one(set, rule) {
            Ok(r) => rules.push(r),
            Err(e) => issues.push(CompileIssue {
                rule_id: rule.id.clone(),
                message: e.to_string(),
            }),
        }
    }
    CompiledRuleSet {
        id: set.id.clone(),
        name: set.name.clone(),
        version: set.version.clone(),
        description: set.description.clone(),
        rules,
        issues,
    }
}

fn compile_one(set: &RuleSet, rule: &Rule) -> Result<CompiledRule, EngineError> {
    if rule.id.is_empty() {
        return Err(EngineError::Compile {
            rule_id: rule.id.clone(),
            message: "rule id must not be empty".to_string(),
        });
    }

    if rule.target.entity.is_empty() {
        return Err(EngineError::Compile {
            rule_id: rule.id.clone(),
            message: "rule target must list at least one entity".to_string(),
        });
    }

    let mut when_facts = Vec::new();
    if let Some(when) = &rule.when {
        condition::collect_facts(when, &mut when_facts);
    }
    let mut assert_facts = Vec::new();
    condition::collect_facts(&rule.assert, &mut assert_facts);

    let checked = dedup(when_facts.iter().chain(assert_facts.iter()));
    for fact in checked {
        ensure_paths_allowed(rule, &fact)?;
    }
    ensure_values_present(rule, &rule.assert)?;
    if let Some(when) = &rule.when {
        ensure_values_present(rule, when)?;
    }

    Ok(CompiledRule {
        id: rule.id.clone(),
        title: rule.title.clone(),
        description: rule.description.clone(),
        severity: rule.severity,
        tags: rule.tags.clone(),
        target: rule.target.clone(),
        assert: rule.assert.clone(),
        message: rule.message.clone(),
        recommendation: rule.recommendation.clone(),
        version: set.version.clone(),
        when_facts,
        assert_facts,
    })
}

fn dedup<'a>(iter: impl Iterator<Item = &'a FactPath>) -> Vec<FactPath> {
    let mut set = BTreeSet::new();
    let mut out = Vec::new();
    for f in iter {
        if set.insert(f.clone()) {
            out.push(f.clone());
        }
    }
    out
}

fn ensure_paths_allowed(rule: &Rule, fact: &FactPath) -> Result<(), EngineError> {
    let target_entity = rule.target.entity.first().copied().expect("checked above");
    path::ensure_allowed(target_entity, fact).map_err(|_| EngineError::Compile {
        rule_id: rule.id.clone(),
        message: format!("fact path `{fact}` is not allowed for target `{target_entity:?}`"),
    })
}

fn ensure_values_present(
    rule: &Rule,
    cond: &runnerguard_model::Condition,
) -> Result<(), EngineError> {
    walk(cond, &mut |single: &SingleCondition| {
        if condition::requires_value(single.op) && single.value.is_none() {
            Err(EngineError::Compile {
                rule_id: rule.id.clone(),
                message: format!("operator `{}` requires a `value` field", single.op.as_str()),
            })
        } else {
            Ok(())
        }
    })
}

fn walk(
    cond: &runnerguard_model::Condition,
    visit: &mut dyn FnMut(&SingleCondition) -> Result<(), EngineError>,
) -> Result<(), EngineError> {
    match cond {
        runnerguard_model::Condition::Single(s) => visit(s),
        runnerguard_model::Condition::Combinator(c) => match c {
            runnerguard_model::ConditionCombinator::All { all } => {
                for child in all {
                    walk(child, visit)?;
                }
                Ok(())
            }
            runnerguard_model::ConditionCombinator::Any { any } => {
                for child in any {
                    walk(child, visit)?;
                }
                Ok(())
            }
            runnerguard_model::ConditionCombinator::Not { not } => walk(not, visit),
        },
    }
}

#[allow(dead_code)]
fn severity_override(
    rule: &Rule,
    default: runnerguard_model::Severity,
) -> runnerguard_model::Severity {
    if matches!(rule.severity, runnerguard_model::Severity::Info)
        && default != runnerguard_model::Severity::Info
    {
        // the rule kept the default; honour the set-level override
        default
    } else {
        rule.severity
    }
}

#[allow(dead_code)]
fn operator_label(op: Operator) -> &'static str {
    op.as_str()
}
