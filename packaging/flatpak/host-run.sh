#!/bin/sh
# /app/bin/vinopilot and /app/bin/vinopilot-gui in the Flatpak.
#
# vinoPilot installs apps on the host: into ~/.local, with the host's bash,
# git, uv, cargo and pkexec. None of that works from inside a sandbox, so
# this runs the real binary (the same one the tarball ships) on the host,
# from where Flatpak keeps this app's files there.
set -eu
app="$(sed -n 's/^app-path=//p' /.flatpak-info)"
[ -n "$app" ] || { echo "vinopilot: no app-path in /.flatpak-info" >&2; exit 1; }
# --watch-bus: closing the sandbox takes the host process with it.
exec flatpak-spawn --host --watch-bus "$app/libexec/vinopilot/$(basename "$0")" "$@"
