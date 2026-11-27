//! vinoPilot appears alongside the other apps in the catalog grid, with the
//! same install/update/uninstall card logic and no About popup in the header.

mod support;

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::App;
use vinopilot_core::catalog::{self, Channel, Id};
use vinopilot_core::detect::{Local, SystemPackage};
use vinopilot_core::remote::Latest;
use vinopilot_gui::Pilot;

const INSTALLED: &str = env!("CARGO_PKG_VERSION");

fn window(local: Local, latest: HashMap<Id, Latest>) -> Harness<'static, (Pilot, bool)> {
    support::window(HashMap::from([(Id::App(App::Pilot), local)]), latest)
}

#[test]
fn vinopilot_appears_as_a_card_without_an_about_popup() {
    let h = window(
        support::installed(Channel::Release, INSTALLED),
        HashMap::new(),
    );
    assert!(h.query_by_label("About vinoPilot").is_none());
    h.get_by_label(Id::App(App::Pilot).blurb());
    h.get_by_label(INSTALLED);
    h.get_by_label("Update");
    h.get_by_label("Uninstall…");
}

#[test]
fn a_newer_release_shows_on_the_vinopilot_card() {
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok("v99.0.0".into())),
            ..Default::default()
        },
    )]);
    let h = window(support::installed(Channel::Release, INSTALLED), latest);
    h.get_by_label(&format!("{INSTALLED} · v99.0.0 available"));
    h.get_by_label("Update all (1)");
}

#[test]
fn an_up_to_date_check_shows_on_the_vinopilot_card() {
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok(format!("v{INSTALLED}"))),
            ..Default::default()
        },
    )]);
    let h = window(support::installed(Channel::Release, INSTALLED), latest);
    h.get_by_label(&format!("{INSTALLED} · up to date"));
}

#[test]
fn a_deb_install_shows_its_package_tag_on_the_vinopilot_card() {
    let local = Local {
        system_package: Some(SystemPackage {
            kind: "deb",
            packages: vec!["vinopilot".into()],
            remove: "sudo apt remove vinopilot".into(),
        }),
        ..support::installed(Channel::Deb, INSTALLED)
    };
    let latest = HashMap::from([(
        Id::App(App::Pilot),
        Latest {
            release: Some(Ok("v99.0.0".into())),
            ..Default::default()
        },
    )]);
    let h = window(local, latest);
    h.get_by_label(&format!("{INSTALLED} · v99.0.0 available"));
    h.get_by_label("Copy removal command for the deb package");
}

#[test]
fn uninstalling_vinopilot_confirms_other_apps_stay() {
    let mut h = window(
        support::installed(Channel::Release, INSTALLED),
        HashMap::new(),
    );
    let note = catalog::uninstall_note(Id::App(App::Pilot), Channel::Release);
    assert!(h.query_by_label(&note).is_none());

    h.get_by_label("Uninstall…").click();
    h.run();
    h.get_by_label(&note);
}
