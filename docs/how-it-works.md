# How vinoPilot works

`vinoPilot` delegates installation to each app's own checksum-verified installer. Its job is to detect how each app is installed, check for updates from the matching source, and run the right script for each install method.

## Installation methods

| Method | Install | Update | `--via` |
|---|---|---|---|
| Release | Runs the app's one-line installer from its latest GitHub release and verifies downloads against `SHA256SUMS` | Runs the local `<app>-upgrade` script saved during installation | `release` |
| Source | Clones the repository into your source folder and runs the checkout's install script | Runs `git pull --ff-only`, then re-runs the install script | `source` |
| Package | Installs with `uv tool` (PyPI) or `cargo install` (crates.io) | Upgrades with the same package tool | `pypi`, `crates` |
| Flatpak (`vinoWhisper`, `vinoSearch`) | Downloads the release's Flatpak bundle, verifies its checksum, and runs `flatpak install --user` | Downloads and installs the newer release's bundle over it | `flatpak` |

The default source folder is `~/Development` if it already exists, or `~/src` otherwise. You can change it in the GUI or by running `vinopilot config source_root DIR`.

Uninstalling a Flatpak deletes its downloaded models and cache while preserving your data and settings in `~/.var/app`.

### Checkouts on another branch

`vinoPilot` never pulls a checkout that is not on its default branch. Instead, behavior depends on the app:

| Apps | Behavior when off the default branch |
|---|---|
| Most apps | Checks out the default branch into a temporary worktree at `~/.local/share/vinopilot/worktrees/<repo>`, builds/installs from it, and removes the worktree afterward. Your main checkout is left untouched. |
| `vinoWhisper`, `vinoPop`, `vinoUserInterface` | Switches your main checkout to the default branch after stashing any uncommitted changes, and logs how to return to your branch. |

Those three apps execute directly from their checkout directory, so installing from a temporary worktree would leave the app running from a separate hidden copy.

After a worktree install, `vinoPilot` records the installed commit in your checkout at `refs/vinopilot/installed`. `vinopilot check` then measures commits behind relative to that ref rather than your active branch.

## Detection

`vinoPilot` inspects your system directly instead of maintaining a private install database, so it automatically recognizes apps installed manually. It checks:

- The `<app>-upgrade` scripts placed in `~/.local/bin` by release installers.
- `uv` tool receipts.
- Cargo's `~/.cargo/.crates.toml`, parsed as TOML: only keys inside the `[v1]` table count, and an unparseable file is treated as having no Cargo installs (as of 2026-11-15; earlier versions matched any quoted `name version (source)` line anywhere in the file).
- Each binary's `--version` output (with a 2-second timeout).

Installations in your home directory take precedence. If none is found, `vinoPilot` checks for its own `.deb`, `.rpm`, or Flatpak package, and for Flatpak installations of supported apps. Because a Flatpak app runs inside its sandbox, its version is read from the newest release entry in its AppStream metainfo.

## Network access

`vinoPilot` stays offline until you click **Check for updates** or run `vinopilot check`. Checking for updates queries GitHub, PyPI, and crates.io for the newest releases and runs `git fetch` in each local checkout.

## Limits

- **Uninstalling an app deletes its downloaded models** while keeping user data and settings (such as transcripts, search indexes, and face templates). Each app's uninstaller determines the exact cleanup; `vinoPilot` invokes it. Shared directories such as `~/.cache/huggingface` and runtimes installed by `ovfetch` are left in place.
- **Older `vinoWhisper` releases lack a built-in uninstaller:** Newer releases provide `vinowhisper-upgrade --uninstall`, which `vinoPilot` uses when present. For older releases, `vinoPilot` removes the `uv` tool, systemd units, tray/window files, menu entries, and models under `~/.local/share/vinowhisper/models` and `~/.cache/vinowhisper`, while keeping transcripts, sessions, and settings.
- **`vinoSpeak` installed from PyPI keeps its models on uninstall:** PyPI installs have neither a source checkout nor `vinospeak-upgrade` to run the uninstaller from. Models remain in `~/.cache/vinospeak`.
- **Uninstalling a source install moves its checkout to the trash** (using `gio`, `kioclient`, or `trash-put`), unless the repository has uncommitted changes, unpushed commits, or stashed work—in which case it is kept and the uninstall explains why. Stale binaries left in `~/.cargo/bin` by an earlier `cargo install` are also removed.
- **Detection for manually installed apps is best-effort:** An app with a binary in `~/.local/bin` but no `-upgrade` script, `uv` receipt, or Cargo record is reported as a source install.
- **A version of `dev` indicates a source build:** Release binaries (and Flatpaks) are stamped with their release tag, whereas local builds from a checkout report `dev`. `vinoPilot` treats a `dev` binary as a source install regardless of any `-upgrade` script beside it and does not offer release updates for it. `vinoAuthFace` has no install marker, so detection relies on its version alone. Older releases of other apps printed their previous release tag even when built from a checkout, so those still rely on install markers. `vinoPilot` follows the same rule for itself; `cargo install vinopilot` from crates.io counts as a release.
- **Non-interactive execution:** Installers receive no terminal and run with non-interactive defaults (`vinoWhisper` and `vinoSpeak` run with `--gui`).

## The steps

This section documents the catalog architecture for contributors. Every action is a sequence of steps, and each step is a shell script executed via `bash -c` either as the current user or as root via `pkexec`. Run `vinopilot steps APP ACTION` to inspect the exact scripts.

| File | Contents |
|---|---|
| `crates/core/src/catalog.rs` | Component definitions, supported install methods, and uninstall cleanup descriptions |
| `crates/core/src/catalog/scripts.rs` | Shared shell prelude prepended to every step, plus longer reusable scripts |
| `crates/core/src/catalog/steps.rs` | Per-component tables mapping each `(install method, action)` pair to its scripts |
| `crates/core/src/catalog/steps.snapshot` | Snapshot pinning the exact generated script for every combination |

Because these scripts form a contract with each app's installer, `every_script_matches_the_snapshot` fails on any modification. Bless intentional changes with `VINOPILOT_BLESS_STEPS=1 cargo test -p vinopilot-core` and review the snapshot diff carefully. Because the snapshot only verifies script text and not upstream installer compatibility, always run the real install, update, and uninstall as well.

Paths and settings are passed into scripts through environment variables (`$CHECKOUT`, `$WORKTREE`, `$OFF_BRANCH`, `$REPO_URL`, `$RELEASE`) rather than interpolated into shell strings. Because `pkexec` clears the environment, root steps receive them explicitly via `pkexec env NAME=value ...`, along with `SUDO_USER` so `vinoAuthFace`'s scripts can locate the desktop user.

### What each app's installer expects

| App | Installer details |
|---|---|
| `vinoWhisper` | `install.sh` runs `vinowhisper-setup` automatically on a first install. |
| `vinoPop` | `get.sh` unpacks the release archive and runs its bundled `install.sh`. |
| `vinoSearch` | The installer forwards `--uninstall` to the release's `scripts/install.sh`. |
| `vinoWatch` | `vinowatch-upgrade` is a copy of `install.sh` (POSIX `sh`). |
| `vinoOpenEffects` | Newer releases support `vinoopeneffects-upgrade --uninstall`. Older releases shipped the uninstaller only in Git, so `vinoPilot` runs it from the local checkout or a shallow clone. |
| `vinoAuthFace` | Runs as root. Source updates pull the checkout as the regular user so file ownership stays intact, then run `deploy.sh` as root. |
| `ovfetch` | Distributed as a standalone binary in GitHub releases and as a binary crate on crates.io. Installing from one source removes the other's copy so it does not shadow the active binary or confuse detection. |
| `vinoPilot` | `install.sh` builds from source when invoked inside a checkout. Its `.deb`, `.rpm`, and Flatpak packages are detected for updates/uninstalls but not offered as generic install targets, since no other component uses them. |
| `vinoUserInterface` | A Rust library rather than an end-user app; "installed" means a local Git checkout that compiles. |

### Packages and Flatpaks

Package steps (`vinoPilot`'s `.deb`/`.rpm` packages and app Flatpak bundles) include three shell helpers:

- `fetch ASSET`: Downloads the named release asset and verifies it against the release's `SHA256SUMS` (just as `install.sh` verifies tarballs), falling back to `ASSET.sha256` if `SHA256SUMS` omits that asset (`vinoWhisper` writes `SHA256SUMS` before building its Flatpak bundle) or does not exist (`vinoSearch`). A mismatched checksum in `SHA256SUMS` always fails.
- `as_root`: Wraps only the system package installation command in `pkexec`; downloading and checksum verification always run as the regular user.
- `scoped_flatpak`: Runs `flatpak` against whichever installation scope (`--user` or `--system`) owns the app. Fresh installs always target `--user` so no password is needed, matching the other install methods.

System-wide Flatpaks invoke `as_root` instead of relying on Flatpak's built-in polkit prompt. Because GUI actions run inside systemd's user manager outside the desktop session, Flatpak's polkit check cannot locate a password agent and fails with `"not allowed for user"`, whereas `pkexec` prompts normally.

Installing a Flatpak bundle with a newer commit replaces the existing deployment. Reinstalling the exact same commit causes Flatpak to return an error whether or not `--reinstall` is passed (Flatpak 1.14, checked 2026-11-09), which `vinoPilot` reports as already up to date. If the app window is currently open, Flatpak keeps the active deployment until the window closes and removes it afterward.

When uninstalling a Flatpak through `vinoPilot`, `vinoPilot` also removes the app's downloaded models (`~/.var/app/<app id>/data/<app>/models`) and cache (`~/.var/app/<app id>/cache`), matching native uninstalls while preserving the rest of `~/.var/app/<app id>` (transcripts, search indexes, and settings). Removing a Flatpak via Discover, GNOME Software, or `flatpak uninstall` does not perform this selective cleanup because Flatpak has no uninstall hook—models remain unless you choose "delete data" (`--delete-data`), which removes your user data as well.
