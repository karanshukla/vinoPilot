//! The one setting: where source checkouts go. Shared by the CLI and the
//! window, in `~/.config/vinopilot/config`, one `key = value` per line.

use std::path::{Path, PathBuf};

use crate::detect::Paths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub source_root: PathBuf,
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn file() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("vinopilot/config")
}

/// `~/Development` when there is one (where the vinoPilot checkouts
/// usually are), else `~/src`.
pub fn default_source_root(home: &Path) -> PathBuf {
    let dev = home.join("Development");
    if dev.is_dir() { dev } else { home.join("src") }
}

/// `~/x` to `$HOME/x`.
pub fn expand(text: &str, home: &Path) -> PathBuf {
    match text.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if text == "~" => home.to_path_buf(),
        None => PathBuf::from(text),
    }
}

pub fn parse(text: &str, home: &Path) -> Config {
    let mut config = Config {
        source_root: default_source_root(home),
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if key.trim() == "source_root" && !value.is_empty() {
            config.source_root = expand(value, home);
        }
    }
    config
}

impl Config {
    pub fn load() -> Config {
        let home = home();
        parse(&std::fs::read_to_string(file()).unwrap_or_default(), &home)
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            path,
            format!("source_root = {}\n", self.source_root.display()),
        )
    }

    pub fn paths(&self) -> Paths {
        Paths {
            home: home(),
            source_root: self.source_root.clone(),
            system_bin: PathBuf::from("/usr/bin"),
            system_flatpak: PathBuf::from("/var/lib/flatpak"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_source_root_and_expands_home() {
        let home = Path::new("/home/k");
        assert_eq!(
            parse("source_root = ~/code\n", home).source_root,
            PathBuf::from("/home/k/code")
        );
        assert_eq!(
            parse("# nothing\n", home).source_root,
            default_source_root(home)
        );
    }
}
