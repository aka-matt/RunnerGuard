//! Boolean evaluation of the [`Condition`] AST.

use crate::context::EvalContext;
use crate::error::EngineError;
use crate::facts;
use crate::operator;
use runnerguard_model::{Condition, ConditionCombinator, FactPath, Operator, SingleCondition};

/// Evaluate a condition tree against a context. Returns `Ok(true)` when
/// every `assert` clause is satisfied (i.e. no operator fires), which is
/// how the rest of the engine talks about "the rule passed".
pub fn evaluate(ctx: &EvalContext<'_>, cond: &Condition) -> Result<bool, EngineError> {
    match cond {
        Condition::Single(s) => evaluate_single(ctx, s),
        Condition::Combinator(c) => evaluate_combinator(ctx, c),
    }
}

fn evaluate_single(ctx: &EvalContext<'_>, single: &SingleCondition) -> Result<bool, EngineError> {
    let actual = facts::resolve(ctx, &single.fact);
    let fires = operator::evaluate(single.op, &actual, &single.value, ctx)?;
    // The operator returns `true` when the assertion fails. We invert so
    // the result is "the assertion held".
    Ok(!fires)
}

fn evaluate_combinator(
    ctx: &EvalContext<'_>,
    combo: &ConditionCombinator,
) -> Result<bool, EngineError> {
    match combo {
        ConditionCombinator::All { all } => eval_all(ctx, all),
        ConditionCombinator::Any { any } => eval_any(ctx, any),
        ConditionCombinator::Not { not } => Ok(!evaluate(ctx, not)?),
    }
}

fn eval_all(ctx: &EvalContext<'_>, conds: &[Condition]) -> Result<bool, EngineError> {
    for c in conds {
        if !evaluate(ctx, c)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn eval_any(ctx: &EvalContext<'_>, conds: &[Condition]) -> Result<bool, EngineError> {
    for c in conds {
        if evaluate(ctx, c)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Collect every leaf fact path from a condition tree. Used both at
/// compile time (to whitelist-check) and to build the cache key for
/// repeat runs.
pub fn collect_facts(cond: &Condition, out: &mut Vec<FactPath>) {
    match cond {
        Condition::Single(s) => out.push(s.fact.clone()),
        Condition::Combinator(c) => match c {
            ConditionCombinator::All { all } => {
                for child in all {
                    collect_facts(child, out);
                }
            }
            ConditionCombinator::Any { any } => {
                for child in any {
                    collect_facts(child, out);
                }
            }
            ConditionCombinator::Not { not } => collect_facts(not, out),
        },
    }
}

/// Operators that demand a literal `value` field.
pub fn requires_value(op: Operator) -> bool {
    matches!(
        op,
        Operator::Equals
            | Operator::NotEquals
            | Operator::Matches
            | Operator::NotMatches
            | Operator::Contains
            | Operator::NotContains
            | Operator::In
            | Operator::NotIn
            | Operator::GreaterThan
            | Operator::GreaterThanOrEqual
            | Operator::LessThan
            | Operator::LessThanOrEqual
            | Operator::CountEquals
            | Operator::CountLessThanOrEqual
            | Operator::ContainsComponent
            | Operator::NotContainsComponent
    )
}
