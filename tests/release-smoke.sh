#!/usr/bin/env bash
# Runs what's in a packed dist/: the tarball matches SHA256SUMS, both
# binaries start and report VERSION, and the CLI is static.
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
