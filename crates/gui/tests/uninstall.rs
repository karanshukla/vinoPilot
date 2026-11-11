//! A card's Uninstall… confirms in place, saying what goes and what stays.

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::{App, theme};
use vinopilot_core::Config;
use vinopilot_core::catalog::{self, Channel, Id};
use vinopilot_core::detect::{Installed, Local};
use vinopilot_gui::Pilot;

fn window(id: Id, channel: Channel) -> Harness<'static, (Pilot, bool)> {
    let local = HashMap::from([(
        id,
        Local {
            installed: Some(Installed {
                channel,
                version: Some("1.0.0".into()),
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
            (Pilot::with(config, local, HashMap::new()), false),
        );
    h.run();
    h
}

#[test]
fn confirming_an_uninstall_says_the_models_go() {
    let id = Id::App(App::Whisper);
    let mut h = window(id, Channel::Release);
    let note = catalog::uninstall_note(id, Channel::Release);
    assert!(note.contains("downloaded models"));
    assert!(h.query_by_label(&note).is_none());

    h.get_by_label("Uninstall…").click();
    h.run();
    h.get_by_label(&note);
    assert!(h.query_by_label(id.blurb()).is_none());

    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.query_by_label(&note).is_none());
    h.get_by_label(id.blurb());
}

#[test]
fn a_source_install_also_says_its_checkout_goes() {
    let id = Id::App(App::Pop);
    let mut h = window(id, Channel::Source);
    h.get_by_label("Uninstall…").click();
    h.run();
    let note = catalog::uninstall_note(id, Channel::Source);
    assert!(note.ends_with("Moves the checkout to the trash."));
    h.get_by_label(&note);
}
