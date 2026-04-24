# Static release binary and dependencies

**Name:** Reproducible and static distribution

**Implementation status:** Not started

**Use cases:** Users and distros get predictable artifacts; maintainers know whether the app expects system libraries, a bundled runtime, or an AppImage-style bundle.

**Short description:** Document and automate release builds: linking strategy (dynamic vs static glibc, bundling, AppImage, or distro packages), and what “static” means for GTK + mpv on Linux.

**Long description:** A fully static binary that includes glibc, GTK, libadwaita, OpenGL, EGL, and libmpv is usually impractical; common approaches are distribution packages, AppImage, or a mostly-static executor plus bundled libs. Rhino should define an official distribution story in this document and keep `Cargo` features or scripts aligned—without mandating a particular store or container format here.

**Specification:**

- CI or documented script produces an artifact for at least one path (e.g. `cargo build --release` with documented dynamic system libs, or another agreed bundle).
- List runtime dependencies: GTK4, libadwaita, libmpv, OpenGL, ffmpeg libs as required by mpv.
- Document the trade-off between shipping against distro packages vs self-contained bundles for maintainers.
- LTO and `strip` for release; optional `panic=abort` only if the team accepts it (documented).
