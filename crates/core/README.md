# vinopilot-core

The core library behind [vinoPilot](https://github.com/karanshukla/vinopilot), providing all non-UI functionality:

- `catalog`: Install, update, and uninstall steps for each app across releases, source checkouts, PyPI, crates.io, and Flatpaks.
- `detect`: Local detection of which apps are installed and how.
- `remote`: Remote version lookups from each installation source.
- `job`: Action execution with streaming output.

Both the `vinopilot` CLI and the `vinopilot-gui` desktop app are built on this crate. Frontends written in other languages should invoke `vinopilot --json` instead; see [docs/json-api.md](https://github.com/karanshukla/vinopilot/blob/main/docs/json-api.md).
