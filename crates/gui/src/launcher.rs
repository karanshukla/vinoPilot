//! The window's icon in the desktop's launcher: a progress bar while a job
//! runs and a badge with the number of updates, through the Unity
//! LauncherEntry signal that KDE's task manager and GNOME's Dash to Dock
//! listen for. Desktops that don't listen ignore it.

use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};

use zbus::zvariant::Value;

use crate::desktop::APP_ID;

const PATH: &str = "/io/github/karanshukla/vinopilot/launcher";
const IFACE: &str = "com.canonical.Unity.LauncherEntry";

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
