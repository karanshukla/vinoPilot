//! The header: the title, settings, and Check for updates / Update all.

use eframe::egui;
use vino_ui::{Button, ChipKind, widgets};
use vinopilot_core::catalog::Id;

use crate::cards::{CARDS, PAGE_MAX};
use crate::{Filter, Pilot};

/// Narrower than this, the header's actions go on a line of their own.
pub(super) const NARROW: f32 = 620.0;

impl Pilot {
    /// `bar`: the title line is in the window's bar, not here.
    pub(super) fn header(&mut self, ui: &mut egui::Ui, bar: bool) {
        let narrow = ui.available_width() < NARROW;
        let compact = vino_ui::SizeClass::for_ui(ui).is_compact();
        if !bar {
            let pilot = std::cell::RefCell::new(self);
            widgets::responsive_bar(
                ui,
                NARROW,
                |ui| pilot.borrow_mut().title_line(ui, !compact),
                |ui| pilot.borrow_mut().actions(ui),
            );
            ui.add_space(6.0);
        } else if narrow {
            ui.horizontal_wrapped(|ui| {
                self.actions(ui);
                if !compact {
                    site_link(ui);
                }
            });
            ui.add_space(6.0);
        }
    }

    /// Connected filter buttons (`All`, `Installed`, `Updates`) above the grid.
    pub(super) fn filter_bar(&mut self, ui: &mut egui::Ui) {
        let total = CARDS.len();
        let installed = CARDS
            .into_iter()
            .filter(|id| self.local.get(id).is_some_and(|l| l.installed.is_some()))
            .count();
        let updates = self.updatable().len();
        let all_label = format!("All ({total})");
        let installed_label = format!("Installed ({installed})");
        let updates_label = format!("Updates ({updates})");
        widgets::button_group(
            ui,
            "Filter components",
            &mut self.filter,
            &[
                (Filter::All, all_label.as_str()),
                (Filter::Installed, installed_label.as_str()),
                (Filter::Updates, updates_label.as_str()),
            ],
        );
    }

    /// The logo, title, settings gear and, with `site`, the site.
    pub(super) fn title_line(&mut self, ui: &mut egui::Ui, site: bool) {
        vino_ui::logo::pilot(ui, 24.0);
        widgets::title(ui, "vinoPilot");
        self.settings_button(ui);
        if site {
            site_link(ui);
        }
    }

    /// Settings gear beside the title: configures the source checkout directory
    /// without consuming vertical space below the catalog.
    fn settings_button(&mut self, ui: &mut egui::Ui) {
        let gear = ui
            .add(Button::ghost("").icon(vino_ui::Icon::Settings).small())
            .on_hover_text("Settings");
        gear.ctx.accesskit_node_builder(gear.id, |node| {
            node.set_label("Settings");
        });
        egui::Popup::from_toggle_button_response(&gear)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        self.settings_popup(ui);
                    });
            });
    }

    /// Check for updates and Update all. In a right-to-left layout the
    /// order flips, so Update all always sits at the outer edge.
    pub(super) fn actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rtl = ui.layout().main_dir() == egui::Direction::RightToLeft;
        let updatable = self.updatable();
        let busy = self.busy();
        let update_all = |ui: &mut egui::Ui, queue: &mut Vec<Id>, batch: &mut usize| {
            ui.add_enabled_ui(!busy, |ui| {
                if !updatable.is_empty()
                    && ui
                        .add(Button::primary(format!("Update all ({})", updatable.len())).small())
                        .clicked()
                {
                    *queue = updatable.clone();
                    *batch = updatable.len();
                }
            });
        };
        if rtl {
            update_all(ui, &mut self.queue, &mut self.batch);
        }
        if self.checking.is_some() {
            widgets::chip(ui, ChipKind::Neutral, "Checking…");
            widgets::spinner(ui);
        } else if ui
            .add(
                Button::secondary("Check for updates")
                    .icon(vino_ui::Icon::Refresh)
                    .small(),
            )
            .clicked()
        {
            self.check(&ctx);
        }
        if !rtl {
            update_all(ui, &mut self.queue, &mut self.batch);
        }
    }
}

fn site_link(ui: &mut egui::Ui) {
    if ui
        .add(Button::ghost(vino_ui::family::SITE_LABEL).small())
        .on_hover_text(vino_ui::family::SITE)
        .clicked()
        && let Err(e) = vino_ui::family::open(vino_ui::family::SITE)
    {
        eprintln!("vinopilot: couldn't open {}: {e}", vino_ui::family::SITE);
    }
}

/// The header's width in `available`: the cards', which sit inside the
/// scroll bar and its gap.
pub(super) fn header_width(ui: &egui::Ui, available: f32) -> f32 {
    let bar = ui.spacing().scroll.allocated_width();
    (available - bar - widgets::SCROLL_GAP).min(PAGE_MAX)
}
