//! The window's icon in the desktop's launcher: a progress bar while a job
//! runs and a badge with the number of updates, through the Unity
//! LauncherEntry signal that KDE's task manager and GNOME's Dash to Dock
//! listen for. Desktops that don't listen ignore it.

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use vinopilot_core::job::Steps;

use zbus::zvariant::Value;

use crate::desktop::APP_ID;

const PATH: &str = "/io/github/karanshukla/vinopilot/launcher";
const IFACE: &str = "com.canonical.Unity.LauncherEntry";

/// A step starts this far into its share of the bar, so the bar shows: KDE
/// draws nothing at 0.
const STEP_FLOOR: f64 = 0.05;
/// A running step never fills more of its share than this.
const STEP_CAP: f64 = 0.9;
/// How long a step takes to get about two thirds of the way from the floor
/// to the cap. Installers don't report progress, so time stands in for it.
const STEP_PACE: Duration = Duration::from_secs(30);

/// The share of a running step's part of the bar after `running_for`:
/// from the floor, slowing toward the cap (pinned by
/// `tests::a_step_starts_visible_and_never_fills_while_it_runs`).
fn step_share(running_for: Duration) -> f64 {
    let pace = running_for.as_secs_f64() / STEP_PACE.as_secs_f64();
    STEP_FLOOR + (STEP_CAP - STEP_FLOOR) * (1.0 - (-pace).exp())
}

/// A running job's fraction: the steps done, and the running step's share.
pub fn job_progress(steps: Steps) -> f64 {
    let done = steps.done as f64 + step_share(steps.running_for);
    (done / steps.of.max(1) as f64).min(1.0)
}

/// Whole percents, so a bar creeping each frame isn't a signal each frame.
pub fn percent(progress: f64) -> f64 {
    (progress * 100.0).round() / 100.0
}

/// What the icon shows.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Entry {
    /// 0.0 to 1.0, while something runs.
    pub progress: Option<f64>,
    /// Updates available, when there are any.
    pub count: Option<i64>,
}

/// Sends each new `Entry` from a thread of its own, so a slow or missing
/// session bus never holds up a frame.
pub struct Launcher {
    tx: Sender<Entry>,
    last: Option<Entry>,
}

impl Launcher {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Entry>();
        std::thread::spawn(move || {
            let Ok(bus) = zbus::blocking::Connection::session() else {
                return;
            };
            let uri = format!("application://{APP_ID}.desktop");
            for entry in rx {
                let _ = bus.emit_signal(None::<()>, PATH, IFACE, "Update", &(&uri, props(entry)));
            }
        });
        Launcher { tx, last: None }
    }

    /// Shows `entry`, if it isn't what's already shown.
    pub fn show(&mut self, entry: Entry) {
        if self.last != Some(entry) {
            self.last = Some(entry);
            let _ = self.tx.send(entry);
        }
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

fn props(entry: Entry) -> HashMap<&'static str, Value<'static>> {
    HashMap::from([
        ("progress", Value::from(entry.progress.unwrap_or(0.0))),
        ("progress-visible", Value::from(entry.progress.is_some())),
        ("count", Value::from(entry.count.unwrap_or(0))),
        ("count-visible", Value::from(entry.count.is_some())),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_starts_visible_and_never_fills_while_it_runs() {
        assert_eq!(step_share(Duration::ZERO), STEP_FLOOR);
        assert!(step_share(STEP_PACE) > 0.5);
        assert!(step_share(Duration::from_secs(3600)) <= STEP_CAP);
    }

    #[test]
    fn a_job_counts_its_finished_steps_whole() {
        let steps = |done, secs| Steps {
            done,
            of: 2,
            running_for: Duration::from_secs(secs),
        };
        assert_eq!(job_progress(steps(0, 0)), STEP_FLOOR / 2.0);
        assert_eq!(job_progress(steps(1, 0)), (1.0 + STEP_FLOOR) / 2.0);
        assert!(job_progress(steps(1, 3600)) < 1.0);
    }

    #[test]
    fn a_hidden_bar_and_badge_are_sent_as_not_visible() {
        let p = props(Entry::default());
        assert_eq!(p["progress-visible"], Value::from(false));
        assert_eq!(p["count-visible"], Value::from(false));
        let p = props(Entry {
            progress: Some(0.5),
            count: Some(3),
        });
        assert_eq!(p["progress"], Value::from(0.5));
        assert_eq!(p["count"], Value::from(3i64));
        assert_eq!(p["count-visible"], Value::from(true));
    }
}
