# Changelog

All notable changes to vinoPilot are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] - 2026-11-27

### Features

- An app that is running can be stopped from its card with an inline `Stop & close`, in both the desktop app and the TUI.

### Changed

- Unnecessary prose is trimmed from the GUI and TUI, and the app uses `vino-user-interface` 0.6.1.

## [0.8.0] - 2026-11-26

### Features

- Redesigned the desktop app as a single-page hub using `vino-user-interface` 0.6.0: full-colour app logos on every card, hairline grid cells with hover previews, connected `All` / `Installed` / `Updates` filters, tonal split `Install` buttons, a header Settings gear popover for the source checkout folder, and responsive layout across 340–1440 px windows.
- Moved vinoPilot itself out of the header's `ⓘ` popup and into the apps grid alongside the other vinoPilot apps, with the same install, update, uninstall, and in-card `Restart to use …` logic.

## [0.7.0] - 2026-11-23

### Features

- vinoAuthFace can be installed, updated and uninstalled from its .deb or .rpm packages, offered after release and source on its card. vinoPilot picks the one your package manager takes (apt-get for deb; dnf, zypper or rpm for rpm; nothing on rpm-ostree), checks the files against `SHA256SUMS`, and finds an existing package install. An install also lets the package fetch the models and hook PAM; an update never enables anything; an uninstall keeps your face templates.

### Bug Fixes

- Uninstalling vinoWhisper or vinoSearch from its Flatpak also deletes the models and cache it kept under `~/.var/app`. Transcripts, the index and settings stay.
- The menu entry written by `vinopilot-gui --install-desktop` escapes special characters in the path to the program, so GLib no longer rejects it.

### Security

- Hardened how root steps are run through pkexec, how the desktop user is found for vinoAuthFace's installer, and the temporary files steps clean up.

### Documentation

- The README and the pages it links are shorter and clearer.

## [0.6.1] - 2026-11-17

### Bug Fixes

- An app installed as a uv tool from a checkout (vinoSpeak, vinoWhisper) shows `dev (git describe)`, not the version in its package metadata: vinoPilot asks the command, as it does for every other source build.
- vinoAuthFace's new logo in the About page and app list (vino-ui 0.5.1).

## [0.6.0] - 2026-11-17

### Features

- Tell a release from a source build by the version an app reports. Every vino app now prints `dev` for a build that isn't a release, and vinoPilot treats a `dev` binary as a source install even when a release marker sits beside it. The version column and card show `dev (git describe)` from the checkout, and a `dev` build is never offered an update by version number. vinoPilot stamps itself the same way: release builds print their version, a build from a checkout prints `dev`, and `--json`'s top-level `"vinopilot"` can now be `dev`.
- vinoWhisper can be installed from its Flatpak bundle (`io.github.karanshukla.vinowhisper`), offered last on its card, as vinoSearch's is.
- The launcher icon shows the running job's progress and a badge with the number of updates available (KDE's task manager, Dash to Dock).

### Bug Fixes

- Updating a system-wide Flatpak asks for a password through pkexec, instead of failing with "not allowed for user" when run from the window.
- The launcher bar shows progress during a one-step job, instead of sitting at 0 until it ends.

## [0.5.0] - 2026-11-14

### Features

- vinoSearch can be installed from its Flatpak bundle (`dev.vinosearch.VinoSearch`). The bundle is checked against the release's `.sha256`, installed for the user, and updated from the next release's bundle; uninstalling keeps `~/.var/app/dev.vinosearch.VinoSearch`. A release without a bundle says so
- The window draws its own title bar, as the other vino apps now do (vino-ui 0.5.0): vinoPilot, the version and the site up in the bar, minimise, maximise and close in the top right corner, and Check for updates and Update all beside them where there's room. The bar drags the window and the edges resize it. A tiling desktop keeps the header as it was, and `VINO_UI_TITLE_BAR=system` brings the desktop's bar back
- A two-finger touchpad swipe scrolls three times as far as before; a mouse wheel is unchanged

### Bug Fixes

- Installing from a source checkout that's on a feature branch no longer fails on `git pull`. The default branch is installed from a worktree, leaving the branch alone. vinoWhisper, vinoPop and vinoUserInterface, which run from the folder they're installed from, have the checkout switched to the default branch instead (local changes are stashed, and the log says how to get back)

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
