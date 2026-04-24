# Preferences and persistent settings

**Name:** GSettings / dconf or portable settings

**Implementation status:** Not started

**Use cases:** Set subtitle look, default volume, hardware decode, and “remember my place” once—apply everywhere next launch.

**Short description:** Preferences dialog: open in new window, hardware decode, volume normalization, subtitle appearance (color, font, scale, background), language priority strings, thumbnail preview, save session, save watch positions, and related toggles. Values sync to running mpv.

**Long description:** Use `Gio.Settings` with a compiled schema, or a TOML/JSON file; the implementation choice is recorded here when fixed. A sync step applies: `sub-*`, `slang`/`alang`, `save-position-on-quit`, `volume`, `hwdec` vs `vf` hflip/vflip, loudnorm filter when normalization is on.

**Specification:**

- Every user-visible option has a key and default.
- Changing a setting updates mpv live without restart, except when a reinit is unavoidable (document exceptions).
- Window maximized state may be remembered (`is-maximized`).
