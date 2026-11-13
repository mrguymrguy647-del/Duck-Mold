//! The recipes opened lately, for the start page.
//!
//! They are kept in a small text file, one path to a line, the newest
//! first, in the folder the system gives programs for their settings:
//! `%APPDATA%\Duck Mold\recent.txt` on Windows, `~/.config/duck-mold/`
//! elsewhere. Beside it, an empty file `tips-seen` says that the first
//! run's tips have been shown, and `hidden.txt` which parts of those
//! recipes were hidden in the picture: each file's path on a line, and
//! under it the parts, one to a line, each after a tab. Nothing else is
//! stored there, and a model is never kept anywhere but in its own
//! recipe file.

use std::path::{Path, PathBuf};

/// How many are remembered.
pub const MOST: usize = 8;

/// The files opened lately.
#[derive(Default)]
pub struct Recent {
    /// Where the list is kept. `None`: nowhere (it is forgotten when the
    /// Studio closes), as in the tests.
    kept_in: Option<PathBuf>,
    files: Vec<PathBuf>,
    /// The parts hidden in each file, by path, for the files in the
    /// list.
    hidden: Vec<(PathBuf, Vec<String>)>,
}

/// Where the system keeps this program's settings.
fn settings_folder() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Duck Mold"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|folder| folder.join("duck-mold"))
    }
}

impl Recent {
    /// A list that is kept nowhere.
    pub fn nowhere() -> Recent {
        Recent::default()
    }

    /// The person's own list, read from their settings folder.
    pub fn of_this_person() -> Recent {
        match settings_folder() {
            Some(folder) => Recent::kept_in(folder.join("recent.txt")),
            None => Recent::nowhere(),
        }
    }

    /// A list kept in the file `kept_in`, read now if it is there.
    pub fn kept_in(kept_in: PathBuf) -> Recent {
        let files = std::fs::read_to_string(&kept_in)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .take(MOST)
            .collect();
        let mut hidden: Vec<(PathBuf, Vec<String>)> = Vec::new();
        for line in std::fs::read_to_string(kept_in.with_file_name("hidden.txt"))
            .unwrap_or_default()
            .lines()
        {
            match (line.strip_prefix('\t'), hidden.last_mut()) {
                (Some(part), Some((_, parts))) if !part.trim().is_empty() => {
                    parts.push(part.trim().to_owned());
                }
                (Some(_), _) => {}
                (None, _) if !line.trim().is_empty() => {
                    hidden.push((PathBuf::from(line.trim()), Vec::new()));
                }
                (None, _) => {}
            }
        }
        Recent {
            kept_in: Some(kept_in),
            files,
            hidden,
        }
    }

    /// The files, newest first, that are still there.
    pub fn files(&self) -> Vec<&Path> {
        self.files
            .iter()
            .filter(|file| file.is_file())
            .map(PathBuf::as_path)
            .collect()
    }

    /// Notes that a file was opened: it goes to the top of the list.
    pub fn opened(&mut self, file: &Path) {
        let file = whole(file);
        self.files.retain(|known| *known != file);
        self.files.insert(0, file);
        self.files.truncate(MOST);
        self.save();
    }

    /// The parts that were hidden in `file` when it was last open, by
    /// path. (Only for the files in the list.)
    pub fn hidden_in(&self, file: &Path) -> &[String] {
        let file = whole(file);
        self.hidden
            .iter()
            .find(|(known, _)| *known == file)
            .map_or(&[], |(_, parts)| parts.as_slice())
    }

    /// Remembers which parts are hidden in `file`, if that has changed.
    pub fn keep_hidden(&mut self, file: &Path, parts: &[String]) {
        if self.hidden_in(file) == parts {
            return;
        }
        let file = whole(file);
        self.hidden.retain(|(known, _)| *known != file);
        if !parts.is_empty() {
            self.hidden.push((file, parts.to_vec()));
        }
        self.save_hidden();
    }

    /// The file that says the first run's tips have been shown.
    fn tips_file(&self) -> Option<PathBuf> {
        self.kept_in
            .as_ref()
            .map(|kept_in| kept_in.with_file_name("tips-seen"))
    }

    /// Whether the first run's tips have been shown, or skipped. Where
    /// nothing is kept (as in the tests), they count as seen: there
    /// would be no way to stop showing them.
    pub fn tips_seen(&self) -> bool {
        self.tips_file().is_none_or(|file| file.exists())
    }

    /// Notes that the tips have been shown, so they aren't again.
    pub fn saw_tips(&self) {
        if let Some(file) = self.tips_file() {
            if let Some(folder) = file.parent() {
                let _ = std::fs::create_dir_all(folder);
            }
            let _ = std::fs::write(file, "");
        }
    }

    /// Writes the list down. If it can't be written, it is only
    /// forgotten: that is no reason to trouble anyone.
    fn save(&mut self) {
        let Some(kept_in) = &self.kept_in else {
            return;
        };
        if let Some(folder) = kept_in.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let lines: Vec<String> = self
            .files
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect();
        let _ = std::fs::write(kept_in, lines.join("\n") + "\n");
        // A file that has dropped off the list takes its hidden parts
        // with it.
        let before = self.hidden.len();
        let files = self.files.clone();
        self.hidden.retain(|(file, _)| files.contains(file));
        if self.hidden.len() != before {
            self.save_hidden();
        }
    }

    /// Writes down which parts are hidden in which files.
    fn save_hidden(&self) {
        let Some(kept_in) = &self.kept_in else {
            return;
        };
        if let Some(folder) = kept_in.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let mut text = String::new();
        for (file, parts) in &self.hidden {
            text.push_str(&file.to_string_lossy());
            text.push('\n');
            for part in parts {
                text.push('\t');
                text.push_str(part);
                text.push('\n');
            }
        }
        let _ = std::fs::write(kept_in.with_file_name("hidden.txt"), text);
    }
}

/// The whole path, so that the list still works when the Studio is next
/// started from another folder.
fn whole(file: &Path) -> PathBuf {
    std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf())
}
