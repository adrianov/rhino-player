# Cargo project and build layout

**Name:** Cargo project and build layout

**Implementation status:** Done (see [20-static-build](20-static-build.md) for static release)

**Use cases:** Contributors and CI can build and test with one toolchain; packagers get a clear dependency list; the project stays maintainable as modules grow.

**Short description:** Rust workspace at the repository root, standard `src/main.rs` entry, dependencies documented for GTK/Adwaita/mpv, and a path toward reproducible and optionally static release builds.

**Long description:** Rhino Player is a Rust application. The repository must have a clear `Cargo.toml` and `src/` layout so features can be added as modules. Build instructions belong in the root `README.md`. Static linking of GTK and GL stacks on Linux is constrained by platform and distro policies; the project documents goals in a dedicated feature and implements what is practical (e.g. LTO, `target-feature`, or bundled dependencies where possible).

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Cargo project and build layout
  Scenario: Build from a clean checkout
    Given documented system dependencies for GTK4 / libadwaita / Rust are installed
    When a contributor runs cargo build from the repository root
    Then the crate builds without undisclosed extra steps

  Scenario: Tests (when present)
    Given integration or unit tests exist for the crate
    When they run cargo test from the repository root
    Then tests complete successfully with the same documented prerequisites
```

- `cargo build` and `cargo test` (once tests exist) run from the repo root without extra steps beyond documented system dependencies.
- `Cargo.toml` lists crate name `rhino-player` (or agreed binary name) and a minimal set of direct dependencies, with a short comment on why each is needed.
- `src/main.rs` is the process entry; additional modules are added as focused `src/<module>.rs` files or under a feature directory such as `src/app/` as the codebase grows.
- The root `README.md` names Rust edition, target OS, and required dev packages for GTK4 and libadwaita at a high level.
- A `release` profile may enable LTO; full static glibc+GTK+mpv is a stretch goal (see [Static release binary](20-static-build.md)).
