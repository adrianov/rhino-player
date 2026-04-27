# Preferences and persistent settings

---
status: wip
priority: p1
layers: [db, mpv, ui]
related: [03, 18, 22, 24, 26]
settings: [seek_bar_preview, master_volume, master_mute, video_smooth_60, video_vs_path, video_mvtools_lib]
---

## Use cases
- Set defaults once (subtitle look, default volume, hardware decode, resume).
- Have those defaults apply on next launch without surprises.

## Description
Every user-visible preference has a key and default. Values live in SQLite (`rhino.sqlite` `settings` table) and per-file resume uses mpv `save-position-on-quit` plus `watch-later-dir` under `~/.config/rhino/watch_later`. Most toggles apply live to the running mpv instance; documented exceptions are listed where they exist.

A dedicated preferences dialog is not yet shipped; preferences are presented as menu items today (e.g. Progress Bar Preview, Smooth Video).

## Behavior

```gherkin
@status:wip @priority:p1 @layer:db @area:preferences
Feature: Preferences and persistent settings

  Scenario: Live apply for most toggles
    Given a documented preference is changed while media is playing
    When the change is committed
    Then mpv reflects the new value without an app restart
    And the value persists across the next launch

  Scenario: Every shipped option has a key and default
    Given a user-visible preference is mentioned in feature docs
    When tooling inspects the settings store
    Then a storage key with a default value exists for that preference

  Scenario: Per-file resume uses the dedicated watch-later directory
    Given save-position-on-quit is enabled
    When the user quits the app while a local file is playing mid-stream
    Then a watch_later sidecar appears under ~/.config/rhino/watch_later
    And reopening the same path resumes from that position
```

## Notes
- Keys persisted today in SQLite include `seek_bar_preview` (toggles [18-thumbnail-preview](18-thumbnail-preview.md)), `master_volume` / `master_mute` (see [22-audio-volume-mute](22-audio-volume-mute.md)), `video_smooth_60` / `video_vs_path` / `video_mvtools_lib` (see [26-sixty-fps-motion](26-sixty-fps-motion.md)), plus subtitle preferences in [24-subtitles](24-subtitles.md).
- `is-maximized` and other window-restore keys are still TBD.
