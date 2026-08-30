Rhino Player **1.6.2** — Homebrew macOS `.app` launches from Finder / Dock (GSettings schema path).

### Highlights

- **Finder / Dock launch** — primes Homebrew `XDG_DATA_DIRS` before GTK so libadwaita finds GSettings schemas when Launch Services starts the `.app` (no login-shell env)
- **Homebrew formula mirror** — `packaging/homebrew/rhino-player.rb` (vendors Smooth MVTools when present; installs vendor script under `share/rhino-player/scripts`)
- **`--version`** — reliable brew test without needing a display for GApplication help

### Download (Assets)

- **`rhino-player_1.6.2-1_amd64.deb`** — Debian / Ubuntu (**x86_64**)
- **`Rhino-Player-1.6.2-macos-arm64.dmg`** — macOS app (**Apple silicon**)

### Install

```sh
brew install adrianov/rhino-player/rhino-player
# or until the tap mirrors this tag:
brew install --HEAD adrianov/rhino-player/rhino-player
```

```bash
sudo apt install ./rhino-player_1.6.2-1_amd64.deb
```

### License

GPL-3.0-or-later — see [`LICENSE`](https://github.com/adrianov/rhino-player/blob/v1.6.2/LICENSE).

---

**Source:** tag [`v1.6.2`](https://github.com/adrianov/rhino-player/releases/tag/v1.6.2)
