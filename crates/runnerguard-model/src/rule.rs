//! JSON rule DSL types. The rule set is JSON-shaped with a small, whitelisted
//! vocabulary; nothing in this crate evaluates rules (that's
//! `runnerguard-rule-engine`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub defaults: RuleDefaults,
    pub rules: Vec<Rule>,
}

impl crate::SchemaVersioned for RuleSet {
    fn schema_version() -> &'static str {
        crate::CURRENT_SCHEMA_VERSION
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RuleDefaults {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_severity")]
    pub severity: super::finding::Severity,
}

fn default_enabled() -> bool {
    true
}
fn default_severity() -> super::finding::Severity {
    super::finding::Severity::Warning
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub severity: super::finding::Severity,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub target: RuleTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub r#assert: Condition,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleTarget {
    pub entity: Vec<RuleTargetEntity>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "match",
        alias = "match"
    )]
    pub match_: Option<Box<Condition>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleTargetEntity {
    Project,
    File,
    Flow,
    Subflow,
    Component,
    PropertyReference,
    FlowReference,
    DataweaveBlock,
}

/// Condition AST. JSON deserialises into either a single-condition object or
/// one of the combinators. We use `#[serde(untagged)]` so rule files can
/// write either style without an extra discriminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Condition {
    Combinator(ConditionCombinator),
    Single(SingleCondition),
}

impl From<SingleCondition> for Condition {
    fn from(value: SingleCondition) -> Self {
        Self::Single(value)
    }
}

impl From<ConditionCombinator> for Condition {
    fn from(value: ConditionCombinator) -> Self {
        Self::Combinator(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConditionCombinator {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SingleCondition {
    pub fact: FactPath,
    pub op: Operator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// A fact path is a dotted, whitelisted identifier. We keep this as a
/// newtype around `String` so the rule engine can reject unknown facts at
/// compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FactPath(pub String);

impl FactPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// First segment of the path (e.g. `flow`, `component`, `project`).
    pub fn root(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }
}

impl From<&str> for FactPath {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for FactPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for FactPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Operator whitelist. Anything outside this enum fails rule compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operator {
    Exists,
    NotExists,
    Equals,
    NotEquals,
    Matches,
    NotMatches,
    Contains,
    NotContains,
    In,
    NotIn,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    CountEquals,
    CountLessThanOrEqual,
    IsPlaceholder,
    IsNotPlaceholder,
    ContainsComponent,
    NotContainsComponent,
    ReferenceResolves,
    AllReferencesResolve,
    RequiredFilesExist,
}

impl Operator {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exists => "exists",
            Self::NotExists => "not-exists",
            Self::Equals => "equals",
            Self::NotEquals => "not-equals",
            Self::Matches => "matches",
            Self::NotMatches => "not-matches",
            Self::Contains => "contains",
            Self::NotContains => "not-contains",
            Self::In => "in",
            Self::NotIn => "not-in",
            Self::GreaterThan => "greater-than",
            Self::GreaterThanOrEqual => "greater-than-or-equal",
            Self::LessThan => "less-than",
            Self::LessThanOrEqual => "less-than-or-equal",
            Self::CountEquals => "count-equals",
            Self::CountLessThanOrEqual => "count-less-than-or-equal",
            Self::IsPlaceholder => "is-placeholder",
            Self::IsNotPlaceholder => "is-not-placeholder",
            Self::ContainsComponent => "contains-component",
            Self::NotContainsComponent => "not-contains-component",
            Self::ReferenceResolves => "reference-resolves",
            Self::AllReferencesResolve => "all-references-resolve",
            Self::RequiredFilesExist => "required-files-exist",
        }
    }
}
