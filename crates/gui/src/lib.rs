//! The vinopilot window: a grid of cards, one per vinoPilot component, each
//! with Install, Update and Uninstall, and the installer's output in a panel
//! underneath. All the knowledge is in `vinopilot-core`; this only draws it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, RichText, vec2};
use vino_ui::{Button, ChipKind, Row, Tone, a11y, theme, widgets};
use vinopilot_core::catalog::{self, Action, Channel, Id};
use vinopilot_core::detect::{Installed, Local, Paths};
use vinopilot_core::job::{Env, Job, Outcome};
use vinopilot_core::remote::Latest;
use vinopilot_core::status::{self, Update};
use vinopilot_core::{Config, config, detect, remote};

pub mod desktop;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nine cards, three by three; one column when three don't fit.
const COLUMNS: usize = 3;
const CARD_MIN: f32 = 260.0;
const CARD_HEIGHT: f32 = 172.0;
const GAP: f32 = 12.0;
/// The grid stops growing here, so cards don't stretch across a 4K screen.
const PAGE_MAX: f32 = 3.0 * 340.0 + 2.0 * GAP;

/// The grid, in vinopilot.ca's order. vinoPilot itself has no card: its
/// version and update live in the header.
const CARDS: [Id; 9] = [
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

pub struct Pilot {
    config: Config,
    source_field: String,
    local: HashMap<Id, Local>,
    scanning: Option<Receiver<HashMap<Id, Local>>>,
    latest: HashMap<Id, Latest>,
    checking: Option<Receiver<HashMap<Id, Latest>>>,
    /// The way each not-installed card will install from.
    choice: HashMap<Id, Channel>,
    /// The card asking "really uninstall?".
    confirm: Option<Id>,
    job: Option<Job>,
    /// Whether the job's end has been acted on (a rescan).
    job_settled: bool,
    /// Components `Update all` still has to do, in order.
    queue: Vec<Id>,
    focused: bool,
}

impl Pilot {
    /// The window for this machine: starts the local scan.
    pub fn new(ctx: &egui::Context) -> Self {
        let mut pilot = Pilot::with(Config::load(), HashMap::new(), HashMap::new());
        pilot.rescan(ctx);
        pilot
    }

    /// A window showing exactly `local` and `latest`, with no scan: for the
    /// screenshots.
    pub fn with(config: Config, local: HashMap<Id, Local>, latest: HashMap<Id, Latest>) -> Self {
        Pilot {
            source_field: tilde(&config.source_root),
            config,
            local,
            scanning: None,
            latest,
            checking: None,
            choice: HashMap::new(),
            confirm: None,
            job: None,
            job_settled: true,
            queue: Vec::new(),
            focused: false,
        }
    }

    fn paths(&self) -> Paths {
        self.config.paths()
    }

    fn rescan(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let paths = self.paths();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(detect::scan(&paths));
            ctx.request_repaint();
        });
        self.scanning = Some(rx);
    }

    fn check(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let paths = self.paths();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(remote::check(&paths));
            ctx.request_repaint();
        });
        self.checking = Some(rx);
    }

    fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(Job::running)
    }

    fn start(&mut self, ctx: &egui::Context, id: Id, channel: Channel, action: Action) {
        let steps = catalog::steps(id, channel, action);
        if steps.is_empty() {
            return;
        }
        let env = Env::for_id(id, self.paths().checkout(id));
        let ctx = ctx.clone();
        let notify = Arc::new(move |_: Option<&str>| ctx.request_repaint());
        self.job = Some(Job::start(id, channel, action, steps, env, notify));
        self.job_settled = false;
        self.confirm = None;
    }

    fn update(&self, id: Id) -> Update {
        status::update(
            self.local.get(&id).and_then(|l| l.installed.as_ref()),
            self.latest.get(&id),
        )
    }

    /// What Update all does: the cards. vinoPilot updates from the header.
    fn updatable(&self) -> Vec<Id> {
        CARDS
            .into_iter()
            .filter(|id| matches!(self.update(*id), Update::Available { .. }))
            .collect()
    }

    /// Takes whatever the background threads have finished.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(found) = self.scanning.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.local = found;
            self.scanning = None;
        }
        if let Some(found) = self.checking.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.latest = found;
            self.checking = None;
        }
        let finished = self.job.as_ref().map(Job::outcome);
        if !self.job_settled && finished.as_ref().is_some_and(|o| *o != Outcome::Running) {
            self.job_settled = true;
            if finished != Some(Outcome::Done) {
                self.queue.clear();
            }
            // A checkout may have moved, so its "behind" is stale too.
            if self
                .job
                .as_ref()
                .is_some_and(|j| j.channel == Channel::Source)
                && !self.latest.is_empty()
            {
                self.check(ctx);
            }
            self.rescan(ctx);
        }
        // Update all: the next one once the scan after the last has landed.
        if !self.busy() && self.job_settled && self.scanning.is_none() && !self.queue.is_empty() {
            let id = self.queue.remove(0);
            if let Some(installed) = self.local.get(&id).and_then(|l| l.installed.clone()) {
                self.start(ctx, id, installed.channel, Action::Update);
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let narrow = ui.available_width() < 620.0;
        ui.horizontal(|ui| {
            widgets::title(ui, "vinoPilot");
            widgets::muted(ui, VERSION);
            self.self_update(ui);
            if ui
                .add(Button::ghost(vino_ui::family::SITE_LABEL).small())
                .on_hover_text(vino_ui::family::SITE)
                .clicked()
                && let Err(e) = vino_ui::family::open(vino_ui::family::SITE)
            {
                eprintln!("vinopilot: couldn't open {}: {e}", vino_ui::family::SITE);
            }
            if !narrow {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.actions(ui);
                });
            }
        });
        if narrow {
            ui.horizontal(|ui| self.actions(ui));
        }
        widgets::muted(
            ui,
            "Each app installs through its own installer, which checks what it downloads.",
        );
        ui.add_space(6.0);
    }

    /// "Update vinoPilot to vX" once a check finds one, through the same
    /// steps as the other components, for however vinoPilot is installed.
    fn self_update(&mut self, ui: &mut egui::Ui) {
        let Update::Available { to } = self.update(Id::Pilot) else {
            return;
        };
        let Some(installed) = self.local.get(&Id::Pilot).and_then(|l| l.installed.clone()) else {
            return;
        };
        let ctx = ui.ctx().clone();
        let clicked = ui
            .add_enabled(
                !self.busy(),
                Button::primary(format!("Update vinoPilot to {to}")).small(),
            )
            .on_hover_text("Restart vinoPilot afterwards to run the new version")
            .clicked();
        if clicked {
            self.queue.clear();
            self.start(&ctx, Id::Pilot, installed.channel, Action::Update);
        }
    }

    /// Check for updates and Update all. In a right-to-left layout the
    /// order flips, so Update all always sits at the outer edge.
    fn actions(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rtl = ui.layout().main_dir() == egui::Direction::RightToLeft;
        let updatable = self.updatable();
        let busy = self.busy();
        let update_all = |ui: &mut egui::Ui, queue: &mut Vec<Id>| {
            ui.add_enabled_ui(!busy, |ui| {
                if !updatable.is_empty()
                    && ui
                        .add(Button::primary(format!("Update all ({})", updatable.len())).small())
                        .clicked()
                {
                    *queue = updatable.clone();
                }
            });
        };
        if rtl {
            update_all(ui, &mut self.queue);
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
            update_all(ui, &mut self.queue);
        }
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
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
                    });
                });
                widgets::muted(ui, id.blurb());
                ui.add_space(2.0);
                status_line(ui, id, &local, &update, looking);
                if let Some(checkout) = &local.checkout {
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
                                if channels.len() > 1 {
                                    let options: Vec<(Channel, &str)> =
                                        channels.iter().map(|c| (*c, c.label(id))).collect();
                                    widgets::segmented(
                                        ui,
                                        &format!("Install {} from", id.name()),
                                        choice,
                                        &options,
                                    );
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
                    });
                });
            });

        self.confirm = confirm;
        if let Some((channel, action)) = intent {
            self.queue.clear();
            self.start(&ctx, id, channel, action);
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        widgets::group_label(ui, "Settings");
        let help = format!(
            "Source installs clone here, e.g. {}",
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

    fn log(&mut self, ui: &mut egui::Ui) {
        let Some(job) = &self.job else { return };
        let outcome = job.outcome();
        let title = format!(
            "{} {} from {}",
            job.action.verb(),
            job.id.name(),
            job.channel.label(job.id)
        );
        let mut close = false;
        ui.horizontal(|ui| {
            match &outcome {
                Outcome::Running => {
                    widgets::spinner(ui);
                    a11y::live(&widgets::chip(ui, ChipKind::Neutral, &title))
                }
                Outcome::Done => {
                    a11y::live(&widgets::chip(ui, ChipKind::Ok, &format!("{title}: done")))
                }
                Outcome::Stopped => a11y::live(&widgets::chip(
                    ui,
                    ChipKind::Warn,
                    &format!("{title}: stopped"),
                )),
                Outcome::Failed(_) => a11y::live(&widgets::chip(
                    ui,
                    ChipKind::Err,
                    &format!("{title}: failed"),
                )),
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if outcome == Outcome::Running {
                    if ui.add(Button::secondary("Stop").small()).clicked() {
                        job.stop();
                        self.queue.clear();
                    }
                } else if ui.add(Button::ghost("Close").small()).clicked() {
                    close = true;
                }
            });
        });
        if let Outcome::Failed(why) = &outcome {
            widgets::banner(ui, Tone::Error, why, |_| {});
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in job.lines() {
                    ui.label(RichText::new(line).monospace().size(12.0));
                }
            });
        if close {
            self.job = None;
        }
    }

    /// The whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        let p = theme::palette(&ctx);

        if self.job.is_some() {
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(220.0)
                .frame(
                    egui::Frame::NONE
                        .fill(p.surface)
                        .inner_margin(egui::Margin::same(12)),
                )
                .show(ui, |ui| self.log(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.bg).inner_margin(egui::Margin {
                left: widgets::PAGE_INSET as i8,
                right: 0,
                top: 20,
                bottom: 0,
            }))
            .show(ui, |ui| {
                // Above the scroll area, so it stays put; as wide as the
                // cards, which sit inside the scroll bar and its gap.
                ui.scope(|ui| {
                    let bar = ui.spacing().scroll.allocated_width();
                    let inside = ui.available_width() - bar - widgets::SCROLL_GAP;
                    ui.set_max_width(inside.min(PAGE_MAX));
                    self.header(ui);
                });
                ui.add_space(6.0);
                widgets::page_scroll(ui, "vinoPilot", PAGE_MAX, |ui| {
                    self.grid(ui);
                    self.settings(ui);
                });
            });
    }
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
    if id == Id::Pilot && version != VERSION {
        shown.on_hover_text(format!("This window is {VERSION}"));
    }
}

/// `/home/k/x` as `~/x`.
fn tilde(path: &std::path::Path) -> String {
    let home = config::home();
    match path.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

impl eframe::App for Pilot {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The desktop's light/dark switch may have flipped while the window was in the background.
        let focused = ui.input(|i| i.focused);
        if focused && !self.focused {
            theme::follow_desktop(ui.ctx());
        }
        self.focused = focused;
        Pilot::ui(self, ui);
    }
}
