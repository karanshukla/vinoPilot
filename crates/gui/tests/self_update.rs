//! The header's version button: plain until a check, then the update.

use std::collections::HashMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use vino_ui::theme;
use vinopilot_core::Config;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Installed, Local};
use vinopilot_core::remote::Latest;
use vinopilot_gui::{Pilot, VERSION};

fn header(latest: HashMap<Id, Latest>) -> Harness<'static, (Pilot, bool)> {
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
            (Pilot::with(config, local, latest), false),
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
