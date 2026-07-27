//! Demonstrates the file-system discovery step. Run with:
//!
//! ```text
//! cargo run -p runnerguard-fs --example discover_project -- ./fixtures/mule-project-basic
//! ```
//!
//! For now this prints whatever project the caller points it at; the
//! fixtures live under fixtures/ and are added in a later phase.

use runnerguard_fs::{DiscoverOptions, discover};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let files = discover(&path, DiscoverOptions::default())?;
    println!("Root: {}", files.root.display());
    println!("Files discovered: {}", files.len());
    for f in files.iter() {
        println!("  [{:?}] {} ({} bytes)", f.kind, f.path, f.size_bytes);
    }
    Ok(())
}
