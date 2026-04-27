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

## Description
The shell uses `adw::ToolbarView` with content extending to top and bottom edges, so chrome overlays the GLArea instead of shrinking it. A `GtkWindowHandle` wraps the main content for primary-drag window move (more reliable than manual GestureDrag on GL/Wayland). Fullscreen and maximize are wired to GTK / Wayland conventions; `gtk::Application::inhibit` with IDLE+SUSPEND prevents dim and sleep while a real file plays and the recent grid is hidden. Pointer hides on the GLArea after 3 seconds without motion. On opening a new file, the window resizes to match landscape aspect (target width 960 px, max height 900 px); portrait, square, or unknown sizes leave the window alone.

**Post-resize aspect lock** (snapping the window to current display video aspect after the user finishes resizing) and **one-click switching between header `MenuButton` popovers** were attempted but did not validate in manual testing in the current pass. They are documented as not shipped.

## Behavior

```gherkin
@status:wip @priority:p1 @layer:ui @area:window
Feature: Window, fullscreen, and presentation

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

  Scenario: Fullscreen via Enter or double-click
    Given the main window is windowed and not maximized
    When the user activates Enter, KP_Enter, or double-clicks the video surface
    Then the current windowed width and height are saved
    And the window enters fullscreen via the maximize-then-fullscreen path

  Scenario: Exiting fullscreen restores last windowed size
    Given the window is fullscreen with a saved windowed size
    When the user exits fullscreen
    Then the window unmaximizes if needed and set_default_size restores the saved size

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
- Inhibit implementation polls every ~500 ms to sync with pause / load / grid state; uninhibit always runs before quit.
- Autohide default 2–3 s; menus or popovers being open keeps chrome visible.
- ToolbarView extends to top and bottom edges so the GLArea fills the available area and chrome overlays the video.
- Acceptance for **Post-resize aspect lock**: after drag-resize the window’s outer size matches video display aspect within a small tolerance, consistently across sessions, without runaway resize loops or spurious updates from pointer motion. Attempted path: debounced "resize end" on `GdkSurface` / `GtkWindow` notify, `set_default_size`, `present()`, ratio tolerance, `RHINO_ASPECT_DEBUG=1` logging.
- See [GTK4 toplevel / aspect notes](../references-gtk4-toplevel-aspect.md) for upstream context (the prior `compute-size` approach was abandoned due to feedback loops).
- Header menu switching attempts: `Popover:modal=false`, capture-phase GestureClick, idle `MenuButton::set_active`. Manual testing still required a second click; revisit with a different model or a deeper GTK / GNOME review.
