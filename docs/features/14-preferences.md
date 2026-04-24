# Preferences and persistent settings

**Name:** GSettings / dconf or portable settings

**Implementation status:** In progress (per-file resume: mpv `save-position-on-quit` + `watch-later-dir` under `~/.config/rhino/watch_later`, no global prefs UI yet)

**Use cases:** Set subtitle look, default volume, hardware decode, and “remember my place” once—apply everywhere next launch.

**Short description:** Preferences dialog: open in new window, hardware decode, volume normalization, subtitle appearance (color, font, scale, background), language priority strings, thumbnail preview, save session, save watch positions, and related toggles. Values sync to running mpv.

**Long description:** Use `Gio.Settings` with a compiled schema, or a TOML/JSON file; the implementation choice is recorded here when fixed. A sync step applies: `sub-*`, `slang`/`alang`, `save-position-on-quit`, `volume`, `hwdec` vs `vf` hflip/vflip, loudnorm filter when normalization is on. **Per-file stop position** is implemented in the player core via libmpv: `save-position-on-quit` and a dedicated `watch-later-dir` (see [mpv embed](03-mpv-embedding.md)) so the next `loadfile` of the same path resumes where playback stopped, without sharing the user’s default `mpv` watch_later store unless we later add a setting.

**Specification:**

- Every user-visible option has a key and default.
- Changing a setting updates mpv live without restart, except when a reinit is unavoidable (document exceptions).
- Window maximized state may be remembered (`is-maximized`).
