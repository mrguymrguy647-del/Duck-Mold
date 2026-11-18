//! When the Studio crashes: a note of what happened, and a word about it
//! on the next start.
//!
//! Every change is already in the recipe (the Studio writes each one at
//! once) and the undo history is on disk, so a crash loses nothing but
//! the drag that was in hand. What it does leave is a question: what
//! happened? [`install`] makes a crash write `crash-<date>.txt` in the
//! Studio's own folder, beside `recent.txt`, holding the version, the
//! message and the place in the code, the file that was open and the
//! last things the Studio said. [`take_unseen`] finds it on the next
//! start, for the start page.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many of the things the Studio said are kept for the note.
pub const LINES_KEPT: usize = 20;

/// What the Studio was doing, kept up to date while it runs, for the note.
struct Doing {
    file: Option<PathBuf>,
    said: Vec<String>,
}

static DOING: Mutex<Doing> = Mutex::new(Doing {
    file: None,
    said: Vec::new(),
});

/// Remembers the file that is open (`None`: the start page) and what the
/// Studio says now, for a crash note. Called every frame; a line the
/// same as the last is kept once.
pub fn remember(file: Option<&Path>, said: &str) {
    let Ok(mut doing) = DOING.lock() else {
        return;
    };
    if doing.file.as_deref() != file {
        doing.file = file.map(Path::to_path_buf);
    }
    if !said.is_empty() && doing.said.last().map(String::as_str) != Some(said) {
        doing.said.push(said.to_owned());
        if doing.said.len() > LINES_KEPT {
            doing.said.remove(0);
        }
    }
}

/// Makes a crash write a note in `folder` (the Studio's own folder),
/// as well as what Rust prints anyway.
pub fn install(folder: PathBuf) {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_owned());
        let place = info.location().map_or_else(
            || "(unknown)".to_owned(),
            |at| format!("{}:{}", at.file(), at.line()),
        );
        let _ = write_note(&folder, &message, &place, SystemTime::now());
        before(info);
    }));
}

/// Writes the note, and gives where it went.
pub fn write_note(
    folder: &Path,
    message: &str,
    place: &str,
    when: SystemTime,
) -> std::io::Result<PathBuf> {
    let (file, said) = match DOING.lock() {
        Ok(doing) => (doing.file.clone(), doing.said.clone()),
        // A crash while the note's lines were being kept: no lines.
        Err(_) => (None, Vec::new()),
    };
    let mut text = format!(
        "Duck Mold Studio {} closed unexpectedly.\n\n\
         When:     {} (UTC)\n\
         Message:  {message}\n\
         Where:    {place}\n\
         Open:     {}\n",
        env!("CARGO_PKG_VERSION"),
        utc(when, " ", ":"),
        file.as_deref().map_or_else(
            || "nothing (the start page)".to_owned(),
            |file| file.display().to_string()
        ),
    );
    if file.is_some() {
        text.push_str(
            "\nEvery change was written to the recipe as it was made, so the model\n\
             is saved up to its last change, and Undo carries on next time.\n",
        );
    }
    text.push_str("\nThe last things the Studio said, oldest first:\n");
    if said.is_empty() {
        text.push_str("  (nothing)\n");
    }
    for line in &said {
        text.push_str(&format!("  {line}\n"));
    }
    text.push_str(
        "\nIf this happens again, please send this note with the recipe to the\n\
         people who make Duck Mold.\n",
    );
    std::fs::create_dir_all(folder)?;
    let path = folder.join(format!("crash-{}.txt", utc(when, "-", "-")));
    std::fs::write(&path, text)?;
    Ok(path)
}

/// A crash the Studio hasn't told about yet.
#[derive(Debug, Clone, PartialEq)]
pub struct Crash {
    /// The note.
    pub note: PathBuf,
    /// The file that was open, if one was and it is still there.
    pub file: Option<PathBuf>,
}

/// The newest crash note in `folder` not told about yet, if there is one.
/// All the notes there count as told about afterwards, so each crash is
/// told about once.
pub fn take_unseen(folder: &Path) -> Option<Crash> {
    let seen_list = folder.join("crashes-seen.txt");
    let seen = std::fs::read_to_string(&seen_list).unwrap_or_default();
    let seen: Vec<&str> = seen.lines().collect();
    let mut notes: Vec<String> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("crash-") && name.ends_with(".txt"))
        .collect();
    notes.sort();
    let newest = notes
        .iter()
        .rev()
        .find(|name| !seen.contains(&name.as_str()))?
        .clone();
    let _ = std::fs::write(&seen_list, notes.join("\n") + "\n");
    let note = folder.join(&newest);
    let text = std::fs::read_to_string(&note).unwrap_or_default();
    let file = text
        .lines()
        .find_map(|line| line.strip_prefix("Open:"))
        .map(|file| PathBuf::from(file.trim()))
        .filter(|file| file.is_file());
    Some(Crash { note, file })
}

/// The time in UTC: `2026-11-18 14:02:33` (with `" "` and `":"`).
fn utc(when: SystemTime, between: &str, in_time: &str) -> String {
    let seconds = when
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (seconds / 86400, seconds % 86400);
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}{between}{:02}{in_time}{:02}{in_time}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The date of a day counted from 1970-01-01 (Howard Hinnant's method).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dates_are_right() {
        assert_eq!(utc(UNIX_EPOCH, " ", ":"), "1970-01-01 00:00:00");
        let when = UNIX_EPOCH + Duration::from_secs(1_795_010_553);
        assert_eq!(utc(when, " ", ":"), "2026-11-18 14:02:33");
        assert_eq!(utc(when, "-", "-"), "2026-11-18-14-02-33");
        // A leap day.
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(utc(leap, " ", ":"), "2000-02-29 00:00:00");
    }
}
