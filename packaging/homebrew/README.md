# Homebrew formula (source of truth for the tap)

Copy into [`adrianov/homebrew-rhino-player`](https://github.com/adrianov/homebrew-rhino-player) as `Formula/rhino-player.rb` when releasing a tap update.

```sh
cp packaging/homebrew/rhino-player.rb /path/to/homebrew-rhino-player/Formula/rhino-player.rb
```

**Finder / Dock launch fix** (`macos_prime_homebrew_runtime_env`) lives in this repo from **1.6.2** onward. Until the tap’s stable URL points at that tag:

```sh
brew install --HEAD adrianov/rhino-player/rhino-player
```

After tagging **v1.6.2**, update `url` / `sha256` in the formula (`curl -fsSL …tar.gz | shasum -a 256`), copy to the tap, and `brew reinstall`.

The formula builds from the GitHub release tarball (or `head`), installs PREFIX share assets, and on macOS builds a signed **`Rhino Player.app`** that links the CLI binary into the bundle. Runtime needs Homebrew GTK 4 / libadwaita / mpv; the app primes `XDG_DATA_DIRS` so Finder / Dock launches find GSettings schemas.
