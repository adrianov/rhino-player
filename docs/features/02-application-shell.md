# Application shell (Adwaita app, lifecycle)

---
status: done
priority: p0
layers: [ui, os-integration]
related: [06, 13, 14, 17, 21, 27]
scope: portable
---

## Use cases
- Familiar desktop app: one icon, standard menus, predictable quit / open behaviour.
- A focused viewing window with dark theme and standard accelerators.

## Description
The application registers its desktop id, builds the main window (header, video surface, bottom bar), wires global actions, and uses a dark chrome style by default. The header carries volume, subtitles, speed, and the main menu; the bottom bar has prev / play / next, time labels, the seek bar, and a trailing **Close Video**.

Activation shows the main window; opening files forwards them to the load layer (see [06-open-and-cli](06-open-and-cli.md)). The session-only **Exit After Current Video** quits the app at natural EOF before any sibling auto-advance.

## Behavior

```gherkin
@status:done @priority:p0 @layer:ui @area:shell
Feature: Application shell

  Scenario: Quit from keyboard
    Given the main window is open
    When the user presses q or Ctrl+Q
    Then a resume snapshot is written
    And the application process exits

  Scenario: Close Video keeps the app running
    Given a video is loaded in the main window
    When the user activates Close Video or Ctrl+W
    Then playback stops
    And the continue / recent grid is shown
    And the application process keeps running

  Scenario: Close Video after warm preload and bottom Play
    Given the continue grid is visible and a title is warm-preloaded from hover
    When the user starts playback from the bottom Play control without activating the card
    And the user activates Close Video or Ctrl+W
    Then playback stops
    And the continue grid is shown
    And the application process keeps running

  Scenario: Close Video returns to the list after a rename
    Given a video is playing
    And its file is renamed on disk meanwhile # an incomplete download that finished
    When the user activates Close Video or Ctrl+W
    Then playback stops
    And the continue / recent grid is shown
    And the application process keeps running

  Scenario: Close on continue list quits when only warm preload is active
    Given the continue grid is visible and a title is warm-preloaded paused in the background
    When the user activates Close Video or Ctrl+W
    Then the application process exits

  Scenario: Exit After Current Video overrides sibling advance
    Given the session-only Exit After Current Video item is enabled
    When the current media reaches natural end-of-playback
    Then the application quits
    And the sibling-folder queue does not load another file first

  Scenario: About dialog is reachable
    Given the main window has focus
    When the user activates About from the main menu or F1
    Then an About dialog appears with app name, version, license, and the themed icon

  Scenario: Application id matches desktop branding
    Given the app is installed for the current user desktop session
    When the desktop shell resolves dash and alt-tab artwork
    Then the icon, desktop entry, and window branding all use the same application id

  Scenario: Dark style is the default
    Given the user has not overridden the style
    When the app starts
    Then the chrome uses the dark style
```

## Notes
- Binding: `GtkApplication` / `adw::Application`, application id `ch.rhino.RhinoPlayer`, `adw::ApplicationWindow` + `adw::ToolbarView` (header + GLArea + bottom bar); `adw::StyleManager` forces dark.
- Global actions: `app.open`, `app.close-video`, `app.exit-after-current`, `app.about`, `app.quit`, `app.move-to-trash`. Accelerators: `app.open` (Ctrl+O), `app.close-video` (Ctrl+W), `app.about` (F1), `app.quit` (Ctrl/Cmd+Q). Plain **q** quits via the capture-phase key controller with an editable-focus guard (see [13](13-input-shortcuts.md)).
- About: `gtk::AboutDialog`.
- Desktop install / branding: `data/install-to-user-dirs.sh`, `data/applications`, `data/metainfo`, `data/icons` (Freedesktop); GNOME dash / alt-tab use `ch.rhino.RhinoPlayer`.
- `app.close-video`: **quit** when the continue grid is visible (browse / warm preload behind the grid) or when nothing local is loaded; **back to browse** when the grid is hidden and a local file or Blu-ray disc tree is loaded (`wire_actions.rs`, `has_loaded_local_media`). That gate reads `media_probe::open_media_path` rather than `shell_media_path`, so a file renamed or deleted mid-playback (a `*.dctmp` download that finished) still counts as loaded — its picture is on screen even though the opened name no longer resolves. Escape (`key_escape_seek.rs`) bypasses the gate and always returns to browse. The continue card afterwards comes from `history::load` adopting the finished sibling, carrying the position saved before the rename.
- User-facing name: `glib::set_application_name` is set to the same string as the initial window title (**Rhino Player**); `glib::set_prgname` remains the application id for `.desktop` / shell matching.
- **macOS:** `gtk_application_set_menubar` uses the **same** `GMenu` instance as the header hamburger (`Open`, `Close`, `Fullscreen`, … `Preferences`, `About`, `Quit`), after actions are registered and the Preferences submenu is rebuilt.
- Main-menu labels use Title Case for desktop-menu readability.
