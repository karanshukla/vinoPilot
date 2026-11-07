//! The newest version of each component, from each place it ships: GitHub
//! releases, PyPI, crates.io and the checkout's upstream. Only
//! [`check`] goes online, and only when the user asks.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use crate::catalog::{Id, Package};
use crate::detect::Paths;

const USER_AGENT: &str = concat!(
    "vinopilot/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/karanshukla/vinopilot)"
);

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Latest {
    /// The newest GitHub release's tag.
    #[serde(serialize_with = "found")]
    pub release: Option<Result<String, String>>,
    /// The newest version on PyPI or crates.io.
    #[serde(serialize_with = "found")]
    pub package: Option<Result<String, String>>,
    /// Commits the checkout is behind its upstream, after a fetch.
    #[serde(serialize_with = "found")]
    pub behind: Option<Result<u32, String>>,
}

/// `null`, `{"value": ...}` or `{"error": "..."}`: the JSON API's shape for
/// a lookup that may not have happened, or may have failed.
fn found<T: serde::Serialize, S: serde::Serializer>(
    value: &Option<Result<T, String>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    match value {
        None => s.serialize_none(),
        Some(result) => {
            let mut map = s.serialize_map(Some(1))?;
            match result {
                Ok(v) => map.serialize_entry("value", v)?,
                Err(e) => map.serialize_entry("error", e)?,
            }
            map.end()
        }
    }
}

/// The body of a 200, or why there isn't one.
fn get(url: &str, accept: &str) -> Result<String, String> {
    let out = crate::command("curl")
        .args(["-sSL", "--max-time", "10", "--max-filesize", "4194304"])
        .args(["-A", USER_AGENT, "-H", &format!("Accept: {accept}")])
        .args(["-w", "\n%{http_code}", url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "curl isn't installed".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    match code.trim() {
        "200" => Ok(body.to_string()),
        "404" => Err("not published yet".into()),
        "403" | "429" => Err("rate-limited; try again later".into()),
        _ if !out.status.success() => Err("offline, or the server didn't answer".into()),
        code => Err(format!("the server said {code}")),
    }
}

pub fn release(repo: &str) -> Result<String, String> {
    let json = get(
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
        "application/vnd.github+json",
    )?;
    vino_ui::update::latest_tag(&json).ok_or_else(|| "no release yet".to_string())
}

/// A version string from a registry, held to the same characters as a tag:
/// it's shown in the window.
fn plain(version: &str) -> Option<String> {
    let ok = !version.is_empty()
        && version.len() <= 32
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+');
    ok.then(|| version.to_string())
}

pub fn pypi_version(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    plain(value["info"]["version"].as_str()?)
}

pub fn crates_version(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let krate = &value["crate"];
    plain(
        krate["max_stable_version"]
            .as_str()
            .or_else(|| krate["max_version"].as_str())?,
    )
}

pub fn package(package: Package) -> Result<String, String> {
    match package {
        Package::PyPi(name) => {
            let json = get(
                &format!("https://pypi.org/pypi/{name}/json"),
                "application/json",
            )?;
            pypi_version(&json).ok_or_else(|| "not on PyPI".into())
        }
        Package::Crate(name) | Package::Library(name) => {
            let json = get(
                &format!("https://crates.io/api/v1/crates/{name}"),
                "application/json",
            )?;
            crates_version(&json).ok_or_else(|| "not on crates.io".into())
        }
    }
}

fn behind(checkout: &Path) -> Result<u32, String> {
    let run = |args: &[&str]| {
        crate::command("git")
            .arg("-C")
            .arg(checkout)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "git isn't installed".to_string())
    };
    if !run(&["fetch", "--quiet"])?.status.success() {
        return Err("git fetch failed".into());
    }
    let out = run(&["rev-list", "--count", "HEAD..@{upstream}"])?;
    if !out.status.success() {
        return Err("the branch has no upstream".into());
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(|_| "git said something odd".into())
}

/// Everything at once, one thread per lookup.
pub fn check(paths: &Paths) -> HashMap<Id, Latest> {
    let handles: Vec<_> = Id::ALL
        .into_iter()
        .map(|id| {
            let checkout = paths.checkout(id);
            std::thread::spawn(move || {
                let release_t =
                    (id != Id::Ui).then(|| std::thread::spawn(move || release(id.repo())));
                let package_t = id.package().map(|p| std::thread::spawn(move || package(p)));
                let behind = checkout.join(".git").exists().then(|| behind(&checkout));
                let join = |t: std::thread::JoinHandle<Result<String, String>>| {
                    t.join().unwrap_or_else(|_| Err("the check crashed".into()))
                };
                let latest = Latest {
                    release: release_t.map(join),
                    package: package_t.map(join),
                    behind,
                };
                (id, latest)
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_answers() {
        assert_eq!(
            pypi_version(r#"{"info":{"version":"1.3.1"}}"#).as_deref(),
            Some("1.3.1")
        );
        assert_eq!(
            crates_version(r#"{"crate":{"max_version":"0.4.0-rc1","max_stable_version":"0.3.0"}}"#)
                .as_deref(),
            Some("0.3.0")
        );
        assert_eq!(pypi_version(r#"{"info":{"version":"<b>1</b>"}}"#), None);
        assert_eq!(crates_version("not json"), None);
    }
}
