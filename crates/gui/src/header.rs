//! The header: the title, vinoPilot's own version and update, About, and
//! Check for updates and Update all.

use eframe::egui::{self, RichText, vec2};
use vino_ui::{Button, ChipKind, theme, widgets};
use vinopilot_core::catalog::{Action, Id};
use vinopilot_core::detect::Installed;
use vinopilot_core::job::Outcome;
use vinopilot_core::status::Update;

use crate::Filter;
use crate::cards::{CARDS, PAGE_MAX};
use crate::{Pilot, VERSION, desktop, tilde};

/// Narrower than this, the header's actions go on a line of their own.
pub(super) const NARROW: f32 = 620.0;

impl Pilot {
    /// `bar`: the title line is in the window's bar, not here.
    pub(super) fn header(&mut self, ui: &mut egui::Ui, bar: bool) {
        let narrow = ui.available_width() < NARROW;
        let compact = vino_ui::SizeClass::for_ui(ui).is_compact();
        if !bar {
            widgets::responsive_bar(
                ui,
                NARROW,
                |ui| self.title_line(ui, !compact),
                |ui| self.actions(ui),
            );
        } else if narrow {
            ui.horizontal_wrapped(|ui| {
                self.actions(ui);
                if !compact {
                    site_link(ui);
                }
            });
        }
        widgets::muted(
            ui,
            "Each app installs using its own checksum-verified installer.",
        );
        ui.add_space(6.0);
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

    /// The logo, title, version button, ⓘ, settings gear and, with `site`, the site.
    pub(super) fn title_line(&mut self, ui: &mut egui::Ui, site: bool) {
        vino_ui::logo::pilot(ui, 24.0);
        widgets::title(ui, "vinoPilot");
        self.version_button(ui);
        self.about_button(ui);
        self.settings_button(ui);
        if site {
            site_link(ui);
        }
    }

    /// The window's version as a button: it checks for a newer vinoPilot,
    /// then updates it (through the same steps as the cards, for however it's
    /// installed), then restarts into the new one.
    fn version_button(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let busy = self.busy();
        let installed = self
            .local
            .get(&Id::App(vino_ui::App::Pilot))
            .and_then(|local| local.installed.clone());
        if let Some(newer) = self.updated_to(installed.as_ref()) {
            if ui
                .add(Button::primary(format!("Restart to use {newer}")).small())
                .on_hover_text(format!(
                    "vinoPilot {newer} is installed; currently running {VERSION}"
                ))
                .clicked()
            {
                restart(&ctx, self.flatpak);
            }
            return;
        }
        if self.checking.is_some() {
            ui.add_enabled(
                false,
                Button::ghost(format!("{VERSION} · checking…")).small(),
            );
            return;
        }
        let how = installed.as_ref().map_or_else(
            || "Running from an uninstalled binary".to_string(),
            |installed| {
                format!(
                    "Installed from {}",
                    installed.channel.label(Id::App(vino_ui::App::Pilot))
                )
            },
        );
        match (self.update(Id::App(vino_ui::App::Pilot)), installed) {
            (Update::Available { to }, Some(installed)) => {
                if ui
                    .add_enabled(
                        !busy,
                        Button::tonal(format!("{VERSION} · update to {to}")).small(),
                    )
                    .on_hover_text(format!("Update vinoPilot to {to}. {how}."))
                    .clicked()
                {
                    self.end_batch();
                    self.start(
                        &ctx,
                        Id::App(vino_ui::App::Pilot),
                        installed.channel,
                        Action::Update,
                    );
                }
            }
            (update, _) => {
                let (label, hover) = check_button_text(update);
                if ui
                    .add(Button::ghost(label).small())
                    .on_hover_text(format!("{hover}. {how}."))
                    .clicked()
                {
                    self.check(&ctx);
                }
            }
        }
    }

    /// ⓘ, always there: a popup with this window's version, how vinoPilot is
    /// installed, and which of its commands are.
    fn about_button(&self, ui: &mut egui::Ui) {
        let info = ui
            .add(Button::ghost("").icon(vino_ui::Icon::Info).small())
            .on_hover_text("About vinoPilot");
        info.ctx.accesskit_node_builder(info.id, |node| {
            node.set_label("About vinoPilot");
        });
        egui::Popup::from_toggle_button_response(&info)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        self.about(ui);
                    });
            });
    }

    /// Settings gear beside ⓘ: configures the source checkout directory without
    /// consuming vertical space below the catalog.
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

    fn about(&self, ui: &mut egui::Ui) {
        ui.set_max_width(widgets::popup_width(ui.ctx(), 380.0));
        let muted = theme::palette(ui.ctx()).muted;
        let dim = |ui: &mut egui::Ui, text: &str| {
            ui.add(egui::Label::new(RichText::new(text).color(muted)).extend())
        };
        let local = self
            .local
            .get(&Id::App(vino_ui::App::Pilot))
            .cloned()
            .unwrap_or_default();
        ui.label(
            RichText::new(format!("vinoPilot {VERSION}"))
                .strong()
                .size(15.0),
        );
        widgets::muted(ui, "Currently running version");
        ui.add_space(6.0);
        egui::Grid::new("about-vinopilot")
            .num_columns(2)
            .spacing(vec2(16.0, 6.0))
            .show(ui, |ui| {
                dim(ui, "Installed from");
                match &local.installed {
                    Some(installed) => {
                        let label = installed.channel.label(Id::App(vino_ui::App::Pilot));
                        match installed.version.as_deref() {
                            Some(version) if version != VERSION => {
                                ui.label(format!("{label}, {version}"))
                            }
                            _ => ui.label(label),
                        };
                    }
                    None if self.scanning.is_some() => {
                        dim(ui, "Scanning…");
                    }
                    None => {
                        dim(
                            ui,
                            "Not found in ~/.local/bin, ~/.cargo/bin, or system packages",
                        );
                    }
                }
                ui.end_row();
                if let Some(checkout) = &local.checkout {
                    dim(ui, "Checkout");
                    ui.label(format!("{} ({})", tilde(&checkout.path), checkout.describe));
                    ui.end_row();
                }
                dim(ui, "Updates");
                match self.update(Id::App(vino_ui::App::Pilot)) {
                    _ if self.checking.is_some() => dim(ui, "Checking…"),
                    Update::Available { to } => ui.label(format!("{to} available")),
                    Update::UpToDate => ui.label("Up to date"),
                    Update::Unknown { reason: Some(why) } => dim(ui, &why),
                    Update::Unknown { reason: None } => dim(ui, "Not checked yet"),
                };
                ui.end_row();
                for part in &self.parts {
                    dim(ui, part_name(part.command));
                    match &part.path {
                        Some(path) => {
                            let version = part.version.as_deref().unwrap_or("version unknown");
                            ui.label(format!("{} ({version})", tilde(path)));
                        }
                        None if part.command == "vinopilot-tui" => {
                            dim(
                                ui,
                                "Not installed (install with: uv tool install vinopilot)",
                            );
                        }
                        None => {
                            dim(ui, "Not installed");
                        }
                    }
                    ui.end_row();
                }
            });
    }

    /// The version vinoPilot's own update just installed, if it isn't this
    /// window's.
    fn updated_to(&self, installed: Option<&Installed>) -> Option<String> {
        let job = self.job.as_ref()?;
        let done = job.id == Id::App(vino_ui::App::Pilot)
            && job.action == Action::Update
            && job.outcome() == Outcome::Done;
        let version = installed?.version.clone()?;
        (done && self.scanning.is_none() && version != VERSION).then_some(version)
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

/// The version button's label and hover when there's nothing to update
/// to: pressing it checks, or checks again.
fn check_button_text(update: Update) -> (String, String) {
    match update {
        Update::UpToDate => (format!("{VERSION} · up to date"), "Check again".into()),
        Update::Unknown { reason: Some(why) } => {
            (VERSION.to_string(), format!("{why}. Check again"))
        }
        _ => (VERSION.to_string(), "Check for vinoPilot updates".into()),
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

/// What each of vinoPilot's commands is, for the About popup.
fn part_name(command: &str) -> &'static str {
    match command {
        "vinopilot" => "CLI",
        "vinopilot-gui" => "Desktop app",
        "vinopilot-tui" => "Terminal UI",
        _ => "Other",
    }
}

/// Starts the installed window and closes this one.
fn restart(ctx: &egui::Context, flatpak: bool) {
    let mut new = if flatpak {
        let mut run = vinopilot_core::command("flatpak");
        run.args(["run", desktop::APP_ID]);
        run
    } else {
        vinopilot_core::command("vinopilot-gui")
    };
    match new.spawn() {
        Ok(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        Err(e) => eprintln!("vinopilot: couldn't start the new vinopilot-gui: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_button_says_up_to_date_only_after_a_check_that_found_nothing() {
        let (label, hover) = check_button_text(Update::UpToDate);
        assert_eq!(label, format!("{VERSION} · up to date"));
        assert_eq!(hover, "Check again");
        let (label, hover) = check_button_text(Update::Unknown { reason: None });
        assert_eq!(label, VERSION);
        assert_eq!(hover, "Check for vinoPilot updates");
    }

    #[test]
    fn the_version_button_gives_a_failed_checks_reason_on_hover() {
        let failed = Update::Unknown {
            reason: Some("rate-limited; try again later".into()),
        };
        let (label, hover) = check_button_text(failed);
        assert_eq!(label, VERSION);
        assert_eq!(hover, "rate-limited; try again later. Check again");
    }
}
