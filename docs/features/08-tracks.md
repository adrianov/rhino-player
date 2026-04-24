# Tracks: audio, video, subtitles

**Name:** Multi-track selection

**Implementation status:** Not started

**Use cases:** Watch in the right language, load a better subtitle file, or pick an alternate video stream when the file contains several.

**Short description:** Menus to select subtitle, audio, and video tracks; add external sub/audio; reflect `sid`/`aid`/`vid` and `track-list` changes in the UI.

**Long description:** The `track-list` property populates menu models with title + language when present. “None” clears selection where appropriate. Add subtitle/audio opens file dialogs. Sub visibility and icons update from `sub-visibility` and `sid`. Optional secondary subtitle cycling can map to a dedicated key binding.

**Specification:**

- Stateful actions for selected track id (integer) or “no” for subs as mpv allows.
- Update menus on `track-list` change; show video track control only if more than one non-albumart video track.
- `sub_add` / `audio_add` for external files; fuzzy auto-load settings use mpv options such as `sub-auto`, `audio-file-auto` as configured in [Preferences](14-preferences.md).
