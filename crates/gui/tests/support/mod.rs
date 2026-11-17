//! The window under test, shared by the integration tests.

use std::collections::HashMap;

use egui_kittest::Harness;
use vino_ui::theme;
use vinopilot_core::Config;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Installed, Local, Part};
use vinopilot_core::remote::Latest;
use vinopilot_gui::Pilot;

/// The window, styled and run once, showing exactly `local`, `latest` and
/// `parts`.
pub fn window(
    local: HashMap<Id, Local>,
    latest: HashMap<Id, Latest>,
    parts: Vec<Part>,
) -> Harness<'static, (Pilot, bool)> {
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

/// Installed through `channel` at `version`, and nothing else found.
pub fn installed(channel: Channel, version: &str) -> Local {
    Local {
        installed: Some(Installed {
            channel,
            version: Some(version.into()),
        }),
        ..Default::default()
    }
}
