//! Cargo entry point: runs `scripts/check-module-lines.sh` (soft limit warns; hard limit exits 1).

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
