//! Cargo entry point for Rhino's full quality check.

use std::process::{self, Command};

fn run(label: &str, cmd: &mut Command) {
    let status = cmd.status().unwrap_or_else(|err| {
        eprintln!("qcheck: failed to run {label}: {err}");
        process::exit(2);
    });
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn main() {
    run(
        "cargo clippy",
        Command::new("cargo").args(["clippy", "--all-targets", "--all-features"]),
    );
    run(
        "cargo module-lines",
        Command::new("cargo").args(["module-lines"]),
    );
}
