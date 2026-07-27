//! Evaluation context: the data the rule engine reads while a rule fires.
//!
//! A context is one of the target entities the rule is iterating over
//! (a project, file, flow, component, …) plus the parsed project it
//! belongs to. Facts are resolved out of this struct — there is no other
//! place the rule engine looks for state.

use runnerguard_model::{
    DataWeaveBlock, FlowRef, MuleComponent, MuleFlow, ParsedProject, ProjectDescriptor, SourceFile,
};
use serde_json::Value;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub enum EvalTarget {
    Project(ProjectDescriptor),
    File(SourceFile),
    Flow(MuleFlow),
    Component(MuleComponent),
    PropertyRef { flow_name: String, value: String },
    FlowRef(FlowRef),
    DataweaveBlock(DataWeaveBlock),
}

#[derive(Debug)]
pub struct EvalContext<'a> {
    pub project: &'a ParsedProject,
    pub target: EvalTarget,
    /// Optional expected value for the condition currently being evaluated.
    /// Set by the engine before each `facts::resolve` call; cleared when
    /// the condition is done. Lets facts that *need* the rule's value to
    /// produce a meaningful `actual` (e.g. `project.required-files`)
    /// compute their value. Wrapped in a `RefCell` so the engine can
    /// update it through the shared `&EvalContext` reference while the
    /// resolver can read it without consuming it.
    pub current_expected: RefCell<Option<Value>>,
}

impl<'a> EvalContext<'a> {
    pub fn new(project: &'a ParsedProject, target: EvalTarget) -> Self {
        Self {
            project,
            target,
            current_expected: RefCell::new(None),
        }
    }

    pub fn target_label(&self) -> &'static str {
        match self.target {
            EvalTarget::Project(_) => "project",
            EvalTarget::File(_) => "file",
            EvalTarget::Flow(_) => "flow",
            EvalTarget::Component(_) => "component",
            EvalTarget::PropertyRef { .. } => "property-reference",
            EvalTarget::FlowRef(_) => "flow-reference",
            EvalTarget::DataweaveBlock(_) => "dataweave-block",
        }
    }
}
