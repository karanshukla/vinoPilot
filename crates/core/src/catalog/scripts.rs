//! The shell the steps are made of, as written. Comments inside a script
//! are part of it: `steps.snapshot` pins every byte.

/// Every step starts with these. `$CHECKOUT`, `$WORKTREE`, `$OFF_BRANCH`,
/// `$REPO_URL` and `$RELEASE` come from the environment (see
/// [`crate::job`]), never from formatting paths into the script.
pub(super) const PRELUDE: &str = r#"set -euo pipefail
BIN="$HOME/.local/bin"
# release_run ASSET [ARGS...]: the release's own one-line installer, which
# checks what it downloads against the release's SHA256SUMS.
release_run() {
    local f; f="$(mktemp)"
    curl -fsSL --retry 3 --connect-timeout 20 -o "$f" "$RELEASE/$1"
    shift
    bash "$f" "$@" </dev/null
    rm -f "$f"
}
# checkout: the source to install from. The base path is the checkout,
# cloned or fast-forwarded on its default branch. On another branch,
# $OFF_BRANCH picks the path (catalog's OffBranch): checkout_worktree
# points CHECKOUT at a new one for the rest of the step, checkout_switch
# keeps it. checkout_done, a step of its own, ends either.
checkout() {
    if [ ! -d "$CHECKOUT/.git" ]; then
        echo "== git clone $REPO_URL $CHECKOUT"
        mkdir -p "$(dirname "$CHECKOUT")"
        git clone "$REPO_URL" "$CHECKOUT"
        return
    fi
    local main branch
    main="$(default_branch)"
    branch="$(git -C "$CHECKOUT" symbolic-ref --quiet --short HEAD || true)"
    if [ "$branch" != "$main" ]; then
        branch="${branch:-$(git -C "$CHECKOUT" rev-parse --short HEAD)}"
        case "$OFF_BRANCH" in
            worktree) checkout_worktree "$main" "$branch"; return ;;
            switch) checkout_switch "$main" "$branch" ;;
            *) echo "✗ OFF_BRANCH is '$OFF_BRANCH'" >&2; return 1 ;;
        esac
    fi
    drop_worktree
    echo "== git pull in $CHECKOUT"
    git -C "$CHECKOUT" pull --ff-only
}
# checkout_worktree MAIN FROM: MAIN in $WORKTREE, which becomes CHECKOUT.
checkout_worktree() {
    echo "== $CHECKOUT is on $2, not $1: installing $1 from $WORKTREE"
    git -C "$CHECKOUT" fetch origin "$1"
    drop_worktree
    mkdir -p "$(dirname "$WORKTREE")"
    git -C "$CHECKOUT" worktree add --detach "$WORKTREE" "origin/$1"
    CHECKOUT="$WORKTREE"
}
# checkout_switch MAIN FROM: the checkout onto MAIN, stashing what FROM has
# that isn't committed. It stays on MAIN, then is pulled as on the base path.
checkout_switch() {
    echo "== $CHECKOUT is on $2, not $1: switching it to $1, the install runs from it"
    local back="git switch $2"
    git -C "$CHECKOUT" show-ref --verify --quiet "refs/heads/$2" || back="git switch --detach $2"
    if [ -n "$(git -C "$CHECKOUT" status --porcelain)" ]; then
        git -C "$CHECKOUT" stash push --include-untracked -m "vinopilot: $2 before switching to $1"
        back="$back && git stash pop"
    fi
    git -C "$CHECKOUT" switch "$1"
    echo "Back to your work: $back (in $CHECKOUT). The installed app then runs that code."
}
# The branch origin's HEAD names, "main" when git can't tell.
default_branch() {
    local ref
    ref="$(git -C "$CHECKOUT" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null)" ||
        { git -C "$CHECKOUT" remote set-head origin --auto >/dev/null 2>&1 &&
            ref="$(git -C "$CHECKOUT" symbolic-ref --quiet --short refs/remotes/origin/HEAD)"; } ||
        ref=origin/main
    echo "${ref#origin/}"
}
# Where the step's install runs from: $WORKTREE when checkout used one.
source_dir() { if [ -e "$WORKTREE/.git" ]; then echo "$WORKTREE"; else echo "$CHECKOUT"; fi; }
drop_worktree() {
    [ -e "$WORKTREE" ] || return 0
    git -C "$CHECKOUT" worktree remove --force "$WORKTREE" 2>/dev/null || rm -rf "$WORKTREE" ||
        { echo "✗ Couldn't remove $WORKTREE" >&2; return 0; }
    git -C "$CHECKOUT" worktree prune
    echo "Removed $WORKTREE"
}
# After an install from $WORKTREE: records its commit as
# refs/vinopilot/installed, which `check` counts from, and removes it.
# After one from the checkout, drops the record.
checkout_done() {
    if [ -e "$WORKTREE/.git" ]; then
        git -C "$CHECKOUT" update-ref refs/vinopilot/installed "$(git -C "$WORKTREE" rev-parse HEAD)"
        drop_worktree
    else
        git -C "$CHECKOUT" update-ref -d refs/vinopilot/installed 2>/dev/null || true
    fi
}
need() { command -v "$1" >/dev/null || { echo "✗ $1 isn't installed: $2" >&2; exit 1; }; }
need_uv() { need uv "install it from https://docs.astral.sh/uv/, then try again"; }
need_cargo() { need cargo "install Rust from https://rustup.rs, then try again"; }
# drop_cargo_copies BIN...: what an earlier `cargo install` left in
# ~/.cargo/bin. Left there it still counts as installed, and on many PATHs
# it runs instead of the release in ~/.local/bin.
drop_cargo_copies() {
    local b
    for b in "$@"; do
        [ -e "$HOME/.cargo/bin/$b" ] || continue
        if command -v cargo >/dev/null && cargo uninstall "$b" 2>/dev/null; then
            echo "cargo uninstall $b"
        else
            rm -f "$HOME/.cargo/bin/$b"
            echo "Removed $HOME/.cargo/bin/$b"
        fi
    done
}
# follow_tui upgrade|uninstall: vinopilot-tui (PyPI's vinopilot) is a
# frontend to this command, so it updates and goes with it. Nothing when it
# isn't a uv tool.
follow_tui() {
    command -v uv >/dev/null || return 0
    [ -f "$(uv tool dir)/vinopilot/uv-receipt.toml" ] || return 0
    echo "== uv tool $1 vinopilot"
    uv tool "$1" vinopilot
}
# The checkout, moved to the trash. Fails, keeping it, when it holds work
# that's nowhere else.
trash_checkout() {
    [ -d "$CHECKOUT/.git" ] || { echo "No checkout at $CHECKOUT"; return 0; }
    drop_worktree
    local why=""
    if [ -n "$(git -C "$CHECKOUT" status --porcelain)" ]; then
        why="it has uncommitted changes"
    elif [ -n "$(git -C "$CHECKOUT" log --branches --not --remotes --oneline)" ]; then
        why="it has commits that aren't pushed"
    elif [ -n "$(git -C "$CHECKOUT" stash list)" ]; then
        why="it has stashed changes"
    fi
    [ -z "$why" ] || { echo "✗ Kept $CHECKOUT: $why" >&2; return 1; }
    if command -v gio >/dev/null && gio trash -- "$CHECKOUT"; then :
    elif command -v kioclient >/dev/null && kioclient move "$CHECKOUT" trash:/; then :
    elif command -v trash-put >/dev/null && trash-put -- "$CHECKOUT"; then :
    else
        echo "✗ Kept $CHECKOUT: no gio, kioclient or trash-put to move it to the trash with" >&2
        return 1
    fi
    echo "Moved $CHECKOUT to the trash"
}"#;

/// `vinowhisper-upgrade --uninstall` in releases that have it, else the same
/// by hand: `vinowhispers_own_uninstaller_runs_when_it_has_one` and
/// `uninstalling_vinowhisper_deletes_its_models_and_keeps_transcripts_and_settings`.
pub(super) const WHISPER_UNINSTALL: &str = r#"if [ -x "$BIN/vinowhisper-upgrade" ] && grep -q -- '--uninstall' "$BIN/vinowhisper-upgrade"; then
    "$BIN/vinowhisper-upgrade" --uninstall </dev/null
    exit
fi
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
systemctl --user disable --now vinowhisper-server.socket vinowhisper-server.service 2>/dev/null || true
rm -f "$units/vinowhisper-server.service" "$units/vinowhisper-server.socket"
systemctl --user daemon-reload 2>/dev/null || true
if command -v uv >/dev/null && uv tool list 2>/dev/null | grep -q '^vinowhisper '; then
    uv tool uninstall vinowhisper
fi
# A checkout's install links its .venv's commands into ~/.local/bin.
for f in "$BIN"/vinowhisper-*; do
    if [ -L "$f" ]; then
        case "$(readlink "$f")" in */.venv/bin/vinowhisper-*) rm -f "$f" ;; esac
    fi
done
rm -f "$BIN/vinowhisper-tray" "$BIN/vinowhisper-gui" "$BIN/vinowhisper-upgrade"
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}"/applications/io.github.karanshukla.vinowhisper*.desktop
rm -f "${XDG_CONFIG_HOME:-$HOME/.config}"/autostart/io.github.karanshukla.vinowhisper*.desktop
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/vinowhisper/models" "${XDG_CACHE_HOME:-$HOME/.cache}/vinowhisper"
echo "Removed vinoWhisper and its models. Your transcripts, sessions and settings are still in ~/.local/share/vinowhisper and ~/.config/vinowhisper."
"#;

pub(super) const SPEAK_SOURCE: &str = r#"need_uv
cd "$CHECKOUT"
uv export --locked --no-dev --no-emit-project --no-hashes -o constraints.txt
uv tool install --force --constraints constraints.txt .
rm -f constraints.txt "$BIN/vinospeak-upgrade"
if command -v cargo >/dev/null; then gui/install.sh </dev/null; else echo "no cargo: skipped the window"; fi
"#;

pub(super) const SPEAK_UNINSTALL: &str = r#"if [ -x "$BIN/vinospeak-upgrade" ]; then
    "$BIN/vinospeak-upgrade" --uninstall </dev/null
else
    if [ -x "$CHECKOUT/gui/install.sh" ]; then "$CHECKOUT/gui/install.sh" --uninstall </dev/null || true; fi
    if command -v uv >/dev/null && uv tool list 2>/dev/null | grep -q '^vinospeak '; then uv tool uninstall vinospeak; fi
    # The checkout's installer removes the models too.
    if [ -f "$CHECKOUT/install.sh" ]; then bash "$CHECKOUT/install.sh" --uninstall </dev/null; fi
fi
drop_cargo_copies vinospeak-gui
"#;

/// `vinoopeneffects-upgrade --uninstall` in releases that have it. Before
/// that the uninstaller shipped only in the repo: the checkout's, or a
/// shallow clone's.
pub(super) const OPENEFFECTS_UNINSTALL: &str = r#"if [ -x "$BIN/vinoopeneffects-upgrade" ] && grep -q -- '--uninstall' "$BIN/vinoopeneffects-upgrade"; then
    bash "$BIN/vinoopeneffects-upgrade" --uninstall </dev/null
elif [ -x "$CHECKOUT/scripts/uninstall.sh" ]; then
    "$CHECKOUT/scripts/uninstall.sh"
else
    tmp="$(mktemp -d)"
    git clone --depth 1 "$REPO_URL" "$tmp/src"
    "$tmp/src/scripts/uninstall.sh"
    rm -rf "$tmp"
fi
drop_cargo_copies vinoopeneffects vinoopeneffects-ctl vinoopeneffects-daemon
"#;

pub(super) const OVFETCH_RELEASE: &str = r#"need sha256sum "install coreutils"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for f in ovfetch-x86_64-linux SHA256SUMS; do
    curl -fsSL --retry 3 --connect-timeout 20 -o "$tmp/$f" "$RELEASE/$f"
done
(cd "$tmp" && awk '$2 == "ovfetch-x86_64-linux" || $2 == "*ovfetch-x86_64-linux"' SHA256SUMS > want \
    && [ "$(wc -l < want)" -eq 1 ] && sha256sum -c --strict --quiet want) \
    || { echo "✗ ovfetch-x86_64-linux doesn't match SHA256SUMS. Nothing was installed." >&2; exit 1; }
echo "checksum verified"
install -Dm755 "$tmp/ovfetch-x86_64-linux" "$BIN/ovfetch"
"$BIN/ovfetch" --version
"#;

/// Before a package step: `fetch` (download, then the checksum), `as_root`
/// (pkexec for the one command that needs it) and `scoped_flatpak` (the
/// user's installation, or the system's through `as_root`). Why each is the
/// way it is: docs/how-it-works.md, "Packages and Flatpaks".
pub(super) const PACKAGE: &str = r#"tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fetch() {
    local code=0
    curl -fsSL --retry 3 --connect-timeout 20 -o "$tmp/$1" "$RELEASE/$1" || code=$?
    [ "$code" -ne 22 ] || { echo "✗ The latest release has no $1" >&2; exit 1; }
    [ "$code" -eq 0 ] || { echo "✗ Couldn't download $1" >&2; exit 1; }
    local sums
    for sums in SHA256SUMS "$1.sha256"; do
        : > "$tmp/want"
        curl -fsSL --retry 3 --connect-timeout 20 -o "$tmp/sums" "$RELEASE/$sums" 2>/dev/null || continue
        awk -v f="$1" '$2 == f || $2 == "*" f { print $1 "  " f }' "$tmp/sums" > "$tmp/want"
        [ ! -s "$tmp/want" ] || break
    done
    [ -s "$tmp/want" ] || { echo "✗ The release has no checksum for $1. Nothing was installed." >&2; exit 1; }
    (cd "$tmp" && [ "$(wc -l < want)" -eq 1 ] && sha256sum -c --strict --quiet want) \
        || { echo "✗ $1 doesn't match its checksum. Nothing was installed." >&2; exit 1; }
    echo "checksum verified"
    # The package manager may read it as another user.
    chmod 755 "$tmp"; chmod 644 "$tmp/$1"
}
as_root() {
    if [ "$(id -u)" -eq 0 ]; then "$@"; return; fi
    need pkexec "it asks for your password; or run this as root: $*"
    echo "== $* (asks for your password)"
    pkexec "$@"
}
flatpak_scope() {
    if flatpak info --user "$APP_ID" >/dev/null 2>&1; then echo --user
    elif flatpak info --system "$APP_ID" >/dev/null 2>&1; then echo --system
    else echo --user
    fi
}
scoped_flatpak() {
    local scope; scope="$(flatpak_scope)"
    if [ "$scope" = --system ]; then as_root env LC_ALL=C flatpak "$1" --system "${@:2}"
    else LC_ALL=C flatpak "$1" --user "${@:2}"
    fi
}
"#;

pub(super) const PILOT_DEB: &str = r#"fetch vinopilot-x86_64.deb
as_root apt-get install -y "$tmp/vinopilot-x86_64.deb""#;

pub(super) const PILOT_RPM: &str = r#"[ ! -e /run/ostree-booted ] || { echo "✗ This system layers packages with rpm-ostree, which vinoPilot doesn't drive: replace the vinopilot package with the new release's .rpm, or use the Flatpak" >&2; exit 1; }
fetch vinopilot-x86_64.rpm
if command -v dnf >/dev/null; then as_root dnf install -y "$tmp/vinopilot-x86_64.rpm"
elif command -v zypper >/dev/null; then as_root zypper --non-interactive install --allow-unsigned-rpm "$tmp/vinopilot-x86_64.rpm"
else as_root rpm -U "$tmp/vinopilot-x86_64.rpm"
fi"#;

pub(super) const PILOT_RPM_UNINSTALL: &str = r#"if command -v dnf >/dev/null; then as_root dnf remove -y vinopilot
elif command -v zypper >/dev/null; then as_root zypper --non-interactive remove vinopilot
else as_root rpm -e vinopilot
fi"#;

/// Flatpak calls installing the commit that's already there an error; here
/// it is nothing to do.
pub(super) const FLATPAK_INSTALL: &str = r#"need flatpak "install it from https://flatpak.org/setup, then try again"
fetch "$ASSET"
if out="$(scoped_flatpak install -y "$tmp/$ASSET" 2>&1)"; then
    echo "$out"
else
    echo "$out"
    case "$out" in
        *"already installed"*) echo "This release's Flatpak is the one installed" ;;
        *) exit 1 ;;
    esac
fi"#;

/// Leaves `~/.var/app/$APP_ID` alone: what the app keeps there is the
/// user's to delete.
pub(super) const FLATPAK_UNINSTALL: &str = r#"need flatpak "it's what this was installed with"
scoped_flatpak uninstall -y "$APP_ID""#;
