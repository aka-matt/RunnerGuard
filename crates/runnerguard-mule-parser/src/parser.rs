//! XML → [`ParsedProject`] implementation backed by `quick-xml`.

use crate::facts::compute;
use crate::index::build as build_index;
use crate::options::ParseOptions;
use crate::project::ParseError as ProjectParseError;
use crate::project::ParsedOutcome;
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use runnerguard_fs::ProjectFiles;
use runnerguard_fs::SourceFileKind;
use runnerguard_model::{
    CURRENT_SCHEMA_VERSION, Diagnostic, DiagnosticLevel, DiagnosticStage, GlobalConfigIndex,
    MuleComponent, MuleDocument, MuleFlow, MuleFlowKind, ParsedProject, ProjectDescriptor,
    SourceSpan,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub fn parse_project(
    files: &ProjectFiles,
    options: &ParseOptions,
) -> Result<ParsedOutcome, ProjectParseError> {
    let mut diagnostics = Vec::new();
    let mut documents = Vec::new();

    for source_file in files.iter() {
        if source_file.kind != SourceFileKind::MuleXml {
            continue;
        }
        // Reject path traversal: the joined path must resolve under files.root.
        let joined = files.root.join(&source_file.path);
        let joined = match joined.canonicalize() {
            Ok(p) => p,
            Err(_) => joined, // fall back; read errors are surfaced below
        };
        let path = match ensure_within_root(&files.root, &joined) {
            Ok(p) => p,
            Err(err) => {
                diagnostics.push(err);
                if !options.continue_on_parse_error {
                    return Err(ProjectParseError::TooManyFailures {
                        count: diagnostics.len(),
                    });
                }
                continue;
            }
        };
        match read_and_parse(&path, &source_file.path, options) {
            Ok(doc) => documents.push(doc),
            Err(err) => {
                diagnostics.push(err);
                if !options.continue_on_parse_error {
                    return Err(ProjectParseError::TooManyFailures {
                        count: diagnostics.len(),
                    });
                }
            }
        }
    }

    if documents.is_empty() {
        diagnostics.push(Diagnostic::new(
            DiagnosticStage::Mule,
            "MULE-001",
            DiagnosticLevel::Error,
            "No Mule XML files were found under src/main/mule.",
        ));
    }

    // Mark every `flow-ref` against the project-wide name set BEFORE the
    // index is built — this way both the per-flow `FlowFacts.flow_refs`
    // and the project-wide `ProjectIndex.unresolved_flow_refs` agree.
    let known = crate::reference::collect_known_names(&documents);
    for doc in &mut documents {
        for flow in &mut doc.flows {
            let components = std::mem::take(&mut flow.components);
            let mut facts = std::mem::take(&mut flow.facts.flow_refs);
            let mut new_components = components;
            for c in &mut new_components {
                crate::reference::mark_resolved_in_flow(c, &mut facts, &known);
            }
            flow.components = new_components;
            flow.facts.flow_refs = facts;
        }
        for sub in &mut doc.sub_flows {
            let components = std::mem::take(&mut sub.components);
            let mut facts = std::mem::take(&mut sub.facts.flow_refs);
            let mut new_components = components;
            for c in &mut new_components {
                crate::reference::mark_resolved_in_flow(c, &mut facts, &known);
            }
            sub.components = new_components;
            sub.facts.flow_refs = facts;
        }
    }

    let index = build_index(&documents);
    let project = build_project_descriptor(files, &documents);

    let parsed = ParsedProject {
        schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        project,
        documents,
        index,
    };
    Ok(ParsedOutcome {
        project: parsed,
        diagnostics,
    })
}

/// Ensure a joined path stays within the project root. Rejects `..` escapes
/// and absolute paths that point outside `root`. Returns a diagnostic on
/// violation rather than panicking. The `Diagnostic` Err is intentionally
/// large — it carries enough context to surface to operators.
#[allow(clippy::result_large_err)]
fn ensure_within_root(root: &Path, joined: &Path) -> Result<std::path::PathBuf, Diagnostic> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical_joined = joined
        .canonicalize()
        .unwrap_or_else(|_| joined.to_path_buf());
    if !canonical_joined.starts_with(&canonical_root) {
        return Err(Diagnostic::new(
            DiagnosticStage::Mule,
            "MULE-005",
            DiagnosticLevel::Error,
            format!(
                "Source file path escapes project root: {}",
                joined.display()
            ),
        ));
    }
    Ok(canonical_joined)
}

/// Count newlines in `text[last_offset..current_offset]` and update
/// the running `line` counter. Returns the (line, column) pair for
/// `current_offset`. Used by [`read_and_parse`] to give every
/// emitted source span a real line number rather than the hard-coded
/// `1` the previous implementation produced.
fn line_column_for(
    text: &str,
    current_offset: u64,
    line: &mut u32,
    line_start_offset: &mut u64,
) -> (u32, u32) {
    let start = (*line_start_offset) as usize;
    let end = (current_offset as usize).min(text.len());
    if start < end {
        for (i, b) in text.as_bytes()[start..end].iter().enumerate() {
            if *b == b'\n' {
                *line += 1;
                *line_start_offset = (start + i + 1) as u64;
            }
        }
    }
    let col = (current_offset - *line_start_offset) + 1;
    (*line, col as u32)
}

#[allow(clippy::result_large_err)]
fn read_and_parse(
    path: &Path,
    relative: &str,
    options: &ParseOptions,
) -> Result<MuleDocument, Diagnostic> {
    let bytes = std::fs::read(path).map_err(|source| diagnostic_io(relative, source))?;
    if bytes.len() as u64 > options.limits.max_file_bytes {
        return Err(Diagnostic::new(
            DiagnosticStage::Xml,
            "XML-001",
            DiagnosticLevel::Error,
            format!(
                "File exceeds max_file_bytes ({} > {}) in {relative}",
                bytes.len(),
                options.limits.max_file_bytes
            ),
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|source| {
        Diagnostic::new(
            DiagnosticStage::Xml,
            "XML-003",
            DiagnosticLevel::Error,
            format!("File is not valid UTF-8: {source}"),
        )
    })?;

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<ElementFrame> = Vec::new();
    let mut top_level: Vec<MuleComponent> = Vec::new();
    let mut flows: Vec<MuleFlow> = Vec::new();
    let mut sub_flows: Vec<MuleFlow> = Vec::new();
    let mut component_counter: usize = 0;
    let mut depth: usize = 0;
    // Real line / column tracking. Each event's `buffer_position()`
    // is the absolute byte offset in the input; the newlines since
    // the previous event are counted and the running `line` is
    // updated. The previous implementation hard-coded `line = 1` for
    // every component, which made every source span useless for
    // multi-file or multi-line XML.
    let mut line: u32 = 1;
    let mut line_start_offset: u64 = 0;

    loop {
        let before_offset = reader.buffer_position();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                if depth > options.limits.max_xml_depth {
                    return Err(Diagnostic::new(
                        DiagnosticStage::Xml,
                        "XML-002",
                        DiagnosticLevel::Error,
                        format!(
                            "XML depth exceeds {} in {relative}",
                            options.limits.max_xml_depth
                        ),
                    ));
                }
                let (qualified, local, attributes) = describe_start(&e);
                let span = line_column_for(text, before_offset, &mut line, &mut line_start_offset);
                let source = SourceSpan::point(relative, span.0, span.1);
                component_counter += 1;
                let component = MuleComponent {
                    id: format!("component:{component_counter:04}"),
                    qualified_name: qualified,
                    local_name: local,
                    namespace_uri: None,
                    attributes,
                    text: None,
                    cdata: Vec::new(),
                    children: Vec::new(),
                    source,
                };
                stack.push(ElementFrame { component });
            }
            Ok(Event::Empty(e)) => {
                let (qualified, local, attributes) = describe_start(&e);
                let span = line_column_for(text, before_offset, &mut line, &mut line_start_offset);
                let source = SourceSpan::point(relative, span.0, span.1);
                component_counter += 1;
                let component = MuleComponent {
                    id: format!("component:{component_counter:04}"),
                    qualified_name: qualified,
                    local_name: local,
                    namespace_uri: None,
                    attributes,
                    text: None,
                    cdata: Vec::new(),
                    children: Vec::new(),
                    source,
                };
                finish_component(
                    &mut stack,
                    &mut top_level,
                    &mut flows,
                    &mut sub_flows,
                    component,
                );
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if let Some(frame) = stack.pop() {
                    finish_component(
                        &mut stack,
                        &mut top_level,
                        &mut flows,
                        &mut sub_flows,
                        frame.component,
                    );
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(frame) = stack.last_mut() {
                    // Accumulate (rather than overwrite) so adjacent
                    // text runs such as `<x>before<y/>after</x>`
                    // don't lose `before`. CDATA is collected
                    // separately — it must not be treated as text.
                    let piece = t.unescape().unwrap_or_default().to_string();
                    if !piece.is_empty() {
                        match frame.component.text.as_mut() {
                            Some(existing) => existing.push_str(&piece),
                            None => frame.component.text = Some(piece),
                        }
                    }
                }
            }
            Ok(Event::CData(c)) => {
                if let Some(frame) = stack.last_mut() {
                    let raw = std::str::from_utf8(&c).unwrap_or("").to_string();
                    frame.component.cdata.push(raw);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                return Err(Diagnostic::new(
                    DiagnosticStage::Xml,
                    "XML-001",
                    DiagnosticLevel::Error,
                    format!("XML syntax error in {relative}: {e}"),
                ));
            }
        }
        buf.clear();
    }

    let mut global_configurations = Vec::new();
    for c in &top_level {
        if let Some(name) = c.attributes.get("name") {
            global_configurations.push(GlobalConfigIndex {
                qualified_name: c.qualified_name.clone(),
                name: name.clone(),
                source: c.source.clone(),
            });
        }
    }

    Ok(MuleDocument {
        schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        id: format!("document:{relative}"),
        project_id: String::new(),
        source: SourceSpan::point(relative, 1, 1),
        namespaces: BTreeMap::new(),
        flows,
        sub_flows,
        global_configurations,
    })
}

/// Push a fully-closed `component` into the right home: nested under its
/// parent if the stack isn't empty, otherwise it's a top-level element
/// that may be a `flow`, a `sub-flow`, or a global config.
///
/// The `<mule>` document root is a transparent passthrough: when it
/// closes, its direct children are redistributed so flows/sub-flows and
/// global configurations are detected at the document level.
fn finish_component(
    stack: &mut [ElementFrame],
    top_level: &mut Vec<MuleComponent>,
    flows: &mut Vec<MuleFlow>,
    sub_flows: &mut Vec<MuleFlow>,
    component: MuleComponent,
) {
    if let Some((parent, _)) = stack.split_last_mut() {
        parent.component.children.push(component);
        return;
    }
    if component.local_name == "mule" {
        let children = component.children;
        for child in children {
            finish_component(stack, top_level, flows, sub_flows, child);
        }
        return;
    }
    match component.local_name.as_str() {
        "flow" => flows.push(into_flow(component, MuleFlowKind::Flow)),
        "sub-flow" => sub_flows.push(into_flow(component, MuleFlowKind::SubFlow)),
        _ => top_level.push(component),
    }
}

fn into_flow(component: MuleComponent, kind: MuleFlowKind) -> MuleFlow {
    let name = component
        .attributes
        .get("name")
        .cloned()
        .unwrap_or_default();
    let facts = compute(&component);
    let source_xml_sha256 = component_source_hash(&component);
    MuleFlow {
        schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        id: format!("flow:{name}"),
        project_id: String::new(),
        kind,
        name,
        source: component.source.clone(),
        namespaces: BTreeMap::new(),
        attributes: component.attributes.clone(),
        components: vec![component],
        facts,
        source_xml_sha256,
    }
}

fn describe_start(
    e: &quick_xml::events::BytesStart<'_>,
) -> (String, String, BTreeMap<String, String>) {
    let name: QName = e.name();
    let raw = std::str::from_utf8(name.as_ref()).unwrap_or("");
    let (prefix, local) = match raw.split_once(':') {
        Some((p, l)) => (p, l.to_string()),
        None => ("", raw.to_string()),
    };
    let qualified = if prefix.is_empty() {
        local.clone()
    } else {
        format!("{prefix}:{local}")
    };
    let attributes = collect_attributes(e);
    (qualified, local, attributes)
}

fn collect_attributes(e: &quick_xml::events::BytesStart<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref())
            .unwrap_or("")
            .to_string();
        let value = attr
            .unescape_value()
            .map(|c| c.to_string())
            .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value).to_string());
        out.insert(key, value);
    }
    out
}

struct ElementFrame {
    component: MuleComponent,
}

fn diagnostic_io(relative: &str, source: std::io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticStage::FileSystem,
        "FS-002",
        DiagnosticLevel::Error,
        format!("Failed to read {relative}: {source}"),
    )
}

fn build_project_descriptor(
    files: &ProjectFiles,
    _documents: &[MuleDocument],
) -> ProjectDescriptor {
    use runnerguard_model::SourceFile;
    let project_id = format!("project:{}", sha_short(&files.root.to_string_lossy()));
    let name = files
        .root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let sources: Vec<SourceFile> = files
        .iter()
        .map(|f| SourceFile {
            path: f.path.clone(),
            size_bytes: f.size_bytes,
            is_text: f.is_text,
            kind: f.kind,
        })
        .collect();

    // The "required files missing" diff is computed by the rule engine
    // against the rule's expected list — the parser does not own it.
    // It is kept as a placeholder field on the descriptor so older
    // consumers still see the right shape.
    ProjectDescriptor {
        schema_version: CURRENT_SCHEMA_VERSION.to_string(),
        id: project_id,
        name,
        group_id: None,
        artifact_id: None,
        version: None,
        mule_version: None,
        sdk_version: None,
        root_path: files.root.to_string_lossy().to_string(),
        files: sources,
        required_files_missing: Vec::new(),
    }
}

fn sha_short(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    hex_lower(&bytes[..6])
}

fn component_source_hash(component: &MuleComponent) -> String {
    let mut hasher = Sha256::new();
    hash_component(component, &mut hasher);
    format!("sha256:{}", hex_lower(&hasher.finalize()))
}

fn hash_component(component: &MuleComponent, hasher: &mut Sha256) {
    hasher.update(component.qualified_name.as_bytes());
    hasher.update(b"\n");
    for (k, v) in &component.attributes {
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
        hasher.update(b"\n");
    }
    for c in &component.cdata {
        hasher.update(c.as_bytes());
    }
    for child in &component.children {
        hash_component(child, hasher);
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
