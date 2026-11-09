#!/usr/bin/env bash
# Runs what's in a packed dist/: everything matches SHA256SUMS, both
# binaries start and report VERSION, the CLI is static, and the .deb and the
# .rpm are that version with those same two binaries, byte for byte.
#
#   tests/release-smoke.sh dist 0.1.0
set -euo pipefail
dist="${1:?dist dir}" version="${2:?version}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
(cd "$dist" && sha256sum -c --strict SHA256SUMS)
tar -xzf "$dist/vinopilot-x86_64-linux.tar.gz" -C "$tmp"
test "$("$tmp/vinopilot/vinopilot" --version)" = "vinopilot $version"
test "$("$tmp/vinopilot/vinopilot-gui" --version)" = "vinopilot-gui $version"
file "$tmp/vinopilot/vinopilot" | grep -qE 'static(ally|-pie) linked' || { echo "vinopilot isn't static" >&2; exit 1; }
HOME="$tmp" "$tmp/vinopilot/vinopilot" list --json > /dev/null
echo "ok - the release tarball runs ($version)"

deb="$dist/vinopilot-x86_64.deb" rpm="$dist/vinopilot-x86_64.rpm"
test "$(dpkg-deb -f "$deb" Version)" = "$version"
dpkg-deb -x "$deb" "$tmp/deb"
test "$(rpm -qp --qf '%{VERSION}' "$rpm")" = "$version"
mkdir "$tmp/rpm"
(cd "$tmp/rpm" && rpm2cpio "$rpm" | cpio -idm --quiet)
for kind in deb rpm; do
    for bin in vinopilot vinopilot-gui; do
        cmp "$tmp/vinopilot/$bin" "$tmp/$kind/usr/bin/$bin"
        test -x "$tmp/$kind/usr/bin/$bin"
    done
    grep -qx 'Exec=vinopilot-gui' "$tmp/$kind/usr/share/applications/io.github.karanshukla.vinopilot.desktop"
    test -s "$tmp/$kind/usr/share/icons/hicolor/scalable/apps/io.github.karanshukla.vinopilot.svg"
    echo "ok - the .$kind has the tarball's binaries ($version)"
done
