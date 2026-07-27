//! Schema validation against JSON Schema Draft 2020-12.
//!
//! [`SchemaValidator::validate`] returns *every* violation it can find.
//! Higher layers (rule engine, `runnerguard rules validate`) surface these
//! to the user as a single batch.

use crate::error::SchemaViolation;
use jsonschema::{Draft, ValidationError};
use serde_json::Value;
use std::fmt::Write as _;

/// Validates a JSON instance against a compiled schema.
pub trait SchemaValidator: Send + Sync {
    fn validate(&self, schema: &Value, instance: &Value) -> Result<(), Vec<SchemaViolation>>;
}

/// Default validator backed by the `jsonschema` crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultSchemaValidator;

impl SchemaValidator for DefaultSchemaValidator {
    fn validate(&self, schema: &Value, instance: &Value) -> Result<(), Vec<SchemaViolation>> {
        let validator = match jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(schema)
        {
            Ok(v) => v,
            Err(err) => {
                return Err(vec![SchemaViolation {
                    pointer: None,
                    message: format!("schema compilation failed: {err}"),
                }]);
            }
        };

        let mut violations = Vec::new();
        for error in validator.iter_errors(instance) {
            violations.push(SchemaViolation {
                pointer: Some(error.instance_path().to_string()),
                message: error_to_message(&error),
            });
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

fn error_to_message(err: &ValidationError<'_>) -> String {
    // jsonschema 0.49 has no `message()` method; the Display impl formats
    // the kind + paths for us. Falling back to a manually composed message
    // keeps tests stable across minor library bumps.
    let mut out = String::new();
    let _ = write!(&mut out, "{}", err);
    if out.is_empty() {
        let _ = write!(
            &mut out,
            "validation failed at {} against schema {}",
            err.instance_path(),
            err.schema_path()
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_a_well_formed_instance() {
        let v = DefaultSchemaValidator;
        let schema = json!({
            "type": "object",
            "required": ["id", "version"],
            "properties": {
                "id": {"type": "string"},
                "version": {"type": "string"},
                "count": {"type": "integer", "minimum": 0}
            }
        });
        let instance = json!({"id": "abc", "version": "1.0.0", "count": 5});
        v.validate(&schema, &instance).expect("should pass");
    }

    #[test]
    fn returns_all_violations_not_just_first() {
        let v = DefaultSchemaValidator;
        let schema = json!({
            "type": "object",
            "required": ["id", "version"],
            "properties": {
                "id": {"type": "string"},
                "version": {"type": "string"}
            }
        });
        let instance = json!({"id": 1, "version": 2});
        let violations = v.validate(&schema, &instance).unwrap_err();
        assert!(violations.len() >= 2, "got {} violations", violations.len());
    }
}
