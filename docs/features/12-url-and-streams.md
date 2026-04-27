# URL and network streams (yt-dlp / protocols)

**Name:** URL playback

**Implementation status:** Not started

**Use cases:** Play web streams and network URLs the same way as local files, with minimal friction.

**Short description:** Open network URLs and use yt-dlp (or the hook equivalent) for supported sites; validate schemes against `protocol-list` where applicable.

**Long description:** A small Adwaita dialog with URL entry: accept `mpv`-supported schemes, or bare hostnames (prepend `https://`), or existing filesystem paths. “Open” vs “Add” maps to `replace` vs `append-play`. mpv is configured with `ytdl` enabled and scripts as needed for stream extraction.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: URL and network streams (target behavior — not started)
  Scenario: Open validated URL
    Given the user enters or pastes a supported scheme or normalized hostname
    When they confirm Open or Add in the dialog
    Then mpv receives loadfile with replace or append-play as chosen
    And invalid input is rejected without corrupting the playlist

  Scenario: Playlist dialog stays in sync
    Given the playlist dialog is visible
    When a new URL is appended successfully
    Then the dialog content refreshes like other queue updates
```

- Parse URL, validate, call `loadfile` with the correct mode.
- If the playlist dialog is open, refresh it after add (same window flow as other queue updates).
