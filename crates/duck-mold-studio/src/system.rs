//! Handing a file to the system: showing it in its folder, or opening
//! it with the program the system opens it with.

use std::path::Path;
use std::process::Command;

/// Opens `path` with the program the system uses for it (a text note in
/// Notepad on Windows). False if that couldn't be started.
pub fn open(path: &Path) -> bool {
    let started = if cfg!(windows) {
        Command::new("explorer").arg(path).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };
    started.is_ok()
}

/// Shows `path` in its folder, picked out where the system can (Windows'
/// Explorer, macOS' Finder); elsewhere the folder is opened. False if
/// that couldn't be started.
pub fn show_in_folder(path: &Path) -> bool {
    let started = if cfg!(windows) {
        let mut select = std::ffi::OsString::from("/select,");
        select.push(path);
        Command::new("explorer").arg(select).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg("-R").arg(path).spawn()
    } else {
        let folder = path.parent().unwrap_or(path);
        Command::new("xdg-open").arg(folder).spawn()
    };
    started.is_ok()
}
