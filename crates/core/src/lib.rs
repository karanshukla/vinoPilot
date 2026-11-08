//! vinopilot's core: the catalog of vinoPilot components and the shell each
//! install, update and uninstall runs ([`catalog`]); what's on this machine
//! ([`detect`]); what's newest ([`remote`]); and running an action
//! ([`job`]). No UI: the `vinopilot` CLI and the `vinopilot-gui` window are
//! both built on it, and the CLI's `--json` output is the API for frontends
//! in other languages.

#![forbid(unsafe_code)]

pub mod catalog;
pub mod config;
pub mod detect;
pub mod job;
pub mod remote;
pub mod status;

pub use catalog::{Action, Channel, Id};
pub use config::Config;

/// A `Command` whose `PATH` has `~/.local/bin` and `~/.cargo/bin` in front
/// when missing: a launcher started from the app menu often lacks them, and
/// every installer puts things there. Every child vinopilot runs goes
/// through this.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    cmd.env("PATH", search_path());
    cmd
}

pub(crate) fn search_path() -> &'static std::ffi::OsString {
    static PATH: std::sync::OnceLock<std::ffi::OsString> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let home = config::home();
        let current = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<_> = std::env::split_paths(&current).collect();
        for dir in [home.join(".cargo/bin"), home.join(".local/bin")] {
            if !dirs.contains(&dir) {
                dirs.insert(0, dir);
            }
        }
        std::env::join_paths(dirs).unwrap_or(current)
    })
}
