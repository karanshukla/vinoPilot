//! The panel under the window while a job runs or has just ended: what it
//! is, how it went, its progress and output, and Stop, Copy log or Close.

use eframe::egui::{self, RichText};
use vino_ui::{Button, ChipKind, Tone, a11y, widgets};
use vinopilot_core::job::Outcome;

use crate::Pilot;

impl Pilot {
    pub(super) fn log(&mut self, ui: &mut egui::Ui) {
        let progress = self.launcher_entry().progress;
        let batch = self.batch;
        let remaining = self.queue.len();
        let Some(job) = &self.job else { return };
        let outcome = job.outcome();
        let base_title = format!(
            "{} {} from {}",
            job.action.verb(),
            job.id.name(),
            job.channel.label(job.id)
        );
        let title = if batch > 1 && outcome == Outcome::Running {
            let current = batch.saturating_sub(remaining);
            format!("{base_title} ({current} of {batch})")
        } else {
            base_title
        };
        let lines = job.lines();
        let mut close = false;
        let mut stop = false;
        ui.horizontal(|ui| {
            match &outcome {
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
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if outcome == Outcome::Running {
                    if ui.add(Button::secondary("Stop").small()).clicked() {
                        stop = true;
                    }
                    if let Some(fraction) = progress {
                        widgets::progress(ui, fraction as f32, 120.0);
                    }
                } else {
                    if ui.add(Button::ghost("Close").small()).clicked() {
                        close = true;
                    }
                    if !lines.is_empty() {
                        widgets::copy_button(ui, &lines.join("\n"), "Copy log");
                    }
                }
            });
        });
        if stop {
            job.stop();
            self.queue.clear();
            self.batch = 0;
        }
        if let Outcome::Failed(why) = &outcome {
            widgets::banner(ui, Tone::Error, why, |_| {});
        }
        if self.close_refused.is_some() && outcome == Outcome::Running {
            widgets::banner(
                ui,
                Tone::Warn,
                "Cannot close while an operation is running. Click Stop first to cancel it.",
                |_| {},
            );
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
