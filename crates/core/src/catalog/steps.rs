//! The steps of each (component, channel, action). One function per
//! component, each a table of its ways in; docs/how-it-works.md says what
//! each app's installer expects.

use vino_ui::App;

use super::scripts::*;
use super::{Action, Channel, Id, Step};

use Action::{Install, Uninstall, Update};
use Channel::{Deb, Flatpak, Package, Release, Rpm, Source};

/// The steps for `action` on `id` installed (or to be installed) through
/// `channel`. Empty when that combination doesn't exist. Updating or
/// uninstalling vinoPilot takes the terminal UI along (pinned by
/// `catalog::tests::the_terminal_ui_follows_vinopilots_updates_and_uninstall`).
/// Uninstalling a source install ends by moving its checkout to the trash
/// (pinned by `catalog::tests::uninstalling_from_source_trashes_the_checkout_last`);
/// installing or updating one ends with `checkout_done`.
pub fn steps(id: Id, channel: Channel, action: Action) -> Vec<Step> {
    let mut steps = component_steps(id, channel, action);
    if steps.is_empty() {
        return steps;
    }
    if id == Id::App(App::Pilot) {
        match action {
            Install => {}
            Update => steps.push(Step::user("follow_tui upgrade")),
            Uninstall => steps.push(Step::user("follow_tui uninstall")),
        }
    }
    match (channel, action) {
        (Source, Uninstall) if id != Id::Ui => steps.push(Step::user("trash_checkout")),
        (Source, Install | Update) => steps.push(Step::user("checkout_done")),
        _ => {}
    }
    steps
}

/// A Flatpak is matched before the apps' own tables, whose `(_, Uninstall)`
/// arms uninstall from the home (pinned by
/// `catalog::tests::a_flatpak_installs_and_uninstalls_its_app_id_in_one_step`).
fn component_steps(id: Id, channel: Channel, action: Action) -> Vec<Step> {
    if let (Flatpak, Some(app_id)) = (channel, id.flatpak()) {
        return flatpak_bundle(id, app_id, action);
    }
    match id {
        Id::App(App::Whisper) => whisper(channel, action),
        Id::App(App::Speak) => speak(channel, action),
        Id::App(App::Pop) => pop(channel, action),
        Id::App(App::Search) => search(channel, action),
        Id::App(App::Watch) => watch(channel, action),
        Id::App(App::OpenEffects) => open_effects(channel, action),
        Id::App(App::AuthFace) => auth_face(channel, action),
        Id::Ovfetch => ovfetch(channel, action),
        Id::App(App::Pilot) => pilot(channel, action),
        Id::Ui => user_interface(channel, action),
    }
}

fn as_user(script: &str) -> Vec<Step> {
    vec![Step::user(script)]
}

fn as_root(script: &str) -> Vec<Step> {
    vec![Step::root(script)]
}

fn no_such_way_in() -> Vec<Step> {
    Vec::new()
}

/// How a kept `<app>-upgrade` script is run: by its shebang, or handed to a
/// shell (vinoWatch's is POSIX `sh`).
#[derive(Clone, Copy)]
enum RunWith {
    Shebang,
    Bash,
    Sh,
}

/// The `<app>-upgrade` copy the release installer keeps in `~/.local/bin`,
/// or the release's `installer` again when it isn't there.
fn kept_upgrade_or(installer: &str, slug: &str, run_with: RunWith) -> String {
    let shell = match run_with {
        RunWith::Shebang => "",
        RunWith::Bash => "bash ",
        RunWith::Sh => "sh ",
    };
    format!(
        r#"if [ -x "$BIN/{slug}-upgrade" ]; then {shell}"$BIN/{slug}-upgrade" </dev/null; else release_run {installer}; fi"#
    )
}

/// A release's Flatpak bundle: checked, then installed over the old one,
/// with `$APP_ID`, `$ASSET` (the bundle's name in the release) and `$MODELS`
/// (where the app keeps its models under its data, as natively) set.
fn flatpak_bundle(id: Id, app_id: &str, action: Action) -> Vec<Step> {
    let body = match action {
        Install | Update => FLATPAK_INSTALL,
        Uninstall => FLATPAK_UNINSTALL,
    };
    let models = match id {
        Id::App(App::Whisper) => "vinowhisper/models",
        Id::App(App::Search) => "vinosearch/models",
        _ => "",
    };
    as_user(&format!(
        "{PACKAGE}APP_ID={app_id}\nMODELS={models}\nASSET={}-x86_64.flatpak\n{body}",
        id.slug()
    ))
}

fn whisper(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run install.sh --gui --yes"),
        (Release, Update) => as_user(&kept_upgrade_or(
            "install.sh",
            "vinowhisper",
            RunWith::Shebang,
        )),
        (Source, Install | Update) => {
            as_user(r#"checkout; "$CHECKOUT/scripts/install.sh" --yes --gui </dev/null"#)
        }
        (Package, Install) => as_user(
            r#"need_uv; uv tool install --force vinowhisper; rm -f "$BIN/vinowhisper-upgrade"; "$BIN/vinowhisper-setup" --gui --yes </dev/null"#,
        ),
        (Package, Update) => as_user(
            "need_uv; uv tool upgrade vinowhisper; systemctl --user try-restart vinowhisper-server.service 2>/dev/null || true",
        ),
        (_, Uninstall) => as_user(WHISPER_UNINSTALL),
        _ => no_such_way_in(),
    }
}

fn speak(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run install.sh --gui"),
        (Release, Update) => as_user(&kept_upgrade_or(
            "install.sh",
            "vinospeak",
            RunWith::Shebang,
        )),
        (Source, Install | Update) => as_user(&format!("checkout\n{SPEAK_SOURCE}")),
        (Package, Install) => as_user(
            r#"need_uv; uv tool install --force --python 3.14 vinospeak; rm -f "$BIN/vinospeak-upgrade""#,
        ),
        (Package, Update) => as_user("need_uv; uv tool upgrade vinospeak"),
        (_, Uninstall) => as_user(SPEAK_UNINSTALL),
        _ => no_such_way_in(),
    }
}

fn pop(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run get.sh"),
        (Release, Update) => as_user(&kept_upgrade_or("get.sh", "vinopop", RunWith::Shebang)),
        (Release, Uninstall) => as_user(
            r#"if [ -x "$BIN/vinopop-upgrade" ]; then "$BIN/vinopop-upgrade" --uninstall </dev/null; else release_run get.sh --uninstall; fi"#,
        ),
        (Source, Install | Update) => {
            as_user(r#"checkout; bash "$CHECKOUT/install.sh" </dev/null"#)
        }
        (Source, Uninstall) => as_user(r#"bash "$CHECKOUT/install.sh" --uninstall </dev/null"#),
        _ => no_such_way_in(),
    }
}

fn search(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run install.sh"),
        (Release, Update) => as_user(&kept_upgrade_or(
            "install.sh",
            "vinosearch",
            RunWith::Shebang,
        )),
        (Release, Uninstall) => as_user(
            "release_run install.sh --uninstall; drop_cargo_copies vinosearch vinosearch-settings",
        ),
        (Source, Install | Update) => {
            as_user(r#"checkout; need_cargo; "$CHECKOUT/scripts/install.sh" </dev/null"#)
        }
        (Source, Uninstall) => as_user(
            r#""$CHECKOUT/scripts/install.sh" --uninstall </dev/null; drop_cargo_copies vinosearch vinosearch-settings"#,
        ),
        _ => no_such_way_in(),
    }
}

fn watch(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run install.sh"),
        (Release, Update) => as_user(&kept_upgrade_or("install.sh", "vinowatch", RunWith::Sh)),
        (Release, Uninstall) => {
            as_user("release_run install.sh --uninstall; drop_cargo_copies vinowatch")
        }
        (Source, Install | Update) => {
            as_user(r#"checkout; need_cargo; sh "$CHECKOUT/install.sh" </dev/null"#)
        }
        (Source, Uninstall) => as_user(
            r#"sh "$CHECKOUT/install.sh" --uninstall </dev/null; drop_cargo_copies vinowatch"#,
        ),
        _ => no_such_way_in(),
    }
}

fn open_effects(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install) => as_user("release_run install.sh"),
        (Release, Update) => as_user(&kept_upgrade_or(
            "install.sh",
            "vinoopeneffects",
            RunWith::Bash,
        )),
        (Source, Install | Update) => {
            as_user(r#"checkout; need_cargo; "$CHECKOUT/scripts/install.sh" </dev/null"#)
        }
        (_, Uninstall) => as_user(OPENEFFECTS_UNINSTALL),
        _ => no_such_way_in(),
    }
}

/// A component's release packages: `$ALWAYS` and `$OPTIONAL` set, then
/// `body`, all as root.
fn release_packages(id: Id, body: &str) -> Vec<Step> {
    let Some(packages) = id.release_packages() else {
        return no_such_way_in();
    };
    as_root(&format!(
        "{PACKAGE}ALWAYS=\"{}\"\nOPTIONAL=\"{}\"\n{body}",
        packages.always.join(" "),
        packages.optional.join(" ")
    ))
}

/// Root throughout, but the checkout is pulled as the user so it stays the
/// user's (pinned by
/// `catalog::tests::vinoauthfaces_checkout_is_pulled_as_the_user_then_deployed_as_root`).
/// Its packages are the whole step as root too (it is root for every other
/// way in), and an install asks them to set it up as the release installer
/// does (pinned by
/// `catalog::tests::installing_vinoauthfaces_package_sets_it_up_and_updating_does_not_enable_anything`).
fn auth_face(channel: Channel, action: Action) -> Vec<Step> {
    let me = Id::App(App::AuthFace);
    match (channel, action) {
        (Deb, Install) => release_packages(me, &format!("{AUTO_SETUP}\n{RELEASE_DEBS}")),
        (Rpm, Install) => release_packages(me, &format!("{AUTO_SETUP}\n{RELEASE_RPMS}")),
        (Deb, Update) => release_packages(me, RELEASE_DEBS),
        (Rpm, Update) => release_packages(me, RELEASE_RPMS),
        (Deb, Uninstall) => release_packages(me, RELEASE_DEBS_UNINSTALL),
        (Rpm, Uninstall) => release_packages(me, RELEASE_RPMS_UNINSTALL),
        (Release, Install) => as_root("release_run install.sh"),
        (Release, Update) => as_root(
            r#"if [ -x /usr/local/bin/vinoauthface-upgrade ]; then /usr/local/bin/vinoauthface-upgrade </dev/null; else release_run install.sh; fi"#,
        ),
        (Source, Install | Update) => vec![
            Step::user("checkout"),
            Step::root(r#"cd "$(source_dir)"; bash ./deploy.sh </dev/null"#),
        ],
        (_, Uninstall) => as_root(
            r#"if [ -x /usr/local/bin/vinoauthface-uninstall ]; then /usr/local/bin/vinoauthface-uninstall; else bash "$CHECKOUT/uninstall.sh"; fi"#,
        ),
        _ => no_such_way_in(),
    }
}

/// Each install drops the other channel's copy, which would otherwise shadow
/// it or be what detection finds (pinned by
/// `catalog::tests::installing_ovfetch_or_vinopilot_drops_the_other_channels_copy`).
fn ovfetch(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install | Update) => {
            as_user(&format!("{OVFETCH_RELEASE}drop_cargo_copies ovfetch\n"))
        }
        (Release, Uninstall) => as_user(
            r#"rm -f "$BIN/ovfetch"; echo "Removed $BIN/ovfetch"; drop_cargo_copies ovfetch"#,
        ),
        (Source, Install | Update) => as_user(
            r#"checkout; need_cargo; cargo install --locked --force --path "$CHECKOUT"; rm -f "$BIN/ovfetch""#,
        ),
        (Package, Install | Update) => {
            as_user(r#"need_cargo; cargo install --locked --force ovfetch; rm -f "$BIN/ovfetch""#)
        }
        (Source | Package, Uninstall) => {
            as_user(r#"drop_cargo_copies ovfetch; rm -f "$BIN/ovfetch""#)
        }
        _ => no_such_way_in(),
    }
}

fn pilot(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Release, Install | Update) => as_user(&format!(
            "{}\ndrop_cargo_copies vinopilot vinopilot-gui",
            kept_upgrade_or("install.sh", "vinopilot", RunWith::Bash)
        )),
        (Release, Uninstall) => {
            as_user("release_run install.sh --uninstall; drop_cargo_copies vinopilot vinopilot-gui")
        }
        (Source, Install | Update) => as_user(
            r#"checkout; need_cargo; bash "$CHECKOUT/install.sh" </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
        ),
        (Source, Uninstall) => as_user(
            r#"bash "$CHECKOUT/install.sh" --uninstall </dev/null; drop_cargo_copies vinopilot vinopilot-gui"#,
        ),
        (Package, Install | Update) => as_user(
            r#"need_cargo; cargo install --locked --force vinopilot vinopilot-gui
rm -f "$BIN/vinopilot" "$BIN/vinopilot-gui" "$BIN/vinopilot-upgrade"
"$HOME/.cargo/bin/vinopilot-gui" --install-desktop"#,
        ),
        (Package, Uninstall) => as_user(
            "need_cargo; vinopilot-gui --uninstall-desktop || true; for c in vinopilot vinopilot-gui; do cargo uninstall \"$c\" || true; done",
        ),
        (Deb, Install | Update) => as_user(&format!("{PACKAGE}{PILOT_DEB}")),
        (Deb, Uninstall) => as_user(&format!("{PACKAGE}as_root apt-get remove -y vinopilot")),
        (Rpm, Install | Update) => as_user(&format!("{PACKAGE}{PILOT_RPM}")),
        (Rpm, Uninstall) => as_user(&format!("{PACKAGE}{PILOT_RPM_UNINSTALL}")),
        (Flatpak, _) => no_such_way_in(),
    }
}

/// A library, so "installed" means a checkout that builds.
fn user_interface(channel: Channel, action: Action) -> Vec<Step> {
    match (channel, action) {
        (Source, Install | Update) => as_user(
            r#"checkout; need_cargo; cd "$CHECKOUT"; cargo build --locked --example gallery; echo "Run it: cargo run --example gallery (in $CHECKOUT)""#,
        ),
        (Source, Uninstall) => as_user("trash_checkout"),
        _ => no_such_way_in(),
    }
}
