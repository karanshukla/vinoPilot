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

impl Local {
    /// The installed version as a person reads it: `dev` with the checkout's
    /// `git describe` after it when there is one (pinned by
    /// `tests::dev_shows_the_checkouts_describe_and_a_number_stays_alone`).
    pub fn version_label(&self) -> Option<String> {
        let version = self.installed.as_ref()?.version.as_deref()?;
        let describe = self.checkout.as_ref().map(|c| c.describe.as_str());
        Some(match describe {
            Some(describe) if version == "dev" && !describe.is_empty() => {
                format!("dev ({describe})")
            }
            _ => version.to_string(),
        })
    }
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
    /// `/var/lib/flatpak`, the system-wide Flatpak installation.
    pub system_flatpak: PathBuf,
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

    /// Where a source install builds the default branch when the checkout
    /// is on another one. Not under ~/.cache: vinoWhisper and vinoPop run
    /// from it.
    pub fn worktree(&self, id: Id) -> PathBuf {
        self.home
            .join(".local/share/vinopilot/worktrees")
            .join(id.checkout_name())
    }
}

/// The installs `cargo install` keeps in `~/.cargo/.crates.toml`: crate name
/// to where it came from. Its `[v1]` keys read
/// `"ovfetch 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)"`.
pub fn parse_crates_toml(text: &str) -> HashMap<String, (String, Channel)> {
    let Ok(records) = text.parse::<toml::Table>() else {
        return HashMap::new();
    };
    let Some(installs) = records.get("v1").and_then(toml::Value::as_table) else {
        return HashMap::new();
    };
    installs
        .keys()
        .filter_map(|key| crate_install(key))
        .collect()
}

/// One `.crates.toml` key, `name version (source)`; any other shape is
/// skipped, not guessed at (pinned by
/// `tests::a_crates_toml_key_without_a_version_and_source_is_skipped`).
fn crate_install(key: &str) -> Option<(String, (String, Channel))> {
    let mut parts = key.splitn(3, ' ');
    let (name, version, source) = (parts.next()?, parts.next()?, parts.next()?);
    let channel = if source.starts_with("(registry+") || source.starts_with("(sparse+") {
        Channel::Package
    } else {
        Channel::Source
    };
    Some((name.to_string(), (version.to_string(), channel)))
}

/// How a `uv tool` was installed, from its `uv-receipt.toml`: by name from
/// PyPI, a release's wheel, or a local directory.
pub fn parse_uv_receipt(text: &str) -> Channel {
    let Ok(receipt) = text.parse::<toml::Table>() else {
        return Channel::Package;
    };
    let requirements = receipt
        .get("tool")
        .and_then(|tool| tool.get("requirements"))
        .or_else(|| receipt.get("requirements"))
        .and_then(toml::Value::as_array);
    let requirements: Vec<&toml::Table> = requirements
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .collect();
    let from_a_wheel = |requirement: &&toml::Table| {
        requirement
            .values()
            .filter_map(toml::Value::as_str)
            .any(|value| value.contains(".whl"))
    };
    let from_a_folder = |requirement: &&toml::Table| {
        requirement.contains_key("directory") || requirement.contains_key("editable")
    };
    if requirements.iter().any(from_a_wheel) {
        Channel::Release
    } else if requirements.iter().any(from_a_folder) {
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
        let numeric =
            word.starts_with(|c: char| c.is_ascii_digit()) && crate::version::is_plain(word);
        (numeric || word == "dev").then(|| word.to_string())
    })
}

/// The `version = "..."` of a Cargo.toml's `[package]`.
pub fn cargo_toml_version(text: &str) -> Option<String> {
    let manifest = text.parse::<toml::Table>().ok()?;
    let version = manifest.get("package")?.get("version")?.as_str()?;
    Some(version.to_string())
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
        Id::App(App::Pilot) => &["vinopilot", "vinopilot-gui"],
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

/// `inside` the files of `app_id`'s Flatpak, the user's installation before
/// the system's: `<installation>/app/<id>/current/active/files/...`.
fn flatpak_file(paths: &Paths, app_id: &str, inside: &str) -> Option<PathBuf> {
    let inside = Path::new("app")
        .join(app_id)
        .join("current/active/files")
        .join(inside);
    first(&[
        paths.home.join(".local/share/flatpak").join(&inside),
        paths.system_flatpak.join(&inside),
    ])
}

/// The newest `<release version="...">` of a metainfo file, which lists
/// them newest first.
pub fn metainfo_version(text: &str) -> Option<String> {
    let tag = text.split("<release ").nth(1)?.split('>').next()?;
    let value = tag.split("version=").nth(1)?;
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let version = value[1..].split(quote).next()?;
    crate::version::is_plain(version).then(|| version.to_string())
}

/// `id` installed as its release's Flatpak. vinoPilot's holds the real
/// binaries (packaging/flatpak), which say their version; an app's runs only
/// in its sandbox, so its version is its metainfo's newest release.
fn flatpak(paths: &Paths, id: Id) -> Option<Installed> {
    let app_id = id.flatpak()?;
    let version = if id == Id::App(App::Pilot) {
        ask_version(&flatpak_file(paths, app_id, "libexec/vinopilot/vinopilot")?)
    } else {
        flatpak_file(paths, app_id, "")?;
        flatpak_file(
            paths,
            app_id,
            &format!("share/metainfo/{app_id}.metainfo.xml"),
        )
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| metainfo_version(&text))
    };
    Some(Installed {
        channel: Channel::Flatpak,
        version,
    })
}

/// vinoPilot installed as one of its release's packages: the `.deb` or
/// `.rpm` that owns `/usr/bin/vinopilot`, else its Flatpak.
fn packaged(paths: &Paths, system: Option<&SystemPackage>) -> Option<Installed> {
    let Some(system) = system else {
        return flatpak(paths, Id::App(App::Pilot));
    };
    let channel = if system.kind == "rpm" {
        Channel::Rpm
    } else {
        Channel::Deb
    };
    Some(Installed {
        channel,
        version: ask_version(&paths.system_bin.join("vinopilot")),
    })
}

/// The first of `candidates` that exists.
fn first(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// One of vinoPilot's own commands, found or not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Part {
    /// `vinopilot`, `vinopilot-gui` or `vinopilot-tui`.
    pub command: &'static str,
    /// In `~/.local/bin`, `~/.cargo/bin` or (a distro package's) `/usr/bin`,
    /// when it's installed.
    pub path: Option<PathBuf>,
    pub version: Option<String>,
}

/// The command, the window and the terminal UI.
pub const PARTS: [&str; 3] = ["vinopilot", "vinopilot-gui", "vinopilot-tui"];

/// Which of vinoPilot's commands are installed, where, and at what version.
pub fn parts(paths: &Paths) -> Vec<Part> {
    let handles: Vec<_> = PARTS
        .into_iter()
        .map(|command| {
            let path = first(&[
                paths.local_bin(command),
                paths.cargo_bin(command),
                paths.system_bin.join(command),
            ]);
            std::thread::spawn(move || Part {
                command,
                version: path.as_deref().and_then(ask_version),
                path,
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
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

/// A binary that says `dev` was built from a checkout, whatever markers sit
/// beside it: a release or Flatpak build is stamped with its number (pinned by
/// `tests::a_release_marker_beside_a_dev_binary_is_a_source_build`,
/// `tests::a_release_marker_beside_a_numbered_binary_is_a_release`).
fn dev_is_source(mut installed: Installed) -> Installed {
    if installed.version.as_deref() == Some("dev") {
        installed.channel = Channel::Source;
    }
    installed
}

/// Where vinoAuthFace's installer puts its command, system-wide.
const AUTHFACE_BIN: &str = "/usr/local/bin/vinoauthface";

/// What a scan knows before it looks at one component: the home, cargo's
/// install records, and uv's tools.
struct HomeEvidence<'a> {
    paths: &'a Paths,
    /// `~/.cargo/.crates.toml`: crate name to (version, where it came from).
    crates: &'a HashMap<String, (String, Channel)>,
    /// `uv tool list`: tool name to version.
    uv_list: &'a HashMap<String, String>,
    /// The component's own `uv-receipt.toml`, when it's a uv tool.
    uv_receipt: Option<&'a str>,
}

impl HomeEvidence<'_> {
    /// `id` as installed in the home (or, for vinoAuthFace, under
    /// /usr/local), from what its installer leaves behind.
    fn install(&self, id: Id) -> Option<Installed> {
        self.by_evidence(id).map(dev_is_source)
    }

    fn by_evidence(&self, id: Id) -> Option<Installed> {
        match id {
            Id::App(App::Whisper) => self.uv_tool("vinowhisper", "vinowhisper-setup"),
            Id::App(App::Speak) => self.uv_tool("vinospeak", "vinospeak"),
            Id::App(App::Pop) => self.pop(),
            Id::App(App::Search) => self.marked("vinosearch-upgrade", "vinosearch"),
            Id::App(App::Watch) => self.marked("vinowatch-upgrade", "vinowatch"),
            Id::App(App::OpenEffects) => {
                self.marked("vinoopeneffects-upgrade", "vinoopeneffects-ctl")
            }
            Id::App(App::AuthFace) => authface(Path::new(AUTHFACE_BIN)),
            Id::Ovfetch => self.ovfetch(),
            Id::App(App::Pilot) => self.pilot(),
            Id::Ui => None,
        }
    }

    /// `command` in `~/.local/bin` or `~/.cargo/bin`: a release when the
    /// release installer's `marker` (its `<app>-upgrade` copy) is beside it,
    /// else a source build (pinned by
    /// `tests::a_command_beside_its_upgrade_script_is_a_release`,
    /// `tests::a_command_without_its_upgrade_script_is_a_source_build`).
    fn marked(&self, marker: &str, command: &str) -> Option<Installed> {
        let found = first(&[self.paths.local_bin(command), self.paths.cargo_bin(command)])?;
        let channel = if self.paths.local_bin(marker).exists() {
            Channel::Release
        } else {
            Channel::Source
        };
        Some(Installed {
            channel,
            version: ask_version(&found),
        })
    }

    /// A `cargo install` of `name`, by cargo's record. The record outlives a
    /// binary deleted by hand, so the binary has to be there too (pinned by
    /// `tests::a_cargo_record_whose_binary_is_gone_is_not_an_install`).
    fn cargo_installed(&self, name: &str) -> Option<Installed> {
        let (version, channel) = self.crates.get(name)?;
        self.paths.cargo_bin(name).exists().then(|| Installed {
            channel: *channel,
            version: Some(version.clone()),
        })
    }

    /// A uv tool by its receipt, at the version `uv tool list` gives it.
    /// Without a receipt it isn't uv's: the marker says (pinned by
    /// `tests::a_uv_receipt_decides_over_the_upgrade_script`,
    /// `tests::without_a_uv_receipt_the_upgrade_script_and_the_binary_decide`).
    /// uv's list has the package's version; a source build's command says
    /// `dev`, so that is asked instead (pinned by
    /// `tests::a_uv_tool_from_a_checkout_is_dev_whatever_its_package_says`).
    fn uv_tool(&self, tool: &str, command: &str) -> Option<Installed> {
        match self.uv_receipt {
            Some(receipt) => {
                let channel = parse_uv_receipt(receipt);
                let listed = || self.uv_list.get(tool).cloned();
                let version = match channel {
                    Channel::Source => ask_version(&self.paths.local_bin(command)).or_else(listed),
                    _ => listed(),
                };
                Some(Installed { channel, version })
            }
            None => self.marked(&format!("{tool}-upgrade"), command),
        }
    }

    /// vinoPop's release unpacks into `~/.local/share/vinopop/app`; a source
    /// install runs from its checkout.
    fn pop(&self) -> Option<Installed> {
        let command = self.paths.local_bin("vinopop");
        let unpacked_release = self.paths.home.join(".local/share/vinopop/app").exists();
        command.exists().then(|| Installed {
            channel: if unpacked_release {
                Channel::Release
            } else {
                Channel::Source
            },
            version: ask_version(&command),
        })
    }

    /// The release's bare binary in `~/.local/bin` runs before a cargo
    /// install's (pinned by `tests::a_release_ovfetch_wins_over_a_cargo_install`).
    fn ovfetch(&self) -> Option<Installed> {
        let release = self.paths.local_bin("ovfetch");
        if release.exists() {
            Some(Installed {
                channel: Channel::Release,
                version: ask_version(&release),
            })
        } else {
            self.cargo_installed("ovfetch")
        }
    }

    /// The release's marker first, then cargo's record, then a binary with
    /// neither, a source build (pinned by
    /// `tests::vinopilot_is_its_release_then_cargos_record`,
    /// `tests::a_vinopilot_with_neither_marker_nor_record_is_a_source_build`).
    fn pilot(&self) -> Option<Installed> {
        let by_marker = || self.marked("vinopilot-upgrade", "vinopilot");
        if self.paths.local_bin("vinopilot-upgrade").exists() {
            by_marker()
        } else {
            self.cargo_installed("vinopilot").or_else(by_marker)
        }
    }
}

fn authface(command: &Path) -> Option<Installed> {
    command.exists().then(|| {
        dev_is_source(Installed {
            channel: Channel::Release,
            version: ask_version(command),
        })
    })
}

/// A library is "installed" when its checkout is there, at its Cargo.toml's
/// version.
fn library_checkout(checkout: &Checkout) -> Installed {
    Installed {
        channel: Channel::Source,
        version: std::fs::read_to_string(checkout.path.join("Cargo.toml"))
            .ok()
            .and_then(|text| cargo_toml_version(&text)),
    }
}

/// `id` installed outside the home: vinoPilot as its package, an app as its
/// Flatpak. A copy in the home runs first, so it is only asked when there is
/// none (pinned by `tests::a_copy_in_the_home_wins_over_vinopilots_flatpak`,
/// `tests::a_copy_in_the_home_wins_over_an_apps_flatpak`).
fn outside_home(paths: &Paths, id: Id, system: Option<&SystemPackage>) -> Option<Installed> {
    if id == Id::App(App::Pilot) {
        packaged(paths, system)
    } else {
        flatpak(paths, id)
    }
}

fn local(
    paths: &Paths,
    id: Id,
    crates: &HashMap<String, (String, Channel)>,
    uv_list: &HashMap<String, String>,
    uv_receipt: Option<String>,
) -> Local {
    let checkout = checkout(paths, id);
    let system_package = system_package(paths, id);
    let in_home = if id == Id::Ui {
        checkout.as_ref().map(library_checkout)
    } else {
        HomeEvidence {
            paths,
            crates,
            uv_list,
            uv_receipt: uv_receipt.as_deref(),
        }
        .install(id)
    };
    let installed = in_home.or_else(|| outside_home(paths, id, system_package.as_ref()));
    Local {
        installed,
        checkout,
        system_package,
        shadowed: shadowed(paths, id, crate::search_path()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Scratch, put_command};

    /// Everything, the system's directories included, inside `home`.
    fn paths_under(home: &Path) -> Paths {
        Paths {
            home: home.to_path_buf(),
            source_root: home.join("src"),
            system_bin: home.join("usr/bin"),
            system_flatpak: home.join("var/lib/flatpak"),
        }
    }

    /// A throwaway home with `commands` in `~/.local/bin`, each answering
    /// `--version` with its own name and `version`.
    fn home_with(name: &str, version: &str, commands: &[&str]) -> Scratch {
        let home = Scratch::new(name);
        for command in commands {
            put_command(&home.join(".local/bin").join(command), version);
        }
        home
    }

    /// What a scan of `home` finds for `id`, with no cargo or uv records.
    fn installed_in(home: &Path, id: Id) -> Option<Installed> {
        installed_with(home, id, &HashMap::new(), None)
    }

    fn installed_with(
        home: &Path,
        id: Id,
        crates: &HashMap<String, (String, Channel)>,
        uv: Option<(&HashMap<String, String>, &str)>,
    ) -> Option<Installed> {
        let (uv_list, receipt) = match uv {
            Some((list, receipt)) => (list.clone(), Some(receipt.to_string())),
            None => (HashMap::new(), None),
        };
        local(&paths_under(home), id, crates, &uv_list, receipt).installed
    }

    fn at(channel: Channel, version: &str) -> Option<Installed> {
        Some(Installed {
            channel,
            version: Some(version.into()),
        })
    }

    #[test]
    fn a_command_beside_its_upgrade_script_is_a_release() {
        let home = home_with("marker", "8.0.0", &["vinosearch", "vinosearch-upgrade"]);
        let search = Id::App(App::Search);
        assert_eq!(installed_in(&home, search), at(Channel::Release, "8.0.0"));
    }

    #[test]
    fn a_command_without_its_upgrade_script_is_a_source_build() {
        let home = home_with("nomarker", "8.0.0", &["vinosearch"]);
        let search = Id::App(App::Search);
        assert_eq!(installed_in(&home, search), at(Channel::Source, "8.0.0"));
    }

    #[test]
    fn a_command_only_in_cargos_bin_is_a_source_build() {
        let home = home_with("cargobin", "", &[]);
        put_command(&home.join(".cargo/bin/vinowatch"), "0.9.0");
        let watch = Id::App(App::Watch);
        assert_eq!(installed_in(&home, watch), at(Channel::Source, "0.9.0"));
    }

    #[test]
    fn an_upgrade_script_alone_is_not_an_install() {
        let home = home_with("onlymarker", "8.0.0", &["vinosearch-upgrade"]);
        assert_eq!(installed_in(&home, Id::App(App::Search)), None);
    }

    #[test]
    fn vinoopeneffects_is_told_by_its_ctl_command() {
        let home = home_with(
            "effects",
            "3",
            &["vinoopeneffects-ctl", "vinoopeneffects-upgrade"],
        );
        let effects = Id::App(App::OpenEffects);
        assert_eq!(installed_in(&home, effects), at(Channel::Release, "3"));
    }

    #[test]
    fn vinopop_is_a_release_when_its_app_folder_is_unpacked() {
        let home = home_with("pop", "1.4.0", &["vinopop"]);
        let pop = Id::App(App::Pop);
        assert_eq!(installed_in(&home, pop), at(Channel::Source, "1.4.0"));
        home.mkdir(".local/share/vinopop/app");
        assert_eq!(installed_in(&home, pop), at(Channel::Release, "1.4.0"));
    }

    #[test]
    fn a_uv_receipt_decides_over_the_upgrade_script() {
        let home = home_with("uv", "9.9.9", &["vinospeak", "vinospeak-upgrade"]);
        let uv_list = HashMap::from([("vinospeak".to_string(), "0.5.1".to_string())]);
        let pypi = r#"requirements = [{ name = "vinospeak", specifier = "==0.5.1" }]"#;
        let found = installed_with(
            &home,
            Id::App(App::Speak),
            &HashMap::new(),
            Some((&uv_list, pypi)),
        );
        assert_eq!(found, at(Channel::Package, "0.5.1"));
    }

    #[test]
    fn a_uv_tool_from_a_checkout_is_dev_whatever_its_package_says() {
        let home = home_with("uvdev", "dev", &["vinospeak"]);
        let uv_list = HashMap::from([("vinospeak".to_string(), "0.9.0".to_string())]);
        let dir = r#"requirements = [{ name = "vinospeak", directory = "/home/k/vinoSpeak" }]"#;
        let found = installed_with(
            &home,
            Id::App(App::Speak),
            &HashMap::new(),
            Some((&uv_list, dir)),
        );
        assert_eq!(found, at(Channel::Source, "dev"));
    }

    #[test]
    fn without_a_uv_receipt_the_upgrade_script_and_the_binary_decide() {
        let home = home_with("nouv", "9.9.9", &["vinospeak", "vinospeak-upgrade"]);
        let found = installed_in(&home, Id::App(App::Speak));
        assert_eq!(found, at(Channel::Release, "9.9.9"));
    }

    fn vinopilot_record() -> HashMap<String, (String, Channel)> {
        HashMap::from([(
            "vinopilot".to_string(),
            ("0.4.0".to_string(), Channel::Package),
        )])
    }

    #[test]
    fn vinopilot_is_its_release_then_cargos_record() {
        let home = home_with("pilot", "0.5.0", &[]);
        let pilot = Id::App(App::Pilot);
        let found = || installed_with(&home, pilot, &vinopilot_record(), None);
        put_command(&home.join(".cargo/bin/vinopilot"), "0.4.0");
        assert_eq!(found(), at(Channel::Package, "0.4.0"));
        put_command(&home.join(".local/bin/vinopilot"), "0.5.0");
        assert_eq!(found(), at(Channel::Package, "0.4.0"));
        put_command(&home.join(".local/bin/vinopilot-upgrade"), "0.5.0");
        assert_eq!(found(), at(Channel::Release, "0.5.0"));
    }

    #[test]
    fn a_vinopilot_with_neither_marker_nor_record_is_a_source_build() {
        let home = home_with("pilotsrc", "0.5.0", &["vinopilot"]);
        let found = installed_in(&home, Id::App(App::Pilot));
        assert_eq!(found, at(Channel::Source, "0.5.0"));
    }

    #[test]
    fn vinoauthface_is_found_by_its_command_alone() {
        let home = home_with("authface", "dev", &["vinoauthface"]);
        let command = home.join(".local/bin/vinoauthface");
        assert_eq!(authface(&command), at(Channel::Source, "dev"));
        assert_eq!(authface(&home.join("nowhere")), None);
    }
    #[test]
    fn a_vinoauthface_with_a_version_or_none_is_a_release() {
        let home = home_with("authface-12", "12", &["vinoauthface"]);
        let command = home.join(".local/bin/vinoauthface");
        assert_eq!(authface(&command), at(Channel::Release, "12"));
        let silent = home.join(".local/bin/silent");
        put_command(&silent, "");
        assert_eq!(authface(&silent).map(|i| i.channel), Some(Channel::Release));
    }

    /// Every app with a marker, and the command that carries its version.
    const MARKED: [(App, &str, &str); 3] = [
        (App::Search, "vinosearch", "vinosearch-upgrade"),
        (App::Watch, "vinowatch", "vinowatch-upgrade"),
        (
            App::OpenEffects,
            "vinoopeneffects-ctl",
            "vinoopeneffects-upgrade",
        ),
    ];

    #[test]
    fn a_release_marker_beside_a_dev_binary_is_a_source_build() {
        for (app, command, marker) in MARKED {
            let home = home_with("marker-dev", "dev", &[command, marker]);
            assert_eq!(
                installed_in(&home, Id::App(app)),
                at(Channel::Source, "dev")
            );
        }
        let pop = home_with("pop-dev", "dev", &["vinopop"]);
        pop.mkdir(".local/share/vinopop/app");
        assert_eq!(
            installed_in(&pop, Id::App(App::Pop)),
            at(Channel::Source, "dev")
        );
        let ovfetch = home_with("ovfetch-dev", "dev", &["ovfetch"]);
        assert_eq!(
            installed_in(&ovfetch, Id::Ovfetch),
            at(Channel::Source, "dev")
        );
        let pilot = home_with("pilot-dev", "dev", &["vinopilot", "vinopilot-upgrade"]);
        assert_eq!(
            installed_in(&pilot, Id::App(App::Pilot)),
            at(Channel::Source, "dev")
        );
    }

    #[test]
    fn a_release_marker_beside_a_numbered_binary_is_a_release() {
        for (app, command, marker) in MARKED {
            let home = home_with("marker-num", "8.0.0", &[command, marker]);
            assert_eq!(
                installed_in(&home, Id::App(app)),
                at(Channel::Release, "8.0.0")
            );
        }
        let pop = home_with("pop-num", "1.4.0", &["vinopop"]);
        pop.mkdir(".local/share/vinopop/app");
        assert_eq!(
            installed_in(&pop, Id::App(App::Pop)),
            at(Channel::Release, "1.4.0")
        );
    }

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

    const OVFETCH_RECORD: &str = "\"ovfetch 0.6.0 (registry+https://github.com/rust-lang/crates.io-index)\" = [\"ovfetch\"]\n";

    #[test]
    fn a_cargo_install_recorded_under_v1_counts() {
        let crates = parse_crates_toml(&format!("[v1]\n{OVFETCH_RECORD}"));
        assert_eq!(crates["ovfetch"], ("0.6.0".into(), Channel::Package));
    }

    #[test]
    fn the_same_record_outside_v1_or_in_a_file_that_does_not_parse_does_not() {
        assert!(parse_crates_toml(&format!("[v2]\n{OVFETCH_RECORD}")).is_empty());
        assert!(parse_crates_toml(OVFETCH_RECORD).is_empty());
        assert!(parse_crates_toml(&format!("[v1]\n{OVFETCH_RECORD}not toml [[[\n")).is_empty());
    }

    #[test]
    fn a_crates_toml_with_no_installs_or_that_is_not_toml_has_none() {
        assert!(parse_crates_toml("").is_empty());
        assert!(parse_crates_toml("[v1]\n").is_empty());
        assert!(parse_crates_toml("[v2]\n\"ovfetch 0.6.0 (registry+x)\" = []\n").is_empty());
        assert!(parse_crates_toml("[v1\n\"ovfetch 0.6.0 (registry+x)\" = [\n").is_empty());
    }

    #[test]
    fn a_crates_toml_key_without_a_version_and_source_is_skipped() {
        assert!(parse_crates_toml("[v1]\n\"ovfetch\" = []\n").is_empty());
    }

    #[test]
    fn a_sparse_registry_is_crates_io_too() {
        let text = "[v1]\n\"ovfetch 0.6.0 (sparse+https://index.crates.io/)\" = [\"ovfetch\"]\n";
        assert_eq!(
            parse_crates_toml(text)["ovfetch"],
            ("0.6.0".into(), Channel::Package)
        );
    }

    #[test]
    fn a_uv_receipt_is_read_under_its_tool_table_as_uv_writes_it() {
        let receipt = |requirement: &str| {
            format!("[tool]\nrequirements = [{requirement}]\nentrypoints = []\n")
        };
        let wheel = receipt(
            r#"{ name = "vinowhisper", path = "/tmp/x/vinowhisper-1.3.1-py3-none-any.whl" }"#,
        );
        let editable = receipt(r#"{ name = "vinospeak", editable = "/home/k/vinoSpeak" }"#);
        let pypi = receipt(r#"{ name = "vinospeak" }"#);
        assert_eq!(parse_uv_receipt(&wheel), Channel::Release);
        assert_eq!(parse_uv_receipt(&editable), Channel::Source);
        assert_eq!(parse_uv_receipt(&pypi), Channel::Package);
    }

    #[test]
    fn a_uv_receipt_without_requirements_or_that_is_not_toml_reads_as_pypi() {
        assert_eq!(parse_uv_receipt(""), Channel::Package);
        assert_eq!(
            parse_uv_receipt("[tool]\nentrypoints = []\n"),
            Channel::Package
        );
        assert_eq!(
            parse_uv_receipt("requirements = [{ name ="),
            Channel::Package
        );
    }

    #[test]
    fn a_cargo_toml_without_its_own_version_or_that_is_not_toml_has_none() {
        let inherited = "[package]\nname = \"x\"\nversion.workspace = true\n";
        assert_eq!(cargo_toml_version(inherited), None);
        let workspace = "[workspace.package]\nversion = \"0.5.0\"\n";
        assert_eq!(cargo_toml_version(workspace), None);
        assert_eq!(cargo_toml_version("[package\nversion = \"1\""), None);
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
    fn dev_shows_the_checkouts_describe_and_a_number_stays_alone() {
        let local = |version: &str, describe: Option<&str>| Local {
            installed: at(Channel::Source, version),
            checkout: describe.map(|d| Checkout {
                path: PathBuf::from("/src/vinoWatch"),
                describe: d.into(),
            }),
            ..Default::default()
        };
        assert_eq!(
            local("dev", Some("v8-9-g1a2b3c4"))
                .version_label()
                .as_deref(),
            Some("dev (v8-9-g1a2b3c4)")
        );
        assert_eq!(local("dev", None).version_label().as_deref(), Some("dev"));
        assert_eq!(
            local("dev", Some("")).version_label().as_deref(),
            Some("dev")
        );
        assert_eq!(
            local("8.0.0", Some("v8-9-g1a2b3c4"))
                .version_label()
                .as_deref(),
            Some("8.0.0")
        );
        assert_eq!(Local::default().version_label(), None);
    }

    #[test]
    fn a_capital_p_vinopop_line_parses_dev_and_numbers() {
        assert_eq!(version_in("vinoPop dev").as_deref(), Some("dev"));
        assert_eq!(version_in("vinoPop 1.5.0").as_deref(), Some("1.5.0"));
    }

    #[test]
    fn cargo_toml_package_version_only() {
        let text =
            "[package]\nname = \"x\"\nversion = \"0.3.0\"\n\n[dependencies]\nversion = \"9\"\n";
        assert_eq!(cargo_toml_version(text).as_deref(), Some("0.3.0"));
        assert_eq!(cargo_toml_version("[workspace]\n"), None);
    }

    fn ovfetch_record() -> HashMap<String, (String, Channel)> {
        HashMap::from([(
            "ovfetch".to_string(),
            ("0.6.0".to_string(), Channel::Source),
        )])
    }

    fn ovfetch_channel(home: &Path) -> Option<Channel> {
        installed_with(home, Id::Ovfetch, &ovfetch_record(), None).map(|i| i.channel)
    }

    #[test]
    fn a_cargo_record_whose_binary_is_gone_is_not_an_install() {
        let home = Scratch::new("ovfetch-gone");
        assert_eq!(ovfetch_channel(&home), None);
    }

    #[test]
    fn a_release_ovfetch_wins_over_a_cargo_install() {
        let home = Scratch::new("ovfetch");
        home.write(".cargo/bin/ovfetch", "");
        assert_eq!(ovfetch_channel(&home), Some(Channel::Source));
        home.write(".local/bin/ovfetch", "");
        assert_eq!(ovfetch_channel(&home), Some(Channel::Release));
    }

    #[test]
    fn a_command_in_both_bins_is_shadowed_by_the_first_on_path() {
        let home = Scratch::new("shadow");
        let paths = paths_under(&home);
        let search = Id::App(App::Search);
        let local = home.write(".local/bin/vinosearch", "release");
        assert_eq!(shadowed(&paths, search, "".as_ref()), vec![]);
        let cargo = home.write(".cargo/bin/vinosearch", "source");

        let (local_bin, cargo_bin) = (local.parent().unwrap(), cargo.parent().unwrap());
        let local_first = std::env::join_paths([local_bin, cargo_bin]).unwrap();
        assert_eq!(
            shadowed(&paths, search, &local_first),
            vec![Shadowed {
                command: "vinosearch".into(),
                runs: local.clone(),
                hidden: cargo.clone(),
            }]
        );
        let cargo_first = std::env::join_paths([cargo_bin, local_bin]).unwrap();
        assert_eq!(shadowed(&paths, search, &cargo_first)[0].runs, cargo);
    }

    #[test]
    fn parts_are_found_with_their_versions() {
        let home = Scratch::new("parts");
        let tui = home.script(".local/bin/vinopilot-tui", "echo 'vinopilot-tui 0.2.0'\n");
        let found = parts(&paths_under(&home));
        assert_eq!(found.iter().map(|p| p.command).collect::<Vec<_>>(), PARTS);
        assert_eq!(found[0].path, None);
        assert_eq!(found[2].path.as_ref(), Some(&tui));
        assert_eq!(found[2].version.as_deref(), Some("0.2.0"));
    }

    /// vinoPilot's Flatpak at `version` in `installation`.
    fn put_pilot_flatpak(installation: &Path, version: &str) {
        put_command(
            &installation.join(
                "app/io.github.karanshukla.vinopilot/current/active/files/libexec/vinopilot/vinopilot",
            ),
            version,
        );
    }

    fn flatpak_at(version: Option<&str>) -> Option<Installed> {
        Some(Installed {
            channel: Channel::Flatpak,
            version: version.map(Into::into),
        })
    }

    #[test]
    fn vinopilot_is_found_in_its_flatpak_user_before_system() {
        let home = Scratch::new("flatpak");
        let pilot = Id::App(App::Pilot);
        put_pilot_flatpak(&paths_under(&home).system_flatpak, "0.2.0");
        assert_eq!(installed_in(&home, pilot), flatpak_at(Some("0.2.0")));
        put_pilot_flatpak(&home.join(".local/share/flatpak"), "0.3.0");
        assert_eq!(installed_in(&home, pilot), flatpak_at(Some("0.3.0")));
    }

    #[test]
    fn a_copy_in_the_home_wins_over_vinopilots_flatpak() {
        let home = Scratch::new("flatpak-home");
        put_pilot_flatpak(&home.join(".local/share/flatpak"), "0.3.0");
        home.write(".local/bin/vinopilot", "");
        let found = installed_in(&home, Id::App(App::Pilot)).map(|i| i.channel);
        assert_eq!(found, Some(Channel::Source));
    }

    const SEARCH_FLATPAK: &str =
        "var/lib/flatpak/app/dev.vinosearch.VinoSearch/current/active/files";

    #[test]
    fn a_flatpak_without_metainfo_is_installed_at_an_unknown_version() {
        let home = Scratch::new("appflatpak-bare");
        home.mkdir(&format!("{SEARCH_FLATPAK}/share/metainfo"));
        assert_eq!(installed_in(&home, Id::App(App::Search)), flatpak_at(None));
    }

    #[test]
    fn an_app_is_found_in_its_flatpak_with_its_metainfos_version() {
        let home = Scratch::new("appflatpak");
        home.write(
            &format!("{SEARCH_FLATPAK}/share/metainfo/dev.vinosearch.VinoSearch.metainfo.xml"),
            "<component>\n<releases>\n<release version=\"9.1.0\" date=\"2026-11-20\"/>\n\
             <release version=\"9.0.0\" date=\"2026-11-01\"/>\n</releases>\n</component>\n",
        );
        let found = installed_in(&home, Id::App(App::Search));
        assert_eq!(found, flatpak_at(Some("9.1.0")));
    }

    #[test]
    fn a_copy_in_the_home_wins_over_an_apps_flatpak() {
        let home = Scratch::new("appflatpak-home");
        home.mkdir(&format!("{SEARCH_FLATPAK}/share/metainfo"));
        home.write(".local/bin/vinosearch", "");
        let found = installed_in(&home, Id::App(App::Search)).map(|i| i.channel);
        assert_eq!(found, Some(Channel::Source));
    }

    #[test]
    fn metainfo_versions() {
        assert_eq!(
            metainfo_version("<release date='2026-01-01' version='1.2.3'>").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(metainfo_version("<releases></releases>"), None);
        assert_eq!(metainfo_version("<release version=\"<b>\"/>"), None);
    }

    #[test]
    fn an_empty_home_has_nothing_installed() {
        let home = Scratch::new("empty");
        let paths = paths_under(&home);
        let in_the_home = Id::ALL
            .into_iter()
            .filter(|&id| id != Id::App(App::AuthFace));
        for id in in_the_home {
            let found = local(&paths, id, &HashMap::new(), &HashMap::new(), None);
            assert_eq!(found, Local::default(), "{}", id.name());
        }
    }
}
