# System diagnostics (CLI)

---
status: done
priority: p2
layers: [os-integration, playback, build]
related: [06, 26]
---

## Use cases
- Verify Smooth Video setup after install without opening the player window.
- Produce a shareable text report when Smooth cannot start (playback engine filter, script host, motion plugin).

## Description
A command-line diagnostics mode prints a short Smooth Video status report, then exits without opening the main window. The report covers the linked playback engine, whether that engine exposes the temporal-smoothing filter, whether the script host for the filter loads, which motion-interpolation plugin path would be used, and whether the bundled smooth script is present. The process exits successfully only when those Smooth requirements are all met.

## Behavior

```gherkin
@status:done @priority:p2 @layer:os-integration @area:diagnostics
Feature: System diagnostics (CLI)

  Scenario: Diagnostics prints a Smooth status report
    Given the user launches the app with the diagnostics switch
    When the process starts
    Then a status report is written to standard output
    And the main window does not open
    And the process exits after the report

  Scenario: Diagnostics succeeds when Smooth requirements are met
    Given the linked playback engine exposes the temporal-smoothing filter
    And the script host for that filter loads
    And the motion-interpolation plugin resolves
    When the user runs diagnostics
    Then the report lists each of those checks as passing
    And the process exits successfully

  Scenario: Diagnostics fails when a Smooth requirement is missing
    Given at least one of the temporal-smoothing filter, script host, or motion plugin is missing
    When the user runs diagnostics
    Then the report flags the failing check
    And the process exits with a failure status
```

## Notes
- Switch: **`--diagnostics`** / **`-D`** (early exit in `src/main.rs` via **`diagnostics::cli_diagnostics_exit`**, after the macOS VSScript DYLD re-exec; **`--version`** / **`-V`** via **`cli_version_exit`** before that re-exec).
- Checks (stdout lines, not a GUI): crate version + binary path; headless libmpv **`vf add vapoursynth=…`** plus drained **warn** log lines (filter present only with log evidence or a successful add — bare **`MPV_ERROR_COMMAND`** is not enough); VSScript pin (**`video_pref::diagnose_vsscript`**); MVTools via **`paths::mvtools_*`**; bundled **`rhino_60_mvtools.vpy`**.
- Owner: **`src/diagnostics.rs`** (+ **`diagnostics/vf_probe.rs`**). On macOS, `main` still performs the VSScript **`DYLD_LIBRARY_PATH`** re-exec before the report runs.
- Exit: **0** when Smooth requirements pass; **1** otherwise. No display required.
