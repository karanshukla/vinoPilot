//! The panel under the window while a job runs or has just ended: what it
//! is, how it went, its output, and Stop or Close.

use eframe::egui::{self, RichText};
use vino_ui::{Button, ChipKind, Tone, a11y, widgets};
use vinopilot_core::job::Outcome;

use crate::Pilot;

impl Pilot {
    pub(super) fn log(&mut self, ui: &mut egui::Ui) {
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
}
