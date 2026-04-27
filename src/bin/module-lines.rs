//! Cargo entry point for the Rust module-size linter.

use std::process::{self, Command};

fn main() {
    let status = Command::new("scripts/check-module-lines.sh")
        .status()
        .unwrap_or_else(|err| {
            eprintln!("module-lines: failed to run scripts/check-module-lines.sh: {err}");
            process::exit(2);
        });
    process::exit(status.code().unwrap_or(1));
}
