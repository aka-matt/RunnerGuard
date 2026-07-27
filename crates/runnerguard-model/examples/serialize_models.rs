//! Demonstrates round-tripping every shared model type through serde.
//!
//! Run with: `cargo run -p runnerguard-model --example serialize_models`.

use runnerguard_model::{
    CURRENT_SCHEMA_VERSION, Finding, Rule, RuleDefaults, RuleSet, RuleTarget, RuleTargetEntity,
    SecretRef, Severity, SingleCondition,
};
use serde_json::json;

fn main() -> Result<(), serde_json::Error> {
    let rule = Rule {
        id: "DEMO-001".to_string(),
        title: "Demo rule".to_string(),
        description: Some("Round-trips through serde".to_string()),
        severity: Severity::Warning,
        enabled: true,
        tags: vec!["demo".to_string()],
        target: RuleTarget {
            entity: vec![RuleTargetEntity::Flow],
            match_: None,
        },
        when: None,
        r#assert: SingleCondition {
            fact: runnerguard_model::FactPath("flow.name".to_string()),
            op: runnerguard_model::Operator::Exists,
            value: None,
        }
        .into(),
        message: "Flow must have a name".to_string(),
        recommendation: None,
    };

    let rule_set = RuleSet {
        schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        id: "demo-rules".to_string(),
        name: "Demo".to_string(),
        version: "1.0.0".to_string(),
        description: Some("Round-trip demo".to_string()),
        defaults: RuleDefaults::default(),
        rules: vec![rule],
    };

    let serialized = serde_json::to_string_pretty(&rule_set)?;
    let parsed: RuleSet = serde_json::from_str(&serialized)?;
    assert_eq!(parsed.rules.len(), 1);

    let finding = Finding::deterministic("DEMO-001", Severity::Info, "demo", "round-tripped");
    let finding_json = serde_json::to_value(&finding)?;
    assert_eq!(finding_json["rule_id"], json!("DEMO-001"));

    // SecretRef Display must always redact, regardless of source.
    let secret = SecretRef::Plain {
        value: "should-never-appear".to_string(),
    };
    println!("secret debug: {secret:?}");
    println!("secret display: {secret}");
    println!("rule set JSON:\n{serialized}");
    Ok(())
}
