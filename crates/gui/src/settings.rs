//! The settings group under the cards: where source checkouts go.

use eframe::egui;
use vino_ui::{Button, Row, widgets};
use vinopilot_core::catalog::Id;
use vinopilot_core::config;

use crate::{Pilot, tilde};

impl Pilot {
    pub(super) fn settings(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        widgets::group_label(ui, "Settings");
        let help = format!(
            "Directory for source checkouts, e.g. {}",
            tilde(&self.paths().checkout(Id::App(vino_ui::App::Search)))
        );
        widgets::group(ui, |ui| {
            Row::new("Source folder").help(&help).show(ui, |ui, named| {
                let wanted = config::expand(self.source_field.trim(), &config::home());
                if ui
                    .add_enabled(
                        wanted != self.config.source_root && !self.busy(),
                        Button::secondary("Save").small(),
                    )
                    .clicked()
                {
                    self.config.source_root = wanted;
                    if let Err(e) = self.config.save() {
                        eprintln!("vinopilot: couldn't save the config: {e}");
                    }
                    self.rescan(&ctx);
                }
                named(ui.add(widgets::field(
                    &mut self.source_field,
                    "~/Development",
                    220.0,
                )));
            });
        });
    }
}
