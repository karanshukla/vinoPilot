//! The CLI's contract with scripts and other frontends (docs/json-api.md):
//! exit codes and the `--json` shape. Change these only on purpose, with a
//! changelog entry.

use std::fs;
use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

/// A throwaway `$HOME`, removed on drop.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("vinopilot-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Home(dir)
    }

    /// `vinopilot ARGS` with only this home and the system's `PATH`.
    fn run(&self, args: &[&str]) -> Output {
        self.run_with(args, &[])
    }

    fn run_with(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vinopilot"));
        cmd.args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.0);
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.output().unwrap()
    }

    /// An executable shell script at `relative`.
    fn script(&self, relative: &str, body: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// `list --json`, which must succeed.
    fn list(&self) -> Value {
        let out = self.run(&["list", "--json"]);
        assert!(out.status.success(), "{}", stderr(&out));
        serde_json::from_slice(&out.stdout).unwrap()
    }

    /// `vinopilot install vinopilot --json` from a `file://` release whose
    /// `install.sh` is `body`. The events, and the exit code.
    fn install_from_release(&self, body: &str) -> (Vec<Value>, Option<i32>) {
        self.script("release/install.sh", body);
        let release = format!("file://{}", self.0.join("release").display());
        let out = self.run_with(
            &["install", "vinopilot", "--json"],
            &[("VINOPILOT_RELEASE_BASE", &release)],
        );
        let events = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{line}: {e}")))
            .collect();
        (events, out.status.code())
    }
}

impl Deref for Home {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn component<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no {id}"))
}

/// Found by where it lives, not in `$HOME`: vinoAuthFace's files, and a
/// system-wide Flatpak or distro package. The machine running the tests may
/// have these.
fn found_outside_any_home(c: &Value) -> bool {
    let channel = c["installed"]["channel"].as_str().unwrap_or("");
    c["id"] == "vinoauthface" || ["flatpak", "deb", "rpm"].contains(&channel)
}

#[test]
fn list_json_names_every_component() {
    let home = Home::new("list-ids");
    let doc = home.list();
    assert_eq!(doc["vinopilot"], env!("CARGO_PKG_VERSION"));
    assert_eq!(doc["source_root"], home.join("src").to_str().unwrap());
    let ids: Vec<&str> = doc["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "vinopilot",
            "vinowhisper",
            "vinoauthface",
            "vinoopeneffects",
            "vinopop",
            "vinospeak",
            "vinosearch",
            "vinowatch",
            "ovfetch",
            "vinouserinterface",
        ]
    );
}

#[test]
fn every_component_has_every_field() {
    let doc = Home::new("list-fields").list();
    for c in doc["components"].as_array().unwrap() {
        for key in [
            "name",
            "blurb",
            "repo",
            "channels",
            "registry",
            "needs_root",
            "installed",
            "checkout",
            "system_package",
            "shadowed",
            "update",
        ] {
            assert!(c.get(key).is_some(), "{} has no {key}", c["id"]);
        }
    }
}

#[test]
fn list_has_no_latest_because_it_stays_offline() {
    let doc = Home::new("list-offline").list();
    for c in doc["components"].as_array().unwrap() {
        assert!(c.get("latest").is_none(), "{}", c["id"]);
        assert_eq!(c["update"], json!({"state": "unknown", "reason": null}));
    }
}

#[test]
fn an_empty_home_has_nothing_installed_and_nothing_shadowed() {
    let doc = Home::new("list-empty").list();
    for c in doc["components"].as_array().unwrap() {
        if !found_outside_any_home(c) {
            assert!(
                c["installed"].is_null(),
                "{} found in an empty home",
                c["id"]
            );
        }
        assert_eq!(c["shadowed"], json!([]), "{}", c["id"]);
    }
}

#[test]
fn channels_are_listed_best_first() {
    let doc = Home::new("list-channels").list();
    assert_eq!(
        component(&doc, "vinowhisper")["channels"],
        json!(["release", "source", "package", "flatpak"])
    );
    assert_eq!(
        component(&doc, "vinosearch")["channels"],
        json!(["release", "source", "flatpak"])
    );
    assert_eq!(
        component(&doc, "vinouserinterface")["channels"],
        json!(["source"])
    );
}

#[test]
fn registry_names_where_a_package_is_published() {
    let doc = Home::new("list-registry").list();
    for (id, registry) in [
        ("vinowhisper", json!("pypi")),
        ("vinospeak", json!("pypi")),
        ("ovfetch", json!("crates.io")),
        ("vinopilot", json!("crates.io")),
        ("vinouserinterface", json!("crates.io")),
        ("vinopop", Value::Null),
        ("vinosearch", Value::Null),
        ("vinoauthface", Value::Null),
    ] {
        assert_eq!(component(&doc, id)["registry"], registry, "{id}");
    }
}

#[test]
fn a_command_in_both_bins_is_listed_as_shadowed() {
    let home = Home::new("shadowed");
    let local = home.script(".local/bin/vinosearch", "echo 'vinosearch 8.0.0'\n");
    let cargo = home.script(".cargo/bin/vinosearch", "echo 'vinosearch 7.0.0'\n");
    let doc = home.list();
    assert_eq!(
        component(&doc, "vinosearch")["shadowed"],
        json!([{"command": "vinosearch", "runs": local, "hidden": cargo}])
    );
}

#[test]
fn an_action_prints_line_events_then_one_done_event() {
    let home = Home::new("events-done");
    let (events, code) =
        home.install_from_release("echo '== Installing'\necho 'checksum verified'\n");
    assert_eq!(
        events,
        [
            json!({"event": "line", "text": "== Installing"}),
            json!({"event": "line", "text": "checksum verified"}),
            json!({"event": "done", "app": "vinopilot", "channel": "release", "result": {"outcome": "done"}}),
        ]
    );
    assert_eq!(code, Some(0));
}

#[test]
fn a_failed_action_gives_the_installers_cross_line_as_its_reason_and_exits_1() {
    let home = Home::new("events-failed");
    let (events, code) = home.install_from_release(
        "echo '== Installing'\necho '✗ No release for this machine' >&2\necho 'bye'\nexit 1\n",
    );
    assert_eq!(
        events.last().unwrap(),
        &json!({
            "event": "done",
            "app": "vinopilot",
            "channel": "release",
            "result": {"outcome": "failed", "reason": "No release for this machine"},
        })
    );
    assert!(
        events[..events.len() - 1]
            .iter()
            .all(|e| e["event"] == "line")
    );
    assert_eq!(code, Some(1));
}

#[test]
fn steps_prints_the_script_without_running_it() {
    let home = Home::new("steps");
    let out = home.run(&["steps", "watch", "install", "--via", "release"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("release_run install.sh"), "{text}");
    assert!(
        text.contains("RELEASE=https://github.com/karanshukla/vinoWatch/releases/latest/download")
    );
}

#[test]
fn config_json_is_the_source_root_that_list_uses() {
    let home = Home::new("config");
    let set = home.run(&["config", "source_root", "~/code"]);
    assert!(set.status.success(), "{}", stderr(&set));
    let out = home.run(&["config", "--json"]);
    let config: Value = serde_json::from_slice(&out.stdout).unwrap();
    let code = home.join("code");
    assert_eq!(config, json!({"source_root": code}));
    assert_eq!(home.list()["source_root"], json!(code));
    assert!(home.join(".config/vinopilot/config").is_file());
}

#[test]
fn usage_errors_exit_2() {
    let home = Home::new("usage");
    for args in [
        &["install", "nope"][..],
        &["install"],
        &["install", "pop", "--via", "pypi"],
        &["install", "pop", "--via", "flatpak"],
        &["frobnicate"],
        &["--bogus"],
    ] {
        let out = home.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
}

#[test]
fn uninstall_without_yes_or_a_terminal_is_a_usage_error() {
    let home = Home::new("uninstall-noyes");
    let out = home.run(&["uninstall", "ovfetch", "--via", "release", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--yes"), "{}", stderr(&out));
    assert!(out.stdout.is_empty(), "it ran");
}

#[test]
fn uninstalling_what_isnt_there_fails_without_running_anything() {
    let home = Home::new("uninstall");
    let out = home.run(&["uninstall", "watch", "--yes"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("isn't installed"));
}

#[test]
fn version() {
    let out = Home::new("version").run(&["--version"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("vinopilot {}", env!("CARGO_PKG_VERSION"))
    );
}
