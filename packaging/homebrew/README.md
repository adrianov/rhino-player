# Homebrew formula (source of truth for the tap)

Copy into [`adrianov/homebrew-rhino-player`](https://github.com/adrianov/homebrew-rhino-player) as `Formula/rhino-player.rb` when releasing a tap update.

```sh
cp packaging/homebrew/rhino-player.rb /path/to/homebrew-rhino-player/Formula/rhino-player.rb
```

Stable `url` is still **v1.6.1**. Finder / Dock launch priming and everything in **1.7.0** need `main` until the next tag:

```sh
brew install --HEAD adrianov/rhino-player/rhino-player
```

After tagging **v1.7.0**, set `url` / `sha256` (`curl -fsSL …tar.gz | shasum -a 256`), copy the formula to the tap, and `brew reinstall`.

The formula builds from the GitHub release tarball (or `head`), installs PREFIX share assets, and on macOS builds a signed **`Rhino Player.app`** that links the CLI binary into the bundle. Runtime needs Homebrew GTK 4 / libadwaita / mpv; the app primes `XDG_DATA_DIRS` so Finder / Dock launches find GSettings schemas.
