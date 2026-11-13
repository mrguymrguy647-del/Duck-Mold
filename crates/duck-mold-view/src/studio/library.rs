//! The parts library: a folder of recipes and model files that can be
//! added to a socket like any shape.
//!
//! The folder is `parts` next to the recipe, unless another is chosen.
//! Every recipe (`.toml`, `.json`) and model file (`.obj`, `.gltf`,
//! `.glb`) in it is a part, shown with a small picture drawn by the same
//! code as `duck-mold render`. Adding one is an `add --file` edit: the
//! recipe then names the file, and nothing is copied.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use duck_mold_core::tools;

use super::viewport::Picture;
use super::{Studio, first_error};
use crate::render;

/// How big a part's small picture is, in pixels each way.
pub const PICTURE_SIZE: usize = 96;

/// The file name endings of what can be a part.
const ENDINGS: [&str; 5] = ["toml", "json", "obj", "gltf", "glb"];

/// One file of the library.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Its name: the file's name without the ending.
    pub name: String,
    /// The file.
    pub path: PathBuf,
    /// The file as the recipe would write it: from the recipe's folder,
    /// with `/` between folders. `parts/lamp.gltf`.
    pub written: String,
    /// Its small picture. `None` if the file can't be built.
    pub picture: Option<Picture>,
    /// Why it can't be used, if it can't: its first mistake.
    pub problem: Option<String>,
}

/// The library as it was last read.
#[derive(Default)]
pub(super) struct Library {
    /// The folder chosen in place of `parts` next to the recipe.
    chosen: Option<PathBuf>,
    items: Vec<Item>,
    /// Each file's time and size when it was read, to see a change.
    stamps: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    /// How many times it has been read.
    reads: usize,
}

/// The files of a folder that can be parts, in name order.
fn part_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ending| ending.to_str())
                    .is_some_and(|ending| ENDINGS.contains(&ending.to_lowercase().as_str()))
        })
        .collect();
    files.sort();
    files
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// `file` as a recipe in `folder` would write it: the way from that
/// folder to the file, with `/` between the pieces. `None` if there is
/// no way that can be written (another drive, on Windows).
fn written_from(folder: &Path, file: &Path) -> Option<String> {
    let folder = folder.canonicalize().ok()?;
    let file = file.canonicalize().ok()?;
    let (folder, file): (Vec<_>, Vec<_>) =
        (folder.components().collect(), file.components().collect());
    let shared = folder.iter().zip(&file).take_while(|(a, b)| a == b).count();
    if shared == 0 {
        return None;
    }
    let mut pieces: Vec<String> = vec!["..".to_owned(); folder.len() - shared];
    pieces.extend(
        file[shared..]
            .iter()
            .map(|piece| piece.as_os_str().to_string_lossy().into_owned()),
    );
    Some(pieces.join("/"))
}

impl Studio {
    /// The library's folder: the one that was chosen, or `parts` next to
    /// the recipe.
    pub fn library_folder(&self) -> PathBuf {
        self.library
            .chosen
            .clone()
            .unwrap_or_else(|| self.path.parent().unwrap_or(Path::new("")).join("parts"))
    }

    /// Writes a part alone to the library's folder, as `duck-mold
    /// save-part` does: `<its name>.toml` (with a number added if that is
    /// taken). Says where it went, and reads the library again.
    pub fn save_to_library(&mut self, part: &str) -> Result<String, String> {
        let folder = self.library_folder();
        let own = part.rsplit('/').next().unwrap_or(part);
        let base = if own.starts_with('#') { "part" } else { own };
        let mut file = folder.join(format!("{base}.toml"));
        let mut count = 2;
        while file.exists() {
            file = folder.join(format!("{base}_{count}.toml"));
            count += 1;
        }
        let done = tools::save_part(&self.path, part, &file)
            .map(|_| {
                let beside = self.path.parent().unwrap_or(Path::new(""));
                let written =
                    written_from(beside, &file).unwrap_or_else(|| file.display().to_string());
                format!("Saved \"{part}\" to the library: {written}")
            })
            .map_err(|failure| format!("Not saved: {}", self.shorten(&first_error(&failure))));
        self.read_library();
        self.say(match &done {
            Ok(said) | Err(said) => said.clone(),
        });
        done
    }

    /// Uses another folder as the library (`None`: `parts` next to the
    /// recipe again), and reads it.
    pub fn set_library_folder(&mut self, folder: Option<PathBuf>) {
        self.library.chosen = folder;
        self.read_library();
    }

    /// The library's parts, in name order.
    pub fn library(&self) -> &[Item] {
        &self.library.items
    }

    /// How many times the library has been read: it changes when the
    /// pictures have to be shown again.
    pub fn library_reads(&self) -> usize {
        self.library.reads
    }

    /// Reads the library's folder again if a file in it was saved, made
    /// or removed. True if it was read.
    pub fn check_library(&mut self) -> bool {
        let files = part_files(&self.library_folder());
        let same = files.len() == self.library.stamps.len()
            && self
                .library
                .stamps
                .iter()
                .zip(&files)
                .all(|((path, was), file)| path == file && *was == stamp(file));
        if same && self.library.reads > 0 {
            return false;
        }
        self.read_library();
        true
    }

    /// Reads the library's folder: builds each file and draws its small
    /// picture.
    pub(super) fn read_library(&mut self) {
        let folder = self.library_folder();
        let beside = self.path.parent().unwrap_or(Path::new("")).to_path_buf();
        let open = self.path.canonicalize().ok();
        let files = part_files(&folder);
        self.library.stamps = files
            .iter()
            .map(|file| (file.clone(), stamp(file)))
            .collect();
        self.library.reads += 1;
        self.library.items = files
            .into_iter()
            // A recipe can't be a part of itself.
            .filter(|file| file.canonicalize().ok() != open)
            .filter_map(|file| {
                let written = written_from(&beside, &file)?;
                let name = file.file_stem()?.to_string_lossy().into_owned();
                let (picture, problem) = match tools::build(&file) {
                    Ok(built) => (
                        Some(Picture {
                            width: PICTURE_SIZE,
                            height: PICTURE_SIZE,
                            pixels: render::render_small(&built.model, PICTURE_SIZE),
                        }),
                        None,
                    ),
                    Err(failure) => (None, Some(first_error(&failure))),
                };
                Some(Item {
                    name,
                    path: file,
                    written,
                    picture,
                    problem,
                })
            })
            .collect();
    }
}
