# Installing, updating and uninstalling vinoPilot

The [README](../README.md#install) has the three packaged ways in: the Flatpak, the .deb and the .rpm. Each holds `vinopilot` (the command) and `vinopilot-gui` (the window), and adds vinoPilot to your app menu. Opening the downloaded Flatpak in Discover or GNOME Software installs it too.

## The Flatpak isn't sandboxed

vinoPilot installs apps on your system with your own `bash`, `git`, `uv`, `cargo`, `flatpak` and `pkexec`. So the Flatpak's only permission is the one to run on the host (`org.freedesktop.Flatpak`), and the window and `flatpak run --command=vinopilot io.github.karanshukla.vinopilot` run there.

The apps it installs go into your home through their own installers, as with every other way in, or as their own Flatpaks where they have one.

## Other ways in

The one-line installer puts the same two binaries in `~/.local/bin`, no sudo:

```sh
curl -fsSL https://github.com/karanshukla/vinopilot/releases/latest/download/install.sh | bash
```

| Variant | Does |
|---|---|
| `\| bash -s -- --no-gui` | skips the window |
| `\| bash -s -- --uninstall` | removes it all |

From crates.io, or a checkout:

```sh
cargo install --locked vinopilot vinopilot-gui && vinopilot-gui --install-desktop
./install.sh                     # in a checkout: builds, then installs as above
```

`--install-desktop` adds the app menu entry and icon (install.sh does it for you). `--uninstall-desktop` takes them out.

The terminal UI is on PyPI: `uv tool install vinopilot` gives `vinopilot-tui`. It offers to fetch the command when it isn't there, and updating or uninstalling vinoPilot takes it along. See [tui/README.md](../tui/README.md).

x86_64 only for the release binaries, same as the apps it installs.

## Updating

vinoPilot updates itself the way it was installed. The version button in the header (or `vinopilot update vinopilot`) downloads the new release's Flatpak, .deb or .rpm, checks it against the release's `SHA256SUMS`, and hands it to `flatpak`, `apt` or `dnf`. The .deb and .rpm ask for your password for that one command.

There's no Flatpak, apt or dnf repository behind them, so your software centre and system updater don't see new versions.

In the window, the version button turns into **update to vX** when there is a newer vinoPilot, then **Restart into vX** once that's installed. The ⓘ beside it shows how vinoPilot is installed, its checkout, and where the command, the window and the terminal UI are, with their versions.

## Uninstalling

The window has no Uninstall for vinoPilot itself. Remove it the way you installed it, or with Remove in your software centre:

```sh
flatpak uninstall --user io.github.karanshukla.vinopilot   # the Flatpak
sudo apt remove vinopilot                                  # the .deb
sudo dnf remove vinopilot                                  # the .rpm
vinopilot-gui --uninstall-desktop && cargo uninstall vinopilot vinopilot-gui   # crates.io
./install.sh --uninstall                                   # in a checkout
```

`vinopilot uninstall vinopilot` works for every way in. From the Flatpak that is `flatpak run --command=vinopilot io.github.karanshukla.vinopilot uninstall vinopilot`.

The apps vinoPilot installed stay. Uninstall those first if you want them gone.
