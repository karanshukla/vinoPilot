//! A distro package's tag, which copies the command that removes it.

mod support;

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::{App, egui};
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Local, SystemPackage};
use vinopilot_gui::Pilot;

const REMOVE: &str = "sudo dnf remove vinoopeneffects";

fn copied(h: &Harness<'_, (Pilot, bool)>) -> bool {
    h.output()
        .platform_output
        .commands
        .iter()
        .any(|c| matches!(c, egui::OutputCommand::CopyText(text) if text == REMOVE))
}

#[test]
fn the_distro_tag_copies_the_remove_command() {
    let local = HashMap::from([(
        Id::App(App::OpenEffects),
        Local {
            system_package: Some(SystemPackage {
                kind: "rpm",
                packages: vec!["vinoopeneffects".into()],
                remove: REMOVE.into(),
            }),
            ..support::installed(Channel::Source, "4")
        },
    )]);
    let mut h = support::window(local, HashMap::new());
    assert!(!copied(&h));

    h.get_by_label("Copy removal command for the rpm package")
        .click();
    h.step();
    assert!(copied(&h));
    h.step();
    h.get_by_label("Copied removal command for the rpm package");
}
