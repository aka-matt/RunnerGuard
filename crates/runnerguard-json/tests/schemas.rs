//! Integration tests that load the JSON Schemas under `schemas/` and
//! validate the bundled rule pack and AI response fixtures against them.
//!
//! These tests are the "do the docs still match the code" guard. They
//! catch breakages where the schema, the model, and the example JSON
//! drift out of sync.

use runnerguard_json::{DefaultJsonCodec, DefaultSchemaValidator, JsonCodec, SchemaValidator};
use runnerguard_model::RuleSet;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn load_schema(rel: &str) -> serde_json::Value {
    let path = workspace_root().join(rel);
    let bytes = std::fs::read(&path).expect("schema file");
    serde_json::from_slice(&bytes).expect("schema parse")
}

fn load_json(rel: &str) -> serde_json::Value {
    let path = workspace_root().join(rel);
    let bytes = std::fs::read(&path).expect("json file");
    serde_json::from_slice(&bytes).expect("json parse")
}

#[test]
fn rule_set_schema_validates_basic_rules() {
    let schema = load_schema("schemas/rule-set.schema.json");
    let instance = load_json("rules/basic.json");
    let v = DefaultSchemaValidator;
    v.validate(&schema, &instance)
        .expect("rules/basic.json should validate against rule-set.schema.json");
}

#[test]
fn rule_set_schema_rejects_unknown_operator() {
    let schema = load_schema("schemas/rule-set.schema.json");
    let mut instance = load_json("rules/basic.json");
    // Plant an unknown operator and drop a rule so the schema check can
    // find it.
    instance["rules"][0]["assert"]["op"] = serde_json::json!("made-up");
    let v = DefaultSchemaValidator;
    let violations = v.validate(&schema, &instance).unwrap_err();
    assert!(
        !violations.is_empty(),
        "expected at least one violation for unknown operator"
    );
}

#[test]
fn rule_set_schema_rejects_missing_required_fields() {
    let schema = load_schema("schemas/rule-set.schema.json");
    let bad = serde_json::json!({
        "schema_version": "1.0",
        "id": "x",
        "name": "x",
        "version": "1.0.0",
        "rules": []
    });
    let v = DefaultSchemaValidator;
    let violations = v.validate(&schema, &bad).unwrap_err();
    assert!(
        !violations.is_empty(),
        "expected at least one violation for empty rules array"
    );
}

#[test]
fn rule_set_schema_rejects_additional_properties() {
    let schema = load_schema("schemas/rule-set.schema.json");
    let mut instance = load_json("rules/basic.json");
    instance["stranger"] = serde_json::json!("not allowed");
    let v = DefaultSchemaValidator;
    let violations = v.validate(&schema, &instance).unwrap_err();
    assert!(
        !violations.is_empty(),
        "expected at least one violation for extra property"
    );
}

#[test]
fn rule_set_schema_supports_combinator_conditions() {
    let schema = load_schema("schemas/rule-set.schema.json");
    let instance = serde_json::json!({
        "schema_version": "1.0",
        "id": "x",
        "name": "x",
        "version": "1.0.0",
        "rules": [
            {
                "id": "DEMO-001",
                "title": "demo",
                "severity": "warning",
                "target": {"entity": ["flow"]},
                "assert": {
                    "all": [
                        {"fact": "flow.name", "op": "exists"},
                        {"any": [
                            {"fact": "flow.name", "op": "not-exists"},
                            {"not": {"fact": "flow.name", "op": "exists"}}
                        ]}
                    ]
                },
                "message": "demo"
            }
        ]
    });
    let v = DefaultSchemaValidator;
    v.validate(&schema, &instance)
        .expect("combinator conditions should validate");
}

#[test]
fn rule_set_round_trip_via_serde() {
    // The input file carries a `$schema` self-reference; the model does
    // not keep that key across a round-trip, so strip it before comparing.
    let mut instance = load_json("rules/basic.json");
    if let Some(obj) = instance.as_object_mut() {
        obj.remove("$schema");
    }
    let set: RuleSet = serde_json::from_value(instance.clone())
        .expect("rules/basic.json should deserialise into RuleSet");
    let re = serde_json::to_value(&set).expect("re-serialise");
    assert_eq!(re, instance, "re-serialised JSON drifted from input");
}

#[test]
fn ai_response_schema_validates_minimal_finding() {
    let schema = load_schema("schemas/ai-response.schema.json");
    let instance = serde_json::json!({
        "schema_version": "1.0",
        "summary": "Looks fine.",
        "suggestions": [
            {
                "id": "AI-X-001",
                "title": "consider this",
                "severity": "info",
                "message": "Body.",
                "target": {"entity": "flow", "name": "order-api-flow"}
            }
        ]
    });
    let v = DefaultSchemaValidator;
    v.validate(&schema, &instance)
        .expect("minimal AI response should validate");
}

#[test]
fn ai_response_schema_rejects_non_ai_id_prefix() {
    let schema = load_schema("schemas/ai-response.schema.json");
    let instance = serde_json::json!({
        "schema_version": "1.0",
        "summary": "x",
        "suggestions": [
            {
                "id": "REGULAR-001",
                "title": "x",
                "severity": "info",
                "message": "x",
                "target": {"entity": "flow"}
            }
        ]
    });
    let v = DefaultSchemaValidator;
    let violations = v.validate(&schema, &instance).unwrap_err();
    assert!(
        !violations.is_empty(),
        "non-AI-prefixed suggestions should be rejected"
    );
}

#[test]
fn finding_schema_validates_deterministic_finding() {
    let schema = load_schema("schemas/finding.schema.json");
    let instance = serde_json::json!({
        "rule_id": "MULE-FLOW-001",
        "severity": "warning",
        "title": "Flow name is not kebab-case",
        "message": "Flow 'OrderApiFlow' does not use kebab-case.",
        "recommendation": "Use a lower-case name.",
        "evidence": {"value": "OrderApiFlow"},
        "origin": "deterministic-rule"
    });
    let v = DefaultSchemaValidator;
    v.validate(&schema, &instance)
        .expect("minimal finding should validate");
}

#[test]
fn flow_schema_validates_a_minimal_flow() {
    let schema = load_schema("schemas/flow.schema.json");
    let instance = serde_json::json!({
        "schema_version": "1.0",
        "id": "demo-flow",
        "project_id": "order-api",
        "kind": "flow",
        "name": "order-api-flow",
        "source": {
            "file": "src/main/mule/order-api.xml",
            "start_line": 1,
            "start_column": 1,
            "end_line": 1,
            "end_column": 1
        },
        "namespaces": {"mule": "http://www.mulesoft.org/schema/mule/core"},
        "attributes": {},
        "components": [],
        "facts": {
            "component_count": 0,
            "max_component_depth": 0,
            "flow_refs": [],
            "property_refs": [],
            "dataweave_blocks": [],
            "has_local_error_handler": false,
            "has_effective_error_handler": false,
            "source_component": null
        },
        "source_xml_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let v = DefaultSchemaValidator;
    v.validate(&schema, &instance)
        .expect("minimal flow should validate");
}

#[test]
fn json_codec_reads_basic_rules() {
    let codec = DefaultJsonCodec;
    let path = workspace_root().join("rules/basic.json");
    let set: RuleSet = codec
        .read_file(&path)
        .expect("codec should load rules/basic.json");
    assert!(
        !set.rules.is_empty(),
        "ruleset should have at least one rule"
    );
}
