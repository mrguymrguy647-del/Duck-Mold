//! Shaping a box by hand: the `changes` a part can have.
//!
//! A shaped part is still a box with a size. Under it comes a list of
//! changes, made to the box one after the other: a pull makes faces
//! stand out (or sink in), with new walls round them; an inset makes a
//! smaller face inside a face, with a ring of faces round it.
//!
//! **Every face has a name made of words**, never a number: the box's
//! own `top`, `front`...; a pull's walls `top_side_front`; an inset's
//! middle `front_inset`. The words are passed down from the box, change
//! by change: every edge of every face knows which side of its face it
//! is on ([`Face::sides`]), and new faces take their names from those.
//! Nothing is named by measuring which way it points, so resizing the
//! box, or changing how far something is pulled, never renames a face.
//!
//! **After every change, four checks** keep the shape whole: every face
//! has room, the shape holds room inside it, no face passes through
//! another, and every edge is shared by exactly two faces. A change that
//! breaks one is a mistake that names the change ([`Mistake`]); a
//! broken shape is never built.
//!
//! As everywhere in Duck Mold, a face's corners go round so that, for
//! corners `a`, `b`, `c`, the direction `(b - a).cross(c - a)` points
//! out of the shape.

use std::collections::{BTreeSet, HashMap};

use crate::issue::Code;
use crate::math::{Mat3, Vec3, vec3};
use crate::recipe::Side;
use crate::shapes::Polygon;

/// One change to a shape, as a recipe writes it.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Pulls faces out the way they face, by `by` metres, and fills the
    /// gap with new walls (in, making a hollow, for a negative `by`).
    /// Faces that touch along an edge are pulled as one piece.
    Pull {
        /// The faces, by name.
        faces: Vec<String>,
        /// How far, in metres.
        by: f64,
        /// What the new walls are called: `<name>_front`... Without
        /// one: `<face>_side_front`...
        name: Option<String>,
    },
    /// Makes a smaller face inside each face, `by` metres in from every
    /// edge, with a ring of faces round it.
    Inset {
        /// The faces, by name.
        faces: Vec<String>,
        /// How far in from the edges, in metres.
        by: f64,
        /// What the face in the middle is called. Without one:
        /// `<face>_inset`.
        name: Option<String>,
    },
    /// Moves faces by x, y and z metres, in the part's own directions.
    /// The faces round them stretch to follow; nothing is added.
    Move {
        /// The faces, by name.
        faces: Vec<String>,
        /// How far along x, y and z.
        by: [f64; 3],
    },
    /// Turns faces about their middle: degrees around x, then y, then z.
    Turn {
        /// The faces, by name.
        faces: Vec<String>,
        /// The degrees.
        by: [f64; 3],
    },
    /// Makes faces bigger or smaller about their middle, along x, y and
    /// z.
    Scale {
        /// The faces, by name.
        faces: Vec<String>,
        /// How much, each way: 1 is as they are.
        by: [f64; 3],
    },
    /// Cuts all the way round the part with a flat cut facing `toward`,
    /// `at` of the way from the middle to that side of the box. Every
    /// face it crosses becomes two; nothing moves.
    Cut {
        /// The side the cut faces.
        toward: Side,
        /// How far from the middle (0) to the side (just under 1).
        at: f64,
        /// What the slices towards the side are called:
        /// `<face>_<name>`. Without one: `<face>_<side>`.
        name: Option<String>,
    },
}

impl Change {
    /// Every word a recipe can write for a change.
    pub const WORDS: [&'static str; 6] = ["pull", "inset", "move", "turn", "scale", "cut"];

    /// The word a recipe writes for it: `pull`, `inset`...
    pub fn word(&self) -> &'static str {
        match self {
            Change::Pull { .. } => "pull",
            Change::Inset { .. } => "inset",
            Change::Move { .. } => "move",
            Change::Turn { .. } => "turn",
            Change::Scale { .. } => "scale",
            Change::Cut { .. } => "cut",
        }
    }

    /// The faces it is made to, as written.
    pub fn faces(&self) -> &[String] {
        match self {
            Change::Pull { faces, .. }
            | Change::Inset { faces, .. }
            | Change::Move { faces, .. }
            | Change::Turn { faces, .. }
            | Change::Scale { faces, .. } => faces,
            Change::Cut { .. } => &[],
        }
    }

    /// What it does, for messages: `pulling "glass" in by 0.3`.
    pub fn doing(&self) -> String {
        match self {
            Change::Pull { faces, by, .. } => {
                let way = if *by < 0.0 { "in" } else { "out" };
                format!("pulling {} {way} by {}", quoted(faces), number(by.abs()))
            }
            Change::Inset { faces, by, .. } => {
                format!("insetting {} by {}", quoted(faces), number(*by))
            }
            Change::Move { faces, by } => {
                let what = match faces.as_slice() {
                    [one] if one.matches('+').count() == 1 => "the edge ",
                    [one] if one.contains('+') => "the corner ",
                    _ => "",
                };
                format!("moving {what}{} by {}", quoted(faces), three(by))
            }
            Change::Cut { toward, at, .. } => {
                format!("cutting towards \"{}\" at {}", toward.name(), number(*at))
            }
            Change::Turn { faces, by } => {
                format!("turning {} by {}", quoted(faces), three(by))
            }
            Change::Scale { faces, by } if by[0] == by[1] && by[1] == by[2] => {
                format!("scaling {} by {}", quoted(faces), number(by[0]))
            }
            Change::Scale { faces, by } => {
                format!("scaling {} by {}", quoted(faces), three(by))
            }
        }
    }
}

/// `[0, 0.2, 0]`.
fn three(values: &[f64; 3]) -> String {
    let [x, y, z] = values.map(number);
    format!("[{x}, {y}, {z}]")
}

/// `a and b`, `a, b and c`, for words already quoted.
fn quoted_words(words: &[String]) -> String {
    match words.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// `"a"`, `"a" and "b"`, `"a", "b" and "c"`.
pub fn quoted(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// A number for a message: to a tenth of a millimetre, with no
/// trailing zeros.
fn number(value: f64) -> String {
    let tidy = (value * 10_000.0).round() / 10_000.0 + 0.0;
    format!("{tidy}")
}

/// Why a list of changes can't be made.
#[derive(Debug, Clone, PartialEq)]
pub struct Mistake {
    /// Which change, counted from 0.
    pub change: usize,
    /// What kind of mistake.
    pub code: Code,
    /// What is wrong, in words.
    pub message: String,
    /// For a face name that isn't there: the name written, and every
    /// name there is at that point, to suggest the closest.
    pub unknown: Option<(String, Vec<String>)>,
}

/// A [`Mistake`] before it is known which change it belongs to.
struct Fault {
    code: Code,
    message: String,
    unknown: Option<(String, Vec<String>)>,
}

impl Fault {
    fn new(code: Code, message: impl Into<String>) -> Fault {
        Fault {
            code,
            message: message.into(),
            unknown: None,
        }
    }

    fn at(self, change: usize) -> Mistake {
        Mistake {
            change,
            code: self.code,
            message: self.message,
            unknown: self.unknown,
        }
    }
}

/// A shaped box: its corners, and its faces, which share them.
#[derive(Debug, Clone, PartialEq)]
pub struct Shaped {
    /// Every corner, where it is in the part's own space.
    pub corners: Vec<Vec3>,
    /// Every face.
    pub faces: Vec<Face>,
}

/// One face of a shaped box.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// Its corners, as places in [`Shaped::corners`], going round so
    /// that it faces out.
    pub corners: Vec<usize>,
    /// For each edge, from `corners[i]` to `corners[i + 1]`, which side
    /// of the face it is on: the top face's edge along the front is on
    /// its `front`. Passed down from the box, never measured.
    pub sides: Vec<Side>,
    /// The way the face faces, in the same words: passed down too. A
    /// pull's wall along the front faces `front`; a pushed-in wall still
    /// does, though it points back into the hollow.
    pub facing: Side,
    /// Its own name.
    pub name: String,
    /// The other names it answers to: `sides` for a box's four sides; a
    /// pull's name for all its walls; the old name of an inset face for
    /// the ring round the new one.
    pub also: Vec<String>,
}

impl Face {
    /// Whether `name` is its own name or one of the others.
    pub fn answers_to(&self, name: &str) -> bool {
        self.name == name || self.also.iter().any(|other| other == name)
    }
}

/// An edge of a shaped box: its two corners, and the faces on each
/// side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// Its two corners, as places in [`Shaped::corners`].
    pub corners: [usize; 2],
    /// The faces on its two sides.
    pub faces: [usize; 2],
}

/// The faces of a shaped box, ready to be a part: flat faces as one
/// polygon each, bent ones cut into triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygons {
    /// The polygons. Each one's `group` is a place in `names`.
    pub polygons: Vec<Polygon>,
    /// The faces' own names, each once, in the order they first come.
    pub names: Vec<String>,
    /// For each polygon, the face of [`Shaped::faces`] it is (part of).
    pub faces: Vec<usize>,
}

/// Builds a box of `size`, narrowed at the top by `taper`, and makes the
/// `changes` to it. `part` is the part's name, for messages.
pub fn shape_box(
    size: [f64; 3],
    taper: [f64; 2],
    changes: &[Change],
    part: Option<&str>,
) -> Result<Shaped, Mistake> {
    let mut shaped = Shaped::boxed(size, taper);
    let half = Vec3::from(size) * 0.5;
    for (at, change) in changes.iter().enumerate() {
        shaped
            .change(change, part, half)
            .map_err(|fault| fault.at(at))?;
    }
    Ok(shaped)
}

/// The way a direction mostly points, in the words of `inspect`:
/// `up`, `down`, `left`, `right`, `front`, `back`.
pub fn facing_word(direction: Vec3) -> &'static str {
    let [x, y, z] = [direction.x.abs(), direction.y.abs(), direction.z.abs()];
    if y >= x && y >= z {
        if direction.y >= 0.0 { "up" } else { "down" }
    } else if x >= z {
        if direction.x >= 0.0 { "right" } else { "left" }
    } else if direction.z >= 0.0 {
        "front"
    } else {
        "back"
    }
}

impl Shaped {
    /// A plain box, centred on its middle, its top narrowed by `taper`
    /// (how wide and deep the top is, as a share of the bottom).
    pub fn boxed(size: [f64; 3], [across, deep]: [f64; 2]) -> Shaped {
        let h = Vec3::from(size) * 0.5;
        let corner = |x: f64, y: f64, z: f64| {
            let up = (y + 1.0) / 2.0;
            vec3(
                x * h.x * (1.0 + (across - 1.0) * up),
                y * h.y,
                z * h.z * (1.0 + (deep - 1.0) * up),
            )
        };
        // Named by where they are: left/right, bottom/top, back/front.
        let corners = vec![
            corner(-1.0, -1.0, -1.0), // 0 lbb
            corner(1.0, -1.0, -1.0),  // 1 rbb
            corner(-1.0, -1.0, 1.0),  // 2 lbf
            corner(1.0, -1.0, 1.0),   // 3 rbf
            corner(-1.0, 1.0, -1.0),  // 4 ltb
            corner(1.0, 1.0, -1.0),   // 5 rtb
            corner(-1.0, 1.0, 1.0),   // 6 ltf
            corner(1.0, 1.0, 1.0),    // 7 rtf
        ];
        use Side::{Back, Bottom, Front, Left, Right, Top};
        let face = |facing: Side, corners: [usize; 4], sides: [Side; 4], also: &[&str]| Face {
            corners: corners.to_vec(),
            sides: sides.to_vec(),
            facing,
            name: facing.name().to_owned(),
            also: also.iter().map(|name| (*name).to_owned()).collect(),
        };
        Shaped {
            corners,
            faces: vec![
                face(Top, [4, 6, 7, 5], [Left, Front, Right, Back], &[]),
                face(Bottom, [0, 1, 3, 2], [Back, Right, Front, Left], &[]),
                face(Left, [0, 2, 6, 4], [Bottom, Front, Top, Back], &["sides"]),
                face(Right, [1, 5, 7, 3], [Back, Top, Front, Bottom], &["sides"]),
                face(Front, [2, 3, 7, 6], [Bottom, Right, Top, Left], &["sides"]),
                face(Back, [0, 4, 5, 1], [Left, Top, Right, Bottom], &["sides"]),
            ],
        }
    }

    // ---- names ------------------------------------------------------------

    /// Every name a face answers to, each once: the faces' own names in
    /// order, then the names that stand for several.
    pub fn names(&self) -> Vec<String> {
        let mut names = self.own_names();
        for face in &self.faces {
            for other in &face.also {
                if !names.contains(other) {
                    names.push(other.clone());
                }
            }
        }
        names
    }

    /// The faces' own names, each once, in the order they first come.
    pub fn own_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for face in &self.faces {
            if !names.contains(&face.name) {
                names.push(face.name.clone());
            }
        }
        names
    }

    /// The faces that answer to `name`.
    pub fn faces_named(&self, name: &str) -> Vec<usize> {
        (0..self.faces.len())
            .filter(|&face| self.faces[face].answers_to(name))
            .collect()
    }

    /// The own names (places in `names`, as [`Shaped::polygons`] gives
    /// them) of the faces that answer to `name`. `None` if none does.
    pub fn groups_of(&self, names: &[String], name: &str) -> Option<Vec<usize>> {
        let groups: BTreeSet<usize> = self
            .faces
            .iter()
            .filter(|face| face.answers_to(name))
            .filter_map(|face| names.iter().position(|known| *known == face.name))
            .collect();
        (!groups.is_empty()).then(|| groups.into_iter().collect())
    }

    /// The faces next to `face`: the ones it shares an edge with.
    pub fn neighbours(&self, face: usize) -> Vec<usize> {
        let edges = self.edge_faces();
        let corners = &self.faces[face].corners;
        let mut next = Vec::new();
        for at in 0..corners.len() {
            let (a, b) = (corners[at], corners[(at + 1) % corners.len()]);
            if let Some(&other) = edges.get(&(b, a))
                && other != face
                && !next.contains(&other)
            {
                next.push(other);
            }
        }
        next
    }

    // ---- measuring ----------------------------------------------------------

    fn at(&self, corners: &[usize]) -> Vec<Vec3> {
        corners.iter().map(|&corner| self.corners[corner]).collect()
    }

    /// Twice the face's area, as a direction: the way it faces, as long
    /// as twice its area. Works for a bent face too.
    fn area_vector(&self, face: usize) -> Vec3 {
        let points = self.at(&self.faces[face].corners);
        let mut sum = Vec3::ZERO;
        for (at, &point) in points.iter().enumerate() {
            sum = sum + point.cross(points[(at + 1) % points.len()]);
        }
        sum
    }

    /// The way a face faces, 1 long.
    pub fn normal(&self, face: usize) -> Option<Vec3> {
        self.area_vector(face).normalized()
    }

    /// The middle of a face: the average of its corners.
    pub fn middle(&self, face: usize) -> Vec3 {
        let points = self.at(&self.faces[face].corners);
        let sum = points.iter().fold(Vec3::ZERO, |sum, &point| sum + point);
        sum * (1.0 / points.len().max(1) as f64)
    }

    /// The face's area, in square metres.
    pub fn area(&self, face: usize) -> f64 {
        self.area_vector(face).length() / 2.0
    }

    /// How wide and tall a face is, measured along the box's own
    /// directions: across and back for one that faces up or down, across
    /// and up for one that faces front or back, back and up for one that
    /// faces left or right.
    pub fn extent(&self, face: usize) -> [f64; 2] {
        let points = self.at(&self.faces[face].corners);
        let span = |along: fn(Vec3) -> f64| {
            let (low, high) = points
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), &p| {
                    (low.min(along(p)), high.max(along(p)))
                });
            high - low
        };
        let (x, y, z) = (span(|p| p.x), span(|p| p.y), span(|p| p.z));
        match facing_word(self.area_vector(face)) {
            "up" | "down" => [x, z],
            "front" | "back" => [x, y],
            _ => [z, y],
        }
    }

    /// How much room the shape holds, in cubic metres. Less than nothing
    /// for a shape turned inside out.
    pub fn volume(&self) -> f64 {
        let mut sum = 0.0;
        for face in 0..self.faces.len() {
            for [a, b, c] in self.triangles(face) {
                let [a, b, c] = [a, b, c].map(|corner| self.corners[corner]);
                sum += a.dot(b.cross(c));
            }
        }
        sum / 6.0
    }

    /// For every edge, as (from, to) corners in the order a face goes
    /// round, the face it belongs to.
    fn edge_faces(&self) -> HashMap<(usize, usize), usize> {
        let mut edges = HashMap::new();
        for (at, face) in self.faces.iter().enumerate() {
            let corners = &face.corners;
            for step in 0..corners.len() {
                edges.insert((corners[step], corners[(step + 1) % corners.len()]), at);
            }
        }
        edges
    }

    // ---- triangles -----------------------------------------------------------

    /// Whether a face lies flat, and bulges nowhere inwards: then it is
    /// kept as one polygon.
    fn is_flat_and_plain(&self, face: usize) -> bool {
        let points = self.at(&self.faces[face].corners);
        if points.len() <= 3 {
            return true;
        }
        let Some(normal) = self.normal(face) else {
            return false;
        };
        let size = points
            .iter()
            .fold(0.0f64, |size, &p| size.max((p - points[0]).length()));
        let near = 1e-9 * size.max(1e-9);
        let middle = self.middle(face);
        let flat = points
            .iter()
            .all(|&point| normal.dot(point - middle).abs() <= near);
        let count = points.len();
        let plain = (0..count).all(|at| {
            let before = points[at] - points[(at + count - 1) % count];
            let after = points[(at + 1) % count] - points[at];
            before.cross(after).dot(normal) > -near * near
        });
        flat && plain
    }

    /// The face cut into triangles, as corners. A flat face is a fan from
    /// its first corner; a bent four-cornered face is folded the way that
    /// bulges out; anything else is cut ear by ear.
    pub fn triangles(&self, face: usize) -> Vec<[usize; 3]> {
        let corners = &self.faces[face].corners;
        let count = corners.len();
        if count < 3 {
            return Vec::new();
        }
        if self.is_flat_and_plain(face) {
            return (1..count - 1)
                .map(|at| [corners[0], corners[at], corners[at + 1]])
                .collect();
        }
        if count == 4 {
            let [a, b, c, d] = [0, 1, 2, 3].map(|at| corners[at]);
            let [pa, pb, pc, pd] = [a, b, c, d].map(|corner| self.corners[corner]);
            // Folded along a-c, the fold bulges out if d is below the
            // plane of a, b, c.
            let out = (pb - pa).cross(pc - pa);
            return if (pd - pa).dot(out) <= 0.0 {
                vec![[a, b, c], [a, c, d]]
            } else {
                vec![[b, c, d], [b, d, a]]
            };
        }
        self.ears(face)
    }

    /// Cuts a face into triangles ear by ear, seen along the way it
    /// faces. Falls back to a fan if that fails.
    fn ears(&self, face: usize) -> Vec<[usize; 3]> {
        let corners = &self.faces[face].corners;
        let normal = self.normal(face).unwrap_or(Vec3::UP);
        let mut left: Vec<usize> = corners.clone();
        let mut out = Vec::new();
        let turn = |a: Vec3, b: Vec3, c: Vec3| (b - a).cross(c - b).dot(normal);
        while left.len() > 3 {
            let count = left.len();
            let ear = (0..count).find(|&at| {
                let [a, b, c] = [
                    left[(at + count - 1) % count],
                    left[at],
                    left[(at + 1) % count],
                ]
                .map(|corner| self.corners[corner]);
                if turn(a, b, c) <= 0.0 {
                    return false;
                }
                // No other corner inside the ear.
                left.iter().all(|&other| {
                    let p = self.corners[other];
                    p == a
                        || p == b
                        || p == c
                        || turn(a, b, p) < 0.0
                        || turn(b, c, p) < 0.0
                        || turn(c, a, p) < 0.0
                })
            });
            let Some(at) = ear else {
                let corners = &self.faces[face].corners;
                return (1..corners.len() - 1)
                    .map(|at| [corners[0], corners[at], corners[at + 1]])
                    .collect();
            };
            out.push([
                left[(at + count - 1) % count],
                left[at],
                left[(at + 1) % count],
            ]);
            left.remove(at);
        }
        out.push([left[0], left[1], left[2]]);
        out
    }

    /// The faces as polygons for a part: a flat face as one polygon, a
    /// bent one as its triangles, each knowing its face's own name.
    pub fn polygons(&self) -> Polygons {
        let names = self.own_names();
        let mut out = Polygons {
            polygons: Vec::new(),
            names: names.clone(),
            faces: Vec::new(),
        };
        for (at, face) in self.faces.iter().enumerate() {
            let group = names
                .iter()
                .position(|name| *name == face.name)
                .unwrap_or(0);
            if self.is_flat_and_plain(at) {
                out.polygons.push(Polygon {
                    corners: self.at(&face.corners),
                    group,
                });
                out.faces.push(at);
            } else {
                for triangle in self.triangles(at) {
                    out.polygons.push(Polygon {
                        corners: self.at(&triangle),
                        group,
                    });
                    out.faces.push(at);
                }
            }
        }
        out
    }

    // ---- changes ---------------------------------------------------------------

    fn change(&mut self, change: &Change, part: Option<&str>, half: Vec3) -> Result<(), Fault> {
        let scale = half.x.max(half.y).max(half.z) * 2.0;
        let names = change.faces();
        let has_edge = names.iter().any(|name| name.contains('+'));
        if has_edge && !matches!(change, Change::Move { .. }) {
            return Err(Fault::new(
                Code::TypeMismatch,
                format!(
                    "only a move can be made to an edge or a corner, not a {}",
                    change.word()
                ),
            ));
        }
        let faces = if has_edge {
            Vec::new()
        } else {
            self.find(names, part)?
        };
        let taken: BTreeSet<String> = self.names().into_iter().collect();
        let touched = match change {
            Change::Pull { by, name, .. } => self.pull(&faces, *by, name.as_deref(), &taken)?,
            Change::Inset { by, name, .. } => self.inset(&faces, *by, name.as_deref(), &taken)?,
            Change::Move { faces: names, by } if has_edge => {
                let mut corners = BTreeSet::new();
                for name in names {
                    corners.extend(self.corners_named(name, part)?);
                }
                let by = Vec3::from(*by);
                self.reshape_corners(&corners, Vec3::ZERO, |point, _| point + by)
            }
            Change::Cut { toward, at, name } => {
                self.cut(*toward, *at, name.as_deref(), half, &taken)?
            }
            Change::Move { by, .. } => {
                let by = Vec3::from(*by);
                self.reshape(&faces, |point, _| point + by)
            }
            Change::Turn { by, .. } => {
                let turn = Mat3::from_rotate(*by);
                self.reshape(&faces, |point, middle| middle + turn * (point - middle))
            }
            Change::Scale { by, .. } => {
                let by = Vec3::from(*by);
                self.reshape(&faces, |point, middle| middle + (point - middle).times(by))
            }
        };
        self.tidy();
        self.check(&touched, &change.doing(), scale)
    }

    /// The corners an edge (`top+front`) or a corner (`top+front+left`)
    /// name stands for: where faces with all those names meet. A face's
    /// own name gives all its corners.
    pub fn corners_named_or_none(&self, name: &str) -> Vec<usize> {
        self.corners_named(name, None).unwrap_or_default()
    }

    fn corners_named(&self, name: &str, part: Option<&str>) -> Result<Vec<usize>, Fault> {
        let parts: Vec<&str> = name.split('+').collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for one in &parts {
            let these = self.faces_named(one);
            if these.is_empty() {
                let on = part.map_or("this part".to_owned(), |part| format!("\"{part}\""));
                return Err(Fault {
                    code: Code::UnknownFace,
                    message: format!("there is no face \"{one}\" on {on}"),
                    unknown: Some(((*one).to_owned(), self.names())),
                });
            }
            groups.push(these);
        }
        let corners_of = |faces: &[usize]| -> BTreeSet<usize> {
            faces
                .iter()
                .flat_map(|&face| self.faces[face].corners.iter().copied())
                .collect()
        };
        let found: BTreeSet<usize> = match groups.as_slice() {
            [one] => corners_of(one),
            [a, b] => {
                // Both ends of every edge where a face of `a` meets one
                // of `b`.
                let edges = self.edge_faces();
                let mut found = BTreeSet::new();
                for &face in a {
                    let corners = &self.faces[face].corners;
                    for at in 0..corners.len() {
                        let (from, to) = (corners[at], corners[(at + 1) % corners.len()]);
                        if edges
                            .get(&(to, from))
                            .is_some_and(|other| b.contains(other))
                        {
                            found.insert(from);
                            found.insert(to);
                        }
                    }
                }
                found
            }
            many => {
                let mut found = corners_of(&many[0]);
                for faces in &many[1..] {
                    let these = corners_of(faces);
                    found.retain(|corner| these.contains(corner));
                }
                found
            }
        };
        if found.is_empty() {
            let what = if parts.len() == 2 { "edge" } else { "corner" };
            let faces = parts
                .iter()
                .map(|one| format!("\"{one}\""))
                .collect::<Vec<_>>();
            let on = part.map_or("this part".to_owned(), |part| format!("\"{part}\""));
            return Err(Fault::new(
                Code::UnknownFace,
                format!(
                    "there is no {what} \"{name}\" on {on}: {} don't meet",
                    quoted_words(&faces)
                ),
            ));
        }
        Ok(found.into_iter().collect())
    }

    /// Every edge, once: its two corners, and the faces on each side.
    pub fn edges(&self) -> Vec<Edge> {
        let edges = self.edge_faces();
        let mut out = Vec::new();
        for (at, face) in self.faces.iter().enumerate() {
            let corners = &face.corners;
            for step in 0..corners.len() {
                let (from, to) = (corners[step], corners[(step + 1) % corners.len()]);
                if let Some(&other) = edges.get(&(to, from))
                    && (from < to || other == at)
                {
                    out.push(Edge {
                        corners: [from, to],
                        faces: [at, other],
                    });
                }
            }
        }
        out
    }

    /// An edge's name: the names of the faces on its two sides, joined
    /// with `+`, the upright one first, then front or back, then left or
    /// right (as `top+front+left` is written).
    pub fn edge_name(&self, edge: &Edge) -> String {
        self.joined_name(&edge.faces)
    }

    /// A corner's name: the names of the faces that meet there.
    pub fn corner_name(&self, corner: usize) -> String {
        let faces: Vec<usize> = (0..self.faces.len())
            .filter(|&face| self.faces[face].corners.contains(&corner))
            .collect();
        self.joined_name(&faces)
    }

    fn joined_name(&self, faces: &[usize]) -> String {
        let rank = |face: usize| match self.faces[face].facing {
            Side::Top | Side::Bottom => 0,
            Side::Front | Side::Back => 1,
            _ => 2,
        };
        let mut sorted: Vec<usize> = faces.to_vec();
        sorted.sort_by_key(|&face| (rank(face), face));
        let mut names: Vec<&str> = Vec::new();
        for face in sorted {
            if !names.contains(&self.faces[face].name.as_str()) {
                names.push(&self.faces[face].name);
            }
        }
        names.join("+")
    }

    /// The faces the names stand for, each once, in order.
    fn find(&self, names: &[String], part: Option<&str>) -> Result<Vec<usize>, Fault> {
        let mut found = BTreeSet::new();
        for name in names {
            let these = self.faces_named(name);
            if these.is_empty() {
                let on = part.map_or("this part".to_owned(), |part| format!("\"{part}\""));
                return Err(Fault {
                    code: Code::UnknownFace,
                    message: format!("there is no face \"{name}\" on {on}"),
                    unknown: Some((name.clone(), self.names())),
                });
            }
            found.extend(these);
        }
        Ok(found.into_iter().collect())
    }

    /// The first of `base`, `base_2`, `base_3`... for which none of the
    /// names `make` gives is taken.
    fn untaken(taken: &BTreeSet<String>, make: impl Fn(&str) -> Vec<String>) -> usize {
        (1..)
            .find(|&count| {
                let suffix = if count == 1 {
                    String::new()
                } else {
                    format!("_{count}")
                };
                make(&suffix).iter().all(|name| !taken.contains(name))
            })
            .unwrap_or(1)
    }

    fn pull(
        &mut self,
        region: &[usize],
        by: f64,
        given: Option<&str>,
        taken: &BTreeSet<String>,
    ) -> Result<Vec<usize>, Fault> {
        let inside: Vec<bool> = (0..self.faces.len())
            .map(|face| region.contains(&face))
            .collect();
        let edges = self.edge_faces();

        // The pieces: faces of the region that share an edge are pulled
        // together, the same way.
        let mut pieces = Join::new(self.faces.len());
        // The corners: where faces of one piece meet round a corner,
        // they move it together. (A corner where two pieces, or two
        // fans of one piece, meet gets a copy for each.)
        let mut slots: HashMap<(usize, usize), usize> = HashMap::new();
        for &face in region {
            for &corner in &self.faces[face].corners {
                let next = slots.len();
                slots.entry((face, corner)).or_insert(next);
            }
        }
        let mut fans = Join::new(slots.len());
        for &face in region {
            let corners = &self.faces[face].corners;
            for at in 0..corners.len() {
                let (a, b) = (corners[at], corners[(at + 1) % corners.len()]);
                if let Some(&other) = edges.get(&(b, a))
                    && inside[other]
                {
                    pieces.join(face, other);
                    fans.join(slots[&(face, a)], slots[&(other, a)]);
                    fans.join(slots[&(face, b)], slots[&(other, b)]);
                }
            }
        }

        // Which way each piece goes: the way its faces face, together.
        let mut ways: HashMap<usize, Vec3> = HashMap::new();
        for &face in region {
            let piece = pieces.find(face);
            let area = self.area_vector(face);
            let sum = ways.entry(piece).or_insert(Vec3::ZERO);
            *sum = *sum + area;
        }
        let mut directions: HashMap<usize, Vec3> = HashMap::new();
        for (piece, sum) in ways {
            let Some(way) = sum.normalized() else {
                return Err(Fault::new(
                    Code::BadShape,
                    "these faces face every way at once, so there is no one way to pull them: pull them one at a time",
                ));
            };
            directions.insert(piece, way);
        }

        // Which corners faces outside the region use too: those stay,
        // and the region gets copies.
        let mut used_outside = vec![false; self.corners.len()];
        for (at, face) in self.faces.iter().enumerate() {
            if !inside[at] {
                for &corner in &face.corners {
                    used_outside[corner] = true;
                }
            }
        }
        let mut fans_at: HashMap<usize, BTreeSet<usize>> = HashMap::new();
        for (&(_, corner), &slot) in &slots {
            fans_at.entry(corner).or_default().insert(fans.find(slot));
        }
        let mut moved: HashMap<usize, usize> = HashMap::new();
        let mut order: Vec<(usize, usize)> = slots.keys().copied().collect();
        order.sort_unstable();
        let old = self.corners.clone();
        for (face, corner) in order {
            let fan = fans.find(slots[&(face, corner)]);
            if moved.contains_key(&fan) {
                continue;
            }
            let shift = directions[&pieces.find(face)] * by;
            let alone = !used_outside[corner] && fans_at[&corner].len() == 1;
            if alone {
                self.corners[corner] = old[corner] + shift;
                moved.insert(fan, corner);
            } else {
                self.corners.push(old[corner] + shift);
                moved.insert(fan, self.corners.len() - 1);
            }
        }

        // The region's faces go to their new corners; walls fill the gap
        // along every edge where the region meets the rest.
        let mut walls: Vec<(usize, usize)> = Vec::new();
        let mut was: HashMap<usize, Vec<usize>> = HashMap::new();
        for &face in region {
            let corners = self.faces[face].corners.clone();
            for at in 0..corners.len() {
                let (a, b) = (corners[at], corners[(at + 1) % corners.len()]);
                if !edges.get(&(b, a)).is_some_and(|&other| inside[other]) {
                    walls.push((face, at));
                }
            }
            let new: Vec<usize> = corners
                .iter()
                .map(|&corner| moved[&fans.find(slots[&(face, corner)])])
                .collect();
            was.insert(face, corners);
            self.faces[face].corners = new;
        }

        // The walls' names.
        let wall_word = |face: &Face, at: usize| face.sides[at].name();
        let mut prefixes: HashMap<String, String> = HashMap::new();
        if given.is_none() {
            for &(face, _) in &walls {
                let base = format!("{}_side", self.faces[face].name);
                if prefixes.contains_key(&base) {
                    continue;
                }
                let words: BTreeSet<&str> = walls
                    .iter()
                    .filter(|(other, _)| format!("{}_side", self.faces[*other].name) == base)
                    .map(|&(other, at)| wall_word(&self.faces[other], at))
                    .collect();
                let count = Shaped::untaken(taken, |suffix| {
                    let mut names = vec![format!("{base}{suffix}")];
                    names.extend(words.iter().map(|word| format!("{base}_{word}{suffix}")));
                    names
                });
                let suffix = if count == 1 {
                    String::new()
                } else {
                    format!("_{count}")
                };
                prefixes.insert(base, suffix);
            }
        }

        let mut touched: Vec<usize> = region.to_vec();
        for (face, at) in walls {
            let old_corners = &was[&face];
            let new_corners = &self.faces[face].corners;
            let count = old_corners.len();
            let next = (at + 1) % count;
            let source = &self.faces[face];
            let word = wall_word(source, at);
            let (name, group) = match given {
                Some(given) => (format!("{given}_{word}"), given.to_owned()),
                None => {
                    let base = format!("{}_side", source.name);
                    let suffix = &prefixes[&base];
                    (format!("{base}_{word}{suffix}"), format!("{base}{suffix}"))
                }
            };
            let wall = Face {
                corners: vec![
                    old_corners[at],
                    old_corners[next],
                    new_corners[next],
                    new_corners[at],
                ],
                sides: vec![
                    source.facing.opposite(),
                    source.sides[next],
                    source.facing,
                    source.sides[(at + count - 1) % count],
                ],
                facing: source.sides[at],
                name,
                also: vec![group],
            };
            self.faces.push(wall);
            touched.push(self.faces.len() - 1);
        }
        Ok(touched)
    }

    fn inset(
        &mut self,
        targets: &[usize],
        by: f64,
        given: Option<&str>,
        taken: &BTreeSet<String>,
    ) -> Result<Vec<usize>, Fault> {
        // Every face is checked before any is changed.
        let mut plans = Vec::new();
        for &face in targets {
            let points = self.at(&self.faces[face].corners);
            let count = points.len();
            let Some(normal) = self.normal(face) else {
                return Err(Fault::new(
                    Code::BadShape,
                    format!("\"{}\" has no room to inset", self.faces[face].name),
                ));
            };
            // Each edge's way inwards, along the face.
            let inward: Vec<Vec3> = (0..count)
                .map(|at| {
                    normal
                        .cross(points[(at + 1) % count] - points[at])
                        .normalized()
                        .unwrap_or(Vec3::ZERO)
                })
                .collect();
            // How far each corner moves for an inset of 1: in from both
            // its edges at once.
            let mut offsets = Vec::with_capacity(count);
            for at in 0..count {
                let (before, after) = (inward[(at + count - 1) % count], inward[at]);
                let bend = 1.0 + before.dot(after);
                if bend < 1e-6 {
                    return Err(Fault::new(
                        Code::BadShape,
                        format!(
                            "\"{}\" turns back on itself at a corner, so it can't be inset",
                            self.faces[face].name
                        ),
                    ));
                }
                offsets.push((before + after) * (1.0 / bend));
            }
            // The widest inset before an edge shrinks to nothing.
            let mut widest = f64::INFINITY;
            for at in 0..count {
                let edge = points[(at + 1) % count] - points[at];
                let length = edge.length();
                let along = edge * (1.0 / length.max(f64::MIN_POSITIVE));
                let shrink = -(offsets[(at + 1) % count] - offsets[at]).dot(along);
                if shrink > 1e-12 {
                    widest = widest.min(length / shrink);
                }
            }
            if by >= widest * (1.0 - 1e-9) {
                let across = (widest * 2.0 * 1000.0).round() / 1000.0;
                let most = (widest * 1000.0).floor() / 1000.0;
                return Err(Fault::new(
                    Code::OutOfRange,
                    format!(
                        "an inset of {} m is too wide for \"{}\": it is {} m across, so the inset must be less than {} m",
                        number(by),
                        self.faces[face].name,
                        number(across),
                        number(most)
                    ),
                ));
            }
            let inner: Vec<Vec3> = (0..count).map(|at| points[at] + offsets[at] * by).collect();
            plans.push((face, inner));
        }

        let mut touched = Vec::new();
        for (face, inner) in plans {
            let old = self.faces[face].clone();
            let count = old.corners.len();
            let middle_name = match given {
                Some(given) => given.to_owned(),
                None => {
                    let base = format!("{}_inset", old.name);
                    let words: Vec<&str> = old.sides.iter().map(|side| side.name()).collect();
                    let count = Shaped::untaken(taken, |suffix| {
                        let middle = format!("{base}{suffix}");
                        let mut names: Vec<String> = words
                            .iter()
                            .map(|word| format!("{middle}_rim_{word}"))
                            .collect();
                        names.push(middle);
                        names
                    });
                    if count == 1 {
                        base
                    } else {
                        format!("{base}_{count}")
                    }
                }
            };
            let first = self.corners.len();
            self.corners.extend(inner);
            let new: Vec<usize> = (first..first + count).collect();
            let mut ring_also = vec![old.name.clone()];
            for other in &old.also {
                if !ring_also.contains(other) {
                    ring_also.push(other.clone());
                }
            }
            for at in 0..count {
                let next = (at + 1) % count;
                let side = old.sides[at];
                self.faces.push(Face {
                    corners: vec![old.corners[at], old.corners[next], new[next], new[at]],
                    sides: vec![
                        side,
                        old.sides[next],
                        side.opposite(),
                        old.sides[(at + count - 1) % count],
                    ],
                    facing: old.facing,
                    name: format!("{middle_name}_rim_{}", side.name()),
                    also: ring_also.clone(),
                });
                touched.push(self.faces.len() - 1);
            }
            self.faces[face] = Face {
                corners: new,
                sides: old.sides.clone(),
                facing: old.facing,
                name: middle_name,
                also: old.also.clone(),
            };
            touched.push(face);
        }
        Ok(touched)
    }

    /// The middle of some faces, each counted by its area.
    pub fn middle_of(&self, faces: &[usize]) -> Vec3 {
        let mut weight = 0.0;
        let mut sum = Vec3::ZERO;
        for &face in faces {
            let area = self.area(face);
            sum = sum + self.middle(face) * area;
            weight += area;
        }
        if weight > 0.0 {
            sum * (1.0 / weight)
        } else {
            faces.first().map_or(Vec3::ZERO, |&face| self.middle(face))
        }
    }

    /// Moves every corner of `faces` to where `to` says (given the
    /// corner and the faces' middle). Gives every face that has one of
    /// those corners: what may now pass through something.
    fn reshape(&mut self, faces: &[usize], to: impl Fn(Vec3, Vec3) -> Vec3) -> Vec<usize> {
        let middle = self.middle_of(faces);
        let corners: BTreeSet<usize> = faces
            .iter()
            .flat_map(|&face| self.faces[face].corners.iter().copied())
            .collect();
        self.reshape_corners(&corners, middle, to)
    }

    /// Moves `corners` to where `to` says; gives the faces that have one.
    fn reshape_corners(
        &mut self,
        corners: &BTreeSet<usize>,
        middle: Vec3,
        to: impl Fn(Vec3, Vec3) -> Vec3,
    ) -> Vec<usize> {
        for &corner in corners {
            self.corners[corner] = to(self.corners[corner], middle);
        }
        (0..self.faces.len())
            .filter(|&face| {
                self.faces[face]
                    .corners
                    .iter()
                    .any(|corner| corners.contains(corner))
            })
            .collect()
    }

    /// Cuts all the way round: every face that crosses the flat cut
    /// facing `toward`, `at` of the way from the middle to that side of
    /// the box (whose half size is `half`), becomes two.
    fn cut(
        &mut self,
        toward: Side,
        at: f64,
        given: Option<&str>,
        half: Vec3,
        taken: &BTreeSet<String>,
    ) -> Result<Vec<usize>, Fault> {
        let way = Vec3::from(toward.offset());
        let reach = way.dot(half).abs();
        let place = at * reach;
        let near = 1e-9 * (half.x.max(half.y).max(half.z) * 2.0).max(1e-9);
        // How far each corner is past the cut, towards the side.
        let past = |point: Vec3| {
            let along = way.dot(point) - place;
            if along.abs() <= near { 0.0 } else { along }
        };
        let side_word = toward.name();
        // The new corners, one for each edge the cut crosses, shared by
        // the two faces of that edge.
        let mut made: HashMap<(usize, usize), usize> = HashMap::new();
        let mut pieces: Vec<(usize, Option<Face>, Option<Face>)> = Vec::new();
        let count = self.faces.len();
        for face in 0..count {
            let corners = self.faces[face].corners.clone();
            let sides = self.faces[face].sides.clone();
            let n = corners.len();
            let d: Vec<f64> = corners.iter().map(|&c| past(self.corners[c])).collect();
            if !(d.iter().any(|&x| x > 0.0) && d.iter().any(|&x| x < 0.0)) {
                continue;
            }
            let crossings = (0..n).filter(|&i| d[i] * d[(i + 1) % n] < 0.0).count()
                + (0..n).filter(|&i| d[i] == 0.0).count();
            if crossings > 2 {
                return Err(Fault::new(
                    Code::BadShape,
                    format!(
                        "cutting towards \"{side_word}\" at {} crosses the face \"{}\" more than once: cut elsewhere",
                        number(at),
                        self.faces[face].name
                    ),
                ));
            }
            // Walk round the face: each corner goes to its side (both,
            // on the cut); where an edge crosses, the new corner goes to
            // both, and the new edge between them lies along the cut.
            let mut middle: (Vec<usize>, Vec<Side>) = (Vec::new(), Vec::new());
            let mut slice: (Vec<usize>, Vec<Side>) = (Vec::new(), Vec::new());
            for i in 0..n {
                let j = (i + 1) % n;
                let (a, b) = (corners[i], corners[j]);
                if d[i] <= 0.0 {
                    middle.0.push(a);
                    middle.1.push(sides[i]);
                }
                if d[i] >= 0.0 {
                    slice.0.push(a);
                    slice.1.push(sides[i]);
                }
                if d[i] * d[j] < 0.0 {
                    let key = (a.min(b), a.max(b));
                    let new = match made.get(&key) {
                        Some(&new) => new,
                        None => {
                            let (pa, pb) = (self.corners[a], self.corners[b]);
                            let share = d[i] / (d[i] - d[j]);
                            self.corners.push(pa + (pb - pa) * share);
                            made.insert(key, self.corners.len() - 1);
                            self.corners.len() - 1
                        }
                    };
                    middle.0.push(new);
                    middle.1.push(sides[i]);
                    slice.0.push(new);
                    slice.1.push(sides[i]);
                }
            }
            // The edge along the cut is the one that leaves the cut: for
            // the middle piece the side faces the cut's side; for the
            // slice, the other way.
            let fix =
                |piece: &mut (Vec<usize>, Vec<Side>), d_of: &dyn Fn(usize) -> f64, word: Side| {
                    let m = piece.0.len();
                    for k in 0..m {
                        let (a, b) = (piece.0[k], piece.0[(k + 1) % m]);
                        if d_of(a) == 0.0 && d_of(b) == 0.0 {
                            piece.1[k] = word;
                        }
                    }
                };
            let points = self.corners.clone();
            let d_of = |corner: usize| past(points[corner]);
            fix(&mut middle, &d_of, toward);
            fix(&mut slice, &d_of, toward.opposite());
            let old = &self.faces[face];
            let keep = Face {
                corners: middle.0,
                sides: middle.1,
                facing: old.facing,
                name: old.name.clone(),
                also: old.also.clone(),
            };
            let cut_off = Face {
                corners: slice.0,
                sides: slice.1,
                facing: old.facing,
                name: String::new(),
                also: old.also.clone(),
            };
            pieces.push((face, Some(keep), Some(cut_off)));
        }
        // The slices' names: `<face>_<side>`, or `<face>_<name>`; a
        // number added if an earlier change made that name already.
        let word = given.unwrap_or(side_word);
        let mut chosen: HashMap<String, String> = HashMap::new();
        let mut touched = Vec::new();
        for (face, keep, cut_off) in pieces {
            let old_name = self.faces[face].name.clone();
            let name = match chosen.get(&old_name) {
                Some(name) => name.clone(),
                None => {
                    let base = format!("{old_name}_{word}");
                    let count = if given.is_some() {
                        1
                    } else {
                        Shaped::untaken(taken, |suffix| vec![format!("{base}{suffix}")])
                    };
                    let name = if count == 1 {
                        base
                    } else {
                        format!("{base}_{count}")
                    };
                    chosen.insert(old_name.clone(), name.clone());
                    name
                }
            };
            if let Some(keep) = keep {
                self.faces[face] = keep;
                touched.push(face);
            }
            if let Some(mut cut_off) = cut_off {
                cut_off.name = name;
                self.faces.push(cut_off);
                touched.push(self.faces.len() - 1);
            }
        }
        Ok(touched)
    }

    /// Drops the corners no face uses any more.
    fn tidy(&mut self) {
        let mut used = vec![false; self.corners.len()];
        for face in &self.faces {
            for &corner in &face.corners {
                used[corner] = true;
            }
        }
        if used.iter().all(|&used| used) {
            return;
        }
        let mut place = vec![usize::MAX; self.corners.len()];
        let mut kept = Vec::new();
        for (at, &corner) in self.corners.iter().enumerate() {
            if used[at] {
                place[at] = kept.len();
                kept.push(corner);
            }
        }
        self.corners = kept;
        for face in &mut self.faces {
            for corner in &mut face.corners {
                *corner = place[*corner];
            }
        }
    }

    // ---- the four checks --------------------------------------------------------

    fn check(&self, touched: &[usize], doing: &str, scale: f64) -> Result<(), Fault> {
        let near = 1e-6 * scale.max(1e-6);

        // Closed: every edge is shared by exactly two faces, going round
        // them opposite ways. Nothing above can break this; it is
        // checked so that a mistake in Duck Mold can't make a broken
        // model.
        let mut edges: HashMap<(usize, usize), usize> = HashMap::new();
        for face in &self.faces {
            let corners = &face.corners;
            for at in 0..corners.len() {
                *edges
                    .entry((corners[at], corners[(at + 1) % corners.len()]))
                    .or_default() += 1;
            }
        }
        let open = edges
            .iter()
            .any(|(&(a, b), &count)| a == b || count != 1 || edges.get(&(b, a)) != Some(&1));
        if open {
            return Err(Fault::new(
                Code::BadShape,
                format!(
                    "{doing} leaves the shape open. That is a mistake in Duck Mold itself: please report it"
                ),
            ));
        }

        // Room: no face squeezed to a line or a point.
        for (at, face) in self.faces.iter().enumerate() {
            let points = self.at(&face.corners);
            let short = (0..points.len())
                .any(|step| (points[(step + 1) % points.len()] - points[step]).length() <= near);
            if short || self.area(at) <= near * near {
                return Err(Fault::new(
                    Code::BadShape,
                    format!("{doing} squeezes the face \"{}\" flat", face.name),
                ));
            }
        }

        // No two corners in one place: the shape would touch itself
        // there, closed by its numbers but not as it stands.
        let cell = near * 4.0;
        let mut seen: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (at, corner) in self.corners.iter().enumerate() {
            let key = |value: f64| (value / cell).floor() as i64;
            let home = (key(corner.x), key(corner.y), key(corner.z));
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let next = (home.0 + dx, home.1 + dy, home.2 + dz);
                        if let Some(others) = seen.get(&next)
                            && others
                                .iter()
                                .any(|&other| (self.corners[other] - *corner).length() <= near)
                        {
                            return Err(Fault::new(
                                Code::BadShape,
                                format!("{doing} makes the shape touch itself at a corner"),
                            ));
                        }
                    }
                }
            }
            seen.entry(home).or_default().push(at);
        }

        // Room inside: a shape turned inside out holds less than nothing.
        if self.volume() <= near * near * near {
            return Err(Fault::new(
                Code::BadShape,
                format!("{doing} turns the shape inside out"),
            ));
        }

        // Nothing passes through anything.
        if let Some((a, b)) = self.crossing(touched, near) {
            let (a, b) = (&self.faces[a].name, &self.faces[b].name);
            let message = if a == b {
                format!("{doing} makes the faces called \"{a}\" pass through each other")
            } else {
                format!("{doing} makes \"{a}\" pass through \"{b}\"")
            };
            return Err(Fault::new(Code::BadShape, message));
        }
        Ok(())
    }

    /// Two faces that pass through each other, one of them in `touched`
    /// (faces nothing moved can't newly cross). Faces that are
    /// neighbours may meet along their edge or at their corner.
    fn crossing(&self, touched: &[usize], near: f64) -> Option<(usize, usize)> {
        let mut triangles: Vec<(usize, [usize; 3])> = Vec::new();
        for face in 0..self.faces.len() {
            triangles.extend(self.triangles(face).into_iter().map(|t| (face, t)));
        }
        let boxes: Vec<(Vec3, Vec3)> = triangles
            .iter()
            .map(|(_, t)| {
                let [a, b, c] = t.map(|corner| self.corners[corner]);
                (a.min(b).min(c), a.max(b).max(c))
            })
            .collect();
        let mut is_touched = vec![false; self.faces.len()];
        for &face in touched {
            is_touched[face] = true;
        }
        for (one, &(face_one, t)) in triangles.iter().enumerate() {
            if !is_touched[face_one] {
                continue;
            }
            for (two, &(face_two, u)) in triangles.iter().enumerate() {
                if face_two == face_one || (is_touched[face_two] && two < one) {
                    continue;
                }
                let ((low_a, high_a), (low_b, high_b)) = (boxes[one], boxes[two]);
                let apart = low_a.x > high_b.x + near
                    || low_b.x > high_a.x + near
                    || low_a.y > high_b.y + near
                    || low_b.y > high_a.y + near
                    || low_a.z > high_b.z + near
                    || low_b.z > high_a.z + near;
                if !apart && self.triangles_cross(t, u, near) {
                    return Some((face_one, face_two));
                }
            }
        }
        None
    }

    fn triangles_cross(&self, t: [usize; 3], u: [usize; 3], near: f64) -> bool {
        let shared: Vec<usize> = t
            .iter()
            .copied()
            .filter(|corner| u.contains(corner))
            .collect();
        let place = |corners: [usize; 3]| corners.map(|corner| self.corners[corner]);
        match shared.as_slice() {
            [] => apart_cross(place(t), place(u), near),
            [corner] => {
                let first = |tri: [usize; 3]| {
                    let at = tri.iter().position(|c| c == corner).unwrap_or(0);
                    [tri[at], tri[(at + 1) % 3], tri[(at + 2) % 3]]
                };
                corner_cross(place(first(t)), place(first(u)))
            }
            [a, b] => {
                let other =
                    |tri: [usize; 3]| tri.iter().copied().find(|c| c != a && c != b).unwrap_or(*a);
                edge_fold(
                    self.corners[*a],
                    self.corners[*b],
                    self.corners[other(t)],
                    self.corners[other(u)],
                    near,
                )
            }
            _ => true,
        }
    }
}

/// Joins things into groups: a union-find.
struct Join {
    parent: Vec<usize>,
}

impl Join {
    fn new(count: usize) -> Join {
        Join {
            parent: (0..count).collect(),
        }
    }

    fn find(&mut self, at: usize) -> usize {
        let mut root = at;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut walk = at;
        while self.parent[walk] != root {
            let next = self.parent[walk];
            self.parent[walk] = root;
            walk = next;
        }
        root
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
    }
}

// ---- triangles passing through triangles ------------------------------------------

/// Two triangles that share an edge (`a`-`b`; their third corners are
/// `c` and `d`) overlap only if they lie in one plane, folded onto each
/// other.
fn edge_fold(a: Vec3, b: Vec3, c: Vec3, d: Vec3, near: f64) -> bool {
    let Some(normal) = (b - a).cross(c - a).normalized() else {
        return false;
    };
    if normal.dot(d - a).abs() > near {
        return false;
    }
    let edge = b - a;
    let one = edge.cross(c - a).dot(normal);
    let two = edge.cross(d - a).dot(normal);
    one * two > 0.0
}

/// Whether the direction `w` is strictly between `p` and `q` (less than
/// a half turn apart), seen along `normal`.
fn between(w: Vec3, p: Vec3, q: Vec3, normal: Vec3) -> bool {
    const TINY: f64 = 1e-9;
    let (Some(w), Some(p), Some(q)) = (w.normalized(), p.normalized(), q.normalized()) else {
        return false;
    };
    let turn = if p.cross(q).dot(normal) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    p.cross(w).dot(normal) * turn > TINY && w.cross(q).dot(normal) * turn > TINY
}

/// Two triangles that share one corner (each given with that corner
/// first) meet anywhere else only if they overlap near it.
fn corner_cross([v, a1, a2]: [Vec3; 3], [_, b1, b2]: [Vec3; 3]) -> bool {
    let (Some(normal_a), Some(normal_b)) = (
        (a1 - v).cross(a2 - v).normalized(),
        (b1 - v).cross(b2 - v).normalized(),
    ) else {
        return false;
    };
    let in_a = |w: Vec3| between(w, a1 - v, a2 - v, normal_a);
    let in_b = |w: Vec3| between(w, b1 - v, b2 - v, normal_b);
    let line = normal_a.cross(normal_b);
    match line.normalized() {
        // In one plane: they overlap if their corners' wedges do.
        None if line.length() < 1e-9 => {
            let (middle_a, middle_b) = (
                (a1 - v).normalized().unwrap_or(Vec3::ZERO)
                    + (a2 - v).normalized().unwrap_or(Vec3::ZERO),
                (b1 - v).normalized().unwrap_or(Vec3::ZERO)
                    + (b2 - v).normalized().unwrap_or(Vec3::ZERO),
            );
            in_a(b1 - v)
                || in_a(b2 - v)
                || in_a(middle_b)
                || in_b(a1 - v)
                || in_b(a2 - v)
                || in_b(middle_a)
        }
        None => false,
        // The two planes meet in a line through the corner: the
        // triangles overlap if both reach along it the same way.
        Some(line) => (in_a(line) && in_b(line)) || (in_a(-line) && in_b(-line)),
    }
}

/// Two triangles with no corner in common: whether they pass through
/// each other, more than just touching.
fn apart_cross(t: [Vec3; 3], u: [Vec3; 3], near: f64) -> bool {
    let (Some(normal_t), Some(normal_u)) = (
        (t[1] - t[0]).cross(t[2] - t[0]).normalized(),
        (u[1] - u[0]).cross(u[2] - u[0]).normalized(),
    ) else {
        return false;
    };
    let snap = |value: f64| if value.abs() <= near { 0.0 } else { value };
    let du = u.map(|p| snap(normal_t.dot(p - t[0])));
    if du.iter().all(|&d| d > 0.0) || du.iter().all(|&d| d < 0.0) {
        return false;
    }
    let dt = t.map(|p| snap(normal_u.dot(p - u[0])));
    if dt.iter().all(|&d| d > 0.0) || dt.iter().all(|&d| d < 0.0) {
        return false;
    }
    if du.iter().all(|&d| d == 0.0) || dt.iter().all(|&d| d == 0.0) {
        return flat_overlap(t, u, normal_t, near);
    }
    let Some(line) = normal_t.cross(normal_u).normalized() else {
        return flat_overlap(t, u, normal_t, near);
    };
    let span = |points: [Vec3; 3], distances: [f64; 3]| {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        let mut take = |p: Vec3| {
            let along = line.dot(p);
            low = low.min(along);
            high = high.max(along);
        };
        for at in 0..3 {
            let next = (at + 1) % 3;
            if distances[at] == 0.0 {
                take(points[at]);
            }
            if distances[at] * distances[next] < 0.0 {
                let share = distances[at] / (distances[at] - distances[next]);
                take(points[at] + (points[next] - points[at]) * share);
            }
        }
        (low, high)
    };
    let (low_t, high_t) = span(t, dt);
    let (low_u, high_u) = span(u, du);
    high_t.min(high_u) - low_t.max(low_u) > near
}

/// Two triangles in one plane: whether they overlap, more than along an
/// edge or at a corner.
fn flat_overlap(t: [Vec3; 3], u: [Vec3; 3], normal: Vec3, near: f64) -> bool {
    // Seen along the plane's way, dropping its biggest direction.
    let flat = |p: Vec3| -> (f64, f64) {
        let [x, y, z] = [normal.x.abs(), normal.y.abs(), normal.z.abs()];
        if x >= y && x >= z {
            (p.y, p.z)
        } else if y >= z {
            (p.z, p.x)
        } else {
            (p.x, p.y)
        }
    };
    let (t, u) = (t.map(flat), u.map(flat));
    let turn = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        let length = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let value = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if value.abs() <= near * length {
            0.0
        } else {
            value
        }
    };
    let strictly_inside = |p: (f64, f64), tri: [(f64, f64); 3]| {
        let sides = [
            turn(tri[0], tri[1], p),
            turn(tri[1], tri[2], p),
            turn(tri[2], tri[0], p),
        ];
        sides.iter().all(|&s| s > 0.0) || sides.iter().all(|&s| s < 0.0)
    };
    for at in 0..3 {
        let (a, b) = (t[at], t[(at + 1) % 3]);
        for step in 0..3 {
            let (c, d) = (u[step], u[(step + 1) % 3]);
            if turn(a, b, c) * turn(a, b, d) < 0.0 && turn(c, d, a) * turn(c, d, b) < 0.0 {
                return true;
            }
        }
    }
    let middle = |tri: [(f64, f64); 3]| {
        (
            (tri[0].0 + tri[1].0 + tri[2].0) / 3.0,
            (tri[0].1 + tri[1].1 + tri[2].1) / 3.0,
        )
    };
    u.iter().any(|&p| strictly_inside(p, t))
        || t.iter().any(|&p| strictly_inside(p, u))
        || strictly_inside(middle(u), t)
        || strictly_inside(middle(t), u)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pull(faces: &[&str], by: f64, name: Option<&str>) -> Change {
        Change::Pull {
            faces: faces.iter().map(|face| (*face).to_owned()).collect(),
            by,
            name: name.map(str::to_owned),
        }
    }

    fn inset(faces: &[&str], by: f64, name: Option<&str>) -> Change {
        Change::Inset {
            faces: faces.iter().map(|face| (*face).to_owned()).collect(),
            by,
            name: name.map(str::to_owned),
        }
    }

    #[test]
    fn a_plain_box_is_closed_and_holds_its_volume() {
        let shaped = shape_box([2.0, 1.0, 0.5], [1.0; 2], &[], None).unwrap();
        assert!((shaped.volume() - 1.0).abs() < 1e-12);
        assert!(shaped.check(&[0, 1, 2, 3, 4, 5], "nothing", 2.0).is_ok());
        for face in 0..6 {
            let normal = shaped.normal(face).unwrap();
            let word = shaped.faces[face].facing.name();
            let expected: Vec3 = Side::from_name(word).unwrap().facing().into();
            assert!((normal - expected).length() < 1e-12, "{word}");
        }
    }

    #[test]
    fn the_box_sides_name_their_neighbours() {
        let shaped = Shaped::boxed([1.0; 3], [1.0; 2]);
        for (at, face) in shaped.faces.iter().enumerate() {
            let next: Vec<&str> = shaped
                .neighbours(at)
                .iter()
                .map(|&other| shaped.faces[other].name.as_str())
                .collect();
            let words: Vec<&str> = face.sides.iter().map(|side| side.name()).collect();
            assert_eq!(next, words, "{}", face.name);
        }
    }

    #[test]
    fn a_pull_adds_four_walls_named_by_side() {
        let shaped = shape_box([1.0; 3], [1.0; 2], &[pull(&["top"], 0.5, None)], None).unwrap();
        assert_eq!(shaped.faces.len(), 10);
        assert!((shaped.volume() - 1.5).abs() < 1e-12);
        let names = shaped.own_names();
        for word in ["left", "front", "right", "back"] {
            assert!(names.contains(&format!("top_side_{word}")), "{names:?}");
        }
        assert_eq!(shaped.faces_named("top_side").len(), 4);
        // The wall along the front faces front, and its edges know their
        // sides: the bottom on the box, the top at the pulled face.
        let front = shaped.faces_named("top_side_front")[0];
        let wall = &shaped.faces[front];
        assert_eq!(wall.facing, Side::Front);
        assert_eq!(
            wall.sides,
            vec![Side::Bottom, Side::Right, Side::Top, Side::Left]
        );
        let normal = shaped.normal(front).unwrap();
        assert!((normal - Vec3::FORWARD).length() < 1e-12);
    }

    #[test]
    fn a_pull_in_makes_a_hollow() {
        let shaped = shape_box(
            [1.0; 3],
            [1.0; 2],
            &[inset(&["top"], 0.2, None), pull(&["top_inset"], -0.5, None)],
            None,
        )
        .unwrap();
        // 1 - 0.6 * 0.6 * 0.5.
        assert!((shaped.volume() - 0.82).abs() < 1e-12);
    }

    #[test]
    fn pulled_through_the_far_side_is_a_mistake() {
        let mistake = shape_box(
            [1.0, 1.0, 0.2],
            [1.0; 2],
            &[
                inset(&["front"], 0.3, Some("glass")),
                pull(&["glass"], -0.3, None),
            ],
            Some("wall"),
        )
        .unwrap_err();
        assert_eq!(mistake.change, 1);
        assert_eq!(mistake.code, Code::BadShape);
        assert!(
            mistake
                .message
                .starts_with("pulling \"glass\" in by 0.3 makes"),
            "{}",
            mistake.message
        );
    }

    #[test]
    fn an_inset_names_its_middle_and_its_ring() {
        let shaped = shape_box(
            [3.0, 2.4, 0.2],
            [1.0; 2],
            &[
                inset(&["front"], 0.6, Some("window")),
                inset(&["window"], 0.06, Some("glass")),
            ],
            None,
        )
        .unwrap();
        assert_eq!(shaped.faces.len(), 6 + 4 + 4);
        assert_eq!(shaped.faces_named("front").len(), 4);
        assert_eq!(shaped.faces_named("window").len(), 4);
        assert_eq!(shaped.faces_named("glass").len(), 1);
        assert_eq!(shaped.faces_named("window_rim_top").len(), 1);
        assert_eq!(shaped.faces_named("glass_rim_left").len(), 1);
        let glass = shaped.faces_named("glass")[0];
        let [wide, tall] = shaped.extent(glass);
        assert!((wide - 1.68).abs() < 1e-9 && (tall - 1.08).abs() < 1e-9);
        // An inset changes the shape nowhere.
        assert!((shaped.volume() - 3.0 * 2.4 * 0.2).abs() < 1e-9);
    }

    #[test]
    fn an_inset_too_wide_says_how_wide_it_may_be() {
        let mistake = shape_box(
            [3.0, 2.4, 0.2],
            [1.0; 2],
            &[
                inset(&["front"], 0.6, Some("window")),
                inset(&["window"], 0.8, None),
            ],
            None,
        )
        .unwrap_err();
        assert_eq!(mistake.change, 1);
        assert_eq!(
            mistake.message,
            "an inset of 0.8 m is too wide for \"window\": it is 1.2 m across, so the inset must be less than 0.6 m"
        );
    }

    #[test]
    fn an_unknown_face_is_named_with_the_faces_there_are() {
        let mistake = shape_box(
            [1.0; 3],
            [1.0; 2],
            &[
                inset(&["front"], 0.1, Some("glass")),
                pull(&["glas"], 0.1, None),
            ],
            Some("wall"),
        )
        .unwrap_err();
        assert_eq!(mistake.message, "there is no face \"glas\" on \"wall\"");
        let (written, names) = mistake.unknown.unwrap();
        assert_eq!(written, "glas");
        assert!(names.contains(&"glass".to_owned()));
    }

    #[test]
    fn faces_pulled_together_get_walls_only_round_the_outside() {
        let shaped = shape_box(
            [1.0; 3],
            [1.0; 2],
            &[pull(&["top", "front"], 0.2, Some("lump"))],
            None,
        )
        .unwrap();
        // Two faces in one piece, sharing an edge: six edges round the
        // outside, so six walls.
        assert_eq!(shaped.faces_named("lump").len(), 6);
        assert!(shaped.volume() > 1.0);
    }

    #[test]
    fn a_second_pull_of_one_face_numbers_its_walls() {
        let shaped = shape_box(
            [1.0; 3],
            [1.0; 2],
            &[pull(&["top"], 0.2, None), pull(&["top"], 0.2, None)],
            None,
        )
        .unwrap();
        assert_eq!(shaped.faces_named("top_side").len(), 4);
        assert_eq!(shaped.faces_named("top_side_2").len(), 4);
        assert_eq!(shaped.faces_named("top_side_front_2").len(), 1);
    }

    #[test]
    fn names_do_not_change_with_the_size_or_the_amounts() {
        let changes = |by: f64| {
            vec![
                inset(&["front"], 0.1, None),
                pull(&["front_inset"], by, None),
                inset(&["top", "back"], 0.05, None),
                pull(&["top_inset"], by / 2.0, None),
                inset(&["front_inset_side_top"], 0.01, Some("ledge")),
                pull(&["ledge", "back_inset"], by / 4.0, None),
            ]
        };
        let mut seen: Option<Vec<String>> = None;
        for size in [[1.0, 1.0, 1.0], [2.0, 0.5, 3.0], [0.4, 3.0, 0.7]] {
            for by in [0.1, 0.3, -0.1] {
                let shaped = shape_box(size, [1.0; 2], &changes(by), None).unwrap();
                let names = shaped.names();
                match &seen {
                    None => seen = Some(names),
                    Some(first) => assert_eq!(first, &names, "{size:?} {by}"),
                }
            }
        }
    }

    #[test]
    fn a_tapered_box_can_be_shaped() {
        let shaped = shape_box(
            [1.0; 3],
            [0.5, 0.5],
            &[pull(&["front"], 0.2, None), inset(&["top"], 0.1, None)],
            None,
        )
        .unwrap();
        assert!(shaped.volume() > 0.0);
    }

    #[test]
    fn polygons_carry_their_own_names() {
        let shaped = shape_box(
            [1.0; 3],
            [1.0; 2],
            &[pull(&["top"], 0.5, Some("chimney"))],
            None,
        )
        .unwrap();
        let out = shaped.polygons();
        assert_eq!(out.polygons.len(), 10);
        assert_eq!(out.names[0], "top");
        assert_eq!(
            shaped
                .groups_of(&out.names, "chimney")
                .map(|groups| groups.len()),
            Some(4)
        );
        assert_eq!(
            shaped.groups_of(&out.names, "sides").map(|g| g.len()),
            Some(4)
        );
        assert_eq!(shaped.groups_of(&out.names, "nothing"), None);
    }

    #[test]
    fn each_kind_of_triangle_meeting_is_judged_right() {
        let p = |x: f64, y: f64, z: f64| vec3(x, y, z);
        let near = 1e-9;
        // Crossed like the links of a chain.
        assert!(apart_cross(
            [p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0), p(1.0, 2.0, 0.0)],
            [p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0), p(1.0, -1.0, 0.0)],
            near
        ));
        // Only touching at a point.
        assert!(!apart_cross(
            [p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0), p(1.0, 2.0, 0.0)],
            [p(1.0, 2.0, 0.0), p(1.0, 3.0, 1.0), p(1.0, 3.0, -1.0)],
            near
        ));
        // Lying on each other, in one plane.
        assert!(apart_cross(
            [p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0), p(1.0, 2.0, 0.0)],
            [p(0.5, 0.5, 0.0), p(1.5, 0.5, 0.0), p(1.0, 3.0, 0.0)],
            near
        ));
        // A fold flat onto itself, and one that isn't.
        assert!(edge_fold(
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(0.5, 1.0, 0.0),
            p(0.2, 0.5, 0.0),
            near
        ));
        assert!(!edge_fold(
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(0.5, 1.0, 0.0),
            p(0.5, -1.0, 0.0),
            near
        ));
        // Two triangles round one corner, folded apart and folded into
        // each other.
        assert!(!corner_cross(
            [p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0)],
            [p(0.0, 0.0, 0.0), p(0.0, 0.0, 1.0), p(-1.0, 0.0, 0.0)]
        ));
        assert!(corner_cross(
            [p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0)],
            [p(0.0, 0.0, 0.0), p(1.0, 1.0, 1.0), p(1.0, 1.0, -1.0)]
        ));
    }
}
