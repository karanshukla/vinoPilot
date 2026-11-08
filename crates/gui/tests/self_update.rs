//! The header's version button (plain until a check, then the update) and
//! the About popup beside it.

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::theme;
use vinopilot_core::Config;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Installed, Local, Part};
use vinopilot_core::remote::Latest;
use vinopilot_gui::{Pilot, VERSION};

fn header(latest: HashMap<Id, Latest>) -> Harness<'static, (Pilot, bool)> {
    header_with(latest, Vec::new())
}

fn header_with(latest: HashMap<Id, Latest>, parts: Vec<Part>) -> Harness<'static, (Pilot, bool)> {
    let local = HashMap::from([(
        Id::Pilot,
        Local {
            installed: Some(Installed {
                channel: Channel::Release,
                version: Some(VERSION.into()),
            }),
            ..Default::default()
        },
    )]);
    let config = Config {
        source_root: "/nowhere".into(),
    };
    let mut h = Harness::builder()
        .with_size((1000.0, 720.0))
        .build_ui_state(
            |ui, (pilot, styled): &mut (Pilot, bool)| {
                if !*styled {
                    theme::apply(ui.ctx());
                    *styled = true;
                }
                pilot.ui(ui);
            },
            (Pilot::with(config, local, latest).with_parts(parts), false),
        );
    h.run();
    h
}

#[test]
fn the_version_is_a_button_before_any_check() {
    header(HashMap::new()).get_by_label(VERSION);
}

#[test]
fn a_newer_release_turns_it_into_the_update() {
    let latest = HashMap::from([(
        Id::Pilot,
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
        Id::Pilot,
        Latest {
            release: Some(Ok(format!("v{VERSION}"))),
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
        version: path.map(|_| VERSION.to_string()),
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
    h.get_by_label(&format!("/nowhere/.local/bin/vinopilot-gui ({VERSION})"));
    h.get_by_label("Not installed: uv tool install vinopilot");
}
