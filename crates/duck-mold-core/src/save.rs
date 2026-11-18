//! Writing files so that nothing is ever half written.
//!
//! Every file Duck Mold writes (a recipe, a `.glb`, a picture) is written
//! beside the real one first, flushed to the disk, and then swapped in
//! with one rename. So:
//!
//! - a program reading the file at that moment (a game, an editor) sees
//!   either the old file or the new one, never half of one;
//! - a power cut at the wrong moment leaves the old file or the new one,
//!   never an empty one;
//! - if writing fails (a full disk, a locked file), the old file is as it
//!   was, and the half-written one beside it is taken away.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The file a new version is written to before it is swapped in:
/// `robot.toml.duck-mold-saving` beside `robot.toml`.
pub fn beside(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".duck-mold-saving");
    path.with_file_name(name)
}

/// Writes `bytes` to `path` safely (see the module). With `expected`,
/// the file must still hold exactly that just before the swap, or nothing
/// is written and `Ok(false)` comes back: another program saved it
/// meanwhile.
pub fn write_safely(path: &Path, bytes: &[u8], expected: Option<&[u8]>) -> std::io::Result<bool> {
    let new = beside(path);
    let written = (|| {
        let mut file = std::fs::File::create(&new)?;
        file.write_all(bytes)?;
        // On the disk, not just in the system's memory, before the swap.
        file.sync_all()
    })();
    if let Err(error) = written {
        let _ = std::fs::remove_file(&new);
        return Err(error);
    }
    if let Some(expected) = expected {
        let now = std::fs::read(path).unwrap_or_default();
        if now != expected {
            let _ = std::fs::remove_file(&new);
            return Ok(false);
        }
    }
    if let Err(error) = std::fs::rename(&new, path) {
        let _ = std::fs::remove_file(&new);
        return Err(error);
    }
    Ok(true)
}

/// Why a file couldn't be written, in a few plain words: "the disk is
/// full", "another program has the file locked".
pub fn why_not_written(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    if is_disk_full(error) {
        return "the disk is full";
    }
    if is_locked(error) {
        return "another program has the file open and locked";
    }
    match error.kind() {
        ErrorKind::NotFound => "that folder doesn't exist",
        ErrorKind::PermissionDenied => {
            "permission denied (the file or its folder may be read-only)"
        }
        ErrorKind::ReadOnlyFilesystem => "the disk can only be read",
        _ => "it couldn't be written there",
    }
}

/// True for an error that means the disk is full.
pub fn is_disk_full(error: &std::io::Error) -> bool {
    if matches!(
        error.kind(),
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
    ) {
        return true;
    }
    match error.raw_os_error() {
        // Windows: ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL.
        Some(39 | 112) if cfg!(windows) => true,
        // Elsewhere: ENOSPC, EDQUOT.
        Some(28 | 122) if !cfg!(windows) => true,
        _ => false,
    }
}

/// True for an error that means another program holds the file
/// (Windows' sharing and lock violations).
pub fn is_locked(error: &std::io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("duck-mold-save-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_file_is_swapped_in_and_nothing_is_left_beside_it() {
        let folder = folder("swap");
        let path = folder.join("model.toml");
        std::fs::write(&path, "old").unwrap();
        assert!(write_safely(&path, b"new", Some(b"old")).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!beside(&path).exists());
        // Changed meanwhile: nothing written, nothing left.
        assert!(!write_safely(&path, b"newer", Some(b"old")).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!beside(&path).exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_failed_write_leaves_the_old_file_and_nothing_beside_it() {
        let folder = folder("fail");
        let path = folder.join("model.toml");
        std::fs::write(&path, "old").unwrap();
        // The rename can't put a file where a folder is: the write
        // beside works, the swap fails.
        let blocked = folder.join("a_folder");
        std::fs::create_dir_all(blocked.join("inside")).unwrap();
        let error = write_safely(&blocked, b"new", None).unwrap_err();
        assert!(!beside(&blocked).exists(), "{error}");
        // A file in a folder that isn't there.
        let error = write_safely(&folder.join("no/such/model.toml"), b"x", None).unwrap_err();
        assert_eq!(why_not_written(&error), "that folder doesn't exist");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn the_causes_are_said_plainly() {
        let full = std::io::Error::from(std::io::ErrorKind::StorageFull);
        assert_eq!(why_not_written(&full), "the disk is full");
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(why_not_written(&denied).starts_with("permission denied"));
        if cfg!(windows) {
            let locked = std::io::Error::from_raw_os_error(32);
            assert_eq!(
                why_not_written(&locked),
                "another program has the file open and locked"
            );
            let full = std::io::Error::from_raw_os_error(112);
            assert_eq!(why_not_written(&full), "the disk is full");
        } else {
            let full = std::io::Error::from_raw_os_error(28);
            assert_eq!(why_not_written(&full), "the disk is full");
        }
    }
}
