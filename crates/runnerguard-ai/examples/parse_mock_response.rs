//! Parse an AI response fixture without touching the network.
//!
//! ```bash
//! cargo run -p runnerguard-ai --example parse_mock_response -- \
//!   fixtures/ai-responses/valid.json
//! ```
//!
//! Demonstrates the response-parsing pipeline: JSON extraction, schema
//! validation, and conversion to deterministic-style `Finding`s. No
//! prompts, no HTTP — strictly offline.

use std::process::ExitCode;

use runnerguard_ai::response::validate_against_schema;
use runnerguard_model::suggestions_to_findings;
use serde_json::Value;

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fixtures/ai-responses/valid.json".to_string());
    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let raw = match runnerguard_ai::extract_json_object(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("could not extract JSON: {e}");
            return ExitCode::FAILURE;
        }
    };
    let schema = match std::fs::read_to_string("schemas/ai-response.schema.json") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not read schema: {e}");
            return ExitCode::FAILURE;
        }
    };
    let analysis = match validate_against_schema(&raw, &schema) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("schema validation failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let findings = suggestions_to_findings(analysis.clone());
    println!(
        "summary: {}\nmodel: {:?}\nsuggestions: {}",
        analysis.summary,
        analysis.model,
        findings.len()
    );
    for f in findings {
        println!(
            "  [{}] {} — {} (entity_id={:?})",
            f.severity.as_str(),
            f.rule_id,
            f.message,
            f.entity_id
        );
    }
    ExitCode::SUCCESS
}

#[allow(dead_code)]
fn _value_unused() -> Value {
    serde_json::json!({})
}
