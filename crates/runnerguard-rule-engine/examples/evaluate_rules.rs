//! Loads a parsed project JSON + a rule-set JSON from disk and prints the
//! findings that the rule engine emits.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-rule-engine \
//!   --example evaluate_rules -- \
//!   ./fixtures/parsed-project.json \
//!   ./rules/basic.json
//! ```

use runnerguard_model::ParsedProject;
use runnerguard_model::RuleSet;
use runnerguard_rule_engine::{compile, evaluate_with_source};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let project_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./fixtures/parsed-project.json"));
    let rules_path = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./rules/basic.json"));

    let project_bytes = std::fs::read(&project_path)?;
    let project: ParsedProject = serde_json::from_slice(&project_bytes)?;
    let rules_bytes = std::fs::read(&rules_path)?;
    let rule_set: RuleSet = serde_json::from_slice(&rules_bytes)?;

    let compiled = compile(&rule_set);
    for issue in &compiled.issues {
        eprintln!("compile issue: {} ({})", issue.rule_id, issue.message);
    }
    let result = evaluate_with_source(&compiled, &rule_set.rules, &project);
    println!(
        "evaluated {} rule(s), {} finding(s), {} error(s)",
        result.evaluated_rules,
        result.findings.len(),
        result.errors.len()
    );
    for f in &result.findings {
        println!(
            "  [{:?}] {} -> {} ({}:{})",
            f.severity,
            f.rule_id,
            f.message,
            f.source
                .as_ref()
                .map(|s| s.file.clone())
                .unwrap_or_default(),
            f.source.as_ref().map(|s| s.start_line).unwrap_or(0),
        );
    }
    Ok(())
}
