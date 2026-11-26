//! The settings popover in the header: where source checkouts go.

use eframe::egui::{self, RichText};
use vino_ui::{Button, theme, widgets};
use vinopilot_core::catalog::Id;
use vinopilot_core::config;

use crate::{Pilot, tilde};

impl Pilot {
    pub(super) fn settings_popup(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = theme::palette(&ctx);
        let max_w = widgets::popup_width(&ctx, 320.0);
        ui.set_max_width(max_w);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.label(
            RichText::new("Settings")
                .size(15.0)
                .family(theme::semibold())
                .color(p.text),
        );
        widgets::small(ui, "Directory for source checkouts:");
        widgets::mono(
            ui,
            &tilde(&self.paths().checkout(Id::App(vino_ui::App::Search))),
        );
        ui.add_space(2.0);
        let wanted = config::expand(self.source_field.trim(), &config::home());
        let dirty = wanted != self.config.source_root;
        let busy = self.busy();
        let mut apply = false;
        let field_w = (max_w - 80.0).clamp(130.0, 220.0);
        ui.horizontal_wrapped(|ui| {
            let edit = ui.add_enabled(
                !busy,
                widgets::field(&mut self.source_field, "~/Development", field_w),
            );
            ctx.accesskit_node_builder(edit.id, |node| {
                node.set_label("Source folder");
            });
            if dirty && !busy && edit.lost_focus() {
                apply = true;
            }
            if ui
                .add_enabled(dirty && !busy, Button::tonal("Apply").small())
                .clicked()
            {
                apply = true;
            }
        });
        if apply {
            self.config.source_root = wanted;
            if let Err(e) = self.config.save() {
                eprintln!("vinopilot: couldn't save the config: {e}");
            }
            self.rescan(&ctx);
        }
    }
}
