#!/usr/bin/env bash
# Installs a packed dist/'s Flatpak bundle for this user and runs both
# commands through it. They start on the host (packaging/flatpak/host-run.sh),
# so this needs a session bus: dbus-run-session gives a headless runner one.
#
#   tests/flatpak-smoke.sh dist 0.1.0
set -euo pipefail
dist="$(cd "${1:?dist dir}" && pwd)" version="${2:?version}"
id=io.github.karanshukla.vinopilot

(cd "$dist" && sha256sum -c --strict SHA256SUMS)
flatpak install --user --noninteractive --reinstall "$dist/vinopilot-x86_64.flatpak"
trap 'flatpak uninstall --user --noninteractive "$id" > /dev/null' EXIT
for bin in vinopilot vinopilot-gui; do
    out="$(dbus-run-session -- flatpak run --command="$bin" "$id" --version)"
    test "$out" = "$bin $version" || { echo "$bin in the Flatpak said: $out" >&2; exit 1; }
done
# The bundle's binaries are the tarball's, byte for byte.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; flatpak uninstall --user --noninteractive "$id" > /dev/null' EXIT
tar -xzf "$dist/vinopilot-x86_64-linux.tar.gz" -C "$tmp"
files="$(flatpak info --user --show-location "$id")/files"
for bin in vinopilot vinopilot-gui; do
    cmp "$tmp/vinopilot/$bin" "$files/libexec/vinopilot/$bin"
done
flatpak info --show-permissions "$id" | tr -d '\n' | grep -qx '\[Session Bus Policy\]org.freedesktop.Flatpak=talk' ||
    { echo "the Flatpak asks for more than org.freedesktop.Flatpak:" >&2; flatpak info --show-permissions "$id" >&2; exit 1; }
echo "ok - the Flatpak runs ($version)"
