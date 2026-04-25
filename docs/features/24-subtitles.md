# Subtitles: style, track picker, and auto-pick

**Name:** Subtitle rendering, track selection, and name-based auto-pick

**Implementation status:** Done (warm default yellow / outline, header button + popover, SQLite prefs, Levenshtein auto-pick from last manual track)

**Use cases:** Watch with readable on-screen text; pick the right track when a file has many subs; have the next file pick the sub stream closest to what you last chose by hand (or, before any manual pick, to `LANG`).

**Short description:** mpv `sub-*` defaults (warm yellow, dark outline, scale), a dedicated **Subtitles** header control with a track list and size/color options, and **automatic** normalized-Levenshtein selection against the **last sub track you chose** in the list (or a short `LANG` hint if none yet).

**Long description:** Defaults echo a readable theatrical look (yellowish text, legible size, border). A **Subtitles** `MenuButton` opens a popover: scrollable sub track list (and **Off**), a scale for `sub-scale`, and a text color control. Prefs (including the stored **last hand-picked** track label and whether the user last chose **Off**) are stored in `settings` in `rhino.sqlite` and re-applied when the player starts and when the user changes them. **Colors are applied as string `#RRGGBB` values** (libmpv expects string `sub-color` / `sub-border-color`; int properties would be ignored). After each successful `loadfile`, subtitle styling is re-applied, then a short delay runs **auto-pick** only if the user is not in **Off**-persistent mode: otherwise subs stay off. If auto-pick runs, it picks among sub tracks by best normalized Levenshtein score against the saved label, or (if the user has never chosen a sub track) against a short `LANG` hint, above a small similarity floor.

**Specification:**

- `sub-color` / `sub-border-color` as **`#RRGGBB` strings**; `sub-border-size` / `sub-scale` and `sub-ass-override=force` so ASS subs follow the app style; re-apply after each load.
- `sub-pos` (0–100, 100 = mpv default at bottom) is raised when the bottom **ToolbarView** bar is revealed so text stays **above** the seek/times row (scaled with window and bar height); resets when chrome auto-hides.
- `sid` + `sub-visibility` for track choice and **Off**; manual track choice updates the **last label** in SQLite and clears the “subs off” flag for the next file’s Levenshtein pass.
- Choosing **Off** persists **`sub_off`** in SQLite: **no** Levenshtein on newly opened files; `sub-visibility` stays **off** after each load until the user picks a real track (which clears `sub_off`). No separate global toggle in the menu for this.
- When `sub_off` is false, auto-pick runs after load (no user switch); does not add OS notifications; errors setting properties are ignored in UI.

**See also:** [Tracks](08-tracks.md) (audio), [Preferences](14-preferences.md).
