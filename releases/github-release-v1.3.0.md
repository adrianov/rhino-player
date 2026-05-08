Rhino Player **1.3.0** — mpv-backed desktop player with a GTK 4 / libadwaita UI. Prebuilt **`.deb`** (Linux) and a packaged **macOS `.zip`** (when built from `scripts/stage-github-release.sh` on macOS) ship from **Assets**; see [README](https://github.com/adrianov/rhino-player/blob/v1.3.0/README.md).

### Highlights

- **Smooth Video (~60 FPS)** — optional VapourSynth + MVTools; **bundled script adapts the workload automatically** so the effect scales better across machines
- **Continue / resume** — recent grid with thumbnails and one-click resume
- **Folder-friendly playback** — sibling next/prev and EOF folder advance
- **Tracks & subtitles** — audio/subtitle selection; subtitle prefs
- **Seek bar preview** — hover to preview frames
- **Desktop integration** — `.desktop` entry, hicolor icons, AppStream metainfo, **`man rhino-player`**

### Download (Assets)

- **`rhino-player_1.3.0-1_amd64.deb`** — Debian / Ubuntu (**x86_64**), when published for this tag.
- **`Rhino-Player-1.3.0-macos-<arch>.zip`** — macOS app bundle, when built and uploaded for this tag.

### Install (`.deb`)

Download the `.deb` from **Assets**, then:

```bash
sudo apt install ./rhino-player_1.3.0-1_amd64.deb
```

If `apt` warns about user `_apt` and a path under your home directory, either ignore it (install usually still works) or copy the `.deb` to `/tmp` and run `sudo apt install /tmp/rhino-player_1.3.0-1_amd64.deb`.

### Requirements

GTK 4, libadwaita, libmpv — declared as package dependencies (`libgtk-4-1`, `libadwaita-1-0`, `libmpv2` or `libmpv1`). Smooth Video needs VapourSynth-capable mpv + MVTools (see README).

### License

GPL-3.0-or-later — see [`LICENSE`](https://github.com/adrianov/rhino-player/blob/v1.3.0/LICENSE).

---

**Source:** tag [`v1.3.0`](https://github.com/adrianov/rhino-player/releases/tag/v1.3.0)
