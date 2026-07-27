//! Resolve fact paths against an [`EvalContext`].
//!
//! Every fact the rule engine reads comes through this module. The result
//! is a [`serde_json::Value`] so operators can use one comparison path
//! regardless of the underlying type. Missing facts resolve to
//! [`Value::Null`] — that is the signal `exists`/`not-exists` checks.

use crate::context::{EvalContext, EvalTarget};
use runnerguard_model::{FactPath, MuleComponent, SourceFile};
use serde_json::{Number, Value};
use std::path::Path;

pub fn resolve(ctx: &EvalContext<'_>, path: &FactPath) -> Value {
    match &ctx.target {
        EvalTarget::Project(_) => resolve_project(ctx, path),
        EvalTarget::File(f) => resolve_file(ctx, f, path),
        EvalTarget::Flow(_) => resolve_flow(ctx, path),
        EvalTarget::Component(_) => resolve_component(ctx, path),
        EvalTarget::PropertyRef { value, .. } => resolve_property(value, path),
        EvalTarget::FlowRef(r) => resolve_reference(r, path),
        EvalTarget::DataweaveBlock(b) => resolve_dataweave(b, path),
    }
}

fn resolve_project(ctx: &EvalContext<'_>, path: &FactPath) -> Value {
    let segments = split(path);
    match segments.as_slice() {
        ["project", "name"] => Value::String(ctx.project.project.name.clone()),
        ["project", "required-files"] => {
            // The "missing files" list depends on the rule's expected
            // value: the operator walks the rule's value and reports
            // whatever entries are not satisfied by the discovered file
            // list. We mirror that here so `{{ actual }}` shows the same
            // list the operator computed.
            let present: Vec<&str> = ctx
                .project
                .project
                .files
                .iter()
                .map(|f| f.path.as_str())
                .collect();
            let expected_borrow = ctx.current_expected.borrow();
            let missing: Vec<Value> = expected_borrow
                .as_ref()
                .and_then(|v| v.as_array())
                .map(|required| {
                    required
                        .iter()
                        .filter_map(|req| {
                            let s = req.as_str()?;
                            let satisfied = if let Some(stripped) = s.strip_suffix('/') {
                                present.iter().any(|p| p.starts_with(stripped))
                            } else {
                                present.iter().any(|p| *p == s)
                            };
                            if satisfied {
                                None
                            } else {
                                Some(Value::String(s.to_string()))
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            Value::Array(missing)
        }
        ["project", "unresolved-flow-refs"] => {
            Value::Number(Number::from(ctx.project.index.unresolved_flow_refs.len()))
        }
        ["project", "flow-count"] => {
            let n: usize = ctx.project.documents.iter().map(|d| d.flows.len()).sum();
            Value::Number(Number::from(n))
        }
        ["project", "subflow-count"] => {
            let n: usize = ctx
                .project
                .documents
                .iter()
                .map(|d| d.sub_flows.len())
                .sum();
            Value::Number(Number::from(n))
        }
        _ => Value::Null,
    }
}

fn resolve_file(_ctx: &EvalContext<'_>, file: &SourceFile, path: &FactPath) -> Value {
    let segments = split(path);
    match segments.as_slice() {
        ["file", "path"] => Value::String(file.path.clone()),
        ["file", "extension"] => Value::String(
            Path::new(&file.path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        ),
        ["file", "kind"] => Value::String(file.kind.label().to_string()),
        _ => Value::Null,
    }
}

fn resolve_flow(ctx: &EvalContext<'_>, path: &FactPath) -> Value {
    let EvalTarget::Flow(flow) = &ctx.target else {
        return Value::Null;
    };
    let segments = split(path);
    match segments.as_slice() {
        ["flow", "name"] => Value::String(flow.name.clone()),
        ["flow", "kind"] => Value::String(flow.kind.as_str().to_string()),
        ["flow", "component-count"] => Value::Number(Number::from(flow.facts.component_count)),
        ["flow", "max-component-depth"] => {
            Value::Number(Number::from(flow.facts.max_component_depth))
        }
        ["flow", "has-local-error-handler"] => Value::Bool(flow.facts.has_local_error_handler),
        ["flow", "has-effective-error-handler"] => {
            Value::Bool(flow.facts.has_effective_error_handler)
        }
        ["flow", "flow-refs"] => Value::Number(Number::from(flow.facts.flow_refs.len())),
        ["flow", "property-refs"] => Value::Number(Number::from(flow.facts.property_refs.len())),
        ["flow", "source-component"] => match &flow.facts.source_component {
            Some(s) => Value::String(s.clone()),
            None => Value::Null,
        },
        _ => Value::Null,
    }
}

fn resolve_component(ctx: &EvalContext<'_>, path: &FactPath) -> Value {
    let EvalTarget::Component(c) = &ctx.target else {
        return Value::Null;
    };
    let segments = split(path);
    match segments.as_slice() {
        ["component", "qualified-name"] => Value::String(c.qualified_name.clone()),
        ["component", "local-name"] => Value::String(c.local_name.clone()),
        ["component", "namespace-uri"] => match &c.namespace_uri {
            Some(s) => Value::String(s.clone()),
            None => Value::Null,
        },
        ["component", "text"] => match &c.text {
            Some(s) => Value::String(s.clone()),
            None => Value::Null,
        },
        ["component", "cdata"] => {
            Value::Array(c.cdata.iter().cloned().map(Value::String).collect())
        }
        ["component", "attributes", rest @ ..] if !rest.is_empty() => {
            let key = rest.join(".");
            c.attributes
                .get(&key)
                .map(|v| Value::String(v.clone()))
                .unwrap_or(Value::Null)
        }
        _ => match_walk_component(ctx, c, path),
    }
}

fn match_walk_component(ctx: &EvalContext<'_>, root: &MuleComponent, path: &FactPath) -> Value {
    // Allow dotted attribute access by scanning the whole tree.
    let segments = split(path);
    if let ["component", "attributes", rest @ ..] = segments.as_slice() {
        if rest.is_empty() {
            return Value::Object(
                root.attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect(),
            );
        }
        let needle = rest.join(".");
        if let Some(v) = root.attributes.get(&needle) {
            return Value::String(v.clone());
        }
        walk_components(root, &mut |c| {
            if c.attributes.contains_key(&needle) {
                // We can't early-return from this closure, but the first
                // hit wins because the walker is depth-first.
            }
        });
        // Linear scan fallback (also covers attribute lookups inside
        // nested scopes).
        if let Some(value) = find_attribute(root, &needle) {
            return Value::String(value);
        }
    }
    let _ = ctx;
    Value::Null
}

fn walk_components<F: FnMut(&MuleComponent)>(root: &MuleComponent, f: &mut F) {
    f(root);
    for child in &root.children {
        walk_components(child, f);
    }
}

fn find_attribute(root: &MuleComponent, key: &str) -> Option<String> {
    if let Some(v) = root.attributes.get(key) {
        return Some(v.clone());
    }
    for child in &root.children {
        if let Some(v) = find_attribute(child, key) {
            return Some(v);
        }
    }
    None
}

fn resolve_property(value: &str, path: &FactPath) -> Value {
    let segments = split(path);
    match segments.as_slice() {
        ["property", "name"] | ["property", "value"] => Value::String(value.to_string()),
        _ => Value::Null,
    }
}

fn resolve_reference(r: &runnerguard_model::FlowRef, path: &FactPath) -> Value {
    let segments = split(path);
    match segments.as_slice() {
        ["reference", "target"] => Value::String(r.target.clone()),
        ["reference", "resolved"] => Value::Bool(r.resolved),
        _ => Value::Null,
    }
}

fn resolve_dataweave(b: &runnerguard_model::DataWeaveBlock, path: &FactPath) -> Value {
    let segments = split(path);
    match segments.as_slice() {
        ["dataweave", "language"] => Value::String(b.language.clone()),
        ["dataweave", "content"] => Value::String(b.content.clone()),
        _ => Value::Null,
    }
}

fn split(path: &FactPath) -> Vec<&str> {
    path.as_str().split('.').collect()
}
