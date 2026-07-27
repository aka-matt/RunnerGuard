//! Round-trip + redaction tests for shared model types.

use runnerguard_model::{
    CURRENT_SCHEMA_VERSION, Diagnostic, DiagnosticLevel, DiagnosticStage, Finding, FindingOrigin,
    Rule, RuleDefaults, RuleSet, RuleTarget, RuleTargetEntity, SecretRef, Severity,
    SingleCondition, SourceSpan,
};
use serde_json::json;

#[test]
fn severity_round_trips_via_kebab_case() {
    let original = Severity::Warning;
    let serialized = serde_json::to_string(&original).unwrap();
    assert_eq!(serialized, "\"warning\"");
    let parsed: Severity = serde_json::from_str(&serialized).unwrap();
    assert_eq!(parsed, original);
}

#[test]
fn finding_deterministic_builder_records_origin() {
    let finding = Finding::deterministic(
        "MULE-FLOW-001",
        Severity::Warning,
        "kebab-case required",
        "Flow name must be kebab-case",
    )
    .with_recommendation("Use lower-case names")
    .with_source(SourceSpan::point("src/main/mule/order-api.xml", 13, 5))
    .with_evidence(json!({"name": "OrderAPIFlow"}))
    .with_rule_version("1.0.0");

    assert_eq!(finding.rule_id, "MULE-FLOW-001");
    assert_eq!(finding.origin, FindingOrigin::DeterministicRule);
    assert_eq!(
        finding.recommendation.as_deref(),
        Some("Use lower-case names")
    );
    let source = finding.source.as_ref().unwrap();
    assert_eq!(source.start_line, 13);
    assert_eq!(source.file, "src/main/mule/order-api.xml");
}

#[test]
fn secret_ref_debug_and_display_redact() {
    let plain = SecretRef::Plain {
        value: "super-secret".to_string(),
    };
    let env = SecretRef::Environment {
        env: "RUNNERGUARD_AI_API_KEY".to_string(),
    };

    assert_eq!(
        format!("{:?}", plain),
        "Plain { value: \"***REDACTED***\" }"
    );
    assert_eq!(
        format!("{:?}", env),
        "Environment { env: \"***REDACTED***\" }"
    );
    assert_eq!(plain.to_string(), "***REDACTED***");
    assert_eq!(env.to_string(), "***REDACTED***");
    assert_eq!(plain.redacted_display(), "***REDACTED***");
}

#[test]
fn secret_ref_resolution_skips_empty() {
    let plain_empty = SecretRef::Plain {
        value: String::new(),
    };
    let env_empty = SecretRef::Environment { env: String::new() };
    assert!(plain_empty.is_unset());
    assert!(env_empty.is_unset());
    assert_eq!(plain_empty.resolve_with(|_| Some("ignored".into())), None);
}

#[test]
fn diagnostic_carries_stage_and_help() {
    let diag = Diagnostic::new(
        DiagnosticStage::Xml,
        "XML-001",
        DiagnosticLevel::Error,
        "Malformed Mule XML",
    )
    .with_help("Run runnerguard parse --strict-parser for details")
    .with_cause("unexpected token at line 4");

    assert_eq!(diag.stage.as_code(), "XML");
    assert_eq!(
        diag.help.as_deref(),
        Some(diag.help.as_ref().unwrap().as_str())
    );
    assert_eq!(diag.causes.len(), 1);
}

#[test]
fn rule_set_with_minimum_fields_deserialises() {
    let json = json!({
        "schema_version": CURRENT_SCHEMA_VERSION,
        "id": "rs",
        "name": "Sample",
        "version": "1.0.0",
        "defaults": RuleDefaults::default(),
        "rules": [
            Rule {
                id: "R-1".into(),
                title: "title".into(),
                description: None,
                severity: Severity::Warning,
                enabled: true,
                tags: vec![],
                target: RuleTarget {
                    entity: vec![RuleTargetEntity::Flow],
                    match_: None,
                },
                when: None,
                r#assert: SingleCondition {
                    fact: runnerguard_model::FactPath("flow.name".into()),
                    op: runnerguard_model::Operator::Exists,
                    value: None,
                }
                .into(),
                message: "msg".into(),
                recommendation: None,
            }
        ]
    });

    let parsed: RuleSet = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.rules.len(), 1);
    assert_eq!(parsed.rules[0].id, "R-1");
}
