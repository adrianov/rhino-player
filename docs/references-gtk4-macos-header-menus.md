# GTK4 header menus on macOS (gdk-macos + native video)

Standard **[Gtk.MenuButton](https://docs.gtk.org/gtk4/class.MenuButton.html)** + **[Gtk.Popover](https://docs.gtk.org/gtk4/class.Popover.html)** in **windowed** mode (same widgets and `theme.rs` classes as Linux). macOS adds a small platform binding layer only where gdk-macos needs it.

Speed, sound, and subtitles share one content widget per menu — **no duplicated menu trees**. Windowed mode shows it in [`Gtk.Popover`](https://docs.gtk.org/gtk4/class.Popover.html); native fullscreen **reparents that same child** into an overlay [`Frame`](https://docs.gtk.org/gtk4/class.Frame.html) (gdk-macos cannot host a working popover popup surface in theater mode). Track lists rebuild synchronously on open in both paths before layout.

## Standard GTK / Adwaita contract (windowed)

- **`MenuButton.set_popover(Popover)`** — real menu control; shared **`rp-header-popover`** / **`rp-popover-box`** content (`theme.rs`).
- **Popover CSS** — style **`popover > contents`**, not the `popover` node ([Popover CSS nodes](https://docs.gtk.org/gtk4/class.Popover.html)).
- **Popover parent** — GTK positions popovers from a mapped parent widget and its Gdk surface geometry ([Popover](https://docs.gtk.org/gtk4/class.Popover.html): parent must be visible and mapped). `MenuButton` sets the parent automatically.
- **Layout** — display CSS in `theme.rs` (padding, list chrome). Adwaita draws popover frame, shadow, and arrow.
- **Outside dismiss** — `autohide=false` on header popovers; capture-phase dismiss on the shell (`chrome_macos_header_popovers.rs`).

## macOS binding (windowed)

gdk-macos can render popover **`contents`** translucent over the native video layer even when Adwaita theme is correct. Same class of bug as `macos_bottom_bar.rs`: **widget-level** CSS providers on map/show (`macos_header_menu::wire_popover`), not a second menu UI.

| Concern | Binding |
|--------|---------|
| Opaque body | `macos_header_menu::wire_popover` — attach provider to popover + inner box on **map/show** |
| Open press guard | `wire_menu_btn_open_guard` / `arm_menu_list_pick_guard` — compositing hold + dismiss pause |
| Layer invalidate defer | `defer_layer_invalidate()` — skip **`invalidate_window_layers`** while a popover popup exists or the open/arm window is active |
| Speed list | pick guard on popover map/show (windowed); macOS keeps menu open after pick — outside dismiss closes |
| Sibling menus | `header_menubtns_switch` — close other popovers before opening (windowed only) |

## Native fullscreen (theater) — why GtkPopover breaks

Native AppKit fullscreen (`toggleFullScreen:`) leaves **popover popup surfaces** with broken Gdk monitor layout. Layout calls `gdk_macos_monitor_get_workarea` with a stale monitor; GTK popdowns immediately. **Monitor stash + Gdk resync does not fix this** — the popup child surface stays broken in theater mode.

Observed failure modes when forcing GtkPopover in theater:

| Symptom | Cause |
|--------|--------|
| Menu opens then closes on one click | Broken popup geometry → instant popdown |
| Transparent rectangle on the left of the screen | Orphan **popover popup surface** at wrong monitor coordinates (often persists while menu is open) |
| Menu looks open but nothing is clickable | Popup surface steals compositing; and/or overlay panel had **`can_target(false)`** so clicks fell through |
| Clicks inside menu ignored / menu closes instantly | Outside-dismiss used **`ToolbarView.pick`** — overlay panel is **not** under `ToolbarView`; picks looked like “outside” |
| Header buttons look disabled | **`set_popover(None)`** without CSS — Adwaita greys `MenuButton` with no popover |
| Full-screen flash on open | **`invalidate_window_layers`** called synchronously on overlay open |
| Horizontal bands of stale header / title chrome through the video | gdk-macos GTK sublayer not repainted after **`outer_ovl`** child shown in theater; AppKit layer snapshot replay (see **Theater overlay compositing** below) |
| Traffic lights drift after menu close | AppKit resets stoplight frames during compositing refresh after our sync (fixed: remember exact per-button X/Y on first draw; re-apply cached frames after every sync and post-invalidate idle) |

## Native fullscreen — shipped solution (Overlay reparent)

**Do not** use a separate Gdk popover surface in theater mode. **Do** reparent the existing popover **child** into a **[Gtk.Overlay](https://docs.gtk.org/gtk4/class.Overlay.html)** panel on the main shell — same pattern as seek-bar preview (`seek_bar_preview.rs`).

### Widget tree

```
ApplicationWindow
└── outer_ovl (gtk::Overlay)          ← shell for dismiss pick + overlay children
    ├── child: root (adw::ToolbarView) — video, header bar, bottom chrome
    ├── overlay: seek preview frame    (may stack above; menu re-raised on open)
    └── overlay: rp-header-menu-overlay Frame — theater menus only
```

Wiring: `chrome_header_menubtns.rs` → `HeaderMenuOverlay::wire(outer_ovl, win, root, header, menus)`.

### Theater lifecycle

**Enter native fullscreen** (`fullscreened_notify`):

1. Close any open overlay panel (`hide_panel`).
2. **`detach_popovers`** — `MenuButton.set_popover(None)` on speed / sound / subtitles (stops gtk from spawning popup surfaces).
3. Add CSS class **`rp-header-menu-fs`** on each `MenuButton` (keeps normal icon/readout styling — see `theme_macos_header_compact.css`).

**Open menu** (capture `GestureClick` on `MenuButton`, fullscreen only):

1. Close sibling overlay / clear **`rp-header-menu-open`** on other buttons.
2. Move real popover **child** from `Popover` → overlay **`Frame`** panel.
3. Put invisible **placeholder** (`pop_ph`, zero size, `can_target(false)`) in the detached `Popover`; **`popdown`**.
4. Anchor panel under the pressed button (`macos_header_menu_overlay_place.rs`: margins on `outer_ovl`).
5. **`show_panel`** — `can_target(true)` on panel + **`enable_target_tree`** on content; **`raise_panel_top`** (unparent + re-`add_overlay`) so menu sits above seek preview.
6. Open state: CSS **`rp-header-menu-open`** (not `MenuButton.set_active` — avoids fighting overlay toggle).
7. Speed: **`arm_list_pick_on_open`** (same 300 ms guard as windowed map/show). **Audio / subtitles:** **`header_menu_tracks::refresh_*_on_open`** before panel layout (same synchronous rebuild as windowed **`Popover.connect_show`**).
8. **`on_overlay_surface_opened`** — compositing refresh (same tail as seek preview in theater).

**Close menu** — toggle same button, outside click, sibling switch, exit fullscreen, `popdown_all`:

1. Restore child from panel → `Popover`.
2. Hide panel; **`can_target(false)`** on panel.
3. **`on_menu_surface_closed`** — compositing tail (no full-window invalidate on open).

**Leave native fullscreen**:

1. `hide_panel` (restore children).
2. **`attach_popovers`** — `set_popover(Some(&pop))`; remove **`rp-header-menu-fs`**.

### Guards (belt-and-suspenders)

| Guard | Role |
|-------|------|
| Capture press + **release** claimed on `MenuButton` | Overlay owns toggle; blocks default popover activation |
| `connect_activate` → `set_active(false)` in fullscreen | MenuButton must not stay “active” without popover |
| `Popover` map/show → **`popdown`** in fullscreen | Safety net if popup surface is created |
| `wire_popover` map/show popdown in fullscreen | Same, on shared popover instances |
| **`rp-header-menu-fs`** CSS | Visual parity when popover detached |

### Outside dismiss (critical)

Dismiss controller lives on **`outer_ovl`**, not `ToolbarView`:

- **`shell.pick(x, y)`** must see overlay panel descendants.
- **`overlay_contains(picked)`** treats hits inside the menu panel as in-menu.
- Windowed popover content lives on a separate popup surface — dismiss on `outer_ovl` still works for header buttons; popover popup clicks do not route through the shell gesture (same as before).

### CSS classes

| Class | Where | Purpose |
|-------|--------|---------|
| `rp-header-popover` | Popover + overlay frame | Shared dark chrome |
| `rp-header-menu-overlay` | Overlay `Frame` | Theater panel shadow/radius (`macos_header_menu` provider) |
| `rp-header-menu-open` | Open `MenuButton` | Highlight without `set_active` |
| `rp-header-menu-fs` | Menu buttons in theater | Enabled look without attached popover |

### Pitfalls (do not reintroduce)

- **Do not** call **`invalidate_window_layers`** synchronously on overlay open — use **`on_overlay_surface_opened`** (arm + queue_draw, full invalidate after ~332 ms).
- **Seek preview in theater** — same helper as header menus (`on_overlay_surface_opened` via `seek_bar_preview/macos_compositing.rs`).
- **Do not** wire outside-dismiss pick on **`ToolbarView`** while overlay is on **`outer_ovl`**.
- **Do not** leave overlay panel at **`can_target(false)`** while visible — menu items never receive clicks.
- **Do not** keep popovers attached in theater **only** to avoid grey buttons — use detach + **`rp-header-menu-fs`** instead (orphan popup surface returns).
- **Do not** re-apply traffic-light X shift on every compositing refresh — use idempotent shift in **`macos_traffic_vertical.rs`**.

## Theater overlay compositing (stale gdk-macos layers)

Rhino on macOS uses a **hybrid render stack**: native mpv video in a **`CAOpenGLLayer`** at index 0 of the AppKit **`contentView`**, with gdk-macos drawing GTK chrome in a sublayer above it. The main video **`GLArea`** is transparent (`theme_macos_transparent.css` + alpha-0 GL clear) so the native layer shows through; header and bottom bars use **widget-level opaque CSS** (`macos_header_menu`, `macos_bottom_bar`).

**Theater overlays** (header menu panel, seek-bar preview frame) are **`GtkOverlay`** children on **`outer_ovl`**, not separate Gdk popup surfaces — same surface as the main window, so no extra compositor window.

### Symptom

When an overlay child becomes visible in native fullscreen, gdk-macos can fail to repaint its GTK sublayer. Stale tiles from the header / titlebar (window title text, “Player” label fragments) appear as **horizontal semi-transparent bands** across the video area. The overlay itself may look correct; the glitch is **behind** it on the video stack.

Same class of bug as Space cross-fade staleness documented in **`macos_window::invalidate_window_layers`** — but triggered by **overlay show/hide** during theater playback, not only display changes.

### Fix: deferred shell refresh (`on_overlay_surface_opened`)

**Do not** call **`invalidate_window_layers`** synchronously when an overlay opens — that causes a full-window flash (removed early in theater menu work).

**Do** use **`macos_header_menu::on_overlay_surface_opened`** (macOS only):

| Step | When | What |
|------|------|------|
| 1 | Open | **`arm_shell_compositing_hold`** — 300 ms window where **`defer_layer_invalidate()`** is true |
| 2 | Open (immediate) | **`refresh_registered_shell_compositing`** — `queue_draw` / `queue_allocate` on header, **`ToolbarView`**, main **`GLArea`**, bottom shell; **skips** layer invalidate while armed |
| 3 | Open (+332 ms) | **`refresh_registered_shell_compositing`** again — full pass including **`invalidate_window_layers`** after the arm window |

**Close** path (menu panel hide, seek preview hide): **`on_menu_surface_closed`** — disarm hold + idle **`refresh_registered_shell_compositing`**.

### Call sites

| Overlay | Open | Close |
|---------|------|-------|
| Header menu (speed / sound / subtitles) | **`HeaderMenuOverlay::toggle`** after **`show_panel`** | **`hide_panel`** → **`on_menu_surface_closed`** |
| Seek-bar preview | **`seek_bar_preview/macos_compositing::on_open`** (first show in theater only) | **`on_close`** → **`on_menu_surface_closed`** |

Seek preview additionally:

- **`raise_overlay_child`** — re-`add_overlay` so preview stacks correctly vs menu panel (menus call **`raise_panel_top`** on open).
- Widget-level opaque CSS on **`rp-seek-thumb-frame`**: frame chrome **`#2d2d2d`** (matches theme); preview **`GLArea`** **`#000000`** for letterboxing over the native video layer.

Windowed mode does not call **`on_overlay_surface_opened`** — popover popup surfaces and normal gdk repaints are sufficient; the stale-tile bug is theater-specific.

### Related helpers

- **`refresh_gdk_shell_compositing`** (`macos_window_gdk_layout.rs`) — low-level repaint + optional invalidate; honors **`defer_layer_invalidate()`**.
- **`refresh_registered_shell_compositing`** (`chrome_shell_layout.rs`) — resolves registered shell widget refs and calls the above.
- **`invalidate_window_layers`** (`macos_window.rs`) — **`setNeedsDisplay:`** + **`displayIfNeeded`** on **`contentView`**; drops cached backing store.

## Module map

| Module | Responsibility |
|--------|----------------|
| `macos_header_menu.rs` | Opaque paint, pick guard, **`on_overlay_surface_opened`** / **`on_menu_surface_closed`**, `defer_layer_invalidate`, `popdown_all`, open-state probes |
| `macos_header_menu_overlay.rs` | `HeaderMenuOverlay` — detach/attach, toggle, sibling close, thread-local registry |
| `macos_header_menu_overlay_place.rs` | Anchor math, scrolled max heights, `show_panel` / `hide_panel_widget`, target tree |
| `macos_header_menu_overlay_input.rs` | Capture gestures, activate block, popover map/show guards |
| `macos_header_menu_debug.rs` | Temporary stderr trace (remove when stable) |
| Seek preview (theater) | `seek_bar_preview/macos_compositing.rs` — raise overlay, opaque frame CSS, **`on_overlay_surface_opened`** |
| `macos_window_gdk_layout.rs` | Compositing refresh; defers invalidate while menus arm/open |
| `macos_traffic_vertical.rs` | Stoplight vertical align + one-time X shift |
| `chrome_macos_header_popovers.rs` | Outside-click dismiss on **`outer_ovl`** |
| `chrome_header_menubtns.rs` | Cluster wiring: switch (windowed) + overlay + dismiss |
| `header_popovers.rs` / `speed_menu.rs` | Popover content trees (single widget reparented in theater) |
| `header_menu_scroll.rs` | Shared scroll max heights + **`rp-header-scroll-*`** CSS tags for overlay restore |
| `theme_macos_header_compact.css` | **`rp-header-menu-fs`** styling |

## Debug

- **`RHINO_MACOS_MENU_DEBUG=1`** — open/close / active trace on stderr (`macos_header_menu_debug.rs`).
- **`RHINO_MACOS_MENU_DEBUG=trace`** (or value containing **`backtrace`**) — same logs plus **`std::backtrace::Backtrace`** on each event.
- Unset or **`0`** — no trace hooks, no log overhead.

## See also

- [17-window-behavior](features/17-window-behavior.md) — fullscreen chrome, autohide, header menus scenario
- [references-gtk4-toplevel-aspect.md](references-gtk4-toplevel-aspect.md)
- [28-playback-speed](features/28-playback-speed.md) — speed menu content
