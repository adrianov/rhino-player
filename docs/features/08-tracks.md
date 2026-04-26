# Tracks: audio, video, subtitles

**Name:** Multi-track selection

**Implementation status:** Done for **audio** track selection; subtitles / video / external tracks: not started

**Use cases:** Watch in the right language, load a better subtitle file, or pick an alternate video stream when the file contains several.

**Short description:** Menus to select subtitle, audio, and video tracks; add external sub/audio; reflect `sid`/`aid`/`vid` and `track-list` changes in the UI. **Current scope:** the header **Sound** control (volume icon) opens one popover: **Volume** (level + mute) and, **only if there are two or more** audio streams, a **track list**; choosing a row sets mpv’s `aid`.

**Long description:** The `track-list` property populates the UI with title and language when present. **Audio (implemented first):** the same popover as volume: a **Volume** row (mute + scale), then **only if** there are **at least two** `type: audio` entries, a **scrollable** list (no section title): each stream is a row with a **radio** (`CheckButton` group). A single track is not shown (no choice to make). There is **no** “None” / no-audio row—**mute** covers that. Labels follow “title – language” with a “Track n” fallback. The list is rebuilt when the popover opens. If there are **zero or one** audio streams, the track block is **hidden** (not an empty section). Subtitles, video, and add-external-file flows stay below for later work.

**Specification:**

**Audio track selection (current)**

- A header **MenuButton** (volume / sound icon) with tooltip for sound; **one** popover shared with [volume UI](22-audio-volume-mute.md): **Volume** row first; the **track** block (scroll list) appears only when `track-list` contains at least **two** **audio** streams. No separate “Audio” heading in the popover.
- The track list includes only `track-list` entries with `type` **audio**. Each row is a **radio** in a single group; the row matching the current `aid` is selected (if `aid` is `no` / muted-off, no row is active until the user picks a track).
- Choosing a track sets `aid` to that track’s `id` (int). **No** UI to set `aid` to `no` here (use **mute**).
- If there are no audio entries, or only one, the track **section is not shown** (only the volume row). After each successful **load** (short delayed tick with subtitle apply), the app still sets [aid] to the only audio [track-list] id if there is **exactly one** stream, and repairs explicit `aid=no` when there are **several** (so playback is not left silent when the demuxer or stale state had no track selected).
- No extra toasts or notifications; errors setting `aid` are ignored in the UI (log only if the project already logs mpv errors elsewhere).

**Later (not implemented yet)**

- Stateful actions / menus for external subs and add-audio; `sub` / `video` track picks; `sub_add` / `audio_add` and prefs (`sub-auto`, `audio-file-auto`) as in [Preferences](14-preferences.md).
- Show video track control only if more than one non-albumart video track; update on `track-list` change without requiring popover re-open (full multi-track spec).
