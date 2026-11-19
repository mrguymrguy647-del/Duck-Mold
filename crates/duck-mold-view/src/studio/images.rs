//! Images: the ones a model has, the Paint tool painting with one, and
//! making or choosing one for a part or a face.
//!
//! Putting an image on a face is a `set --face-image` edit; on a whole
//! part, a `set --image`. A new image is `duck-mold new-image`: a PNG
//! that is see-through all over, written beside the recipe.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use duck_mold_core::edit::{self, Changes, Edit};
use duck_mold_core::images::{self, ImagePlan};
use duck_mold_core::picture::Picture;
use duck_mold_core::recipe::NO_IMAGE;
use duck_mold_core::source::Syntax;
use duck_mold_core::tools;

use super::Studio;
use super::handles::Tool;

/// One of the model's images, for the palette's row of them.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// The file as a recipe names it: its path from the recipe's
    /// folder, `textures/robot_face.png`.
    pub file: String,
    /// Where it is on the disk.
    pub path: PathBuf,
    /// Its pixels.
    pub picture: Arc<Picture>,
}

impl ImageInfo {
    /// Its name without its folders: `robot_face.png`.
    pub fn name(&self) -> &str {
        self.file.rsplit('/').next().unwrap_or(&self.file)
    }
}

/// What the Paint tool paints with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Brush {
    /// A palette colour, by name.
    Colour(String),
    /// An image, by its path from the recipe's folder.
    Image(String),
    /// "No image": takes a face's image off.
    NoImage,
}

/// The path of `file` (as messages show it) from the folder of `recipe`
/// (as messages show it): what a recipe writes to name it.
fn from_recipe(recipe: &str, file: &str) -> String {
    let recipe = recipe.replace('\\', "/");
    let mut folder: Vec<&str> = recipe.split('/').collect();
    folder.pop();
    let pieces: Vec<&str> = file.split('/').collect();
    let shared = folder
        .iter()
        .zip(&pieces)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<&str> = vec![".."; folder.len() - shared];
    out.extend(&pieces[shared..]);
    out.join("/")
}

/// A picture that a part's mesh file brought with it, on one of its
/// materials.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnImage {
    /// The material, which is the face's name.
    pub face: String,
    /// The picture.
    pub picture: Arc<Picture>,
}

/// Whether a picture of the model is a file a recipe can name as an
/// image: a PNG on the disk, not one from inside a mesh file.
fn is_recipe_image(file: &str) -> bool {
    file.to_lowercase().ends_with(".png") && !file.contains(": ")
}

impl Studio {
    /// The model's images, in the order the recipe first names them.
    pub fn images(&self) -> Vec<ImageInfo> {
        let Some(model) = self.model() else {
            return Vec::new();
        };
        let recipe = duck_mold_core::load::display_path(&self.path);
        let folder = self.path.parent().unwrap_or(Path::new(""));
        model
            .pictures
            .iter()
            // A picture that came inside a mesh file, or as a JPEG
            // beside one, isn't a file a recipe can name.
            .filter(|picture| is_recipe_image(&picture.file))
            .map(|picture| {
                let file = from_recipe(&recipe, &picture.file);
                ImageInfo {
                    path: folder.join(&file),
                    file,
                    picture: picture.clone(),
                }
            })
            .collect()
    }

    /// The pictures a part made from a mesh file shows that the file
    /// brought with it: each material that shows one, in the file's
    /// order, with its picture. Empty for any other part. `said` is
    /// what the recipe says about the part.
    pub fn own_images(&self, part: &str, said: &edit::Facts) -> Vec<OwnImage> {
        let Some(model) = self.model() else {
            return Vec::new();
        };
        let Some(part) = model.part(part) else {
            return Vec::new();
        };
        if part.kind.shape != "mesh" {
            return Vec::new();
        }
        // What the recipe gives an image (or "none") isn't the file's.
        let recipe_says = |face: &str| {
            said.image.is_some() || said.face_images.iter().any(|(named, _)| named == face)
        };
        let mut own: Vec<OwnImage> = Vec::new();
        for number in part.faces.clone() {
            let Some(image) = &model.faces[number].image else {
                continue;
            };
            let Some(face) = part.face_name(number) else {
                continue;
            };
            if own.iter().any(|known| known.face == face) || recipe_says(face) {
                continue;
            }
            own.push(OwnImage {
                face: face.to_owned(),
                picture: model.pictures[image.picture].clone(),
            });
        }
        own
    }

    /// What the Paint tool paints with: the image picked, "no image", or
    /// else the colour picked (the palette's first, if none was).
    pub fn brush(&self) -> Option<Brush> {
        match &self.paint_image {
            Some(Some(file)) if self.images().iter().any(|image| image.file == *file) => {
                Some(Brush::Image(file.clone()))
            }
            Some(None) => Some(Brush::NoImage),
            _ => self.paint_colour().map(Brush::Colour),
        }
    }

    /// Picks an image to paint with (by its path from the recipe's
    /// folder), and takes the Paint tool in hand. `None` picks "no
    /// image": a click then takes a face's image off.
    pub fn pick_image(&mut self, file: Option<&str>) {
        self.paint_image = Some(file.map(str::to_owned));
        self.tool = Tool::Paint;
        self.say(match file {
            Some(file) => format!(
                "Painting with {}: click a face to put it there. Shift+click puts it on the whole part",
                file.rsplit('/').next().unwrap_or(file)
            ),
            None => {
                "Taking images off: click a face to take its image off. Shift+click takes the whole part's off".to_owned()
            }
        });
    }

    /// Puts the image brush down: the Paint tool paints colours again.
    pub fn drop_image(&mut self) {
        self.paint_image = None;
    }

    /// What a click with the Paint tool would do where the pointer is,
    /// in words, for the status bar: `front of head: put robot_face.png
    /// on it`.
    pub fn paint_hint(&self, whole: bool) -> Option<String> {
        if self.tool != Tool::Paint || self.look_only {
            return None;
        }
        let (part, face) = self.face_under(self.view, self.pointer?)?;
        let place = if whole {
            format!("all of {part}")
        } else {
            format!("{face} of {part}")
        };
        Some(match self.brush()? {
            Brush::Colour(name) => format!("{place}: paint it {name}"),
            Brush::Image(file) => format!(
                "{place}: put {} on it",
                file.rsplit('/').next().unwrap_or(&file)
            ),
            Brush::NoImage => format!("{place}: take its image off"),
        })
    }

    /// The edit that gives a part (or, with `face`, one face of it) an
    /// image, or with [`NO_IMAGE`] takes it off. For the whole part, the
    /// images its single faces had go too, so the part shows one image.
    fn image_edit(&self, part: &str, face: Option<&str>, file: &str) -> Edit {
        let changes = match face {
            Some(face) => Changes {
                face_images: vec![(face.to_owned(), file.to_owned())],
                ..Changes::default()
            },
            None => {
                let own = Syntax::from_path(&self.path)
                    .and_then(|syntax| edit::describe(&self.text, syntax, self.model(), part))
                    .map(|facts| facts.face_images)
                    .unwrap_or_default();
                Changes {
                    image: Some(file.to_owned()),
                    face_images: own
                        .into_iter()
                        .map(|(face, _)| (face, NO_IMAGE.to_owned()))
                        .collect(),
                    ..Changes::default()
                }
            }
        };
        Edit::Set {
            part: part.to_owned(),
            changes,
        }
    }

    /// Gives a part, or one face of it, an image the recipe can name
    /// (a path from its folder), or takes its image off (`None`).
    pub fn set_image(
        &mut self,
        part: &str,
        face: Option<&str>,
        file: Option<&str>,
    ) -> Result<String, String> {
        let edit = self.image_edit(part, face, file.unwrap_or(NO_IMAGE));
        self.apply(&edit)
    }

    /// A click with the Paint tool while an image (or "no image") is
    /// picked: see [`Studio::paint_at`].
    pub(super) fn paint_image_at(
        &mut self,
        part: String,
        face: String,
        whole: bool,
        file: Option<String>,
    ) -> Result<String, String> {
        self.set_image(&part, (!whole).then_some(face.as_str()), file.as_deref())
    }

    /// What a new image for a part (or one face of it) would be: its
    /// file and its size. Nothing is written.
    pub fn plan_image(&self, part: &str, face: Option<&str>) -> Result<ImagePlan, String> {
        images::plan_image(&self.path, part, face, None)
            .map_err(|failure| self.shorten(&super::first_error(&failure)))
    }

    /// Makes a new image (`duck-mold new-image`): a see-through PNG,
    /// `size` pixels wide and high, written where the plan says, and
    /// put on the part with an edit. If the edit can't be made, the
    /// file is taken away again. Says what was made.
    pub fn new_image(&mut self, plan: &ImagePlan, size: [u32; 2]) -> Result<String, String> {
        if self.look_only {
            return Err("This model is only being looked at".to_owned());
        }
        let most = duck_mold_core::picture::MAX_SIDE;
        if size[0] == 0 || size[1] == 0 || size[0] > most || size[1] > most {
            return Err(format!(
                "An image is 1 x 1 to {most} x {most} pixels, not {} x {}",
                size[0], size[1]
            ));
        }
        if plan.path.exists() {
            return Err(format!("{} is already there", plan.file));
        }
        if let Some(folder) = plan.path.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|error| format!("Can't make the folder for {}: {error}", plan.file))?;
        }
        let count = (size[0] as usize) * (size[1] as usize);
        let png =
            duck_mold_core::picture::write_png(size[0], size[1], &vec![[0, 0, 0, 0]; count], false);
        tools::write_file(&plan.path, &png)
            .map_err(|failure| self.shorten(&super::first_error(&failure)))?;
        match self.apply(&plan.edit()) {
            Ok(_) => {
                let said = format!(
                    "Made {} ({} x {} pixels, see-through) and put it on {}",
                    plan.file,
                    size[0],
                    size[1],
                    plan.onto()
                );
                self.say(said.clone());
                self.written = Some(plan.path.clone());
                Ok(said)
            }
            Err(why) => {
                let _ = std::fs::remove_file(&plan.path);
                Err(why)
            }
        }
    }

    /// Puts a PNG the person chose on a part (or one face of it). A
    /// file inside the recipe's folder is named where it is; one from
    /// anywhere else is copied into `textures/` beside the recipe first
    /// (under another name, if that one is taken by a different file),
    /// so the recipe can be moved with its folder. Says what was done.
    pub fn use_image(
        &mut self,
        part: &str,
        face: Option<&str>,
        chosen: &Path,
    ) -> Result<String, String> {
        if self.look_only {
            return Err("This model is only being looked at".to_owned());
        }
        let bytes =
            std::fs::read(chosen).map_err(|_| format!("Can't read {}", chosen.display()))?;
        // Checked before anything is copied: a clear sentence here
        // rather than a mistake in the recipe.
        let name = chosen
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image.png".to_owned());
        if let Err(bad) = duck_mold_core::picture::read_png(&bytes, &name) {
            return Err(format!("{name} {}. {}", bad.what, bad.fix));
        }
        let folder = self.path.parent().unwrap_or(Path::new("")).to_path_buf();
        let whole =
            |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let inside = whole(chosen)
            .strip_prefix(whole(&folder))
            .ok()
            .map(|within| within.to_string_lossy().replace('\\', "/"));
        let (file, copied) = match inside {
            Some(file) => (file, false),
            None => {
                let stem = Path::new(&name)
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "image".to_owned());
                let mut file = format!("textures/{stem}.png");
                let mut number = 2;
                // The same picture already there is used as it is.
                loop {
                    match std::fs::read(folder.join(&file)) {
                        Ok(there) if there == bytes => break (file, false),
                        Ok(_) => {
                            file = format!("textures/{stem}_{number}.png");
                            number += 1;
                        }
                        Err(_) => {
                            std::fs::create_dir_all(folder.join("textures")).map_err(|error| {
                                format!("Can't make the textures folder: {error}")
                            })?;
                            tools::write_file(&folder.join(&file), &bytes)
                                .map_err(|failure| self.shorten(&super::first_error(&failure)))?;
                            break (file, true);
                        }
                    }
                }
            }
        };
        match self.set_image(part, face, Some(&file)) {
            Ok(said) => {
                if copied {
                    let said = format!(
                        "Copied {name} to {file}, beside the recipe, and put it on {}",
                        match face {
                            Some(face) => format!("{face} of {part}"),
                            None => part.to_owned(),
                        }
                    );
                    self.say(said.clone());
                    Ok(said)
                } else {
                    Ok(said)
                }
            }
            Err(why) => {
                if copied {
                    let _ = std::fs::remove_file(folder.join(&file));
                }
                Err(why)
            }
        }
    }
}

/// The image a part, or one face of it, shows, with what is needed to
/// paint it in a picture program.
#[derive(Debug, Clone)]
pub struct Shown {
    /// The image.
    pub image: ImageInfo,
    /// The model's faces that show it on this part (their numbers).
    pub faces: Vec<usize>,
    /// The colour under it: the face's palette colour, or the plain
    /// grey of a face nobody coloured.
    pub under: [u8; 3],
}

impl Studio {
    /// The image a part shows (`face`: on that face; `None`: the first
    /// image any of its faces has), with the faces of the part that
    /// show it. `None` if there is none.
    pub fn image_shown(&self, part: &str, face: Option<&str>) -> Option<Shown> {
        let model = self.model()?;
        let found = model.part(part)?;
        let candidates: Vec<usize> = match face {
            Some(face) => found.faces_named(face),
            None => found.faces.clone().collect(),
        };
        let images = self.images();
        // For the whole part: the part's own image if it has one (its
        // `image` line), rather than that of whichever face comes first.
        let own = match face {
            Some(_) => None,
            None => Syntax::from_path(&self.path)
                .and_then(|syntax| edit::describe(&self.text, syntax, self.model(), part))
                .and_then(|facts| facts.image)
                .and_then(|file| images.iter().position(|image| image.file == file)),
        };
        let shows = |number: usize| {
            model.faces[number]
                .image
                .as_ref()
                .map(|image| image.picture)
        };
        let first = candidates
            .iter()
            .copied()
            .find(|&number| own.is_some() && shows(number) == own)
            .or_else(|| {
                candidates
                    .iter()
                    .copied()
                    .find(|&number| shows(number).is_some())
            })?;
        let picture = shows(first)?;
        let image = images.into_iter().nth(picture)?;
        // Every face of the part with that image: what the guide shows.
        let faces: Vec<usize> = found
            .faces
            .clone()
            .filter(|&number| {
                model.faces[number]
                    .image
                    .as_ref()
                    .is_some_and(|image| image.picture == picture)
            })
            .collect();
        let under = model.faces[first]
            .color
            .and_then(|color| model.palette.get(color))
            .map_or(duck_mold_core::picture::NO_COLOR, |color| color.rgb);
        Some(Shown {
            image,
            faces,
            under,
        })
    }

    /// The guide for the image a part (or one face of it) shows, as the
    /// bytes of a PNG: thin lines the shape of the faces it covers. See
    /// [`crate::quill::guide_png`].
    pub fn guide(&self, part: &str, face: Option<&str>) -> Option<Vec<u8>> {
        let shown = self.image_shown(part, face)?;
        let model = self.model()?;
        crate::quill::guide_png(
            model,
            model.part(part)?,
            &shown.faces,
            [shown.image.picture.width, shown.image.picture.height],
        )
    }

    /// Gets the image a part (or one face of it) shows ready to be
    /// painted in a picture program: writes its guide into `guides` (a
    /// folder of Duck Mold's own, never beside the recipe) and gives the
    /// job to start. Nothing is started here.
    pub fn paint_job(
        &self,
        part: &str,
        face: Option<&str>,
        with: crate::quill::With,
        guides: &Path,
    ) -> Result<crate::quill::PaintJob, String> {
        let Some(shown) = self.image_shown(part, face) else {
            return Err(match face {
                Some(face) => format!("{face} of {part} has no image yet: make one first"),
                None => format!("{part} has no image yet: make one first"),
            });
        };
        // The guide is only for Duck Quill, which draws it over the
        // picture; any other program gets the image alone.
        let guide = match (&with, self.guide(part, face)) {
            (crate::quill::With::DuckQuill(_), Some(png)) => {
                let name = shown
                    .image
                    .path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "image".to_owned());
                let file = guides.join(format!("{name}.guide.png"));
                std::fs::create_dir_all(guides)
                    .and_then(|()| std::fs::write(&file, png))
                    .ok()
                    .map(|()| file)
            }
            _ => None,
        };
        Ok(crate::quill::PaintJob {
            image: shown.image.path.clone(),
            guide,
            paper: shown.under,
            with,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_images_path_is_from_the_recipes_folder() {
        assert_eq!(
            from_recipe("examples/robot.toml", "examples/textures/face.png"),
            "textures/face.png"
        );
        assert_eq!(
            from_recipe("robot.toml", "textures/face.png"),
            "textures/face.png"
        );
        assert_eq!(
            from_recipe("game/art/robot.toml", "game/shared/face.png"),
            "../shared/face.png"
        );
        assert_eq!(
            from_recipe("C:\\models\\robot.toml", "C:/models/textures/face.png"),
            "textures/face.png"
        );
    }
}
