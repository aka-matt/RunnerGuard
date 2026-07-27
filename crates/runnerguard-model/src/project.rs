//! `MuleSoft` project model: descriptors, flow JSON, and the per-flow
//! component tree.

use crate::finding::Finding;
use crate::source::SourceSpan;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Top-level descriptor of a `MuleSoft` project, derived from `pom.xml` and
/// `mule-artifact.json`. Paths are repository-relative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDescriptor {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub mule_version: Option<String>,
    pub sdk_version: Option<String>,
    pub root_path: String,
    #[serde(default)]
    pub files: Vec<SourceFile>,
    #[serde(default)]
    pub required_files_missing: Vec<String>,
}

/// A single file the discovery stage found in the project tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub size_bytes: u64,
    pub is_text: bool,
    pub kind: SourceFileKind,
}

impl SourceFile {
    /// Convenience for tests and seed data.
    pub fn new(path: impl Into<String>, size_bytes: u64, kind: SourceFileKind) -> Self {
        let path = path.into();
        let is_text = matches!(
            kind,
            SourceFileKind::MuleXml
                | SourceFileKind::MunitXml
                | SourceFileKind::ResourceProperties
                | SourceFileKind::ResourceYaml
                | SourceFileKind::ResourceJson
                | SourceFileKind::ResourceDataweave
                | SourceFileKind::ResourceOther
                | SourceFileKind::Pom
                | SourceFileKind::MuleArtifact
                | SourceFileKind::OtherXml
                | SourceFileKind::OtherJson
                | SourceFileKind::OtherYaml
                | SourceFileKind::OtherText
        );
        Self {
            path,
            size_bytes,
            is_text,
            kind,
        }
    }

    pub fn label(&self) -> &'static str {
        self.kind.label()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFileKind {
    MuleXml,
    MunitXml,
    ResourceProperties,
    ResourceYaml,
    ResourceJson,
    ResourceDataweave,
    ResourceOther,
    Pom,
    MuleArtifact,
    OtherXml,
    OtherJson,
    OtherYaml,
    OtherText,
    Binary,
}

impl SourceFileKind {
    pub fn label(self) -> &'static str {
        self.as_label()
    }

    pub fn as_label(self) -> &'static str {
        match self {
            Self::MuleXml => "Mule XML",
            Self::MunitXml => "MUnit XML",
            Self::ResourceProperties => "Properties resource",
            Self::ResourceYaml => "YAML resource",
            Self::ResourceJson => "JSON resource",
            Self::ResourceDataweave => "DataWeave resource",
            Self::ResourceOther => "Other resource",
            Self::Pom => "Maven POM",
            Self::MuleArtifact => "Mule artifact descriptor",
            Self::OtherXml => "Other XML",
            Self::OtherJson => "Other JSON",
            Self::OtherYaml => "Other YAML",
            Self::OtherText => "Other text",
            Self::Binary => "Binary",
        }
    }
}

/// Parsed XML document after the namespace-aware reader has normalised
/// prefix bindings. Holds the resolved flow / subflow tree plus per-flow
/// facts. One document per source XML file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MuleDocument {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub source: SourceSpan,
    pub namespaces: BTreeMap<String, String>,
    pub flows: Vec<MuleFlow>,
    pub sub_flows: Vec<MuleFlow>,
    pub global_configurations: Vec<GlobalConfigIndex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MuleFlowKind {
    Flow,
    SubFlow,
}

impl MuleFlowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::SubFlow => "sub-flow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MuleFlow {
    pub schema_version: String,
    pub id: String,
    pub project_id: String,
    pub kind: MuleFlowKind,
    pub name: String,
    pub source: SourceSpan,
    pub namespaces: BTreeMap<String, String>,
    pub attributes: BTreeMap<String, String>,
    pub components: Vec<MuleComponent>,
    pub facts: FlowFacts,
    pub source_xml_sha256: String,
}

/// Component in the flow / subflow tree. Recursive `children` make this a
/// full representation of the original XML structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MuleComponent {
    pub id: String,
    pub qualified_name: String,
    pub local_name: String,
    pub namespace_uri: Option<String>,
    pub attributes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub cdata: Vec<String>,
    #[serde(default)]
    pub children: Vec<MuleComponent>,
    pub source: SourceSpan,
}

/// Pre-computed facts the rule engine reads. Building them at parse time
/// keeps the rule engine from walking the tree repeatedly.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FlowFacts {
    pub component_count: usize,
    pub max_component_depth: usize,
    pub flow_refs: Vec<FlowRef>,
    pub property_refs: Vec<String>,
    pub dataweave_blocks: Vec<DataWeaveBlock>,
    pub has_local_error_handler: bool,
    pub has_effective_error_handler: bool,
    pub source_component: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowRef {
    pub target: String,
    pub resolved: bool,
    pub source: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataWeaveBlock {
    pub language: String,
    pub version: Option<String>,
    pub content_hash: String,
    pub content: String,
    pub source: SourceSpan,
}

/// Top-level configuration element under `<mule>` (e.g.
/// `http:listener-config`). Tracked separately so the project index can
/// resolve `config-ref` lookups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalConfigIndex {
    pub qualified_name: String,
    pub name: String,
    pub source: SourceSpan,
}

/// The complete parsed project: documents + cross-reference index. This is
/// what the rule engine consumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedProject {
    pub schema_version: String,
    pub project: ProjectDescriptor,
    pub documents: Vec<MuleDocument>,
    pub index: ProjectIndex,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub flows: Vec<FlowIndexEntry>,
    pub sub_flows: Vec<FlowIndexEntry>,
    pub global_configurations: Vec<GlobalConfigIndex>,
    pub unresolved_flow_refs: Vec<UnresolvedFlowRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowIndexEntry {
    pub name: String,
    pub document_id: String,
    pub flow_id: String,
    pub kind: MuleFlowKind,
    pub source: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedFlowRef {
    pub target: String,
    pub referenced_from: FlowIndexEntry,
    pub source: SourceSpan,
}

/// Output the rule engine emits, used by the report renderer.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ArtifactIndex {
    pub findings: Vec<Finding>,
    pub flow_artifacts: Vec<FlowArtifactRef>,
    pub diagnostics: Vec<crate::diagnostic::Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowArtifactRef {
    pub flow_id: String,
    pub artifact_path: String,
    pub sha256: String,
}

// We deliberately use `BTreeMap` for `namespaces` and `attributes` so that
// JSON output is in lexicographic key order — that's our stable ordering
// across runs. Do not "optimise" these with HashMap.
