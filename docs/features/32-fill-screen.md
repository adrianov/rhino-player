# Fill Screen

---
status: done
priority: p1
layers: [ui, playback, persistence]
related: [17, 10]
---

## Use cases

- Eliminate black bars when the viewport aspect ratio does not match the picture.
- Eliminate black strips that are baked into the video frames themselves.
- Quickly switch between cropped-fill and fitted view in windowed or fullscreen playback.

## Description

When a video is open and the **current viewport** aspect ratio differs from the picture’s aspect ratio **after baked-in black strips are removed** (or from the full frame when strips are not detected), a **Fill Screen** button appears in the header bar. Activating it zooms and crops so the picture covers the viewport: letterboxing/pillarboxing is removed by fill-zoom, and baked-in strips are cropped out so they leave the viewport. The button acts as a toggle; tapping again restores the fitted view. The button is hidden when the viewport already matches that content aspect. In that case the picture fills the viewport on its own: baked-in strips are cropped away so they do not leave a margin around the image.

## Behavior

```gherkin
@status:done @priority:p1 @layer:playback
Feature: Fill Screen

  Background:
    Given a video is playing

  Scenario: Fill button visible on viewport aspect mismatch
    Given the viewport aspect ratio does not match the picture aspect ratio
    When the player shows the video
    Then the Fill Screen button is visible in the header bar
    And the button is not in the active state

  Scenario: Fill button visible in windowed mode
    Given the player is not in fullscreen
    And the viewport aspect ratio does not match the picture aspect ratio
    When the player shows the video
    Then the Fill Screen button is visible in the header bar

  Scenario: Fill button visible when strips change the content aspect
    Given the full-frame aspect ratio matches the viewport
    And the frames contain detectable black strips
    And removing those strips yields a different aspect ratio than the viewport
    When strip detection finishes
    Then the Fill Screen button is visible in the header bar

  Scenario: Fill button hidden when viewport matches content aspect
    Given the viewport aspect ratio matches the picture aspect ratio after any strip removal
    When the player shows the video
    Then the Fill Screen button is not visible

  Scenario: Picture fills a viewport that matches the content aspect
    Given the frames contain detectable black strips
    And the viewport aspect ratio matches the picture aspect ratio after those strips are removed
    When the player shows the video
    Then the picture fills the viewport
    And the black strips stay outside the viewport
    And the Fill Screen button is not visible

  Scenario: Activate fill
    Given the Fill Screen button is visible and inactive
    When the user clicks the Fill Screen button
    Then the video zooms to fill the entire viewport
    And the button changes to the active state

  Scenario: Activate fill crops baked-in strips
    Given the Fill Screen button is visible because strip removal changes the content aspect
    And the button is inactive
    When the user clicks the Fill Screen button
    Then the picture enlarges so the black strips leave the viewport
    And the button changes to the active state

  Scenario: Deactivate fill
    Given the Fill Screen button is in the active state
    When the user clicks the Fill Screen button
    Then the video returns to the fitted (letterboxed/pillarboxed) view
    And baked-in strips are visible again if present in the frames
    And the button returns to the inactive state

  Scenario: Fill follows fullscreen exit when windowed viewport still mismatches
    Given the Fill Screen button is in the active state in fullscreen
    When the user exits fullscreen
    And the windowed viewport aspect still differs from the content aspect
    Then the filled view remains
    And the Fill Screen button stays visible and active

  Scenario: Fill zoom ends when the viewport matches the content aspect
    Given the Fill Screen button is in the active state
    And the frames contain detectable black strips
    When the viewport aspect ratio matches the content aspect ratio
    Then the picture fills the viewport without extra zoom
    And the black strips stay outside the viewport
    And the button is no longer visible

  Scenario Outline: Fill carries to an adjacent sibling
    Given the Fill Screen button is in the active state
    When the player transitions to the <sibling> video by <transition>
    Then the filled view is applied to that sibling when the viewport aspect differs from its content aspect
    And the Fill Screen button is visible and active

    Examples:
      |sibling |transition                |
      |next    |playback reaching the end |
      |next    |the user stepping forward |
      |previous|the user stepping backward|

  Scenario: Fitted choice carries to an adjacent sibling
    Given the Fill Screen button is not in the active state
    And the next sibling's content aspect differs from the viewport
    When the next sibling video starts after playback ends
    Then the fitted view is shown until fill is activated

  Scenario: Sibling's remembered fitted choice wins over carried fill
    Given the Fill Screen button is in the active state
    And the next sibling has a remembered fitted choice
    And that sibling's content aspect differs from the viewport
    When the next sibling starts after playback ends
    Then the fitted view is shown
    And the Fill Screen button is inactive

  Scenario: Fill resets when an unrelated video opens
    Given the Fill Screen button is in the active state
    When a new video that is neither the next nor the previous sibling starts playing
    Then fill zoom is off
    And the fitted view is shown when the viewport aspect differs from the picture
    And the button visibility reflects the new video's content aspect and the viewport

  Scenario: Fill choice remembered for its video
    Given the Fill Screen button was activated for a video
    When that same video is opened again with a viewport aspect mismatch
    Then the video automatically fills the viewport
    And the button appears in the active state

  Scenario: Explicit fitted choice also remembered
    Given the user had switched fill off for a video while it could fill
    When that same video is opened again with a viewport aspect mismatch
    Then the video shows the fitted view with the button inactive

  Scenario: Videos without a stored choice open fitted
    Given a video has never had its Fill Screen button toggled
    When it plays with a viewport aspect mismatch
    Then the video shows the fitted view until the user activates fill

  Scenario: Strip detection result remembered for its video
    Given strip detection finished for a video
    When that same unchanged video is opened again
    Then the player reuses the stored strip result without running detection again
    And Fill Screen button visibility matches that stored content aspect

  Scenario: Changed file re-runs strip detection
    Given a stored strip result exists for a video path
    And the file on disk was replaced or modified since that result was stored
    When that path is opened again
    Then strip detection runs again
    And the persistent store updates with the new result

  Scenario: Strip detection does not interrupt playback
    Given smooth motion is on
    When strip detection runs for a video
    Then playback keeps advancing without a pause or black frames
    And the video processing chain is not rebuilt for detection

  Scenario: Probe without a readable picture never caches a clean result
    Given strip detection started for a video
    When every sampled frame is dark or unreadable
    Then no clean result is written to the persistent store
    And detection retries a bounded number of times
    And a later video reconfiguration re-arms detection

  Scenario: Implausible strip reading never caches a clean result
    Given strip detection started while the video shows mostly dark opening frames
    When the strip reading covers only a small part of the frame
    Then no clean result is written to the persistent store
    And detection retries a bounded number of times
    And Fill Screen button visibility follows the real strips once a reading succeeds

  Scenario: Scene dark on one side is not mistaken for strips
    Given a video without baked-in strips
    And strip detection runs during a scene that is dark on one side
    When strip detection finishes
    Then the persistent store records no strips for the video
    And the Fill Screen button visibility follows the full-frame aspect

  Scenario: Paused start defers strip detection
    Given a video opened while playback is paused
    When strip detection would start
    Then the persistent store keeps no strip result for the video while playback stays paused
    And resuming playback runs strip detection
    And Fill Screen button visibility afterwards reflects the detected strips

  Scenario: Legacy cached clean result re-probes
    Given the persistent store holds a clean strip result that predates probed-clean marking
    When that video is opened again
    Then strip detection runs again
    And the persistent store is rewritten with the probed result
```

## Notes

- Implemented in `src/video_fill.rs` (+ `fill_sync`, + `fill_sync/picture.rs` crop-when-matched, + `fill_sync/probe.rs` kick/restore wiring); baked-in strips owned by `src/black_bars` (packed `frame` bounds + screenshot-sampling `probe.rs` scheduling, + `probe_finish.rs` gather completion/verdict, + `probe_defer.rs` rescheduling chains).
- Aspect fill uses mpv `panscan`: `0.0` = fitted (default), `1.0` = fills the video surface, crops symmetrically.
- Baked-in strips: the probe samples displayed frames with mpv `screenshot-raw video` (`media_probe::with_raw_frame`, software path — no render-thread round-trip) and trims dark edges with `black_bars::frame::content_bounds`; `GATHER_SAMPLES` frames `SAMPLE_GAP` apart are united (`Gather::absorb`, restarted when the size pair changes). The **vf chain is never touched** — the earlier temporary lavfi `cropdetect` vf restarted the Smooth60 VapourSynth graph on insert and removal (multi-second freezes plus black warm-up frames that cached sub-region crops). The result drives mpv `video-crop` (`WxH+X+Y`) while Fill is on, and also when the viewport already matches the picture aspect (`panscan` stays `0` so the cropped picture fills that window). Cleared when Fill is off and the viewport does not match, or when media changes. Probe timing / shared crop guards live in `black_bars` (`DETECT_DELAY`, `pump_bar_probe` on reconfig, `READY_RETRY` / `READY_RETRY_MAX` fallback). If a probe finished before Bob attached, `VideoReconfig` / `FileLoaded` re-arms detection once (`BarProbe::needs_deint_reprobe`). Fill sync leaves panscan alone while decode size (`dwidth`/`dheight`) is briefly unavailable (Bob/reconfig).
- Crop rect space: the cache (`media.bar_crop`) and `BarState::Crop` are canonical in **decoded-frame space** (`video-params`), while `video-crop` applies to the image the filter chain feeds the VO (`video-out-params`) — Smooth60's size cap downscales that image (e.g. 1552×872 for a 1920×1080 decode), so mpv rejects an unscaled decode-space rect. Screenshots honor an active `video-crop`, so `probe::sample_frame` maps each reading into the shown VO region (`crop_space::shown_vo_region` from `video-out-params/crop-*`) before uniting. Mapping lives in `black_bars/crop_space.rs` (`chain_sizes`, `scale_rect_between`, `crop_rect_in_vo_space`); `probe_finish::gather_verdict` classifies against `ChainSizes.vo` (`classify_reading`: strips are assumed centered, so each axis keeps only its thinner margin — a scene dark on one side is not a strip; edges under `MIN_BAR_FRAC` read as clean) and maps a crop to decode space, so a clean file probed under a downscaling chain still reads as `Clean`. `apply_video_crop` scales the cached rect into the current VO image.
- Aspect ratio tolerance constant in `src/video_fill.rs` (`AR_TOLERANCE`).
- Viewport aspect from the video surface widget size (`GLArea`); content aspect from strip `CropRect` when known, else mpv `dwidth` / `dheight`.
- Button icon: `view-fill-symbolic` (`data/icons/hicolor/scalable/actions/view-fill-symbolic.svg`).
- Button visibility is refreshed by `video_fill::request_fill_resync()` from `VideoReconfig` and `FileLoaded`, on fullscreen changes, and on video-surface resize after `bind_fill_viewport`; strip probe starts from FileLoaded / path reset unless a fresh cached result exists.
- Visibility logging is change-only (`FillSync::last_show`): one `[rhino] fill:` line per verdict flip, not per resize/reconfig burst; media changes additionally log one `fill: reset pref … (stored=… carry=…)` line from `reset_preferred`.
- Fill choice persists per video in `media.fill_screen` (`db::media_fill_screen` /
  `db::media_save_fill_screen`); written only on an explicit button toggle, restored on media open when the viewport can fill.
- Fill intent carries across sibling transitions: `video_fill::request_fill_carry(&target)` is set before the load in `advance_to_next_sibling` (EOF) and `load_sibling_pick` (buttons / shortcuts / MPRIS / Now Playing) and consumed by `FillSync::reset_preferred` — the carry applies only when the media that actually opened is the bound target (`paths_same_file`), so a failed or abandoned sibling load can never leak it onto an unrelated open. A sibling without its own `media.fill_screen` row inherits the previous video's intent (on or off); an explicit stored row still wins, and any unrelated open keeps the fitted default.
- Strip probe result persists per video in `media.bar_crop` + `media.bar_crop_mtime_ns` + `media.bar_crop_size` (`db::media_bar_crop` / `db::media_save_bar_crop`): `c` = probed clean (no strips), `d:c` = clean with Bob in vf, `WxH+X+Y` / `d:WxH+X+Y` = crop (`d:` = Bob seen). Reused only when nanosecond mtime and size still match; a cached pre-Bob result still re-arms when Bob attaches later.
- **No picture is never a clean verdict**: a gather whose samples are all dark or unreadable (`[rhino] bars: frame sample unavailable`) stays `Pending` — `black_bars::probe_defer` retries on a bounded chain (`READ_RETRIES` × `READ_RETRY`) and never writes the store; `VideoReconfig` (`dispatch_sync_ui_media_change`) and unpause (`on_pause_event`) re-arm it via `video_fill::request_fill_resync()`. A reading is only probed-clean when it spans the full shown frame; readings outside the measured frame or sub-region lock-ons from near-black content (`crop_meaningful`) are implausible — retried bounded, never cached (`classify_reading` in `gather_verdict`).
- Crop rows written by the old cropdetect probe are dropped once at DB open (`migrate_media_bar_crop_columns`, settings key `bar_crop_resample_v1`); clean rows stay, so only formerly cropped files re-probe.
- Legacy `media.bar_crop` rows (`""` / `d`) predate the probed-clean marker and are decoded as unprobed (`db::decode_bar_crop` → `None`), so the next open re-probes once and rewrites the row in the new format; rows written before the marker were only ever produced from real metadata, so crop rows stay trusted (still validated via `CropRect::parse_video_crop`).
