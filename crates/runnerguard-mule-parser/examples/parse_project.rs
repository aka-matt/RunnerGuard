//! Parses a Mule project at the given path and writes one JSON file per
//! flow / sub-flow into the output directory, plus a project index.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-mule-parser --example parse_project -- \
//!   ./fixtures/mule-project-basic --output ./tmp/parsed
//! ```

use runnerguard_fs::write_atomic;
use runnerguard_fs::{DiscoverOptions, discover};
use runnerguard_model::SourceFile;
use runnerguard_mule_parser::{ParseOptions, ParsedOutcome, parse_project};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let project_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let output_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./tmp/parsed"));

    let files = discover(&project_dir, DiscoverOptions::default())?;
    if files.is_empty() {
        eprintln!("No files discovered under {}", project_dir.display());
        return Ok(());
    }

    let outcome = parse_project(&files, &ParseOptions::default())?;
    write_outputs(&outcome, &output_dir)?;

    let docs = &outcome.project.documents;
    let flows: usize = docs.iter().map(|d| d.flows.len()).sum();
    let subs: usize = docs.iter().map(|d| d.sub_flows.len()).sum();
    println!(
        "Parsed {} document(s), {} flow(s), {} sub-flow(s). Output: {}",
        docs.len(),
        flows,
        subs,
        output_dir.display()
    );
    Ok(())
}

fn write_outputs(outcome: &ParsedOutcome, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let flow_dir = dir.join("flows");
    std::fs::create_dir_all(&flow_dir)?;
    for doc in &outcome.project.documents {
        for flow in &doc.flows {
            let safe = sanitize(&flow.name);
            let path = flow_dir.join(format!("{safe}--{}.json", short_hash(&flow.id)));
            let bytes = serde_json::to_vec_pretty(flow).unwrap_or_default();
            write_atomic(&path, &bytes)?;
        }
    }
    write_atomic(
        &dir.join("project.json"),
        &serde_json::to_vec_pretty(&outcome.project).unwrap_or_default(),
    )?;
    write_atomic(
        &dir.join("diagnostics.json"),
        &serde_json::to_vec_pretty(&outcome.diagnostics).unwrap_or_default(),
    )?;
    Ok(())
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let bytes = h.finalize();
    let mut out = String::with_capacity(8);
    for b in &bytes[..4] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// Quiet the dead-code warning when `SourceFile` is imported but unused.
#[allow(dead_code)]
fn _ensure_link(_: SourceFile) {}
