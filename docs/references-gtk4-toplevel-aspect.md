# GTK4 + GNOME: toplevel size and “aspect ratio” windows

This note lists **authoritative** APIs for **GTK4** and **GDK4** (what Rhino Player targets). It is not a line-for-line spec of our code.

## Toplevel size on Wayland and GTK4

- **[GdkToplevel::compute-size](https://docs.gtk.org/gdk4/signal.Toplevel.compute-size.html)**  
  Emitted when the toplevel’s size is being negotiated (e.g. with the Wayland compositor). The application can answer by mutating a **[GdkToplevelSize](https://docs.gtk.org/gdk4/struct.ToplevelSize.html)**: in particular **`gdk_toplevel_size_set_size`**, and may read **`gdk_toplevel_size_get_bounds`**. Failing to handle the signal is documented as leading to an arbitrary size.

- **[GdkToplevelSize](https://docs.gtk.org/gdk4/struct.ToplevelSize.html)**  
  Opaque; supports **bounds**, **set_size**, **set_min_size**, **set_shadow_width**.

- **GDK / Wayland developer notes**  
  - [GDK4 — Wayland interaction](https://docs.gtk.org/gdk4/wayland.html) (backend checks; backend-specific headers when you must call `gdkwayland-` APIs).  
  - Wayland and GTK4 have moved most window sizing into **present** / **toplevel** flows rather than X11 **geometry hints** alone.

## What is *not* the primary story in GTK4

- **GTK3-style `GdkGeometry` + `set_geometry_hints` with `min_aspect` / `max_aspect`** is the classic **X11** pattern. It still appears in many tutorials. On **GTK4 + Wayland + Mutter (GNOME)**, the well-supported, documented place to participate in the **next** size is the **`compute-size` → `ToplevelSize`** path above, not a separate “window aspect” property on `GtkWindow` or `AdwWindow`.

- **[GtkAspectFrame](https://docs.gtk.org/gtk4/class.AspectFrame.html)**  
  Constrains the **child widget’s** allocation inside the window; it does **not** set the **window**’s aspect by itself. Useful for a video widget’s frame, not as a global substitute for `GdkToplevel::compute-size`.

## libadwaita

- **[AdwWindow](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1.8/class.Window.html)** subclasses `GtkWindow` and does not replace GDK’s toplevel sizing protocol. Application-specific policies (e.g. match video **w**/**h** from a decoder) are still application logic, implemented with GTK/GDK above.

## Implementation caution (re-entrancy)

Calling **`gdk_toplevel_size_set_size`** from a `compute-size` callback can lead to **synchronous re-entry** of the same callback while the first call is still on the stack. **Do not** treat every nested run as spurious: during interactive resize, **`GtkWindow` geometry can lag the underlying `GdkSurface`** (the pair that better reflects the compositor’s in-flight size may appear in a nested pass). Cap recursion depth to avoid runaway stacks; prefer dimensions from **surface vs window** when they disagree and one side’s aspect is further from your target.

## Internal project doc

- Feature behavior: [features/17-window-behavior.md](features/17-window-behavior.md).
