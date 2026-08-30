# Homebrew / macOS packaged app launch

---
status: done
priority: p0
layers: [os-integration, build, ui]
related: [02, 26]
scope: platform-specific
---

## Use cases
- As a macOS viewer, I want the Homebrew-installed app to open from Finder or the Dock the same way it does from a terminal.
- As a packager, I want the formula’s `.app` to ship share assets and optional Smooth libraries so a fresh install is usable.

## Description
Targets **macOS** when Rhino is installed via Homebrew (or a release `.app` that still links Homebrew GTK / libadwaita / mpv). Launching from Finder or the Dock does not inherit a login-shell environment, so the viewer must discover the Homebrew GSettings schema tree before the UI toolkit initializes. The same install also places Freedesktop share data (and, when available, vendored Smooth plugins) where the running binary can find them.

## Behavior

```gherkin
@status:done @priority:p0 @layer:os-integration @area:packaging
Feature: Homebrew / macOS packaged app launch

  Scenario: Finder launch shows the main window
    Given Rhino is installed via Homebrew with its macOS app bundle
    And the Homebrew UI toolkit schemas are present on the machine
    When the user opens the app from Finder or the Dock
    Then the main window appears
    And the process does not abort during toolkit startup

  Scenario: CLI symlink starts the same binary
    Given Rhino is installed via Homebrew with its macOS app bundle
    When the user runs the `rhino-player` command from a terminal
    Then the main window appears

  Scenario: Packaged Smooth scripts are available inside the app
    Given Rhino is installed via Homebrew with its macOS app bundle
    When Smooth Video uses the bundled motion script with no custom script path
    Then the packaged motion script file is found next to the app resources
```

## Notes
- Binding: `macos_prime_homebrew_runtime_env` (`src/paths_homebrew_macos.rs`) prepends `/opt/homebrew/share` or `/usr/local/share` to `XDG_DATA_DIRS` when `glib-2.0/schemas` exists — call from `main` before GTK init (and before/after the VapourSynth `DYLD_LIBRARY_PATH` re-exec). Without this, libadwaita aborts with “No GSettings schemas are installed on the system” under Launch Services.
- Formula source of truth in-repo: `packaging/homebrew/rhino-player.rb` (copy to tap `adrianov/homebrew-rhino-player`). Builds `.app`, copies `share/rhino-player/vs`, optionally vendors MVTools via `SKIP_MISSING=1 scripts/macos-vendor-smooth-libs.sh`, installs the vendor script under `share/rhino-player/scripts` for config seeding.
- CLI test: `rhino-player --version` (avoids needing a display for GApplication `--help`).
