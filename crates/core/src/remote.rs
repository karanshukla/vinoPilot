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

/// A GET: the body of a 200, or why there isn't one. [`curl`] is the real
/// one; the lookups take it as an argument so the tests can answer for it.
type Fetch<'a> = &'a dyn Fn(&str, &str) -> Result<String, String>;

/// `url` through curl, asking for `accept`. The only thing here that opens
/// a connection, besides `git fetch`.
fn curl(url: &str, accept: &str) -> Result<String, String> {
    let out = crate::command("curl")
        .args(["-sSL", "--max-time", "10", "--max-filesize", "4194304"])
        .args(["-A", USER_AGENT, "-H", &format!("Accept: {accept}")])
        .args(["-w", "\n%{http_code}", url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "curl isn't installed".to_string())?;
    read_response(&String::from_utf8_lossy(&out.stdout), out.status.success())
}

/// What curl printed, as the body of a 200 or a sentence for the row: the
/// body, then the status code on a line of its own (`-w "\n%{http_code}"`).
fn read_response(printed: &str, curl_succeeded: bool) -> Result<String, String> {
    let (body, code) = printed.rsplit_once('\n').unwrap_or(("", printed));
    match code.trim() {
        "200" => Ok(body.to_string()),
        "404" => Err("not published yet".into()),
        "403" | "429" => Err("rate-limited; try again later".into()),
        _ if !curl_succeeded => Err("offline, or the server didn't answer".into()),
        code => Err(format!("the server said {code}")),
    }
}

pub fn release(repo: &str) -> Result<String, String> {
    release_from(&curl, repo)
}

fn release_from(fetch: Fetch, repo: &str) -> Result<String, String> {
    let json = fetch(
        &format!("https://api.github.com/repos/{repo}/releases/latest"),
        "application/vnd.github+json",
    )?;
    vino_ui::update::latest_tag(&json).ok_or_else(|| "no release yet".to_string())
}

/// The longest version a registry may answer with and still be shown.
const MAX_VERSION_LEN: usize = 32;

/// A version string from a registry, held to the same characters as a tag:
/// it's shown in the window.
fn plain(version: &str) -> Option<String> {
    let ok = version.len() <= MAX_VERSION_LEN && crate::version::is_plain(version);
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
    package_from(&curl, package)
}

fn package_from(fetch: Fetch, package: Package) -> Result<String, String> {
    match package {
        Package::PyPi(name) => {
            let json = fetch(
                &format!("https://pypi.org/pypi/{name}/json"),
                "application/json",
            )?;
            pypi_version(&json).ok_or_else(|| "not on PyPI".into())
        }
        Package::Crate(name) | Package::Library(name) => {
            let json = fetch(
                &format!("https://crates.io/api/v1/crates/{name}"),
                "application/json",
            )?;
            crates_version(&json).ok_or_else(|| "not on crates.io".into())
        }
    }
}

/// What a git command said: whether it exited zero, and its stdout.
struct GitAnswer {
    succeeded: bool,
    stdout: String,
}

/// The commit a worktree install built, recorded by the catalog's
/// `checkout_done` when the checkout itself is on another branch.
const INSTALLED_REF: &str = "refs/vinopilot/installed";

fn behind(checkout: &Path) -> Result<u32, String> {
    behind_by(&|args| {
        let out = crate::command("git")
            .arg("-C")
            .arg(checkout)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "git isn't installed".to_string())?;
        Ok(GitAnswer {
            succeeded: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        })
    })
}

/// Commits behind after a fetch: from what was installed when a worktree
/// install recorded it, else from the checkout's own branch to its upstream.
fn behind_by(git: &dyn Fn(&[&str]) -> Result<GitAnswer, String>) -> Result<u32, String> {
    if !git(&["fetch", "--quiet"])?.succeeded {
        return Err("git fetch failed".into());
    }
    let recorded = git(&["rev-parse", "--verify", "--quiet", INSTALLED_REF])?.succeeded;
    let (range, missing) = if recorded {
        (
            "refs/vinopilot/installed..refs/remotes/origin/HEAD",
            "origin has no default branch",
        )
    } else {
        ("HEAD..@{upstream}", "the branch has no upstream")
    };
    let count = git(&["rev-list", "--count", range])?;
    if !count.succeeded {
        return Err(missing.into());
    }
    count
        .stdout
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

    #[test]
    fn a_200_is_its_body_and_every_other_answer_a_sentence() {
        assert_eq!(
            read_response("{\"a\":\n1}\n200", true).unwrap(),
            "{\"a\":\n1}"
        );
        let why = |printed, ok| read_response(printed, ok).unwrap_err();
        assert_eq!(why("Not Found\n404", true), "not published yet");
        assert_eq!(why("\n403", true), "rate-limited; try again later");
        assert_eq!(why("\n429", true), "rate-limited; try again later");
        assert_eq!(why("oops\n500", true), "the server said 500");
        assert_eq!(why("\n000", false), "offline, or the server didn't answer");
        assert_eq!(why("", false), "offline, or the server didn't answer");
    }

    /// A fetch that answers `body` and writes down what it was asked for.
    fn answering<'a>(
        body: Result<&'a str, &'a str>,
        asked: &'a std::cell::RefCell<Vec<String>>,
    ) -> impl Fn(&str, &str) -> Result<String, String> + 'a {
        move |url, _accept| {
            asked.borrow_mut().push(url.to_string());
            body.map(str::to_string).map_err(str::to_string)
        }
    }

    #[test]
    fn a_release_is_the_latest_tag_github_names() {
        let asked = Default::default();
        let fetch = answering(Ok(r#"{"tag_name":"v8"}"#), &asked);
        assert_eq!(
            release_from(&fetch, "karanshukla/vinoSearch").unwrap(),
            "v8"
        );
        assert_eq!(
            *asked.borrow(),
            ["https://api.github.com/repos/karanshukla/vinoSearch/releases/latest"]
        );
        let fetch = answering(Ok("{}"), &asked);
        assert_eq!(release_from(&fetch, "a/b").unwrap_err(), "no release yet");
        let fetch = answering(Err("not published yet"), &asked);
        assert_eq!(
            release_from(&fetch, "a/b").unwrap_err(),
            "not published yet"
        );
    }

    #[test]
    fn a_package_is_asked_of_its_own_registry() {
        let asked = Default::default();
        let pypi = answering(Ok(r#"{"info":{"version":"1.3.1"}}"#), &asked);
        assert_eq!(
            package_from(&pypi, Package::PyPi("vinowhisper")).unwrap(),
            "1.3.1"
        );
        let crates = answering(Ok(r#"{"crate":{"max_version":"0.6.0"}}"#), &asked);
        assert_eq!(
            package_from(&crates, Package::Crate("ovfetch")).unwrap(),
            "0.6.0"
        );
        assert_eq!(
            *asked.borrow(),
            [
                "https://pypi.org/pypi/vinowhisper/json",
                "https://crates.io/api/v1/crates/ovfetch"
            ]
        );
        let empty = answering(Ok("{}"), &asked);
        assert_eq!(
            package_from(&empty, Package::PyPi("x")).unwrap_err(),
            "not on PyPI"
        );
        assert_eq!(
            package_from(&empty, Package::Library("x")).unwrap_err(),
            "not on crates.io"
        );
    }

    /// A git whose `fetch` and `rev-parse` succeed or not, and whose
    /// `rev-list --count RANGE` prints `count`; the range it was asked for
    /// is written down.
    fn fake_git<'a>(
        fetches: bool,
        recorded: bool,
        count: Option<&'a str>,
        range: &'a std::cell::RefCell<String>,
    ) -> impl Fn(&[&str]) -> Result<GitAnswer, String> + 'a {
        move |args| {
            let (succeeded, stdout) = match args[0] {
                "fetch" => (fetches, ""),
                "rev-parse" => (recorded, ""),
                "rev-list" => {
                    *range.borrow_mut() = args[2].to_string();
                    (count.is_some(), count.unwrap_or(""))
                }
                other => panic!("unexpected git {other}"),
            };
            Ok(GitAnswer {
                succeeded,
                stdout: stdout.to_string(),
            })
        }
    }

    #[test]
    fn a_checkout_is_counted_from_its_branch_to_its_upstream() {
        let range = Default::default();
        assert_eq!(
            behind_by(&fake_git(true, false, Some("3\n"), &range)),
            Ok(3)
        );
        assert_eq!(*range.borrow(), "HEAD..@{upstream}");
        assert_eq!(
            behind_by(&fake_git(true, false, None, &range)).unwrap_err(),
            "the branch has no upstream"
        );
    }

    #[test]
    fn a_worktree_install_is_counted_from_the_commit_it_recorded() {
        let range = Default::default();
        assert_eq!(behind_by(&fake_git(true, true, Some("0\n"), &range)), Ok(0));
        assert_eq!(
            *range.borrow(),
            "refs/vinopilot/installed..refs/remotes/origin/HEAD"
        );
        assert_eq!(
            behind_by(&fake_git(true, true, None, &range)).unwrap_err(),
            "origin has no default branch"
        );
    }

    #[test]
    fn a_failed_fetch_or_an_odd_count_is_said_not_guessed() {
        let range = Default::default();
        assert_eq!(
            behind_by(&fake_git(false, false, Some("3"), &range)).unwrap_err(),
            "git fetch failed"
        );
        assert_eq!(
            behind_by(&fake_git(true, false, Some("many"), &range)).unwrap_err(),
            "git said something odd"
        );
        let no_git = |_: &[&str]| Err("git isn't installed".to_string());
        assert_eq!(behind_by(&no_git).unwrap_err(), "git isn't installed");
    }

    #[test]
    fn a_registry_version_of_32_characters_is_shown_and_33_is_not() {
        assert!(plain(&"1".repeat(MAX_VERSION_LEN)).is_some());
        assert!(plain(&"1".repeat(MAX_VERSION_LEN + 1)).is_none());
    }
}
