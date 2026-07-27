//! Validates a JSON rule file against an embedded schema and reports every
//! violation.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-json \
//!   --example validate_rule_file -- \
//!   ../../implementation_docs/muleguard-basic-rules.json
//! ```

use runnerguard_json::{DefaultJsonCodec, DefaultSchemaValidator, JsonCodec, SchemaValidator};
use serde_json::json;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../implementation_docs/muleguard-basic-rules.json"));

    let codec = DefaultJsonCodec;
    let validator = DefaultSchemaValidator;

    // Tiny embedded schema — enough to demonstrate the validator and reject
    // the obvious typos. The production schema lives under schemas/.
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["schema_version", "id", "name", "version", "rules"],
        "properties": {
            "schema_version": {"type": "string"},
            "id": {"type": "string"},
            "name": {"type": "string"},
            "version": {"type": "string"},
            "rules": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "title", "severity", "target", "assert", "message"],
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "severity": {
                            "type": "string",
                            "enum": ["info", "warning", "error", "critical"]
                        },
                        "target": {"type": "object"},
                        "assert": {"type": "object"},
                        "message": {"type": "string"}
                    }
                }
            }
        }
    });

    let rule_file: serde_json::Value = codec.read_file(&path)?;
    match validator.validate(&schema, &rule_file) {
        Ok(()) => {
            let rules = rule_file["rules"].as_array().map(|r| r.len()).unwrap_or(0);
            println!("OK: {} rules loaded from {}", rules, path.display());
            Ok(())
        }
        Err(violations) => {
            for v in violations {
                println!("{}: {}", v.pointer.unwrap_or_default(), v.message);
            }
            Err("schema validation failed".into())
        }
    }
}
