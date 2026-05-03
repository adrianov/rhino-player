Rhino Player **1.0.0** — mpv-backed desktop player with a GTK 4 / libadwaita UI. **This release ships a Linux `.deb` only.** A packaged macOS zip may follow in a later release; macOS remains supported [from source](https://github.com/adrianov/rhino-player/blob/v1.0.0/README.md#macos-experimental).

### Highlights

- **Continue / resume** — recent grid with thumbnails and one-click resume
- **Folder-friendly playback** — sibling next/prev and EOF folder advance
- **Tracks & subtitles** — audio/subtitle selection; subtitle prefs
- **Seek bar preview** — hover to preview frames
- **Smooth Video (~60 FPS)** — optional VapourSynth + MVTools when installed separately
- **Desktop integration** — `.desktop` entry, hicolor icons, AppStream metainfo, **`man rhino-player`**

### Download (Assets)

- **`rhino-player_1.0.0-1_amd64.deb`** — Debian / Ubuntu (**x86_64**).

### Install

Download the `.deb` from **Assets**, then:

```bash
sudo apt install ./rhino-player_1.0.0-1_amd64.deb
```

If `apt` warns about user `_apt` and a path under your home directory, either ignore it (install usually still works) or copy the `.deb` to `/tmp` and run `sudo apt install /tmp/rhino-player_1.0.0-1_amd64.deb`.

### Requirements

GTK 4, libadwaita, libmpv — declared as package dependencies (`libgtk-4-1`, `libadwaita-1-0`, `libmpv2` or `libmpv1`).

### License

GPL-3.0-or-later — see [`LICENSE`](https://github.com/adrianov/rhino-player/blob/v1.0.0/LICENSE).

---

**Source:** tag [`v1.0.0`](https://github.com/adrianov/rhino-player/releases/tag/v1.0.0)
