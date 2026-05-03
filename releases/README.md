# Release artifacts (GitHub)

This directory is the **staging area** on your machine (or in CI) for files you **attach** to a [GitHub Release](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases).

## Git vs GitHub Releases (why binaries are not committed)

**GitHub does not store downloadable `.deb` / `.zip` assets inside the git tree.** A Release on GitHub has:

1. **The repo at a tag** — only what git tracks (source, scripts, this README).
2. **Separate upload attachments** — the built `.deb`, `.zip`, etc., stored by GitHub next to that Release.

So users download installers from the Release **Assets** list, not by cloning the repository. Committing large binaries to git would bloat history and slow every clone; the usual pattern is **build → upload as Release assets** (manually, `gh release create …`, or GitHub Actions).

The repo tracks **only** this README; built files under `releases/` stay **untracked** (see `.gitignore`) until you upload them to GitHub.

## Layout

Keep **one release tag’s assets** here at a time, or use unique versioned filenames so multiple builds do not collide.

| Platform | Artifact pattern | How it is produced |
|----------|------------------|-------------------|
| **Ubuntu / Debian** | `rhino-player_<semver>-<deb-rev>_<arch>.deb` | `./scripts/build-deb.sh` or `./scripts/stage-github-release.sh` on Linux |
| **macOS** | `Rhino-Player-<semver>-macos-<arm64\|x86_64>.zip` | `./scripts/stage-github-release.sh` on macOS (zipped `.app`) |
| **Windows** (planned) | `rhino-player-<semver>-windows-x86_64.zip` (suggested) | Not automated yet; ship portable `.exe` / installer when added |

`<arch>` for `.deb` follows Debian (`amd64`, `arm64`, …).

## Commands

- **Linux (.deb):** `./scripts/stage-github-release.sh` or `./scripts/build-deb.sh`
- **macOS (.zip):** `./scripts/stage-github-release.sh` (builds `Rhino Player.app`, then zips it into `releases/`)

Override deb output only if needed: `OUTPUT=/tmp ./scripts/build-deb.sh`

## Upload to GitHub

From the repo root, after tagging:

```bash
gh release create "v${VERSION}" releases/rhino-player_*.deb releases/Rhino-Player-*.zip --generate-notes
```

Adjust globs if you only built one platform. You can also upload assets through the GitHub web UI: attach the same files from `releases/`.

### Actions matrix (hint)

Typical CI shape: **job per OS**, each runs the matching script, then **`actions/upload-release-asset`** or **`softprops/action-gh-release`** with `files: releases/*` for that job’s artifact.
