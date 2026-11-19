# Contributing

Reports from machines with different setups are especially helpful—for example, an existing app installation that `vinoPilot` detects incorrectly, or an install, update, or uninstall failure. Please attach the output of `vinopilot list --json` along with the installer log.

## Development setup

The Rust workspace uses a standard Cargo setup with no extra prerequisites. Tests run against temporary home directories and a `file://` mock release, so nothing is installed to your system.

```bash
cargo test --workspace
cargo run -p vinopilot -- list
cargo run -p vinopilot-gui
```

## Before pushing

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo build -p vinopilot -p vinopilot-gui && tests/install-e2e.sh target/debug
cd tui && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen ruff format --check .
```

CI also builds the release tarball, `.deb`, `.rpm`, and Flatpak using the same steps as `release.yml`, installs them in containers (`.deb` on Debian, `.rpm` on Fedora), and runs smoke checks. Pull request packages are attached to the workflow run as the `dist` artifact.

## Guidelines

- **Terminal UI (`tui/`):** Requires [uv](https://docs.astral.sh/uv/). Run `cd tui && uv sync` to create the environment, and `VINOPILOT_BIN=../target/debug/vinopilot uv run vinopilot-tui` to run against your local CLI build. Its test suite drives a mock `vinopilot`, so any `--json` schema change must also update `tui/tests/fake_vinopilot.py`.
- **JSON contract:** `crates/cli/tests/cli.rs` pins the `--json` schema. Update it whenever the format changes and document the change in `CHANGELOG.md` and `docs/json-api.md`.
- **GUI screenshots:** When changing the desktop window, regenerate the screenshots with `cargo test -p vinopilot-gui --test screenshots -- --ignored` (requires a GPU or `lavapipe`) and commit `docs/images`.
- **Catalog steps:** When changing an app's scripts under `crates/core/src/catalog/`, update the snapshot with `VINOPILOT_BLESS_STEPS=1 cargo test -p vinopilot-core`, test the real install, update, and uninstall end-to-end, and note that in your PR. Automated tests only verify script syntax and catch accidental changes. See [docs/how-it-works.md](docs/how-it-works.md#the-steps) for the catalog layout.
- **Comments:** Prefer clear naming, helper extraction, or paired tests over comments, and cite the governing test in any remaining comments. See [docs/comment-style.md](docs/comment-style.md); `crates/cli/tests/doc_links.rs` and `tui/tests/test_doc_links.py` verify these citations.
- **Commits:** Use Conventional Commits (`feat:`, `fix:`, `docs:`, etc.); `CHANGELOG.md` is generated with `git cliff`.
- **CI actions:** Pin every GitHub Actions `uses:` reference to a full commit SHA with a version comment.
- **Dependency advisories:** Include a written reason beside any advisory ignore in `deny.toml`.

## Adding a vinoPilot app

1. Add a component identifier: if the app is already in `vino-user-interface`'s `App` enum, it maps through `Id::App`; otherwise add a new variant to `Id` with its name, blurb, and repository.
2. Define its scripts in `catalog::steps` for every channel returned by `channels()` (a test fails until all combinations are defined).
3. Add local installation detection in `detect::local`.
4. Add a card to the GUI's `CARDS` array (keeping the 3×3 grid complete) and add a row to the table in `README.md`.
