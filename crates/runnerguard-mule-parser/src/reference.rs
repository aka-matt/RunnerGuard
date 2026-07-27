//! Walk a tree of MuleComponents and mark each `flow-ref` as resolved or
//! not based on the supplied set of known flow / subflow names.
//!
//! Mutates each `FlowFacts::flow_refs` entry in place so the rule engine
//! sees the final state. The `mark_resolved` function is invoked from
//! [`crate::parser`] after the project-wide name set has been collected
//! from every parsed document.

use runnerguard_model::{FlowRef, MuleComponent, MuleDocument};
use std::collections::HashSet;

/// Build the set of all known flow / subflow names across the parsed
/// documents.
pub fn collect_known_names(documents: &[MuleDocument]) -> HashSet<String> {
    let mut names = HashSet::new();
    for doc in documents {
        for flow in &doc.flows {
            names.insert(flow.name.clone());
        }
        for sub in &doc.sub_flows {
            names.insert(sub.name.clone());
        }
    }
    names
}

/// Mark each `flow-ref` in `flow`'s component tree as resolved / unresolved
/// against `known_names`, and rewrite the matching `FlowFacts::flow_refs`
/// entry to carry the same `resolved` flag.
pub fn mark_resolved_in_flow(
    flow: &mut MuleComponent,
    facts: &mut Vec<FlowRef>,
    known_names: &HashSet<String>,
) {
    let target = flow.attributes.get("name").cloned().unwrap_or_default();
    if flow.local_name == "flow-ref" {
        let resolved = known_names.contains(&target);
        if let Some(r) = facts.iter_mut().find(|r| r.target == target) {
            r.resolved = resolved;
        }
    }
    for child in &mut flow.children {
        mark_resolved_in_flow(child, facts, known_names);
    }
}
