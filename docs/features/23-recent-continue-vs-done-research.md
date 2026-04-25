# Recent list: “still watching” vs finished, thumbnails, remove + undo

**Name:** Recent history semantics, removal, and undo

**Implementation status:** Research / plan only (not implemented)

**Use cases:** Keep the recent grid aligned with **shows in progress** (not a dump of everything ever opened). Avoid useless or misleading thumbnails at **end of file**. Let users **drop** an entry with one click, with a **safe undo** if it was accidental.

**Short description:** Define “finished” vs “in progress,” adjust DB + UI rules, add a **dismiss** control on each card, and an **undo** path (likely `AdwToast`) without confirmation dialogs.

---

## 1. Current behavior (codebase, 2026)

| Area | Behavior |
|------|------------|
| **`history` table** | Append / touch on **open** (`history::record` from `try_load` when `LoadOpts::record`). Ordered by `last_opened`; max 20 rows (`db::MAX_HISTORY`). |
| **`media` table** | `duration_sec`, `time_pos_sec`, `thumb_png` (+ mtime / `thumb_time_pos_sec`) updated from **`record_playback_for_current`**, **`set_thumb`**, quit path **`save_cached_thumb`**, grid **`ensure_thumbnail`**. |
| **Sibling advance at EOF** | `maybe_advance_sibling_on_eof` → `try_load` → **`MpvBundle::load_file_path`**, which calls **`write_resume_snapshot`**, **`record_playback_for_current`** with the **still-loaded** file (the one that just hit EOF), then `loadfile` the next. So the **finished** file gets **playback row** written with **end-ish** `time-pos` / `duration`; no **`save_cached_thumb`** in that path. |
| **Back to grid (Escape)** | Idle chain: **`record_playback_for_current`**, later **`save_cached_thumb`** — this is where a **finished** file can still get a **new or updated** raster thumb (often a **black or last frame**), which matches the user complaint. |
| **Stale / remove today** | Missing file: grey card; click uses **`on_stale` → `history::remove`**. No general “remove seen file,” no undo. |

---

## 2. Research: UX patterns (GNOME / Adwaita)

- **“Undo” for destructive list actions** in core apps is almost always a **transient bar** (toast / banner) with an **“Undo”** action, not a confirmation dialog. Reference behavior: **Files** (trash/undo), **Epiphany** (tab closed), many **Settings** pages.
- **libadwaita 1.4+:** `adw::Toast` with `add_button` / `Button::new` linking to a callback, optional **`timeout`** (0 = no auto-dismiss, else typically **3–8 s** for undo windows). `Toast` is usually hosted in an **`adw::ToastOverlay`** (or the shell’s `AdwApplicationWindow` toast API if you standardize on it).
- **No extra confirmation** for “remove from list” matches **music playlist** / **read-it-later** patterns; **undo** is the safety net.
- **Accessibility:** The dismiss (✕) must be a real **Button** (or `GtkButton` in overlay) with **`accessible-name`** = “Remove from recent” (or similar), not only a gesture.

---

## 3. Proposed product rules

### 3.1 “Finished” file (no longer “continue watching”)

- **Definition (v1):** Natural **end of playback** for a local file: we already detect **`eof-reached`** for sibling advance. Treat **“user watched to EOF”** the same when **there is no next file** to load: still **exactly one** `eof` handling pass per file (existing `sibling_eof_done` gate).
- **Action on finished:**  
  1. **Do not** run **`save_cached_thumb`** / **quit-screenshot** for that file **because** it finished (see §4).  
  2. **Remove** the canonical path from **`history`** (and see §4 for **`media`**).  
  3. Optionally **clear** that path’s **watch_later** sidecar if we want “re-open from zero”; **product decision** — default in plan: **leave watch_later** unless product wants “finished = forget resume” (bigger change).

### 3.2 “In progress” (stays in recent)

- **Opened** and **not** finished: stays in `history` as today, thumbs + `%` updated on switch / back-to-grid (subject to §4.2).

### 3.3 Manual remove (✕ on card)

- **No confirmation.**
- **Removes** `history` row (and **media** cleanup per §4).
- **Shows** an **`AdwToast`**: e.g. “Removed from recent” with **[Undo]**.
- **Undo:** re-insert the path into `history` at the **front** (`history::record` semantics), and **restore** `media` data **if** we stashed a small snapshot in memory (see §5).

### 3.4 Edge cases to specify in implementation

- **Finish + sibling advance:** remove **only** the **completed** file from `history` before/after `loadfile` next; the **new** file should get **`history::record`** as today when `try_load` runs with `record: true` (or we dedupe: opening next is still “an open” — keep current `record` behavior for the *new* file).  
- **Finish, no next file:** remove finished file from `history`; user stays on idle player — grid may appear only after Escape; **back_to_browse** should not re-add the finished file (already removed).  
- **Remove then Undo** within timeout: list order may differ slightly from “exact previous order” if we only call `record` (front) — **acceptable** for v1.  
- **Stream / URL later:** out of scope; **local files** only for this doc unless `06-open-and-cli` is updated.

---

## 4. Thumbnails: what to skip / clear

| Trigger | Proposed change |
|--------|-----------------|
| **EOF “finished”** (auto or last-in-folder) | **Do not** update thumb from **end state**; when we **drop** the path from `history`, either **`DELETE` `media` row** for that path or **`thumb_png` = NULL`**, `time_pos_sec` cleared — grid then shows **placeholder** if the path ever reappears. **Avoid** `save_cached_thumb` on paths we classify as “finished and removed.” |
| **Escape to grid** after **in-progress** | Keep current **`save_cached_thumb` + `record_playback`** (still watching). |
| **Escape to grid** after **finished** (if we still allow that state) | Skip thumb capture for finished-only session — requires a **flag** in session (“last file ended at EOF”) or **history membership** check: if not in `history` and we’re not showing it, skip. |

**Implementation note:** `load_file_path` always **`record_playback_for_current`** for the *previous* file. For a **finished** file, writing **~100%** in `media` is **ok** for analytics or confusing for “% on re-add”; **recommend** on finish: **remove** `media` row when removing from `history`, or set **`time_pos_sec = 0`** and keep duration only if needed — **decide in implementation**; simplest is **delete `media` row** when **removing from `history` for “done”** so re-open is clean.

---

## 5. Undo: data to stash (in-memory)

For **manual ✕** (and optionally **auto-finish remove** if we ever toast it):

- **`UndoToken`** (held **5–8 s** on the main loop, e.g. `glib::source::timeout_add_once` to forget):  
  - `path` (canonical `String`)  
  - **Optional** `last_opened` / order hint — **or** just call `history::record(path)` to put it back on top.  
  - **Optional** restore **`media`**: clone small structs from DB **before** delete (`duration`, `time_pos`, thumb BLOB) if we want **pixel-identical** undo; v1 can **skip** and only restore **history** (user loses % until next play).

**Toast wiring:** one global or per-window **`ToastOverlay`**; on Undo, `history::record` + optional `db::` restore; `recent_view` **refill** or targeted row refresh.

---

## 6. Implementation phases (suggested)

1. **DB helpers:** `remove_history` already; add **`remove_media_path`** or `delete media where path = ?` used whenever we **remove** from `history` for this feature.  
2. **Finish path:** in **`maybe_advance_sibling_on_eof`**, when we detect `eof` and resolve `finished` path, **`history::remove` + clear media** for `finished` **before** `try_load(next)`; same when **`next` is `None`**. **Guard** so we only run **once** per file (`sibling_eof_done` already).  
3. **Escape / quit:** if **`back_to_browse`**, skip **`save_cached_thumb`** when the only played file is **not** in `history` (finished-and-removed) or when **`eof-reached` still true** and we’ve committed finish — **needs a `Cell<bool>` or “session completed paths”** to avoid another DB query in hot path.  
4. **UI:** **✕** button on each card (non-stale and stale?), **`connect_clicked` stop propagation** if needed so card open doesn’t fire.  
5. **`AdwToast` + ToastOverlay** + **undo** path.  
6. **Docs:** update **`21-recent-videos-launch.md`**, **`docs/README.md` index**, product copy for toast strings.

---

## 7. Open questions (answer before or during implement)

- **Watch_later on “finished”:** keep resume file on disk (user re-opens from file manager) vs delete — **recommend: keep** for v1.  
- **Re-add via Undo after auto-remove:** should we toast “Removed [name]” for **autoplay next**? Might be **noise**; **recommend: toast only for manual ✕**; auto-finish is **silent** remove.  
- **Percent 100%** before remove: if we **delete** `media` at finish, the grid won’t show a **flash** of 100% — good.

---

## 8. Related files (implementation touch list)

- `src/app.rs` — `maybe_advance_sibling_on_eof`, `back_to_browse` idle / `save_cached_thumb`  
- `src/db.rs` / `src/history.rs` — history + media delete helpers  
- `src/recent_view.rs` — card layout, ✕, refill, `schedule_thumb_backfill` (skip for removed)  
- `src/media_probe.rs` — `persist_on_quit`, `save_cached_thumb` guards  
- New or extended **`docs/features/21-…`** and this file’s **status** when shipped.

---

## 9. References (external)

- [GNOME HIG – Patterns](https://developer.gnome.org/hig/patterns/feedback/) (toasts, undo)  
- [libadwaita – `Toast` / `ToastOverlay`](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/) — use version matching the crate’s `libadwaita` **1.5** / `0.7` in this repo for API names.

This document is the **planning gate**; **do not** implement large behavior changes without updating **`21-recent-videos-launch.md`** in lockstep and adjusting acceptance criteria there.
