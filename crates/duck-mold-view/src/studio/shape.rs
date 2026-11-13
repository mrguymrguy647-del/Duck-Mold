//! Shaping the selected box by hand: Face mode.
//!
//! In Part mode (what there always was) a click picks a part and the
//! handles move it. In Face mode the selected box is shaped: a click
//! picks one of its faces, by the name the recipe calls it, and the
//! handles change the shape itself. Every change is an edit an agent
//! can run too: a drag of the Pull arrow is `pull`, of the Inset knob
//! `inset`; the Changes list in the panel is `set-change` and
//! `remove-change`.
//!
//! The other parts are drawn faded, and the faces of the box drawn with
//! their outlines, so it is clear what a click picks.

use std::ops::Range;

use duck_mold_core::math::Vec3;
use duck_mold_core::model::Part;
use duck_mold_core::shaping::Shaped;

use super::viewport::Mark;
use super::{Studio, part_in};

/// What a click picks, and what the handles change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Parts: a click selects a part, the handles move, turn and size
    /// it.
    Part,
    /// The faces of the selected box: a click selects a face, the
    /// handles shape the box.
    Face,
    /// The edges of the selected box: a click selects an edge, the
    /// arrows move it.
    Edge,
    /// The corners of the selected box: a click selects a corner, the
    /// arrows move it.
    Corner,
}

/// The tool in hand in Face mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceTool {
    /// One arrow out of the faces: pull them out or push them in (T).
    Pull,
    /// Three arrows: slide the faces; the faces round them stretch (G).
    Move,
    /// Three rings: turn the faces about their middle (R).
    Turn,
    /// Three boxes and one in the middle: make the faces bigger or
    /// smaller (S).
    Scale,
    /// A knob on an edge: drag it in to make a smaller face inside (I).
    Inset,
    /// A ring round the part, across the edge under the pointer: a
    /// click cuts there (K).
    Cut,
}

/// The faces' outlines.
const OUTLINE: [u8; 3] = [0xB8, 0xBE, 0xCC];
/// Duck yellow: the faces selected.
const CHOSEN: [u8; 3] = [0xFF, 0xCC, 0x2E];

impl Studio {
    /// What a click picks now: parts, or the faces of the selected box.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Why the selected part can't be shaped, if it can't: only a box,
    /// without a bevel, in a recipe that can be edited.
    pub fn why_not_shaped(&self) -> Option<String> {
        if self.look_only {
            return Some("Looking only: nothing can be shaped here".to_owned());
        }
        let Some(path) = self.selected.as_deref() else {
            return Some("Select a box first: Face mode shapes the selected part".to_owned());
        };
        if !self.also.is_empty() {
            return Some("Select one part to shape: Face mode shapes one at a time".to_owned());
        }
        let part = self.part(path)?;
        let Some(facts) = self.facts() else {
            return Some(format!(
                "\"{path}\" is inside another file: open that file to shape it"
            ));
        };
        if facts.shape.as_deref() != Some("box") {
            let what = match (&facts.shape, part.kind.shape.as_str()) {
                (Some(shape), _) => format!("is {}", a_shape(shape)),
                (None, "assembly") => "is made of other parts".to_owned(),
                (None, _) => "comes from a file".to_owned(),
            };
            return Some(format!(
                "Only a box can be shaped in this version: \"{path}\" {what}"
            ));
        }
        if facts.sizes.bevel.is_some_and(|bevel| bevel > 0.0) {
            return Some(format!(
                "A bevelled box can't be shaped in this version: set the bevel of \"{path}\" to 0 first"
            ));
        }
        None
    }

    /// Goes into Face mode, to shape the selected box (Tab). Says why
    /// not if it can't be shaped.
    pub fn face_mode(&mut self) -> Result<String, String> {
        if let Some(why) = self.why_not_shaped() {
            self.say(why.clone());
            return Err(why);
        }
        self.cancel_drag();
        self.mode = Mode::Face;
        self.faces.clear();
        self.face_tool = FaceTool::Pull;
        let said = format!(
            "Shaping \"{}\": click a face, then drag its arrow to pull it out or push it in. Tab goes back to parts",
            self.selected.as_deref().unwrap_or_default()
        );
        self.say(said.clone());
        Ok(said)
    }

    /// Back to Part mode.
    pub fn part_mode(&mut self) {
        if self.mode == Mode::Part {
            return;
        }
        self.cancel_drag();
        self.mode = Mode::Part;
        self.faces.clear();
        self.say("Back to parts: a click selects a part".to_owned());
    }

    /// Tab: into Face mode, or back to Part mode.
    pub fn toggle_face_mode(&mut self) -> Result<String, String> {
        match self.mode {
            Mode::Part | Mode::Edge | Mode::Corner => self.face_mode(),
            Mode::Face => {
                self.part_mode();
                Ok(self.status().to_owned())
            }
        }
    }

    /// Goes into Edge mode, to move the selected box's edges, or cut it
    /// (E). Says why not if it can't be shaped.
    pub fn edge_mode(&mut self) -> Result<String, String> {
        if let Some(why) = self.why_not_shaped() {
            self.say(why.clone());
            return Err(why);
        }
        self.cancel_drag();
        self.mode = Mode::Edge;
        self.faces.clear();
        if !matches!(self.face_tool, FaceTool::Move | FaceTool::Cut) {
            self.face_tool = FaceTool::Move;
        }
        let said = format!(
            "The edges of \"{}\": click one, then drag its arrows. K cuts round the part. E goes back to parts",
            self.selected.as_deref().unwrap_or_default()
        );
        self.say(said.clone());
        Ok(said)
    }

    /// Goes into Corner mode, to move the selected box's corners (C).
    /// Says why not if it can't be shaped.
    pub fn corner_mode(&mut self) -> Result<String, String> {
        if let Some(why) = self.why_not_shaped() {
            self.say(why.clone());
            return Err(why);
        }
        self.cancel_drag();
        self.mode = Mode::Corner;
        self.faces.clear();
        self.face_tool = FaceTool::Move;
        let said = format!(
            "The corners of \"{}\": click one, then drag its arrows. C goes back to parts",
            self.selected.as_deref().unwrap_or_default()
        );
        self.say(said.clone());
        Ok(said)
    }

    /// C: into Corner mode, or back to Part mode.
    pub fn toggle_corner_mode(&mut self) -> Result<String, String> {
        match self.mode {
            Mode::Corner => {
                self.part_mode();
                Ok(self.status().to_owned())
            }
            _ => self.corner_mode(),
        }
    }

    /// E: into Edge mode, or back to Part mode.
    pub fn toggle_edge_mode(&mut self) -> Result<String, String> {
        match self.mode {
            Mode::Edge => {
                self.part_mode();
                Ok(self.status().to_owned())
            }
            _ => self.edge_mode(),
        }
    }

    /// The faces selected, by their own names; in Edge mode, the edges
    /// (`top+front`).
    pub fn selected_faces(&self) -> &[String] {
        &self.faces
    }

    /// Selects a face of the box by name, or none. With `adding`, the
    /// face joins the faces selected, or leaves them if it is one.
    pub fn select_face(&mut self, name: Option<&str>, adding: bool) {
        match name {
            None => {
                if !adding {
                    self.faces.clear();
                }
            }
            Some(name) if adding => {
                if let Some(at) = self.faces.iter().position(|known| known == name) {
                    self.faces.remove(at);
                } else {
                    self.faces.push(name.to_owned());
                }
            }
            Some(name) => self.faces = vec![name.to_owned()],
        }
    }

    /// The selected part as it is shown, and its shape with its faces,
    /// edges and corners: the shape its changes made, or the plain box.
    pub fn shaped_selected(&self) -> Option<(&Part, Shaped)> {
        if self.mode == Mode::Part {
            return None;
        }
        let path = self.selected.as_deref()?;
        let part = part_in(&self.shown_model()?.parts, path)?;
        let shaped = match &part.shaped {
            Some(shaped) => (**shaped).clone(),
            None => {
                let sizes = self.facts()?.sizes;
                Shaped::boxed(
                    sizes.size.unwrap_or([1.0; 3]),
                    sizes.taper.unwrap_or([1.0; 2]),
                )
            }
        };
        Some((part, shaped))
    }

    /// The name of the face of the box being shaped under the pointer.
    pub fn face_at(&self, size: (usize, usize), at: (f64, f64)) -> Option<String> {
        if self.mode != Mode::Face {
            return None;
        }
        let (path, face) = self.face_under(size, at)?;
        (Some(path.as_str()) == self.selected.as_deref()).then_some(face)
    }

    /// A click in Face mode: selects the face under the pointer (with
    /// `adding`, adds it or takes it out). A click off the box lets go of
    /// the faces. Gives the face clicked.
    pub fn click_face(
        &mut self,
        size: (usize, usize),
        at: (f64, f64),
        adding: bool,
    ) -> Option<String> {
        let edges = self.mode != Mode::Face;
        let face = match self.mode {
            Mode::Edge => self.edge_at(size, at),
            Mode::Corner => self.corner_at(size, at),
            _ => self.face_at(size, at),
        };
        self.select_face(face.as_deref(), adding);
        if let Some(face) = &face {
            let said = match (self.faces.len(), edges) {
                (0, true) => "No edge selected".to_owned(),
                (0, false) => "No face selected".to_owned(),
                (1, true) if self.mode == Mode::Corner => {
                    format!("The corner \"{face}\": drag its arrows to move it")
                }
                (1, true) => format!("The edge \"{face}\": drag its arrows to move it"),
                (1, false) => format!("The face \"{face}\": drag its arrow to pull it"),
                (count, true) => format!("{count} edges: they move together"),
                (count, false) => format!("{count} faces: they are pulled together"),
            };
            self.say(said);
        }
        face
    }

    /// Escape, one step at a time: a held handle, then the faces (or
    /// edges), then Face (or Edge) mode, then the selection. False when there was nothing to
    /// let go of.
    pub fn escape(&mut self) -> bool {
        if self.grab.is_some() {
            self.cancel_drag();
            return true;
        }
        if self.mode != Mode::Part {
            if self.faces.is_empty() {
                self.part_mode();
            } else {
                self.faces.clear();
            }
            return true;
        }
        if self.selected.is_some() {
            self.select(None);
            return true;
        }
        false
    }

    /// Keeps Face mode right after the file was read again: out of it if
    /// the part can't be shaped any more, and only faces that are still
    /// there selected.
    pub(super) fn keep_face_mode(&mut self) {
        if self.mode == Mode::Part {
            return;
        }
        if self.why_not_shaped().is_some() {
            self.mode = Mode::Part;
            self.faces.clear();
            return;
        }
        if let Some((_, shaped)) = self.shaped_selected() {
            if self.mode == Mode::Face {
                let names = shaped.own_names();
                self.faces.retain(|face| names.contains(face));
            } else {
                self.faces
                    .retain(|edge| !shaped.corners_named_or_none(edge).is_empty());
            }
        }
    }

    /// The model's faces of the box being shaped that are named `name`.
    fn model_faces(&self, name: &str) -> Vec<Range<usize>> {
        let Some(path) = self.selected.as_deref() else {
            return Vec::new();
        };
        self.shown_model()
            .and_then(|model| part_in(&model.parts, path))
            .map(|part| part.faces_named(name))
            .unwrap_or_default()
            .into_iter()
            .map(|at| at..at + 1)
            .collect()
    }

    /// The faces to brighten in Face mode: those selected, and the one
    /// under the pointer.
    pub(super) fn face_lit(&self) -> Vec<Range<usize>> {
        if self.mode != Mode::Face {
            return Vec::new();
        }
        let mut lit: Vec<Range<usize>> = self
            .faces
            .iter()
            .flat_map(|face| self.model_faces(face))
            .collect();
        if self.grab.is_none()
            && let Some(pointer) = self.pointer
            && let Some(face) = self.face_at(self.view, pointer)
        {
            lit.extend(self.model_faces(&face));
        }
        lit
    }

    /// The faces of the part that aren't faded in Face mode: the box's.
    pub(super) fn face_focus(&self) -> Option<Range<usize>> {
        if self.mode == Mode::Part {
            return None;
        }
        let path = self.selected.as_deref()?;
        part_in(&self.shown_model()?.parts, path).map(|part| part.faces.clone())
    }

    /// The outlines of the box's faces, drawn over it: thin for every
    /// face that looks this way, thick and yellow for those selected.
    pub(super) fn face_marks(&self) -> Vec<Mark> {
        let mut marks = match self.mode {
            Mode::Edge => self.edge_marks(),
            Mode::Corner => self.corner_marks(),
            _ => self.face_outlines(),
        };
        marks.extend(self.cut_marks());
        marks
    }

    /// Face mode's outlines of the faces.
    fn face_outlines(&self) -> Vec<Mark> {
        let Some((part, _)) = self.shaped_selected() else {
            return Vec::new();
        };
        let Some(model) = self.shown_model() else {
            return Vec::new();
        };
        let offset = self.offset();
        let eye = self.orbit.eye();
        let chosen: Vec<usize> = self
            .faces
            .iter()
            .flat_map(|name| part.faces_named(name))
            .collect();
        let mut thin = Vec::new();
        let mut thick = Vec::new();
        for at in part.faces.clone() {
            let face = &model.faces[at];
            let is_chosen = chosen.contains(&at);
            let looks_here = face
                .normal()
                .is_some_and(|normal| normal.dot(eye - (face.corners[0] + offset)) > 0.0);
            if !looks_here && !is_chosen {
                continue;
            }
            for (step, &from) in face.corners.iter().enumerate() {
                let to = face.corners[(step + 1) % face.corners.len()];
                let (from, to) = (from + offset, to + offset);
                if is_chosen {
                    thick.push(Mark::Line {
                        from,
                        to,
                        color: CHOSEN,
                    });
                } else {
                    thin.push(Mark::Thin {
                        from,
                        to,
                        color: OUTLINE,
                    });
                }
            }
        }
        thin.extend(thick);
        thin
    }

    /// The middle of the faces selected and the way they face together,
    /// in the part's own space: `None` if none is selected.
    pub(super) fn faces_middle(&self, shaped: &Shaped) -> Option<(Vec3, Vec3)> {
        if matches!(self.mode, Mode::Edge | Mode::Corner) {
            // The middle of the edges' (or corners') points.
            let corners: std::collections::BTreeSet<usize> = self
                .faces
                .iter()
                .flat_map(|edge| shaped.corners_named_or_none(edge))
                .collect();
            if corners.is_empty() {
                return None;
            }
            let sum = corners
                .iter()
                .fold(Vec3::ZERO, |sum, &corner| sum + shaped.corners[corner]);
            return Some((sum * (1.0 / corners.len() as f64), Vec3::UP));
        }
        let mut weight = 0.0;
        let mut middle = Vec3::ZERO;
        let mut facing = Vec3::ZERO;
        for name in &self.faces {
            for face in shaped.faces_named(name) {
                let area = shaped.area(face);
                middle = middle + shaped.middle(face) * area;
                facing = facing + shaped.normal(face).unwrap_or(Vec3::ZERO) * area;
                weight += area;
            }
        }
        (weight > 0.0).then(|| (middle * (1.0 / weight), facing))
    }

    /// The point of the screen a face of the box is drawn at: its
    /// middle. For tests, and the tour.
    pub fn face_point(&self, name: &str) -> Option<(f64, f64)> {
        let (part, shaped) = self.shaped_selected()?;
        let face = *shaped.faces_named(name).first()?;
        let at = part.place.apply(shaped.middle(face)) + self.offset();
        self.orbit.on_screen(at, self.view.0, self.view.1)
    }
}

/// "a cylinder", "an arch".
fn a_shape(name: &str) -> String {
    if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {name}")
    } else {
        format!("a {name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_article_fits_the_word() {
        assert_eq!(a_shape("arch"), "an arch");
        assert_eq!(a_shape("box"), "a box");
    }
}
