use super::scripts::PRELUDE;
use super::*;
use crate::test_support::Scratch;

const ACTIONS: [Action; 3] = [Action::Install, Action::Update, Action::Uninstall];

/// Whether every one of `tools` runs here: the tests that run a step for
/// real pass by returning early on a machine without them.
fn have(tools: &[&str]) -> bool {
    tools.iter().all(|tool| {
        std::process::Command::new(tool)
            .arg("--version")
            .output()
            .is_ok()
    })
}

const EVERY_CHANNEL: [Channel; 6] = [
    Channel::Release,
    Channel::Source,
    Channel::Package,
    Channel::Deb,
    Channel::Rpm,
    Channel::Flatpak,
];

/// Every step of every (component, channel, action), the prelude once.
fn rendered_steps() -> String {
    let mut out = format!("<PRELUDE>\n{PRELUDE}\n</PRELUDE>\n");
    for id in Id::ALL {
        for channel in EVERY_CHANNEL {
            for action in ACTIONS {
                out.push_str(&format!("### {} {channel:?} {action:?}\n", id.slug()));
                for step in steps(id, channel, action) {
                    let body = step
                        .script
                        .strip_prefix(PRELUDE)
                        .expect("every step starts with the prelude");
                    let who = if step.root { "root" } else { "user" };
                    out.push_str(&format!("--- {who}{body}\n"));
                }
            }
        }
    }
    out
}

/// The scripts are a contract with each app's installer. A change here
/// is deliberate: read the diff, then bless it with
/// `VINOPILOT_BLESS_STEPS=1 cargo test -p vinopilot-core`.
#[test]
fn every_script_matches_the_snapshot() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/catalog/steps.snapshot");
    let rendered = rendered_steps();
    if std::env::var_os("VINOPILOT_BLESS_STEPS").is_some() {
        std::fs::write(path, &rendered).unwrap();
    }
    let pinned = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        rendered == pinned,
        "the steps differ from {path}: rerun with VINOPILOT_BLESS_STEPS=1 and read the diff"
    );
}

#[test]
fn every_offered_channel_can_install_update_and_uninstall() {
    for id in Id::ALL {
        for channel in id.channels() {
            for action in ACTIONS {
                assert!(
                    !steps(id, channel, action).is_empty(),
                    "{} {:?} {:?}",
                    id.name(),
                    channel,
                    action
                );
            }
        }
    }
}

#[test]
fn only_authface_asks_for_root() {
    for id in Id::ALL {
        for channel in id.channels() {
            for action in ACTIONS {
                let root = steps(id, channel, action).iter().any(|s| s.root);
                assert_eq!(
                    root,
                    id.needs_root(),
                    "{} {channel:?} {action:?}",
                    id.name()
                );
            }
        }
    }
}

#[test]
fn vinoauthfaces_checkout_is_pulled_as_the_user_then_deployed_as_root() {
    for action in [Action::Install, Action::Update] {
        let steps = steps(Id::App(App::AuthFace), Channel::Source, action);
        assert!(!steps[0].root, "{action:?}");
        assert!(steps[0].script.ends_with("\ncheckout"), "{action:?}");
        assert!(steps[1].root, "{action:?}");
        assert!(steps[1].script.contains("deploy.sh"), "{action:?}");
    }
}

#[test]
fn uninstalling_from_source_trashes_the_checkout_last() {
    let ends_trashing = |id: Id, channel| {
        steps(id, channel, Action::Uninstall)
            .last()
            .is_some_and(|s| !s.root && s.script.ends_with("trash_checkout"))
    };
    for id in Id::ALL {
        assert!(ends_trashing(id, Channel::Source), "{}", id.name());
        assert!(!ends_trashing(id, Channel::Release), "{}", id.name());
        assert!(!ends_trashing(id, Channel::Package), "{}", id.name());
    }
}

#[test]
fn the_terminal_ui_follows_vinopilots_updates_and_uninstall() {
    let follows = |id, channel, action| {
        steps(id, channel, action)
            .iter()
            .any(|s| s.script.contains("\nfollow_tui "))
    };
    for id in Id::ALL {
        for channel in id.channels() {
            let pilot = id == Id::App(App::Pilot);
            assert!(!follows(id, channel, Action::Install), "{}", id.name());
            assert_eq!(follows(id, channel, Action::Update), pilot, "{}", id.name());
            assert_eq!(
                follows(id, channel, Action::Uninstall),
                pilot,
                "{}",
                id.name()
            );
        }
    }
}

#[test]
fn installing_ovfetch_or_vinopilot_drops_the_other_channels_copy() {
    let script = |id, channel, action| -> String {
        steps(id, channel, action)
            .iter()
            .map(|s| s.script.as_str())
            .collect()
    };
    for (id, bin) in [(Id::Ovfetch, "ovfetch"), (Id::App(App::Pilot), "vinopilot")] {
        for action in [Action::Install, Action::Update] {
            let release = script(id, Channel::Release, action);
            assert!(
                release.contains(&format!("drop_cargo_copies {bin}")),
                "{bin} release {action:?}"
            );
            let package = script(id, Channel::Package, action);
            assert!(
                package.contains(&format!("rm -f \"$BIN/{bin}\"")),
                "{bin} package {action:?}"
            );
        }
    }
    assert!(
        script(Id::Ovfetch, Channel::Source, Action::Install).contains(r#"rm -f "$BIN/ovfetch""#)
    );
    assert!(
        script(Id::App(App::Pilot), Channel::Source, Action::Install)
            .contains("drop_cargo_copies vinopilot")
    );
}

/// Every channel a component can be found installed through.
fn channels(id: Id) -> Vec<Channel> {
    let mut all = id.channels();
    if id == Id::App(App::Pilot) {
        all.extend([Channel::Deb, Channel::Rpm, Channel::Flatpak]);
    }
    all
}

#[test]
fn vinopilots_packages_update_from_the_release_checked_and_ask_for_root_themselves() {
    for (channel, asset) in [
        (Channel::Deb, "vinopilot-x86_64.deb"),
        (Channel::Rpm, "vinopilot-x86_64.rpm"),
        (Channel::Flatpak, "vinopilot-x86_64.flatpak"),
    ] {
        assert!(!Id::App(App::Pilot).channels().contains(&channel));
        for action in ACTIONS {
            let steps = steps(Id::App(App::Pilot), channel, action);
            assert!(!steps.is_empty(), "{channel:?} {action:?}");
            assert!(steps.iter().all(|s| !s.root), "{channel:?} {action:?}");
        }
        let update = &steps(Id::App(App::Pilot), channel, Action::Update)[0].script;
        assert!(
            update.contains(&format!("fetch {asset}"))
                || update.contains(&format!("ASSET={asset}\nneed flatpak")),
            "{channel:?}"
        );
        assert!(update.contains("sha256sum -c"), "{channel:?}");
    }
}

#[test]
fn no_other_component_updates_through_a_deb_or_rpm() {
    for id in Id::ALL.into_iter().filter(|&id| id != Id::App(App::Pilot)) {
        for channel in [Channel::Deb, Channel::Rpm] {
            assert!(
                steps(id, channel, Action::Update).is_empty(),
                "{}",
                id.name()
            );
        }
    }
}

const FLATPAK_APPS: [(Id, &str); 2] = [
    (Id::App(App::Whisper), "io.github.karanshukla.vinowhisper"),
    (Id::App(App::Search), "dev.vinosearch.VinoSearch"),
];

#[test]
fn only_vinowhisper_and_vinosearch_offer_their_flatpak() {
    for id in Id::ALL {
        let offered = id.channels().contains(&Channel::Flatpak);
        let has = FLATPAK_APPS.iter().any(|(app, _)| *app == id);
        assert_eq!(offered, has, "{}", id.name());
    }
    for (id, app_id) in FLATPAK_APPS {
        assert_eq!(id.flatpak(), Some(app_id));
    }
}

#[test]
fn a_flatpak_is_never_the_default_way_in() {
    for id in Id::ALL {
        assert_ne!(id.channels()[0], Channel::Flatpak, "{}", id.name());
    }
}

#[test]
fn a_flatpak_installs_and_uninstalls_its_app_id_in_one_step() {
    for (id, app_id) in FLATPAK_APPS {
        for action in ACTIONS {
            let steps = steps(id, Channel::Flatpak, action);
            assert_eq!(steps.len(), 1, "{} {action:?}", id.name());
            let script = &steps[0].script;
            assert!(script.contains(&format!("APP_ID={app_id}\n")));
            assert_eq!(
                script.contains("flatpak uninstall"),
                action == Action::Uninstall
            );
        }
        let note = uninstall_note(id, Channel::Flatpak);
        assert!(note.contains(&format!("~/.var/app/{app_id}")), "{note}");
    }
}

#[test]
fn a_flatpak_never_runs_the_homes_upgrade_script() {
    for (id, _) in FLATPAK_APPS {
        for action in ACTIONS {
            let script = &steps(id, Channel::Flatpak, action)[0].script;
            assert!(!script.contains("-upgrade"), "{} {action:?}", id.name());
        }
    }
}

/// Uninstalling a Flatpak deletes its models and cache, as the native
/// uninstallers do, and keeps the rest of `~/.var/app`.
#[test]
fn a_flatpak_uninstall_deletes_its_models_and_cache_and_keeps_your_data() {
    let mine = [
        (
            "io.github.karanshukla.vinowhisper",
            "data/vinowhisper/transcripts/a.vwsession",
        ),
        ("dev.vinosearch.VinoSearch", "data/vinosearch/index.db"),
    ];
    for ((id, app_id), (_, kept)) in FLATPAK_APPS.into_iter().zip(mine) {
        let root = Scratch::new(&format!("flatpak-uninstall-{}", id.slug()));
        let home = root.mkdir("home");
        let dir = format!("home/.var/app/{app_id}");
        let models = format!("{dir}/data/{}/models/m.bin", id.slug());
        root.write(&models, "a model");
        root.write(&format!("{dir}/cache/ov-cache/blob"), "compiled");
        root.write(&format!("{dir}/{kept}"), "yours");
        root.write(
            &format!("{dir}/config/{}/config.toml", id.slug()),
            "settings",
        );
        root.script("stubs/flatpak", "[ \"$1\" != info ] || exit 1\n");
        let step = &steps(id, Channel::Flatpak, Action::Uninstall)[0];
        let out = std::process::Command::new("bash")
            .args(["-c", &step.script])
            .env_clear()
            .env("HOME", &home)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("stubs").display()),
            )
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let left = |p: &str| root.join(p).exists();
        assert!(
            !left(&models) && !left(&format!("{dir}/cache")),
            "{}",
            id.name()
        );
        assert!(left(&format!("{dir}/{kept}")), "{}", id.name());
        assert!(
            left(&format!("{dir}/config/{}/config.toml", id.slug())),
            "{}",
            id.name()
        );
    }
}

/// A failed `flatpak uninstall` deletes nothing.
#[test]
fn a_failed_flatpak_uninstall_keeps_the_models() {
    let (id, app_id) = FLATPAK_APPS[0];
    let root = Scratch::new("flatpak-uninstall-fails");
    let home = root.mkdir("home");
    let models = format!("home/.var/app/{app_id}/data/vinowhisper/models/m.bin");
    root.write(&models, "a model");
    root.script("stubs/flatpak", "[ \"$1\" != info ] || exit 1\nexit 1\n");
    let step = &steps(id, Channel::Flatpak, Action::Uninstall)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env_clear()
        .env("HOME", &home)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("stubs").display()),
        )
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(root.join(&models).exists());
}

/// The stubbed `flatpak info`: installed nowhere yet.
const INFO_FINDS_NOTHING: &str = "exit 1";
/// The stubbed `flatpak info`: installed system-wide only.
const INFO_FINDS_SYSTEM: &str = "[ \"$2\" = --system ]; exit";

/// Runs `id`'s Flatpak install against a `file://` release holding the
/// bundle's checksum in the file `sums_name`, and a `SHA256SUMS` that
/// lists only other files when that isn't it. `flatpak` is stubbed to
/// write its arguments down. What it was run with, or the step's stderr.
fn install_flatpak(id: Id, sums_name: &str, good: bool, info: &str) -> Result<String, String> {
    let asset = format!("{}-x86_64.flatpak", id.slug());
    let root = Scratch::new(&format!(
        "flatpak-step-{}-{sums_name}-{good}-{}",
        id.slug(),
        info.len()
    ));
    let bundle = root.write(&format!("release/{asset}"), "a bundle");
    let sum = std::process::Command::new("sha256sum")
        .arg(&bundle)
        .output()
        .unwrap();
    let hash = String::from_utf8_lossy(&sum.stdout)[..64].to_string();
    let hash = if good { hash } else { "0".repeat(64) };
    root.write(
        "release/SHA256SUMS",
        format!("{}  install.sh\n", "1".repeat(64)),
    );
    root.write(
        &format!("release/{sums_name}"),
        format!("{hash}  {asset}\n"),
    );
    root.script(
        "stubs/flatpak",
        &format!("[ \"$1\" != info ] || {{ {info}; }}\necho \"$@\" >> \"$HOME/ran\"\n"),
    );
    root.script(
        "stubs/pkexec",
        "printf 'pkexec ' >> \"$HOME/ran\"\nexec \"$@\"\n",
    );
    let home = root.mkdir("home");
    let step = &steps(id, Channel::Flatpak, Action::Install)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env_clear()
        .env("HOME", &home)
        .env(
            "RELEASE",
            format!("file://{}", root.join("release").display()),
        )
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("stubs").display()),
        )
        .output()
        .unwrap();
    let ran = std::fs::read_to_string(home.join("ran")).unwrap_or_default();
    if out.status.success() {
        Ok(ran)
    } else {
        assert_eq!(ran, "", "flatpak ran after a failed check");
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

/// The bundle is checked against `sums`, then installed for the user; a
/// wrong checksum installs nothing.
fn assert_checked_then_installed(id: Id, sums: &str) {
    let ran = install_flatpak(id, sums, true, INFO_FINDS_NOTHING).unwrap();
    let asset = format!("{}-x86_64.flatpak", id.slug());
    assert!(ran.starts_with("install --user -y "), "{ran}");
    assert!(ran.trim_end().ends_with(&asset), "{ran}");
    let why = install_flatpak(id, sums, false, INFO_FINDS_NOTHING).unwrap_err();
    assert!(why.contains("doesn't match its checksum"), "{why}");
}

#[test]
fn a_flatpak_listed_in_sha256sums_is_checked_then_installed() {
    if have(&["bash", "curl", "sha256sum"]) {
        assert_checked_then_installed(Id::App(App::Search), "SHA256SUMS");
    }
}

#[test]
fn a_flatpak_with_its_own_sha256_file_is_checked_then_installed() {
    if have(&["bash", "curl", "sha256sum"]) {
        let search = Id::App(App::Search);
        assert_checked_then_installed(search, "vinosearch-x86_64.flatpak.sha256");
        let whisper = Id::App(App::Whisper);
        assert_checked_then_installed(whisper, "vinowhisper-x86_64.flatpak.sha256");
    }
}

fn running_as_root() -> bool {
    let id = std::process::Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&id.stdout).trim() == "0"
}

#[test]
fn a_system_wide_flatpak_updates_through_pkexec() {
    let step = &steps(Id::App(App::Pilot), Channel::Flatpak, Action::Install)[0];
    assert!(
        step.script.contains("\n    /usr/bin/pkexec \"$@\"\n"),
        "{}",
        step.script
    );
    if !have(&["bash", "curl", "sha256sum"])
        || !std::path::Path::new("/usr/bin/pkexec").exists()
        || running_as_root()
    {
        let _ = INFO_FINDS_SYSTEM;
        return;
    }
    let pilot = Id::App(App::Pilot);
    let _ = install_flatpak(pilot, "SHA256SUMS", true, INFO_FINDS_SYSTEM);
}

#[test]
fn a_release_without_the_flatpak_says_so() {
    if !have(&["bash", "curl"]) {
        return;
    }
    let step = &steps(Id::App(App::Search), Channel::Flatpak, Action::Install)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env("RELEASE", "file:///nowhere/at/all")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(why.contains("✗ "), "{why}");
}

#[test]
fn a_library_has_no_release_or_package_install() {
    assert_eq!(Id::Ui.channels(), vec![Channel::Source]);
}

#[test]
fn components_parse_by_slug_name_or_bare_name() {
    for id in Id::ALL {
        assert_eq!(Id::parse(&id.slug()), Some(id));
        assert_eq!(Id::parse(id.name()), Some(id));
    }
    assert_eq!(Id::parse("whisper"), Some(Id::App(App::Whisper)));
    assert_eq!(Id::parse("ui"), Some(Id::Ui));
    assert_eq!(Id::parse("nope"), None);
}

/// GitHub ignores the repo's case; the checkout folder, named after it,
/// doesn't, and a `vinoPilot` here would be a second checkout.
#[test]
fn vinopilots_checkout_folder_is_lowercase() {
    assert_eq!(Id::App(App::Pilot).checkout_name(), "vinopilot");
    assert_eq!(Id::App(App::Pilot).repo(), "karanshukla/vinopilot");
}

#[test]
fn checkout_names_match_the_repos() {
    assert_eq!(Id::App(App::Search).checkout_name(), "vinoSearch");
    assert_eq!(Id::Ui.checkout_name(), "vinoUserInterface");
}

#[test]
fn the_uninstall_note_names_the_models_only_for_apps_that_have_them() {
    for id in Id::ALL {
        for channel in id.channels() {
            let note = uninstall_note(id, channel);
            let models = matches!(id, Id::App(app) if app != App::Pilot && app != App::Watch);
            assert_eq!(note.contains("models"), models, "{} {channel:?}", id.name());
            assert_eq!(
                note.contains("trash"),
                channel == Channel::Source,
                "{} {channel:?}",
                id.name()
            );
            assert_eq!(
                note.matches("trash").count(),
                usize::from(channel == Channel::Source)
            );
        }
    }
}

/// Runs vinoWhisper's uninstall step in a home of its own, with
/// `systemctl` and `uv` stubbed out so nothing reaches the real session.
fn uninstall_whisper(name: &str, setup: impl FnOnce(&Scratch)) -> Scratch {
    let root = Scratch::new(&format!("whisper-{name}"));
    for stub in ["systemctl", "uv"] {
        root.script(&format!("stubs/{stub}"), "exit 0\n");
    }
    for dir in [
        "data/vinowhisper/transcripts",
        "data/vinowhisper/sessions",
        "cache/vinowhisper/export-tmp",
        "home/.config/vinowhisper",
        "home/.local/bin",
    ] {
        root.mkdir(dir);
    }
    root.write("data/vinowhisper/models/whisper-small.en/model.xml", "x");
    root.write("data/vinowhisper/transcripts/2026-11-11-120000.txt", "x");
    setup(&root);
    let step = &steps(Id::App(App::Whisper), Channel::Release, Action::Uninstall)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env_clear()
        .env("HOME", root.join("home"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("stubs").display()),
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    root
}

const WHISPER_UPGRADE: &str = "home/.local/bin/vinowhisper-upgrade";

#[test]
fn uninstalling_vinowhisper_deletes_its_models_and_keeps_transcripts_and_settings() {
    if !have(&["bash"]) {
        return;
    }
    let root = uninstall_whisper("inline", |_| {});
    assert!(!root.join("data/vinowhisper/models").exists());
    assert!(!root.join("cache/vinowhisper").exists());
    assert!(
        root.join("data/vinowhisper/transcripts/2026-11-11-120000.txt")
            .exists()
    );
    assert!(root.join("data/vinowhisper/sessions").exists());
    assert!(root.join("home/.config/vinowhisper").exists());
}

#[test]
fn vinowhispers_own_uninstaller_runs_when_it_has_one() {
    if !have(&["bash"]) {
        return;
    }
    let root = uninstall_whisper("own", |root| {
        root.script(
            WHISPER_UPGRADE,
            "# --uninstall: remove it\necho \"$@\" > \"$HOME/ran\"\n",
        );
    });
    let ran = std::fs::read_to_string(root.join("home/ran")).unwrap();
    assert_eq!(ran.trim(), "--uninstall");
    assert!(root.join("data/vinowhisper/models").exists());
}

#[test]
fn an_upgrade_script_without_uninstall_is_removed_not_run() {
    if !have(&["bash"]) {
        return;
    }
    let root = uninstall_whisper("old", |root| {
        root.script(WHISPER_UPGRADE, "echo \"$@\" > \"$HOME/ran\"\n");
    });
    assert!(!root.join("home/ran").exists());
    assert!(!root.join(WHISPER_UPGRADE).exists());
    assert!(!root.join("data/vinowhisper/models").exists());
}

#[test]
fn every_script_parses() {
    let Ok(bash) = std::process::Command::new("bash").arg("--version").output() else {
        return;
    };
    assert!(bash.status.success());
    for id in Id::ALL {
        for channel in channels(id) {
            for action in ACTIONS {
                for step in steps(id, channel, action) {
                    let out = std::process::Command::new("bash")
                        .args(["-n", "-c", &step.script])
                        .output()
                        .unwrap();
                    assert!(
                        out.status.success(),
                        "{} {channel:?} {action:?}: {}",
                        id.name(),
                        String::from_utf8_lossy(&out.stderr)
                    );
                }
            }
        }
    }
}

#[test]
fn installs_that_run_from_the_checkout_switch_it() {
    let switch: Vec<_> = Id::ALL
        .into_iter()
        .filter(|id| id.off_branch() == OffBranch::Switch)
        .collect();
    assert_eq!(switch, [Id::App(App::Whisper), Id::App(App::Pop), Id::Ui]);
}

/// The prelude's checkout on a real repo: `origin`, with a `checkout` of it
/// on `feature` holding uncommitted work, and a commit on main it hasn't
/// pulled.
struct Repos(Scratch);

impl Repos {
    fn new(name: &str) -> Self {
        let repos = Repos(Scratch::new(&format!("worktree-{name}")));
        repos.run(
            r#"git init -q -b main origin && git -C origin commit -q --allow-empty -m one
checkout
git -C "$CHECKOUT" switch -q -c feature && echo work > "$CHECKOUT/wip"
git -C origin commit -q --allow-empty -m two"#,
        );
        repos
    }

    /// Runs `script` after the prelude, with `OffBranch::Worktree`.
    fn run(&self, script: &str) -> String {
        self.run_as(OffBranch::Worktree, script)
    }

    fn run_as(&self, off_branch: OffBranch, script: &str) -> String {
        let out = std::process::Command::new("bash")
            .args(["-c", &format!("{PRELUDE}\n{script}")])
            .current_dir(&*self.0)
            .env("CHECKOUT", self.0.join("checkout"))
            .env("WORKTREE", self.worktree())
            .env("OFF_BRANCH", off_branch.name())
            .env("REPO_URL", self.0.join("origin"))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(
            out.status.success(),
            "{script}\n{stdout}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        stdout
    }

    fn worktree(&self) -> std::path::PathBuf {
        self.0.join("share/worktrees/checkout")
    }
}

#[test]
fn a_checkout_off_the_default_branch_installs_from_a_worktree() {
    if !have(&["git"]) {
        return;
    }
    let repos = Repos::new("off");
    let out = repos
        .run(r#"checkout; echo "building in $CHECKOUT"; git -C "$CHECKOUT" log -1 --format=%s"#);
    assert!(out.contains("is on feature, not main"), "{out}");
    assert!(out.contains("share/worktrees/checkout\n"), "{out}");
    assert!(out.ends_with("two\n"), "{out}");
    assert_eq!(
        repos.run(r#"source_dir; git -C "$CHECKOUT" branch --show-current; cat "$CHECKOUT/wip""#),
        format!("{}\nfeature\nwork\n", repos.worktree().display())
    );
}

#[test]
fn checkout_done_records_the_worktrees_commit_and_removes_it() {
    if !have(&["git"]) {
        return;
    }
    let repos = Repos::new("done");
    repos.run("checkout");
    repos.run("checkout_done");
    assert!(!repos.worktree().exists());
    assert_eq!(
        repos.run(r#"git -C "$CHECKOUT" log -1 --format=%s refs/vinopilot/installed"#),
        "two\n"
    );
}

#[test]
fn switch_moves_the_checkout_to_main_stashing_its_work_and_drops_the_record() {
    if !have(&["git"]) {
        return;
    }
    let repos = Repos::new("switch");
    repos.run("checkout");
    repos.run("checkout_done");
    let out = repos.run_as(OffBranch::Switch, "checkout; checkout_done");
    assert!(
        out.contains("Back to your work: git switch feature && git stash pop"),
        "{out}"
    );
    let out = repos.run(
        r#"git -C "$CHECKOUT" branch --show-current; git -C "$CHECKOUT" log -1 --format=%s
git -C "$CHECKOUT" stash list --format=%s; [ -e "$CHECKOUT/wip" ] || echo stashed
git -C "$CHECKOUT" rev-parse -q --verify refs/vinopilot/installed || echo gone"#,
    );
    assert_eq!(
        out,
        "main\ntwo\nOn feature: vinopilot: feature before switching to main\nstashed\ngone\n"
    );
    assert!(!repos.worktree().exists());
}

#[test]
fn a_checkout_on_main_is_a_plain_pull_either_way() {
    if !have(&["git"]) {
        return;
    }
    let repos = Repos::new("main");
    repos.run(
        r#"git -C "$CHECKOUT" stash -q --include-untracked; git -C "$CHECKOUT" switch -q main"#,
    );
    for off_branch in [OffBranch::Worktree, OffBranch::Switch] {
        let out = repos.run_as(off_branch, "checkout");
        assert!(out.contains("== git pull in"), "{out}");
        assert!(!out.contains("switching"), "{out}");
        assert!(!out.contains("installing main from"), "{out}");
    }
}
