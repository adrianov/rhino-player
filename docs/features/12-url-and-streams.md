# URL and network streams (yt-dlp / protocols)

---
status: planned
priority: p2
layers: [ui, mpv]
related: [06, 07]
mpv_props: [protocol-list]
---

## Use cases
- Play web streams and network URLs the same way as local files.
- Use yt-dlp under the hood for supported sites.

## Description
A small Adwaita dialog accepts URL input, validates against mpv-supported schemes (and treats bare hostnames as `https://`), then dispatches `loadfile` with `replace` or `append-play`. mpv is configured with `ytdl` enabled and any required scripts.

## Behavior

```gherkin
@status:planned @priority:p2 @layer:ui @area:url
Feature: URL and network streams

  Scenario: Open validated URL replaces current media
    Given the user enters a supported scheme or normalized hostname
    When they confirm Open
    Then mpv runs loadfile with replace for that URL

  Scenario: Add validated URL appends after the current item
    Given the user enters a supported scheme or normalized hostname
    When they confirm Add
    Then the playback engine loads the URL after the current item without replacing it

  Scenario: Invalid input is rejected
    Given the user enters input that matches no allowed scheme and is not a path
    When they confirm Open or Add
    Then no loadfile runs and the queue of titles is unchanged
```

## Notes
- Bare hostnames are normalised to `https://<host>`.
- yt-dlp handling lives entirely inside mpv; the app does not invoke yt-dlp directly.
