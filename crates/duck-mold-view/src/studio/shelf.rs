//! The animation library: the animations that can be copied into the
//! model. The starters that come inside the program, and the files in
//! the folder `animations` beside the recipe (each a file `duck-mold
//! save-animation` wrote, or any recipe with animations).
//!
//! Copying one is an `add-animation --from` (or `--starter`) edit, and
//! saving one is what `duck-mold save-animation` does.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use duck_mold_core::edit::{self, AnimationFrom, Edit, animations_in};
use duck_mold_core::source::Syntax;
use duck_mold_core::starters::STARTERS;
use duck_mold_core::tools;

use super::{Studio, first_error};

/// The folder beside the recipe that holds saved animations.
pub const FOLDER: &str = "animations";

/// One animation of the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelved {
    /// Its name.
    pub name: String,
    /// Where it comes from: a starter, or a file.
    pub from: AnimationFrom,
    /// Where it comes from, in a few words: `starter`, or the file as
    /// the recipe would write it (`animations/walk.toml`).
    pub source: String,
    /// What it is for and what it does, in a sentence.
    pub about: String,
    /// The parts it moves.
    pub parts: Vec<String>,
    /// Why it can't be used, if it can't: the file can't be read.
    pub problem: Option<String>,
}

/// The library as it was last read.
#[derive(Default)]
pub(super) struct Shelf {
    files: Vec<Shelved>,
    /// Each file's time and size when it was read, to see a change.
    stamps: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    /// How many times it has been read.
    reads: usize,
}

/// The files of a folder that can hold animations, in name order.
fn animation_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && Syntax::from_path(path).is_some())
        .collect();
    files.sort();
    files
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// Seconds as a sentence says them: `0.8 s`.
fn seconds(at: f64) -> String {
    format!("{} s", (at * 100.0).round() / 100.0)
}

impl Studio {
    /// The folder of saved animations: `animations` beside the recipe.
    pub fn animation_folder(&self) -> PathBuf {
        self.path.parent().unwrap_or(Path::new("")).join(FOLDER)
    }

    /// The animation library: the six starters, then the animations of
    /// the files in the folder `animations`.
    pub fn animation_library(&self) -> Vec<Shelved> {
        let starters = STARTERS.iter().map(|starter| Shelved {
            name: starter.name.to_owned(),
            from: AnimationFrom::Starter(starter.name.to_owned()),
            source: "starter".to_owned(),
            about: format!("For {}. {}", starter.made_for, starter.does),
            parts: starter
                .parts
                .iter()
                .map(|part| (*part).to_owned())
                .collect(),
            problem: None,
        });
        starters.chain(self.shelf.files.iter().cloned()).collect()
    }

    /// How many times the files of the animation library have been
    /// read.
    pub fn animation_library_reads(&self) -> usize {
        self.shelf.reads
    }

    /// The parts an animation of the library moves that this model
    /// doesn't have.
    pub fn missing_parts(&self, shelved: &Shelved) -> Vec<String> {
        shelved
            .parts
            .iter()
            .filter(|part| self.part(part).is_none())
            .cloned()
            .collect()
    }

    /// Copies an animation of the library into the model, with the poses
    /// it uses: `add-animation --from` (or `--starter`), under its own
    /// name or, if that is taken, with a number. What it does to parts
    /// the model doesn't have is left out (`--skip-missing`). It is then
    /// the timeline's animation.
    pub fn add_from_library(&mut self, shelved: &Shelved) -> Result<String, String> {
        let name = edit::unused_name(&shelved.name, &self.animation_names());
        let from = match &shelved.from {
            AnimationFrom::File { file, .. } => AnimationFrom::File {
                file: file.clone(),
                take: Some(shelved.name.clone()),
            },
            starter => starter.clone(),
        };
        let done = self.apply(&Edit::AddAnimation {
            name: name.clone(),
            looping: false,
            from: Some(from),
            skip_missing: true,
        });
        if let Ok(said) = &done {
            // Chosen for the timeline, still saying what was added.
            self.choose_animation(Some(&name));
            self.say(said.clone());
        }
        done
    }

    /// Writes an animation of the model, and the poses it uses, to the
    /// folder `animations` beside the recipe, as `duck-mold
    /// save-animation` does: `<its name>.toml` (with a number added if
    /// that is taken). Says where it went, and reads the library again.
    pub fn save_to_animation_library(&mut self, animation: &str) -> Result<String, String> {
        let folder = self.animation_folder();
        let mut file = folder.join(format!("{animation}.toml"));
        let mut count = 2;
        while file.exists() {
            file = folder.join(format!("{animation}_{count}.toml"));
            count += 1;
        }
        let done = tools::save_animation(&self.path, animation, &file)
            .map(|_| {
                let name = file
                    .file_name()
                    .map_or(String::new(), |name| name.to_string_lossy().into_owned());
                format!(
                    "Saved the animation \"{animation}\" to the library: {FOLDER}/{name}. Another model takes it from the Animations list"
                )
            })
            .map_err(|failure| format!("Not saved: {}", self.shorten(&first_error(&failure))));
        self.read_shelf();
        self.say(match &done {
            Ok(said) | Err(said) => said.clone(),
        });
        done
    }

    /// Reads the folder of animations again if a file in it was saved,
    /// made or removed. True if it was read.
    pub(super) fn check_shelf(&mut self) -> bool {
        let files = animation_files(&self.animation_folder());
        let same = files.len() == self.shelf.stamps.len()
            && self
                .shelf
                .stamps
                .iter()
                .zip(&files)
                .all(|((path, was), file)| path == file && *was == stamp(file));
        if same && self.shelf.reads > 0 {
            return false;
        }
        self.read_shelf();
        true
    }

    /// Reads the folder of animations.
    pub(super) fn read_shelf(&mut self) {
        let files = animation_files(&self.animation_folder());
        self.shelf.stamps = files
            .iter()
            .map(|file| (file.clone(), stamp(file)))
            .collect();
        self.shelf.reads += 1;
        let open = self.path.canonicalize().ok();
        self.shelf.files = files
            .iter()
            // The recipe's own animations are already in it.
            .filter(|file| file.canonicalize().ok() != open)
            .flat_map(|file| {
                let name = file
                    .file_name()
                    .map_or(String::new(), |name| name.to_string_lossy().into_owned());
                let written = format!("{FOLDER}/{name}");
                let from = AnimationFrom::File {
                    file: written.clone(),
                    take: None,
                };
                let offered = Syntax::from_path(file)
                    .and_then(|syntax| animations_in(&std::fs::read_to_string(file).ok()?, syntax));
                match offered {
                    Some(offered) if !offered.is_empty() => offered
                        .into_iter()
                        .map(|offered| Shelved {
                            about: format!(
                                "{} keys, {}{}. Moves {}.",
                                offered.keys,
                                seconds(offered.length),
                                if offered.looping { ", loops" } else { "" },
                                offered.parts.join(", ")
                            ),
                            name: offered.name,
                            from: from.clone(),
                            source: written.clone(),
                            parts: offered.parts,
                            problem: None,
                        })
                        .collect::<Vec<_>>(),
                    Some(_) => vec![Shelved {
                        name: name.clone(),
                        from,
                        source: written,
                        about: String::new(),
                        parts: Vec::new(),
                        problem: Some("it has no animations".to_owned()),
                    }],
                    None => vec![Shelved {
                        name: name.clone(),
                        from,
                        source: written,
                        about: String::new(),
                        parts: Vec::new(),
                        problem: Some("it can't be read: check it with duck-mold check".to_owned()),
                    }],
                }
            })
            .collect();
    }
}
