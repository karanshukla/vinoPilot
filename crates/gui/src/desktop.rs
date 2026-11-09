//! The app menu entry and icon, written by the binary itself so every way
//! in (install.sh, `cargo install`, a checkout) gets the same entry.

use std::path::{Path, PathBuf};

pub const APP_ID: &str = vinopilot_core::catalog::PILOT_APP_ID;
const ICON: &str = include_str!("../data/io.github.karanshukla.vinopilot.svg");

fn data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| vinopilot_core::config::home().join(".local/share"))
}

fn entry_path(data: &Path) -> PathBuf {
    data.join("applications").join(format!("{APP_ID}.desktop"))
}

fn icon_path(data: &Path) -> PathBuf {
    data.join("icons/hicolor/scalable/apps")
        .join(format!("{APP_ID}.svg"))
}

pub fn entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]
Type=Application
Name=vinoPilot
GenericName=App manager
Comment=Install, update and uninstall the vinoPilot apps
Exec={}
Icon={APP_ID}
Terminal=false
Categories=Settings;PackageManager;
Keywords=vino;openvino;npu;install;update;
StartupWMClass={APP_ID}
",
        exec.display()
    )
}

fn refresh(data: &Path) {
    let _ = vinopilot_core::command("update-desktop-database")
        .arg("-q")
        .arg(data.join("applications"))
        .status();
}

/// Writes the entry (running `exec`) and the icon under `$XDG_DATA_HOME`.
pub fn install(exec: &Path) -> std::io::Result<PathBuf> {
    let data = data_home();
    let entry_file = entry_path(&data);
    let icon_file = icon_path(&data);
    for file in [&entry_file, &icon_file] {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&icon_file, ICON)?;
    std::fs::write(&entry_file, entry(exec))?;
    refresh(&data);
    Ok(entry_file)
}

pub fn uninstall() -> std::io::Result<()> {
    let data = data_home();
    for file in [entry_path(&data), icon_path(&data)] {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    refresh(&data);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_runs_the_given_binary_and_names_the_icon() {
        let text = entry(Path::new("/home/k/.local/bin/vinopilot-gui"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("\nExec=/home/k/.local/bin/vinopilot-gui\n"));
        assert!(text.contains(&format!("\nIcon={APP_ID}\n")));
        assert!(ICON.starts_with("<svg"));
    }
}
