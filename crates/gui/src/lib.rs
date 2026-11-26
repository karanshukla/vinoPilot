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
pub mod launcher;
mod log;
mod settings;

use cards::{CARDS, PAGE_MAX};
use header::{NARROW, header_width};
use launcher::{Entry, Launcher, job_progress, percent};

pub use vinopilot_core::VERSION;

/// Which subset of the catalog the grid shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Filter {
    All,
    Installed,
    Updates,
}

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
    /// Which subset of components the grid displays.
    filter: Filter,
    /// The card asking "really uninstall?".
    confirm: Option<Id>,
    job: Option<Job>,
    /// Whether the job's end has been acted on (a rescan).
    job_settled: bool,
    /// Components `Update all` still has to do, in order.
    queue: Vec<Id>,
    /// How many `Update all` started with; 0 for a single job.
    batch: usize,
    /// The icon in the desktop's launcher; none in the tests.
    launcher: Option<Launcher>,
    /// When closing was last refused because a job was running.
    close_refused: Option<f64>,
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
        pilot.launcher = Some(Launcher::new());
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
            filter: Filter::All,
            confirm: None,
            job: None,
            job_settled: true,
            queue: Vec::new(),
            batch: 0,
            launcher: None,
            close_refused: None,
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

    pub(super) fn active_job(&self) -> Option<(Id, Action)> {
        self.job
            .as_ref()
            .filter(|j| j.running())
            .map(|j| (j.id, j.action))
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
            self.local
                .get(&id)
                .and_then(|local| local.installed.as_ref()),
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
            self.settle_job(ctx, finished);
        }
        if !self.busy() && self.job_settled && self.scanning.is_none() {
            self.update_next_queued(ctx);
        }
    }

    /// After a job: rescan, and re-check a source install, whose checkout may
    /// have moved and left its "behind" stale. A failure ends Update all.
    fn settle_job(&mut self, ctx: &egui::Context, finished: Option<Outcome>) {
        self.job_settled = true;
        if finished != Some(Outcome::Done) {
            self.end_batch();
        }
        let moved_a_checkout = self
            .job
            .as_ref()
            .is_some_and(|j| j.channel == Channel::Source);
        if moved_a_checkout && !self.latest.is_empty() {
            self.check(ctx);
        }
        self.rescan(ctx);
    }

    /// Update all is over, or was never on.
    fn end_batch(&mut self) {
        self.queue.clear();
        self.batch = 0;
    }

    /// The launcher icon's bar: the running job's steps done, and with
    /// Update all, the components done of all of them (pinned by
    /// `tests::a_single_job_fills_the_bar_by_its_steps` and
    /// `tests::update_all_fills_the_bar_by_components_then_steps`). The badge
    /// counts what Update all and the header would update (pinned by
    /// `tests::the_badge_counts_the_cards_and_vinopilot_with_an_update`).
    fn launcher_entry(&self) -> Entry {
        let job = self
            .job
            .as_ref()
            .filter(|j| j.running())
            .map(|j| job_progress(j.steps()));
        let progress = if self.batch > 0 && (job.is_some() || !self.queue.is_empty()) {
            let left = self.queue.len() + usize::from(job.is_some());
            let done = self.batch.saturating_sub(left) as f64 + job.unwrap_or(0.0);
            Some(done / self.batch as f64)
        } else {
            job
        };
        let pilot = matches!(
            self.update(Id::App(vino_ui::App::Pilot)),
            Update::Available { .. }
        );
        let count = self.updatable().len() + usize::from(pilot);
        Entry {
            progress: progress.map(percent),
            count: (count > 0).then_some(count as i64),
        }
    }

    /// Closing mid-job would leave an install half done (pinned by
    /// `tests::the_window_wont_close_while_a_job_runs`).
    fn refuse_close_while_busy(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_refused = Some(ctx.input(|i| i.time));
        }
        if !self.busy() {
            self.close_refused = None;
        }
    }

    /// Update all's next component, once the scan after the last has landed.
    fn update_next_queued(&mut self, ctx: &egui::Context) {
        if self.queue.is_empty() {
            return;
        }
        let id = self.queue.remove(0);
        if let Some(installed) = self
            .local
            .get(&id)
            .and_then(|local| local.installed.clone())
        {
            self.start(ctx, id, installed.channel, Action::Update);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.refuse_close_while_busy(&ctx);
        let palette = theme::palette(&ctx);
        let content_rect = ctx.content_rect();
        let inset = widgets::content_inset(content_rect.width());
        let compact = vino_ui::SizeClass::of(content_rect.width()).is_compact();

        if self.job.is_some() {
            let log_height = (content_rect.height() * 0.34).clamp(110.0, 220.0);
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(log_height)
                .size_range(88.0..=f32::INFINITY)
                .frame(
                    egui::Frame::NONE
                        .fill(palette.surface)
                        .inner_margin(egui::Margin::same(if compact { 10 } else { 12 })),
                )
                .show(ui, |ui| self.log(ui));
        }
        let bar = self.app_title_bar;
        if bar {
            self.title_bar(ui, inset);
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(palette.bg)
                    .inner_margin(egui::Margin {
                        left: inset as i8,
                        right: 0,
                        top: if bar {
                            2
                        } else if compact {
                            12
                        } else {
                            20
                        },
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                self.header_above_the_cards(ui, bar);
                ui.add_space(6.0);
                widgets::page_scroll(ui, "vinoPilot", PAGE_MAX, |ui| {
                    self.filter_bar(ui);
                    ui.add_space(if compact { 8.0 } else { 12.0 });
                    self.grid(ui);
                });
            });
    }
}

impl Pilot {
    /// With the app's title bar, the title line becomes the bar the window
    /// is dragged by.
    fn title_bar(&mut self, ui: &mut egui::Ui, inset: f32) {
        let narrow = header_width(ui, ui.available_width() - inset) < NARROW;
        let pilot = std::cell::RefCell::new(self);
        vino_ui::window::bar(
            ui,
            inset,
            |ui| pilot.borrow_mut().title_line(ui, !narrow),
            |ui| {
                if !narrow {
                    pilot.borrow_mut().actions(ui);
                }
            },
        );
    }

    /// The header, outside the scroll area and as wide as the cards.
    fn header_above_the_cards(&mut self, ui: &mut egui::Ui, bar: bool) {
        ui.scope(|ui| {
            ui.set_max_width(header_width(ui, ui.available_width()));
            self.header(ui, bar);
        });
    }

    /// The desktop's light/dark switch may have flipped while the window was
    /// in the background.
    fn follow_the_desktop_theme_on_focus(&mut self, ui: &egui::Ui) {
        let focused = ui.input(|input| input.focused);
        if focused && !self.focused {
            theme::follow_desktop(ui.ctx());
        }
        self.focused = focused;
    }
}

/// Whether `exe` is in a Flatpak's files: `<installation>/app/<id>/<arch>/
/// <branch>/<commit>/files/libexec/vinopilot/vinopilot-gui`, where
/// packaging/flatpak/host-run.sh starts it on the host.
fn in_flatpak(exe: &std::path::Path) -> bool {
    let mut parts = exe.components().map(|part| part.as_os_str());
    parts.any(|part| part == "app") && parts.next().is_some_and(|part| part == desktop::APP_ID)
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
        self.follow_the_desktop_theme_on_focus(ui);
        Pilot::ui(self, ui);
        let entry = self.launcher_entry();
        if let Some(launcher) = &mut self.launcher {
            launcher.show(entry);
            // The bar creeps with time, not only with output.
            if entry.progress.is_some() {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
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
    use vinopilot_core::detect::Installed;

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

    fn installed(version: &str) -> Local {
        Local {
            installed: Some(Installed {
                channel: Channel::Release,
                version: Some(version.into()),
            }),
            ..Default::default()
        }
    }

    fn newer() -> Latest {
        Latest {
            release: Some(Ok("v99.0.0".into())),
            ..Default::default()
        }
    }

    fn pilot(ids: &[Id]) -> Pilot {
        let local = ids.iter().map(|id| (*id, installed("1.0.0"))).collect();
        let latest = ids.iter().map(|id| (*id, newer())).collect();
        let config = Config {
            source_root: "/nowhere".into(),
        };
        Pilot::with(config, local, latest)
    }

    fn between(progress: Option<f64>, low: f64, high: f64) -> bool {
        progress.is_some_and(|p| (low..=high).contains(&p))
    }

    /// A job whose first step is done and whose second runs until stopped.
    fn halfway() -> Job {
        let step = |script: &str| catalog::Step {
            script: script.into(),
            root: false,
        };
        let job = Job::start(
            Id::Ovfetch,
            Channel::Release,
            Action::Update,
            vec![step("true"), step("sleep 30")],
            Env::for_id(
                Id::Ovfetch,
                &Config {
                    source_root: "/nowhere".into(),
                }
                .paths(),
            ),
            Arc::new(|_| {}),
        );
        while job.steps().done == 0 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        job
    }

    #[test]
    fn the_badge_counts_the_cards_and_vinopilot_with_an_update() {
        let ids = [Id::Ovfetch, Id::Ui, Id::App(vino_ui::App::Pilot)];
        assert_eq!(pilot(&ids).launcher_entry().count, Some(3));
        assert_eq!(pilot(&[]).launcher_entry(), Entry::default());
    }

    #[test]
    fn a_single_job_fills_the_bar_by_its_steps() {
        let mut p = pilot(&[]);
        p.job = Some(halfway());
        // One of two done, and the second just started.
        assert!(between(p.launcher_entry().progress, 0.52, 0.56));
        p.job.as_ref().unwrap().stop();
        p.job.as_ref().unwrap().wait();
        assert_eq!(p.launcher_entry().progress, None);
    }

    #[test]
    fn update_all_fills_the_bar_by_components_then_steps() {
        let mut p = pilot(&[]);
        p.batch = 4;
        p.queue = vec![Id::Ovfetch, Id::Ui, Id::App(vino_ui::App::Pop)];
        // Between the first and second: one of four done.
        assert_eq!(p.launcher_entry().progress, Some(0.25));
        // The second, one of its two steps done.
        p.queue.remove(0);
        p.job = Some(halfway());
        assert!(between(p.launcher_entry().progress, 0.37, 0.40));
        p.job.as_ref().unwrap().stop();
    }

    fn close(ctx: &egui::Context, pilot: &mut Pilot) -> bool {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let mut out = ctx.run_ui(input, |ui| pilot.ui(ui));
        out.textures_delta.clear();
        !out.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    }

    #[test]
    fn the_window_wont_close_while_a_job_runs() {
        let ctx = egui::Context::default();
        let mut p = pilot(&[]);
        assert!(close(&ctx, &mut p));
        p.job = Some(halfway());
        assert!(!close(&ctx, &mut p));
        assert!(p.close_refused.is_some());
        p.job.as_ref().unwrap().stop();
        p.job.as_ref().unwrap().wait();
        assert!(close(&ctx, &mut p));
    }
}
