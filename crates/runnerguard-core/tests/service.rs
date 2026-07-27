//! Integration tests for the orchestration service.
//!
//! These tests build a tiny Mule project on disk, run [`ScanService`]
//! against it, and verify that the event stream and the produced
//! report both look right.

use runnerguard_core::{ScanEvent, ScanService, collecting_sink};
use runnerguard_model::{ReportFormat, Rule, RuleDefaults, RuleSet, ScanRequest, Severity};

fn write_xml(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir.join("src/main/mule")).unwrap();
    std::fs::write(
        dir.join("src/main/mule").join(name),
        format!(
            "<?xml version=\"1.0\"?>\n<mule xmlns=\"http://www.mulesoft.org/schema/mule/core\"\n      xmlns:http=\"http://www.mulesoft.org/schema/mule/http\">\n{body}\n</mule>\n"
        ),
    )
    .unwrap();
}

fn write_pom(dir: &std::path::Path) {
    std::fs::write(
        dir.join("pom.xml"),
        r#"<?xml version="1.0"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
  <version>1.0.0</version>
</project>"#,
    )
    .unwrap();
}

#[test]
fn end_to_end_scan_emits_started_and_finished() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().to_path_buf();
    write_pom(&project);
    write_xml(
        &project,
        "demo.xml",
        r#"<flow name="hello">
        <http:listener config-ref="cfg" path="/x" />
        <set-payload value="ok" />
    </flow>"#,
    );

    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let req = ScanRequest {
        project_dir: project.clone(),
        rule_files: vec![],
        output_dir: out_dir.clone(),
        formats: vec![ReportFormat::Markdown],
        include_tests: false,
        write_flow_json: false,
        ai_mode: runnerguard_model::AiMode::Disabled,
        fail_on: Severity::Warning,
        rule_filter: Default::default(),
    };
    let mut sink = collecting_sink();
    let outcome = ScanService::new().run_with_sink(&req, &mut sink);
    let kinds: Vec<&'static str> = sink
        .events
        .iter()
        .map(|e| match e {
            ScanEvent::Started { .. } => "started",
            ScanEvent::FileDiscovered { .. } => "discovered",
            ScanEvent::RulesLoaded { .. } => "rules",
            ScanEvent::XmlParsed { .. } => "parsed",
            ScanEvent::FlowArtifactWritten { .. } => "artifact",
            ScanEvent::RuleStarted { .. } => "rule_started",
            ScanEvent::RuleCompleted { .. } => "rule_completed",
            ScanEvent::AiStarted => "ai_started",
            ScanEvent::AiCompleted { .. } => "ai_completed",
            ScanEvent::ReportWritten { .. } => "report",
            ScanEvent::Warning { .. } => "warning",
            ScanEvent::Finished { .. } => "finished",
            ScanEvent::ConfigLoaded { .. } => "config",
        })
        .collect();
    assert!(kinds.contains(&"started"), "kinds: {:?}", kinds);
    assert!(kinds.contains(&"finished"), "kinds: {:?}", kinds);
    assert!(
        !outcome.report_paths.is_empty(),
        "expected at least one report path"
    );
}

#[test]
fn rules_with_failing_assertion_emit_a_finding() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().to_path_buf();
    write_pom(&project);
    write_xml(
        &project,
        "demo.xml",
        r#"<flow name="hello">
        <set-payload value="ok" />
    </flow>"#,
    );

    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let rules_dir = dir.path().join("rules");
    std::fs::create_dir_all(&rules_dir).unwrap();
    let rules_path = rules_dir.join("basic.json");

    let rule_set = RuleSet {
        schema_version: "1.0".to_string(),
        id: "rs:test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        defaults: RuleDefaults::default(),
        rules: vec![Rule {
            id: "RG-DEMO-001".to_string(),
            title: "Flow must be named goodbye".to_string(),
            description: None,
            severity: Severity::Warning,
            enabled: true,
            tags: vec![],
            target: runnerguard_model::RuleTarget {
                entity: vec![runnerguard_model::RuleTargetEntity::Flow],
                match_: None,
            },
            when: None,
            assert: runnerguard_model::Condition::Single(runnerguard_model::SingleCondition {
                fact: runnerguard_model::FactPath("flow.name".to_string()),
                op: runnerguard_model::Operator::Equals,
                value: Some(serde_json::json!("goodbye")),
            }),
            message: "Flow should be named goodbye".to_string(),
            recommendation: None,
        }],
    };
    std::fs::write(&rules_path, serde_json::to_vec_pretty(&rule_set).unwrap()).unwrap();

    let req = ScanRequest {
        project_dir: project.clone(),
        rule_files: vec![rules_path.clone()],
        output_dir: out_dir.clone(),
        formats: vec![ReportFormat::Markdown],
        include_tests: false,
        write_flow_json: true,
        ai_mode: runnerguard_model::AiMode::Disabled,
        fail_on: Severity::Warning,
        rule_filter: Default::default(),
    };
    let outcome = ScanService::new().run(&req);
    assert_eq!(outcome.result.findings.len(), 1);
    assert_eq!(outcome.result.findings[0].rule_id, "RG-DEMO-001");
    // Per-flow artifact should have been written.
    assert!(
        !outcome.artifact_paths.is_empty(),
        "expected flow artifacts"
    );
}
