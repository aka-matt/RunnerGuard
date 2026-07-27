//! Build the project-wide [`ProjectIndex`] and resolve cross-flow
//! references once parsing is done.

use runnerguard_model::{
    FlowIndexEntry, MuleDocument, MuleFlow, MuleFlowKind, ProjectIndex, UnresolvedFlowRef,
};
use std::collections::HashMap;

pub fn build(documents: &[MuleDocument]) -> ProjectIndex {
    let mut by_name: HashMap<String, &MuleFlow> = HashMap::new();
    let mut by_doc: HashMap<String, &MuleDocument> = HashMap::new();

    let mut flows = Vec::new();
    let mut sub_flows = Vec::new();

    for doc in documents {
        by_doc.insert(doc.id.clone(), doc);
        for flow in &doc.flows {
            flows.push(FlowIndexEntry {
                name: flow.name.clone(),
                document_id: doc.id.clone(),
                flow_id: flow.id.clone(),
                kind: MuleFlowKind::Flow,
                source: flow.source.clone(),
            });
            by_name.insert(flow.name.clone(), flow);
        }
        for sub in &doc.sub_flows {
            sub_flows.push(FlowIndexEntry {
                name: sub.name.clone(),
                document_id: doc.id.clone(),
                flow_id: sub.id.clone(),
                kind: MuleFlowKind::SubFlow,
                source: sub.source.clone(),
            });
            by_name.insert(sub.name.clone(), sub);
        }
    }

    let mut unresolved = Vec::new();
    for doc in documents {
        for flow in &doc.flows {
            for r in &flow.facts.flow_refs {
                if !by_name.contains_key(&r.target) {
                    unresolved.push(UnresolvedFlowRef {
                        target: r.target.clone(),
                        referenced_from: FlowIndexEntry {
                            name: flow.name.clone(),
                            document_id: doc.id.clone(),
                            flow_id: flow.id.clone(),
                            kind: MuleFlowKind::Flow,
                            source: r.source.clone(),
                        },
                        source: r.source.clone(),
                    });
                }
            }
        }
        for sub in &doc.sub_flows {
            for r in &sub.facts.flow_refs {
                if !by_name.contains_key(&r.target) {
                    unresolved.push(UnresolvedFlowRef {
                        target: r.target.clone(),
                        referenced_from: FlowIndexEntry {
                            name: sub.name.clone(),
                            document_id: doc.id.clone(),
                            flow_id: sub.id.clone(),
                            kind: MuleFlowKind::SubFlow,
                            source: r.source.clone(),
                        },
                        source: r.source.clone(),
                    });
                }
            }
        }
    }

    ProjectIndex {
        flows,
        sub_flows,
        global_configurations: documents
            .iter()
            .flat_map(|d| d.global_configurations.iter().cloned())
            .collect(),
        unresolved_flow_refs: unresolved,
    }
}
