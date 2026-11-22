# Installing, updating, and uninstalling vinoPilot

The [README](../README.md#install) covers the three packaged install methods: Flatpak, `.deb`, and `.rpm`. Each package contains both `vinopilot` (the CLI) and `vinopilot-gui` (the desktop window) and adds vinoPilot to your application menu. You can also install the downloaded `.flatpak` bundle by opening it in Discover or GNOME Software.

## Flatpak host permissions

Because `vinoPilot` manages apps on your system by invoking your host `bash`, `git`, `uv`, `cargo`, `flatpak`, and `pkexec` tools, its Flatpak package is not sandboxed. Its sole permission is host execution (`org.freedesktop.Flatpak`), and both the GUI and `flatpak run --command=vinopilot io.github.karanshukla.vinopilot` execute on the host.

Apps installed by the `vinoPilot` Flatpak are installed into your home directory through their own installers (the same as every other `vinoPilot` install method), or as their own Flatpaks where supported.

## Other install methods

### One-line installer (no sudo)

Installs both `vinopilot` and `vinopilot-gui` into `~/.local/bin`:

```sh
curl -fsSL https://github.com/karanshukla/vinopilot/releases/latest/download/install.sh | bash
```

| Option | Effect |
|---|---|
| `\| bash -s -- --no-gui` | Installs the CLI only (skips `vinopilot-gui`) |
| `\| bash -s -- --uninstall` | Uninstalls `vinoPilot` completely |

### crates.io or source checkout

```sh
cargo install --locked vinopilot vinopilot-gui && vinopilot-gui --install-desktop
./install.sh                     # inside a Git checkout: builds and installs locally
```

`--install-desktop` installs the application menu entry and icon (`install.sh` runs this automatically). `--uninstall-desktop` removes them.

### Terminal UI (PyPI)

The terminal UI is published on PyPI: run `uv tool install vinopilot` to install `vinopilot-tui`. If the Rust `vinopilot` CLI is not installed yet, `vinopilot-tui` offers to fetch it automatically, and updating or uninstalling `vinoPilot` keeps `vinopilot-tui` in sync. See [tui/README.md](../tui/README.md).

Prebuilt release binaries are available for Linux `x86_64` only, matching the apps `vinoPilot` installs.

## Updating

`vinoPilot` updates itself using whichever method originally installed it. Clicking the version button in the GUI header (or running `vinopilot update vinopilot`) downloads the new release's Flatpak, `.deb`, or `.rpm`, verifies it against `SHA256SUMS`, and passes it to `flatpak`, `apt`, or `dnf`. `.deb` and `.rpm` updates prompt for your password for the package installation step.

Because there is no hosted Flatpak, APT, or DNF repository backing these packages, your desktop software center and system updater will not automatically notify you of new releases.

In the GUI, the header version button changes to **update to vX** when a newer release is available, and to **Restart to use vX** once installed. The **ⓘ** button beside it displays how `vinoPilot` was installed, its source checkout (if any), and the paths and versions of the CLI, GUI, and TUI.

## Uninstalling

The GUI does not provide an Uninstall button for `vinoPilot` itself. Remove it with the command matching your install method, or click Remove in your software center:

```sh
flatpak uninstall --user io.github.karanshukla.vinopilot                       # Flatpak
sudo apt remove vinopilot                                                      # .deb
sudo dnf remove vinopilot                                                      # .rpm
vinopilot-gui --uninstall-desktop && cargo uninstall vinopilot vinopilot-gui   # crates.io
./install.sh --uninstall                                                       # Git checkout
```

Alternatively, `vinopilot uninstall vinopilot` works for every install method. When running from the Flatpak, use:

```sh
flatpak run --command=vinopilot io.github.karanshukla.vinopilot uninstall vinopilot
```

Apps previously installed by `vinoPilot` remain installed; uninstall them first if you want them removed as well.
