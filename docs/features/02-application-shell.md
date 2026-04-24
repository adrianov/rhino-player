# Application shell (Adwaita app, lifecycle)

**Name:** Application shell (Adwaita app, lifecycle)

**Implementation status:** In progress

**Use cases:** Users get a familiar GNOME-style app: one icon, standard menus, predictable quit/open behavior, and a window that fits the rest of the desktop.

**Short description:** A `GtkApplication` / `adw::Application` with application ID, primary window, global actions (quit, preferences, about, shortcuts), and dark style preference where appropriate.

**Long description:** The app follows GNOME HIG: libadwaita for layout and styling, a single app instance with optional new-window or secondary instances per preferences, and standard accelerators. On startup, register actions and present the main window. On shutdown, clean up mpv and persist session if enabled. About dialog should show app name, version, license, and links without blocking playback logic.

**Current code:** `adw::Application` with ID `ch.rhino.RhinoPlayer`, `adw::ApplicationWindow`, `adw::ToolbarView` (header + `GLArea` + bottom seek bar), `adw::HeaderBar` with app menu, `app.open` (Ctrl+O), `app.about` (F1), `app.quit` (Ctrl+Q), and `gtk::AboutDialog`. `adw::init()` and `adw::StyleManager` force dark.

**Specification:**

- Application ID in reverse-DNS form (`ch.rhino.RhinoPlayer`) registered once.
- Global shortcuts: at minimum quit (Ctrl+Q) and open preferences (Ctrl+,) as applicable; about from app menu.
- `activate` shows the main window; `open` receives files/URIs and forwards to the window/playlist layer (see [Open and CLI](06-open-and-cli.md)).
- Prefer or default to dark theme for a cinema-style experience unless the user overrides (system or in-app).
