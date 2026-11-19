//! Making a new image for a part: `duck-mold new-image`, and the
//! Studio's "New image".
//!
//! A new image is a PNG that is see-through all over, with the
//! proportions of what it will cover, so that its pixels come out square
//! on the model, and as many pixels to the metre as the model's other
//! images have, so that all its pixels are one size. It is written
//! beside the recipe (in `textures/`, unless told otherwise) and put on
//! the part, or on one face of it, with an ordinary `set` edit.

use std::path::{Path, PathBuf};

use crate::edit::{self, Changes, Edit, Edited};
use crate::inspect::images_report;
use crate::issue::{Code, Issue};
use crate::load::{display_join, display_path, pretending};
use crate::picture::write_png;
use crate::source::Syntax;
use crate::tools::{self, Failure};

/// How many pixels to the metre a model's first image gets.
pub const PIXELS_PER_METRE: f64 = 64.0;
/// The fewest pixels along the longer side of a model's first image.
pub const SMALLEST_LONG_SIDE: u32 = 32;
/// The most pixels along the longer side of an image made here.
pub const LARGEST_LONG_SIDE: u32 = 256;

/// What a new image would be, before it is made.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagePlan {
    /// The file, as the recipe will name it: `textures/robot_head_front.png`.
    pub file: String,
    /// Where that is on the disk.
    pub path: PathBuf,
    /// The part it goes on, by path.
    pub part: String,
    /// The face it goes on; `None` for the whole part.
    pub face: Option<String>,
    /// How big what it covers is on the model, across and down, in
    /// metres. (For an image on several faces of different sizes: the
    /// biggest.)
    pub metres: [f64; 2],
    /// How many pixels wide and high it will be.
    pub size: [u32; 2],
    /// How many pixels to the metre the model's other images have (the
    /// middle one, when they differ). `None` for a model's first image.
    pub like_the_others: Option<f64>,
}

impl ImagePlan {
    /// What it goes on, in words: `the front of "head"`, or `"head"`.
    pub fn onto(&self) -> String {
        match &self.face {
            Some(face) => format!("the face \"{face}\" of \"{}\"", self.part),
            None => format!("\"{}\"", self.part),
        }
    }

    /// How many of its pixels there are to a metre on the model (the
    /// mean of across and down).
    pub fn pixels_per_metre(&self) -> f64 {
        ((f64::from(self.size[0]) / self.metres[0]) * (f64::from(self.size[1]) / self.metres[1]))
            .sqrt()
    }

    /// The size it has with its longer side `long` pixels, keeping its
    /// proportions: the Studio's Small, Medium, Large and Fine.
    pub fn with_long_side(&self, long: u32) -> [u32; 2] {
        with_long_side(self.metres, long)
    }

    /// The edit that puts it on the part.
    pub fn edit(&self) -> Edit {
        let changes = match &self.face {
            Some(face) => Changes {
                face_images: vec![(face.clone(), self.file.clone())],
                ..Changes::default()
            },
            None => Changes {
                image: Some(self.file.clone()),
                ..Changes::default()
            },
        };
        Edit::Set {
            part: self.part.clone(),
            changes,
        }
    }
}

/// A size in pixels with the proportions of `metres` (across and down)
/// and `long` pixels along its longer side.
pub fn with_long_side(metres: [f64; 2], long: u32) -> [u32; 2] {
    let long = long.max(1);
    let short =
        |short: f64, longer: f64| ((f64::from(long) * short / longer).round() as u32).max(1);
    if metres[0] >= metres[1] {
        [long, short(metres[1], metres[0])]
    } else {
        [short(metres[0], metres[1]), long]
    }
}

/// A sensible size for an image that covers `metres` (across and down):
/// as many pixels to the metre as the model's other images
/// (`like_the_others`), or [`PIXELS_PER_METRE`] for its first, whose
/// longer side is at least [`SMALLEST_LONG_SIDE`]; never more than
/// [`LARGEST_LONG_SIDE`] along a side.
pub fn sensible_size(metres: [f64; 2], like_the_others: Option<f64>) -> [u32; 2] {
    let per_metre = like_the_others.unwrap_or(PIXELS_PER_METRE);
    let longer = metres[0].max(metres[1]);
    let mut long = (longer * per_metre).round().max(1.0) as u32;
    if like_the_others.is_none() {
        long = long.max(SMALLEST_LONG_SIDE);
    }
    with_long_side(metres, long.min(LARGEST_LONG_SIDE))
}

fn usage(message: String) -> Failure {
    Failure::Usage(Box::new(Issue::error(Code::Io, "", message)))
}

/// The name a new image gets when none is given:
/// `textures/<model>_<part>_<face>.png`, with a number after it if that
/// is taken.
fn default_file(recipe: &Path, part: &str, face: Option<&str>) -> String {
    let model = recipe
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_owned());
    let mut name = format!("{model}_{}", part.replace(['/', '#'], "_"));
    if let Some(face) = face {
        name.push('_');
        name.push_str(&face.replace('+', "_"));
    }
    let folder = recipe.parent().unwrap_or(Path::new(""));
    let mut file = format!("textures/{name}.png");
    let mut number = 2;
    while folder.join(&file).exists() {
        file = format!("textures/{name}_{number}.png");
        number += 1;
    }
    file
}

/// Works out what a new image for a part (or, with `face`, for one face
/// of it) would be: its file and its size. Nothing is written.
///
/// `file` names it (a path from the recipe's folder, ending in `.png`);
/// without one it is `textures/<model>_<part>_<face>.png`. Refused, with
/// a sentence, if that file is already there, if the part or the face
/// isn't, or if no face would show the image.
pub fn plan_image(
    recipe: &Path,
    part: &str,
    face: Option<&str>,
    file: Option<&str>,
) -> Result<ImagePlan, Failure> {
    let text = std::fs::read_to_string(recipe).map_err(|error| {
        usage(format!(
            "can't read \"{}\": {}",
            display_path(recipe),
            match error.kind() {
                std::io::ErrorKind::NotFound => "there is no such file",
                _ => "it can't be opened",
            }
        ))
    })?;
    let Some(syntax) = Syntax::from_path(recipe) else {
        return Err(usage(format!(
            "\"{}\" isn't a recipe: its name must end in .toml or .json",
            display_path(recipe)
        )));
    };
    let display = display_path(recipe);
    let file = match file {
        Some(file) => file.replace('\\', "/"),
        None => default_file(recipe, part, face),
    };
    let path = recipe.parent().unwrap_or(Path::new("")).join(&file);
    if path.exists() {
        let flag = match face {
            Some(face) => format!("--face-image {face}={file}"),
            None => format!("--image {file}"),
        };
        return Err(usage(format!(
            "\"{file}\" is already there: a new image never writes over a file. To use it as it is: duck-mold edit {display} set {part} {flag}"
        )));
    }

    // How many pixels to the metre the model's images have now.
    let before = tools::build(recipe)?;
    let mut others: Vec<f64> = images_report(&before.model)
        .iter()
        .flat_map(|images| images.files.iter())
        .flat_map(|file| file.on.iter().map(|on| on.pixels_per_metre))
        .collect();
    others.sort_by(f64::total_cmp);
    let like_the_others = others.get(others.len().saturating_sub(1) / 2).copied();

    // What the image would cover: the recipe as the edit would leave
    // it, built with a one-pixel image where the new one will be.
    let mut plan = ImagePlan {
        file: file.clone(),
        path: path.clone(),
        part: part.to_owned(),
        face: face.map(str::to_owned),
        metres: [1.0, 1.0],
        size: [1, 1],
        like_the_others,
    };
    let one_pixel = write_png(1, 1, &[[0, 0, 0, 0]], false);
    let applied = pretending(&path, one_pixel, || {
        edit::apply(&text, syntax, recipe, &display, &plan.edit())
    })?;
    let shown = display_join(&display, &file);
    let covered = applied
        .model
        .as_ref()
        .and_then(images_report)
        .and_then(|images| {
            images
                .files
                .into_iter()
                .find(|known| known.file == shown)
                .and_then(|known| {
                    known
                        .on
                        .into_iter()
                        .map(|on| on.metres)
                        .max_by(|a, b| (a[0] * a[1]).total_cmp(&(b[0] * b[1])))
                })
        });
    let Some(metres) = covered.filter(|metres| metres[0] > 0.0 && metres[1] > 0.0) else {
        return Err(usage(format!(
            "no face of {} would show an image there (bevelled edges and parts from mesh files without a layout can't take one)",
            plan.onto()
        )));
    };
    plan.metres = metres;
    plan.size = sensible_size(metres, like_the_others);
    Ok(plan)
}

/// What [`new_image`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct NewImage {
    /// The image that was made.
    pub plan: ImagePlan,
    /// The edit that put it on the part.
    pub edited: Edited,
}

impl NewImage {
    /// One sentence: what was made and where it went.
    pub fn summary(&self) -> String {
        format!(
            "made {} ({} x {} pixels, see-through, {} to the metre) and put it on {}",
            self.plan.file,
            self.plan.size[0],
            self.plan.size[1],
            self.plan.pixels_per_metre().round(),
            self.plan.onto()
        )
    }
}

/// `duck-mold new-image`: makes a new, wholly see-through PNG for a part
/// (or, with `face`, one face of it) and puts it on with a `set` edit.
///
/// `size` is how many pixels wide and high; without it, the size
/// [`plan_image`] works out. If the edit can't be made, the image file
/// is taken away again and nothing has changed.
pub fn new_image(
    recipe: &Path,
    part: &str,
    face: Option<&str>,
    file: Option<&str>,
    size: Option<[u32; 2]>,
) -> Result<NewImage, Failure> {
    let mut plan = plan_image(recipe, part, face, file)?;
    if let Some([wide, high]) = size {
        let most = crate::picture::MAX_SIDE;
        if wide == 0 || high == 0 || wide > most || high > most {
            return Err(usage(format!(
                "an image is 1 x 1 to {most} x {most} pixels, not {wide} x {high}"
            )));
        }
        plan.size = [wide, high];
    }
    if let Some(folder) = plan
        .path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty())
    {
        std::fs::create_dir_all(folder).map_err(|error| {
            usage(format!(
                "can't make the folder \"{}\": {error}",
                display_path(folder)
            ))
        })?;
    }
    let count = (plan.size[0] as usize) * (plan.size[1] as usize);
    let png = write_png(
        plan.size[0],
        plan.size[1],
        &vec![[0, 0, 0, 0]; count],
        false,
    );
    tools::write_file(&plan.path, &png)?;
    match edit::edit_file(recipe, &plan.edit(), false) {
        Ok(edited) => Ok(NewImage { plan, edited }),
        Err(failure) => {
            let _ = std::fs::remove_file(&plan.path);
            Err(failure)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_image_has_the_faces_proportions_and_at_least_32_pixels() {
        // The robot's head front, 42 x 32 cm: 27 x 20 at 64 to the
        // metre, so its longer side is brought up to 32.
        assert_eq!(sensible_size([0.42, 0.32], None), [32, 24]);
        // A one-metre crate face.
        assert_eq!(sensible_size([1.0, 1.0], None), [64, 64]);
        // A wall: never more than 256 along a side.
        assert_eq!(sensible_size([6.0, 3.0], None), [256, 128]);
        // Taller than wide.
        assert_eq!(sensible_size([0.5, 2.0], None), [32, 128]);
    }

    #[test]
    fn a_later_image_has_as_many_pixels_to_the_metre_as_the_others() {
        // 32 to the metre, like the plank crate.
        assert_eq!(sensible_size([1.0, 1.0], Some(32.0)), [32, 32]);
        assert_eq!(sensible_size([0.5, 0.25], Some(32.0)), [16, 8]);
        // Small, but the same pixel size: not brought up to 32.
        assert_eq!(sensible_size([0.1, 0.1], Some(32.0)), [3, 3]);
        assert_eq!(sensible_size([100.0, 1.0], Some(32.0)), [256, 3]);
    }

    #[test]
    fn a_long_side_keeps_the_proportions() {
        assert_eq!(with_long_side([0.42, 0.32], 64), [64, 49]);
        assert_eq!(with_long_side([0.32, 0.42], 16), [12, 16]);
        assert_eq!(with_long_side([1.0, 0.001], 16), [16, 1]);
    }
}
