# Window: size, fullscreen, UI auto-hide, inhibit idle

---
status: wip
priority: p1
layers: [ui, platform]
related: [02, 06, 13, 21]
mpv_props: [pause, path, dwidth, dheight, fullscreen]
---

## Use cases
- Immersive fullscreen with chrome that hides when not needed.
- The screen does not lock or sleep during a movie.
- On opening a landscape file, the window resizes to match its aspect.
- When another application had foreground focus, opening a media title raises the viewer.
- Fullscreen viewing still allows reading **local wall-clock time** without leaving fullscreen.

## Description
The shell uses `adw::ToolbarView` with content extending to top and bottom edges, so chrome overlays the GLArea instead of shrinking it. A `GtkWindowHandle` wraps the main content for primary-drag window move (more reliable than manual GestureDrag on GL/Wayland). Fullscreen and maximize are wired to GTK / Wayland conventions; `gtk::Application::inhibit` with IDLE+SUSPEND prevents dim and sleep while a real file plays and the recent grid is hidden. Pointer hides on the GLArea after 3 seconds without motion. On opening a new file, the window is presented so it can take focus when another app was foreground; the window resizes to match landscape aspect (target width 960 px, max height 900 px); portrait, square, or unknown sizes leave the window alone. While fullscreen, the header can show **local time** beside the playback menus so the system clock stays glanceable when chrome is visible.

**Post-resize aspect lock** (snapping the window to current display video aspect after the user finishes resizing) and **one-click switching between header `MenuButton` popovers** were attempted but did not validate in manual testing in the current pass. They are documented as not shipped.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:ui @area:window
Feature: Window, fullscreen, and presentation

  Scenario: Open path brings the window forward
    Given another application had foreground focus
    When the viewer loads a local media title from an open gesture
    Then the viewer window is presented so it advances to the top

  Scenario: Idle inhibit while playing behind chrome
    Given a real media path is loaded, pause is false, and the recent grid is hidden
    When those conditions hold
    Then GTK inhibits IDLE and SUSPEND
    And inhibit is removed when any condition fails or the app quits

  Scenario: Fit-on-open for landscape video
    Given a newly loaded file reports dwidth and dheight
    And the window is neither fullscreen nor maximized
    When width is greater than height
    Then the window resizes toward the documented landscape aspect (target width 960 px, max height 900 px, with clamping)
    And portrait, square, or unknown sizes leave window dimensions unchanged

  Scenario: Fullscreen via shortcuts, double-click, or main menu
    Given the main window is windowed and not maximized
    When the user activates fullscreen from keyboard shortcuts, double-clicks the video surface, or chooses fullscreen from the main menu
    Then the current windowed width and height are saved
    And the window enters fullscreen via the maximize-then-fullscreen path

  Scenario: Main menu exits fullscreen
    Given the window is fullscreen
    When the user chooses fullscreen from the main menu
    Then the window leaves fullscreen

  Scenario: Exiting fullscreen restores last windowed size
    Given the window is fullscreen with a saved windowed size
    When the user exits fullscreen
    Then the window unmaximizes if needed and set_default_size restores the saved size

  Scenario: Fullscreen shows local wall-clock time in the header
    Given the window is fullscreen
    When the header chrome is visible toward the playback menus
    Then local wall-clock time appears to the left of those menus
    And twelve-hour versus twenty-four-hour and showing seconds match the desktop clock preferences when the platform exposes them
    And the readout updates while fullscreen remains active
    And leaving fullscreen hides the readout

  Scenario: Chrome autohide while playing
    Given a file is playing and the recent grid is hidden
    When pointer motion stops over the main window for 3 seconds
    Then the header and bottom toolbars hide
    And any pointer motion reveals them immediately

  Scenario: Chrome stays visible on the recent grid
    Given the recent-videos overlay is showing
    When the user is idle
    Then the header and bottom toolbars remain visible

  Scenario: Pointer hides on the video after 3s
    Given the pointer is on the GLArea
    When 3 seconds pass without movement on that area
    Then the cursor is set to none on the video surface

  Scenario: Post-resize aspect lock — not shipped
    Given the user finishes a manual resize with video visible
    When acceptance is evaluated on Wayland + GTK4
    Then aspect lock remains documented as not shipped until acceptance is met

  Scenario: One-click header menu switch — not shipped
    Given a header MenuButton popover is open
    When the user clicks another header MenuButton
    Then a single click switching to the next popover is not yet reliable in manual testing
    And users may need a second click in this pass
```

## Notes
- Main menu **Fullscreen** invokes `app.toggle-fullscreen` (same maximize-then-fullscreen path as other toggles); **F11** is bound as a shortcut. Enter / **f** also toggle fullscreen from the window key controller (see input wiring).
- Fullscreen-only header clock: `GtkLabel` packed on `HeaderBar` before speed / sound / subtitle / main menu; reads **`org.gnome.desktop.interface`** (`clock-format` **12h** / **24h**, `clock-show-seconds`) when that schema exists so the string matches the desktop shell clock (no forced `%X` / seconds / AM–PM). Fallback **`%H:%M`** when settings are unavailable; visible updates use `glib::timeout_add_seconds_local(1, …)` while fullscreen because no toolkit signal fires per wall-clock second.
- Inhibit implementation polls every ~500 ms to sync with pause / load / grid state; uninhibit always runs before quit.
- Autohide default 2–3 s; menus or popovers being open keeps chrome visible.
- ToolbarView extends to top and bottom edges so the GLArea fills the available area and chrome overlays the video.
- Acceptance for **Post-resize aspect lock**: after drag-resize the window’s outer size matches video display aspect within a small tolerance, consistently across sessions, without runaway resize loops or spurious updates from pointer motion. Attempted path: debounced "resize end" on `GdkSurface` / `GtkWindow` notify, `set_default_size`, `present()`, ratio tolerance, `RHINO_ASPECT_DEBUG=1` logging.
- See [GTK4 toplevel / aspect notes](../references-gtk4-toplevel-aspect.md) for upstream context (the prior `compute-size` approach was abandoned due to feedback loops).
- Header menu switching attempts: `Popover:modal=false`, capture-phase GestureClick, idle `MenuButton::set_active`. Manual testing still required a second click; revisit with a different model or a deeper GTK / GNOME review.
