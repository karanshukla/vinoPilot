//! The header's version button (plain until a check, then the update) and
//! the About popup beside it.

mod support;

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::App;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Local, Part, SystemPackage};
use vinopilot_core::remote::Latest;
use vinopilot_gui::{Pilot, VERSION};

/// What the installed copy says. A test build is `dev`, which is never offered
/// an update, so these tests install a numbered one.
const INSTALLED: &str = env!("CARGO_PKG_VERSION");

fn header(latest: HashMap<Id, Latest>) -> Harness<'static, (Pilot, bool)> {
    header_with(latest, Vec::new())
}

fn header_with(latest: HashMap<Id, Latest>, parts: Vec<Part>) -> Harness<'static, (Pilot, bool)> {
    window(
        support::installed(Channel::Release, INSTALLED),
        latest,
        parts,
    )
}

/// The window with vinoPilot itself as `local`.
fn window(
    local: Local,
    latest: HashMap<Id, Latest>,
    parts: Vec<Part>,
) -> Harness<'static, (Pilot, bool)> {
    support::window(HashMap::from([(Id::App(App::Pilot), local)]), latest, parts)
}

#[test]
fn the_version_is_a_button_before_any_check() {
    header(HashMap::new()).get_by_label(VERSION);
}

#[test]
fn a_newer_release_turns_it_into_the_update() {
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok("v99.0.0".into())),
            ..Default::default()
        },
    )]);
    header(latest).get_by_label(&format!("{VERSION} · update to v99.0.0"));
}

#[test]
fn an_up_to_date_check_says_so() {
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok(format!("v{INSTALLED}"))),
            ..Default::default()
        },
    )]);
    header(latest).get_by_label(&format!("{VERSION} · up to date"));
}

#[test]
fn about_shows_how_vinopilot_and_its_commands_are_installed() {
    let part = |command, path: Option<&str>| Part {
        command,
        path: path.map(Into::into),
        version: path.map(|_| INSTALLED.to_string()),
    };
    let parts = vec![
        part("vinopilot", Some("/nowhere/.local/bin/vinopilot")),
        part("vinopilot-gui", Some("/nowhere/.local/bin/vinopilot-gui")),
        part("vinopilot-tui", None),
    ];
    let mut h = header_with(HashMap::new(), parts);
    assert!(h.query_by_label("Installed from").is_none());
    h.get_by_label("About vinoPilot").click();
    h.run();
    h.get_by_label("Installed from");
    h.get_by_label("Not checked yet");
    h.get_by_label(&format!("/nowhere/.local/bin/vinopilot-gui ({INSTALLED})"));
    h.get_by_label("Not installed: uv tool install vinopilot");
}

#[test]
fn a_deb_install_updates_from_the_header_like_any_other() {
    let local = Local {
        system_package: Some(SystemPackage {
            kind: "deb",
            packages: vec!["vinopilot".into()],
            remove: "sudo apt remove vinopilot".into(),
        }),
        ..support::installed(Channel::Deb, INSTALLED)
    };
    let parts = vec![Part {
        command: "vinopilot-gui",
        path: Some("/usr/bin/vinopilot-gui".into()),
        version: Some(INSTALLED.to_string()),
    }];
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok("v99.0.0".into())),
            ..Default::default()
        },
    )]);
    let mut h = window(local, latest, parts);
    h.get_by_label(&format!("{VERSION} · update to v99.0.0"));
    h.get_by_label("About vinoPilot").click();
    h.run();
    h.get_by_label(&format!("deb package, {INSTALLED}"));
    h.get_by_label(&format!("/usr/bin/vinopilot-gui ({INSTALLED})"));
}
