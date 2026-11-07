//! Runs an action's steps in the background, one at a time, keeping their
//! output for the log panel.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::catalog::{Action, Channel, Id, Step};

/// Called with each line of output as it arrives, then with `None` when
/// the job ends: the window repaints, the CLI prints.
pub type Notify = Arc<dyn Fn(Option<&str>) + Send + Sync>;

/// Lines kept; an installer that compiles prints thousands.
const KEEP: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "outcome", content = "reason", rename_all = "lowercase")]
pub enum Outcome {
    Running,
    Done,
    Failed(String),
    Stopped,
}

#[derive(Default)]
struct Shared {
    lines: Vec<String>,
    outcome: Option<Outcome>,
    child: Option<Child>,
    stop: bool,
}

pub struct Job {
    pub id: Id,
    pub channel: Channel,
    pub action: Action,
    shared: Arc<Mutex<Shared>>,
}

/// What a step needs from the environment instead of formatted into it.
pub struct Env {
    pub checkout: PathBuf,
    pub repo_url: String,
    pub release: String,
}

impl Env {
    pub fn for_id(id: Id, checkout: PathBuf) -> Self {
        Env {
            checkout,
            repo_url: format!("https://github.com/{}.git", id.repo()),
            release: format!("https://github.com/{}/releases/latest/download", id.repo()),
        }
    }
}

/// Drops terminal colour codes and carriage-return redraws.
pub fn clean(line: &str) -> String {
    let line = line.rsplit('\r').find(|l| !l.is_empty()).unwrap_or("");
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn command(step: &Step, env: &Env) -> Command {
    let vars = [
        ("CHECKOUT", env.checkout.to_string_lossy().into_owned()),
        ("REPO_URL", env.repo_url.clone()),
        ("RELEASE", env.release.clone()),
        ("NO_COLOR", "1".into()),
    ];
    let mut cmd = if step.root {
        // pkexec clears the environment: pass what the step reads, and
        // SUDO_USER so vinoAuthFace's scripts find the desktop user.
        let mut cmd = crate::command("pkexec");
        cmd.arg("env");
        for (k, v) in &vars {
            cmd.arg(format!("{k}={v}"));
        }
        if let Ok(user) = std::env::var("USER") {
            cmd.arg(format!("SUDO_USER={user}"));
        }
        cmd.arg("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
        cmd.args(["bash", "-c", &step.script]);
        cmd
    } else {
        let mut cmd = crate::command("bash");
        cmd.args(["-c", &step.script]).envs(vars);
        cmd
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn pump(shared: Arc<Mutex<Shared>>, from: impl Read + Send + 'static, notify: Notify) {
    std::thread::spawn(move || {
        for line in BufReader::new(from).lines() {
            let Ok(line) = line else { break };
            let line = clean(&line);
            let mut s = shared.lock().unwrap();
            s.lines.push(line.clone());
            if s.lines.len() > KEEP {
                let extra = s.lines.len() - KEEP;
                s.lines.drain(..extra);
            }
            drop(s);
            notify(Some(&line));
        }
    });
}

impl Job {
    pub fn start(
        id: Id,
        channel: Channel,
        action: Action,
        steps: Vec<Step>,
        env: Env,
        notify: Notify,
    ) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            outcome: Some(Outcome::Running),
            ..Default::default()
        }));
        let job = Job {
            id,
            channel,
            action,
            shared: shared.clone(),
        };
        std::thread::spawn(move || {
            let outcome = run(&shared, &steps, &env, &notify);
            shared.lock().unwrap().outcome = Some(outcome);
            notify(None);
        });
        job
    }

    pub fn lines(&self) -> Vec<String> {
        self.shared.lock().unwrap().lines.clone()
    }

    pub fn outcome(&self) -> Outcome {
        self.shared
            .lock()
            .unwrap()
            .outcome
            .clone()
            .unwrap_or(Outcome::Running)
    }

    /// Blocks until the job ends.
    pub fn wait(&self) -> Outcome {
        loop {
            match self.outcome() {
                Outcome::Running => std::thread::sleep(std::time::Duration::from_millis(50)),
                done => return done,
            }
        }
    }

    pub fn running(&self) -> bool {
        self.outcome() == Outcome::Running
    }

    /// Kills the running step and skips the rest. A pkexec step belongs to
    /// root and can't be killed from here; it stops after it ends.
    pub fn stop(&self) {
        let mut s = self.shared.lock().unwrap();
        s.stop = true;
        if let Some(child) = s.child.as_mut() {
            let _ = child.kill();
        }
    }
}

fn run(shared: &Arc<Mutex<Shared>>, steps: &[Step], env: &Env, notify: &Notify) -> Outcome {
    for step in steps {
        if shared.lock().unwrap().stop {
            return Outcome::Stopped;
        }
        let mut child = match command(step, env).spawn() {
            Ok(child) => child,
            Err(e) => {
                let program = if step.root { "pkexec" } else { "bash" };
                return Outcome::Failed(format!("Couldn't run {program}: {e}"));
            }
        };
        if let Some(out) = child.stdout.take() {
            pump(shared.clone(), out, notify.clone());
        }
        if let Some(err) = child.stderr.take() {
            pump(shared.clone(), err, notify.clone());
        }
        shared.lock().unwrap().child = Some(child);
        let status = loop {
            let mut s = shared.lock().unwrap();
            let Some(child) = s.child.as_mut() else {
                return Outcome::Failed("lost the process".into());
            };
            match child.try_wait() {
                Ok(Some(status)) => {
                    s.child = None;
                    break status;
                }
                Ok(None) => {}
                Err(e) => return Outcome::Failed(e.to_string()),
            }
            drop(s);
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        // Let the pumps drain the pipes before reading the last line.
        std::thread::sleep(std::time::Duration::from_millis(100));
        if shared.lock().unwrap().stop {
            return Outcome::Stopped;
        }
        if !status.success() {
            // pkexec exits 126 when the password dialog is dismissed.
            if step.root && matches!(status.code(), Some(126 | 127)) {
                return Outcome::Failed(
                    "Not authorised: the password prompt was cancelled.".into(),
                );
            }
            let lines = shared.lock().unwrap().lines.clone();
            let why = lines
                .iter()
                .rev()
                .find(|l| l.trim_start().starts_with('✗'))
                .or_else(|| lines.iter().rev().find(|l| !l.trim().is_empty()))
                .map(|l| l.trim_start_matches(['✗', ' ']).to_string())
                .unwrap_or_else(|| format!("exited with {status}"));
            return Outcome::Failed(why);
        }
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_strips_colour_and_redraws() {
        assert_eq!(clean("\x1b[1;32m✓\x1b[0m done"), "✓ done");
        assert_eq!(clean("10%\r50%\r100%"), "100%");
        assert_eq!(clean("plain"), "plain");
    }
}
