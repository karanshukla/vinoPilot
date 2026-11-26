//! The catalog grid: user-facing apps and developer/runtime tools, each with
//! its logo, status, split Install button or Update/Uninstall actions, and
//! inline confirmation or running-job feedback.

use eframe::egui::{self, RichText, vec2};
use vino_ui::{Button, ChipKind, Icon, Tone, a11y, logo, theme, widgets};
use vinopilot_core::catalog::{self, Action, Channel, Id};
use vinopilot_core::detect::{Installed, Local, SystemPackage};
use vinopilot_core::status::Update;

use crate::{Filter, Pilot, VERSION, tilde};

/// Up to three columns wide; drops to two, then one.
const MAX_COLUMNS: usize = 3;
/// Narrowest comfortable card width for a logo, title, tags, and split button.
const CARD_MIN: f32 = 276.0;
/// Minimum cell height so adjacent cards in a row align at 1x scale without
/// clipping taller content at 2x text scale.
const CARD_MIN_HEIGHT: f32 = 138.0;
pub(super) const GAP: f32 = 14.0;
/// The grid stops growing here, so cards don't stretch across a 4K screen.
pub(super) const PAGE_MAX: f32 = 3.0 * 340.0 + 2.0 * GAP;

/// End-user vinoPilot apps, in vinopilot.ca's order.
const APPS: [Id; 7] = [
    Id::App(vino_ui::App::Whisper),
    Id::App(vino_ui::App::AuthFace),
    Id::App(vino_ui::App::OpenEffects),
    Id::App(vino_ui::App::Pop),
    Id::App(vino_ui::App::Speak),
    Id::App(vino_ui::App::Search),
    Id::App(vino_ui::App::Watch),
];

/// Developer and runtime components managed alongside the apps.
const TOOLS: [Id; 2] = [Id::Ovfetch, Id::Ui];

/// Every card in catalog order. vinoPilot itself lives in the header.
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
    fn matches_filter(&self, id: Id) -> bool {
        match self.filter {
            Filter::All => true,
            Filter::Installed => self.local.get(&id).is_some_and(|l| l.installed.is_some()),
            Filter::Updates => matches!(self.update(id), Update::Available { .. }),
        }
    }

    pub(super) fn grid(&mut self, ui: &mut egui::Ui) {
        let apps: Vec<Id> = APPS
            .into_iter()
            .filter(|id| self.matches_filter(*id))
            .collect();
        let tools: Vec<Id> = TOOLS
            .into_iter()
            .filter(|id| self.matches_filter(*id))
            .collect();
        if apps.is_empty() && tools.is_empty() {
            let message = match self.filter {
                Filter::Updates => "Everything is up to date.",
                Filter::Installed => "No components are installed yet.",
                Filter::All => "No components found.",
            };
            widgets::empty_state(ui, message, |ui| {
                if ui.add(Button::ghost("Show all").small()).clicked() {
                    self.filter = Filter::All;
                }
            });
            return;
        }
        if !apps.is_empty() {
            self.card_rows(ui, &apps);
        }
        if !tools.is_empty() {
            widgets::group_label(ui, "Developer & runtime");
            ui.add_space(6.0);
            self.card_rows(ui, &tools);
        }
    }

    fn card_rows(&mut self, ui: &mut egui::Ui, ids: &[Id]) {
        let width = ui.available_width();
        let columns = columns_in(width);
        let card = (width - GAP * (columns - 1) as f32) / columns as f32;
        for chunk in ids.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                for id in chunk {
                    ui.allocate_ui_with_layout(
                        vec2(card, CARD_MIN_HEIGHT),
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
        let running_here = self
            .active_job()
            .filter(|(job_id, _)| *job_id == id)
            .map(|(_, action)| action);
        let busy = self.busy();
        let mut outcome = Outcome {
            intent: None,
            confirm: self.confirm,
        };
        let channels = id.channels();
        let looking = self.scanning.is_some() && self.local.is_empty();
        let now = ctx.input(|input| input.time);
        let copied_now = self
            .copied
            .is_some_and(|(copied, at)| copied == id && now - at < COPIED_FOR);
        if copied_now {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(COPIED_FOR));
        }
        let mut copy_clicked = false;
        let install_from = format!("Install {} options", id.name());

        let tinted = confirming || running_here.is_some();
        let margin = if tinted { 12.0 } else { 0.0 };
        let fill = if confirming {
            palette.error_tint
        } else if running_here.is_some() {
            palette.tint
        } else {
            egui::Color32::TRANSPARENT
        };
        let shown = egui::Frame::new()
            .fill(fill)
            .inner_margin(egui::Margin {
                left: margin as i8,
                right: margin as i8,
                top: 12,
                bottom: 12,
            })
            .show(ui, |ui| {
                ui.set_width(width - 2.0 * margin);
                ui.set_min_height(CARD_MIN_HEIGHT - 24.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                let top = ui.cursor().top();

                title_row(ui, id, &local, &update, copied_now, &mut copy_clicked);
                blurb(ui, id, &local, confirming);
                ui.add_space(2.0);
                status_line(ui, id, &local, &update, looking);
                checkout_note(ui, &local, confirming);

                let used = ui.cursor().top() - top;
                let pad = (CARD_MIN_HEIGHT - 24.0 - used - widgets::SMALL_HEIGHT).max(8.0);
                ui.add_space(pad);

                if let Some(action) = running_here {
                    ui.horizontal(|ui| {
                        widgets::spinner(ui);
                        widgets::chip(ui, ChipKind::Neutral, &format!("{}…", action.verb()));
                    });
                } else {
                    ui.add_enabled_ui(!busy && !looking, |ui| {
                        action_buttons(
                            ui,
                            id,
                            &local,
                            &update,
                            confirming,
                            &channels,
                            &install_from,
                            &mut outcome,
                        );
                    });
                }
            });
        let rect = shown.response.rect;
        let painter = ui.painter();
        if confirming {
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                0.0,
                palette.error,
            );
        } else if running_here.is_some() {
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                0.0,
                palette.accent,
            );
        }
        painter.hline(
            rect.left()..=rect.right(),
            rect.top(),
            egui::Stroke::new(1.0, palette.rule),
        );

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

fn component_badge(ui: &mut egui::Ui, id: Id) {
    match id {
        Id::App(app) => {
            logo::app(ui, app, 22.0);
        }
        Id::Ovfetch => {
            widgets::file_tile(ui, Icon::Hardware, 22.0);
        }
        Id::Ui => {
            widgets::file_tile(ui, Icon::Flask, 22.0);
        }
    }
}

fn title_row(
    ui: &mut egui::Ui,
    id: Id,
    local: &Local,
    update: &Update,
    copied_now: bool,
    copy_clicked: &mut bool,
) {
    ui.horizontal(|ui| {
        component_badge(ui, id);
        let name = ui.label(
            RichText::new(id.name())
                .size(15.0)
                .family(theme::semibold()),
        );
        a11y::heading(&name, id.name(), 3);
        app_hover_preview(name, id, local, update);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(installed) = &local.installed {
                widgets::tag(ui, Tone::Neutral, installed.channel.label(id));
            }
            if id.needs_root() {
                widgets::tag(ui, Tone::Neutral, "system")
                    .on_hover_text("Installs system-wide and prompts for your password");
            }
            if let Some(shadowed) = local.shadowed.first() {
                widgets::tag(ui, Tone::Warn, "2 copies").on_hover_text(format!(
                    "{} is active; {} is shadowed by it.\n\
                     Uninstall and reinstall to keep a single copy.",
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

fn app_hover_preview(response: egui::Response, id: Id, local: &Local, update: &Update) {
    let desc = id.blurb().to_owned();
    let status = match (&local.installed, update) {
        (Some(inst), Update::Available { to }) => {
            format!("Installed via {} · {to} available", inst.channel.label(id))
        }
        (Some(inst), Update::UpToDate) => {
            format!("Installed via {} · up to date", inst.channel.label(id))
        }
        (Some(inst), Update::Unknown { reason: Some(why) }) => {
            format!("Installed via {} · {why}", inst.channel.label(id))
        }
        (Some(inst), Update::Unknown { reason: None }) => {
            format!("Installed via {}", inst.channel.label(id))
        }
        (None, _) => "Not installed".to_owned(),
    };
    let checkout = local
        .checkout
        .as_ref()
        .map(|c| format!("{} · {}", tilde(&c.path), c.describe));
    widgets::hover_card(response, id.name(), move |ui| {
        widgets::small(ui, desc);
        widgets::small(ui, status);
        if let Some(path) = &checkout {
            widgets::mono(ui, path);
        }
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
    if let Some(checkout) = local.checkout.as_ref().filter(|_| !confirming) {
        let text = format!("{} · {}", tilde(&checkout.path), checkout.describe);
        widgets::mono(ui, &text).on_hover_text(format!("Source checkout: {text}"));
    }
}

#[allow(clippy::too_many_arguments)]
fn action_buttons(
    ui: &mut egui::Ui,
    id: Id,
    local: &Local,
    update: &Update,
    confirming: bool,
    channels: &[Channel],
    install_from: &str,
    outcome: &mut Outcome,
) {
    ui.horizontal_wrapped(|ui| match &local.installed {
        None => {
            let default_channel = channels[0];
            if channels.len() > 1 {
                let split = widgets::split_button(
                    ui,
                    Button::tonal("Install").small(),
                    install_from,
                    |ui| {
                        ui.set_min_width(170.0);
                        for &channel in channels {
                            let label = format!("Install from {}", channel.label(id));
                            if ui.button(label).clicked() {
                                outcome.intent = Some((channel, Action::Install));
                                ui.close();
                            }
                        }
                    },
                );
                if split
                    .primary
                    .on_hover_text(format!("Install from {}", default_channel.label(id)))
                    .clicked()
                {
                    outcome.intent = Some((default_channel, Action::Install));
                }
            } else if ui
                .add(Button::tonal("Install").small())
                .on_hover_text(format!("Install from {}", default_channel.label(id)))
                .clicked()
            {
                outcome.intent = Some((default_channel, Action::Install));
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
                Button::tonal("Update")
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
}

/// Three columns when three cards of `CARD_MIN` fit, two when two fit, else one
/// (pinned by `tests::three_cards_that_fit_are_three_columns`,
/// `tests::two_cards_that_fit_are_two_columns`, and
/// `tests::a_narrow_window_is_one_column`).
fn columns_in(width: f32) -> usize {
    for cols in (2..=MAX_COLUMNS).rev() {
        if width + GAP >= cols as f32 * (CARD_MIN + GAP) {
            return cols;
        }
    }
    1
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
            "{} was installed by your system package manager and cannot be removed by vinoPilot.\n\
             Click to copy the removal command:\n{}",
            system.packages.join(", "),
            system.remove
        ));
    let name = if copied {
        format!("Copied removal command for the {} package", system.kind)
    } else {
        format!("Copy removal command for the {} package", system.kind)
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
                "Scanning…"
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
        Update::UpToDate => (format!("{version} · up to date"), palette.muted),
        Update::Unknown { reason: Some(why) } => (format!("{version} · {why}"), palette.muted),
        Update::Unknown { reason: None } => (version.to_string(), palette.text),
    };
    let shown = ui.add(egui::Label::new(RichText::new(text).size(13.0).color(color)).truncate());
    if id == Id::App(vino_ui::App::Pilot) && raw.as_deref() != Some(VERSION) {
        shown.on_hover_text(format!("Currently running version {VERSION}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_cards_that_fit_are_three_columns() {
        let fits = MAX_COLUMNS as f32 * CARD_MIN + (MAX_COLUMNS - 1) as f32 * GAP;
        assert_eq!(columns_in(fits), 3);
        assert_eq!(columns_in(PAGE_MAX), 3);
    }

    #[test]
    fn two_cards_that_fit_are_two_columns() {
        let three = 3.0 * CARD_MIN + 2.0 * GAP;
        let two = 2.0 * CARD_MIN + GAP;
        assert_eq!(columns_in(three - 1.0), 2);
        assert_eq!(columns_in(two), 2);
    }

    #[test]
    fn a_narrow_window_is_one_column() {
        let two = 2.0 * CARD_MIN + GAP;
        assert_eq!(columns_in(two - 1.0), 1);
        assert_eq!(columns_in(420.0), 1);
    }
}
