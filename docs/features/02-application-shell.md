# Application shell (Adwaita app, lifecycle)

---
status: done
priority: p0
layers: [ui, platform]
related: [06, 13, 14, 17, 21, 27]
actions: [app.open, app.close-video, app.exit-after-current, app.about, app.quit, app.move-to-trash]
---

## Use cases
- Familiar GNOME-style app: one icon, standard menus, predictable quit / open behaviour.
- A focused viewing window with dark theme and standard accelerators.

## Description
A `GtkApplication` / `adw::Application` registers application id `ch.rhino.RhinoPlayer`, builds an `adw::ApplicationWindow` with `adw::ToolbarView` (header + GLArea + bottom bar), wires global actions, and forces dark style. The header carries volume, subtitles, speed, and the main menu; the bottom bar has prev / play / next, time labels, the seek bar, and a trailing **Close Video**.

`activate` shows the main window; `open` receives files and forwards them to the load layer (see [06-open-and-cli](06-open-and-cli.md)). The session-only **Exit After Current Video** quits the app at natural EOF before any sibling auto-advance.

## Behavior

```gherkin
@status:done @priority:p0 @layer:ui @area:shell
Feature: Application shell

  Scenario: Quit from keyboard
    Given the main window is open
    When the user presses q or Ctrl+Q
    Then app.quit runs persistence
    And the application process exits

  Scenario: Close Video keeps the app running
    Given a video is loaded in the main window
    When the user activates Close Video or Ctrl+W
    Then playback stops
    And the continue / recent grid is shown
    And the application process keeps running

  Scenario: Exit After Current Video overrides sibling advance
    Given the session-only Exit After Current Video item is enabled
    When the current media reaches natural end-of-playback
    Then the application quits
    And the sibling-folder queue does not load another file first

  Scenario: About dialog is reachable
    Given the main window has focus
    When the user activates About from the main menu or F1
    Then a gtk::AboutDialog appears with app name, version, license, and the themed icon

  Scenario: Application id matches desktop branding
    Given the app is installed via data/install-to-user-dirs.sh
    When GNOME resolves dash and alt-tab artwork
    Then the icon, .desktop Icon=, and window branding all use ch.rhino.RhinoPlayer

  Scenario: Dark style is the default
    Given the user has not overridden the style
    When the app starts
    Then adw::StyleManager forces dark
```

## Notes
- Global accelerators: `app.open` (Ctrl+O), `app.close-video` (Ctrl+W), `app.about` (F1), `app.quit` (q, Ctrl+Q).
- Main-menu labels use Title Case for desktop-menu readability.
- `glib::set_prgname` matches the app id so the process name aligns with the `.desktop` basename.
- Packaged metadata: `data/applications`, `data/metainfo`, `data/icons` (Freedesktop layout).
