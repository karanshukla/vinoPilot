# Contributing

The most useful thing you can bring is a report from a machine set up differently from mine: an app installed some way vinoPilot misreads, or an install, update or uninstall that fails. Attach `vinopilot list --json` and the installer's output.

## Setup

Plain cargo, nothing else needed. The tests use throwaway homes and a `file://` fake release; none of them install anything for real.

```bash
cargo test --workspace
cargo run -p vinopilot -- list
cargo run -p vinopilot-gui
```

## Before you push

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo build -p vinopilot -p vinopilot-gui && tests/install-e2e.sh target/debug
```

CI also builds the release tarball the way `release.yml` does and runs it.

- `crates/cli/tests/cli.rs` is the `--json` contract. Change it when you change the shape, and say so in the changelog and `docs/json-api.md`.
- Changed the window: regenerate the pictures with `cargo test -p vinopilot-gui --test screenshots -- --ignored` (needs a GPU or lavapipe) and commit `docs/images`.
- Changed an app's steps in `crates/core/src/catalog.rs`: run that install, update and uninstall for real and say so in the PR. The tests only check the scripts parse.
- Keep code comments to the *why*, one or two lines.
- Commits are conventional (`feat:`, `fix:`, `docs:`, ...); the changelog is generated from them with `git cliff`.
- Every Actions `uses:` is pinned to a commit SHA with a version comment.
- A `deny.toml` advisory ignore needs a written reason next to it.

## Adding a vinoPilot app

1. A variant: if it's in vino-user-interface's `App`, it comes with `Id::App`; otherwise add one to `Id` with its name, blurb and repo.
2. Its steps in `catalog::steps` for every channel `channels()` offers (a test fails until they exist).
3. How to tell it's installed, in `detect::local`.
4. A card in the window's `CARDS` (keep the grid full: it's 3 by 3), and a row in the README table.
