//! `duck-mold studio` and `duck-mold preview`: starting the Studio.
//!
//! The Studio is a program of its own, `DuckMoldStudio`, so that this one
//! holds no window library and builds quickly on machines with no
//! screen. It is looked for next to `duck-mold`, where a build and a
//! release both put it.

use std::path::{Path, PathBuf};
use std::process::Command;

use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::load::display_path;
use duck_mold_core::tools::Failure;

/// The Studio program's file name on this system.
pub const PROGRAM: &str = if cfg!(windows) {
    "DuckMoldStudio.exe"
} else {
    "DuckMoldStudio"
};

fn refuse(message: String) -> Failure {
    Failure::Usage(Box::new(Issue::error(Code::Io, "", message)))
}

/// Where the Studio program is: next to this one. `DUCK_MOLD_STUDIO` names
/// another place (the tests use it).
fn program() -> Result<PathBuf, Failure> {
    let beside = match std::env::var_os("DUCK_MOLD_STUDIO") {
        Some(named) => PathBuf::from(named),
        None => std::env::current_exe()
            .ok()
            .and_then(|this| this.parent().map(|folder| folder.join(PROGRAM)))
            .unwrap_or_else(|| PathBuf::from(PROGRAM)),
    };
    if beside.is_file() {
        Ok(beside)
    } else {
        Err(refuse(format!(
            "the Studio program isn't here: \"{}\" is missing. Build it with: cargo build --release -p duck-mold-studio",
            display_path(&beside)
        )))
    }
}

/// Starts the Studio on `file` (or on its start page) and waits until its
/// window is closed. With `look_only`, nothing can be changed there.
pub fn run(file: Option<&Path>, look_only: bool) -> Result<(), Failure> {
    if let Some(file) = file
        && !file.is_file()
    {
        return Err(refuse(format!(
            "can't read \"{}\": there is no such file",
            display_path(file)
        )));
    }
    let program = program()?;
    let mut command = Command::new(&program);
    if look_only {
        command.arg("--look");
    }
    if let Some(file) = file {
        command.arg(file);
    }
    let status = command.status().map_err(|error| {
        refuse(format!(
            "can't start \"{}\": {error}",
            display_path(&program)
        ))
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(refuse(
            "the Studio couldn't open its window. On a machine with no screen, use render"
                .to_owned(),
        ))
    }
}
