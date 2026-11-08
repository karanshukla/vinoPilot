//! What's on this machine: which way each component was installed, its
//! version, and its checkout. Local only: nothing here goes online.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use vino_ui::App;

use crate::catalog::{Channel, Id};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Local {
    /// How it's installed, when it is.
    pub installed: Option<Installed>,
    pub checkout: Option<Checkout>,
    /// A distro package with the same commands, which vinoPilot can't
    /// remove: an old `rpm`/`deb` build, say.
    pub system_package: Option<SystemPackage>,
    /// Commands in both `~/.local/bin` and `~/.cargo/bin`: a release install
    /// and a cargo one side by side, only one of which runs.
    pub shadowed: Vec<Shadowed>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Shadowed {
    /// `vinosearch`.
    pub command: String,
    /// The copy that comes first on `PATH`, so the one that runs.
    pub runs: PathBuf,
    /// The copy it hides.
    pub hidden: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemPackage {
    /// `rpm` or `deb`.
    pub kind: &'static str,
    /// `vinoopeneffects`, `vinoopeneffects-daemon`.
    pub packages: Vec<String>,
    /// `sudo dnf remove vinoopeneffects vinoopeneffects-daemon`.
    pub remove: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Installed {
    pub channel: Channel,
    /// `1.4.0`, `dev`, or `None` when the binary wouldn't say.
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Checkout {
    pub path: PathBuf,
    /// `git describe --tags --always --dirty`.
    pub describe: String,
}

/// Where things live, so the tests can point at a throwaway home.
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub source_root: PathBuf,
    /// `/usr/bin`, where a distro package puts its commands.
    pub system_bin: PathBuf,
}

impl Paths {
    fn local_bin(&self, name: &str) -> PathBuf {
        self.home.join(".local/bin").join(name)
    }

    fn cargo_bin(&self, name: &str) -> PathBuf {
        self.home.join(".cargo/bin").join(name)
    }

    pub fn checkout(&self, id: Id) -> PathBuf {
        self.source_root.join(id.checkout_name())
    }
}

/// The installs `cargo install` keeps in `~/.cargo/.crates.toml`: crate name
/// to where it came from.
pub fn parse_crates_toml(text: &str) -> HashMap<String, (String, Channel)> {
    let mut out = HashMap::new();
    for line in text.lines() {
        // "ovfetch 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["ovfetch"]
        let Some(key) = line.strip_prefix('"').and_then(|l| l.split('"').next()) else {
            continue;
        };
        let mut parts = key.splitn(3, ' ');
        let (Some(name), Some(version), Some(source)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let channel = if source.starts_with("(registry+") || source.starts_with("(sparse+") {
            Channel::Package
        } else {
            Channel::Source
        };
        out.insert(name.to_string(), (version.to_string(), channel));
    }
    out
}

/// How a `uv tool` was installed, from its `uv-receipt.toml`: by name from
/// PyPI, a release's wheel, or a local directory.
pub fn parse_uv_receipt(text: &str) -> Channel {
    let requirements = text
        .lines()
        .find(|l| l.trim_start().starts_with("requirements"))
        .unwrap_or("");
    if requirements.contains(".whl") {
        Channel::Release
    } else if requirements.contains("directory") || requirements.contains("editable") {
        Channel::Source
    } else {
        Channel::Package
    }
}

/// `uv tool list`'s `name vX.Y.Z` lines.
pub fn parse_uv_list(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|l| !l.starts_with('-') && !l.starts_with(' '))
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let name = words.next()?;
            let version = words.next()?.strip_prefix('v')?;
            Some((name.to_string(), version.to_string()))
        })
        .collect()
}

/// The version in a `--version` line: `vinosearch 8.0.0` is `8.0.0`,
/// `vinoauthface dev` is `dev`.
pub fn version_in(text: &str) -> Option<String> {
    let line = text.lines().next()?;
    line.split_whitespace().skip(1).find_map(|word| {
        let word = word.strip_prefix('v').unwrap_or(word);
        let numeric = word.starts_with(|c: char| c.is_ascii_digit())
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+');
        (numeric || word == "dev").then(|| word.to_string())
    })
}

/// The `version = "..."` of a Cargo.toml's `[package]`.
pub fn cargo_toml_version(text: &str) -> Option<String> {
    let package = text.split("[package]").nth(1)?;
    let section = package.split("\n[").next()?;
    section.lines().find_map(|l| {
        let rest = l
            .trim()
            .strip_prefix("version")?
            .trim_start()
            .strip_prefix('=')?;
        Some(rest.trim().trim_matches('"').to_string())
    })
}

/// Runs `program --version` for at most two seconds: a binary that opens a
/// window instead of answering mustn't hang the scan.
fn ask_version(program: &Path) -> Option<String> {
    let mut child = crate::command(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    version_in(&String::from_utf8_lossy(&out.stdout))
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = crate::command("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn checkout(paths: &Paths, id: Id) -> Option<Checkout> {
    let path = paths.checkout(id);
    if !path.join(".git").exists() {
        return None;
    }
    let describe = git(&path, &["describe", "--tags", "--always", "--dirty"]).unwrap_or_default();
    Some(Checkout { path, describe })
}

/// The commands `id` installs, and a distro package of it would put in
/// `/usr/bin`.
fn commands(id: Id) -> &'static [&'static str] {
    match id {
        Id::App(App::Whisper) => &["vinowhisper", "vinowhisper-setup", "vinowhisper-tray"],
        Id::App(App::Speak) => &["vinospeak", "vinospeak-gui"],
        Id::App(App::Pop) => &["vinopop"],
        Id::App(App::Search) => &["vinosearch", "vinosearch-settings"],
        Id::App(App::Watch) => &["vinowatch"],
        Id::App(App::OpenEffects) => &[
            "vinoopeneffects",
            "vinoopeneffects-ctl",
            "vinoopeneffects-daemon",
        ],
        Id::App(App::AuthFace) => &["vinoauthface"],
        Id::Ovfetch => &["ovfetch"],
        Id::Pilot => &["vinopilot", "vinopilot-gui"],
        Id::Ui => &[],
    }
}

/// `rpm -qf --qf '%{NAME}\n'`: a name per owned file, a sentence for the rest.
pub fn parse_rpm_owners(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains(' '))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// `dpkg-query -S`: `pkg: /usr/bin/x`, or `a, b: /usr/bin/x` when shared.
pub fn parse_dpkg_owners(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|l| l.split_once(": /").map(|(pkgs, _)| pkgs))
        .flat_map(|pkgs| pkgs.split(", "))
        .map(|p| p.split(':').next().unwrap_or(p).trim().to_string())
        .filter(|p| !p.is_empty() && !p.contains(' '))
        .collect();
    names.sort();
    names.dedup();
    names
}

fn has(program: &str) -> bool {
    ["/usr/bin", "/bin", "/usr/sbin"]
        .iter()
        .any(|dir| Path::new(dir).join(program).exists())
}

/// What a package manager would remove `packages` with.
fn remove_command(rpm: bool, packages: &[String]) -> String {
    let tool = if !rpm {
        "sudo apt remove"
    } else if Path::new("/run/ostree-booted").exists() {
        "sudo rpm-ostree uninstall"
    } else if has("dnf") {
        "sudo dnf remove"
    } else if has("zypper") {
        "sudo zypper remove"
    } else {
        "sudo rpm -e"
    };
    format!("{tool} {}", packages.join(" "))
}

/// The distro packages that own any of `id`'s commands in `/usr/bin`. Asks
/// the package manager only when one of them is there.
fn system_package(paths: &Paths, id: Id) -> Option<SystemPackage> {
    let files: Vec<PathBuf> = commands(id)
        .iter()
        .map(|c| paths.system_bin.join(c))
        .filter(|p| p.exists())
        .collect();
    if files.is_empty() {
        return None;
    }
    let ask = |program: &str, args: &[&str]| -> Option<String> {
        let out = crate::command(program)
            .args(args)
            .args(&files)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let (rpm, packages) = [
        (
            true,
            ask("rpm", &["-qf", "--qf", "%{NAME}\\n"]).map(|t| parse_rpm_owners(&t)),
        ),
        (
            false,
            ask("dpkg-query", &["-S"]).map(|t| parse_dpkg_owners(&t)),
        ),
    ]
    .into_iter()
    .find_map(|(rpm, found)| found.filter(|p| !p.is_empty()).map(|p| (rpm, p)))?;
    Some(SystemPackage {
        kind: if rpm { "rpm" } else { "deb" },
        remove: remove_command(rpm, &packages),
        packages,
    })
}

/// `id`'s commands that are in both `~/.local/bin` and `~/.cargo/bin`, the
/// one that runs being whichever comes first on `path` (`~/.local/bin` when
/// neither is on it).
fn shadowed(paths: &Paths, id: Id, path: &std::ffi::OsStr) -> Vec<Shadowed> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path).collect();
    let rank = |dir: &Path| dirs.iter().position(|d| d == dir).unwrap_or(usize::MAX);
    let (local, cargo) = (paths.home.join(".local/bin"), paths.home.join(".cargo/bin"));
    let local_first = rank(&local) <= rank(&cargo);
    commands(id)
        .iter()
        .filter_map(|c| {
            let (l, k) = (local.join(c), cargo.join(c));
            if !l.exists() || !k.exists() || l.canonicalize().ok() == k.canonicalize().ok() {
                return None;
            }
            let (runs, hidden) = if local_first { (l, k) } else { (k, l) };
            Some(Shadowed {
                command: (*c).to_string(),
                runs,
                hidden,
            })
        })
        .collect()
}

/// The first of `candidates` that exists.
fn first(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// What each component looks like on this machine.
pub fn scan(paths: &Paths) -> HashMap<Id, Local> {
    let crates = std::fs::read_to_string(paths.home.join(".cargo/.crates.toml"))
        .map(|t| parse_crates_toml(&t))
        .unwrap_or_default();
    let uv_list = crate::command("uv")
        .args(["tool", "list"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| parse_uv_list(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    let uv_receipt = |tool: &str| {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| paths.home.join(".local/share"));
        std::fs::read_to_string(data.join("uv/tools").join(tool).join("uv-receipt.toml")).ok()
    };

    let handles: Vec<_> = Id::ALL
        .into_iter()
        .map(|id| {
            let paths = paths.clone();
            let crates = crates.clone();
            let uv_list = uv_list.clone();
            let receipt = match id {
                Id::App(App::Whisper) => uv_receipt("vinowhisper"),
                Id::App(App::Speak) => uv_receipt("vinospeak"),
                _ => None,
            };
            std::thread::spawn(move || (id, local(&paths, id, &crates, &uv_list, receipt)))
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

fn local(
    paths: &Paths,
    id: Id,
    crates: &HashMap<String, (String, Channel)>,
    uv_list: &HashMap<String, String>,
    uv_receipt: Option<String>,
) -> Local {
    let installed = |channel, version| Some(Installed { channel, version });
    let by_marker = |marker: &str, bin: &str| -> Option<Installed> {
        let bin = first(&[paths.local_bin(bin), paths.cargo_bin(bin)])?;
        let channel = if paths.local_bin(marker).exists() {
            Channel::Release
        } else {
            Channel::Source
        };
        installed(channel, ask_version(&bin))
    };
    let from_crates = |name: &str| -> Option<Installed> {
        let (version, channel) = crates.get(name)?;
        // The record outlives a binary deleted by hand.
        paths.cargo_bin(name).exists().then_some(())?;
        installed(*channel, Some(version.clone()))
    };
    let from_uv = |tool: &str, bin: &str| -> Option<Installed> {
        match (uv_receipt.as_deref(), uv_list.get(tool)) {
            (Some(receipt), version) => installed(parse_uv_receipt(receipt), version.cloned()),
            (None, _) => by_marker(&format!("{tool}-upgrade"), bin),
        }
    };

    let installed = match id {
        Id::App(App::Whisper) => from_uv("vinowhisper", "vinowhisper-setup"),
        Id::App(App::Speak) => from_uv("vinospeak", "vinospeak"),
        Id::App(App::Pop) => {
            let release = paths.home.join(".local/share/vinopop/app").exists();
            paths.local_bin("vinopop").exists().then(|| Installed {
                channel: if release {
                    Channel::Release
                } else {
                    Channel::Source
                },
                version: ask_version(&paths.local_bin("vinopop")),
            })
        }
        Id::App(App::Search) => by_marker("vinosearch-upgrade", "vinosearch"),
        Id::App(App::Watch) => by_marker("vinowatch-upgrade", "vinowatch"),
        Id::App(App::OpenEffects) => by_marker("vinoopeneffects-upgrade", "vinoopeneffects-ctl"),
        Id::App(App::AuthFace) => {
            let bin = Path::new("/usr/local/bin/vinoauthface");
            bin.exists().then(|| {
                let version = ask_version(bin);
                // Release builds are stamped with their tag; a checkout's is `dev`.
                let channel = if version.as_deref() == Some("dev") {
                    Channel::Source
                } else {
                    Channel::Release
                };
                Installed { channel, version }
            })
        }
        Id::Ovfetch => {
            let bin = paths.local_bin("ovfetch");
            if bin.exists() {
                installed(Channel::Release, ask_version(&bin))
            } else {
                from_crates("ovfetch")
            }
        }
        Id::Pilot => {
            if paths.local_bin("vinopilot-upgrade").exists() {
                by_marker("vinopilot-upgrade", "vinopilot")
            } else {
                from_crates("vinopilot").or_else(|| by_marker("vinopilot-upgrade", "vinopilot"))
            }
        }
        Id::Ui => None,
    };

    let checkout = checkout(paths, id);
    let installed = if id == Id::Ui {
        checkout.as_ref().map(|c| Installed {
            channel: Channel::Source,
            version: std::fs::read_to_string(c.path.join("Cargo.toml"))
                .ok()
                .and_then(|t| cargo_toml_version(&t)),
        })
    } else {
        installed
    };
    Local {
        installed,
        checkout,
        system_package: system_package(paths, id),
        shadowed: shadowed(paths, id, crate::search_path()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crates_toml_tells_crates_io_from_a_path() {
        let text = r#"[v1]
"ovfetch 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)" = ["ovfetch"]
"vinosearch 7.0.0 (path+file:///home/k/Development/vinoSearch)" = ["vinosearch"]
"#;
        let crates = parse_crates_toml(text);
        assert_eq!(crates["ovfetch"], ("0.6.0".into(), Channel::Package));
        assert_eq!(crates["vinosearch"], ("7.0.0".into(), Channel::Source));
        assert_eq!(crates.len(), 2);
    }

    #[test]
    fn uv_receipts_tell_pypi_from_a_wheel_or_a_checkout() {
        let pypi = r#"requirements = [{ name = "vinospeak", specifier = "==0.5.1" }]"#;
        let wheel = r#"requirements = [{ name = "vinowhisper", path = "/tmp/x/vinowhisper-1.3.1-py3-none-any.whl" }]"#;
        let dir = r#"requirements = [{ name = "vinospeak", directory = "/home/k/vinoSpeak" }]"#;
        assert_eq!(parse_uv_receipt(pypi), Channel::Package);
        assert_eq!(parse_uv_receipt(wheel), Channel::Release);
        assert_eq!(parse_uv_receipt(dir), Channel::Source);
    }

    #[test]
    fn uv_list_reads_names_and_versions() {
        let text = "git-cliff v2.14.2\n- git-cliff\nvinospeak v0.5.1\n- vinospeak\n";
        let tools = parse_uv_list(text);
        assert_eq!(tools["vinospeak"], "0.5.1");
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn package_owners_from_rpm_and_dpkg() {
        let rpm = "vinoopeneffects\nvinoopeneffects-ctl\nvinoopeneffects\nfile /usr/bin/x is not owned by any package\n";
        assert_eq!(
            parse_rpm_owners(rpm),
            vec!["vinoopeneffects", "vinoopeneffects-ctl"]
        );
        let dpkg = "vinowatch: /usr/bin/vinowatch\nvinopop:amd64, vinopop-data: /usr/bin/vinopop\n";
        assert_eq!(
            parse_dpkg_owners(dpkg),
            vec!["vinopop", "vinopop-data", "vinowatch"]
        );
        assert!(
            parse_dpkg_owners("dpkg-query: no path found matching pattern /usr/bin/x\n").is_empty()
        );
    }

    #[test]
    fn version_lines() {
        assert_eq!(version_in("vinosearch 8.0.0\n").as_deref(), Some("8.0.0"));
        assert_eq!(version_in("vinoPop 1.4.0").as_deref(), Some("1.4.0"));
        assert_eq!(version_in("vinoauthface dev").as_deref(), Some("dev"));
        assert_eq!(version_in("ovfetch v0.6.0").as_deref(), Some("0.6.0"));
        assert_eq!(version_in("usage: vinospeak [-h]"), None);
        assert_eq!(version_in(""), None);
    }

    #[test]
    fn cargo_toml_package_version_only() {
        let text =
            "[package]\nname = \"x\"\nversion = \"0.3.0\"\n\n[dependencies]\nversion = \"9\"\n";
        assert_eq!(cargo_toml_version(text).as_deref(), Some("0.3.0"));
        assert_eq!(cargo_toml_version("[workspace]\n"), None);
    }

    #[test]
    fn a_release_ovfetch_wins_over_a_cargo_install() {
        let home = std::env::temp_dir().join(format!("vinopilot-ovfetch-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".local/bin")).unwrap();
        let paths = Paths {
            home: home.clone(),
            source_root: home.join("src"),
            system_bin: home.join("usr/bin"),
        };
        let crates = HashMap::from([(
            "ovfetch".to_string(),
            ("0.6.0".to_string(), Channel::Source),
        )]);
        let uv = HashMap::new();
        let channel = |paths: &Paths| {
            local(paths, Id::Ovfetch, &crates, &uv, None)
                .installed
                .map(|i| i.channel)
        };
        // Only the record: the binary went without cargo uninstall.
        assert_eq!(channel(&paths), None);
        std::fs::create_dir_all(home.join(".cargo/bin")).unwrap();
        std::fs::write(home.join(".cargo/bin/ovfetch"), "").unwrap();
        assert_eq!(channel(&paths), Some(Channel::Source));
        std::fs::write(home.join(".local/bin/ovfetch"), "").unwrap();
        assert_eq!(channel(&paths), Some(Channel::Release));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn a_command_in_both_bins_is_shadowed_by_the_first_on_path() {
        let home = std::env::temp_dir().join(format!("vinopilot-shadow-{}", std::process::id()));
        let (local, cargo) = (home.join(".local/bin"), home.join(".cargo/bin"));
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(&cargo).unwrap();
        let paths = Paths {
            home: home.clone(),
            source_root: home.join("src"),
            system_bin: home.join("usr/bin"),
        };
        let search = App::Search;
        std::fs::write(local.join("vinosearch"), "release").unwrap();
        assert_eq!(shadowed(&paths, Id::App(search), "".as_ref()), vec![]);
        std::fs::write(cargo.join("vinosearch"), "source").unwrap();

        let local_first = std::env::join_paths([&local, &cargo]).unwrap();
        assert_eq!(
            shadowed(&paths, Id::App(search), &local_first),
            vec![Shadowed {
                command: "vinosearch".into(),
                runs: local.join("vinosearch"),
                hidden: cargo.join("vinosearch"),
            }]
        );
        let cargo_first = std::env::join_paths([&cargo, &local]).unwrap();
        assert_eq!(
            shadowed(&paths, Id::App(search), &cargo_first)[0].runs,
            cargo.join("vinosearch")
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn an_empty_home_has_nothing_installed() {
        let home = std::env::temp_dir().join(format!("vinopilot-test-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        let paths = Paths {
            home: home.clone(),
            source_root: home.join("src"),
            system_bin: home.join("usr/bin"),
        };
        let crates = HashMap::new();
        let uv = HashMap::new();
        for id in Id::ALL {
            if id == Id::App(App::AuthFace) {
                continue; // system-wide, outside the fake home
            }
            assert_eq!(
                local(&paths, id, &crates, &uv, None),
                Local::default(),
                "{}",
                id.name()
            );
        }
        std::fs::remove_dir_all(home).unwrap();
    }
}
