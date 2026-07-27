//! JSON codec and JSON Schema validation for RunnerGuard.
//!
//! This crate is responsible for **format** concerns only. Whether a value
//! "passes a rule" is not its job — it only checks that bytes are valid
//! JSON and that the deserialised object satisfies the supplied schema.
//!
//! Two traits are exposed:
//!
//! - [`JsonCodec`]: round-trip a `serde::Serialize`/`Deserialize` value
//!   through bytes.
//! - [`SchemaValidator`]: validate any `serde_json::Value` against a
//!   compiled Draft 2020-12 schema, returning *all* violations.
//!
//! Errors are returned as [`JsonError`] / [`SchemaViolation`]. Higher
//! layers translate these into the cross-cutting [`Diagnostic`] type.

#![deny(unsafe_code)]

pub mod codec;
pub mod error;
pub mod schema;

pub use codec::{DefaultJsonCodec, JsonCodec};
pub use error::{JsonError, SchemaViolation};
pub use schema::{DefaultSchemaValidator, SchemaValidator};
