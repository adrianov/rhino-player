# Application shell (Adwaita app, lifecycle)

**Name:** Application shell (Adwaita app, lifecycle)

**Implementation status:** Done (no separate preferences or Shortcuts help window yet; [06-open-and-cli](06-open-and-cli.md) for `open` scope)

**Use cases:** Users get a familiar GNOME-style app: one icon, standard menus, predictable quit/open behavior, and a window that fits the rest of the desktop.

**Short description:** A `GtkApplication` / `adw::Application` with application ID, primary window, global actions (quit, preferences, about, shortcuts), and dark style preference where appropriate.

**Long description:** The app follows GNOME HIG: libadwaita for layout and styling, a single app instance with optional new-window or secondary instances per preferences, and standard accelerators. On startup, register actions and present the main window. On shutdown, clean up mpv and persist session if enabled. About dialog should show app name, version, license, and links without blocking playback logic.

**Current code:** `adw::Application` with ID `ch.rhino.RhinoPlayer`, `adw::ApplicationWindow`, `adw::ToolbarView` (header + `GLArea` + bottom bar with play/pause + seek + times), `adw::HeaderBar` with volume and app menu, `app.open` (Ctrl+O), `app.about` (F1), `app.quit` (q, Ctrl+Q), and `gtk::AboutDialog` (logo from themed icon `ch.rhino.RhinoPlayer`). `icons::register_hicolor_from_manifest` adds `data/icons` to the icon search path for `cargo run`. `main` sets `glib::set_prgname` to the same id so the process name matches the `*.desktop` basename. For **panel / alt+tab** icons, GNOME-style shells use the installed `ch.rhino.RhinoPlayer.desktop` → `data/install-to-user-dirs.sh` to `~/.local/share/`. Packaged: `data/applications` + `data/metainfo` + `data/icons` follow Freedesktop naming (see `data/README.md`). `adw::init()` and `adw::StyleManager` force dark.

**Specification:**

- Application ID in reverse-DNS form (`ch.rhino.RhinoPlayer`) registered once; the same string is the **icon name** in the hicolor theme, `.desktop` `Icon=`, and window / About dialog branding. **GNOME** (and similar) resolve dash / app switcher artwork from a **Freedesktop** `applications/*.desktop` on `XDG_DATA_DIRS`, not from `GtkWindow` alone; the project ships `data/install-to-user-dirs.sh` to install into `~/.local/share` for local runs.
- Global shortcuts: at minimum quit (q, Ctrl+Q) and open preferences (Ctrl+,) as applicable; about from app menu.
- `activate` shows the main window; `open` receives files/URIs and forwards to the window/playlist layer (see [Open and CLI](06-open-and-cli.md)).
- Prefer or default to dark theme for a focused viewing experience unless the user overrides (system or in-app).
