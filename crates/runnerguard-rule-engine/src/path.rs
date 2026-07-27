//! Whitelist of fact paths the rule engine understands.
//!
//! Paths are dotted identifiers whose **first segment** names a target
//! context (`project`, `file`, `flow`, `component`, `property`,
//! `reference`, `dataweave`). The remainder is a small, fixed grammar
//! resolved per-context. Anything outside the whitelist is a compile
//! error — that is the contract the rule DSL promises.

use crate::error::EngineError;
use runnerguard_model::{FactPath, RuleTargetEntity};
use std::collections::BTreeSet;

/// Whole whitelist, per target.
pub fn whitelist(target: RuleTargetEntity) -> &'static BTreeSet<FactPath> {
    match target {
        RuleTargetEntity::Project => project(),
        RuleTargetEntity::File => file(),
        RuleTargetEntity::Flow | RuleTargetEntity::Subflow => flow(),
        RuleTargetEntity::Component => component(),
        RuleTargetEntity::PropertyReference => property(),
        RuleTargetEntity::FlowReference => reference(),
        RuleTargetEntity::DataweaveBlock => dataweave(),
    }
}

/// True if the path is legal in any target. Used for template variable
/// substitution where the context is unknown.
pub fn any(path: &FactPath) -> bool {
    [
        project(),
        file(),
        flow(),
        component(),
        property(),
        reference(),
        dataweave(),
    ]
    .iter()
    .any(|set| set.contains(path))
}

fn project() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        [
            "project.name",
            "project.required-files",
            "project.unresolved-flow-refs",
            "project.flow-count",
            "project.subflow-count",
        ]
        .into_iter()
        .map(FactPath::from)
        .collect()
    })
}
fn file() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        ["file.path", "file.extension", "file.kind"]
            .into_iter()
            .map(FactPath::from)
            .collect()
    })
}
fn flow() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        [
            "flow.name",
            "flow.kind",
            "flow.component-count",
            "flow.max-component-depth",
            "flow.has-local-error-handler",
            "flow.has-effective-error-handler",
            "flow.flow-refs",
            "flow.property-refs",
            "flow.source-component",
        ]
        .into_iter()
        .map(FactPath::from)
        .collect()
    })
}
fn component() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        [
            "component.qualified-name",
            "component.local-name",
            "component.namespace-uri",
            "component.text",
            "component.cdata",
        ]
        .into_iter()
        .map(FactPath::from)
        .collect()
    })
}
fn property() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        ["property.name", "property.value"]
            .into_iter()
            .map(FactPath::from)
            .collect()
    })
}
fn reference() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        ["reference.target", "reference.resolved"]
            .into_iter()
            .map(FactPath::from)
            .collect()
    })
}
fn dataweave() -> &'static BTreeSet<FactPath> {
    static SET: std::sync::OnceLock<BTreeSet<FactPath>> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        ["dataweave.language", "dataweave.content"]
            .into_iter()
            .map(FactPath::from)
            .collect()
    })
}

/// Touch all `OnceLock`s at startup so first evaluation doesn't pay the
/// cost.
pub fn warm() {
    let _ = (
        project(),
        file(),
        flow(),
        component(),
        property(),
        reference(),
        dataweave(),
    );
}

/// Compile-time-style check: returns an error if the path isn't allowed for
/// the given target.
pub fn ensure_allowed(target: RuleTargetEntity, path: &FactPath) -> Result<(), EngineError> {
    if whitelist(target).contains(path) {
        return Ok(());
    }
    // Allow arbitrary `component.attributes.<name>` lookups — there are
    // too many Mule attribute keys to enumerate. The compiler still has
    // to prove the prefix is legal.
    if target == RuleTargetEntity::Component && path.as_str().starts_with("component.attributes.") {
        return Ok(());
    }
    Err(EngineError::Compile {
        rule_id: String::new(),
        message: format!(
            "unknown fact path `{}` for target `{}`",
            path,
            target_label(target)
        ),
    })
}

fn target_label(target: RuleTargetEntity) -> &'static str {
    match target {
        RuleTargetEntity::Project => "project",
        RuleTargetEntity::File => "file",
        RuleTargetEntity::Flow => "flow",
        RuleTargetEntity::Subflow => "subflow",
        RuleTargetEntity::Component => "component",
        RuleTargetEntity::PropertyReference => "property-reference",
        RuleTargetEntity::FlowReference => "flow-reference",
        RuleTargetEntity::DataweaveBlock => "dataweave-block",
    }
}
