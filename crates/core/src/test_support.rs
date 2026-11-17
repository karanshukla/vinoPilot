//! Fixtures shared by the unit tests.

use std::fs;
use std::ops::Deref;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A fresh directory under the temp dir, removed on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    /// `name` keeps tests that run in parallel apart.
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("vinopilot-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Writes `contents` to `relative`, creating its directories.
    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    /// An executable shell script at `relative`.
    pub fn script(&self, relative: &str, body: &str) -> PathBuf {
        let path = self.write(relative, format!("#!/bin/sh\n{body}"));
        make_executable(&path);
        path
    }

    pub fn mkdir(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn make_executable(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A command at `path` that answers `--version` with its name and `version`.
pub fn put_command(path: &Path, version: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let name = path.file_name().unwrap().to_string_lossy();
    fs::write(path, format!("#!/bin/sh\necho '{name} {version}'\n")).unwrap();
    make_executable(path);
}
