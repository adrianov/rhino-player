# Playback entity

---
status: done
priority: p1
layers: [persistence, storage, playback]
related: [06, 21, 30]
scope: portable
---

## Use cases
- Continue watching, history, and resume refer to one logical title whether it is a single file or many DVD chapter files.
- Opening any chapter of a multi-file DVD title updates the same continue entry as opening another chapter in that title.
- Standalone videos behave as today: one file, one row, local time on the seek bar.

## Description
A **playback entity** is what the persistent store and unified transport treat as one title. Most files are a **single-file** entity (the path itself). DVD chapter files under the same title set form one **multi-part** entity with a shared timeline and one database key (the disc folder that contains `VIDEO_TS/`).

Callers resolve an on-disk path to an entity before reading or writing history, resume, or duration — they do not branch on file type at each site.

## Behavior

```gherkin
@status:done @priority:p1 @layer:persistence @area:playback-entity
Feature: Playback entity

  Scenario: Standalone file maps to itself
    Given a normal video file on disk
    When the app records history or resume for that path
    Then the entity key is that file
    And playback position and duration use local seconds from the engine

  Scenario: DVD chapters share one entity
    Given several chapter files belong to the same DVD title set
    When the user opens any chapter in that set
    Then history and resume use one shared entity key
    And stored time is whole-title seconds
    And per-chapter duplicate rows are removed after writes

  Scenario: Resume load picks the correct chapter file
    Given a multi-part entity has a stored whole-title resume time
    When the app opens a chapter in that entity
    Then the correct chapter file loads if needed
    And playback starts at the local offset within that chapter

  Scenario: Single-file entity uses unified timeline only when multi-part
    Given a standalone video file
    When the user scrubs the transport bar
    Then the bar spans only that file’s duration

  Scenario: Close saves whole-title position for multi-part entities
    Given a multi-part entity is playing mid-title
    When the user closes playback or quits the app
    Then the persistent store records whole-title seconds on the entity key
    And not a separate row for the currently open chapter file
    And reopening any chapter in that entity resumes at the stored whole-title position

  Scenario: Title-set track menus use entity stream list
    Given a multi-part entity is playing any chapter
    When the user opens the Sound or Subtitles control
    Then the listed audio and subtitle variants match the title-set info for that entity
    And the same variants appear on every chapter of the entity
    And choosing a variant applies the matching stream on the current chapter

  Scenario: Window title follows the active entity
    Given playback advances to a different DVD title entity
    When the new title begins playing
    Then the window title shows the new entity name
    And the title is not blank
```

## Notes
- Module: `src/playback_entity.rs` (`PlaybackEntity`, `PlaybackEntityKind::SingleFile` | `DvdTitle`).
- DVD structure helpers remain in `src/dvd_entity.rs`; unified timeline in `src/dvd_vob_timeline.rs`.
- Multi-part DVD entity key: disc root folder containing `VIDEO_TS/` (`dvd_entity::title_playback_entity`). Legacy first-chapter `.vob` SQLite keys are still read via `entity_db_lookup_keys` and purged on write.
- All resume / duration writes: `playback_entity::persist_from_mpv` / `persist_playback` / `clear_entity_resume` (`playback_entity_persist.rs`); never per-chapter `.vob` rows after write.
- `db::history_key`, `history::record`, `media_probe::record_playback_for_current`, and `MpvBundle::load_file_path` go through `playback_entity`.
- Continue grid percent / resume: `playback_entity::card_resume_duration` reads only the **entity** SQLite row; legacy per-chapter `.vob` rows are migrated to whole-title seconds and purged.
- Title-set **Sound** / **Subtitles** menus: `playback_entity_tracks.rs` (`title_set_streams`, `audio_menu_rows`, `sub_menu_rows`, slot → mpv id resolve); wired from `audio_tracks.rs` / `sub_tracks.rs`. Screenshot: [`docs/screenshots/02-dvd-title-tracks.webp`](../screenshots/02-dvd-title-tracks.webp).
- Window / header title: `playback_entity_title.rs` (`window_title_for`); synced on `try_load` and on transport `FileLoaded` / `path` change.
