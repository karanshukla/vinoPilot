//! What vinopilot manages and the shell each action runs. Each app's own
//! installer does the work (and the hash checks); this only knows which one
//! to call, with what, for each way of installing it.
//!
//! The components and their ways in are here, the shell in [`scripts`], and
//! which of it each (component, channel, action) runs in [`steps`].

use vino_ui::App;

mod scripts;
mod steps;
#[cfg(test)]
mod tests;

use scripts::PRELUDE;
pub use steps::steps;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    App(App),
    Ovfetch,
    Ui,
}

impl Id {
    pub const ALL: [Id; 10] = [
        Id::App(App::Pilot),
        Id::App(App::Whisper),
        Id::App(App::AuthFace),
        Id::App(App::OpenEffects),
        Id::App(App::Pop),
        Id::App(App::Speak),
        Id::App(App::Search),
        Id::App(App::Watch),
        Id::Ovfetch,
        Id::Ui,
    ];

    /// The name in JSON and on the command line: `vinowhisper`, `ovfetch`.
    pub fn slug(self) -> String {
        self.name().to_lowercase()
    }

    /// A component by its slug or name, with or without the `vino`:
    /// `vinoWhisper`, `vinowhisper` and `whisper` are all vinoWhisper.
    pub fn parse(text: &str) -> Option<Id> {
        let want = text.to_lowercase();
        let bare = |id: Id| id.slug().strip_prefix("vino").map(str::to_string);
        Id::ALL.into_iter().find(|id| {
            id.slug() == want
                || bare(*id).as_deref() == Some(want.as_str())
                || (want == "ui" && *id == Id::Ui)
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Id::App(app) => app.name(),
            Id::Ovfetch => "ovfetch",
            Id::Ui => "vinoUserInterface",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Id::App(App::Pilot) => "Installs, updates, and uninstalls the vinoPilot apps",
            Id::App(app) => app.blurb(),
            Id::Ovfetch => "Downloads the OpenVINO runtime for Intel NPUs",
            Id::Ui => "Shared egui component library used by the vinoPilot apps",
        }
    }

    /// `owner/name` on GitHub. vinoPilot's is lowercase, like its release
    /// URLs and its checkout folder (pinned by
    /// `tests::vinopilots_checkout_folder_is_lowercase`).
    pub fn repo(self) -> &'static str {
        match self {
            Id::App(App::Pilot) => "karanshukla/vinopilot",
            Id::App(app) => app.repo(),
            Id::Ovfetch => "karanshukla/ovfetch",
            Id::Ui => "karanshukla/vinoUserInterface",
        }
    }

    /// The checkout's folder name under the source folder.
    pub fn checkout_name(self) -> &'static str {
        self.repo()
            .rsplit_once('/')
            .map_or(self.repo(), |(_, name)| name)
    }

    pub fn package(self) -> Option<Package> {
        match self {
            Id::App(App::Whisper) => Some(Package::PyPi("vinowhisper")),
            Id::App(App::Speak) => Some(Package::PyPi("vinospeak")),
            Id::Ovfetch => Some(Package::Crate("ovfetch")),
            Id::App(App::Pilot) => Some(Package::Crate("vinopilot")),
            Id::Ui => Some(Package::Library("vino-user-interface")),
            Id::App(_) => None,
        }
    }

    /// The app id of its Flatpak, for the components whose release has one
    /// (`<slug>-x86_64.flatpak`, as vinoPilot's own).
    pub fn flatpak(self) -> Option<&'static str> {
        match self {
            Id::App(App::Pilot) => Some(PILOT_APP_ID),
            Id::App(App::Whisper) => Some("io.github.karanshukla.vinowhisper"),
            Id::App(App::Search) => Some("dev.vinosearch.VinoSearch"),
            _ => None,
        }
    }

    /// Installs under /usr/local and /etc, or as a package, so every step
    /// runs through pkexec.
    pub fn needs_root(self) -> bool {
        self == Id::App(App::AuthFace)
    }

    /// How a source install handles a checkout that isn't on its default
    /// branch.
    pub fn off_branch(self) -> OffBranch {
        match self {
            Id::App(App::Whisper) | Id::App(App::Pop) | Id::Ui => OffBranch::Switch,
            _ => OffBranch::Worktree,
        }
    }

    /// The packages of its release, as `<name>-x86_64.deb` / `.rpm`, for the
    /// components that ship them besides vinoPilot's own (found, never
    /// offered). The first is the one the rest depend on; the later ones are
    /// installed with it by default, and the `optional` ones only where
    /// they already are.
    pub fn release_packages(self) -> Option<ReleasePackages> {
        match self {
            Id::App(App::AuthFace) => Some(ReleasePackages {
                always: &["vinoauthface", "vinoauthface-tray"],
                optional: &["vinoauthface-settings"],
            }),
            _ => None,
        }
    }

    /// The ways in it offers, best first. This machine's own package format
    /// comes after the release and source, for a component that ships
    /// packages; vinoPilot's own Flatpak, like its `.deb` and `.rpm`, is found
    /// but never offered (pinned by
    /// `tests::only_vinowhisper_and_vinosearch_offer_their_flatpak`,
    /// `tests::a_flatpak_is_never_the_default_way_in`,
    /// `tests::vinoauthface_offers_the_package_format_its_machine_uses`).
    pub fn channels(self) -> Vec<Channel> {
        self.channels_on(Channel::native_package())
    }

    /// [`Id::channels`] on a machine whose package format is `native`.
    pub fn channels_on(self, native: Option<Channel>) -> Vec<Channel> {
        let mut all = Vec::new();
        if self != Id::Ui {
            all.push(Channel::Release);
        }
        all.push(Channel::Source);
        if matches!(self.package(), Some(Package::PyPi(_) | Package::Crate(_))) {
            all.push(Channel::Package);
        }
        if self.release_packages().is_some() {
            all.extend(native);
        }
        if self.flatpak().is_some() && self != Id::App(App::Pilot) {
            all.push(Channel::Flatpak);
        }
        all
    }
}

/// The `.deb`s and `.rpm`s of one component's release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleasePackages {
    /// Installed by an install, and by every update.
    pub always: &'static [&'static str],
    /// Updated only where already installed (they depend on the same
    /// version of `always`, so one left behind would block the update).
    pub optional: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Package {
    PyPi(&'static str),
    Crate(&'static str),
    /// On crates.io, but a library: nothing to install, apps depend on it.
    Library(&'static str),
}

impl Package {
    pub fn name(self) -> &'static str {
        match self {
            Package::PyPi(n) | Package::Crate(n) | Package::Library(n) => n,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// The GitHub release's hash-checked installer.
    Release,
    /// A git checkout in the source folder, built locally.
    Source,
    /// PyPI through `uv tool`, or crates.io through `cargo install`.
    Package,
    /// A release's `.deb`, system-wide. vinoPilot's own is found this way
    /// but never offered; [`Id::release_packages`] lists who else has one,
    /// and `channels()` offers it on a machine that uses apt.
    Deb,
    /// The same as an `.rpm`, on a machine that uses dnf, zypper or rpm.
    Rpm,
    /// The release's Flatpak bundle, for the user or system-wide: vinoPilot's
    /// own (found, not offered), and the apps that have one ([`Id::flatpak`]).
    Flatpak,
}

impl Channel {
    /// `release`, `source`, `package` (also `pypi`, `crates`), `flatpak`,
    /// `deb` or `rpm`.
    pub fn parse(text: &str) -> Option<Channel> {
        match text.to_lowercase().as_str() {
            "release" | "binary" => Some(Channel::Release),
            "source" | "git" => Some(Channel::Source),
            "package" | "pypi" | "crates" | "crates.io" | "cargo" | "uv" => Some(Channel::Package),
            "deb" => Some(Channel::Deb),
            "rpm" => Some(Channel::Rpm),
            "flatpak" => Some(Channel::Flatpak),
            _ => None,
        }
    }

    /// The package format this machine installs: apt's `.deb`, or the `.rpm`
    /// of dnf, zypper or rpm. None on an image that layers packages
    /// (rpm-ostree), which vinoPilot doesn't drive, and where there is no
    /// package manager it knows.
    pub fn native_package() -> Option<Channel> {
        Channel::native_package_with(|path| std::path::Path::new(path).exists())
    }

    /// [`Channel::native_package`] given a way to ask whether a path exists.
    pub fn native_package_with(exists: impl Fn(&str) -> bool) -> Option<Channel> {
        if exists("/run/ostree-booted") {
            None
        } else if exists("/usr/bin/apt-get") {
            Some(Channel::Deb)
        } else if ["/usr/bin/dnf", "/usr/bin/zypper", "/usr/bin/rpm"]
            .into_iter()
            .any(exists)
        {
            Some(Channel::Rpm)
        } else {
            None
        }
    }

    pub fn label(self, id: Id) -> &'static str {
        match (self, id.package()) {
            (Channel::Release, _) => "Release",
            (Channel::Source, _) => "Source",
            (Channel::Package, Some(Package::PyPi(_))) => "PyPI",
            (Channel::Package, _) => "crates.io",
            (Channel::Deb, _) => "deb package",
            (Channel::Rpm, _) => "rpm package",
            (Channel::Flatpak, _) => "Flatpak",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Update,
    Uninstall,
}

impl Action {
    pub fn verb(self) -> &'static str {
        match self {
            Action::Install => "Installing",
            Action::Update => "Updating",
            Action::Uninstall => "Uninstalling",
        }
    }
}

/// Where a source install builds from when the checkout is on another
/// branch. Either way the checkout's own branch is never pulled; on the
/// default branch both are a plain `git pull --ff-only`. Reaches the
/// prelude's `checkout` as `$OFF_BRANCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffBranch {
    /// A new path: the default branch in a worktree at `$WORKTREE`, removed
    /// after the install. For installs that copy everything out.
    Worktree,
    /// The checkout's own path, switched to the default branch with its
    /// changes stashed, and left there. For installs that keep running from
    /// it (venv links, launchers pointing into it), where a worktree would
    /// be a second copy the app quietly runs from.
    Switch,
}

impl OffBranch {
    pub fn name(self) -> &'static str {
        match self {
            OffBranch::Worktree => "worktree",
            OffBranch::Switch => "switch",
        }
    }
}

/// One shell script, run with `bash -c` as the user, or as root through
/// pkexec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub script: String,
    pub root: bool,
}

impl Step {
    fn user(script: &str) -> Self {
        Step {
            script: format!("{PRELUDE}\n{script}"),
            root: false,
        }
    }

    fn root(script: &str) -> Self {
        Step {
            script: format!("{PRELUDE}\n{script}"),
            root: true,
        }
    }
}

/// vinoPilot's Flatpak, and the menu entry's name.
pub const PILOT_APP_ID: &str = "io.github.karanshukla.vinopilot";

/// What uninstalling `id` installed through `channel` removes and what it
/// keeps, in a sentence or two for the confirmation. Each app's own
/// uninstaller deletes its downloaded models and keeps the user's data.
pub fn uninstall_note(id: Id, channel: Channel) -> String {
    if let (Channel::Flatpak, Some(app_id)) = (channel, id.flatpak())
        && id != Id::App(App::Pilot)
    {
        return format!(
            "Removes the app and its downloaded models. Your data and settings remain in ~/.var/app/{app_id}."
        );
    }
    let what = match id {
        Id::App(App::Pilot) => "Removes vinoPilot. Apps installed by vinoPilot remain installed.",
        Id::App(App::Watch) => "Removes the app.",
        Id::App(_) => {
            "Removes the app and its downloaded models. Your data and settings remain intact."
        }
        Id::Ovfetch => "Removes ovfetch. Previously downloaded runtimes remain installed.",
        Id::Ui => return "Moves the checkout to the trash.".into(),
    };
    if channel == Channel::Source {
        format!("{what} Moves the checkout to the trash.")
    } else {
        what.into()
    }
}
