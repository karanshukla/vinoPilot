//! What vinopilot manages and the shell each action runs. Each app's own
//! installer does the work (and the hash checks); this only knows which one
//! to call, with what, for each way of installing it.

use vino_ui::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    App(App),
    Ovfetch,
    Ui,
}

impl Id {
    pub const ALL: [Id; 10] = [
        Id::App(App::Pilot),
        Id::App(App::Whisper),
        Id::App(App::AuthFace),
        Id::App(App::OpenEffects),
        Id::App(App::Pop),
        Id::App(App::Speak),
        Id::App(App::Search),
        Id::App(App::Watch),
        Id::Ovfetch,
        Id::Ui,
    ];

    /// The name in JSON and on the command line: `vinowhisper`, `ovfetch`.
    pub fn slug(self) -> String {
        self.name().to_lowercase()
    }

    /// A component by its slug or name, with or without the `vino`:
    /// `vinoWhisper`, `vinowhisper` and `whisper` are all vinoWhisper.
    pub fn parse(text: &str) -> Option<Id> {
        let want = text.to_lowercase();
        let bare = |id: Id| id.slug().strip_prefix("vino").map(str::to_string);
        Id::ALL.into_iter().find(|id| {
            id.slug() == want
                || bare(*id).as_deref() == Some(want.as_str())
                || (want == "ui" && *id == Id::Ui)
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Id::App(app) => app.name(),
            Id::Ovfetch => "ovfetch",
            Id::Ui => "vinoUserInterface",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            // Its own card says what it is to the person using it.
            Id::App(App::Pilot) => "This app: installs and updates the others",
            Id::App(app) => app.blurb(),
            Id::Ovfetch => "Fetches the OpenVINO runtime an NPU needs",
            Id::Ui => "The egui library the apps are drawn with",
        }
    }

    /// `owner/name` on GitHub.
    pub fn repo(self) -> &'static str {
        match self {
            // Lowercase like its release URLs and the checkout folder
            // install.sh makes; GitHub ignores case, the folder name doesn't.
            Id::App(App::Pilot) => "karanshukla/vinopilot",
            Id::App(app) => app.repo(),
            Id::Ovfetch => "karanshukla/ovfetch",
            Id::Ui => "karanshukla/vinoUserInterface",
        }
    }

    /// The checkout's folder name under the source folder.
    pub fn checkout_name(self) -> &'static str {
        self.repo()
            .rsplit_once('/')
            .map_or(self.repo(), |(_, name)| name)
    }

    pub fn package(self) -> Option<Package> {
        match self {
            Id::App(App::Whisper) => Some(Package::PyPi("vinowhisper")),
            Id::App(App::Speak) => Some(Package::PyPi("vinospeak")),
            Id::Ovfetch => Some(Package::Crate("ovfetch")),
            Id::App(App::Pilot) => Some(Package::Crate("vinopilot")),
            Id::Ui => Some(Package::Library("vino-user-interface")),
            Id::App(_) => None,
        }
    }

    /// Installs under /usr/local and /etc, so every step runs through pkexec.
    pub fn needs_root(self) -> bool {
        self == Id::App(App::AuthFace)
    }

    pub fn channels(self) -> Vec<Channel> {
        let mut all = Vec::new();
        if self != Id::Ui {
            all.push(Channel::Release);
        }
        all.push(Channel::Source);
        if matches!(self.package(), Some(Package::PyPi(_) | Package::Crate(_))) {
            all.push(Channel::Package);
        }
        all
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Package {
    PyPi(&'static str),
    Crate(&'static str),
    /// On crates.io, but a library: nothing to install, apps depend on it.
    Library(&'static str),
}

impl Package {
    pub fn name(self) -> &'static str {
        match self {
            Package::PyPi(n) | Package::Crate(n) | Package::Library(n) => n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// The GitHub release's hash-checked installer.
    Release,
    /// A git checkout in the source folder, built locally.
    Source,
    /// PyPI through `uv tool`, or crates.io through `cargo install`.
    Package,
    /// vinoPilot's own release as a `.deb`, system-wide. Like the next two,
    /// only vinoPilot is found this way, and no `channels()` offers it.
    Deb,
    /// The same as an `.rpm`.
    Rpm,
    /// The same as a Flatpak bundle, for the user or system-wide.
    Flatpak,
}

impl Channel {
    /// `release`, `source`, or `package` (also `pypi`, `crates`); `deb`,
    /// `rpm` and `flatpak` for vinoPilot itself.
    pub fn parse(text: &str) -> Option<Channel> {
        match text.to_lowercase().as_str() {
            "release" | "binary" => Some(Channel::Release),
            "source" | "git" => Some(Channel::Source),
            "package" | "pypi" | "crates" | "crates.io" | "cargo" | "uv" => Some(Channel::Package),
            "deb" => Some(Channel::Deb),
            "rpm" => Some(Channel::Rpm),
            "flatpak" => Some(Channel::Flatpak),
            _ => None,
        }
    }

    pub fn label(self, id: Id) -> &'static str {
        match (self, id.package()) {
            (Channel::Release, _) => "Release",
            (Channel::Source, _) => "Source",
            (Channel::Package, Some(Package::PyPi(_))) => "PyPI",
            (Channel::Package, _) => "crates.io",
            (Channel::Deb, _) => "deb package",
            (Channel::Rpm, _) => "rpm package",
            (Channel::Flatpak, _) => "Flatpak",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Uninstall,
}

impl Action {
    pub fn verb(self) -> &'static str {
        match self {
            Action::Install => "Installing",
            Action::Update => "Updating",
            Action::Uninstall => "Uninstalling",
        }
    }
}

/// One shell script, run with `bash -c` as the user, or as root through
/// pkexec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub script: String,
    pub root: bool,
}

impl Step {
    fn user(script: &str) -> Self {
        Step {
            script: format!("{PRELUDE}\n{script}"),
            root: false,
        }
    }

    fn root(script: &str) -> Self {
        Step {
            script: format!("{PRELUDE}\n{script}"),
            root: true,
        }
    }
}

/// Every step starts with these. `$CHECKOUT`, `$REPO_URL` and `$RELEASE`
/// come from the environment (see [`crate::job`]), never from formatting
/// paths into the script.
const PRELUDE: &str = r#"set -euo pipefail
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
# The checkout, cloned or fast-forwarded.
checkout() {
    if [ -d "$CHECKOUT/.git" ]; then
        echo "== git pull in $CHECKOUT"
        git -C "$CHECKOUT" pull --ff-only
    else
        echo "== git clone $REPO_URL $CHECKOUT"
        mkdir -p "$(dirname "$CHECKOUT")"
        git clone "$REPO_URL" "$CHECKOUT"
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

/// vinoWhisper's own uninstaller is `vinowhisper-upgrade --uninstall`, in
/// releases that have it. Without it, this undoes what its installer and
/// vinowhisper-setup write, then deletes the models and their cache once the
/// server is stopped. Transcripts, sessions and settings stay.
const WHISPER_UNINSTALL: &str = r#"if [ -x "$BIN/vinowhisper-upgrade" ] && grep -q -- '--uninstall' "$BIN/vinowhisper-upgrade"; then
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

const SPEAK_SOURCE: &str = r#"need_uv
cd "$CHECKOUT"
uv export --locked --no-dev --no-emit-project --no-hashes -o constraints.txt
uv tool install --force --constraints constraints.txt .
rm -f constraints.txt "$BIN/vinospeak-upgrade"
if command -v cargo >/dev/null; then gui/install.sh </dev/null; else echo "no cargo: skipped the window"; fi
"#;

const SPEAK_UNINSTALL: &str = r#"if [ -x "$BIN/vinospeak-upgrade" ]; then
    "$BIN/vinospeak-upgrade" --uninstall </dev/null
else
    if [ -x "$CHECKOUT/gui/install.sh" ]; then "$CHECKOUT/gui/install.sh" --uninstall </dev/null || true; fi
    if command -v uv >/dev/null && uv tool list 2>/dev/null | grep -q '^vinospeak '; then uv tool uninstall vinospeak; fi
    # The checkout's installer removes the models too.
    if [ -f "$CHECKOUT/install.sh" ]; then bash "$CHECKOUT/install.sh" --uninstall </dev/null; fi
fi
drop_cargo_copies vinospeak-gui
"#;

/// vinoOpenEffects' uninstaller is `vinoopeneffects-upgrade --uninstall` in
/// releases that have it. Before that it shipped only in the repo: use the
/// checkout's, or a shallow clone's.
const OPENEFFECTS_UNINSTALL: &str = r#"if [ -x "$BIN/vinoopeneffects-upgrade" ] && grep -q -- '--uninstall' "$BIN/vinoopeneffects-upgrade"; then
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

const OVFETCH_RELEASE: &str = r#"need sha256sum "install coreutils"
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

/// vinoPilot's Flatpak, and the menu entry's name.
pub const PILOT_APP_ID: &str = "io.github.karanshukla.vinopilot";

/// Before vinoPilot's own package steps: `fetch ASSET` downloads that file
/// of the release into `$tmp` and checks it against the release's
/// SHA256SUMS, as install.sh does the tarball. `as_root` is pkexec for the
/// one command that installs it; the download and the check stay the user's.
const PILOT_PACKAGE: &str = r#"tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fetch() {
    local f
    for f in SHA256SUMS "$1"; do
        curl -fsSL --retry 3 --connect-timeout 20 -o "$tmp/$f" "$RELEASE/$f"
    done
    (cd "$tmp" && awk -v f="$1" '$2 == f || $2 == "*" f { print $1 "  " f }' SHA256SUMS > want \
        && [ "$(wc -l < want)" -eq 1 ] && sha256sum -c --strict --quiet want) \
        || { echo "✗ $1 doesn't match SHA256SUMS. Nothing was installed." >&2; exit 1; }
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
    if flatpak info --user io.github.karanshukla.vinopilot >/dev/null 2>&1; then echo --user; else echo --system; fi
}
"#;

const PILOT_DEB: &str = r#"fetch vinopilot-x86_64.deb
as_root apt-get install -y "$tmp/vinopilot-x86_64.deb""#;

const PILOT_RPM: &str = r#"[ ! -e /run/ostree-booted ] || { echo "✗ This system layers packages with rpm-ostree, which vinoPilot doesn't drive: replace the vinopilot package with the new release's .rpm, or use the Flatpak" >&2; exit 1; }
fetch vinopilot-x86_64.rpm
if command -v dnf >/dev/null; then as_root dnf install -y "$tmp/vinopilot-x86_64.rpm"
elif command -v zypper >/dev/null; then as_root zypper --non-interactive install --allow-unsigned-rpm "$tmp/vinopilot-x86_64.rpm"
else as_root rpm -U "$tmp/vinopilot-x86_64.rpm"
fi"#;

const PILOT_RPM_UNINSTALL: &str = r#"if command -v dnf >/dev/null; then as_root dnf remove -y vinopilot
elif command -v zypper >/dev/null; then as_root zypper --non-interactive remove vinopilot
else as_root rpm -e vinopilot
fi"#;

/// A bundle with a newer commit installs over the old one; the same commit
/// is an error to Flatpak, `--reinstall` or not (1.14, 2026-11-09), and
/// nothing to do here. A running window keeps its files until it closes:
/// Flatpak removes a deployment that's in use afterwards.
///
/// `-y`, not `--noninteractive`: the latter also stops polkit from asking
/// for a password, and a system-wide bundle install always needs one
/// (`install-bundle` is `auth_admin`, and Flatpak's rule for the admin group
/// doesn't cover it), so it failed with "not allowed for user".
const PILOT_FLATPAK: &str = r#"need flatpak "it's what this vinoPilot was installed with"
fetch vinopilot-x86_64.flatpak
if out="$(LC_ALL=C flatpak install "$(flatpak_scope)" -y "$tmp/vinopilot-x86_64.flatpak" 2>&1)"; then
    echo "$out"
else
    echo "$out"
    case "$out" in
        *"already installed"*) echo "This release's Flatpak is the one installed" ;;
        *) exit 1 ;;
    esac
fi"#;

/// What uninstalling `id` installed through `channel` removes and what it
/// keeps, in a sentence or two for the confirmation. Each app's own
/// uninstaller deletes its downloaded models and keeps the user's data.
pub fn uninstall_note(id: Id, channel: Channel) -> String {
    let what = match id {
        Id::App(App::Pilot) => "Removes this app. The apps it installed stay.",
        Id::App(App::Watch) => "Removes the app.",
        Id::App(_) => "Removes the app and its downloaded models. Your own data and settings stay.",
        Id::Ovfetch => "Removes ovfetch. The runtime it fetched stays.",
        Id::Ui => return "Moves the checkout to the trash.".into(),
    };
    if channel == Channel::Source {
        format!("{what} Moves the checkout to the trash.")
    } else {
        what.into()
    }
}

/// The steps for `action` on `id` installed (or to be installed) through
/// `channel`. Empty when that combination doesn't exist. Updating or
/// uninstalling vinoPilot takes the terminal UI along. Uninstalling a
/// source install ends by moving its checkout to the trash.
pub fn steps(id: Id, channel: Channel, action: Action) -> Vec<Step> {
    let mut steps = app_steps(id, channel, action);
    if id == Id::App(App::Pilot) && !steps.is_empty() {
        match action {
            Action::Install => {}
            Action::Update => steps.push(Step::user("follow_tui upgrade")),
            Action::Uninstall => steps.push(Step::user("follow_tui uninstall")),
        }
    }
    if id != Id::Ui
        && channel == Channel::Source
        && action == Action::Uninstall
        && !steps.is_empty()
    {
        steps.push(Step::user("trash_checkout"));
    }
    steps
}

fn app_steps(id: Id, channel: Channel, action: Action) -> Vec<Step> {
    use Action::*;
    use Channel::*;
    let s = Step::user;
    let r = Step::root;
    match (id, channel, action) {
        // vinoWhisper: install.sh runs vinowhisper-setup on a first install.
        (Id::App(App::Whisper), Release, Install) => vec![s("release_run install.sh --gui --yes")],
        (Id::App(App::Whisper), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinowhisper-upgrade" ]; then "$BIN/vinowhisper-upgrade" </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::Whisper), Source, Install | Update) => {
            vec![s(
                r#"checkout; "$CHECKOUT/scripts/install.sh" --yes --gui </dev/null"#,
            )]
        }
        (Id::App(App::Whisper), Package, Install) => vec![s(
            r#"need_uv; uv tool install --force vinowhisper; rm -f "$BIN/vinowhisper-upgrade"; "$BIN/vinowhisper-setup" --gui --yes </dev/null"#,
        )],
        (Id::App(App::Whisper), Package, Update) => vec![s(
            "need_uv; uv tool upgrade vinowhisper; systemctl --user try-restart vinowhisper-server.service 2>/dev/null || true",
        )],
        (Id::App(App::Whisper), _, Uninstall) => vec![s(WHISPER_UNINSTALL)],

        // vinoSpeak
        (Id::App(App::Speak), Release, Install) => vec![s("release_run install.sh --gui")],
        (Id::App(App::Speak), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinospeak-upgrade" ]; then "$BIN/vinospeak-upgrade" </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::Speak), Source, Install | Update) => {
            vec![s(&format!("checkout\n{SPEAK_SOURCE}"))]
        }
        (Id::App(App::Speak), Package, Install) => {
            vec![s(
                r#"need_uv; uv tool install --force --python 3.14 vinospeak; rm -f "$BIN/vinospeak-upgrade""#,
            )]
        }
        (Id::App(App::Speak), Package, Update) => vec![s("need_uv; uv tool upgrade vinospeak")],
        (Id::App(App::Speak), _, Uninstall) => vec![s(SPEAK_UNINSTALL)],

        // vinoPop: get.sh unpacks the release and runs its install.sh.
        (Id::App(App::Pop), Release, Install) => vec![s("release_run get.sh")],
        (Id::App(App::Pop), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinopop-upgrade" ]; then "$BIN/vinopop-upgrade" </dev/null; else release_run get.sh; fi"#,
        )],
        (Id::App(App::Pop), Release, Uninstall) => vec![s(
            r#"if [ -x "$BIN/vinopop-upgrade" ]; then "$BIN/vinopop-upgrade" --uninstall </dev/null; else release_run get.sh --uninstall; fi"#,
        )],
        (Id::App(App::Pop), Source, Install | Update) => {
            vec![s(r#"checkout; bash "$CHECKOUT/install.sh" </dev/null"#)]
        }
        (Id::App(App::Pop), Source, Uninstall) => {
            vec![s(r#"bash "$CHECKOUT/install.sh" --uninstall </dev/null"#)]
        }

        // vinoSearch: the installer passes --uninstall through to the
        // release's scripts/install.sh.
        (Id::App(App::Search), Release, Install) => vec![s("release_run install.sh")],
        (Id::App(App::Search), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinosearch-upgrade" ]; then "$BIN/vinosearch-upgrade" </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::Search), Release, Uninstall) => vec![s(
            "release_run install.sh --uninstall; drop_cargo_copies vinosearch vinosearch-settings",
        )],
        (Id::App(App::Search), Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; "$CHECKOUT/scripts/install.sh" </dev/null"#,
            )]
        }
        (Id::App(App::Search), Source, Uninstall) => {
            vec![s(
                r#""$CHECKOUT/scripts/install.sh" --uninstall </dev/null; drop_cargo_copies vinosearch vinosearch-settings"#,
            )]
        }

        // vinoWatch: vinowatch-upgrade is a copy of install.sh.
        (Id::App(App::Watch), Release, Install) => vec![s("release_run install.sh")],
        (Id::App(App::Watch), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinowatch-upgrade" ]; then sh "$BIN/vinowatch-upgrade" </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::Watch), Release, Uninstall) => {
            vec![s(
                "release_run install.sh --uninstall; drop_cargo_copies vinowatch",
            )]
        }
        (Id::App(App::Watch), Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; sh "$CHECKOUT/install.sh" </dev/null"#,
            )]
        }
        (Id::App(App::Watch), Source, Uninstall) => {
            vec![s(
                r#"sh "$CHECKOUT/install.sh" --uninstall </dev/null; drop_cargo_copies vinowatch"#,
            )]
        }

        // vinoOpenEffects
        (Id::App(App::OpenEffects), Release, Install) => vec![s("release_run install.sh")],
        (Id::App(App::OpenEffects), Release, Update) => vec![s(
            r#"if [ -x "$BIN/vinoopeneffects-upgrade" ]; then bash "$BIN/vinoopeneffects-upgrade" </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::OpenEffects), Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; "$CHECKOUT/scripts/install.sh" </dev/null"#,
            )]
        }
        (Id::App(App::OpenEffects), _, Uninstall) => vec![s(OPENEFFECTS_UNINSTALL)],

        // vinoAuthFace: root throughout; the checkout is pulled as the user
        // so it stays the user's.
        (Id::App(App::AuthFace), Release, Install) => vec![r("release_run install.sh")],
        (Id::App(App::AuthFace), Release, Update) => vec![r(
            r#"if [ -x /usr/local/bin/vinoauthface-upgrade ]; then /usr/local/bin/vinoauthface-upgrade </dev/null; else release_run install.sh; fi"#,
        )],
        (Id::App(App::AuthFace), Source, Install | Update) => vec![
            s("checkout"),
            r(r#"cd "$CHECKOUT"; bash ./deploy.sh </dev/null"#),
        ],
        (Id::App(App::AuthFace), _, Uninstall) => vec![r(
            r#"if [ -x /usr/local/bin/vinoauthface-uninstall ]; then /usr/local/bin/vinoauthface-uninstall; else bash "$CHECKOUT/uninstall.sh"; fi"#,
        )],

        // ovfetch: a bare binary in the release, a bin crate on crates.io.
        // Each install drops the other channel's copy, which would otherwise
        // shadow it or be what detection finds.
        (Id::Ovfetch, Release, Install | Update) => {
            vec![s(&format!("{OVFETCH_RELEASE}drop_cargo_copies ovfetch\n"))]
        }
        (Id::Ovfetch, Release, Uninstall) => {
            vec![s(
                r#"rm -f "$BIN/ovfetch"; echo "Removed $BIN/ovfetch"; drop_cargo_copies ovfetch"#,
            )]
        }
        (Id::Ovfetch, Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; cargo install --locked --force --path "$CHECKOUT"; rm -f "$BIN/ovfetch""#,
            )]
        }
        (Id::Ovfetch, Package, Install | Update) => {
            vec![s(
                r#"need_cargo; cargo install --locked --force ovfetch; rm -f "$BIN/ovfetch""#,
            )]
        }
        (Id::Ovfetch, Source | Package, Uninstall) => {
            vec![s(r#"drop_cargo_copies ovfetch; rm -f "$BIN/ovfetch""#)]
        }

        // vinoPilot: its install.sh builds from a checkout when run from one.
        (Id::App(App::Pilot), Release, Install | Update) => vec![s(
            r#"if [ -x "$BIN/vinopilot-upgrade" ]; then bash "$BIN/vinopilot-upgrade" </dev/null; else release_run install.sh; fi
drop_cargo_copies vinopilot vinopilot-gui"#,
        )],
        (Id::App(App::Pilot), Release, Uninstall) => vec![s(
            "release_run install.sh --uninstall; drop_cargo_copies vinopilot vinopilot-gui",
        )],
        (Id::App(App::Pilot), Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; bash "$CHECKOUT/install.sh" </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
            )]
        }
        (Id::App(App::Pilot), Source, Uninstall) => {
            vec![s(
                r#"bash "$CHECKOUT/install.sh" --uninstall </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
            )]
        }
        (Id::App(App::Pilot), Package, Install | Update) => {
            vec![s(
                r#"need_cargo; cargo install --locked --force vinopilot vinopilot-gui
rm -f "$BIN/vinopilot" "$BIN/vinopilot-gui" "$BIN/vinopilot-upgrade"
"$HOME/.cargo/bin/vinopilot-gui" --install-desktop"#,
            )]
        }
        (Id::App(App::Pilot), Package, Uninstall) => vec![s(
            "need_cargo; vinopilot-gui --uninstall-desktop || true; for c in vinopilot vinopilot-gui; do cargo uninstall \"$c\" || true; done",
        )],

        // vinoPilot's packages: the release's file for that package manager,
        // checked, then installed over the old one.
        (Id::App(App::Pilot), Deb, Install | Update) => {
            vec![s(&format!("{PILOT_PACKAGE}{PILOT_DEB}"))]
        }
        (Id::App(App::Pilot), Deb, Uninstall) => vec![s(&format!(
            "{PILOT_PACKAGE}as_root apt-get remove -y vinopilot"
        ))],
        (Id::App(App::Pilot), Rpm, Install | Update) => {
            vec![s(&format!("{PILOT_PACKAGE}{PILOT_RPM}"))]
        }
        (Id::App(App::Pilot), Rpm, Uninstall) => {
            vec![s(&format!("{PILOT_PACKAGE}{PILOT_RPM_UNINSTALL}"))]
        }
        (Id::App(App::Pilot), Flatpak, Install | Update) => {
            vec![s(&format!("{PILOT_PACKAGE}{PILOT_FLATPAK}"))]
        }
        (Id::App(App::Pilot), Flatpak, Uninstall) => vec![s(&format!(
            "{PILOT_PACKAGE}flatpak uninstall \"$(flatpak_scope)\" -y {PILOT_APP_ID}"
        ))],

        // vinoUserInterface: a library, so "installed" means a checkout that builds.
        (Id::Ui, Source, Install | Update) => vec![s(
            r#"checkout; need_cargo; cd "$CHECKOUT"; cargo build --locked --example gallery; echo "Run it: cargo run --example gallery (in $CHECKOUT)""#,
        )],
        (Id::Ui, Source, Uninstall) => vec![s("trash_checkout")],

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIONS: [Action; 3] = [Action::Install, Action::Update, Action::Uninstall];

    #[test]
    fn every_offered_channel_can_install_update_and_uninstall() {
        for id in Id::ALL {
            for channel in id.channels() {
                for action in ACTIONS {
                    assert!(
                        !steps(id, channel, action).is_empty(),
                        "{} {:?} {:?}",
                        id.name(),
                        channel,
                        action
                    );
                }
            }
        }
    }

    #[test]
    fn only_authface_asks_for_root() {
        for id in Id::ALL {
            for channel in id.channels() {
                for action in ACTIONS {
                    let root = steps(id, channel, action).iter().any(|s| s.root);
                    assert_eq!(
                        root,
                        id.needs_root(),
                        "{} {channel:?} {action:?}",
                        id.name()
                    );
                }
            }
        }
    }

    #[test]
    fn uninstalling_from_source_trashes_the_checkout_last() {
        let ends_trashing = |id: Id, channel| {
            steps(id, channel, Action::Uninstall)
                .last()
                .is_some_and(|s| !s.root && s.script.ends_with("trash_checkout"))
        };
        for id in Id::ALL {
            assert!(ends_trashing(id, Channel::Source), "{}", id.name());
            assert!(!ends_trashing(id, Channel::Release), "{}", id.name());
            assert!(!ends_trashing(id, Channel::Package), "{}", id.name());
        }
    }

    #[test]
    fn the_terminal_ui_follows_vinopilots_updates_and_uninstall() {
        let follows = |id, channel, action| {
            steps(id, channel, action)
                .iter()
                .any(|s| s.script.contains("\nfollow_tui "))
        };
        for id in Id::ALL {
            for channel in id.channels() {
                let pilot = id == Id::App(App::Pilot);
                assert!(!follows(id, channel, Action::Install), "{}", id.name());
                assert_eq!(follows(id, channel, Action::Update), pilot, "{}", id.name());
                assert_eq!(
                    follows(id, channel, Action::Uninstall),
                    pilot,
                    "{}",
                    id.name()
                );
            }
        }
    }

    #[test]
    fn installing_ovfetch_or_vinopilot_drops_the_other_channels_copy() {
        let script = |id, channel, action| -> String {
            steps(id, channel, action)
                .iter()
                .map(|s| s.script.as_str())
                .collect()
        };
        for (id, bin) in [(Id::Ovfetch, "ovfetch"), (Id::App(App::Pilot), "vinopilot")] {
            for action in [Action::Install, Action::Update] {
                let release = script(id, Channel::Release, action);
                assert!(
                    release.contains(&format!("drop_cargo_copies {bin}")),
                    "{bin} release {action:?}"
                );
                let package = script(id, Channel::Package, action);
                assert!(
                    package.contains(&format!("rm -f \"$BIN/{bin}\"")),
                    "{bin} package {action:?}"
                );
            }
        }
        assert!(
            script(Id::Ovfetch, Channel::Source, Action::Install)
                .contains(r#"rm -f "$BIN/ovfetch""#)
        );
        assert!(
            script(Id::App(App::Pilot), Channel::Source, Action::Install)
                .contains("drop_cargo_copies vinopilot")
        );
    }

    /// Every channel a component can be found installed through.
    fn channels(id: Id) -> Vec<Channel> {
        let mut all = id.channels();
        if id == Id::App(App::Pilot) {
            all.extend([Channel::Deb, Channel::Rpm, Channel::Flatpak]);
        }
        all
    }

    #[test]
    fn vinopilots_packages_update_from_the_release_checked_and_ask_for_root_themselves() {
        for (channel, asset) in [
            (Channel::Deb, "vinopilot-x86_64.deb"),
            (Channel::Rpm, "vinopilot-x86_64.rpm"),
            (Channel::Flatpak, "vinopilot-x86_64.flatpak"),
        ] {
            assert!(!Id::App(App::Pilot).channels().contains(&channel));
            for action in ACTIONS {
                let steps = steps(Id::App(App::Pilot), channel, action);
                assert!(!steps.is_empty(), "{channel:?} {action:?}");
                // Only the package manager runs as root, not the whole step.
                assert!(steps.iter().all(|s| !s.root), "{channel:?} {action:?}");
            }
            let update = &steps(Id::App(App::Pilot), channel, Action::Update)[0].script;
            assert!(update.contains(&format!("fetch {asset}")), "{channel:?}");
            assert!(update.contains("sha256sum -c"), "{channel:?}");
            // No other component has them.
            assert!(steps(Id::Ovfetch, channel, Action::Update).is_empty());
        }
    }

    #[test]
    fn a_library_has_no_release_or_package_install() {
        assert_eq!(Id::Ui.channels(), vec![Channel::Source]);
    }

    #[test]
    fn components_parse_by_slug_name_or_bare_name() {
        for id in Id::ALL {
            assert_eq!(Id::parse(&id.slug()), Some(id));
            assert_eq!(Id::parse(id.name()), Some(id));
        }
        assert_eq!(Id::parse("whisper"), Some(Id::App(App::Whisper)));
        assert_eq!(Id::parse("ui"), Some(Id::Ui));
        assert_eq!(Id::parse("nope"), None);
    }

    #[test]
    fn checkout_names_match_the_repos() {
        assert_eq!(Id::App(App::Search).checkout_name(), "vinoSearch");
        assert_eq!(Id::Ui.checkout_name(), "vinoUserInterface");
    }

    #[test]
    fn the_uninstall_note_names_the_models_only_for_apps_that_have_them() {
        for id in Id::ALL {
            for channel in id.channels() {
                let note = uninstall_note(id, channel);
                let models = matches!(id, Id::App(app) if app != App::Pilot && app != App::Watch);
                assert_eq!(note.contains("models"), models, "{} {channel:?}", id.name());
                assert_eq!(
                    note.contains("trash"),
                    channel == Channel::Source,
                    "{} {channel:?}",
                    id.name()
                );
                assert_eq!(
                    note.matches("trash").count(),
                    usize::from(channel == Channel::Source)
                );
            }
        }
    }

    /// Runs vinoWhisper's uninstall step in a home of its own, with
    /// `systemctl` and `uv` stubbed out so nothing reaches the real session.
    fn uninstall_whisper(name: &str, setup: impl FnOnce(&std::path::Path)) -> std::path::PathBuf {
        use std::fs;
        let root =
            std::env::temp_dir().join(format!("vinopilot-whisper-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        let stubs = root.join("stubs");
        fs::create_dir_all(&stubs).unwrap();
        for stub in ["systemctl", "uv"] {
            let path = stubs.join(stub);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            make_executable(&path);
        }
        let data = root.join("data/vinowhisper");
        let cache = root.join("cache/vinowhisper");
        for dir in [
            data.join("models/whisper-small.en"),
            data.join("transcripts"),
            data.join("sessions"),
            cache.join("export-tmp"),
            home.join(".config/vinowhisper"),
            home.join(".local/bin"),
        ] {
            fs::create_dir_all(dir).unwrap();
        }
        fs::write(data.join("models/whisper-small.en/model.xml"), "x").unwrap();
        fs::write(data.join("transcripts/2026-11-11-120000.txt"), "x").unwrap();
        setup(&home);
        let step = &steps(Id::App(App::Whisper), Channel::Release, Action::Uninstall)[0];
        let out = std::process::Command::new("bash")
            .args(["-c", &step.script])
            .env_clear()
            .env("HOME", &home)
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("PATH", format!("{}:/usr/bin:/bin", stubs.display()))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        root
    }

    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn uninstalling_vinowhisper_deletes_its_models_and_keeps_transcripts_and_settings() {
        if std::process::Command::new("bash")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let root = uninstall_whisper("inline", |_| {});
        assert!(!root.join("data/vinowhisper/models").exists());
        assert!(!root.join("cache/vinowhisper").exists());
        assert!(
            root.join("data/vinowhisper/transcripts/2026-11-11-120000.txt")
                .exists()
        );
        assert!(root.join("data/vinowhisper/sessions").exists());
        assert!(root.join("home/.config/vinowhisper").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vinowhispers_own_uninstaller_runs_when_it_has_one() {
        if std::process::Command::new("bash")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        // An upgrade script that knows --uninstall is run with it, and the
        // inline steps aren't.
        let root = uninstall_whisper("own", |home| {
            let upgrade = home.join(".local/bin/vinowhisper-upgrade");
            std::fs::write(
                &upgrade,
                "#!/bin/sh\n# --uninstall: remove it\necho \"$@\" > \"$HOME/ran\"\n",
            )
            .unwrap();
            make_executable(&upgrade);
        });
        let ran = std::fs::read_to_string(root.join("home/ran")).unwrap();
        assert_eq!(ran.trim(), "--uninstall");
        assert!(root.join("data/vinowhisper/models").exists());
        std::fs::remove_dir_all(&root).unwrap();

        // An older one that doesn't is left alone, and the inline steps run.
        let root = uninstall_whisper("old", |home| {
            let upgrade = home.join(".local/bin/vinowhisper-upgrade");
            std::fs::write(&upgrade, "#!/bin/sh\necho \"$@\" > \"$HOME/ran\"\n").unwrap();
            make_executable(&upgrade);
        });
        assert!(!root.join("home/ran").exists());
        assert!(!root.join("home/.local/bin/vinowhisper-upgrade").exists());
        assert!(!root.join("data/vinowhisper/models").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_script_parses() {
        let Ok(bash) = std::process::Command::new("bash").arg("--version").output() else {
            return;
        };
        assert!(bash.status.success());
        for id in Id::ALL {
            for channel in channels(id) {
                for action in ACTIONS {
                    for step in steps(id, channel, action) {
                        let out = std::process::Command::new("bash")
                            .args(["-n", "-c", &step.script])
                            .output()
                            .unwrap();
                        assert!(
                            out.status.success(),
                            "{} {channel:?} {action:?}: {}",
                            id.name(),
                            String::from_utf8_lossy(&out.stderr)
                        );
                    }
                }
            }
        }
    }
}
