//! The built model: a tree of placed parts and the faces they own.
//!
//! This is what a recipe becomes. It is deliberately more than a heap of
//! triangles: every part keeps its name, where it was put, its sockets
//! and which faces are its own. `inspect` reads that to say which part
//! floats or overlaps, and later phases can take rigs, colliders and LODs
//! from the same tree. The flat list of triangles for a `.glb` is made
//! from it at the very end.

use std::ops::Range;

use crate::math::{Bounds, Transform, Vec3};
use crate::recipe::PaletteColor;

/// A finished model.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// The model's name.
    pub name: String,
    /// The palette: every colour a face can have.
    pub palette: Vec<PaletteColor>,
    /// Every face of the model, where it finally is, in the order the
    /// recipe lists the parts.
    pub faces: Vec<Face>,
    /// The parts, as a tree: the recipe's list, each with the parts
    /// inside it.
    pub parts: Vec<Part>,
    /// The sockets of the model as a whole: the seven built-in ones and
    /// any the recipe declares at its top.
    pub sockets: Vec<Socket>,
}

/// One flat face of the finished model.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// Its corners, clockwise seen from outside.
    pub corners: Vec<Vec3>,
    /// Which palette colour it has: a place in [`Model::palette`]. `None`
    /// for a face nobody coloured.
    pub color: Option<usize>,
}

impl Face {
    /// How many triangles the face is cut into.
    pub fn triangle_count(&self) -> usize {
        self.corners.len().saturating_sub(2)
    }

    /// The face's triangles: a fan from its first corner.
    pub fn triangles(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        (1..self.corners.len().saturating_sub(1))
            .map(|at| [self.corners[0], self.corners[at], self.corners[at + 1]])
    }

    /// The direction the face points, 1 long. `None` for a face with no
    /// area.
    pub fn normal(&self) -> Option<Vec3> {
        let mut sum = Vec3::ZERO;
        for (at, &corner) in self.corners.iter().enumerate() {
            let next = self.corners[(at + 1) % self.corners.len()];
            sum = sum + corner.cross(next);
        }
        sum.normalized()
    }
}

/// One placed part.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Its name in its list: `left_arm`, or `#2` for an unnamed part (the
    /// one the error messages call `parts[2]`).
    pub name: String,
    /// Its name from the top of the model: `left_arm/hand`.
    pub path: String,
    /// What it is.
    pub kind: PartKind,
    /// The socket it is joined to, as the recipe wrote it:
    /// `body.left_arm`.
    pub attached_to: Option<String>,
    /// True if it, or a part it is inside, was mirrored an odd number of
    /// times: a left arm made into a right arm.
    pub mirrored: bool,
    /// How to get from the part's own space to the model's. Kept for
    /// later phases (a rig turns a part around its join).
    pub place: Transform,
    /// Its sockets, where they finally are.
    pub sockets: Vec<Socket>,
    /// Which of [`Model::faces`] are its own, the faces of the parts
    /// inside it included.
    pub faces: Range<usize>,
    /// The box around it. `None` only for a part with no faces.
    pub bounds: Option<Bounds>,
    /// The parts inside it, if it is an assembly.
    pub parts: Vec<Part>,
}

/// What a part is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct PartKind {
    /// `box`, `cylinder`, `sphere`, `cone` or `wedge`; `assembly` for a
    /// part made of parts; `mesh` for one read from a mesh file.
    pub shape: String,
    /// The `define` it came from, if it was made with `use`.
    pub uses: Option<String>,
    /// The file it came from, if it was made with `file`.
    pub file: Option<String>,
}

impl PartKind {
    /// For people: `box`, `arm (box)`, `wheel.toml (assembly)`.
    pub fn label(&self) -> String {
        match (&self.uses, &self.file) {
            (Some(name), _) => format!("{name} ({})", self.shape),
            (None, Some(file)) => format!("{file} ({})", self.shape),
            (None, None) => self.shape.clone(),
        }
    }
}

/// A named point with a direction, where it finally is in the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Socket {
    /// Its name.
    pub name: String,
    /// Where it is.
    pub at: Vec3,
    /// The direction it faces, 1 long.
    pub facing: Vec3,
}

impl Model {
    /// How many triangles the model has.
    pub fn triangle_count(&self) -> usize {
        self.faces.iter().map(Face::triangle_count).sum()
    }

    /// How many corners a `.glb` stores for it: every face keeps its own.
    pub fn vertex_count(&self) -> usize {
        self.faces.iter().map(|face| face.corners.len()).sum()
    }

    /// The box around the whole model.
    pub fn bounds(&self) -> Option<Bounds> {
        Bounds::around(
            self.faces
                .iter()
                .flat_map(|face| face.corners.iter().copied()),
        )
    }

    /// Every part that has faces of its own (not an assembly), in order,
    /// however deep it is.
    pub fn leaves(&self) -> Vec<&Part> {
        fn gather<'a>(parts: &'a [Part], into: &mut Vec<&'a Part>) {
            for part in parts {
                if part.parts.is_empty() {
                    into.push(part);
                } else {
                    gather(&part.parts, into);
                }
            }
        }
        let mut leaves = Vec::new();
        gather(&self.parts, &mut leaves);
        leaves
    }
}
