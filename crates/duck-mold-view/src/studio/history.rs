//! Undo and redo.
//!
//! Every edit is written to the recipe file at once, so there is no
//! "unsaved" state to go back to. What is kept instead is the file's
//! whole text before and after each change, in the order they happened.
//! Undo writes the text from before the latest change back to the file;
//! redo writes the text from after it again.
//!
//! A change made by another program while the Studio is open (an agent,
//! a text editor) is a step too, called "Changed outside the Studio", so
//! undo always takes back the most recent change to the file, whoever
//! made it, and nobody's work is silently lost.
//!
//! The steps are kept in memory, and forgotten when the Studio closes.

use duck_mold_core::edit;

use super::{Studio, first_error};

/// How many steps are kept. The oldest are forgotten after that.
pub const MOST_STEPS: usize = 500;

/// What a step that came from outside is called.
pub const FROM_OUTSIDE: &str = "Changed outside the Studio";

/// One change to the file.
pub(super) struct Step {
    /// What it did, in a few words.
    title: String,
    /// The file's text before it.
    before: String,
    /// The file's text after it.
    after: String,
}

impl Studio {
    /// Remembers a change the Studio made.
    pub(super) fn edited(&mut self, before: String, title: &str) {
        self.remember(before, title);
    }

    /// Remembers a change another program made.
    pub(super) fn changed_outside(&mut self, before: String) {
        self.remember(before, FROM_OUTSIDE);
    }

    fn remember(&mut self, before: String, title: &str) {
        if before == self.text {
            return;
        }
        self.undo.push(Step {
            title: title.to_owned(),
            before,
            after: self.text.clone(),
        });
        if self.undo.len() > MOST_STEPS {
            self.undo.remove(0);
        }
        // A new change after an undo: what was undone can't come back.
        self.redo.clear();
    }

    /// What undo would take back, if there is anything.
    pub fn can_undo(&self) -> Option<&str> {
        self.undo.last().map(|step| step.title.as_str())
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
        // change, and it becomes the step to undo.
        self.check_files();
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
        let places = self.places();
        match edit::swap_in(&self.path, Some(from), to) {
            Ok(true) => {
                self.reload();
                self.keep_still(&places);
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
                refuse(self, &why)
            }
        }
    }
}
