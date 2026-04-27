# Rhino Player — architecture

This document records the **architectural layers** of Rhino Player and the boundary between what is fixed for the product and what is replaced when porting to another platform. Feature behavior is captured in `docs/features/` and is portable across any port; this file is the contract for the **stack** that supports it.

Three layers:

1. **Product behavior** — `docs/features/<NN>-<slug>.md`. Domain language only. Portable across all ports.
2. **Architecture core** — fixed for Rhino. Same on every port. Documented here.
3. **Platform binding** — varies per port. Documented here for the current Linux binding; a new port adds a column.

## Fixed core (product identity)

These choices are part of what Rhino *is*. They are the same across every port and may be assumed by feature scenarios.

| Concern | Choice | Why fixed |
|---|---|---|
| Language | **Rust** | Native performance; single statically-linked binary path; mature bindings for media, GUI, and OS APIs; strict ownership / concurrency model fits a media-engine consumer. |
| Playback engine | **mpv** (libmpv + render API) | Broadest codec coverage; mature filter graph (VapourSynth, FFmpeg `vf`); deterministic property model; precise A/V sync; well-documented `loadfile` / `seek` / `keep-open` semantics. |
| Persistent store | **SQLite** | Single-file, transactional, portable across operating systems; no daemon; trivially backed up; stable file format across versions. |

A change to any of these would change Rhino itself, not a port. New fundamental dependencies that would join this list require updating this doc and `product-and-use-cases.md`.

## Domain glossary

Feature scenarios use the **left** column. The **right** column is the current core API name; agents and developers map between them when implementing.

| Domain term (used in scenarios) | Current core binding |
|---|---|
| the playback engine | mpv (libmpv) |
| playback position | mpv `time-pos` |
| total length | mpv `duration` |
| paused state | mpv `pause` |
| natural end of playback | mpv `eof-reached`, `EndFile` event |
| audio track | mpv `aid` (within `track-list`) |
| subtitle track | mpv `sid` (within `track-list`) |
| video track | mpv `vid` (within `track-list`) |
| available tracks | mpv `track-list` |
| playback speed | mpv `speed` |
| volume / mute / volume cap | mpv `volume`, `mute`, `volume-max` |
| video display dimensions | mpv `dwidth`, `dheight` |
| smooth-motion filter | mpv `vf vapoursynth` (with bundled or custom `.vpy`) |
| current media path | mpv `path` |
| chapter list / current chapter | mpv `chapter-list`, `chapter` |
| watch-later / resume sidecar | mpv `save-position-on-quit` + `watch-later-dir` |
| the persistent store | SQLite `rhino.sqlite` (history / media / settings tables) |
| user config directory | platform user-config root + `rhino/` subdirectory |
| the platform's trash | OS-native trash facility |

## Platform binding (varies per port)

The columns below describe the **current Linux binding** in detail; macOS and Windows columns are sketches kept here so the boundary is explicit. A new port fills its own column and updates each feature's `## Notes` to match.

| Concern | Linux (current) | macOS (sketch) | Windows (sketch) |
|---|---|---|---|
| UI toolkit | GTK 4 + libadwaita | AppKit / SwiftUI | WinUI 3 |
| Application / lifecycle | `adw::Application` (`ch.rhino.RhinoPlayer`) | `NSApplication` + `NSApplicationDelegate` | `Microsoft.UI.Xaml.Application` |
| Window / video surface | `adw::ApplicationWindow` + `gtk::GLArea` (libmpv render API on EGL/GL) | `NSWindow` + `MTKView` / Metal layer | `Window` + `SwapChainPanel` |
| Action / command system | GIO `app.<id>` actions | responder chain / `NSAction` target-action | XAML commands |
| Config / data root | `$XDG_CONFIG_HOME` or `~/.config/rhino/` | `~/Library/Application Support/Rhino/` | `%APPDATA%\Rhino\` |
| Trash | Freedesktop XDG (`gio::File::trash`; Trash/files + .trashinfo for untrash) | `NSWorkspace.recycle` (Finder Trash) | `IFileOperation::DeleteItem` |
| File associations / open | GIO `Application::open` (HANDLES_OPEN target) | `NSApplicationDelegate openURLs:` | file-type registration + `OnFileActivated` |
| Single instance | GIO single-instance + remote activation | `NSApplication` runs once per bundle | named pipe / single-instance mutex |
| Audio output | PulseAudio / PipeWire (`ao=pulse`) | CoreAudio (`ao=coreaudio`) | WASAPI (`ao=wasapi`) |
| Media keys / shell integration | MPRIS over D-Bus | `MPRemoteCommandCenter`, `MPNowPlayingInfoCenter` | `SystemMediaTransportControls` |
| Idle / sleep inhibit | `gtk::Application::inhibit` (IDLE + SUSPEND) | `IOPMAssertion` | `SetThreadExecutionState` |
| Packaging | dynamic system libs; AppImage / distro packages | `.app` bundle (notarized) | MSIX / portable ZIP |
| Build tooling | Cargo (Rust core; system libs from distro) | Cargo + Xcode toolchain for AppKit shim | Cargo + MSVC toolchain for WinUI shim |
| Theme / dark mode | libadwaita `StyleManager` | `NSAppearance` | XAML theme resources |
| Drag and drop | GTK drop targets | `NSDraggingDestination` | `DragDrop` events |

## Boundary rules

- **Feature scenarios** stay in **domain language**. They may rely on the **fixed-core** invariants above (e.g. "stored in the persistent store") but must not name **platform-binding** specifics.
- **`## Notes` in feature docs** is the only place to write current-binding details. When the binding changes (e.g. a new port), that section is rewritten; scenarios are not.
- A port adds a column to the **Platform binding** table above and rewrites each feature's `## Notes` for the new binding. **No `## Behavior` block is rewritten** during a port. If a port forces a scenario edit, that means the contract leaked binding details — fix the leak in the scenario, do not duplicate the spec per platform.
- Adding a new "fixed core" choice (something that would replace Rust, SQLite, or mpv on every port) requires updating this doc, `product-and-use-cases.md`, and reviewing every feature's scenarios for assumptions that no longer hold.
- Build and packaging are part of the platform binding (with Rust as the fixed-core language). They are tracked in [`features/01-cargo-skeleton.md`](features/01-cargo-skeleton.md) and [`features/20-static-build.md`](features/20-static-build.md); both are `scope: platform-specific`.

## Related

- [`docs/product-and-use-cases.md`](product-and-use-cases.md) — audience, value, and the feature-area map.
- [`docs/README.md`](README.md) — feature index and document-first template summary.
- [`.cursor/rules/document-first.mdc`](../.cursor/rules/document-first.mdc) — the rule that ties scenarios to this architecture.
