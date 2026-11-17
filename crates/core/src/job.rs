//! Runs an action's steps in the background, one at a time, keeping their
//! output for the log panel.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::catalog::{Action, Channel, Id, OffBranch, Step};
use crate::detect::Paths;

/// Called with each line of output as it arrives, then with `None` when
/// the job ends: the window repaints, the CLI prints.
pub type Notify = Arc<dyn Fn(Option<&str>) + Send + Sync>;

/// Lines kept; an installer that compiles prints thousands.
const KEEP: usize = 4000;

/// How often a running step, or a waiting caller, looks again.
const POLL: Duration = Duration::from_millis(50);

/// What the output threads get to empty the pipes after a step exits, before
/// its last line is read.
const PUMP_DRAIN: Duration = Duration::from_millis(100);

/// pkexec's exit codes for a dismissed password dialog (126) and for not
/// being authorised (127).
const PKEXEC_REFUSED: [i32; 2] = [126, 127];

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
    /// Steps finished, of `steps`.
    done: usize,
    steps: usize,
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
    pub worktree: PathBuf,
    pub off_branch: OffBranch,
    pub repo_url: String,
    pub release: String,
}

impl Env {
    pub fn for_id(id: Id, paths: &Paths) -> Self {
        Env {
            checkout: paths.checkout(id),
            worktree: paths.worktree(id),
            off_branch: id.off_branch(),
            repo_url: format!("https://github.com/{}.git", id.repo()),
            release: release_base(id),
        }
    }
}

/// Where `id`'s latest release's files are. For vinoPilot itself,
/// `VINOPILOT_RELEASE_BASE` moves it, as it does for install.sh: the tests
/// update from a `file://` release.
fn release_base(id: Id) -> String {
    let pilot = id == Id::App(vino_ui::App::Pilot);
    std::env::var("VINOPILOT_RELEASE_BASE")
        .ok()
        .filter(|base| pilot && !base.is_empty())
        .unwrap_or_else(|| format!("https://github.com/{}/releases/latest/download", id.repo()))
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

/// Why a step failed, from its output: the installer's last `✗` line, else
/// its last line that says anything.
fn failure_reason(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .rev()
        .find(|l| l.trim_start().starts_with('✗'))
        .or_else(|| lines.iter().rev().find(|l| !l.trim().is_empty()))
        .map(|l| l.trim_start_matches(['✗', ' ']).to_string())
}

fn command(step: &Step, env: &Env) -> Command {
    let vars = [
        ("CHECKOUT", env.checkout.to_string_lossy().into_owned()),
        ("WORKTREE", env.worktree.to_string_lossy().into_owned()),
        ("OFF_BRANCH", env.off_branch.name().into()),
        ("REPO_URL", env.repo_url.clone()),
        ("RELEASE", env.release.clone()),
        ("NO_COLOR", "1".into()),
    ];
    let mut cmd = if step.root {
        through_pkexec(&step.script, &vars)
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

/// `script` as root. pkexec clears the environment, so `vars` go on `env`'s
/// command line, with `SUDO_USER` for vinoAuthFace's scripts to find the
/// desktop user.
fn through_pkexec(script: &str, vars: &[(&str, String)]) -> Command {
    let mut cmd = crate::command("pkexec");
    cmd.arg("env");
    for (k, v) in vars {
        cmd.arg(format!("{k}={v}"));
    }
    if let Ok(user) = std::env::var("USER") {
        cmd.arg(format!("SUDO_USER={user}"));
    }
    cmd.arg("PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
    cmd.args(["bash", "-c", script]);
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
            steps: steps.len(),
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
                Outcome::Running => std::thread::sleep(POLL),
                done => return done,
            }
        }
    }

    /// Steps finished and steps in all: how far along the job is, for the
    /// launcher's progress bar.
    pub fn steps(&self) -> (usize, usize) {
        let s = self.shared.lock().unwrap();
        (s.done, s.steps)
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
            std::thread::sleep(POLL);
        };
        std::thread::sleep(PUMP_DRAIN);
        if shared.lock().unwrap().stop {
            return Outcome::Stopped;
        }
        if !status.success() {
            if step.root
                && status
                    .code()
                    .is_some_and(|code| PKEXEC_REFUSED.contains(&code))
            {
                return Outcome::Failed(
                    "Not authorised: the password prompt was cancelled.".into(),
                );
            }
            let lines = shared.lock().unwrap().lines.clone();
            return Outcome::Failed(
                failure_reason(&lines).unwrap_or_else(|| format!("exited with {status}")),
            );
        }
        shared.lock().unwrap().done += 1;
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

    #[test]
    fn colour_and_cursor_sequences_go_and_their_text_stays() {
        assert_eq!(clean("\x1b[31merror\x1b[0m: no cargo"), "error: no cargo");
        assert_eq!(clean("\x1b[38;5;208mwarn\x1b[m"), "warn");
        assert_eq!(clean("\x1b[2K\x1b[1Gdone"), "done");
    }

    #[test]
    fn a_progress_line_is_its_last_redraw_that_says_anything() {
        assert_eq!(clean("  0%\r 50%\r100%\r"), "100%");
        assert_eq!(clean("\r\r"), "");
        assert_eq!(clean("Compiling\r\x1b[32mFinished\x1b[0m"), "Finished");
    }

    /// Only `ESC [` sequences are understood. Any other escape loses the
    /// ESC and the character after it, so a terminal title's text and its
    /// BEL stay in the line.
    #[test]
    fn an_osc_title_loses_its_introducer_and_keeps_its_text() {
        assert_eq!(clean("\x1b]0;building\x07ready"), "0;building\x07ready");
        assert_eq!(clean("\x1b(Bplain"), "Bplain");
    }

    fn env() -> Env {
        Env {
            checkout: "/src/app".into(),
            worktree: "/share/worktrees/app".into(),
            off_branch: OffBranch::Worktree,
            repo_url: "https://example.invalid/app.git".into(),
            release: "https://example.invalid/release".into(),
        }
    }

    fn step(root: bool) -> Step {
        Step {
            script: "true".into(),
            root,
        }
    }

    /// pkexec clears the environment, so a root step's variables can only
    /// arrive as `env` arguments.
    #[test]
    fn a_root_step_gets_its_variables_as_pkexec_env_arguments() {
        let cmd = command(&step(true), &env());
        assert_eq!(cmd.get_program(), "pkexec");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "env");
        for var in [
            "CHECKOUT=/src/app",
            "WORKTREE=/share/worktrees/app",
            "OFF_BRANCH=worktree",
            "REPO_URL=https://example.invalid/app.git",
            "RELEASE=https://example.invalid/release",
            "NO_COLOR=1",
        ] {
            assert!(args.iter().any(|a| a == var), "{var} missing from {args:?}");
        }
        assert_eq!(args[args.len() - 3..], ["bash", "-c", "true"]);
    }

    #[test]
    fn a_user_step_gets_its_variables_from_the_environment() {
        let cmd = command(&step(false), &env());
        assert_eq!(cmd.get_program(), "bash");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, ["-c", "true"]);
        let checkout = cmd
            .get_envs()
            .find(|(name, _)| *name == "CHECKOUT")
            .and_then(|(_, value)| value);
        assert_eq!(checkout, Some("/src/app".as_ref()));
    }

    #[test]
    fn a_job_counts_the_steps_it_has_finished() {
        let notify: Notify = Arc::new(|_| {});
        let steps = vec![step(false), step(false)];
        let job = Job::start(
            Id::Ovfetch,
            Channel::Release,
            Action::Update,
            steps,
            env(),
            notify,
        );
        assert_eq!(job.wait(), Outcome::Done);
        assert_eq!(job.steps(), (2, 2));
    }

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_failure_is_explained_by_the_installers_last_cross_line() {
        let output = lines(&["building", "  ✗ no cargo", "✗ gave up", "cleaning up", ""]);
        assert_eq!(failure_reason(&output).as_deref(), Some("gave up"));
    }

    #[test]
    fn a_failure_without_a_cross_line_is_explained_by_the_last_line_that_says_something() {
        let output = lines(&["building", "error: linker not found", "  ", ""]);
        assert_eq!(
            failure_reason(&output).as_deref(),
            Some("error: linker not found")
        );
        assert_eq!(failure_reason(&lines(&["", " "])), None);
    }
}
