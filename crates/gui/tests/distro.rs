//! A distro package's tag, which copies the command that removes it.

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::{App, egui, theme};
use vinopilot_core::Config;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Installed, Local, SystemPackage};
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
            installed: Some(Installed {
                channel: Channel::Source,
                version: Some("4".into()),
            }),
            checkout: None,
            system_package: Some(SystemPackage {
                kind: "rpm",
                packages: vec!["vinoopeneffects".into()],
                remove: REMOVE.into(),
            }),
            shadowed: vec![],
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
            (Pilot::with(config, local, HashMap::new()), false),
        );
    h.run();
    assert!(!copied(&h));

    h.get_by_label("Copy the command that removes the rpm package")
        .click();
    h.step();
    assert!(copied(&h));
    h.step();
    h.get_by_label("Copied the command that removes the rpm package");
}
