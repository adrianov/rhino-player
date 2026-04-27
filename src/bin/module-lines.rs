//! Cargo entry point: runs `scripts/check-module-lines.sh` (advisory soft limit; always exits 0 if the script runs).

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
