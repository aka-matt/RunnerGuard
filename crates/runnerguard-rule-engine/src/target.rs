//! Enumerate candidate target entities for each rule.
//!
//! The rule engine never walks the project looking for "things to test";
//! instead, for each rule it asks the target iterator for every entity
//! the rule might fire against. This keeps evaluation deterministic and
//! the per-target work localised.

use crate::context::EvalTarget;
use runnerguard_model::{MuleComponent, MuleFlow, ParsedProject, RuleTarget, RuleTargetEntity};
use std::collections::BTreeSet;

/// One target the engine has selected for a rule.
pub struct Candidate {
    pub target: EvalTarget,
}

/// Walk the project, yielding one [`Candidate`] per matching entity. The
/// `target` field of the rule says which kinds of entity to enumerate;
/// optional `match_` further filters inside each entity.
pub fn candidates<'a>(project: &'a ParsedProject, target: &RuleTarget) -> Vec<Candidate> {
    let mut out = Vec::new();
    for entity in &target.entity {
        match entity {
            RuleTargetEntity::Project => out.push(Candidate {
                target: EvalTarget::Project(project.project.clone()),
            }),
            RuleTargetEntity::File => {
                for f in &project.project.files {
                    out.push(Candidate {
                        target: EvalTarget::File(f.clone()),
                    });
                }
            }
            RuleTargetEntity::Flow => {
                for d in &project.documents {
                    for f in &d.flows {
                        out.push(Candidate {
                            target: EvalTarget::Flow(f.clone()),
                        });
                    }
                }
            }
            RuleTargetEntity::Subflow => {
                for d in &project.documents {
                    for f in &d.sub_flows {
                        out.push(Candidate {
                            target: EvalTarget::Flow(f.clone()),
                        });
                    }
                }
            }
            RuleTargetEntity::Component => {
                for d in &project.documents {
                    for f in d.flows.iter().chain(d.sub_flows.iter()) {
                        for c in &f.components {
                            push_components(c, &mut out);
                        }
                    }
                }
            }
            RuleTargetEntity::PropertyReference => {
                for d in &project.documents {
                    for f in d.flows.iter().chain(d.sub_flows.iter()) {
                        for p in &f.facts.property_refs {
                            out.push(Candidate {
                                target: EvalTarget::PropertyRef {
                                    flow_name: f.name.clone(),
                                    value: p.clone(),
                                },
                            });
                        }
                    }
                }
            }
            RuleTargetEntity::FlowReference => {
                for d in &project.documents {
                    for f in d.flows.iter().chain(d.sub_flows.iter()) {
                        for r in &f.facts.flow_refs {
                            out.push(Candidate {
                                target: EvalTarget::FlowRef(r.clone()),
                            });
                        }
                    }
                }
            }
            RuleTargetEntity::DataweaveBlock => {
                for d in &project.documents {
                    for f in d.flows.iter().chain(d.sub_flows.iter()) {
                        for b in &f.facts.dataweave_blocks {
                            out.push(Candidate {
                                target: EvalTarget::DataweaveBlock(b.clone()),
                            });
                        }
                    }
                }
            }
        }
    }
    out
}

fn push_components(root: &MuleComponent, out: &mut Vec<Candidate>) {
    out.push(Candidate {
        target: EvalTarget::Component(root.clone()),
    });
    for child in &root.children {
        push_components(child, out);
    }
}

/// Stable, deterministic ordering of candidates. The engine relies on
/// this so two scans over the same project produce identical finding
/// lists.
pub fn sort(candidates: &mut [Candidate]) {
    candidates.sort_by(|a, b| key(a).cmp(&key(b)));
}

fn key(c: &Candidate) -> String {
    match &c.target {
        EvalTarget::Project(p) => format!("project::{}", p.id),
        EvalTarget::File(f) => format!("file::{}", f.path),
        EvalTarget::Flow(f) => format!("flow::{}::{}", f.source.file, f.id),
        EvalTarget::Component(c) => format!("component::{}::{}", c.source.file, c.id),
        EvalTarget::PropertyRef { flow_name, value } => {
            format!("property::{flow_name}::{value}")
        }
        EvalTarget::FlowRef(r) => format!("reference::{}::{}", r.source.file, r.target),
        EvalTarget::DataweaveBlock(b) => {
            format!("dataweave::{}::{}", b.source.file, b.content_hash)
        }
    }
}

/// Optional `match_` clause — used by callers after constructing the
/// [`EvalContext`]. We don't execute the predicate here because the
/// engine wants to evaluate the `match_` condition with the same context
/// it uses for `when`/`assert`.
pub fn match_clause(target: &RuleTarget) -> Option<&runnerguard_model::Condition> {
    target.match_.as_deref()
}

/// Whether the candidate's target entity kind is in the rule's target
/// list. We use this when `target.entity` lists more than one kind.
pub fn entity_in(entities: &[RuleTargetEntity], candidate: &Candidate) -> bool {
    let kind = match &candidate.target {
        EvalTarget::Project(_) => RuleTargetEntity::Project,
        EvalTarget::File(_) => RuleTargetEntity::File,
        EvalTarget::Flow(_) => RuleTargetEntity::Flow,
        EvalTarget::Component(_) => RuleTargetEntity::Component,
        EvalTarget::PropertyRef { .. } => RuleTargetEntity::PropertyReference,
        EvalTarget::FlowRef(_) => RuleTargetEntity::FlowReference,
        EvalTarget::DataweaveBlock(_) => RuleTargetEntity::DataweaveBlock,
    };
    entities.contains(&kind)
}

/// Helper for tests: project a flow into the target list's expected kind.
pub fn flow_kind(flow: &MuleFlow) -> RuleTargetEntity {
    match flow.kind {
        runnerguard_model::MuleFlowKind::Flow => RuleTargetEntity::Flow,
        runnerguard_model::MuleFlowKind::SubFlow => RuleTargetEntity::Subflow,
    }
}

/// Stable ordering helper for callers that want a `BTreeSet` of names
/// without writing it inline.
pub fn unique_names(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values.into_iter().collect()
}
