//! `SecretRef` resolution for AI providers.
//!
//! The provider never holds a raw key — it only sees the [`SecretRef`]
//! the user configured and resolves it right before stuffing it into an
//! `Authorization: Bearer …` header. The resolved value is never written
//! back to the prompt, logs, or diagnostics.

use crate::error::AiError;
use runnerguard_model::SecretRef;

pub fn resolve(secret: &SecretRef) -> Result<String, AiError> {
    secret.resolve().ok_or_else(|| {
        if secret.is_unset() {
            AiError::Internal("secret is unset".to_string())
        } else {
            AiError::Internal("secret could not be resolved from the environment".to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_plain() {
        let s = SecretRef::Plain {
            value: "abc".to_string(),
        };
        assert_eq!(resolve(&s).unwrap(), "abc");
    }

    #[test]
    fn rejects_empty_plain() {
        let s = SecretRef::Plain {
            value: String::new(),
        };
        assert!(resolve(&s).is_err());
    }

    #[test]
    fn resolves_env() {
        // We can't mutate the environment in a `#![deny(unsafe_code)]`
        // crate, so test resolution against an env var we control by
        // asking the resolver for whatever the test runner happens to
        // expose. PATH is set on every platform we target.
        let s = SecretRef::Environment {
            env: "PATH".to_string(),
        };
        let v = resolve(&s).expect("PATH should resolve");
        assert!(!v.is_empty());
    }

    #[test]
    fn rejects_missing_env() {
        let s = SecretRef::Environment {
            env: "RUNNERGUARD_NOT_SET_XYZ_123".to_string(),
        };
        assert!(resolve(&s).is_err());
    }

    #[test]
    fn empty_is_unset() {
        let s = SecretRef::Empty;
        assert!(resolve(&s).is_err());
    }
}
