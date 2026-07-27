//! `xtask` — project-local developer + release automation.
//!
//! Mirrors the CI matrix the design doc specifies. Each subcommand is a
//! thin wrapper around a `cargo` invocation so the same checks run
//! locally and in CI.
//!
//! ```text
//! cargo run -p xtask -- ci     # everything CI runs
//! cargo run -p xtask -- lint
//! cargo run -p xtask -- test
//! cargo run -p xtask -- fmt
//! cargo run -p xtask -- doc
//! cargo run -p xtask -- release
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "xtask",
    about = "Local automation for the RunnerGuard workspace",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run the full CI matrix (fmt + clippy + test + doc).
    Ci {
        /// Skip the doc check (used to break up local loops).
        #[arg(long)]
        skip_doc: bool,
    },
    /// `cargo fmt --all -- --check`.
    Fmt,
    /// `cargo clippy --workspace --all-targets -- -D warnings`.
    Lint,
    /// `cargo test --workspace`.
    Test,
    /// `cargo doc --workspace --no-deps`.
    Doc,
    /// Build the release binary.
    Release,
    /// Convenience: tidy `Cargo.lock` if it changed.
    Lock,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Ci { skip_doc } => ci(skip_doc),
        Cmd::Fmt => cargo(&["fmt", "--all", "--", "--check"]),
        Cmd::Lint => cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
        Cmd::Test => cargo(&["test", "--workspace"]),
        Cmd::Doc => cargo(&["doc", "--workspace", "--no-deps"]),
        Cmd::Release => cargo(&["build", "--release", "-p", "runnerguard-cli"]),
        Cmd::Lock => cargo(&["check", "--workspace", "--locked"]),
    }
}

fn ci(skip_doc: bool) -> Result<()> {
    cargo(&["fmt", "--all", "--", "--check"])?;
    cargo(&["check", "--workspace", "--all-targets"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace"])?;
    if !skip_doc {
        cargo(&["doc", "--workspace", "--no-deps"])?;
    }
    Ok(())
}

fn cargo(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo").args(args).status()?;
    if !status.success() {
        anyhow::bail!("cargo {} failed with {}", args.join(" "), status);
    }
    Ok(())
}
