//! Decides whether this build is a release, for `vinopilot_core::VERSION`.

use std::path::Path;

#[path = "src/stamp.rs"]
mod stamp;

fn main() {
    println!("cargo:rerun-if-env-changed=VINOPILOT_VERSION");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/stamp.rs");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let pkg_version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let env = std::env::var("VINOPILOT_VERSION").ok();
    if let Some(v) = stamp::stamp(env, &pkg_version, Path::new(&manifest_dir)) {
        assert_eq!(
            v, pkg_version,
            "VINOPILOT_VERSION ({v}) is not the crate version ({pkg_version})"
        );
        println!("cargo:rustc-env=VINOPILOT_STAMP={v}");
    }
}
