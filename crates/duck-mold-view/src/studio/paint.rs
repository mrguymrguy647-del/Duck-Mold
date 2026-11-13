//! Colour: the Paint tool, and the palette's own edits.
//!
//! Painting a face is a `set --face-color` edit; painting a whole part
//! a `set --color`. A face here is one a recipe can name: a box's
//! `top`, a cylinder's `side`, a mesh's material. The palette is
//! changed with `add-color`, `set-color` and `remove-color`.

use std::ops::Range;

use duck_mold_core::edit::{self, Changes, Edit};
use duck_mold_core::source::Syntax;

use super::handles::Tool;
use super::{Studio, viewport};

fn hex_of(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

impl Studio {
    /// The palette colour the Paint tool paints with: the one picked,
    /// or else the palette's first. `None` only when the palette is
    /// empty.
    pub fn paint_colour(&self) -> Option<String> {
        let palette = self.palette();
        self.paint
            .clone()
            .filter(|name| palette.iter().any(|(known, _)| known == name))
            .or_else(|| palette.first().map(|(name, _)| name.clone()))
    }

    /// Picks the colour to paint with, and takes the Paint tool in hand.
    /// `None` puts the colour down.
    pub fn pick_colour(&mut self, name: Option<&str>) {
        self.paint = name.map(str::to_owned);
        if let Some(name) = name {
            self.tool = Tool::Paint;
            self.say(format!(
                "Painting with \"{name}\": click a face to colour it. Shift+click colours the whole part"
            ));
        }
    }

    /// The face under the pointer that a recipe can name: the part it
    /// is on, by path, and the face's name.
    pub fn face_under(&self, size: (usize, usize), at: (f64, f64)) -> Option<(String, String)> {
        let model = self.model()?;
        let hit = viewport::pick(model, &self.orbit, self.offset(), size, at, &self.unseen())?;
        let part = model
            .leaves()
            .into_iter()
            .find(|leaf| leaf.faces.contains(&hit.face))?;
        Some((part.path.clone(), part.face_name(hit.face)?.to_owned()))
    }

    /// The faces the Paint tool would colour where the pointer is: lit
    /// in the picture before the click.
    pub(super) fn paint_aim(&self) -> Vec<Range<usize>> {
        if self.tool != Tool::Paint || self.look_only {
            return Vec::new();
        }
        let Some(pointer) = self.pointer else {
            return Vec::new();
        };
        let Some((path, face)) = self.face_under(self.view, pointer) else {
            return Vec::new();
        };
        self.part(&path)
            .map(|part| part.faces_named(&face))
            .unwrap_or_default()
            .into_iter()
            .map(|at| at..at + 1)
            .collect()
    }

    /// A click with the Paint tool: colours the face under the pointer
    /// with the colour in hand; with `whole` (Shift held), the whole
    /// part, taking away the colours its single faces had.
    pub fn paint_at(
        &mut self,
        size: (usize, usize),
        at: (f64, f64),
        whole: bool,
    ) -> Result<String, String> {
        self.hover(size, Some(at));
        let Some(color) = self.paint_colour() else {
            let why = "There is no colour to paint with: add one to the palette first".to_owned();
            self.say(why.clone());
            return Err(why);
        };
        let Some((part, face)) = self.face_under(size, at) else {
            let why = "Nothing to paint there: click a face of a part".to_owned();
            self.say(why.clone());
            return Err(why);
        };
        let changes = if whole {
            // The faces' own colours go, so the whole part is one colour.
            let faces = Syntax::from_path(&self.path)
                .and_then(|syntax| edit::describe(&self.text, syntax, self.model(), &part))
                .map(|facts| facts.face_colors)
                .unwrap_or_default();
            Changes {
                color: Some(color),
                face_colors: faces
                    .into_iter()
                    .map(|(face, _)| (face, "none".to_owned()))
                    .collect(),
                ..Changes::default()
            }
        } else {
            Changes {
                face_colors: vec![(face, color)],
                ..Changes::default()
            }
        };
        self.apply(&Edit::Set { part, changes })
    }

    /// Adds a colour to the recipe's palette: an `add-color` edit. It
    /// becomes the colour to paint with.
    pub fn add_colour(&mut self, name: &str, rgb: [u8; 3]) -> Result<String, String> {
        let said = self.apply(&Edit::AddColor {
            name: name.to_owned(),
            hex: hex_of(rgb),
        })?;
        self.paint = Some(name.to_owned());
        Ok(said)
    }

    /// Changes a palette colour, everywhere it is used: a `set-color`
    /// edit.
    pub fn change_colour(&mut self, name: &str, rgb: [u8; 3]) -> Result<String, String> {
        self.apply(&Edit::SetColor {
            name: name.to_owned(),
            hex: hex_of(rgb),
        })
    }

    /// Whether anything in the model is painted with a palette colour.
    pub fn colour_in_use(&self, name: &str) -> bool {
        self.model().is_some_and(|model| {
            model
                .palette
                .iter()
                .position(|color| color.name == name)
                .is_some_and(|at| model.faces.iter().any(|face| face.color == Some(at)))
        })
    }

    /// Takes a colour out of the palette: a `remove-color` edit. What
    /// is painted with it is painted with `replace_with` first; without
    /// one, the edit is refused if anything is.
    pub fn remove_colour(
        &mut self,
        name: &str,
        replace_with: Option<&str>,
    ) -> Result<String, String> {
        let said = self.apply(&Edit::RemoveColor {
            name: name.to_owned(),
            replace_with: replace_with.map(str::to_owned),
        })?;
        if self.paint.as_deref() == Some(name) {
            self.paint = replace_with.map(str::to_owned);
        }
        Ok(said)
    }
}
