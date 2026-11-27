//! The panel under the window while a job runs or has just ended: what it
//! is, how it went, its progress and output, and Stop, Copy log or Close.

use eframe::egui::{self, RichText};
use vino_ui::{Button, ChipKind, Tone, a11y, widgets};
use vinopilot_core::catalog::Action;
use vinopilot_core::job::Outcome;

use crate::Pilot;

impl Pilot {
    pub(super) fn log(&mut self, ui: &mut egui::Ui) {
        let progress = self.launcher_entry().progress;
        let batch = self.batch;
        let remaining = self.queue.len();
        let Some(job) = &self.job else { return };
        let outcome = job.outcome();
        let base_title = match job.action {
            Action::Uninstall => format!(
                "{} {} ({})",
                job.action.verb(),
                job.id.name(),
                job.channel.label(job.id)
            ),
            Action::Install | Action::Update => format!(
                "{} {} from {}",
                job.action.verb(),
                job.id.name(),
                job.channel.label(job.id)
            ),
        };
        let title = if batch > 1 && outcome == Outcome::Running {
            let current = batch.saturating_sub(remaining);
            format!("{base_title} ({current} of {batch})")
        } else {
            base_title
        };
        let lines = job.lines();
        let mut close = false;
        let mut stop = false;
        widgets::responsive_bar(
            ui,
            480.0,
            |ui| match &outcome {
                Outcome::Running => {
                    widgets::spinner(ui);
                    a11y::live(&widgets::chip(ui, ChipKind::Neutral, &title));
                }
                Outcome::Done => {
                    a11y::live(&widgets::chip(ui, ChipKind::Ok, &format!("{title}: done")));
                }
                Outcome::Stopped => {
                    a11y::live(&widgets::chip(
                        ui,
                        ChipKind::Warn,
                        &format!("{title}: stopped"),
                    ));
                }
                Outcome::Failed(_) => {
                    a11y::live(&widgets::chip(
                        ui,
                        ChipKind::Err,
                        &format!("{title}: failed"),
                    ));
                }
            },
            |ui| {
                if outcome == Outcome::Running {
                    if ui.add(Button::secondary("Stop").small()).clicked() {
                        stop = true;
                    }
                    if let Some(fraction) = progress {
                        let bar_w = ui.available_width().clamp(64.0, 120.0);
                        widgets::progress(ui, fraction as f32, bar_w);
                    }
                } else {
                    if ui.add(Button::ghost("Close").small()).clicked() {
                        close = true;
                    }
                    if !lines.is_empty() {
                        widgets::copy_button(ui, &lines.join("\n"), "Copy log");
                    }
                }
            },
        );
        if stop {
            job.stop();
            self.queue.clear();
            self.batch = 0;
        }
        if let Outcome::Failed(why) = &outcome {
            widgets::banner(ui, Tone::Error, why, |_| {});
        }
        if self.close_refused.is_some()
            && outcome == Outcome::Running
            && widgets::banner(
                ui,
                Tone::Warn,
                "Stop the running task before closing.",
                |ui| ui.add(Button::secondary("Stop & close").small()).clicked(),
            )
        {
            job.stop();
            self.queue.clear();
            self.batch = 0;
            self.close_after_stop = true;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &lines {
                    ui.label(RichText::new(line).monospace().size(12.0));
                }
            });
        if close {
            self.job = None;
        }
    }
}
