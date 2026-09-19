# Legacy full screen (frame covers the screen, no Space)

---
status: done
priority: p1
layers: [ui, os-integration]
related: [17, 14]
scope: platform-specific
---

## Use cases
- Fullscreen without macOS relocating the window to a new Space (virtual desktop).
- Menu bar and Dock yield to the video but slide back when the pointer touches the screen edges.
- Leaving full screen restores the window's exact prior position and size.
- The native fullscreen presentation stays reachable through the window's own zoom (green) control.

## Description
On macOS the usual fullscreen entry points — keyboard shortcut, double-click, main menu — engage the platform's native fullscreen presentation, which moves the window into a newly created Space. This feature adds a **legacy full screen** mode, modeled on IINA's option of the same name: the window simply grows until its content covers the target display while remaining in the ordinary window layer — no Space is created and no transition controller runs.

Legacy full screen is a **preference** (View → Preferences → Legacy Full Screen) and is **on by default**. The native presentation stays available through the window's zoom control; toggling while it is active exits it exactly as before.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:fullscreen
Feature: Legacy full screen (frame covers the screen, no Space)

  Scenario: Fullscreen toggles enter legacy full screen by default
    Given the window is windowed
    And the legacy-full-screen preference is on
    When the user triggers fullscreen from the keyboard, double-clicks the video surface or header, or picks fullscreen from the main menu
    Then the window content covers the target display
    And the window stays on the ordinary window layer
    And no new Space is created

  Scenario: Menu bar and Dock auto-hide during legacy full screen
    Given the window content covers the display in legacy full screen
    When the pointer is away from the top and bottom screen edges
    Then the menu bar and the Dock are hidden
    And they slide back when the pointer reaches the matching screen edge
    And they obscure the window while revealed

  Scenario: Leaving legacy full screen restores the prior frame
    Given the window is in legacy full screen
    When the user toggles fullscreen again
    Then the window returns to its exact prior position and size

  Scenario: Entering legacy full screen from a zoomed window first restores the windowed size
    Given the window is maximized (zoomed)
    When the user enters legacy full screen
    Then the window first returns to its windowed size
    And its content then covers the display

  Scenario: Fullscreen behaviors carry over into legacy full screen
    Given the window content covers the display in legacy full screen
    When the pointer idles during playback, or the user summons the header
    Then the header and bottom toolbars auto-hide and reveal exactly as in native fullscreen
    And the fullscreen wall-clock readout appears and keeps ticking
    And the prominent window controls hide together with the chrome
    And entering while paused resumes playback, and leaving restores the pre-entry pause state

  Scenario: Open-time and resize-time sizing skip legacy full screen
    Given the window content covers the display in legacy full screen
    When a media title finishes loading, or the user completes a manual resize
    Then the window is neither resized to the video aspect nor nudged to a fit-on-open target

  Scenario: Header menus stay regular popovers in legacy full screen
    Given the window content covers the display in legacy full screen
    When the user opens a header menu panel
    Then the panel opens as an ordinary popover anchored to its button
    And it dismisses normally without leaving legacy full screen

  Scenario: Native full screen remains reachable and exits normally
    Given the legacy-full-screen preference is on
    When the user activates the window zoom (green) control
    Then the window enters the native fullscreen presentation as before
    And toggling fullscreen during native full screen exits the native presentation

  Scenario: Preference defaults to on and survives restart
    Given a fresh install
    Then the legacy-full-screen preference is on
    When the user turns it off and restarts the application
    Then it stays off
    And fullscreen toggles use the native presentation again

  Scenario: Preference changes apply from the next toggle
    Given the window is in legacy full screen
    When the user switches the legacy-full-screen preference off
    Then the running legacy session continues undisturbed
    And the next fullscreen toggle exits legacy full screen
```

## Notes
- Reference: IINA `MainWindowController.legacyAnimateToFullscreen` / `legacyAnimateToWindowed` (`~/iina/iina/MainWindowController.swift`) — AppKit style-mask adjustments, `NSApp.presentationOptions` auto-hide for menu bar + Dock, `setFrame(screen.frame)`, floating level, shadow off, and an exact-frame restore on exit.
- **State owner:** `src/macos_legacy_fs.rs` — thread-local `active` flag plus the saved prior frame / level / shadow / presentation options, and a notify hook `Fn(&ApplicationWindow, bool /*entering*/)`. GTK `fullscreened` stays native-only: gdk-macos derives it from the AppKit style mask (`_gdk_macos_surface_update_fullscreen_state`) and drives fullscreen through `toggleFullScreen:` (`macos_window_fs.rs`), so the legacy mode cannot reuse it.
- **Frame:** content-rect route — `frameRectForContentRect(screen.frame)` so the video reaches every display edge even though the NSWindow keeps a native titlebar strip; the frame is saved before entering and restored verbatim on exit. `setFrame:display:` propagates through `GdkMacosWindow` resize notifications into GDK.
- **Square corners:** the window server rounds titled-window corners (macOS 11+), which showed as rounded video corners in the cover. Enter drops the `Titled` style-mask bit for the cover and exit restores the saved mask before the frame restore; the gdk backend overrides `canBecomeKeyWindow`, so key status survives. Overscanning the cover past the screen edges is not viable — gdk-macos clamps the GTK toplevel back to the monitor workarea on the next toplevel layout pass — and the old `setTopLeftRoundedCornerRadius:`-style private selectors no longer exist on current macOS.
- **Toggle choke point:** `macos_apply_toggle` in `src/app/base/chrome_macos_toggle.rs`, in order: clear a stale GTK-fullscreen latch → native exit (wins whenever the AppKit style mask is set — including when native FS was entered over a live legacy session via the zoom control; it also calls `macos_legacy_fs::note_native_exit_started`, which arms the cover convergence) → legacy exit when active → legacy enter when the preference is on → native paths otherwise. Entering is fully owned by `macos_legacy_fs::enter`: while the native exit latch (`macos_fs_exit`) is armed it is skipped; from a maximized window it first unmaximizes and covers only once the zoom-out has completed and the frame is stable — gdk-macos raises the *maximized* notify and drops the GTK flag when the unmaximize is requested, not when AppKit's async zoom lands, so a bounded AppKit-frame stability check (frame stable across ticks, not maximized, not workarea-sized) bound to the transition generation replaces the notify wait; each tick re-validates (no newer request, no native exit latch, no native fullscreen) and budget exhaustion gives up with a log line instead of restarting, while a double press refreshes the wait and re-issues `unmaximize` only on the first request. This keeps the saved prior frame settled windowed geometry.
- **Fullscreen gates:** `window_fullscreened` (in `chrome_fullscreen_and_fit.rs`, scope of `app::base`) = GTK/native fullscreen **or** legacy active. Consumed by the fit-on-open skip, post-resize aspect-snap skip, fullscreen clock ticker guard, and the browse-strip double-click guard. Traffic lights are the exception: native fullscreen leaves their visibility to AppKit, while legacy treats them as ordinary window controls that follow the chrome reveal only (`sync_header_window_controls_macos`). Header-menu theater overlay reparenting (`macos_header_menu*`) deliberately remains **native-only** — in legacy mode popovers are ordinary gdk popups and keep working.
- **GTK-visible enter/leave steps** reuse the fullscreened-notify machinery: the notify hook (registered in `w_in_fullscreen`, `shell_fullscreen.rs`) runs the same bars / pause-stash / wall-clock / chrome-apply / cursor-hide sequence as `fs_notify_enter` (minus the maximize chaining); the leave sequence restores bars, clock, pointer, pause, and chrome — the `fs_restore` geometry machinery is bypassed because the AppKit frame restore replaces it.
- **Preference:** `settings` KV row `legacy_full_screen` (`db::load_legacy_full_screen` / `save_legacy_full_screen`, default **true**), exposed as the stateful `app.legacy-fullscreen` action with a "Legacy Full Screen" row appended to the Preferences submenu rebuild. macOS-only; the row and action are not built on Linux.
- **Bars-off before the jump:** the enter notify hook runs first, and `fs_notify_enter_chrome` (`shell_fs_notify.rs`) collapses the bars with `gtk-enable-animations` off for the turn (`collapse_chrome_without_animation`) so no reveal tween leaves bar pixels behind; the AppKit cover apply is deferred one `fullscreen_timing::BAR_HIDE_COMMIT` slice so GTK commits a bars-less surface buffer before the big resize — a laid-out header in the rescaled buffer smears into a mid-screen ghost. The enter also arms the pointer-motion squelch (`LAYOUT_SQUELCH`, same window `hide_bars_now` uses): gdk resamples stationary-pointer motion continuously, and an unsquelched sample after the jump passes the position dedupe (`fs_notify_reset` cleared `last_xy`) and re-reveals the bars over the video. The suppression is reference-counted (`ANIM_SUPPRESS` in `shell_fs_notify_legacy.rs`): overlapping collapses save the display-wide setting once at depth 0→1 and restore it only when the last pending suppression finishes — a nested enter must not save the already-disabled value and restore it last, which would leave GTK animations disabled display-wide.
- **Native interleave over the session:** all prior AppKit words (frame, level, shadow, presentation, style mask) are captured in `save_prior_window_state` **before** the session goes active, so an exit landing inside the `BAR_HIDE_COMMIT` deferral restores the live window's own values (a no-op). The deferred cover apply (`apply_cover_after_bar_hide`, `macos_legacy_fs_cover.rs`) revalidates the foreign-owner state (`busy_for_reassert`) and defers on `TRANSITION_SETTLE` ticks until a native transition releases the window (`BUSY_DEFERS` budget, always-on abandon log). The `fullscreened_notify` leave branch routes through the combined native-or-legacy state: a native leave over a live legacy session skips the ordinary windowed leave (clock hide, bars restore, sub-position) — the cover continues fullscreened (`fs_notify_on_event`, `shell_fullscreen.rs`). Note the interleave is currently unreachable from the UI: the View menu item and ctrl+cmd+F are the app's own fullscreen toggle (they toggle legacy), and the traffic-light button hides with the chrome at the legacy notify — native FS is only enterable from the windowed state.
- Window level during legacy full screen is `NSFloatingWindowLevel` (below the menu bar and the blackout covers, above ordinary windows); restored on exit. Dock/menu-bar auto-hide restores the exact prior `NSApplication.presentationOptions` word.
- **Maximized-latch / async-zoom trap:** the screen-covering frame latches AppKit `isZoomed`, so `windowDidMove` synthesizes a GDK *maximized* notification during legacy enter; `max_mode_route` (`shell_max_mode.rs`) therefore fires its maximized→native-fullscreen chain only when the legacy preference is **off**. Exit does **not** heal via `unmaximize`: that zoom-out lands asynchronously on top of the exact-frame restore, and a native cycle run over the session rewrites AppKit's stored normal frame, so any zoom-out lands on dirt. Instead, both the legacy exit and the post-native cover (`note_native_exit_started`) verify their target frame after a settle interval and re-`setFrame` until it sticks (`reassert_step`, bounded). Every pending re-assert is bound to a session generation bumped on each enter/exit — a callback from a finished or superseded session is inert. The windowed (exit) flavor stands down when user geometry owns the frame: a pure move (target size at another origin) or a shrink/mixed resize is dimension-inferred, and a user live resize during the retry window cancels the chain outright — the exit arms an `NSWindowWillStartLiveResizeNotification` observer (`arm_user_resize_watch`, `macos_legacy_fs_reassert.rs`) whose pair is posted only for interactive resizes, never for programmatic `setFrame` writes (the restore, the re-apply, the aspect snap, GTK's stale content-size adoption). The flag persists across sessions and only the windowed flavor consumes it (`!reapply_presentation` gate in `reassert_step`), so the cover flavor always converges and re-inserts the auto-hide presentation bits a native cycle drops, because with a visible menu bar `constrainFrameRect` caps the cover at screen-minus-menu-bar. Verified live 2026-09-18: F enter/exit restores the 960×540 frame byte-exact; green-button native FS over legacy → F peels the native layer, the cover re-converges, F restores the exact prior frame; pref off → native path; View → Preferences → "Legacy Full Screen" toggles the KV row (checkmark reflects state).
- **App-activation level swap:** a legacy cover at `NSFloatingWindowLevel` buries other apps' windows under it when the user cmd+tabs away (the cover stays top-most on its screen). `wire_level_swap_on_app_active` (`macos_legacy_fs_levels.rs`, included from `macos_legacy_fs.rs`; wired once per process from `enter`) observes `NSApplicationDid{Resign,Become}ActiveNotification` and swaps the cover level: normal while inactive, floating again on return. Handlers are gated on `ACTIVE`, refetch the window from the widget, and log `legacy-fs: app {inactive,active} — level {normal,floating}`.
- **Traffic-light displacement (stale NSToolbar):** a native fullscreen round-trip leaves a stale, empty `NSToolbar` on the window (macOS 26's exit machinery re-assigns one after `prep_native_fullscreen_exit` already dropped it; gdk-macos's `updateToolbarAppearence` assigns one to indent the lights and never clears it). The lights then sit ~20px right of the standard slot until some later relayout. Fix: `clear_stale_titlebar_toolbar` (`macos_window_fs.rs`) drops the toolbar — called on every chrome reveal (`set_traffic_lights_visible`, visible branch) and once `2×TRANSITION_SETTLE` after exit completion (`schedule_titlebar_toolbar_clear`, from `macos_leave_fs_restore_now`). Probed with the always-on `legacy-fs: chrome at={enter,exit,reassert} …` line (`log_window_chrome`, `macos_legacy_fs.rs`): pre-fix `toolbar=true close=38,…`; post-fix `toolbar=false close=18,…`.
- **Activation ghost band (maximized-latch replay):** during a cmd+tab return to the app, the maximized-residue latch replays the windowed geometry transiently (the shell log shows `css=[maximized] … win=960x540 … ns=1920x1080` during activation); GSK commits a frame at the old windowed size and the CALayer stretches it to the cover, baking a stretched chrome strip mid-screen — no later damage repaints that region. The re-assert machinery re-pins the frame but never repaints. Fix: `schedule_activate_repaint` (`macos_legacy_fs_levels.rs`) queues a full-window `queue_draw` `2×TRANSITION_SETTLE` after each become-active while a legacy session is live (`ACTIVE` gate), logging `legacy-fs: activate repaint queued` — full damage rewrites every pixel and heals the band. Fires only on activation; no timer otherwise.
