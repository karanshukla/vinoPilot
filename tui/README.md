# vinoPilot in a terminal

The [vinoPilot](https://github.com/karanshukla/vinopilot) window, for a terminal: a table of the vinoPilot apps with their version, how each is installed and whether there's something newer, the installer's output underneath, and one key per action.

It's a thin frontend. Everything it knows comes from the `vinopilot` command's `--json` output, and every action is a `vinopilot` run, so what the window does and what the table does can't drift apart. If the command isn't there, the first screen offers to fetch it from the latest release with the same hash-checked installer the README gives.

## Install

```sh
uv tool install vinopilot
vinopilot-tui
```

or `pipx install vinopilot`. The command is `vinopilot-tui`, not `vinopilot`: that name is the Rust command this drives, and `uv tool` would otherwise overwrite it in `~/.local/bin`.

## Keys

| Key | Does |
|---|---|
| `c` | Check for updates (the only thing that goes online, besides the actions themselves) |
| `i` | Install the selected app; asks which way when there's more than one |
| `u` | Update it, the way it's installed |
| `a` | Update everything that has an update, vinoPilot included |
| `x` | Uninstall it, after a yes |
| `s` | Show the shell an action would run, without running it |
| `r` | Change where source checkouts go |
| `Esc` | Stop the running action |
| `q` | Quit |

After an action the table is rescanned: offline, or online again if you had checked for updates.

## Limits

- vinoAuthFace's steps run through `pkexec`. On a desktop the polkit agent asks for your password in a dialog. Over ssh with no agent, pkexec has no way to ask and the step fails.
- Stopping an action ends the `vinopilot` run, but a `pkexec` child is root's and runs on.
- `VINOPILOT_BIN=/path/to/vinopilot` picks the command to drive, for a checkout build.
