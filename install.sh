#!/usr/bin/env bash
# Install or upgrade vinoPilot for this user:
#
#   curl -fsSL https://github.com/karanshukla/vinopilot/releases/latest/download/install.sh | bash
#
# Options go after `bash -s --`, e.g. `| bash -s -- --no-gui`:
#
#   vN.N.N       that release instead of the newest (or VINO_UPDATE_TAG=vN.N.N)
#   --no-gui     only the `vinopilot` command, no window or app menu entry
#   --uninstall  remove what this put in ~/.local
#
# It downloads the release's SHA256SUMS and tarball, and refuses the tarball
# unless it's listed there exactly once and matches. Then it puts vinopilot
# and vinopilot-gui in ~/.local/bin, the window in the app menu, and a copy
# of itself as vinopilot-upgrade, which the window's Update button runs.
# No sudo: everything goes in your home.
#
# From a checkout (./install.sh) it builds with cargo instead, and leaves
# no vinopilot-upgrade, so the window knows it's a source install.

# Everything is in main, called on the last line: piped into bash, a download
# cut short must not run half a script.
set -euo pipefail

REPO="karanshukla/vinopilot"
APP_ID="io.github.karanshukla.vinopilot"

fail() { echo "✗ $*" >&2; exit 1; }

# Global: the EXIT trap runs after main has returned.
WORK=""
cleanup() { [ -z "$WORK" ] || rm -rf "$WORK"; }
trap cleanup EXIT

main() {
    local tag="${VINO_UPDATE_TAG:-}" gui=1 uninstall=0 arg
    for arg in "$@"; do
        case "$arg" in
            v[0-9]*) tag="$arg" ;;
            --no-gui) gui=0 ;;
            --uninstall) uninstall=1 ;;
            -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]:-$0}" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
            *) fail "Unknown option '$arg' (try --help)" ;;
        esac
    done
    if [ -n "$tag" ] && ! [[ "$tag" =~ ^v[0-9]+(\.[0-9]+)*$ ]]; then
        fail "Not a release tag: '$tag'"
    fi
    [ "$(id -u)" -ne 0 ] || fail "Run this as yourself, not root: vinoPilot installs into your home"

    local bin="$HOME/.local/bin"
    local data="${XDG_DATA_HOME:-$HOME/.local/share}"
    local desktop="$data/applications/$APP_ID.desktop"
    local icon="$data/icons/hicolor/scalable/apps/$APP_ID.svg"

    if [ "$uninstall" = 1 ]; then
        rm -f "$bin/vinopilot" "$bin/vinopilot-gui" "$bin/vinopilot-upgrade" "$desktop" "$icon"
        command -v update-desktop-database >/dev/null && update-desktop-database -q "$data/applications" 2>/dev/null || true
        echo "Removed vinoPilot. Its setting is still in ${XDG_CONFIG_HOME:-$HOME/.config}/vinopilot."
        exit 0
    fi

    # A checkout: this script beside the workspace's Cargo.toml.
    local here="" src from_release=0
    if [ -f "${BASH_SOURCE[0]:-}" ]; then
        local dir
        dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        if [ -f "$dir/Cargo.toml" ] && [ -d "$dir/crates/gui" ]; then
            here="$dir"
        fi
    fi

    if [ -n "$here" ]; then
        command -v cargo >/dev/null || fail "No cargo to build with: install Rust from https://rustup.rs, or install the release with the curl line in --help"
        echo "== Building from $here"
        local packages=(-p vinopilot)
        [ "$gui" = 0 ] || packages+=(-p vinopilot-gui)
        cargo build --release --locked --manifest-path "$here/Cargo.toml" "${packages[@]}"
        src="$here/target/release"
    else
        [ "$(uname -m)" = x86_64 ] || fail "The release binaries are x86_64 only; build from a checkout on $(uname -m)"
        local tool
        for tool in curl tar sha256sum awk; do
            command -v "$tool" >/dev/null || fail "Missing $tool: install it, then run this again"
        done
        local base
        if [ -n "${VINOPILOT_RELEASE_BASE:-}" ]; then
            base="$VINOPILOT_RELEASE_BASE"
        elif [ -n "$tag" ]; then
            base="https://github.com/$REPO/releases/download/$tag"
        else
            base="https://github.com/$REPO/releases/latest/download"
        fi
        local asset="vinopilot-x86_64-linux.tar.gz"
        WORK="$(mktemp -d)"
        local work="$WORK"
        echo "== Downloading ${tag:-the latest release} from $base"
        local name
        for name in SHA256SUMS "$asset"; do
            curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors --connect-timeout 20 \
                -o "$work/$name" "$base/$name" || fail "Download failed: $base/$name"
        done
        (cd "$work" \
            && awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 "  " f }' SHA256SUMS > want \
            && [ "$(wc -l < want)" -eq 1 ] \
            && sha256sum -c --strict --quiet want) \
            || fail "Checksum verification failed for $asset. Nothing was installed."
        echo "checksum verified"
        tar -xzf "$work/$asset" -C "$work" --no-same-owner
        src="$work/vinopilot"
        [ -x "$src/vinopilot" ] || fail "$asset has no vinopilot binary"
        from_release=1
    fi

    install -Dm755 "$src/vinopilot" "$bin/vinopilot"
    if [ "$gui" = 1 ]; then
        install -Dm755 "$src/vinopilot-gui" "$bin/vinopilot-gui"
        XDG_DATA_HOME="$data" "$bin/vinopilot-gui" --install-desktop
    else
        rm -f "$bin/vinopilot-gui" "$desktop" "$icon"
    fi
    if [ "$from_release" = 1 ]; then
        install -Dm755 "$src/install.sh" "$bin/vinopilot-upgrade"
    else
        rm -f "$bin/vinopilot-upgrade"
    fi

    echo "Installed $("$bin/vinopilot" --version) in $bin"
    [ "$gui" = 0 ] || echo "Open it from the app menu (vinoPilot), or run vinopilot-gui."
    case ":$PATH:" in
        *":$bin:"*) ;;
        *) echo "Note: $bin isn't on your PATH; add it to run vinopilot by name." ;;
    esac
}

main "$@"
