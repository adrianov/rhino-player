# Subtitles: style, track picker, and auto-pick

**Name:** Subtitle rendering, track selection, and name-based auto-pick

**Implementation status:** Done (warm default yellow / outline, header button + popover, SQLite prefs, Levenshtein auto-pick from last manual track)

**Use cases:** Watch with readable on-screen text; pick the right track when a file has many subs; have the next file pick the sub stream closest to what you last chose by hand (or, before any manual pick, to `LANG`).

**Short description:** mpv `sub-*` defaults (warm yellow, dark outline, scale), a dedicated **Subtitles** header control with a track list and size/color options, and **automatic** normalized-Levenshtein selection against the **last sub track you chose** in the list (or a short `LANG` hint if none yet).

**Long description:** Defaults echo a readable theatrical look (yellowish text, legible size, border). A **Subtitles** `MenuButton` in the titlebar is **hidden** unless the current `track-list` has at least one **sub** track (no empty popover for container-only or sub-free files). The button uses a bounded resync after opening because mpv can populate embedded or external subtitle tracks after the first load callback; warm-preloaded continue opens run the same lightweight scan. Detection prefers the JSON `track-list` and falls back to `track-list/count` + per-track `type` properties. When shown, the control opens a popover: scrollable sub track list (and **Off**), a scale for `sub-scale`, and a text color control. Prefs (including the stored **last hand-picked** track label and whether the user last chose **Off**) are stored in `settings` in `rhino.sqlite` and re-applied when the player starts and when the user changes them. **Colors are applied as string `#RRGGBB` values** (libmpv expects string `sub-color` / `sub-border-color`; int properties would be ignored). After each successful `loadfile`, subtitle styling is re-applied, then a short delay runs **auto-pick** only if the user is not in **Off**-persistent mode: otherwise subs stay off. If auto-pick runs, it picks among sub tracks by best normalized Levenshtein score against the saved label, or (if the user has never chosen a sub track) against a short `LANG` hint, above a small similarity floor.

**Specification:**

**Scenarios (Gherkin):**

```gherkin
Feature: Subtitles styling and selection
  Scenario: Header button appears only when subs exist
    Given track-list eventually contains at least one subtitle stream for the loaded file
    When scanning completes
    Then the Subtitles menu button appears with track rows and styling controls

  Scenario: Manual pick persists for future files
    Given the user selects a subtitle track or Off from the popover
    When load completes on later files
    Then SQLite stores the last label choice and Off preference influences whether auto-pick runs

  Scenario: Auto-pick uses saved label or LANG before similarity threshold
    Given sub_off is false and load finishes with delay after styling apply
    When auto-pick runs without user intervention
    Then sid selects best match among tracks using normalized Levenshtein vs saved label or LANG fallback

  Scenario: Toolbar clearance keeps subs readable
    Given the bottom toolbar may hide or show during playback
    When chrome visibility changes
    Then sub-pos adjusts so subtitles remain above the transport row when documented
```

- `sub-color` / `sub-border-color` as **`#RRGGBB` strings**; `sub-border-size` / `sub-scale` and `sub-ass-override=force` so ASS subs follow the app style; re-apply after each load.
- `sub-pos` (0–100, 100 = mpv default at bottom) is raised when the bottom **ToolbarView** bar is revealed so text stays **above** the seek/times row (scaled with window and bar height); resets when chrome auto-hides.
- `sid` + `sub-visibility` for track choice and **Off**; manual track choice updates the **last label** in SQLite and clears the “subs off” flag for the next file’s Levenshtein pass.
- The titlebar subtitle button is hidden before/while scanning, then shown as soon as `track-list` contains a `sub` entry; the scan is bounded and stops once tracks are found.
- Choosing **Off** persists **`sub_off`** in SQLite: **no** Levenshtein on newly opened files; `sub-visibility` stays **off** after each load until the user picks a real track (which clears `sub_off`). No separate global toggle in the menu for this.
- When `sub_off` is false, auto-pick runs after load (no user switch); does not add OS notifications; errors setting properties are ignored in UI.

**See also:** [Tracks](08-tracks.md) (audio), [Preferences](14-preferences.md).
