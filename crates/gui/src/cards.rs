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
/// Wide enough for the four ways in of a card with a Flatpak, on a row of
/// their own.
const CARD_MIN: f32 = 290.0;
/// Narrower inside than this, four ways in go above Install, not beside it.
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
        let columns = if width + GAP >= COLUMNS as f32 * (CARD_MIN + GAP) {
            COLUMNS
        } else {
            1
        };
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
        let p = theme::palette(&ctx);
        let local = self.local.get(&id).cloned().unwrap_or_default();
        let update = self.update(id);
        let confirming = self.confirm == Some(id);
        let margin = 14.0;
        let busy = self.busy();
        let mut intent: Option<(Channel, Action)> = None;
        let mut confirm = self.confirm;
        let channels = id.channels();
        let choice = self.choice.entry(id).or_insert(channels[0]);
        let looking = self.scanning.is_some() && self.local.is_empty();
        let now = ctx.input(|i| i.time);
        let copied_now = self
            .copied
            .is_some_and(|(copied, at)| copied == id && now - at < COPIED_FOR);
        if copied_now {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(COPIED_FOR));
        }
        let mut copy_clicked = false;
        let options: Vec<(Channel, &str)> = channels.iter().map(|c| (*c, c.label(id))).collect();
        let ways_above = channels.len() > 3 && width - 2.0 * margin - 2.0 < WAYS_BESIDE;
        let install_from = format!("Install {} from", id.name());

        egui::Frame::new()
            .fill(if confirming { p.error_tint } else { p.surface })
            .stroke(egui::Stroke::new(1.0, p.rule))
            .corner_radius(widgets::corner(10.0))
            .inner_margin(egui::Margin::same(margin as i8))
            .show(ui, |ui| {
                ui.set_width(width - 2.0 * margin - 2.0);
                ui.set_height(CARD_HEIGHT - 2.0 * margin - 2.0);
                ui.spacing_mut().item_spacing.y = 4.0;

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
                            widgets::tag(ui, Tone::Neutral, "system").on_hover_text(
                                "Installs system-wide, so it asks for your password",
                            );
                        }
                        if let Some(s) = local.shadowed.first() {
                            widgets::tag(ui, Tone::Warn, "2 copies").on_hover_text(format!(
                                "{} runs; {} is another install it hides.\n\
                                 Uninstall and reinstall to keep one.",
                                tilde(&s.runs),
                                tilde(&s.hidden)
                            ));
                        }
                        if let Some(system) = &local.system_package
                            && distro_tag(ui, system, copied_now)
                        {
                            ui.ctx().copy_text(system.remove.clone());
                            copy_clicked = true;
                        }
                    });
                });
                match &local.installed {
                    // Confirming: what uninstalling removes and keeps, in
                    // place of the blurb.
                    Some(installed) if confirming => {
                        let note = catalog::uninstall_note(id, installed.channel);
                        ui.add(
                            egui::Label::new(RichText::new(note).size(12.5).color(p.error)).wrap(),
                        );
                    }
                    _ => {
                        widgets::muted(ui, id.blurb());
                    }
                }
                ui.add_space(2.0);
                status_line(ui, id, &local, &update, looking);
                // The note says where the checkout goes, in its place.
                if let Some(checkout) = local.checkout.as_ref().filter(|_| !confirming) {
                    let text = format!("{} · {}", tilde(&checkout.path), checkout.describe);
                    ui.add(
                        egui::Label::new(RichText::new(&text).size(12.0).color(p.muted)).truncate(),
                    )
                    .on_hover_text(format!("Source checkout: {text}"));
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_enabled_ui(!busy && !looking, |ui| {
                        ui.horizontal_wrapped(|ui| match &local.installed {
                            None => {
                                if ui.add(Button::primary("Install").small()).clicked() {
                                    intent = Some((*choice, Action::Install));
                                }
                                if channels.len() > 1 && !ways_above {
                                    widgets::segmented(ui, &install_from, choice, &options);
                                }
                            }
                            Some(installed) if confirming => {
                                let label = if id == Id::Ui {
                                    "Move to trash"
                                } else {
                                    "Uninstall"
                                };
                                if ui.add(Button::destructive(label).small()).clicked() {
                                    intent = Some((installed.channel, Action::Uninstall));
                                }
                                if ui.add(Button::ghost("Cancel").small()).clicked() {
                                    confirm = None;
                                }
                            }
                            Some(installed) => {
                                let button = if matches!(update, Update::Available { .. }) {
                                    Button::primary("Update")
                                } else {
                                    Button::secondary("Update")
                                };
                                if ui.add(button.small()).clicked() {
                                    intent = Some((installed.channel, Action::Update));
                                }
                                let label = if id == Id::Ui {
                                    "Trash checkout…"
                                } else {
                                    "Uninstall…"
                                };
                                if ui.add(Button::ghost(label).small()).clicked() {
                                    confirm = Some(id);
                                }
                            }
                        });
                        // Bottom-up: this lands above the row just drawn.
                        if local.installed.is_none() && ways_above {
                            widgets::segmented(ui, &install_from, choice, &options);
                        }
                    });
                });
            });

        self.confirm = confirm;
        if copy_clicked {
            self.copied = Some((id, now));
        }
        if let Some((channel, action)) = intent {
            self.queue.clear();
            self.start(&ctx, id, channel, action);
        }
    }
}

/// How long a distro tag says "copied".
const COPIED_FOR: f64 = 1.5;

/// `RPM` in the warn tag's colours, as a button: a distro package vinoPilot
/// doesn't remove, which copies the command that does. True when clicked.
fn distro_tag(ui: &mut egui::Ui, system: &SystemPackage, copied: bool) -> bool {
    let p = theme::palette(ui.ctx());
    let text = if copied { "copied" } else { system.kind };
    let button = egui::Button::new(
        RichText::new(text.to_uppercase())
            .size(10.5)
            .family(theme::bold())
            .color(p.warn),
    )
    .fill(p.warn_tint)
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
    let p = theme::palette(ui.ctx());
    let Some(Installed { version, .. }) = &local.installed else {
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
    let version = version.as_deref().unwrap_or("installed");
    let (text, color) = match update {
        Update::Available { to } => (format!("{version} · {to} available"), p.warn),
        Update::UpToDate => (format!("{version} · up to date"), p.accent),
        Update::Unknown { reason: Some(why) } => (format!("{version} · {why}"), p.muted),
        Update::Unknown { reason: None } => (version.to_string(), p.text),
    };
    let shown = ui.add(egui::Label::new(RichText::new(text).size(13.0).color(color)).truncate());
    if id == Id::App(vino_ui::App::Pilot) && version != VERSION {
        shown.on_hover_text(format!("This window is {VERSION}"));
    }
}
