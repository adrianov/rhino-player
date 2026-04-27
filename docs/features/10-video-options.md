# Video options: aspect, crop, zoom, filters

---
status: planned
priority: p2
layers: [ui, mpv]
related: [04, 26, 28]
mpv_props: [video-aspect-override, video-zoom, video-pan-x, video-pan-y, contrast, brightness, gamma, saturation, hue, sub-delay, audio-delay, speed]
---

## Use cases
- Fix letterboxing, wrong aspect, or color without re-encoding.
- Align subtitles and audio when sync drifts.
- Slow or speed up playback for a given file.

## Description
A popover or Adwaita menu adjusts per-video display and sync properties: `video-aspect-override`, crop, zoom, contrast, brightness, gamma, saturation, hue, `sub-delay`, `audio-delay`, and `speed`, with a single reset action. Flip / rotate controls may be hidden when hardware decode lacks a copy path.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:ui @area:video-options
Feature: Per-video options menu

  Scenario: Controls reflect mpv state on open
    Given media is loaded with adjustable video properties
    When the options popover opens
    Then every widget shows the current mpv value, not a stale placeholder

  Scenario: Changes apply immediately
    Given the options menu is open
    When the user adjusts crop, zoom, color, delay, or speed controls
    Then mpv receives the matching property update without restart

  Scenario: Reset restores documented defaults
    Given the user has altered multiple video-related properties
    When they activate Reset all
    Then every adjusted property returns to its documented default
    And no app restart is required
```

## Notes
- The dedicated speed control with three steps (1.0× / 1.5× / 2.0×) is owned by [28-playback-speed](28-playback-speed.md); this menu may surface a free-form slider later.
- Keep flip / rotate behind a feature check when hardware decode lacks a `-copy` path.
