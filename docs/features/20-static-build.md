# Static release binary and dependencies

---
status: planned
priority: p2
layers: [build]
related: [01]
---

## Use cases
- Users and distros get predictable artifacts.
- Maintainers know whether the app expects system libraries or a bundle.

## Description
A fully static binary including glibc, GTK, libadwaita, OpenGL, EGL, and libmpv is impractical on Linux; the official release strategy is documented here. Options include distro packages, AppImage, or a mostly-static executor with bundled libraries. Cargo features and scripts stay aligned with the chosen path.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:build @area:release
Feature: Release and dependency story

  Scenario: Documented build produces a release artifact
    Given the maintainer follows the documented release path on a supported environment
    When they run the prescribed release build
    Then an artifact is produced with documented dynamic dependencies or a documented bundle layout

  Scenario: Dependency transparency
    Given a packager reads this document before packaging Rhino
    When they compare runtime needs against a target distribution
    Then GTK4, libadwaita, libmpv, OpenGL, and ffmpeg-related prerequisites are explicit

  Scenario: Hardening options are opt-in and documented
    Given LTO, strip, or panic settings are mentioned
    When a packager enables them
    Then the trade-offs remain documented rather than silent defaults
```

## Notes
- CI or a documented script produces an artifact for at least one supported path.
- LTO and `strip` are recommended for release; `panic=abort` only if the team accepts the trade-offs.
