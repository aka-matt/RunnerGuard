//! End-to-end tests that drive the rule engine against the
//! `fixtures/mule-project-basic/` and `fixtures/mule-project-invalid/*`
//! projects. These are the "do the docs still match the code" guards —
//! the fixture READMEs document the expected findings, and these tests
//! assert them. If a fixture starts producing different findings, the
//! change is intentional and the test must be updated alongside the
//! fixture README.

use runnerguard_fs::{DiscoverOptions, discover};
use runnerguard_model::RuleSet;
use runnerguard_mule_parser::{ParseOptions, parse_project};
use runnerguard_rule_engine::compile;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn run_fixture(name: &str) -> Vec<String> {
    let project_dir = workspace_root().join("fixtures").join(name);
    let rules_path = workspace_root().join("rules/basic.json");
    let files = discover(&project_dir, DiscoverOptions::default()).expect("discovery");
    let parsed = parse_project(&files, &ParseOptions::default()).expect("parse");
    let rules_bytes = std::fs::read(&rules_path).expect("rules");
    let set: RuleSet = serde_json::from_slice(&rules_bytes).expect("rules json");
    let compiled = compile(&set);
    let outcome =
        runnerguard_rule_engine::evaluate_with_source(&compiled, &set.rules, &parsed.project);
    let mut ids: Vec<String> = outcome.findings.iter().map(|f| f.rule_id.clone()).collect();
    ids.sort();
    ids.dedup();
    ids
}

fn evaluate_rule_ids(name: &str) -> Vec<String> {
    let project_dir = workspace_root().join("fixtures").join(name);
    let rules_path = workspace_root().join("rules/basic.json");
    let files = discover(&project_dir, DiscoverOptions::default()).expect("discovery");
    let parsed = parse_project(&files, &ParseOptions::default()).expect("parse");
    let rules_bytes = std::fs::read(&rules_path).expect("rules");
    let set: RuleSet = serde_json::from_slice(&rules_bytes).expect("rules json");
    let compiled = compile(&set);
    let outcome =
        runnerguard_rule_engine::evaluate_with_source(&compiled, &set.rules, &parsed.project);
    let mut ids: Vec<String> = outcome
        .findings
        .iter()
        .map(|f| format!("{}:{}", f.severity.as_str(), f.rule_id))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

#[test]
fn basic_fixture_produces_only_error_handler_finding() {
    let ids = evaluate_rule_ids("mule-project-basic");
    // The basic fixture has an HTTP listener but no local error handler,
    // so the MULE-FLOW-004 rule is the only thing that fires.
    assert_eq!(ids, vec!["warning:MULE-FLOW-004".to_string()]);
}

#[test]
fn bare_project_reports_missing_required_files() {
    let ids = evaluate_rule_ids("mule-project-invalid/bare-project");
    assert!(
        ids.iter().any(|s| s == "error:MULE-PROJ-001"),
        "expected MULE-PROJ-001 error, got {ids:?}"
    );
}

#[test]
fn unresolvable_flow_ref_is_detected() {
    let ids = evaluate_rule_ids("mule-project-invalid/unresolvable-flow-ref");
    assert!(
        ids.iter().any(|s| s == "error:MULE-FLOW-003"),
        "expected MULE-FLOW-003 error, got {ids:?}"
    );
}

#[test]
fn bad_kebab_case_flags_kebab_rule() {
    let ids = evaluate_rule_ids("mule-project-invalid/bad-kebab-case");
    assert!(
        ids.iter().any(|s| s == "warning:MULE-FLOW-001"),
        "expected MULE-FLOW-001 warning, got {ids:?}"
    );
}

#[test]
fn logger_secret_is_critical() {
    let ids = evaluate_rule_ids("mule-project-invalid/logger-secret");
    assert!(
        ids.iter().any(|s| s == "critical:MULE-LOG-001"),
        "expected critical MULE-LOG-001, got {ids:?}"
    );
}

#[test]
fn every_rule_compiles_against_basic_ruleset() {
    let rules_path = workspace_root().join("rules/basic.json");
    let bytes = std::fs::read(&rules_path).expect("rules");
    let set: RuleSet = serde_json::from_slice(&bytes).expect("rules json");
    let compiled = compile(&set);
    assert!(
        compiled.issues.is_empty(),
        "compile issues: {:?}",
        compiled.issues
    );
    assert_eq!(compiled.rules.len(), set.rules.len());
    // Sanity: every rule in the basic pack has a non-empty id.
    for r in &compiled.rules {
        assert!(!r.id.is_empty());
    }
}

#[test]
fn basic_ruleset_round_trips_through_evaluate() {
    // Smoke test: a malformed fixture is not a panic.
    let _ = run_fixture("mule-project-invalid/malformed-xml");
}
