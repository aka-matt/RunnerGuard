//! Loads the bundled default config, prints it with secrets redacted.
//!
//! Run with:
//!
//! ```text
//! cargo run -p runnerguard-config --example resolve_config --
//! ```

use runnerguard_config::{defaults, render_redacted_yaml, validate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = defaults();
    let issues = validate(&cfg);
    if !issues.is_empty() {
        for issue in issues {
            eprintln!("{issue}");
        }
    } else {
        println!("default config validates cleanly");
    }
    let yaml = render_redacted_yaml(&cfg)?;
    println!("{yaml}");
    Ok(())
}
