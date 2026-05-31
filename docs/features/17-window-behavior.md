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
- On a multi-display setup, dim every display except the one showing the viewer.

## Description
The shell uses `adw::ToolbarView` with content extending to top and bottom edges, so chrome overlays the GLArea instead of shrinking it. A `GtkWindowHandle` wraps the main content for primary-drag window move (more reliable than manual GestureDrag on GL/Wayland). Fullscreen and maximize are wired to GTK / Wayland conventions; `gtk::Application::inhibit` with IDLE+SUSPEND prevents dim and sleep while a real file plays and the recent grid is hidden. Pointer hides on the GLArea after 3 seconds without motion. On opening a new file, the window is presented so it can take focus when another app was foreground; the window resizes to match landscape aspect (target width 960 px, max height 900 px); portrait, square, or unknown sizes leave the window alone. While fullscreen, the header can show **local time** beside the playback menus so the system clock stays glanceable when chrome is visible. When at least two displays are connected, the header may offer a toggle that blacks out every display except the one hosting the viewer; the choice persists across sessions.

After the user finishes a manual resize, the window may snap by a few pixels when its outer aspect is already close to the playing video’s display aspect. **One-click switching between header menu popovers** did not validate in manual testing in the current pass and remains not shipped.

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

  Scenario: Double-click top toolbar exits fullscreen
    Given the window is fullscreen
    When the user double-clicks primary on the top toolbar
    Then the window leaves fullscreen

  Scenario: Double-click top toolbar enters fullscreen during playback
    Given a media title is loaded and pause may be either state
    And the recent grid is hidden and the window is not fullscreen
    When the user double-clicks primary on the top toolbar
    Then the window enters fullscreen via the maximize-then-fullscreen path

  Scenario: Entering fullscreen while paused resumes playback
    Given a media title is loaded and playback is paused
    And the recent grid is hidden and the window is not fullscreen
    When the user enters fullscreen
    Then playback resumes

  Scenario: Exiting fullscreen restores pause only when entry had unpaused a paused title and playback is still running
    Given a media title was paused before entering fullscreen
    And playback is running when the user exits fullscreen
    When the user exits fullscreen
    Then playback is paused again

  Scenario: Exiting fullscreen does not change pause when already paused or was playing before entry
    Given the window leaves fullscreen
    When playback is already paused at exit, or was not paused before that fullscreen session
    Then the exit does not unpause playback
    And the exit does not pause playback solely because of leaving fullscreen

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
    And the prominent window-management controls grouped with that top toolbar hide
    And any pointer motion reveals them immediately

  Scenario: Chrome stays visible on the recent grid
    Given the recent-videos overlay is showing
    When the user is idle
    Then the header and bottom toolbars remain visible
    And the prominent window-management controls grouped with that top toolbar remain visible

  Scenario: Pointer hides on the video after 3s
    Given the pointer is on the GLArea
    When 3 seconds pass without movement on that area
    Then the cursor is set to none on the video surface

  Scenario: Post-resize aspect snap when already close
    Given a media title is playing with a known display aspect
    And the window is neither fullscreen nor maximized
    And the continue grid is hidden
    When the user finishes a manual resize
    And the window’s outer width-to-height ratio is already close to the video display aspect but not an exact match
    Then the window size is adjusted by the smallest total pixel change on width and height that matches the video aspect
    And no adjustment runs when the ratios already match or are far apart

  Scenario: Post-resize aspect snap skipped in browse or maximized modes
    Given the continue grid is visible, or the window is fullscreen or maximized
    When the user finishes a manual resize
    Then the window size is not adjusted for video aspect

  Scenario: One-click header menu switch — not shipped
    Given a header MenuButton popover is open
    When the user clicks another header MenuButton
    Then a single click switching to the next popover is not yet reliable in manual testing
    And users may need a second click in this pass

  Scenario: Blackout toggle hidden on a single display
    Given the platform reports one connected display
    When the header chrome is visible
    Then the blackout-other-displays control is not shown

  Scenario: Blackout toggle visible with multiple displays
    Given the platform reports at least two connected displays
    When the header chrome is visible
    Then the blackout-other-displays control appears in the header toolbar
    And its styling matches the other header menu controls

  Scenario: Enable blackout while playing
    Given at least two displays are connected
    And a media title is loaded and playing
    And the viewer window is the active window on one display
    When the user turns on blackout-other-displays
    Then every other connected display shows a solid black surface above normal content
    And the display showing the viewer remains unchanged

  Scenario: Blackout does not apply while paused
    Given blackout-other-displays is on
    And playback is paused
    When the viewer window is the active window
    Then the black surfaces on other displays are not shown

  Scenario: Disable blackout while active
    Given blackout-other-displays is on and playback is playing
    When the user turns off blackout-other-displays
    Then the black surfaces on other displays are removed

  Scenario: Blackout clears when the viewer loses focus
    Given blackout-other-displays is on and playback is playing
    When the viewer window is no longer the active window
    Then the black surfaces on other displays are removed

  Scenario: Blackout follows the viewer across displays
    Given blackout-other-displays is on and playback is playing
    When the viewer window moves to another connected display
    Then the black surfaces are recalculated so the new host display stays visible
    And every other connected display is blacked out

  Scenario: Blackout preference persists
    Given the user enabled blackout-other-displays
    When the application restarts
    Then blackout-other-displays remains enabled
    And it applies again the next time playback is active with multiple displays connected

  Scenario: Theater overlay panels do not ghost header chrome on the video
    Given the window is in native fullscreen presentation
    And a media title is playing with the continue grid hidden
    When the user opens a header menu panel or hovers the seek bar preview
    Then the video surface shows no horizontal bands of stale header chrome
    And the overlay panel renders with opaque chrome
```

## Notes
- **Fullscreen pause bookmark:** `fs_pause_stash: RefCell<Option<bool>>` — on first `fullscreened_notify` enter per session, record whether playback was paused; unpause only when `Some(true)`. On deferred leave (same timing as windowed size restore), pause back only when stash was `Some(true)` and mpv is still unpaused; if the user paused again during fullscreen, leave paused. `Some(false)` or no stash → exit does not pause. Spurious re-enter notifies skip re-stashing while stash is set.
- Header **double-click fullscreen:** primary **double-click** on `HeaderBar` calls the same fullscreen toggle as the video gesture; fullscreen **exit** ignores the browse-overlay guard so the toolbar is always a target to leave fullscreen; fullscreen **entry** skips while the overlay is visible (same as GL double-click). **`gtk-titlebar-double-click`** is set to **`none`** in **`theme::apply`** so GDK does not also run **toggle-maximize** on that gesture (capture order could demaximize after our toggle).
- Fullscreen-only header clock: `GtkLabel` packed on `HeaderBar` before speed / sound / subtitle / main menu; reads **`org.gnome.desktop.interface`** (`clock-format` **12h** / **24h**, `clock-show-seconds`) when that schema exists so the string matches the desktop shell clock (no forced `%X` / seconds / AM–PM). Fallback **`%H:%M`** when settings are unavailable; visible updates use `glib::timeout_add_seconds_local(1, …)` while fullscreen because no toolkit signal fires per wall-clock second.
- Inhibit implementation polls every ~500 ms to sync with pause / load / grid state; uninhibit always runs before quit.
- Autohide default 2–3 s; menus or popovers being open keeps chrome visible.
- Fit-on-open: `chrome_window_video_fit.rs` + `chrome_shell_layout.rs` — landscape fit + **`schedule_shell_layout_after_gtk_resize`**. macOS bottom chrome: **`macos_bottom_bar.rs`** — [`gtk::Box`] with `.rpb-header` plus **widget-level** CSS provider (display CSS alone is insufficient on gdk-macos); **`nudge_gdk_compositing_width`** after shell sync mimics manual edge-drag repaint; **`schedule_macos_shell_refresh_after_vf`** after VapourSynth `vf add`. **`RHINO_SHELL_DEBUG=1`**: watch **`bottom_h`**, **`shell=…x…`**, **`gdk width nudge`** lines.
- ToolbarView extends to top and bottom edges so the GLArea fills the available area and chrome overlays the video. Client-side decorations: baseline for `shows-start-title-buttons` / `shows-end-title-buttons` is sampled after map (idle) while chrome first shows—not after a hide—or `apply_chrome` would capture `(false,false)` and restore would leave traffic lights off; invalid pairs are ignored in favor of a short `(true,true)` fallback until GTK reports a decorated side.
- **Fit-on-open:** `chrome_window_video_fit.rs` — landscape **960×540-class** fit only when the window is still the default size or **smaller** than that target (grow-only). Otherwise keeps size; optional aspect nudge via `snap_size_after_user_resize`.
- **Post-resize aspect snap:** `aspect_resize_snap.rs` — coded `width`×`height` in `WinAspectCell`; snap when width **or** height is within **60%** of aspect-correct; compute one-axis deltas **+W**, **−W**, **+H**, **−H** to match aspect (formulas `W′=round(H×vw/vh)`, `H′=round(W×vh/vw)`); apply the **smallest** delta if **|Δ|/side ≤ 50%**. Debounce 200 ms → `apply_window_outer_size`.
- See [GTK4 toplevel / aspect notes](../references-gtk4-toplevel-aspect.md) for upstream context (the prior `compute-size` approach was abandoned due to feedback loops).
- Header menu switching attempts: `Popover:modal=false`, capture-phase GestureClick, idle `MenuButton::set_active`. Manual testing still required a second click on Linux; revisit with a deeper GTK / GNOME review.
- **Multi-monitor activation:** Portable behavior is `gtk_window_present` only (compositor picks the output on Wayland). **macOS:** `window_present::present_on_activation_display` (startup only) sets `NSWindow` frame on the `NSScreen` under `NSEvent::mouseLocation` (else `mainScreen`) **before** `present`, briefly hides an already-visible window to avoid one frame on the wrong display, then re-applies frame synchronously after `present`; skipped when fullscreen or maximized. Later `NSApplicationDidBecomeActiveNotification` (Dock or clicking the window) calls `present` only — no re-centering.
- **Startup shell:** Continue strip uses `recent_view::fill_continue_strip` (SQLite durations + cached WebP thumbs only) **before** `present`. libmpv init is queued from `GLArea` realize on the next idle; transport / seek-preview / input wiring runs on the next idle after that (`deferred_after_present.rs`; seek preview only when the preference is on). Warm preload of the first continue file runs on the next idle after transport observers are installed (`run_continue_warm_preload`); card hover uses immediate `warm_hover_hooks` with a single-flight gate (one `loadfile`, one queued path after full load). `recent_visible` is seeded from the continue-strip intent (`want_recent`), not `Widget::is_visible()` (false until the window is mapped). While the grid is visible, Smooth / VapourSynth resync and the resume seek are deferred until reveal/unpause. Resume is applied on `FileLoaded` only (never before the demuxer is ready).
- **macOS header menus:** Windowed — standard [`GtkMenuButton`] + [`GtkPopover`]; gdk-macos opaque CSS on map/show (`macos_header_menu::wire_popover`); `autohide=false` + capture dismiss on **`outer_ovl`**; 300 ms speed pick guard; defer **`invalidate_window_layers`** while a popover popup exists. **Native theater fullscreen** — popovers detached from buttons; same menu **child** reparented into **`outer_ovl`** overlay panel (no gdk popup surface); class **`rp-header-menu-fs`** for enabled chrome; **`on_overlay_surface_opened`** on panel show — full binding in [`references-gtk4-macos-header-menus.md`](../references-gtk4-macos-header-menus.md) (**Theater overlay compositing**).
- **macOS fullscreen:** Native AppKit style mask is authoritative; GDK **`is_fullscreen`** can stick after exit. **`clear_stale_gtk_fullscreen`** when GDK fullscreen but AppKit is not. **Toggle:** **`chrome_macos_toggle`** — does not use **`fs_transition_try_begin`** (380 ms busy blocked rapid Enter after exit); defers while **`inFullscreenTransition`**. **Exit:** **`prepare_fullscreen_exit`**, **`TRANSITION_SETTLE`**, **`toggleFullScreen:`**. **Enter:** **`enter fullscreen`** log + **`native_toggle_fullscreen_enter`**. **`RHINO_MACOS_FS_DEBUG=1`**. Notify busy clear **120 ms** on macOS (Linux **380 ms**).
- **Multi-monitor blackout (macOS):** `screen_blackout` — borderless `NSWindow` per non-viewer `NSScreen` at `NSMainMenuWindowLevel + 1`, black background, `orderFront` relative to the viewer's native window. Active when the preference is on, the viewer is the active window, and playback is running (same gate as idle inhibit: loaded path, not paused, continue grid hidden); cleared on pause, deactivation, browse overlay, or preference off. Header `MenuButton` (`rp-blackout-mbtn`, bundled `video-display-symbolic`, readout **On** / **Off**); hidden when `NSScreen::screens().len() < 2`. SQLite key `black_out_screens`. `NSApplicationDidChangeScreenParametersNotification` and `NSWindowDidChangeScreenNotification` refresh overlay geometry; transport pause / tick resync overlays. Linux: control hidden (no binding yet).
- **macOS theater overlay compositing (fixed):** showing **`outer_ovl`** children (header menu panel, seek preview) in native fullscreen used to leave stale gdk-macos header tiles on the video; **`on_overlay_surface_opened`** + close tail **`on_menu_surface_closed`** refresh the shell (arm 300 ms → queue_draw → full invalidate ~332 ms later). See [`references-gtk4-macos-header-menus.md`](../references-gtk4-macos-header-menus.md).
- **Known macOS glitch (partial):** after programmatic fit-on-open or VapourSynth attach, gdk-macos can still leave the **bottom toolbar** layer transparent until a surface resize triggers compositing refresh; USER-priority bottom CSS + surface-notify refresh + post-`vf` passes mitigate DVD/VOB open. Repeated zoom/maximize/fullscreen churn can briefly show video through opaque chrome — **`invalidate_window_layers`** helps Space/cross-fade staleness only.
