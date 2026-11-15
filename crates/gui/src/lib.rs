//! The vinopilot window: a grid of cards, one per vinoPilot component, each
//! with Install, Update and Uninstall, and the installer's output in a panel
//! underneath. All the knowledge is in `vinopilot-core`; this only draws it.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;
use vino_ui::{theme, widgets};
use vinopilot_core::catalog::{self, Action, Channel, Id};
use vinopilot_core::detect::{Local, Part, Paths};
use vinopilot_core::job::{Env, Job, Outcome};
use vinopilot_core::remote::Latest;
use vinopilot_core::status::{self, Update};
use vinopilot_core::{Config, config, detect, remote};

mod cards;
pub mod desktop;
mod header;
mod log;
mod settings;

use cards::{CARDS, PAGE_MAX};
use header::{NARROW, header_width};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a local scan finds: each component, and vinoPilot's own commands.
type Scan = (HashMap<Id, Local>, Vec<Part>);

pub struct Pilot {
    config: Config,
    source_field: String,
    local: HashMap<Id, Local>,
    /// vinoPilot's own commands, for the About popup.
    parts: Vec<Part>,
    scanning: Option<Receiver<Scan>>,
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
    /// The card whose remove command was just copied, and when.
    copied: Option<(Id, f64)>,
    /// This window is the Flatpak's, started from where Flatpak keeps it:
    /// it restarts through `flatpak run`, having no command on `PATH`.
    flatpak: bool,
    /// The window's buttons and edges are drawn here, not by the desktop
    /// (vino-ui's `window`).
    app_title_bar: bool,
}

impl Pilot {
    /// The window for this machine: starts the local scan.
    pub fn new(ctx: &egui::Context) -> Self {
        let mut pilot = Pilot::with(Config::load(), HashMap::new(), HashMap::new());
        pilot.flatpak = std::env::current_exe().is_ok_and(|exe| in_flatpak(&exe));
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
            parts: Vec::new(),
            scanning: None,
            latest,
            checking: None,
            choice: HashMap::new(),
            confirm: None,
            job: None,
            job_settled: true,
            queue: Vec::new(),
            focused: false,
            copied: None,
            flatpak: false,
            app_title_bar: false,
        }
    }

    /// The same window, with `parts` as the commands found: for the tests.
    pub fn with_parts(mut self, parts: Vec<Part>) -> Self {
        self.parts = parts;
        self
    }

    /// The same window with its own window buttons, for a window opened
    /// without the desktop's decorations (`vino_ui::window::viewport`).
    pub fn with_app_title_bar(mut self, on: bool) -> Self {
        self.app_title_bar = on;
        self
    }

    fn paths(&self) -> Paths {
        self.config.paths()
    }

    fn rescan(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let paths = self.paths();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((detect::scan(&paths), detect::parts(&paths)));
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
        let env = Env::for_id(id, &self.paths());
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
        if let Some((found, parts)) = self.scanning.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.local = found;
            self.parts = parts;
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
        // With the app's title bar, the header's title line moves up into a
        // bar of its own, with the window buttons at its end: that bar is
        // what the window is dragged by. Update all and the rest stay beside
        // the title where there's room, as they were.
        let bar = self.app_title_bar;
        if bar {
            let narrow = header_width(ui, ui.available_width() - widgets::PAGE_INSET) < NARROW;
            // Both halves act on the window, one after the other.
            let pilot = std::cell::RefCell::new(&mut *self);
            vino_ui::window::bar(
                ui,
                widgets::PAGE_INSET,
                |ui| pilot.borrow_mut().title_line(ui, !narrow),
                |ui| {
                    if !narrow {
                        pilot.borrow_mut().actions(ui);
                    }
                },
            );
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(p.bg).inner_margin(egui::Margin {
                left: widgets::PAGE_INSET as i8,
                right: 0,
                top: if bar { 2 } else { 20 },
                bottom: 0,
            }))
            .show(ui, |ui| {
                // Above the scroll area, so it stays put; as wide as the
                // cards, which sit inside the scroll bar and its gap.
                ui.scope(|ui| {
                    ui.set_max_width(header_width(ui, ui.available_width()));
                    self.header(ui, bar);
                });
                ui.add_space(6.0);
                widgets::page_scroll(ui, "vinoPilot", PAGE_MAX, |ui| {
                    self.grid(ui);
                    self.settings(ui);
                });
            });
    }
}

/// Whether `exe` is in a Flatpak's files: `<installation>/app/<id>/<arch>/
/// <branch>/<commit>/files/libexec/vinopilot/vinopilot-gui`, where
/// packaging/flatpak/host-run.sh starts it on the host.
fn in_flatpak(exe: &std::path::Path) -> bool {
    let mut parts = exe.components().map(|c| c.as_os_str());
    parts.any(|c| c == "app") && parts.next().is_some_and(|c| c == desktop::APP_ID)
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
        if self.app_title_bar {
            vino_ui::window::edges(ui);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        vino_ui::input::faster_touchpad(raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_flatpak_is_told_by_where_its_binary_is() {
        for dir in ["/var/lib/flatpak", "/home/k/.local/share/flatpak"] {
            let exe = format!(
                "{dir}/app/{}/x86_64/master/0a1b/files/libexec/vinopilot/vinopilot-gui",
                desktop::APP_ID
            );
            assert!(in_flatpak(Path::new(&exe)), "{exe}");
        }
        assert!(!in_flatpak(Path::new("/usr/bin/vinopilot-gui")));
        assert!(!in_flatpak(Path::new("/home/k/app/vinopilot-gui")));
    }
}
