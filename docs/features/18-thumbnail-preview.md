# Thumbnails: seek bar preview

---
status: done
priority: p1
layers: [ui, input, playback, persistence]
related: [03, 04, 14, 21]
scope: portable
---

## Use cases
- Scrub the timeline visually before seeking, especially on long local files.

## Description
Hovering over the seek bar shows a framed video preview and the corresponding playback time while media is open for playback (playing or paused). Moving along the bar updates the preview without changing the current playback position. The framed preview does not appear on the continue screen when no video is open for playback.

The preview can be disabled in Preferences. It is available for media the application can preview locally and does not interrupt playback.

## Behavior

```gherkin
@status:done @priority:p1 @layer:ui @area:preview
Feature: Thumbnails: seek bar preview

  Scenario: Show a preview for local media
    Given seek bar preview is on
    And locally previewable media is open
    When the user hovers the seek bar at any position
    Then a thumbnail above the bar shows the video at the hovered time
    And the preview shows the formatted hover time

  Scenario: No preview thumbnail while browsing without open playback
    Given seek bar preview is on
    And the continue screen is visible
    And no video is open for playback (playing or paused)
    When the user hovers the seek bar
    Then no preview thumbnail appears
    And the hover time label may still update

  Scenario: Keep unavailable media unchanged
    Given the open media cannot be previewed locally
    When the user hovers the seek bar
    Then no preview appears
    And playback remains unchanged

  Scenario: Follow rapid pointer movement
    Given a preview is showing above the seek bar
    When the user moves the pointer quickly to a new position
    Then the preview shows the latest hovered time
    And an older hover does not replace it

  Scenario: Respect the preview preference
    Given seek bar preview is off
    When the user hovers the seek bar
    Then no thumbnail appears
    And transport remains usable

  Scenario: Leaving the seek bar restores chrome
    Given the preview is showing above the seek bar
    When the pointer leaves the seek bar
    Then the preview is no longer shown
    And the window chrome arrangement matches the layout before the hover

  Scenario: Seek preview on inactive fullscreen does not ghost chrome
    Given seek bar preview is on
    And locally previewable media is open in full screen
    And the viewer window is not the active window
    When the user hovers the seek bar so the preview appears
    Then the video surface shows no bands of stale window chrome
    And the header and bottom chrome do not briefly flash or redraw
    And a thumbnail above the bar shows the video at the hovered time

  Scenario Outline: Show the preview in every window mode
    Given seek bar preview is on
    And locally previewable media is open in "<mode>"
    When the user hovers the seek bar
    Then a thumbnail above the bar shows the video at the hovered time

    Examples:
      | mode          |
      | normal window |
      | full screen   |
```

## Notes
- Settings: SQLite `seek_bar_preview` defaults to **on**; toggled from main menu Preferences (gio stateful action `seek-bar-preview`).
- Hover time is `(x / width) * bar_upper` capped by [seek_bar_label_time]. Pointer release on the seek bar (trough or thumb drag) seeks the main player to that hover time, not the raw GtkRange thumb value; preview off falls back to capped thumb time ([`seek_wiring`](../../src/app/seek_wiring.rs)).
- Linux: preview **`GtkFrame`** on **`outer_ovl`** above the bottom bar. macOS: the same frame is inside an independent non-modal **`GtkPopover`** surface anchored to the seek bar.
- Thumbnail sizing follows the source aspect and the bounds in `state_and_vo_pump.rs`.
- Framed preview opens only when the continue strip is hidden (`recent_visible` false) and an openable target is ready — warm preload behind the browse grid does not count as open playback. Returning to browse dismisses any open framed preview (`dismiss_for_browse`).
- Motion coalescing uses `PREVIEW_DEBOUNCE`; the debounce and frame pump run at default GLib priority.
- The `Progress Bar Preview` row is the only preview-related preference; no separate preferences window.
- Recent grid thumbnails use `screenshot-raw` plus DB WebP cache via `media_probe` / `thumb_texture`; this feature does not feed the grid.
- Load selection and decode limits are owned by `preview_media_load.rs`; the separate `MpvPreviewGl` never seeks the main player. Optical-media mapping is delegated to the playback entity and timeline modules.
- Leaving the bar hides the overlay with `set_visible(false)` but keeps the cached target. Reopen renders a warm frame immediately; `need_load` reloads after GL context loss. Main-media changes clear the target without replacing the preview GL context.
- Debug: `[rhino] preview:` lifecycle and failure lines are always printed; `RHINO_PREVIEW_DEBUG=1` adds frame-pump trace.
- **macOS independence:** `seek_bar_preview/macos_popup.rs` owns a non-modal, arrowless **`GtkPopover`** parented to the seek bar. Popup show/hide never calls shell compositing or invalidates the main content view, so it cannot redraw header/bottom chrome. Linux keeps the in-window overlay path.
