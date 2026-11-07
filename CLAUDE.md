# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

vinoPilot: installs, updates and uninstalls the vinoPilot apps (vinoWhisper, vinoAuthFace, vinoOpenEffects, vinoPop, vinoSpeak, vinoSearch, vinoWatch), ovfetch, the vinoUserInterface library and itself, from their releases, a source checkout, PyPI or crates.io. User docs are the README; don't duplicate them here.

## Layout

A cargo workspace, planned to become a monorepo with a Python TUI beside it (driving `vinopilot --json`, not reimplementing the catalog). Keep repo-level tooling language-neutral.

- `crates/core` (`vinopilot-core`): no UI, no egui.
  - `catalog.rs`: the components and, per (component, channel, action), the shell steps. Each step is a script run with `bash -c` (or `pkexec env ... bash -c` when `root`), with a shared prelude (`release_run`, `checkout`, `need_*`). Paths reach scripts as `$CHECKOUT`, `$REPO_URL`, `$RELEASE` env vars, never formatted in.
  - `detect.rs`: how each is installed, by looking: `<app>-upgrade` markers in `~/.local/bin`, uv receipts, `~/.cargo/.crates.toml`, `--version` (2 s timeout).
  - `remote.rs`: GitHub latest release, PyPI, crates.io, `git fetch` + behind count. The only module that goes online.
  - `status.rs`: installed + latest into "update available", and the `--json` component shape.
  - `job.rs`: runs steps in a background thread, streams lines through a `Notify` callback.
  - `lib.rs` `command()`: every child process goes through it, for the `PATH` fix-up (`~/.local/bin`, `~/.cargo/bin`).
- `crates/cli` (`vinopilot`): hand-rolled args, `--json` is the API (docs/json-api.md, pinned by tests/cli.rs).
- `crates/gui` (`vinopilot-gui`): lib (`Pilot`) + thin main. A 3x3 grid (`CARDS`: seven apps, ovfetch, vinoUserInterface; one column when three don't fit), log panel at the bottom. vinoPilot has no card: its version and self-update are in the header, and Update all skips it.

## Commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check
cargo build -p vinopilot -p vinopilot-gui && tests/install-e2e.sh target/debug
cargo test -p vinopilot-gui --test screenshots -- --ignored   # regenerates docs/images
```

## Gotchas

- **Each app's installer is the source of truth.** Read the app's `install.sh` (and `scripts/install.sh`, `uninstall.sh`) before changing its steps; flags differ per app (vinoWatch's is POSIX `sh`, vinoAuthFace's needs root, vinoOpenEffects' uninstaller only ships in the repo, vinoWhisper has none). `every_script_parses` only checks syntax.
- **pkexec clears the environment.** Root steps get their vars through `pkexec env`, plus `SUDO_USER` so vinoAuthFace's `deploy.sh` builds and finds the desktop user as itself. A pkexec child is root's: `Job::stop` can't kill it.
- **`core` forbids unsafe**, so no `env::set_var`; that's why `command()` sets `PATH` per child.
- **The release CLI is static musl, the window is glibc.** eframe dlopens libEGL/wayland/X11, which static musl can't. The window is built on ubuntu-22.04 for an old glibc floor.
- **vino-user-interface's `Updater` is keyed on its `App` enum**, which has no vinoPilot, so vinoPilot updates itself as a catalog component (`Id::Pilot`) instead.
- **`Pilot::ui` doesn't call `theme::follow_desktop`**; the eframe `App` impl does. Otherwise the screenshot test's light render comes out dark.
- **Nothing goes online until asked.** `list` and the window's startup scan are local; only `check` / Check for updates call `remote`.

## CI and repo rules

- Actions pinned to commit SHAs with a version comment; Dependabot with a 7-day cooldown, nothing auto-merges.
- Conventional commits; CHANGELOG.md from `git cliff`. A release tag needs a matching `## [x.y.z]` section and every crate at that version.
- Releases: build (no write token), attest, publish, then crates.io by trusted publishing behind the `crates-io` environment's approval, core first.
