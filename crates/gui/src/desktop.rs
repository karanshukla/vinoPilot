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

pub fn entry(exec: &Path) -> std::io::Result<String> {
    let exec_str = exec.to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "executable path is not valid UTF-8",
        )
    })?;
    if exec_str
        .bytes()
        .any(|b| b == b'\r' || b == b'\n' || b == b'\0' || b < 0x20 || b == 0x7f)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "executable path contains control characters",
        ));
    }
    let quoted_exec = quote_desktop_exec_arg(exec_str);
    Ok(format!(
        "[Desktop Entry]
Type=Application
Name=vinoPilot
GenericName=App manager
Comment=Install, update and uninstall the vinoPilot apps
Exec={quoted_exec}
Icon={APP_ID}
Terminal=false
Categories=Settings;PackageManager;
Keywords=vino;openvino;npu;install;update;
StartupWMClass={APP_ID}
"
    ))
}

fn quote_desktop_exec_arg(arg: &str) -> String {
    const RESERVED: &[u8] = b" \t\n\"'\\><~|&;$*?#()`";
    let needs_quotes = arg.is_empty() || arg.bytes().any(|b| RESERVED.contains(&b) || b == b'%');
    if !needs_quotes {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 4);
    out.push('"');
    for ch in arg.chars() {
        match ch {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            '%' => out.push_str("%%"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn refresh(data: &Path) {
    let _ = vinopilot_core::command("update-desktop-database")
        .arg("-q")
        .arg(data.join("applications"))
        .status();
}

/// Writes the entry (running `exec`) and the icon under `$XDG_DATA_HOME`.
pub fn install(exec: &Path) -> std::io::Result<PathBuf> {
    let entry_text = entry(exec)?;
    let data = data_home();
    let entry_file = entry_path(&data);
    let icon_file = icon_path(&data);
    for file in [&entry_file, &icon_file] {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(&icon_file, ICON)?;
    std::fs::write(&entry_file, entry_text)?;
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
        let text = entry(Path::new("/home/k/.local/bin/vinopilot-gui")).unwrap();
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("\nExec=/home/k/.local/bin/vinopilot-gui\n"));
        assert!(text.contains(&format!("\nIcon={APP_ID}\n")));
        assert!(ICON.starts_with("<svg"));
    }

    #[test]
    fn entry_rejects_control_characters_and_newlines() {
        assert!(entry(Path::new("/home/k/bad\nExec=evil")).is_err());
        assert!(entry(Path::new("/home/k/bad\rExec=evil")).is_err());
        assert!(entry(Path::new("/home/k/bad\x07path")).is_err());
    }

    #[test]
    fn entry_quotes_and_escapes_special_characters_per_desktop_spec() {
        let text = entry(Path::new("/home/k/My Apps/$bin\"100%")).unwrap();
        assert!(
            text.contains("\nExec=\"/home/k/My Apps/\\$bin\\\"100%%\"\n"),
            "{text}"
        );
    }
}
