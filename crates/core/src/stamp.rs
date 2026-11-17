//! Shared with build.rs, which decides the stamp; lib.rs compiles it only to
//! run these tests.

use std::path::Path;

/// `VINOPILOT_VERSION` when release CI sets it. Otherwise the crate sources
/// outside a git checkout are a `cargo install vinopilot` from the registry
/// (the published tarball has no `.git`), which is a release too. A checkout
/// is dev: its `.git` sits two levels above this crate, in the workspace root
/// (pinned by `stamp::tests::a_registry_build_is_a_release_and_a_checkout_is_dev`).
pub fn stamp(env: Option<String>, pkg_version: &str, manifest_dir: &Path) -> Option<String> {
    match env.filter(|v| !v.is_empty()) {
        Some(v) => Some(v),
        None if manifest_dir.join("../../.git").exists() => None,
        None => Some(pkg_version.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::stamp;
    use crate::test_support::Scratch;

    /// A workspace-shaped folder, with a `.git` at its root when `git`; the
    /// crate's own folder is what the stamp is asked about.
    fn crate_dir(name: &str, git: bool) -> (Scratch, std::path::PathBuf) {
        let root = Scratch::new(name);
        root.write("crates/core/Cargo.toml", "");
        if git {
            root.write(".git", "gitdir: elsewhere");
        }
        let dir = root.join("crates/core");
        (root, dir)
    }

    #[test]
    fn a_registry_build_is_a_release_and_a_checkout_is_dev() {
        let (_registry, registry) = crate_dir("stamp-registry", false);
        let (_checkout, checkout) = crate_dir("stamp-checkout", true);
        assert_eq!(stamp(None, "0.4.0", &registry), Some("0.4.0".into()));
        assert_eq!(stamp(None, "0.4.0", &checkout), None);
        assert_eq!(stamp(Some(String::new()), "0.4.0", &checkout), None);
    }

    /// CI runs this from the unpacked crate, which is what the registry
    /// compiles, with the variable set; in a checkout it has nothing to say.
    #[test]
    fn a_build_outside_a_checkout_is_a_release() {
        if std::env::var_os("VINOPILOT_EXPECT_RELEASE").is_some() {
            assert_eq!(crate::VERSION, env!("CARGO_PKG_VERSION"));
        }
    }

    #[test]
    fn release_ci_stamps_even_inside_a_checkout() {
        let (_root, checkout) = crate_dir("stamp-ci", true);
        assert_eq!(
            stamp(Some("0.4.0".into()), "0.4.0", &checkout),
            Some("0.4.0".into())
        );
    }
}
