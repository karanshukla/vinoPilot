//! The grid of cards, one per component: its version, how it's installed,
//! and Install, Update and Uninstall.

use eframe::egui::{self, RichText, vec2};
use vino_ui::{Button, Tone, a11y, theme, widgets};
use vinopilot_core::catalog::{self, Action, Channel, Id};
use vinopilot_core::detect::{Installed, Local, SystemPackage};
use vinopilot_core::status::Update;

use crate::{Pilot, VERSION, tilde};

/// Nine cards, three by three; one column when three don't fit.
const COLUMNS: usize = 3;
/// Wide enough for a Flatpak card's four ways in on a row of their own.
const CARD_MIN: f32 = 290.0;
/// Narrower inside than this, four ways in go above Install.
const WAYS_BESIDE: f32 = 330.0;
const CARD_HEIGHT: f32 = 172.0;
pub(super) const GAP: f32 = 12.0;
/// The grid stops growing here, so cards don't stretch across a 4K screen.
pub(super) const PAGE_MAX: f32 = 3.0 * 340.0 + 2.0 * GAP;
/// The grid, in vinopilot.ca's order. vinoPilot itself has no card: its
/// version and update live in the header.
pub(super) const CARDS: [Id; 9] = [
    Id::App(vino_ui::App::Whisper),
    Id::App(vino_ui::App::AuthFace),
    Id::App(vino_ui::App::OpenEffects),
    Id::App(vino_ui::App::Pop),
    Id::App(vino_ui::App::Speak),
    Id::App(vino_ui::App::Search),
    Id::App(vino_ui::App::Watch),
    Id::Ovfetch,
    Id::Ui,
];

impl Pilot {
    pub(super) fn grid(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let columns = columns_in(width);
        let card = (width - GAP * (columns - 1) as f32) / columns as f32;
        for chunk in CARDS.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for id in chunk {
                    ui.allocate_ui_with_layout(
                        vec2(card, CARD_HEIGHT),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| self.card(ui, *id, card),
                    );
                }
            });
            ui.add_space(GAP);
        }
    }

    fn card(&mut self, ui: &mut egui::Ui, id: Id, width: f32) {
        let ctx = ui.ctx().clone();
        let palette = theme::palette(&ctx);
        let local = self.local.get(&id).cloned().unwrap_or_default();
        let update = self.update(id);
        let confirming = self.confirm == Some(id);
        let margin = 14.0;
        let busy = self.busy();
        let mut outcome = Outcome {
            intent: None,
            confirm: self.confirm,
        };
        let channels = id.channels();
        let choice = self.choice.entry(id).or_insert(channels[0]);
        let looking = self.scanning.is_some() && self.local.is_empty();
        let now = ctx.input(|input| input.time);
        let copied_now = self
            .copied
            .is_some_and(|(copied, at)| copied == id && now - at < COPIED_FOR);
        if copied_now {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(COPIED_FOR));
        }
        let mut copy_clicked = false;
        let options: Vec<(Channel, &str)> = channels.iter().map(|c| (*c, c.label(id))).collect();
        let ways_above = ways_go_above(channels.len(), width - 2.0 * margin - 2.0);
        let install_from = format!("Install {} from", id.name());

        egui::Frame::new()
            .fill(if confirming {
                palette.error_tint
            } else {
                palette.surface
            })
            .stroke(egui::Stroke::new(1.0, palette.rule))
            .corner_radius(widgets::corner(10.0))
            .inner_margin(egui::Margin::same(margin as i8))
            .show(ui, |ui| {
                ui.set_width(width - 2.0 * margin - 2.0);
                ui.set_height(CARD_HEIGHT - 2.0 * margin - 2.0);
                ui.spacing_mut().item_spacing.y = 4.0;

                title_row(ui, id, &local, copied_now, &mut copy_clicked);
                blurb(ui, id, &local, confirming);
                ui.add_space(2.0);
                status_line(ui, id, &local, &update, looking);
                checkout_note(ui, &local, confirming);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_enabled_ui(!busy && !looking, |ui| {
                        let ways = Ways {
                            choice,
                            options: &options,
                            label: &install_from,
                            above: ways_above,
                        };
                        action_buttons(ui, id, &local, &update, confirming, ways, &mut outcome);
                    });
                });
            });

        self.confirm = outcome.confirm;
        if copy_clicked {
            self.copied = Some((id, now));
        }
        if let Some((channel, action)) = outcome.intent {
            self.end_batch();
            self.start(&ctx, id, channel, action);
        }
    }
}

/// What a card's buttons asked for this frame.
struct Outcome {
    intent: Option<(Channel, Action)>,
    confirm: Option<Id>,
}

/// The ways in a card offers, and whether they sit above Install.
struct Ways<'a> {
    choice: &'a mut Channel,
    options: &'a [(Channel, &'a str)],
    label: &'a str,
    above: bool,
}

fn title_row(ui: &mut egui::Ui, id: Id, local: &Local, copied_now: bool, copy_clicked: &mut bool) {
    ui.horizontal(|ui| {
        let name = ui.label(
            RichText::new(id.name())
                .size(15.0)
                .family(theme::semibold()),
        );
        a11y::heading(&name, id.name(), 3);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(installed) = &local.installed {
                widgets::tag(ui, Tone::Neutral, installed.channel.label(id));
            }
            if id.needs_root() {
                widgets::tag(ui, Tone::Neutral, "system")
                    .on_hover_text("Installs system-wide, so it asks for your password");
            }
            if let Some(shadowed) = local.shadowed.first() {
                widgets::tag(ui, Tone::Warn, "2 copies").on_hover_text(format!(
                    "{} runs; {} is another install it hides.\n\
                     Uninstall and reinstall to keep one.",
                    tilde(&shadowed.runs),
                    tilde(&shadowed.hidden)
                ));
            }
            if let Some(system) = &local.system_package
                && distro_tag(ui, system, copied_now)
            {
                ui.ctx().copy_text(system.remove.clone());
                *copy_clicked = true;
            }
        });
    });
}

fn blurb(ui: &mut egui::Ui, id: Id, local: &Local, confirming: bool) {
    let palette = theme::palette(ui.ctx());
    match &local.installed {
        Some(installed) if confirming => {
            let note = catalog::uninstall_note(id, installed.channel);
            ui.add(egui::Label::new(RichText::new(note).size(12.5).color(palette.error)).wrap());
        }
        _ => {
            widgets::muted(ui, id.blurb());
        }
    }
}

fn checkout_note(ui: &mut egui::Ui, local: &Local, confirming: bool) {
    let palette = theme::palette(ui.ctx());
    if let Some(checkout) = local.checkout.as_ref().filter(|_| !confirming) {
        let text = format!("{} · {}", tilde(&checkout.path), checkout.describe);
        ui.add(egui::Label::new(RichText::new(&text).size(12.0).color(palette.muted)).truncate())
            .on_hover_text(format!("Source checkout: {text}"));
    }
}

fn action_buttons(
    ui: &mut egui::Ui,
    id: Id,
    local: &Local,
    update: &Update,
    confirming: bool,
    ways: Ways,
    outcome: &mut Outcome,
) {
    let Ways {
        choice,
        options,
        label: install_from,
        above: ways_above,
    } = ways;
    ui.horizontal_wrapped(|ui| match &local.installed {
        None => {
            if ui.add(Button::primary("Install").small()).clicked() {
                outcome.intent = Some((*choice, Action::Install));
            }
            if options.len() > 1 && !ways_above {
                widgets::segmented(ui, install_from, choice, options);
            }
        }
        Some(installed) if confirming => {
            let label = if id == Id::Ui {
                "Move to trash"
            } else {
                "Uninstall"
            };
            if ui.add(Button::destructive(label).small()).clicked() {
                outcome.intent = Some((installed.channel, Action::Uninstall));
            }
            if ui.add(Button::ghost("Cancel").small()).clicked() {
                outcome.confirm = None;
            }
        }
        Some(installed) => {
            let button = if matches!(update, Update::Available { .. }) {
                Button::primary("Update")
            } else {
                Button::secondary("Update")
            };
            if ui.add(button.small()).clicked() {
                outcome.intent = Some((installed.channel, Action::Update));
            }
            let label = if id == Id::Ui {
                "Trash checkout…"
            } else {
                "Uninstall…"
            };
            if ui.add(Button::ghost(label).small()).clicked() {
                outcome.confirm = Some(id);
            }
        }
    });
    // Bottom-up: this lands above the row just drawn.
    if local.installed.is_none() && ways_above {
        widgets::segmented(ui, install_from, choice, options);
    }
}

/// Three columns when three cards of `CARD_MIN` fit in `width`, else one.
fn columns_in(width: f32) -> usize {
    if width + GAP >= COLUMNS as f32 * (CARD_MIN + GAP) {
        COLUMNS
    } else {
        1
    }
}

/// Whether a card `inner` wide puts its ways in on a row above Install:
/// only four of them, and only when they don't fit beside it.
fn ways_go_above(ways: usize, inner: f32) -> bool {
    ways > 3 && inner < WAYS_BESIDE
}

/// How long a distro tag says "copied".
const COPIED_FOR: f64 = 1.5;

/// `RPM` in the warn tag's colours, as a button: a distro package vinoPilot
/// doesn't remove, which copies the command that does. True when clicked.
fn distro_tag(ui: &mut egui::Ui, system: &SystemPackage, copied: bool) -> bool {
    let palette = theme::palette(ui.ctx());
    let text = if copied { "copied" } else { system.kind };
    let button = egui::Button::new(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .family(theme::bold())
            .color(palette.warn),
    )
    .fill(palette.warn_tint)
    .stroke(egui::Stroke::NONE)
    .corner_radius(4.0)
    .min_size(vec2(0.0, 16.0));
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = vec2(6.0, 0.0);
            ui.spacing_mut().interact_size.y = 16.0;
            ui.add(button)
        })
        .inner
        .on_hover_text(format!(
            "{} came from your package manager, which vinoPilot doesn't uninstall.\n\
             Click to copy the command that does:\n{}",
            system.packages.join(", "),
            system.remove
        ));
    let name = if copied {
        format!(
            "Copied the command that removes the {} package",
            system.kind
        )
    } else {
        format!("Copy the command that removes the {} package", system.kind)
    };
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response.clicked()
}

/// "1.4.0 · v1.4.1 available", coloured by whether it needs the user.
fn status_line(ui: &mut egui::Ui, id: Id, local: &Local, update: &Update, looking: bool) {
    let palette = theme::palette(ui.ctx());
    let Some(Installed { version: raw, .. }) = &local.installed else {
        widgets::small(
            ui,
            if looking {
                "Looking…"
            } else {
                "Not installed"
            },
        );
        return;
    };
    let label = local.version_label();
    let version = label.as_deref().unwrap_or("installed");
    let (text, color) = match update {
        Update::Available { to } => (format!("{version} · {to} available"), palette.warn),
        Update::UpToDate => (format!("{version} · up to date"), palette.accent),
        Update::Unknown { reason: Some(why) } => (format!("{version} · {why}"), palette.muted),
        Update::Unknown { reason: None } => (version.to_string(), palette.text),
    };
    let shown = ui.add(egui::Label::new(RichText::new(text).size(13.0).color(color)).truncate());
    if id == Id::App(vino_ui::App::Pilot) && raw.as_deref() != Some(VERSION) {
        shown.on_hover_text(format!("This window is {VERSION}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_cards_that_fit_are_three_columns() {
        let fits = COLUMNS as f32 * CARD_MIN + (COLUMNS - 1) as f32 * GAP;
        assert_eq!(columns_in(fits), COLUMNS);
        assert_eq!(columns_in(PAGE_MAX), COLUMNS);
    }

    #[test]
    fn a_pixel_short_of_three_cards_is_one_column() {
        let fits = COLUMNS as f32 * CARD_MIN + (COLUMNS - 1) as f32 * GAP;
        assert_eq!(columns_in(fits - 1.0), 1);
        assert_eq!(columns_in(420.0), 1);
    }

    #[test]
    fn four_ways_in_go_above_install_on_a_card_too_narrow_for_them() {
        assert!(ways_go_above(4, WAYS_BESIDE - 1.0));
        assert!(!ways_go_above(4, WAYS_BESIDE));
    }

    #[test]
    fn three_ways_in_stay_beside_install_however_narrow() {
        assert!(!ways_go_above(3, WAYS_BESIDE - 1.0));
        assert!(!ways_go_above(1, 0.0));
    }
}
