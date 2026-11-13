# Changelog

All notable changes to vinoPilot are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.1] - 2026-11-13

### Bug Fixes

- Updating a system-wide Flatpak install failed with "InstallBundle not allowed for user". It now asks for your password, as installing a bundle system-wide requires

## [0.4.0] - 2026-11-11

### Features

- Uninstalling an app removes its downloaded models and keeps your own data and settings. The confirm says so before you press Uninstall, in the window and the CLI
- vinoWhisper and vinoOpenEffects are uninstalled with their own `--uninstall` when the installed version has it

## [0.3.0] - 2026-11-09

### Features

- A .deb, an .rpm and a Flatpak bundle on every release, holding the same two binaries as the tarball. The Flatpak's commands run on the host, its only permission being `org.freedesktop.Flatpak`
- vinoPilot updates and uninstalls itself as the .deb, .rpm or Flatpak it came in: it fetches the release's file, checks it against `SHA256SUMS` and hands it to apt, dnf, zypper or flatpak
- The About popup and the version button name the package vinoPilot came in

### Changed

- **Breaking, `vinopilot-core`:** `Channel` has `Deb`, `Rpm` and `Flatpak`, for vinoPilot's own install. In `--json`, `installed.channel` can be `deb`, `rpm` or `flatpak` for vinopilot

## [0.2.0] - 2026-11-08

### Features

- The window's version is a button: it checks for a newer vinoPilot, updates it and restarts the window
- An About popup says how vinoPilot and its commands are installed
- `p` in the terminal UI checks, updates and restarts vinoPilot from the status line
- Updating or uninstalling vinoPilot takes the terminal UI along
- A warning when a command is in both `~/.local/bin` and `~/.cargo/bin`, where one hides the other
- The other vino apps' About pages list vinoPilot (vino-ui 0.4.0), and the window has vino-ui 0.4's quieter look: ink and near-white primary buttons, green kept for what is up to date or on

### Changed

- **Breaking, `vinopilot-core`:** `Id::Pilot` is gone; vinoPilot is `Id::App(App::Pilot)`, from vino-ui 0.4.0's `App`. Its name, blurb, repo, slug and `--json` output are unchanged

### Fixed

- Installing ovfetch or vinoPilot removes the other channel's copy
- vinoWhisper and vinoSpeak installs remove a release's `-upgrade` script

## [0.1.0] - 2026-11-07

### Features

- Install, update and uninstall vinoWhisper, vinoAuthFace, vinoOpenEffects, vinoPop, vinoSpeak, vinoSearch, vinoWatch, ovfetch, vinoUserInterface and vinoPilot itself, from their GitHub releases, a source checkout, PyPI or crates.io
- Detect how each is already installed, and check GitHub, PyPI, crates.io and checkouts for something newer
- `vinopilot` command with `--json` output as the API for other frontends
- `vinopilot-gui` window: a card grid with Update all, and the installer's output underneath
- `vinopilot-tui` on PyPI (`uv tool install vinopilot`): the same in a terminal, driving `vinopilot --json`
- `install.sh`: hash-checked release install, source install from a checkout, `--no-gui`, `--uninstall`
