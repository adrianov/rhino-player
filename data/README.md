# Installed data (Freedesktop / GNOME)

| Path (repo) | Typical install | Role |
|---------------|-----------------|------|
| [icons/README.md](icons/README.md) | `share/icons` | hicolor PNGs, name `ch.rhino.RhinoPlayer` |
| `applications/ch.rhino.RhinoPlayer.desktop` | `share/applications` | Launcher, `Icon=` and `StartupWMClass=` |
| `metainfo/ch.rhino.RhinoPlayer.metainfo.xml` | `share/metainfo` | AppStream catalog |

`Exec=` in the in-repo file assumes `rhino-player` is on `PATH` (e.g. `/usr/bin/rhino-player`). For **GNOME and KDE** to show the correct **taskbar, dash, and alt+tab** icon, the `ch.rhino.RhinoPlayer.desktop` file must be installed (the shell looks up `Icon=` from that name; `gtk::Window` / the icon theme are not always enough for the compositor). Use:

```bash
./data/install-to-user-dirs.sh
```

Pass your binary as the first argument if it is not the default `target/debug` or `target/release` path. Then log out and back in if the shell still shows a generic icon (cache / session).

There is no Meson/CMake in-tree yet; packagers or `sudo install` can copy these paths following the [XDG base directories] for system-wide data.

[XDG base directories]: https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
