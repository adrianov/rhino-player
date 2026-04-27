# Cargo project and build layout

---
status: done
priority: p0
layers: [build]
related: [20]
---

## Use cases
- Contributors and CI build and test with one toolchain.
- Packagers see a clear dependency list.
- The codebase stays maintainable as modules grow.

## Description
Rhino Player is a Rust application with a standard `Cargo.toml` and `src/` layout at the repository root. Build instructions live in the root `README.md`. Static linking of GTK and GL stacks on Linux is constrained by platform policy; reproducible release builds are owned by [feature 20](20-static-build.md).

## Behavior

```gherkin
@status:done @priority:p0 @layer:build
Feature: Cargo project and build layout

  Scenario: Build from a clean checkout
    Given documented system dependencies for GTK4, libadwaita, and Rust are installed
    When a contributor runs cargo build from the repository root
    Then the crate builds without undisclosed extra steps

  Scenario: Tests run from the repository root
    Given integration or unit tests exist for the crate
    When the contributor runs cargo test from the repository root
    Then tests complete successfully with the same documented prerequisites

  Scenario: Crate name and entry are stable
    Given the repository is checked out at any tagged release
    When tooling reads Cargo.toml
    Then the binary name is rhino-player and the entry point is src/main.rs
```

## Notes
- `Cargo.toml` lists direct dependencies with a short comment per crate explaining its role.
- Additional modules are added as focused `src/<module>.rs` files or under a feature directory such as `src/app/` as the codebase grows.
- The root `README.md` names Rust edition, target OS, and required dev packages for GTK4 and libadwaita at a high level.
- The `release` profile may enable LTO; full static glibc + GTK + mpv is a stretch goal owned by [20-static-build](20-static-build.md).
