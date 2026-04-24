# Cargo project and build layout

**Name:** Cargo project and build layout

**Implementation status:** In progress

**Use cases:** Contributors and CI can build and test with one toolchain; packagers get a clear dependency list; the project stays maintainable as modules grow.

**Short description:** Rust workspace at the repository root, standard `src/main.rs` entry, dependencies documented for GTK/Adwaita/mpv, and a path toward reproducible and optionally static release builds.

**Long description:** Rhino Player is a Rust application. The repository must have a clear `Cargo.toml` and `src/` layout so features can be added as modules. Build instructions belong in the root `README.md`. Static linking of GTK and GL stacks on Linux is constrained by platform and distro policies; the project documents goals in a dedicated feature and implements what is practical (e.g. LTO, `target-feature`, or bundled dependencies where possible).

**Specification:**

- `cargo build` and `cargo test` (once tests exist) run from the repo root without extra steps beyond documented system dependencies.
- `Cargo.toml` lists crate name `rhino-player` (or agreed binary name) and a minimal set of direct dependencies, with a short comment on why each is needed.
- `src/main.rs` is the process entry; additional modules are added as `src/<module>.rs` or `src/<dir>/mod.rs` as the codebase grows.
- The root `README.md` names Rust edition, target OS, and required dev packages for GTK4 and libadwaita at a high level.
- A `release` profile may enable LTO; full static glibc+GTK+mpv is a stretch goal (see [Static release binary](20-static-build.md)).
