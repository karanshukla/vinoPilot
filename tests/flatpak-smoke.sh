#!/usr/bin/env bash
# Installs a packed dist/'s Flatpak bundle for this user, runs both commands
# through it, and has it update itself from dist/ as the release. They start on the host (packaging/flatpak/host-run.sh),
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
# Its own update, the way host-run.sh starts it: the binary, on the host.
VINOPILOT_RELEASE_BASE="file://$dist" "$files/libexec/vinopilot/vinopilot" list --json | tr -d ' \n' |
    grep -q "\"installed\":{\"channel\":\"flatpak\",\"version\":\"$version\"}" ||
    { echo "vinopilot does not see its own Flatpak" >&2; exit 1; }
# The same bundle again is nothing to do, and not a failure.
VINOPILOT_RELEASE_BASE="file://$dist" "$files/libexec/vinopilot/vinopilot" update vinopilot | tee "$tmp/update"
grep -qx "checksum verified" "$tmp/update"
# A newer one, the same files in a new commit, replaces it.
ostree init --repo="$tmp/repo" --mode=archive
flatpak build-import-bundle "$tmp/repo" "$dist/vinopilot-x86_64.flatpak"
flatpak build-commit-from --force --src-ref="app/$id/x86_64/master" "$tmp/repo" "app/$id/x86_64/master"
mkdir "$tmp/next"
flatpak build-bundle "$tmp/repo" "$tmp/next/vinopilot-x86_64.flatpak" "$id"
(cd "$tmp/next" && sha256sum vinopilot-x86_64.flatpak > SHA256SUMS)
before="$(flatpak info --user --show-commit "$id")"
VINOPILOT_RELEASE_BASE="file://$tmp/next" "$files/libexec/vinopilot/vinopilot" update vinopilot
after="$(flatpak info --user --show-commit "$id")"
test "$before" != "$after" || { echo "the Flatpak's update left commit $before installed" >&2; exit 1; }
out="$(dbus-run-session -- flatpak run --command=vinopilot "$id" --version)"
test "$out" = "vinopilot $version" || { echo "after its update, the Flatpak said: $out" >&2; exit 1; }
flatpak info --show-permissions "$id" | tr -d '\n' | grep -qx '\[Session Bus Policy\]org.freedesktop.Flatpak=talk' ||
    { echo "the Flatpak asks for more than org.freedesktop.Flatpak:" >&2; flatpak info --show-permissions "$id" >&2; exit 1; }
echo "ok - the Flatpak runs ($version)"
