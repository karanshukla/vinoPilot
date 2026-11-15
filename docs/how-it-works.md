# How vinoPilot works

vinoPilot doesn't install anything itself. Each app has its own hash-checked installer, and vinoPilot knows which one to run for each way in.

## The ways in

| Way in | Install | Update | `--via` |
|---|---|---|---|
| Release | the app's one-line installer from its latest GitHub release, which checks the download against the release's `SHA256SUMS` | the `<app>-upgrade` copy each installer keeps | `release` |
| Source | clones the repo into your source folder, then runs the checkout's own install script | `git pull --ff-only`, then the same again | `source` |
| Package | `uv tool` for PyPI, `cargo install` for crates.io | the same tools | `pypi`, `crates` |
| Flatpak (vinoSearch) | the release's Flatpak bundle, checked against the release's checksums and installed with `flatpak install --user` | downloads the new release's bundle over it | `flatpak` |

The source folder is `~/Development` if you have one, else `~/src`. Change it in the window or with `vinopilot config source_root DIR`.

Uninstalling a Flatpak leaves the app's data in `~/.var/app`.

### A checkout on another branch

A checkout that isn't on its default branch is never pulled. What happens instead depends on the app:

| Apps | What vinoPilot does |
|---|---|
| Most | Checks out the default branch in `~/.local/share/vinopilot/worktrees/<repo>`, installs from that, then removes it. Your checkout is left alone. |
| vinoWhisper, vinoPop, vinoUserInterface | These run from their checkout, so the checkout itself switches to the default branch, with uncommitted changes stashed first. The log says how to get back. |

A worktree for those three would be a second copy the app quietly runs from, which is why they switch instead.

After a worktree install, the commit it built is recorded in the checkout as `refs/vinopilot/installed`. `vinopilot check` counts how far behind that commit is, not your branch.

## Detection

vinoPilot works out how each app is installed by looking, not by keeping its own records:

- the `-upgrade` scripts the release installers leave in `~/.local/bin`
- `uv`'s tool receipts
- cargo's `~/.cargo/.crates.toml`
- each binary's `--version` (given two seconds to answer)

So it also sees apps you installed by hand before vinoPilot existed.

A copy in your home runs first. Without one, vinoPilot is found as its .deb, .rpm or Flatpak, and an app as its Flatpak. A Flatpak app only runs in its sandbox, so its version comes from its metainfo's newest release.

## Going online

Nothing goes online until you press **Check for updates** (or run `vinopilot check`). Then it asks GitHub, PyPI and crates.io for the newest versions and runs `git fetch` in each checkout.

## Limits

- **Uninstalling an app removes its downloaded models** and keeps your own data and settings (transcripts, indexes, face templates). Each app's own uninstaller decides what that covers; vinoPilot runs it. Shared folders such as `~/.cache/huggingface` and the runtime ovfetch fetched stay.
- **Only newer vinoWhisper releases have an uninstaller** (`vinowhisper-upgrade --uninstall`), and vinoPilot uses it when it's there. Otherwise vinoPilot's own undoes what the installer writes (the `uv` tool, the units, the tray and window, the menu entries) and deletes the models in `~/.local/share/vinowhisper/models` and `~/.cache/vinowhisper`; transcripts, sessions and settings stay.
- **A vinoSpeak installed from PyPI keeps its models** when uninstalled: there's no checkout or `vinospeak-upgrade` to run its uninstaller from. They're in `~/.cache/vinospeak`.
- **Uninstalling a source install moves its checkout to the trash** (with `gio`, `kioclient` or `trash-put`), unless it has uncommitted, unpushed or stashed work: then it's kept and the uninstall says why. Copies an old `cargo install` left in `~/.cargo/bin` go too.
- **Detection is a best guess for hand installs.** An app with no `-upgrade` script, no uv receipt and no cargo record but a binary in `~/.local/bin` shows as a source install.
- **vinoAuthFace's channel comes from its version.** Release builds are stamped with their tag, checkouts say `dev`.
- Installers that want to ask you something get no terminal; each runs with its non-interactive defaults (vinoWhisper and vinoSpeak with `--gui`).

## The steps

For contributors. Every action is a list of steps, and a step is one shell script run with `bash -c`, as you or as root through `pkexec`. `vinopilot steps APP ACTION` prints them.

| File | Holds |
|---|---|
| `crates/core/src/catalog.rs` | the components, their ways in, and what an uninstall says it removes |
| `crates/core/src/catalog/scripts.rs` | the shell itself: the prelude every step starts with, and the longer scripts |
| `crates/core/src/catalog/steps.rs` | one table per component: which script each (way in, action) runs |
| `crates/core/src/catalog/steps.snapshot` | every script of every combination, pinned |

The scripts are a contract with each app's installer, so `every_script_matches_the_snapshot` fails on any change to one. A deliberate change is blessed with `VINOPILOT_BLESS_STEPS=1 cargo test -p vinopilot-core`, and the snapshot's diff is what to review. The snapshot proves the bytes; it doesn't prove the app's installer still takes them, so run the install, update and uninstall for real too.

Paths and settings reach a script as environment variables (`$CHECKOUT`, `$WORKTREE`, `$OFF_BRANCH`, `$REPO_URL`, `$RELEASE`), never formatted into it. `pkexec` clears the environment, so a root step gets them as `pkexec env NAME=value ...` arguments, plus `SUDO_USER` so vinoAuthFace's scripts find the desktop user.

### What each app's installer expects

| App | Notes |
|---|---|
| vinoWhisper | `install.sh` runs `vinowhisper-setup` on a first install. |
| vinoPop | `get.sh` unpacks the release and runs its `install.sh`. |
| vinoSearch | The installer passes `--uninstall` through to the release's `scripts/install.sh`. |
| vinoWatch | `vinowatch-upgrade` is a copy of `install.sh`, which is POSIX `sh`. |
| vinoOpenEffects | Its uninstaller is `vinoopeneffects-upgrade --uninstall` in releases that have it. Before that it shipped only in the repo, so vinoPilot uses the checkout's, or a shallow clone's. |
| vinoAuthFace | Root throughout. The checkout is pulled as the user so it stays the user's, then `deploy.sh` runs as root. |
| ovfetch | A bare binary in the release, a bin crate on crates.io. Each install drops the other channel's copy, which would otherwise shadow it or be what detection finds. |
| vinoPilot | Its `install.sh` builds from a checkout when run from one. Its .deb, .rpm and Flatpak are found, not offered: no other component has them. |
| vinoUserInterface | A library, so "installed" means a checkout that builds. |

### Packages and Flatpaks

A package step (vinoPilot's .deb and .rpm, and any Flatpak bundle) starts with three helpers:

- `fetch ASSET` downloads that file of the release and checks it against the release's `SHA256SUMS`, as install.sh does the tarball, or against its own `ASSET.sha256` in a release without one (vinoSearch's).
- `as_root` is `pkexec` for the one command that installs a system package. The download and the check stay the user's.
- `scoped_flatpak` runs `flatpak` where the app is installed. A first install goes to the user's installation: no password, like the other ways in.

A system-wide Flatpak goes through `as_root`, not Flatpak's own polkit prompt. Flatpak's polkit check finds no password agent for a process outside the desktop's session, and the window's steps run in systemd's user manager, so it fails with "not allowed for user" where `pkexec` asks.

A bundle with a newer commit installs over the old one. The same commit is an error to Flatpak, `--reinstall` or not (Flatpak 1.14, checked 2026-11-09), and vinoPilot reports it as nothing to do. A running window keeps its files until it closes: Flatpak removes a deployment that's in use afterwards.

Uninstalling a Flatpak leaves `~/.var/app/<app id>` alone. What the app keeps there is the user's to delete.
