use super::scripts::PRELUDE;
use super::*;

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
            // Only the package manager runs as root, not the whole step.
            assert!(steps.iter().all(|s| !s.root), "{channel:?} {action:?}");
        }
        let update = &steps(Id::App(App::Pilot), channel, Action::Update)[0].script;
        assert!(
            update.contains(&format!("fetch {asset}"))
                || update.contains(&format!("ASSET={asset}\nneed flatpak")),
            "{channel:?}"
        );
        assert!(update.contains("sha256sum -c"), "{channel:?}");
        // No other component has them.
        assert!(steps(Id::Ovfetch, channel, Action::Update).is_empty());
    }
}

#[test]
fn vinowhisper_and_vinosearch_also_install_from_their_flatpak() {
    for id in Id::ALL {
        let offered = id.channels().contains(&Channel::Flatpak);
        let has = matches!(id, Id::App(App::Whisper | App::Search));
        assert_eq!(offered, has, "{}", id.name());
        // The default way in stays the first.
        assert_ne!(id.channels()[0], Channel::Flatpak);
        if !has {
            continue;
        }
        let app_id = id.flatpak().unwrap();
        let named = if id == Id::App(App::Search) {
            "dev.vinosearch.VinoSearch"
        } else {
            "io.github.karanshukla.vinowhisper"
        };
        assert_eq!(app_id, named);
        for action in ACTIONS {
            let steps = steps(id, Channel::Flatpak, action);
            assert_eq!(steps.len(), 1, "{} {action:?}", id.name());
            let script = &steps[0].script;
            assert!(script.contains(&format!("APP_ID={app_id}\n")));
            // Never the home's uninstaller: that isn't how it got here.
            assert!(!script.contains("-upgrade"), "{} {action:?}", id.name());
            assert_eq!(
                script.contains("flatpak uninstall"),
                action == Action::Uninstall
            );
        }
        let note = uninstall_note(id, Channel::Flatpak);
        assert!(note.contains(&format!("~/.var/app/{app_id}")), "{note}");
    }
}

/// Runs `id`'s Flatpak install against a `file://` release holding the
/// bundle's checksum in the file `sums_name`, and a `SHA256SUMS` that
/// lists only other files when that isn't it. `flatpak` is stubbed to
/// write its arguments down. What it was run with, or the step's stderr.
fn install_flatpak(id: Id, sums_name: &str, good: bool, system: bool) -> Result<String, String> {
    use std::fs;
    let asset = format!("{}-x86_64.flatpak", id.slug());
    let root = std::env::temp_dir().join(format!(
        "vinopilot-flatpak-step-{}-{sums_name}-{good}-{system}-{}",
        id.slug(),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let (release, stubs, home) = (root.join("release"), root.join("stubs"), root.join("home"));
    for dir in [&release, &stubs, &home] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(release.join(&asset), "a bundle").unwrap();
    let sum = std::process::Command::new("sha256sum")
        .arg(release.join(&asset))
        .output()
        .unwrap();
    let hash = String::from_utf8_lossy(&sum.stdout)[..64].to_string();
    let hash = if good { hash } else { "0".repeat(64) };
    fs::write(
        release.join("SHA256SUMS"),
        format!("{}  install.sh\n", "1".repeat(64)),
    )
    .unwrap();
    fs::write(release.join(sums_name), format!("{hash}  {asset}\n")).unwrap();
    // `info` fails, installed nowhere yet, unless it's `system`-wide.
    let info = if system {
        "[ \"$2\" = --system ]; exit"
    } else {
        "exit 1"
    };
    let flatpak =
        format!("#!/bin/sh\n[ \"$1\" != info ] || {{ {info}; }}\necho \"$@\" >> \"$HOME/ran\"\n");
    let pkexec = "#!/bin/sh\nprintf 'pkexec ' >> \"$HOME/ran\"\nexec \"$@\"\n";
    for (name, script) in [("flatpak", flatpak.as_str()), ("pkexec", pkexec)] {
        fs::write(stubs.join(name), script).unwrap();
        make_executable(&stubs.join(name));
    }
    let step = &steps(id, Channel::Flatpak, Action::Install)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env_clear()
        .env("HOME", &home)
        .env("RELEASE", format!("file://{}", release.display()))
        .env("PATH", format!("{}:/usr/bin:/bin", stubs.display()))
        .output()
        .unwrap();
    let ran = fs::read_to_string(home.join("ran")).unwrap_or_default();
    fs::remove_dir_all(root).unwrap();
    if out.status.success() {
        Ok(ran)
    } else {
        assert_eq!(ran, "", "flatpak ran after a failed check");
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

#[test]
fn a_flatpak_is_checked_then_installed_for_the_user() {
    if !have(&["bash", "curl", "sha256sum"]) {
        return;
    }
    // Either checksum file: vinoPilot's SHA256SUMS lists its bundle;
    // vinoWhisper's is written before the bundle is built, so the
    // bundle's is in its own .sha256 beside it.
    for (id, sums) in [
        (Id::App(App::Search), "SHA256SUMS"),
        (Id::App(App::Search), "vinosearch-x86_64.flatpak.sha256"),
        (Id::App(App::Whisper), "vinowhisper-x86_64.flatpak.sha256"),
    ] {
        let ran = install_flatpak(id, sums, true, false).unwrap();
        let asset = format!("{}-x86_64.flatpak", id.slug());
        assert!(ran.starts_with("install --user -y "), "{ran}");
        assert!(ran.trim_end().ends_with(&asset), "{ran}");
        let why = install_flatpak(id, sums, false, false).unwrap_err();
        assert!(why.contains("doesn't match its checksum"), "{why}");
    }
}

#[test]
fn a_system_wide_flatpak_updates_through_pkexec() {
    if !have(&["bash", "curl", "sha256sum"]) {
        return;
    }
    // As root, as_root runs it as it is: nothing to check.
    let root = std::process::Command::new("id").arg("-u").output().unwrap();
    if String::from_utf8_lossy(&root.stdout).trim() == "0" {
        return;
    }
    let ran = install_flatpak(Id::App(App::Pilot), "SHA256SUMS", true, true).unwrap();
    assert!(ran.starts_with("pkexec install --system -y "), "{ran}");
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
fn uninstall_whisper(name: &str, setup: impl FnOnce(&std::path::Path)) -> std::path::PathBuf {
    use std::fs;
    let root =
        std::env::temp_dir().join(format!("vinopilot-whisper-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let stubs = root.join("stubs");
    fs::create_dir_all(&stubs).unwrap();
    for stub in ["systemctl", "uv"] {
        let path = stubs.join(stub);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        make_executable(&path);
    }
    let data = root.join("data/vinowhisper");
    let cache = root.join("cache/vinowhisper");
    for dir in [
        data.join("models/whisper-small.en"),
        data.join("transcripts"),
        data.join("sessions"),
        cache.join("export-tmp"),
        home.join(".config/vinowhisper"),
        home.join(".local/bin"),
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(data.join("models/whisper-small.en/model.xml"), "x").unwrap();
    fs::write(data.join("transcripts/2026-11-11-120000.txt"), "x").unwrap();
    setup(&home);
    let step = &steps(Id::App(App::Whisper), Channel::Release, Action::Uninstall)[0];
    let out = std::process::Command::new("bash")
        .args(["-c", &step.script])
        .env_clear()
        .env("HOME", &home)
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("PATH", format!("{}:/usr/bin:/bin", stubs.display()))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    root
}

fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

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
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn vinowhispers_own_uninstaller_runs_when_it_has_one() {
    if !have(&["bash"]) {
        return;
    }
    // An upgrade script that knows --uninstall is run with it, and the
    // inline steps aren't.
    let root = uninstall_whisper("own", |home| {
        let upgrade = home.join(".local/bin/vinowhisper-upgrade");
        std::fs::write(
            &upgrade,
            "#!/bin/sh\n# --uninstall: remove it\necho \"$@\" > \"$HOME/ran\"\n",
        )
        .unwrap();
        make_executable(&upgrade);
    });
    let ran = std::fs::read_to_string(root.join("home/ran")).unwrap();
    assert_eq!(ran.trim(), "--uninstall");
    assert!(root.join("data/vinowhisper/models").exists());
    std::fs::remove_dir_all(&root).unwrap();

    // An older one that doesn't is left alone, and the inline steps run.
    let root = uninstall_whisper("old", |home| {
        let upgrade = home.join(".local/bin/vinowhisper-upgrade");
        std::fs::write(&upgrade, "#!/bin/sh\necho \"$@\" > \"$HOME/ran\"\n").unwrap();
        make_executable(&upgrade);
    });
    assert!(!root.join("home/ran").exists());
    assert!(!root.join("home/.local/bin/vinowhisper-upgrade").exists());
    assert!(!root.join("data/vinowhisper/models").exists());
    std::fs::remove_dir_all(root).unwrap();
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

/// The prelude's checkout on a real repo: a checkout on a feature branch
/// is left alone, the default branch is built in the worktree, and
/// checkout_done records it and removes the worktree unless kept.
#[test]
fn a_checkout_off_the_default_branch_installs_from_a_worktree() {
    if !have(&["git"]) {
        return;
    }
    let tmp = std::env::temp_dir().join(format!("vinopilot-worktree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let run_as = |off_branch: OffBranch, script: &str| {
        let out = std::process::Command::new("bash")
            .args(["-c", &format!("{PRELUDE}\n{script}")])
            .current_dir(&tmp)
            .env("CHECKOUT", tmp.join("checkout"))
            .env("WORKTREE", tmp.join("share/worktrees/checkout"))
            .env("OFF_BRANCH", off_branch.name())
            .env("REPO_URL", tmp.join("origin"))
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
    };
    let run = |script: &str| run_as(OffBranch::Worktree, script);
    run(
        r#"git init -q -b main origin && git -C origin commit -q --allow-empty -m one
checkout
git -C "$CHECKOUT" switch -q -c feature && echo work > "$CHECKOUT/wip"
git -C origin commit -q --allow-empty -m two"#,
    );

    let out =
        run(r#"checkout; echo "building in $CHECKOUT"; git -C "$CHECKOUT" log -1 --format=%s"#);
    assert!(out.contains("is on feature, not main"), "{out}");
    assert!(out.contains("share/worktrees/checkout\n"), "{out}");
    assert!(out.ends_with("two\n"), "{out}");
    assert_eq!(
        run(r#"source_dir; git -C "$CHECKOUT" branch --show-current; cat "$CHECKOUT/wip""#),
        format!(
            "{}\nfeature\nwork\n",
            tmp.join("share/worktrees/checkout").display()
        )
    );
    run("checkout_done");
    assert!(!tmp.join("share/worktrees/checkout").exists());
    assert_eq!(
        run(r#"git -C "$CHECKOUT" log -1 --format=%s refs/vinopilot/installed"#),
        "two\n"
    );

    // Switch: the checkout itself goes to main, its work stashed, and
    // the record goes.
    let out = run_as(OffBranch::Switch, "checkout; checkout_done");
    assert!(
        out.contains("Back to your work: git switch feature && git stash pop"),
        "{out}"
    );
    let out = run(
        r#"git -C "$CHECKOUT" branch --show-current; git -C "$CHECKOUT" log -1 --format=%s
git -C "$CHECKOUT" stash list --format=%s; [ -e "$CHECKOUT/wip" ] || echo stashed
git -C "$CHECKOUT" rev-parse -q --verify refs/vinopilot/installed || echo gone"#,
    );
    assert_eq!(
        out,
        "main\ntwo\nOn feature: vinopilot: feature before switching to main\nstashed\ngone\n"
    );
    assert!(!tmp.join("share/worktrees/checkout").exists());
    // On main already, either is a plain pull.
    let out = run_as(OffBranch::Switch, "checkout");
    assert!(out.contains("== git pull in"), "{out}");
    assert!(!out.contains("switching"), "{out}");
    std::fs::remove_dir_all(&tmp).unwrap();
}
