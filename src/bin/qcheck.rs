//! Cargo entry point for Rhino's full quality check.
//!
//! Runs Clippy on the whole crate. Module length is governed by
//! `.cursor/rules/refactor-touched-longest.mdc` and `clippy::too_many_lines`.

use std::process::{self, Command};

fn main() {
    let status = Command::new("cargo")
        .args(["clippy", "--all-targets", "--all-features"])
        .status()
        .unwrap_or_else(|err| {
            eprintln!("qcheck: failed to run cargo clippy: {err}");
            process::exit(2);
        });
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}
