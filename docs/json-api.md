# JSON API

`vinopilot --json` defines the machine-readable interface between `vinoPilot`'s Rust core and external frontends (such as the Python TUI in `tui/`). Any breaking change requires a changelog entry. The contract is verified by `crates/cli/tests/cli.rs`, and each rule below cites its governing tests.

## `list` and `check`

`vinopilot list --json` (offline) and `vinopilot check --json` (online) output a single JSON document where every component object includes all fields (pinned by `crates/cli/tests/cli.rs::list_json_names_every_component`, `crates/cli/tests/cli.rs::every_component_has_every_field`):

```json
{
  "vinopilot": "0.1.0",
  "source_root": "/home/you/Development",
  "components": [
    {
      "id": "vinosearch",
      "name": "vinoSearch",
      "blurb": "Find files by what they're about",
      "repo": "karanshukla/vinoSearch",
      "channels": ["release", "source"],
      "registry": null,
      "needs_root": false,
      "installed": { "channel": "release", "version": "8.0.0" },
      "checkout": { "path": "/home/you/Development/vinoSearch", "describe": "v8-9-g1a2b3c4" },
      "system_package": null,
      "shadowed": [],
      "latest": {
        "release": { "value": "v8" },
        "package": null,
        "behind": { "value": 9 }
      },
      "update": { "state": "up_to_date" }
    }
  ]
}
```

| Field | Description |
|---|---|
| `vinopilot` (top-level) | Version of this `vinopilot` binary: the release version string, or `"dev"` for an unstamped local build from a checkout (pinned by `crates/cli/tests/cli.rs::an_unstamped_build_reports_dev`, `crates/cli/tests/cli.rs::a_release_build_reports_its_version`). |
| `id` | CLI identifier accepted by commands (e.g., `vinowhisper`, `ovfetch`, `vinouserinterface`, `vinopilot`). |
| `channels` | Supported install methods ordered from most preferred to least: `release`, `source`, `package` (PyPI or crates.io), and `flatpak` (GitHub release Flatpak bundle for `vinowhisper` and `vinosearch`) (pinned by `crates/cli/tests/cli.rs::channels_are_listed_best_first`). |
| `registry` | `"pypi"`, `"crates.io"`, or `null`. Present on every component with a `package` channel, as well as `vinouserinterface` (published on crates.io as a library, though installed only from `source`) (pinned by `crates/cli/tests/cli.rs::registry_names_where_a_package_is_published`). |
| `needs_root` | `true` if the component's actions run through `pkexec`. |
| `installed` | `null` if not installed, or `{"channel": ..., "version": ...}`. `version` is `null` if the binary did not report one, or `"dev"` for a source-checkout build; any `"dev"` binary is always classified under `source` regardless of any adjacent release marker and is not offered version-based release updates (pinned by `crates/cli/tests/cli.rs::a_dev_binary_beside_a_release_marker_is_a_source_build_with_its_describe`). `channel` is one of `channels`—or for `vinopilot` itself, also `deb`, `rpm`, or `flatpak` (used for `update` and `uninstall`, not offered by `install`). For Flatpak installs, `version` comes from the newest release in its AppStream metainfo (or `null` if absent). |
| `checkout` | `null`, or the Git checkout found in `source_root`. Its `describe` field is intended to display alongside a `"dev"` version. |
| `system_package` | `null`, or system packages (`rpm` or `deb`) owning the component's binaries in `/usr/bin`, e.g. `{"kind": "rpm", "packages": ["vinoopeneffects"], "remove": "sudo dnf remove vinoopeneffects"}`. `vinoPilot` does not remove system packages automatically; `remove` provides the command for the user. |
| `shadowed` | Commands present in both `~/.local/bin` and `~/.cargo/bin` (such as a release binary alongside a Cargo build): `[{"command": "vinosearch", "runs": "/home/you/.local/bin/vinosearch", "hidden": "/home/you/.cargo/bin/vinosearch"}]`, where `runs` is first on `PATH`. Empty when only one copy exists (pinned by `crates/cli/tests/cli.rs::a_command_in_both_bins_is_listed_as_shadowed`). |
| `latest` | Included only in `check --json` output (pinned by `crates/cli/tests/cli.rs::list_has_no_latest_because_it_stays_offline`). Each lookup value is `null` (not applicable), `{"value": ...}`, or `{"error": "sentence"}`. |
| `update` | `{"state": "available", "to": "v1.3.1"}`, `{"state": "up_to_date"}`, or `{"state": "unknown", "reason": null or "sentence"}`, evaluated against the source matching the current install method. |

## `install`, `update`, and `uninstall`

Action commands stream one JSON object per line as the installer executes:

```json
{"event":"line","text":"== Downloading the latest release from https://..."}
{"event":"line","text":"checksum verified"}
{"event":"done","app":"vinowatch","channel":"release","result":{"outcome":"done"}}
```

`result` is one of:

- `{"outcome": "done"}` (exit code `0`)
- `{"outcome": "stopped"}` (exit code `1`)
- `{"outcome": "failed", "reason": "the installer's ✗ line, or its last"}` (exit code `1`)

These streaming events and exit codes are verified by integration tests (pinned by `crates/cli/tests/cli.rs::an_action_prints_line_events_then_one_done_event`, `crates/cli/tests/cli.rs::a_failed_action_gives_the_installers_cross_line_as_its_reason_and_exits_1`).

When run without a terminal, `uninstall` requires `--yes`; otherwise it exits with status `2` (pinned by `crates/cli/tests/cli.rs::uninstall_without_yes_or_a_terminal_is_a_usage_error`).

## Other commands

- `vinopilot steps APP ACTION [--via METHOD]`: Prints the exact shell script an action would execute without running it (pinned by `crates/cli/tests/cli.rs::steps_prints_the_script_without_running_it`).
- `vinopilot config --json`: Prints `{"source_root": "..."}`. `vinopilot config source_root DIR` updates the value in `~/.config/vinopilot/config`, which both `list` and the GUI read (pinned by `crates/cli/tests/cli.rs::config_json_is_the_source_root_that_list_uses`).
