//! Compute the pre-computed [`FlowFacts`] for a parsed flow tree.
//!
//! Pre-computing keeps the rule engine from walking the tree repeatedly —
//! `flow.component-count`, `flow.flow-refs`, etc. all read from these.

use runnerguard_model::{DataWeaveBlock, FlowFacts, FlowRef, MuleComponent, SourceSpan};
use sha2::{Digest, Sha256};

pub fn compute(component: &MuleComponent) -> FlowFacts {
    let mut facts = FlowFacts::default();
    walk(component, 1, &mut facts);
    facts
}

fn walk(component: &MuleComponent, depth: usize, facts: &mut FlowFacts) {
    facts.component_count += 1;
    if depth > facts.max_component_depth {
        facts.max_component_depth = depth;
    }

    if let Some(name) = component.attributes.get("name") {
        if component.local_name == "flow-ref" {
            facts.flow_refs.push(FlowRef {
                target: name.clone(),
                resolved: false,
                source: component.source.clone(),
            });
        }
        if component.local_name == "sub-flow" {
            facts.source_component =
                Some(format!("{}:{}", qualified_name_or_local(component), name));
        }
    }

    // Mark a flow as having a local error handler the moment we see one.
    if component.local_name == "error-handler" {
        facts.has_local_error_handler = true;
        facts.has_effective_error_handler = true;
    }

    // Source-component: first message-source style element we find.
    if facts.source_component.is_none() {
        let q = component.qualified_name.as_str();
        if matches!(
            q,
            "http:listener"
                | "http:request"
                | "vm:listener"
                | "vm:inbound-endpoint"
                | "file:listener"
                | "jms:listener"
                | "db:listener"
                | "scheduler:scheduling-strategy"
                | "sub-flow"
        ) {
            facts.source_component = Some(q.to_string());
        }
    }

    for cdata in &component.cdata {
        facts
            .dataweave_blocks
            .push(make_dataweave_block(cdata, &component.source));
    }

    for attr_value in component.attributes.values() {
        for placeholder in scan_placeholders(attr_value) {
            if !facts.property_refs.contains(&placeholder) {
                facts.property_refs.push(placeholder);
            }
        }
    }

    for child in &component.children {
        walk(child, depth + 1, facts);
    }
}

fn qualified_name_or_local(component: &MuleComponent) -> &str {
    if component.qualified_name.is_empty() {
        &component.local_name
    } else {
        &component.qualified_name
    }
}

fn scan_placeholders(value: &str) -> impl Iterator<Item = String> + '_ {
    // Mule property placeholder: ${name}. Only the name is captured.
    let bytes = value.as_bytes();
    let mut i = 0;
    let mut out: Vec<String> = Vec::new();
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' && bytes[i + 1] == b'{' {
            if let Some(end) = value[i + 2..].find('}') {
                let name = &value[i + 2..i + 2 + end];
                out.push(name.to_string());
                i = i + 2 + end + 1;
                continue;
            }
        }
        i += 1;
    }
    out.into_iter()
}

fn make_dataweave_block(content: &str, source: &SourceSpan) -> DataWeaveBlock {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = format!("sha256:{}", hex_lower(&hasher.finalize()));
    let version = if content.contains("%dw 2.0") {
        Some("2.0".to_string())
    } else {
        None
    };
    DataWeaveBlock {
        language: "dataweave".to_string(),
        version,
        content_hash: hash,
        content: content.to_string(),
        source: source.clone(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
