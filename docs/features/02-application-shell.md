# Application shell (Adwaita app, lifecycle)

**Name:** Application shell (Adwaita app, lifecycle)

**Implementation status:** Done (no separate preferences or Shortcuts help window yet; [06-open-and-cli](06-open-and-cli.md) for `open` scope)

**Use cases:** Users get a familiar GNOME-style app: one icon, standard menus, predictable quit/open behavior, and a window that fits the rest of the desktop.

**Short description:** A `GtkApplication` / `adw::Application` with application ID, primary window, global actions (quit, preferences, about, shortcuts), and dark style preference where appropriate.

**Long description:** The app follows GNOME HIG: libadwaita for layout and styling, a single app instance with optional new-window or secondary instances per preferences, and standard accelerators. On startup, register actions and present the main window. On shutdown, clean up mpv and persist session if enabled. About dialog should show app name, version, license, and links without blocking playback logic.

**Current code:** `src/app.rs` is a thin include hub; the shell lives under `src/app/`. It wires `adw::Application` with ID `ch.rhino.RhinoPlayer`, `adw::ApplicationWindow`, `adw::ToolbarView` (header + `GLArea` + bottom bar: prev/play/next, times, seek, **Close Video** as trailing `window-close-symbolic` on the bottom bar, same as `app.close-video`) + `adw::HeaderBar` (volume, app menu), `app.open` (Ctrl+O), `app.close-video` (Ctrl+W: leave playback for the continue list, not quit), session-only `app.exit-after-current` (**Exit After Current Video** checkmark: quit at EOF before sibling auto-advance), `app.about` (F1), `app.quit` (q, Ctrl+Q), and `gtk::AboutDialog` (logo from themed icon `ch.rhino.RhinoPlayer`). `icons::register_hicolor_from_manifest` adds `data/icons` to the icon search path for `cargo run`. `main` sets `glib::set_prgname` to the same id so the process name matches the `*.desktop` basename. For **panel / alt+tab** icons, GNOME-style shells use the installed `ch.rhino.RhinoPlayer.desktop` → `data/install-to-user-dirs.sh` to `~/.local/share/`. Packaged: `data/applications` + `data/metainfo` + `data/icons` follow Freedesktop naming (see `data/README.md`). `adw::init()` and `adw::StyleManager` force dark.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Application shell
  Scenario: Quit from keyboard
    Given the main window is open
    When the user activates quit (e.g. q or Ctrl+Q)
    Then the application exits after completing documented persistence

  Scenario: Close Video leaves the app running
    Given a video is playing or loaded in the main window
    When the user activates Close Video (or Ctrl+W)
    Then playback stops and the continue/recent affordance is shown
    And the application process does not exit solely for that reason

  Scenario: Exit After Current Video takes precedence over sibling advance
    Given the session-only "Exit After Current Video" menu item is enabled
    When the current media reaches natural end-of-playback as defined for EOF handling
    Then the application quits
    And the sibling-folder queue does not load another file first
```

- Application ID in reverse-DNS form (`ch.rhino.RhinoPlayer`) registered once; the same string is the **icon name** in the hicolor theme, `.desktop` `Icon=`, and window / About dialog branding. **GNOME** (and similar) resolve dash / app switcher artwork from a **Freedesktop** `applications/*.desktop` on `XDG_DATA_DIRS`, not from `GtkWindow` alone; the project ships `data/install-to-user-dirs.sh` to install into `~/.local/share` for local runs.
- Global shortcuts: at minimum quit (q, Ctrl+Q) and open preferences (Ctrl+,) as applicable; about from app menu.
- Main-menu item labels use title capitalization for desktop-menu readability, matching macOS-style menu wording.
- **Exit After Current Video:** a checkable main-menu item backed by a stateful application action. It is session-only, defaults off on launch, and quits after the currently playing media reaches EOF or the app’s EOF tail-stall detector fires. It must take priority over sibling folder auto-advance.
- `activate` shows the main window; `open` receives files/URIs and forwards to the window/playlist layer (see [Open and CLI](06-open-and-cli.md)).
- Prefer or default to dark theme for a focused viewing experience unless the user overrides (system or in-app).
