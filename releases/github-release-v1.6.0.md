Rhino Player **1.6.0** — mpv-backed desktop player with a GTK 4 / libadwaita UI. Prebuilt **`.deb`** (Debian / Ubuntu) and macOS **`.dmg`** ship from **Assets**; see [README](https://github.com/adrianov/rhino-player/blob/v1.6.0/README.md).

### Highlights

- **Continue search** — find neighbours and catalog media above the continue strip (fuzzy / misspelled queries, trash from results)
- **DVD & Blu-ray** — unified DVD title timeline; Blu-ray / AVCHD open and richer disc track menus
- **Smooth Video** — load-aware pause under CPU pressure, measured FPS in the toolbar, playhead-preserving enable while playing; macOS vendors MVTools in the app
- **Fill screen** — fullscreen crop-to-fill, remembered per media
- **Folder open** — opens a folder at the last unfinished file (or the first video)
- **macOS polish** — Finder drag-and-drop, seek-preview framing, fullscreen / chrome stability
- **Incomplete downloads** — Direct Connect `.dctmp` play-through with EOF hold instead of auto-advance

### Download (Assets)

- **`rhino-player_1.6.0-1_amd64.deb`** — Debian / Ubuntu (**x86_64**)
- **`Rhino-Player-1.6.0-macos-arm64.dmg`** — macOS app (**Apple silicon**)

### Install (`.deb`)

```bash
sudo apt install ./rhino-player_1.6.0-1_amd64.deb
```

### Install (`.dmg`)

Open the disk image and drag **Rhino Player** to Applications (or run it from the volume). Runtime still needs Homebrew **GTK 4**, **libadwaita**, and **mpv** — see the README.

### Requirements

GTK 4, libadwaita, libmpv — on Linux declared as package dependencies; on macOS from Homebrew. Smooth Video needs VapourSynth-capable mpv + MVTools (bundled on macOS in the `.app`).

### License

GPL-3.0-or-later — see [`LICENSE`](https://github.com/adrianov/rhino-player/blob/v1.6.0/LICENSE).

---

**Source:** tag [`v1.6.0`](https://github.com/adrianov/rhino-player/releases/tag/v1.6.0)
