//! Deterministic JSON rule engine for RunnerGuard.
//!
//! The engine takes a compiled rule set and a parsed MuleSoft project,
//! walks the candidate entities per rule, evaluates `when`/`match_`/
//! `assert` conditions, and emits [`Finding`]s. All other layers see
//! just [`engine::evaluate`] and [`compile::compile`].
//!
//! The engine never panics on user-supplied data; regex and fact-path
//! problems become compilation issues, not aborts.

#![deny(unsafe_code)]

pub mod compile;
pub mod condition;
pub mod context;
pub mod engine;
pub mod error;
pub mod facts;
pub mod finding;
pub mod operator;
pub mod path;
pub mod target;
pub mod template;

pub use compile::{CompiledRule, CompiledRuleSet, compile};
pub use context::{EvalContext, EvalTarget};
pub use engine::{EvaluationResult, evaluate, evaluate_with_source};
pub use error::{CompileIssue, EngineError};
pub use finding::build as build_finding;
