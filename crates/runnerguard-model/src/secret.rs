//! Secret reference resolution and redaction.
//!
//! `SecretRef` is the canonical way to express "this setting is a secret":
//! either an environment variable name or, for tests only, a plaintext value.
//! Its `Debug` impl always prints `***REDACTED***` so accidentally
//! `dbg!`-printing a config can never leak.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Where to find a secret at runtime.
///
/// Serializes untagged so YAML configs can write either `{ value: ... }` or
/// `{ env: ... }` directly.
///
/// `Debug` is hand-rolled to always print `***REDACTED***` so accidentally
/// `dbg!`-printing a config can never leak.
#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretRef {
    /// Inline plaintext — only acceptable for tests and local development.
    Plain { value: String },
    /// Reference to an environment variable resolved at runtime.
    Environment { env: String },
    /// Default = empty environment reference (must be replaced before use).
    #[default]
    Empty,
}

impl SecretRef {
    /// Resolve the secret against an environment lookup function.
    /// Returns `None` if the source is missing (empty value, unset env var,
    /// or env var that resolves to the empty string).
    pub fn resolve_with<F>(&self, lookup: F) -> Option<String>
    where
        F: FnOnce(&str) -> Option<String>,
    {
        match self {
            Self::Plain { value } => {
                if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                }
            }
            Self::Environment { env } => lookup(env).filter(|s| !s.is_empty()),
            Self::Empty => None,
        }
    }

    /// Resolve the secret against the process environment.
    pub fn resolve(&self) -> Option<String> {
        self.resolve_with(|name| std::env::var(name).ok())
    }

    /// `true` when neither `value` nor `env` carries usable data.
    pub fn is_unset(&self) -> bool {
        match self {
            Self::Plain { value } => value.is_empty(),
            Self::Environment { env } => env.is_empty(),
            Self::Empty => true,
        }
    }

    /// Render this secret for `config show` — always redacted.
    pub fn redacted_display(&self) -> String {
        "***REDACTED***".to_string()
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display also redacts: there is no scenario where a user wants the
        // raw secret shown via `format!("{}", secret)`.
        f.write_str("***REDACTED***")
    }
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Hand-rolled so the inner string can never be accidentally leaked
        // through `dbg!`, `unwrap()`, `assert_eq!`, or panic messages.
        match self {
            Self::Plain { .. } => f.write_str("Plain { value: \"***REDACTED***\" }"),
            Self::Environment { .. } => f.write_str("Environment { env: \"***REDACTED***\" }"),
            Self::Empty => f.write_str("Empty"),
        }
    }
}
