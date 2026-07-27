//! End-to-end parser tests: build a small Mule project on disk, parse it,
//! and assert on the resulting [`ParsedProject`].

use runnerguard_fs::SourceFile;
use runnerguard_model::SourceFileKind;
use runnerguard_mule_parser::{ParseOptions, parse_project};

fn xml_for(_name: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<mule xmlns="http://www.mulesoft.org/schema/mule/core"
      xmlns:http="http://www.mulesoft.org/schema/mule/http"
      xmlns:ee="http://www.mulesoft.org/schema/mule/ee/core"
      xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">

{body}
</mule>
"#
    )
}

#[test]
fn parses_a_simple_http_flow() {
    let dir = tempfile::tempdir().unwrap();
    let body = r#"
    <http:listener-config name="httpListenerConfig">
        <http:listener-connection host="0.0.0.0" port="${http.port}" />
    </http:listener-config>

    <flow name="order-api-flow">
        <http:listener config-ref="httpListenerConfig" path="/orders" />
        <set-payload value="ok" />
    </flow>
"#;
    std::fs::write(dir.path().join("order-api.xml"), xml_for("order-api", body)).unwrap();

    let mut files = runnerguard_fs::ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile::new(
        "order-api.xml",
        body.len() as u64,
        SourceFileKind::MuleXml,
    ));
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    assert!(
        parsed.diagnostics.is_empty(),
        "diagnostics: {:?}",
        parsed.diagnostics
    );
    assert_eq!(parsed.project.documents.len(), 1);
    let doc = &parsed.project.documents[0];
    assert_eq!(doc.flows.len(), 1);
    assert_eq!(doc.flows[0].name, "order-api-flow");
    let facts = &doc.flows[0].facts;
    assert!(facts.component_count >= 2, "got {}", facts.component_count);
    assert!(!facts.has_local_error_handler);
    assert_eq!(facts.source_component.as_deref(), Some("http:listener"));
}

#[test]
fn parses_subflow_and_records_unresolved_reference() {
    let dir = tempfile::tempdir().unwrap();
    let body = r#"
    <flow name="caller">
        <flow-ref name="missing-subflow" />
    </flow>
"#;
    std::fs::write(dir.path().join("caller.xml"), xml_for("caller", body)).unwrap();
    let mut files = runnerguard_fs::ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile::new(
        "caller.xml",
        body.len() as u64,
        SourceFileKind::MuleXml,
    ));
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    let doc = &parsed.project.documents[0];
    assert_eq!(doc.flows.len(), 1);
    assert_eq!(doc.flows[0].facts.flow_refs.len(), 1);
    assert_eq!(doc.flows[0].facts.flow_refs[0].target, "missing-subflow");
    assert_eq!(parsed.project.index.unresolved_flow_refs.len(), 1);
}

#[test]
fn dataweave_cdata_is_extracted() {
    let dir = tempfile::tempdir().unwrap();
    let body = r#"
    <flow name="transform">
        <set-payload><![CDATA[
%dw 2.0
output application/json
---
{ status: "ok" }
        ]]></set-payload>
    </flow>
"#;
    std::fs::write(dir.path().join("transform.xml"), xml_for("transform", body)).unwrap();
    let mut files = runnerguard_fs::ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile::new(
        "transform.xml",
        body.len() as u64,
        SourceFileKind::MuleXml,
    ));
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    let doc = &parsed.project.documents[0];
    assert_eq!(doc.flows[0].facts.dataweave_blocks.len(), 1);
    assert!(
        doc.flows[0].facts.dataweave_blocks[0]
            .content
            .contains("%dw 2.0")
    );
}

#[test]
fn invalid_xml_emits_a_diagnostic_and_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("broken.xml"), "<mule><flow").unwrap();
    let mut files = runnerguard_fs::ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile::new("broken.xml", 11, SourceFileKind::MuleXml));
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    assert!(!parsed.diagnostics.is_empty());
}

#[test]
fn empty_project_yields_diagnostic_but_no_panic() {
    let dir = tempfile::tempdir().unwrap();
    let files = runnerguard_fs::ProjectFiles::new(dir.path().to_path_buf());
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("No Mule XML"))
    );
}

// Pull in PathBuf so the unused-import lint stays quiet when SourceFileKind
// is the alias target.
#[allow(dead_code)]
fn _kind_match() {
    let _: SourceFileKind = SourceFileKind::MuleXml;
}

#[test]
fn source_spans_track_real_line_numbers() {
    use runnerguard_fs::{ProjectFiles, SourceFile, SourceFileKind};
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let mule_dir = dir.path().join("src/main/mule");
    fs::create_dir_all(&mule_dir).unwrap();
    // The flow element is on line 6 — every previous implementation
    // hard-coded `line = 1` so the resulting source span was useless.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<mule xmlns="http://www.mulesoft.org/schema/mule/core"
      xmlns:http="http://www.mulesoft.org/schema/mule/http">

<!-- line 5 -->
<flow name="late-flow">
    <http:listener config-ref="httpListenerConfig" path="/api"/>
</flow>
</mule>
"#;
    let file_path = mule_dir.join("late.xml");
    fs::write(&file_path, xml).unwrap();
    let mut files = ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile {
        path: "src/main/mule/late.xml".to_string(),
        kind: SourceFileKind::MuleXml,
        is_text: true,
        size_bytes: 0,
    });
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    let flow = parsed
        .project
        .documents
        .first()
        .and_then(|d| d.flows.first())
        .expect("flow must be parsed");
    assert!(
        flow.source.start_line >= 5,
        "flow source line should reflect the XML, got {}",
        flow.source.start_line
    );
}

#[test]
fn text_accumulator_keeps_adjacent_runs() {
    use runnerguard_fs::{ProjectFiles, SourceFile, SourceFileKind};
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let mule_dir = dir.path().join("src/main/mule");
    fs::create_dir_all(&mule_dir).unwrap();
    // `<x>before<y/>after</x>` must keep BOTH `before` and `after`;
    // the previous implementation overwrote text on each Text event.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<mule xmlns="http://www.mulesoft.org/schema/mule/core">
<flow name="text-flow">
<set-payload value="foo"/>
<set-variable variableName="x" value="bar"/>
</flow>
</mule>
"#;
    fs::write(mule_dir.join("text.xml"), xml).unwrap();
    let mut files = ProjectFiles::new(dir.path().to_path_buf());
    files.push(SourceFile {
        path: "src/main/mule/text.xml".to_string(),
        kind: SourceFileKind::MuleXml,
        is_text: true,
        size_bytes: 0,
    });
    let parsed = parse_project(&files, &ParseOptions::default()).unwrap();
    assert!(
        parsed.diagnostics.is_empty(),
        "diagnostics should be empty: {:?}",
        parsed.diagnostics
    );
}
