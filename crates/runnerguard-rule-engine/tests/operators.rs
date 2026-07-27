//! Operator unit tests. Each test exercises one operator against a
//! tiny in-memory project so the rule engine's behaviour is verifiable
//! without spinning up the full parser pipeline.

use runnerguard_model::{
    Condition, FactPath, MuleComponent, MuleFlow, MuleFlowKind, Operator, ParsedProject,
    ProjectDescriptor, ProjectIndex, Rule, RuleDefaults, RuleSet, RuleTarget, RuleTargetEntity,
    Severity, SingleCondition, SourceFile, SourceFileKind, SourceSpan,
};
use runnerguard_rule_engine::{compile, evaluate_with_source};
use serde_json::json;

fn make_project_with_flow(name: &str, component_names: &[&str]) -> ParsedProject {
    let components: Vec<MuleComponent> = component_names
        .iter()
        .enumerate()
        .map(|(i, name)| MuleComponent {
            id: format!("component:{i}"),
            qualified_name: (*name).to_string(),
            local_name: (*name).to_string(),
            namespace_uri: None,
            attributes: Default::default(),
            text: None,
            cdata: vec![],
            children: vec![],
            source: SourceSpan::point(*name, 1, 1),
        })
        .collect();

    let flow = MuleFlow {
        schema_version: "1.0".to_string(),
        id: format!("flow:{name}"),
        project_id: "project:test".to_string(),
        kind: MuleFlowKind::Flow,
        name: name.to_string(),
        source: SourceSpan::point(name, 1, 1),
        namespaces: Default::default(),
        attributes: Default::default(),
        components: components.clone(),
        facts: runnerguard_model::FlowFacts {
            component_count: component_names.len(),
            ..Default::default()
        },
        source_xml_sha256: "sha256:dummy".to_string(),
    };

    ParsedProject {
        schema_version: "1.0".to_string(),
        project: ProjectDescriptor {
            schema_version: "1.0".to_string(),
            id: "project:test".to_string(),
            name: "demo".to_string(),
            group_id: None,
            artifact_id: None,
            version: None,
            mule_version: None,
            sdk_version: None,
            root_path: "/tmp/demo".to_string(),
            files: vec![SourceFile::new("demo.xml", 1, SourceFileKind::MuleXml)],
            required_files_missing: vec![],
        },
        documents: vec![runnerguard_model::MuleDocument {
            schema_version: "1.0".to_string(),
            id: "doc:0".to_string(),
            project_id: "project:test".to_string(),
            source: SourceSpan::point("demo.xml", 1, 1),
            namespaces: Default::default(),
            flows: vec![flow],
            sub_flows: vec![],
            global_configurations: vec![],
        }],
        index: ProjectIndex::default(),
    }
}

fn build_rule_set(
    rule_id: &str,
    target: RuleTargetEntity,
    target_match: Option<&str>,
    conditions: Vec<Condition>,
    assertion: Condition,
) -> RuleSet {
    let rule = Rule {
        id: rule_id.to_string(),
        title: "Test rule".to_string(),
        description: None,
        severity: Severity::Warning,
        enabled: true,
        tags: vec![],
        target: RuleTarget {
            entity: vec![target],
            match_: target_match.map(|s| Box::new(simple_eq("flow.name", s))),
        },
        when: None,
        assert: assertion,
        message: format!("Rule {rule_id} fired"),
        recommendation: None,
    };
    let _ = conditions;
    RuleSet {
        schema_version: "1.0".to_string(),
        id: "ruleset:test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        defaults: RuleDefaults::default(),
        rules: vec![rule],
    }
}

fn simple_eq(fact: &str, value: &str) -> Condition {
    Condition::Single(SingleCondition {
        fact: FactPath(fact.to_string()),
        op: Operator::Equals,
        value: Some(json!(value)),
    })
}

#[test]
fn equals_operator_fires_on_mismatch() {
    let project = make_project_with_flow("hello", &["http:listener"]);
    let rule_set = build_rule_set(
        "RG-EQ-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Equals,
            value: Some(json!("goodbye")),
        }),
    );
    let compiled = compile(&rule_set);
    assert!(compiled.issues.is_empty(), "issues: {:?}", compiled.issues);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].rule_id, "RG-EQ-001");
}

#[test]
fn equals_operator_does_not_fire_on_match() {
    let project = make_project_with_flow("hello", &["http:listener"]);
    let rule_set = build_rule_set(
        "RG-EQ-002",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Equals,
            value: Some(json!("hello")),
        }),
    );
    let compiled = compile(&rule_set);
    assert!(compiled.issues.is_empty());
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert!(result.findings.is_empty());
}

#[test]
fn not_equals_fires_on_match() {
    let project = make_project_with_flow("hello", &["http:listener"]);
    let rule_set = build_rule_set(
        "RG-NE-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::NotEquals,
            value: Some(json!("hello")),
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings.len(), 1);
}

#[test]
fn greater_than_and_less_than() {
    let project = make_project_with_flow("hello", &["a", "b", "c"]);
    // 3 > 2 holds, so greater-than does NOT fire.
    let rule_set = build_rule_set(
        "RG-CMP-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.component-count".to_string()),
            op: Operator::GreaterThan,
            value: Some(json!(2)),
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert!(
        result.findings.is_empty(),
        "3 > 2 holds, greater-than should not fire"
    );

    // 3 < 2 fails, so less-than DOES fire.
    let rule_set2 = build_rule_set(
        "RG-CMP-002",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.component-count".to_string()),
            op: Operator::LessThan,
            value: Some(json!(2)),
        }),
    );
    let compiled2 = compile(&rule_set2);
    let result2 = evaluate_with_source(&compiled2, &rule_set2.rules, &project);
    assert_eq!(
        result2.findings.len(),
        1,
        "3 < 2 fails, less-than should fire"
    );
}

#[test]
fn in_and_not_in() {
    let project = make_project_with_flow("hello", &[]);
    let rule_set = build_rule_set(
        "RG-IN-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.kind".to_string()),
            op: Operator::In,
            value: Some(json!(["flow", "sub-flow"])),
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert!(
        result.findings.is_empty(),
        "kind=flow is in [flow, sub-flow]"
    );

    let rule_set2 = build_rule_set(
        "RG-IN-002",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.kind".to_string()),
            op: Operator::NotIn,
            value: Some(json!(["flow", "sub-flow"])),
        }),
    );
    let compiled2 = compile(&rule_set2);
    let result2 = evaluate_with_source(&compiled2, &rule_set2.rules, &project);
    assert_eq!(result2.findings.len(), 1);
}

#[test]
fn exists_and_not_exists() {
    let project = make_project_with_flow("hello", &["http:listener"]);
    let rule_set = build_rule_set(
        "RG-EX-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.source-component".to_string()),
            op: Operator::Exists,
            value: None,
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(
        result.findings.len(),
        1,
        "source-component is missing for this fixture, so exists fires"
    );

    let rule_set2 = build_rule_set(
        "RG-EX-002",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.source-component".to_string()),
            op: Operator::NotExists,
            value: None,
        }),
    );
    let compiled2 = compile(&rule_set2);
    let result2 = evaluate_with_source(&compiled2, &rule_set2.rules, &project);
    assert!(
        result2.findings.is_empty(),
        "source-component is missing so not-exists should not fire"
    );
}

#[test]
fn matches_with_regex() {
    let project = make_project_with_flow("order-api-flow", &[]);
    let rule_set = build_rule_set(
        "RG-RX-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Matches,
            value: Some(json!("^order-.*$")),
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert!(result.findings.is_empty(), "matches");
}

#[test]
fn contains_component_against_flow_tree() {
    let project = make_project_with_flow("hello", &["http:listener", "set-payload"]);
    let rule_set = build_rule_set(
        "RG-CC-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Equals,
            value: Some(json!("hello")),
        }),
    );
    // Need a target that fires a contains-component assertion. Build a
    // second rule that checks the flow contains "logger".
    let cc_rule = Rule {
        id: "RG-CC-002".to_string(),
        title: "Flow should contain logger".to_string(),
        description: None,
        severity: Severity::Warning,
        enabled: true,
        tags: vec![],
        target: RuleTarget {
            entity: vec![RuleTargetEntity::Flow],
            match_: None,
        },
        when: None,
        assert: Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::ContainsComponent,
            value: Some(json!("logger")),
        }),
        message: "logger missing".to_string(),
        recommendation: None,
    };
    let rule_set2 = RuleSet {
        schema_version: "1.0".to_string(),
        id: "ruleset:test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        defaults: RuleDefaults::default(),
        rules: vec![cc_rule],
    };
    let compiled = compile(&rule_set2);
    let result = evaluate_with_source(&compiled, &rule_set2.rules, &project);
    assert_eq!(result.findings.len(), 1, "logger missing should fire");
    let _ = (compiled, rule_set);
}

#[test]
fn invalid_fact_path_is_a_compile_issue() {
    let rule = Rule {
        id: "RG-BAD-001".to_string(),
        title: "Bad".to_string(),
        description: None,
        severity: Severity::Warning,
        enabled: true,
        tags: vec![],
        target: RuleTarget {
            entity: vec![RuleTargetEntity::Flow],
            match_: None,
        },
        when: None,
        assert: Condition::Single(SingleCondition {
            fact: FactPath("flow.nonexistent".to_string()),
            op: Operator::Equals,
            value: Some(json!("x")),
        }),
        message: "msg".to_string(),
        recommendation: None,
    };
    let rule_set = RuleSet {
        schema_version: "1.0".to_string(),
        id: "ruleset:test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        defaults: RuleDefaults::default(),
        rules: vec![rule],
    };
    let compiled = compile(&rule_set);
    assert!(
        !compiled.issues.is_empty(),
        "unknown fact path must be a compile issue"
    );
}

#[test]
fn all_combinator_requires_all_to_hold() {
    let project = make_project_with_flow("hello", &["a", "b"]);
    let rule_set = build_rule_set(
        "RG-ALL-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Combinator(runnerguard_model::ConditionCombinator::All {
            all: vec![
                Condition::Single(SingleCondition {
                    fact: FactPath("flow.component-count".to_string()),
                    op: Operator::GreaterThan,
                    value: Some(json!(1)),
                }),
                Condition::Single(SingleCondition {
                    fact: FactPath("flow.name".to_string()),
                    op: Operator::Equals,
                    value: Some(json!("goodbye")),
                }),
            ],
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    // The `equals name==goodbye` branch fails; the `>1` branch holds.
    assert_eq!(
        result.findings.len(),
        1,
        "only the failing leaf should fire"
    );
}

#[test]
fn any_combinator_fires_when_no_child_holds() {
    let project = make_project_with_flow("hello", &[]);
    let rule_set = build_rule_set(
        "RG-ANY-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Combinator(runnerguard_model::ConditionCombinator::Any {
            any: vec![
                Condition::Single(SingleCondition {
                    fact: FactPath("flow.name".to_string()),
                    op: Operator::Equals,
                    value: Some(json!("a")),
                }),
                Condition::Single(SingleCondition {
                    fact: FactPath("flow.name".to_string()),
                    op: Operator::Equals,
                    value: Some(json!("b")),
                }),
            ],
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings.len(), 1, "no child matches -> any fires");
}

#[test]
fn not_combinator_fires_when_child_holds() {
    let project = make_project_with_flow("hello", &[]);
    let rule_set = build_rule_set(
        "RG-NOT-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Combinator(runnerguard_model::ConditionCombinator::Not {
            not: Box::new(Condition::Single(SingleCondition {
                fact: FactPath("flow.name".to_string()),
                op: Operator::Equals,
                value: Some(json!("hello")),
            })),
        }),
    );
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings.len(), 1, "child holds -> not fires");
}

#[test]
fn severity_override_is_honoured() {
    let project = make_project_with_flow("hello", &[]);
    let mut rule_set = build_rule_set(
        "RG-SEV-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Equals,
            value: Some(json!("goodbye")),
        }),
    );
    rule_set.rules[0].severity = Severity::Error;
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings[0].severity, Severity::Error);
}

#[test]
fn disabled_rule_is_skipped() {
    let project = make_project_with_flow("hello", &[]);
    let mut rule_set = build_rule_set(
        "RG-DIS-001",
        RuleTargetEntity::Flow,
        None,
        vec![],
        Condition::Single(SingleCondition {
            fact: FactPath("flow.name".to_string()),
            op: Operator::Equals,
            value: Some(json!("goodbye")),
        }),
    );
    rule_set.rules[0].enabled = false;
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert!(result.findings.is_empty());
    assert!(result.evaluated_rules <= 1);
}

#[test]
fn findings_are_sorted_by_severity_then_rule_id() {
    let project = make_project_with_flow("hello", &[]);
    let rules = vec![
        Rule {
            id: "RG-LOW-001".to_string(),
            title: "low".to_string(),
            description: None,
            severity: Severity::Info,
            enabled: true,
            tags: vec![],
            target: RuleTarget {
                entity: vec![RuleTargetEntity::Flow],
                match_: None,
            },
            when: None,
            assert: Condition::Single(SingleCondition {
                fact: FactPath("flow.name".to_string()),
                op: Operator::Equals,
                value: Some(json!("goodbye")),
            }),
            message: "low".to_string(),
            recommendation: None,
        },
        Rule {
            id: "RG-HIGH-001".to_string(),
            title: "high".to_string(),
            description: None,
            severity: Severity::Critical,
            enabled: true,
            tags: vec![],
            target: RuleTarget {
                entity: vec![RuleTargetEntity::Flow],
                match_: None,
            },
            when: None,
            assert: Condition::Single(SingleCondition {
                fact: FactPath("flow.name".to_string()),
                op: Operator::Equals,
                value: Some(json!("goodbye")),
            }),
            message: "high".to_string(),
            recommendation: None,
        },
    ];
    let rule_set = RuleSet {
        schema_version: "1.0".to_string(),
        id: "ruleset:test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        defaults: RuleDefaults::default(),
        rules,
    };
    let compiled = compile(&rule_set);
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    assert_eq!(result.findings.len(), 2);
    assert_eq!(result.findings[0].rule_id, "RG-HIGH-001");
    assert_eq!(result.findings[1].rule_id, "RG-LOW-001");
}
