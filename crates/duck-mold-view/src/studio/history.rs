//! Undo and redo.
//!
//! Every edit is written to the recipe file at once, so there is no
//! "unsaved" state to go back to. What is kept instead is the file's
//! whole text before and after each change, in the order they happened.
//! Undo writes the text from before the latest change back to the file;
//! redo writes the text from after it again.
//!
//! A change made by another program while the Studio is open (an agent,
//! a text editor) is kept as a step too, called "Changed outside the
//! Studio", but it is a wall: Undo stops there and says so. Undo only
//! ever takes back what the Studio did, so another program's work is
//! never written over.
//!
//! With [`Studio::keep_history_in`], the steps are kept on disk too, one
//! file for each recipe, so that Undo carries on after the Studio is
//! closed and opened again, or after a crash.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use duck_mold_core::edit;
use serde::{Deserialize, Serialize};

use super::{Studio, first_error};

/// How many steps are kept in memory. The oldest are forgotten after that.
pub const MOST_STEPS: usize = 500;

/// How many steps are kept on disk for each recipe.
pub const MOST_STEPS_KEPT: usize = 100;

/// How much text the steps kept on disk for one recipe may hold.
pub const MOST_BYTES_KEPT: usize = 5 * 1024 * 1024;

/// What a step that came from outside is called.
pub const FROM_OUTSIDE: &str = "Changed outside the Studio";

/// What a step is called when the recipe was changed while the Studio
/// was closed.
pub const WHILE_CLOSED: &str = "Changed while the Studio was closed";

/// One change to the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Step {
    /// What it did, in a few words.
    title: String,
    /// The file's text before it.
    before: String,
    /// The file's text after it.
    after: String,
    /// For a change another program made: when it was seen, in seconds
    /// since 1970. Undo stops at such a step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    outside: Option<u64>,
}

/// The steps as they are kept on disk.
#[derive(Serialize, Deserialize)]
struct Kept {
    /// Which recipe they are for.
    recipe: PathBuf,
    undo: Vec<Step>,
    redo: Vec<Step>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// "just now", "3 minutes ago", "2 hours ago", "4 days ago".
fn ago(seconds: u64) -> String {
    let plural = |count: u64, what: &str| {
        if count == 1 {
            format!("1 {what} ago")
        } else {
            format!("{count} {what}s ago")
        }
    };
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => plural(seconds / 60, "minute"),
        3600..86400 => plural(seconds / 3600, "hour"),
        _ => plural(seconds / 86400, "day"),
    }
}

/// A short name for a path that stays the same from one run to the
/// next (FNV-1a), for the history file's name.
fn short_hash(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:08x}", hash >> 32)
}

/// Where the steps of the recipe at `recipe` are kept, in `folder`:
/// `robot-1a2b3c4d.json`.
pub fn history_file(folder: &Path, recipe: &Path) -> PathBuf {
    let recipe = std::fs::canonicalize(recipe).unwrap_or_else(|_| recipe.to_owned());
    let stem = recipe
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    folder.join(format!("{stem}-{}.json", short_hash(&recipe)))
}

impl Studio {
    /// Keeps the undo history on disk, in `folder` (made if it isn't
    /// there), and carries on from what was kept there for this recipe
    /// last time.
    ///
    /// If the recipe holds what the history says it held last, Undo
    /// goes back through the old steps. If it was changed while the
    /// Studio was closed, that change is a wall, as one made while it is
    /// open.
    pub fn keep_history_in(&mut self, folder: impl Into<PathBuf>) {
        if self.look_only {
            return;
        }
        let file = history_file(&folder.into(), &self.path);
        let kept: Option<Kept> = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        self.history_file = Some(file);
        let Some(kept) = kept else {
            return;
        };
        let last = kept
            .undo
            .last()
            .map(|step| &step.after)
            .or_else(|| kept.redo.last().map(|step| &step.before));
        let Some(last) = last.cloned() else {
            return;
        };
        // Anything done in this window already comes after the kept steps.
        let mine = std::mem::take(&mut self.undo);
        self.undo = kept.undo;
        if last == self.text {
            self.redo = kept.redo;
        } else {
            let seen = std::fs::metadata(&self.path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or_else(now, |since| since.as_secs());
            self.undo.push(Step {
                title: WHILE_CLOSED.to_owned(),
                before: last,
                after: self.text.clone(),
                outside: Some(seen),
            });
            self.redo.clear();
        }
        self.undo.extend(mine);
        self.keep_history();
    }

    /// Writes the history to disk, if it is kept there. A history that
    /// can't be written is no reason to stop working: the recipe itself
    /// is already saved.
    fn keep_history(&mut self) {
        let Some(file) = &self.history_file else {
            return;
        };
        // The newest steps that fit.
        let mut bytes = 0;
        let mut first = self.undo.len();
        for step in self.undo.iter().rev() {
            let size = step.before.len() + step.after.len() + step.title.len();
            if self.undo.len() - first >= MOST_STEPS_KEPT || bytes + size > MOST_BYTES_KEPT {
                break;
            }
            bytes += size;
            first -= 1;
        }
        let redo_room = MOST_STEPS_KEPT.saturating_sub(self.undo.len() - first);
        let redo_first = self.redo.len().saturating_sub(redo_room);
        let kept = Kept {
            recipe: self.path.clone(),
            undo: self.undo[first..].to_vec(),
            redo: self.redo[redo_first..].to_vec(),
        };
        let Ok(text) = serde_json::to_vec(&kept) else {
            return;
        };
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = duck_mold_core::save::write_safely(file, &text, None);
    }

    /// Remembers a change the Studio made.
    pub(super) fn edited(&mut self, before: String, title: &str) {
        self.remember(before, title, None);
    }

    /// Remembers a change another program made: a wall for Undo.
    pub(super) fn changed_outside(&mut self, before: String) {
        self.remember(before, FROM_OUTSIDE, Some(now()));
    }

    fn remember(&mut self, before: String, title: &str, outside: Option<u64>) {
        if before == self.text {
            return;
        }
        self.undo.push(Step {
            title: title.to_owned(),
            before,
            after: self.text.clone(),
            outside,
        });
        if self.undo.len() > MOST_STEPS {
            self.undo.remove(0);
        }
        // A new change after an undo: what was undone can't come back.
        self.redo.clear();
        self.keep_history();
    }

    /// What undo would take back, if there is anything it may take back.
    /// `None` at a change made outside the Studio: see
    /// [`Studio::undo_stops`].
    pub fn can_undo(&self) -> Option<&str> {
        self.undo
            .last()
            .filter(|step| step.outside.is_none())
            .map(|step| step.title.as_str())
    }

    /// Why Undo stops where it is: the latest change was made by another
    /// program, and Undo doesn't write over it.
    pub fn undo_stops(&self) -> Option<String> {
        let step = self.undo.last()?;
        let seen = step.outside?;
        let name = self.title();
        let when = if step.title == WHILE_CLOSED {
            "while the Studio was closed".to_owned()
        } else {
            format!("outside the Studio {}", ago(now().saturating_sub(seen)))
        };
        Some(format!(
            "{name} was changed {when}. Undo stops here, so that change isn't lost"
        ))
    }

    /// What redo would bring back, if there is anything.
    pub fn can_redo(&self) -> Option<&str> {
        self.redo.last().map(|step| step.title.as_str())
    }

    /// Takes back the latest change to the file. Says what it took back.
    pub fn undo(&mut self) -> Result<String, String> {
        self.step_back_or_forward(true)
    }

    /// Makes the change that was last undone again.
    pub fn redo(&mut self) -> Result<String, String> {
        self.step_back_or_forward(false)
    }

    fn step_back_or_forward(&mut self, back: bool) -> Result<String, String> {
        let refuse = |studio: &mut Studio, why: &str| {
            studio.say(why);
            Err(why.to_owned())
        };
        if self.look_only {
            return refuse(self, "This window is only for looking: nothing was changed");
        }
        // If another program has just saved the file, that is the latest
        // change: a wall, which undo doesn't go past.
        self.check_files();
        if let Some(why) = self.missing() {
            return refuse(self, &why);
        }
        if back && let Some(why) = self.undo_stops() {
            return refuse(self, &why);
        }
        let step = if back {
            self.undo.pop()
        } else {
            self.redo.pop()
        };
        let Some(step) = step else {
            return refuse(
                self,
                if back {
                    "Nothing to undo"
                } else {
                    "Nothing to redo"
                },
            );
        };
        let (from, to) = if back {
            (&step.after, &step.before)
        } else {
            (&step.before, &step.after)
        };
        // Written only if the file still holds what this step left
        // there. If it doesn't, someone saved it this very moment.
        match edit::swap_in(&self.path, Some(from), to) {
            Ok(true) => {
                self.reload();
                let said = if back {
                    format!("Undid: {}", step.title)
                } else {
                    format!("Redid: {}", step.title)
                };
                if back {
                    self.redo.push(step);
                } else {
                    self.undo.push(step);
                }
                self.keep_history();
                self.say(said.clone());
                Ok(said)
            }
            Ok(false) => {
                if back {
                    self.undo.push(step);
                } else {
                    self.redo.push(step);
                }
                refuse(
                    self,
                    "The file was saved by another program just now: try again",
                )
            }
            Err(failure) => {
                let why = first_error(&failure);
                if back {
                    self.undo.push(step);
                } else {
                    self.redo.push(step);
                }
                refuse(self, &super::capital(&why))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_said_plainly() {
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(150), "2 minutes ago");
        assert_eq!(ago(7200), "2 hours ago");
        assert_eq!(ago(86400 * 3), "3 days ago");
    }

    #[test]
    fn the_history_file_is_named_after_the_recipe() {
        let file = history_file(Path::new("/kept"), Path::new("/models/robot.toml"));
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("robot-") && name.ends_with(".json"),
            "{name}"
        );
        // The same path, the same name; another path, another.
        assert_eq!(
            file,
            history_file(Path::new("/kept"), Path::new("/models/robot.toml"))
        );
        assert_ne!(
            file,
            history_file(Path::new("/kept"), Path::new("/other/robot.toml"))
        );
    }
}
