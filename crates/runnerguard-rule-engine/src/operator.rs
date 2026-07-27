//! Operator implementations: the leaf work the rule engine performs.
//!
//! Every operator is a pure function of (`actual`, `expected`) where
//! `actual` is the resolved fact and `expected` is the value written in
//! the rule file. Operators that need extra context (e.g. the flow for
//! `contains-component`) take an [`EvalContext`].
//!
//! Two facts are deliberately treated as missing-equivalent: the absence
//! of a fact resolves to [`Value::Null`], and `exists` / `not-exists` are
//! the only operators that distinguish a missing fact from any other
//! value.
//!
//! **Semantic convention**: each operator returns `true` when the
//! assertion fails (i.e. the operator "fires" and a finding is emitted).
//! `equals(actual="x", expected="y")` returns true because the assertion
//! "actual equals expected" is false.

use crate::context::{EvalContext, EvalTarget};
use crate::error::EngineError;
use regex::Regex;
use runnerguard_model::{FlowRef, MuleComponent, Operator, ParsedProject};
use serde_json::Value;
use std::collections::HashSet;

/// Evaluate an operator against the current context. Returns `Ok(true)`
/// when the assertion fails (the rule's `assert` clause is violated).
pub fn evaluate(
    op: Operator,
    actual: &Value,
    expected: &Option<Value>,
    ctx: &EvalContext<'_>,
) -> Result<bool, EngineError> {
    match op {
        Operator::Exists => Ok(actual.is_null()),
        Operator::NotExists => Ok(!actual.is_null()),
        Operator::Equals => op_equals(actual, expected),
        Operator::NotEquals => op_not_equals(actual, expected),
        Operator::Matches => op_matches_fires_when(actual, expected, false),
        Operator::NotMatches => op_matches_fires_when(actual, expected, true),
        Operator::Contains => op_contains_fires_when(actual, expected, false),
        Operator::NotContains => op_contains_fires_when(actual, expected, true),
        Operator::In => op_in_fires_when(actual, expected, false),
        Operator::NotIn => op_in_fires_when(actual, expected, true),
        Operator::GreaterThan => op_compare_ord(actual, expected, OrdOp::Le),
        Operator::GreaterThanOrEqual => op_compare_ord(actual, expected, OrdOp::Lt),
        Operator::LessThan => op_compare_ord(actual, expected, OrdOp::Ge),
        Operator::LessThanOrEqual => op_compare_ord(actual, expected, OrdOp::Gt),
        Operator::CountEquals => op_count(actual, expected, CountOp::Eq),
        Operator::CountLessThanOrEqual => op_count(actual, expected, CountOp::Le),
        Operator::IsPlaceholder => Ok(!is_placeholder(string_value(actual))),
        Operator::IsNotPlaceholder => Ok(is_placeholder(string_value(actual))),
        Operator::ContainsComponent => op_contains_component(ctx, expected, false),
        Operator::NotContainsComponent => op_contains_component(ctx, expected, true),
        Operator::ReferenceResolves => Ok(!actual.as_bool().unwrap_or(false)),
        Operator::AllReferencesResolve => Ok(!all_references_resolve(ctx)),
        Operator::RequiredFilesExist => Ok(!required_files_exist(ctx, expected)),
    }
}

pub fn operator_label(op: Operator) -> &'static str {
    op.as_str()
}

fn string_value(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn op_equals(actual: &Value, expected: &Option<Value>) -> Result<bool, EngineError> {
    let e = required(expected, "equals")?;
    Ok(actual != e)
}

fn op_not_equals(actual: &Value, expected: &Option<Value>) -> Result<bool, EngineError> {
    let e = required(expected, "not-equals")?;
    Ok(actual == e)
}

fn op_matches_fires_when(
    actual: &Value,
    expected: &Option<Value>,
    fires_when_matches: bool,
) -> Result<bool, EngineError> {
    let pattern = string_arg(expected, "matches")?;
    let regex = Regex::new(pattern).map_err(|e| EngineError::Evaluate {
        rule_id: String::new(),
        message: format!("invalid regex `{pattern}`: {e}"),
    })?;
    let actual_str = string_value(actual);
    let matches = regex.is_match(actual_str);
    Ok(if fires_when_matches {
        matches
    } else {
        !matches
    })
}

fn op_contains_fires_when(
    actual: &Value,
    expected: &Option<Value>,
    fires_when_contains: bool,
) -> Result<bool, EngineError> {
    let needle = string_arg(expected, "contains")?;
    let contains = match actual {
        Value::String(s) => s.contains(needle),
        Value::Array(arr) => arr.iter().any(|v| v == &Value::String(needle.to_string())),
        _ => false,
    };
    Ok(if fires_when_contains {
        contains
    } else {
        !contains
    })
}

fn op_in_fires_when(
    actual: &Value,
    expected: &Option<Value>,
    fires_when_in: bool,
) -> Result<bool, EngineError> {
    let options = expected
        .as_ref()
        .and_then(|v| v.as_array())
        .ok_or_else(|| EngineError::Evaluate {
            rule_id: String::new(),
            message: format!("`in` requires an array of values, got {actual:?}"),
        })?;
    let is_in = options.iter().any(|v| v == actual);
    Ok(if fires_when_in { is_in } else { !is_in })
}

#[derive(Copy, Clone)]
enum OrdOp {
    Gt,
    Ge,
    Lt,
    Le,
}

fn op_compare_ord(
    actual: &Value,
    expected: &Option<Value>,
    op: OrdOp,
) -> Result<bool, EngineError> {
    let actual_n = numeric(actual).ok_or_else(|| EngineError::Evaluate {
        rule_id: String::new(),
        message: format!("comparison operator requires a numeric actual, got {actual:?}"),
    })?;
    let expected_n = match expected {
        Some(v) => numeric(v).ok_or_else(|| EngineError::Evaluate {
            rule_id: String::new(),
            message: format!("comparison operator requires a numeric expected, got {v:?}"),
        })?,
        None => {
            return Err(EngineError::Evaluate {
                rule_id: String::new(),
                message: "comparison operator requires an expected value".to_string(),
            });
        }
    };
    let holds = match op {
        OrdOp::Gt => actual_n > expected_n,
        OrdOp::Ge => actual_n >= expected_n,
        OrdOp::Lt => actual_n < expected_n,
        OrdOp::Le => actual_n <= expected_n,
    };
    // We get the *failing* form here (e.g. GreaterThan uses Le, so
    // GreaterThan fires when actual <= expected). `holds` is whether
    // the inverse relation holds; we fire when it does.
    Ok(holds)
}

#[derive(Copy, Clone)]
enum CountOp {
    Eq,
    Le,
}

fn op_count(actual: &Value, expected: &Option<Value>, op: CountOp) -> Result<bool, EngineError> {
    let count = match actual {
        Value::Array(a) => a.len() as i64,
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::Null => 0,
        Value::String(s) => s.len() as i64,
        _ => 1,
    };
    let expected_n = match expected {
        Some(v) => numeric(v).ok_or_else(|| EngineError::Evaluate {
            rule_id: String::new(),
            message: format!("count operator requires a numeric expected, got {v:?}"),
        })?,
        None => {
            return Err(EngineError::Evaluate {
                rule_id: String::new(),
                message: "count operator requires an expected value".to_string(),
            });
        }
    };
    let holds = match op {
        CountOp::Eq => count == expected_n,
        CountOp::Le => count <= expected_n,
    };
    Ok(holds)
}

fn is_placeholder(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.starts_with("${") && trimmed.ends_with('}')
}

fn op_contains_component(
    ctx: &EvalContext<'_>,
    expected: &Option<Value>,
    fires_when_present: bool,
) -> Result<bool, EngineError> {
    let needle = string_arg(expected, "contains-component")?;
    let present = match &ctx.target {
        EvalTarget::Flow(flow) => flow_contains(flow, needle),
        _ => false,
    };
    Ok(if fires_when_present {
        present
    } else {
        !present
    })
}

fn flow_contains(flow: &runnerguard_model::MuleFlow, needle: &str) -> bool {
    flow.components.iter().any(|c| tree_contains(c, needle))
}

fn tree_contains(c: &MuleComponent, needle: &str) -> bool {
    if c.qualified_name == needle || c.local_name == needle {
        return true;
    }
    c.children.iter().any(|child| tree_contains(child, needle))
}

fn all_references_resolve(ctx: &EvalContext<'_>) -> bool {
    let project: &ParsedProject = ctx.project;
    let flow_refs: Vec<&FlowRef> = match &ctx.target {
        EvalTarget::Flow(f) => f.facts.flow_refs.iter().collect(),
        EvalTarget::File(_) | EvalTarget::Component(_) => project
            .documents
            .iter()
            .flat_map(|d| d.flows.iter().chain(d.sub_flows.iter()))
            .flat_map(|f| f.facts.flow_refs.iter())
            .collect(),
        _ => Vec::new(),
    };
    flow_refs.iter().all(|r| r.resolved)
}

fn required_files_exist(ctx: &EvalContext<'_>, expected: &Option<Value>) -> bool {
    let Some(required) = expected.as_ref().and_then(|v| v.as_array()) else {
        return true;
    };
    let present: Vec<&str> = ctx
        .project
        .project
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    for req in required {
        let Some(req_str) = req.as_str() else {
            continue;
        };
        let satisfied = if let Some(stripped) = req_str.strip_suffix('/') {
            present.iter().any(|p| p.starts_with(stripped))
        } else {
            present.iter().any(|p| p == &req_str)
        };
        if !satisfied {
            return false;
        }
    }
    true
}

fn numeric(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn required<'a>(expected: &'a Option<Value>, op: &str) -> Result<&'a Value, EngineError> {
    expected.as_ref().ok_or_else(|| EngineError::Evaluate {
        rule_id: String::new(),
        message: format!("`{op}` requires an expected value"),
    })
}

fn string_arg<'a>(expected: &'a Option<Value>, op: &str) -> Result<&'a str, EngineError> {
    match expected {
        Some(Value::String(s)) => Ok(s.as_str()),
        Some(other) => Err(EngineError::Evaluate {
            rule_id: String::new(),
            message: format!("`{op}` requires a string value, got {other:?}"),
        }),
        None => Err(EngineError::Evaluate {
            rule_id: String::new(),
            message: format!("`{op}` requires an expected value"),
        }),
    }
}

#[allow(dead_code)]
fn distinct_values(values: &[Value]) -> HashSet<&str> {
    values.iter().filter_map(|v| v.as_str()).collect()
}
