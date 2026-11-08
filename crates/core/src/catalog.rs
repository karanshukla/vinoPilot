//! What vinopilot manages and the shell each action runs. Each app's own
//! installer does the work (and the hash checks); this only knows which one
//! to call, with what, for each way of installing it.

use vino_ui::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    App(App),
    Ovfetch,
    Ui,
    /// vinopilot itself.
    Pilot,
}

impl Id {
    pub const ALL: [Id; 10] = [
        Id::Pilot,
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
            Id::Pilot => "vinoPilot",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Id::App(app) => app.blurb(),
            Id::Ovfetch => "Fetches the OpenVINO runtime an NPU needs",
            Id::Ui => "The egui library the apps are drawn with",
            Id::Pilot => "This app: installs and updates the others",
        }
    }

    /// `owner/name` on GitHub.
    pub fn repo(self) -> &'static str {
        match self {
            Id::App(app) => app.repo(),
            Id::Ovfetch => "karanshukla/ovfetch",
            Id::Ui => "karanshukla/vinoUserInterface",
            Id::Pilot => "karanshukla/vinopilot",
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
            Id::Pilot => Some(Package::Crate("vinopilot")),
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
}

impl Channel {
    /// `release`, `source`, or `package` (also `pypi`, `crates`).
    pub fn parse(text: &str) -> Option<Channel> {
        match text.to_lowercase().as_str() {
            "release" | "binary" => Some(Channel::Release),
            "source" | "git" => Some(Channel::Source),
            "package" | "pypi" | "crates" | "crates.io" | "cargo" | "uv" => Some(Channel::Package),
            _ => None,
        }
    }

    pub fn label(self, id: Id) -> &'static str {
        match (self, id.package()) {
            (Channel::Release, _) => "Release",
            (Channel::Source, _) => "Source",
            (Channel::Package, Some(Package::PyPi(_))) => "PyPI",
            (Channel::Package, _) => "crates.io",
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

/// vinoWhisper has no uninstaller: this undoes what its installer and
/// vinowhisper-setup write, and leaves the models and config.
const WHISPER_UNINSTALL: &str = r#"units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
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
echo "Removed vinoWhisper. Its models and settings are still in ~/.local/share and ~/.config."
"#;

const SPEAK_SOURCE: &str = r#"need_uv
cd "$CHECKOUT"
uv export --locked --no-dev --no-emit-project --no-hashes -o constraints.txt
uv tool install --force --constraints constraints.txt .
rm -f constraints.txt
if command -v cargo >/dev/null; then gui/install.sh </dev/null; else echo "no cargo: skipped the window"; fi
"#;

const SPEAK_UNINSTALL: &str = r#"if [ -x "$BIN/vinospeak-upgrade" ]; then
    "$BIN/vinospeak-upgrade" --uninstall </dev/null
else
    if [ -x "$CHECKOUT/gui/install.sh" ]; then "$CHECKOUT/gui/install.sh" --uninstall </dev/null || true; fi
    if command -v uv >/dev/null && uv tool list 2>/dev/null | grep -q '^vinospeak '; then uv tool uninstall vinospeak; fi
fi
drop_cargo_copies vinospeak-gui
"#;

/// vinoOpenEffects' uninstaller ships in the repo, not in ~/.local: use the
/// checkout's, or a shallow clone's.
const OPENEFFECTS_UNINSTALL: &str = r#"if [ -x "$CHECKOUT/scripts/uninstall.sh" ]; then
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

/// The steps for `action` on `id` installed (or to be installed) through
/// `channel`. Empty when that combination doesn't exist. Updating or
/// uninstalling vinoPilot takes the terminal UI along. Uninstalling a
/// source install ends by moving its checkout to the trash.
pub fn steps(id: Id, channel: Channel, action: Action) -> Vec<Step> {
    let mut steps = app_steps(id, channel, action);
    if id == Id::Pilot && !steps.is_empty() {
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
            "need_uv; uv tool install vinowhisper; \"$BIN/vinowhisper-setup\" --gui --yes </dev/null",
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
            vec![s("need_uv; uv tool install --python 3.14 vinospeak")]
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
        (Id::Pilot, Release, Install | Update) => vec![s(
            r#"if [ -x "$BIN/vinopilot-upgrade" ]; then bash "$BIN/vinopilot-upgrade" </dev/null; else release_run install.sh; fi
drop_cargo_copies vinopilot vinopilot-gui"#,
        )],
        (Id::Pilot, Release, Uninstall) => vec![s(
            "release_run install.sh --uninstall; drop_cargo_copies vinopilot vinopilot-gui",
        )],
        (Id::Pilot, Source, Install | Update) => {
            vec![s(
                r#"checkout; need_cargo; bash "$CHECKOUT/install.sh" </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
            )]
        }
        (Id::Pilot, Source, Uninstall) => {
            vec![s(
                r#"bash "$CHECKOUT/install.sh" --uninstall </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
            )]
        }
        (Id::Pilot, Package, Install | Update) => {
            vec![s(
                r#"need_cargo; cargo install --locked --force vinopilot vinopilot-gui
rm -f "$BIN/vinopilot" "$BIN/vinopilot-gui" "$BIN/vinopilot-upgrade"
"$HOME/.cargo/bin/vinopilot-gui" --install-desktop"#,
            )]
        }
        (Id::Pilot, Package, Uninstall) => vec![s(
            "need_cargo; vinopilot-gui --uninstall-desktop || true; for c in vinopilot vinopilot-gui; do cargo uninstall \"$c\" || true; done",
        )],

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
                let pilot = id == Id::Pilot;
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
        for (id, bin) in [(Id::Ovfetch, "ovfetch"), (Id::Pilot, "vinopilot")] {
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
            script(Id::Pilot, Channel::Source, Action::Install)
                .contains("drop_cargo_copies vinopilot")
        );
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
    fn every_script_parses() {
        let Ok(bash) = std::process::Command::new("bash").arg("--version").output() else {
            return;
        };
        assert!(bash.status.success());
        for id in Id::ALL {
            for channel in id.channels() {
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
