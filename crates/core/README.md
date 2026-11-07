# vinopilot-core

What [vinoPilot](https://github.com/karanshukla/vinopilot) knows about the vinoPilot apps, with no UI: how each one installs, updates and uninstalls from its release, a source checkout, PyPI or crates.io (`catalog`); how each is installed on this machine (`detect`); the newest version from each source (`remote`); and running an action with its output streamed (`job`).

The `vinopilot` command and the `vinopilot-gui` window are built on it. Frontends in other languages should drive `vinopilot --json` instead; see [docs/json-api.md](https://github.com/karanshukla/vinopilot/blob/main/docs/json-api.md).
