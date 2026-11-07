#!/usr/bin/env bash
# install.sh's release path end to end, with no network and no real $HOME:
# a fake release in a temp dir, served to curl as file://, piped into bash
# the way the README's one-liner runs it.
#
#   tests/install-e2e.sh target/debug
set -euo pipefail

bins="$(cd "${1:?usage: tests/install-e2e.sh DIR_WITH_BINARIES}" && pwd)"
root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

pass() { echo "ok - $*"; }
die() { echo "not ok - $*" >&2; exit 1; }

# The release, packed as .github/package-release.sh packs it.
release="$tmp/release"
mkdir -p "$release/vinopilot"
cp "$bins/vinopilot" "$bins/vinopilot-gui" "$root/install.sh" "$release/vinopilot/"
tar -czf "$release/vinopilot-x86_64-linux.tar.gz" -C "$release" vinopilot
rm -rf "$release/vinopilot"
cp "$root/install.sh" "$release/"
(cd "$release" && sha256sum vinopilot-x86_64-linux.tar.gz install.sh > SHA256SUMS)

run() {
    local home="$1"; shift
    env -i PATH="/usr/bin:/bin" HOME="$home" XDG_DATA_HOME="$home/.local/share" \
        VINOPILOT_RELEASE_BASE="file://$release" bash -s -- "$@" < "$root/install.sh"
}

home="$tmp/home"
mkdir -p "$home"
run "$home" >/dev/null || die "install from the release"
for f in .local/bin/vinopilot .local/bin/vinopilot-gui .local/bin/vinopilot-upgrade \
    .local/share/applications/io.github.karanshukla.vinopilot.desktop \
    .local/share/icons/hicolor/scalable/apps/io.github.karanshukla.vinopilot.svg; do
    [ -e "$home/$f" ] || die "install left no $f"
done
grep -q "^Exec=$home/.local/bin/vinopilot-gui$" "$home/.local/share/applications/io.github.karanshukla.vinopilot.desktop" \
    || die "the desktop entry runs the wrong binary"
"$home/.local/bin/vinopilot" --version | grep -q '^vinopilot ' || die "installed vinopilot doesn't run"
pass "installs the release, the window, the menu entry and vinopilot-upgrade"

# What the Update button runs: the kept copy, with a tag.
env -i PATH="/usr/bin:/bin" HOME="$home" XDG_DATA_HOME="$home/.local/share" \
    VINOPILOT_RELEASE_BASE="file://$release" VINO_UPDATE_TAG=v9.9.9 \
    bash "$home/.local/bin/vinopilot-upgrade" >/dev/null || die "vinopilot-upgrade"
pass "vinopilot-upgrade reinstalls"

run "$home" --uninstall >/dev/null || die "--uninstall"
[ -z "$(find "$home/.local" -name '*vinopilot*' -not -type d)" ] || die "--uninstall left files behind"
pass "--uninstall removes everything it installed"

home2="$tmp/home2"
mkdir -p "$home2"
run "$home2" --no-gui >/dev/null || die "--no-gui"
[ -x "$home2/.local/bin/vinopilot" ] && [ ! -e "$home2/.local/bin/vinopilot-gui" ] || die "--no-gui installed the window"
pass "--no-gui installs only the command"

# A swapped tarball: the sums no longer match.
echo tampered >> "$release/vinopilot-x86_64-linux.tar.gz"
home3="$tmp/home3"
mkdir -p "$home3"
if run "$home3" >/dev/null 2>&1; then die "installed a tarball that doesn't match SHA256SUMS"; fi
[ ! -e "$home3/.local/bin/vinopilot" ] || die "a refused install left a binary"
pass "refuses a tarball that doesn't match SHA256SUMS"

# A tarball listed twice: ambiguous, refused.
(cd "$release" && sha256sum vinopilot-x86_64-linux.tar.gz > SHA256SUMS && sha256sum vinopilot-x86_64-linux.tar.gz >> SHA256SUMS)
if run "$home3" >/dev/null 2>&1; then die "installed with the tarball listed twice"; fi
pass "refuses a SHA256SUMS that lists the tarball twice"

if run "$home3" "v1; rm -rf /" >/dev/null 2>&1; then die "took a bad tag"; fi
pass "refuses a tag that isn't one"
