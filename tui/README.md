# vinoPilot TUI

A terminal user interface for [vinoPilot](https://github.com/karanshukla/vinopilot). It displays a table of the `vinoPilot` apps with their installed versions, installation methods, and update status, streams installer output underneath, and maps each action to a single key.

`vinopilot-tui` is a thin frontend over the Rust `vinopilot` CLI: all state comes from `vinopilot --json` and every action executes `vinopilot`, keeping behavior identical to the GUI. If `vinopilot` is not yet installed, the startup screen offers to download and install it from the latest release using the standard checksum-verified installer.

## Install

```sh
uv tool install vinopilot
vinopilot-tui
```

You can also install with `pipx install vinopilot`. The command is named `vinopilot-tui` rather than `vinopilot` so it does not collide with the Rust `vinopilot` binary in `~/.local/bin`.

## Keybindings

| Key | Action |
|---|---|
| `c` | Check for updates (the only network lookup aside from running install/update actions) |
| `i` | Install the selected app (prompts for an install method when multiple are available) |
| `u` | Update the selected app using its current install method |
| `a` | Update all apps with available updates, including `vinoPilot` itself |
| `p` | Check for a newer `vinoPilot` release (shown in the status line), update it, and restart |
| `x` | Uninstall the selected app (asks for confirmation) |
| `s` | Preview the shell script an action would run without executing it |
| `r` | Change the source checkout directory |
| `Esc` | Stop the currently running action |
| `q` | Quit |

After an action completes, the table refreshes automatically (offline by default, or online if you previously ran an update check).

## Limitations and configuration

- `vinoAuthFace` steps run via `pkexec`. On a desktop session, your polkit agent prompts for your password in a dialog. Over SSH without a polkit agent, `pkexec` cannot prompt and the action fails.
- Pressing `Esc` stops the `vinopilot` process, but a child process already running as root under `pkexec` continues to completion.
- Set `VINOPILOT_BIN=/path/to/vinopilot` to target a specific CLI binary (for example, a local Cargo build).
