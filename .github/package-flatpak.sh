#!/bin/sh
# Wraps the release's binaries as a Flatpak bundle:
#
#   .github/package-flatpak.sh DIST
#
# DIST is what package-release.sh made. This unpacks its tarball, builds
# packaging/flatpak's manifest around those binaries, writes
# DIST/vinopilot-x86_64.flatpak and adds it to DIST/SHA256SUMS. Needs flatpak
# and flatpak-builder; the runtime and SDK come from Flathub, for this user.
set -eu

dist="$(cd "$1" && pwd)"
here="$(cd "$(dirname "$0")/../packaging" && pwd)"
id=io.github.karanshukla.vinopilot
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cp "$here/flatpak/$id.yml" "$here/flatpak/host-run.sh" "$here/$id.metainfo.xml" "$work/"
tar -xzf "$dist/vinopilot-x86_64-linux.tar.gz" -C "$work" --no-same-owner
mv "$work/vinopilot" "$work/src"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
(cd "$work" && flatpak-builder --user --install-deps-from=flathub --disable-rofiles-fuse \
    --state-dir="$work/state" --repo="$work/repo" "$work/build" "$id.yml")
# --runtime-repo: installing the bundle can fetch the runtime it needs.
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$work/repo" "$dist/vinopilot-x86_64.flatpak" "$id"
(cd "$dist" && sha256sum vinopilot-x86_64.flatpak >> SHA256SUMS)
