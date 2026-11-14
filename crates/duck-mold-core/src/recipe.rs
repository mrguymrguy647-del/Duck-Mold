//! The recipe: what a file may say, read from the tree and checked.
//!
//! [`decode`] turns the tree from [`crate::source`] into a [`Recipe`]. It
//! checks everything that can be checked by looking at one file alone:
//! field names, kinds of values, ranges, fields that can't go together.
//! It never stops at the first mistake: every problem becomes an
//! [`Issue`], and a part that couldn't be read is kept as a placeholder so
//! the rest of the recipe can still be checked.
//!
//! What needs more than one file, or the built shapes, is checked later:
//! whether a colour is in the palette, whether a socket exists
//! ([`crate::assemble`]).

use std::sync::Arc;

use crate::issue::{Code, Issue};
use crate::mesh::MeshFile;
use crate::shaping::Change;
use crate::source::{Entry, Node, Pos, Value};

/// Where something was written, kept so that a problem found later can
/// still name the file, the line and the field.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Loc {
    /// The file, as it was named.
    pub file: String,
    /// The line and column.
    pub pos: Pos,
    /// The field, as a path: `parts[3].attach`.
    pub path: String,
}

impl Loc {
    /// An error at this place.
    pub fn error(&self, code: Code, message: impl Into<String>) -> Issue {
        Issue::error(code, self.path.clone(), message)
            .in_file(self.file.clone())
            .at(self.pos)
    }

    /// A warning at this place.
    pub fn warning(&self, code: Code, message: impl Into<String>) -> Issue {
        Issue::warning(code, self.path.clone(), message)
            .in_file(self.file.clone())
            .at(self.pos)
    }
}

/// A word from the recipe together with where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct Named {
    /// The word: a colour name, a part name, a file path.
    pub text: String,
    /// Where it was written.
    pub loc: Loc,
}

/// A whole recipe file.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// The file it came from, as it was named.
    pub file: String,
    /// The model's name.
    pub name: String,
    /// A sentence for people, if there is one.
    pub description: Option<String>,
    /// The colours.
    pub palette: PaletteDef,
    /// Parts defined once, to `use` many times.
    pub define: Vec<Define>,
    /// The parts of the model.
    pub parts: Vec<PartDef>,
    /// Extra sockets on the whole model.
    pub sockets: Vec<SocketDef>,
    /// Where the model's zero point is. `None` means the default: the
    /// middle of the underside.
    pub origin: Option<Origin>,
    /// The poses: parts turned and moved from where they stand at rest.
    pub poses: Vec<PoseDef>,
    /// The animations: poses at times.
    pub animations: Vec<AnimationDef>,
}

/// An animation, as the recipe writes it: a name, and the poses it goes
/// through, each with the time it is reached.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationDef {
    /// Its name.
    pub name: String,
    /// Where it was written.
    pub loc: Loc,
    /// True if it starts again after its last key (`loop = true`).
    pub looping: bool,
    /// True if the motion flows through each pose and starts and ends
    /// gently; false if it goes straight from pose to pose at a steady
    /// speed (`smooth = false`).
    pub smooth: bool,
    /// The keys, in the order of their times.
    pub keys: Vec<KeyDef>,
}

/// One key of an animation: a pose, and when it is reached.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyDef {
    /// Seconds from the animation's start.
    pub at: f64,
    /// The pose's name.
    pub pose: String,
    /// Where it was written.
    pub loc: Loc,
}

/// The fields of an animation, in the order they are written.
pub(crate) const ANIMATION_FIELDS: [&str; 3] = ["loop", "smooth", "keys"];

/// How late a key can be, in seconds: an hour.
pub const LONGEST_ANIMATION: f64 = 3600.0;

/// A pose, as the recipe writes it: a name, and a line for every part
/// that isn't at rest.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseDef {
    /// Its name.
    pub name: String,
    /// Where it was written.
    pub loc: Loc,
    /// The parts it turns or moves.
    pub parts: Vec<PosePart>,
}

/// One part in a pose.
#[derive(Debug, Clone, PartialEq)]
pub struct PosePart {
    /// The part, by path: `left_arm/elbow`.
    pub part: String,
    /// Where it was written.
    pub loc: Loc,
    /// Degrees around x, then y, then z, about the part's pivot.
    pub turn: [f64; 3],
    /// How far it is moved from where it stands at rest, in metres.
    pub move_by: [f64; 3],
}

/// The pose that is always there and can't be written: the model as the
/// recipe builds it.
pub const REST: &str = "rest";

/// Where a recipe's colours come from.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteDef {
    /// Written in the recipe (or no palette at all: an empty list).
    Inline(Vec<PaletteColor>),
    /// `palette = "palette.toml"`: a file to read them from.
    File(Named),
}

/// One named colour.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteColor {
    /// Its name.
    pub name: String,
    /// Red, green and blue, 0 to 255 each.
    pub rgb: [u8; 3],
    /// Where it was written.
    pub loc: Loc,
    /// True when it is written in the recipe being built itself. Only
    /// those get a "never used" warning: a shared palette file or a part
    /// library holds more colours than one model needs.
    pub own: bool,
}

/// A part defined once under `define`.
#[derive(Debug, Clone, PartialEq)]
pub struct Define {
    /// The name parts `use` it by.
    pub name: String,
    /// Where the name was written.
    pub loc: Loc,
    /// The part.
    pub part: PartDef,
}

/// The model's zero point, as the recipe wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct Origin {
    /// The socket it names.
    pub socket: SocketRef,
    /// Where `origin` was written.
    pub loc: Loc,
}

/// A socket, named from outside: `"bottom"` or `"trunk.bottom"`.
#[derive(Debug, Clone, PartialEq)]
pub enum SocketRef {
    /// A socket of the thing itself.
    Own(String),
    /// A socket of one of its parts: part name, then socket name.
    Part(String, String),
}

/// One part: an entry of `parts` or of `define`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartDef {
    /// Where the part was written.
    pub loc: Loc,
    /// Its name, if it has one.
    pub name: Option<String>,
    /// Where it gets its shape from.
    pub source: Source,
    /// The colour for the whole part.
    pub color: Option<Named>,
    /// Colours for single faces.
    pub colors: Vec<FaceColor>,
    /// Where it goes.
    pub placement: Placement,
    /// How much bigger or smaller, along x, y and z.
    pub scale: [f64; 3],
    /// The direction it is flipped across, if any.
    pub mirror: Option<Axis>,
    /// Its own named sockets.
    pub sockets: Vec<SocketDef>,
    /// A shape's cut edges and narrowed top, if it has them.
    pub trim: Trim,
    /// Changes made to its shape once it is built: pulls, insets...
    /// Only a box has them.
    pub changes: Vec<ChangeDef>,
    /// How far it may turn when the model is posed.
    pub limits: Limits,
}

/// How far a part may turn about its join when the model is posed: its
/// `limits`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Limits {
    /// No `limits`: it may turn any way, any amount, and move.
    #[default]
    Free,
    /// `limits = "fixed"`: it neither turns nor moves.
    Fixed,
    /// A table: around x, y and z, how far it may turn. A direction the
    /// table doesn't name doesn't turn.
    Turns([Turns; 3]),
}

/// How far a part may turn around one direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Turns {
    /// Not at all.
    No,
    /// Any amount, round and round: `"free"`.
    Any,
    /// Between two angles, in degrees: the first 0 or less, the second 0
    /// or more.
    Between(f64, f64),
}

impl Turns {
    /// How far past what is allowed an angle is: 0 if it is allowed.
    pub fn past(self, degrees: f64) -> f64 {
        match self {
            Turns::Any => 0.0,
            Turns::No => degrees.abs(),
            Turns::Between(low, high) => (low - degrees).max(degrees - high).max(0.0),
        }
    }

    /// The nearest allowed angle to this one.
    pub fn kept(self, degrees: f64) -> f64 {
        match self {
            Turns::Any => degrees,
            Turns::No => 0.0,
            Turns::Between(low, high) => degrees.clamp(low, high),
        }
    }

    /// The same, seen in a mirror across x: for a turn around y or z,
    /// which then goes the other way.
    fn flipped(self) -> Turns {
        match self {
            Turns::Between(low, high) => Turns::Between(-high + 0.0, -low + 0.0),
            other => other,
        }
    }
}

impl Limits {
    /// The words for x, y and z.
    pub const AXES: [&'static str; 3] = ["x", "y", "z"];

    /// How far the part may turn around x, y and z.
    pub fn turns(self) -> [Turns; 3] {
        match self {
            Limits::Free => [Turns::Any; 3],
            Limits::Fixed => [Turns::No; 3],
            Limits::Turns(turns) => turns,
        }
    }

    /// Whether a pose may move the part.
    pub fn may_move(self) -> bool {
        self != Limits::Fixed
    }

    /// The limits of the part's mirror image, left to right: turns
    /// around y and z go the other way.
    pub fn mirrored(self) -> Limits {
        match self {
            Limits::Turns([x, y, z]) => Limits::Turns([x, y.flipped(), z.flipped()]),
            other => other,
        }
    }

    /// In a few words: `free`, `fixed`, `x -30..40, y -80..80`, `doesn't
    /// turn`.
    pub fn describe(self) -> String {
        match self {
            Limits::Free => "free".to_owned(),
            Limits::Fixed => "fixed".to_owned(),
            Limits::Turns(turns) => {
                let named: Vec<String> = Limits::AXES
                    .iter()
                    .zip(turns)
                    .filter_map(|(axis, turns)| match turns {
                        Turns::No => None,
                        Turns::Any => Some(format!("{axis} free")),
                        Turns::Between(low, high) => {
                            Some(format!("{axis} {}..{}", short(low), short(high)))
                        }
                    })
                    .collect();
                if named.is_empty() {
                    "doesn't turn".to_owned()
                } else {
                    named.join(", ")
                }
            }
        }
    }
}

/// A number without a needless `.0`, and never `-0`.
fn short(number: f64) -> String {
    let number = (number * 1000.0).round() / 1000.0 + 0.0;
    format!("{number}")
}

/// One change to a shaped part's shape, and where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeDef {
    /// Where it was written: `parts[0].changes[2]`.
    pub loc: Loc,
    /// What it does.
    pub change: Change,
}

/// The two things that can be done to a shape after it is built: its
/// edges cut off (`bevel`), and its top made narrower or wider than its
/// bottom (`taper`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trim {
    /// How much is cut off each edge, in metres. 0 for sharp edges.
    pub bevel: f64,
    /// How many flat cuts round each edge: 1 is one flat cut.
    pub bevel_steps: usize,
    /// How wide (x) and deep (z) the top is, as a share of the bottom.
    /// 1 for straight sides.
    pub taper: [f64; 2],
}

impl Default for Trim {
    fn default() -> Self {
        Trim {
            bevel: 0.0,
            bevel_steps: 1,
            taper: [1.0; 2],
        }
    }
}

impl Trim {
    /// Whether the shape's edges are cut.
    pub fn is_bevelled(&self) -> bool {
        self.bevel > 0.0
    }

    /// Whether the top is narrower or wider than the bottom.
    pub fn is_tapered(&self) -> bool {
        self.taper != [1.0; 2]
    }
}

/// Where a part gets its shape from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// One of the built-in shapes.
    Shape(Shape),
    /// A part from `define`.
    Use(Named),
    /// A file, as the recipe named it. Reading it turns this into
    /// [`Source::Recipe`] or [`Source::Mesh`].
    File(Named),
    /// A list of parts of its own: an assembly.
    Parts(Vec<PartDef>),
    /// Another recipe, read from a file.
    Recipe(Box<Recipe>),
    /// A mesh, read from a file.
    Mesh(Arc<MeshFile>),
    /// The part couldn't be read. The mistake is already reported;
    /// nothing that refers to this part is reported again.
    Broken,
}

/// A built-in shape with its sizes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// A box: width (x), height (y), depth (z).
    Box {
        /// Its three sizes.
        size: [f64; 3],
    },
    /// An upright cylinder.
    Cylinder {
        /// From the middle to the middle of a flat side.
        radius: f64,
        /// From bottom to top.
        height: f64,
        /// How many flat sides.
        segments: usize,
    },
    /// A ball.
    Sphere {
        /// From the middle to the surface.
        radius: f64,
        /// How many flat sides round its middle.
        segments: usize,
    },
    /// A cone, point at the top.
    Cone {
        /// From the middle of the base to the middle of a flat side.
        radius: f64,
        /// From base to point.
        height: f64,
        /// How many flat sides.
        segments: usize,
    },
    /// A ramp: full height at the back, sloping down to the front edge.
    Wedge {
        /// Its three sizes.
        size: [f64; 3],
    },
    /// An upright cylinder with round ends: a pill.
    Capsule {
        /// From the middle to the middle of a flat side.
        radius: f64,
        /// From bottom to top, round ends included.
        height: f64,
        /// How many flat sides round it.
        segments: usize,
    },
    /// A ring lying flat: a doughnut.
    Torus {
        /// From the middle of the hole to the middle of the ring.
        radius: f64,
        /// How thick the ring is: from its middle to its surface.
        tube: f64,
        /// How many flat sides go round the hole.
        segments: usize,
    },
    /// An upright pipe, open at both ends.
    Tube {
        /// From the middle to the middle of a flat side, outside.
        radius: f64,
        /// How thick its wall is.
        wall: f64,
        /// From bottom to top.
        height: f64,
        /// How many flat sides.
        segments: usize,
    },
    /// Four slopes meeting in a point at the top.
    Pyramid {
        /// Its three sizes.
        size: [f64; 3],
    },
    /// A roof: two slopes, left and right, meeting in a ridge that runs
    /// from front to back.
    Prism {
        /// Its three sizes.
        size: [f64; 3],
    },
    /// Steps going up from the front edge to the full height at the back.
    Stairs {
        /// Its three sizes.
        size: [f64; 3],
        /// How many steps.
        steps: usize,
    },
    /// A wall with a round-topped doorway through it, front to back.
    Arch {
        /// Its three sizes.
        size: [f64; 3],
        /// How thick it is at each side of the doorway and above it.
        wall: f64,
        /// How many flat pieces the round top is made of.
        segments: usize,
    },
}

impl Shape {
    /// The names a recipe can write in `shape`.
    pub const NAMES: [&'static str; 12] = [
        "box", "cylinder", "sphere", "cone", "wedge", "capsule", "torus", "tube", "pyramid",
        "prism", "stairs", "arch",
    ];

    /// The shape's name: `box`, `cylinder`...
    pub fn name(&self) -> &'static str {
        match self {
            Shape::Box { .. } => "box",
            Shape::Cylinder { .. } => "cylinder",
            Shape::Sphere { .. } => "sphere",
            Shape::Cone { .. } => "cone",
            Shape::Wedge { .. } => "wedge",
            Shape::Capsule { .. } => "capsule",
            Shape::Torus { .. } => "torus",
            Shape::Tube { .. } => "tube",
            Shape::Pyramid { .. } => "pyramid",
            Shape::Prism { .. } => "prism",
            Shape::Stairs { .. } => "stairs",
            Shape::Arch { .. } => "arch",
        }
    }

    /// The fields that give a shape of this name its size, and the
    /// options it can have, in the order a recipe usually writes them.
    pub fn size_fields(name: &str) -> &'static [&'static str] {
        match name {
            "box" => &["size", "bevel", "bevel_steps", "taper"],
            "cylinder" => &[
                "radius",
                "height",
                "segments",
                "bevel",
                "bevel_steps",
                "taper",
            ],
            "cone" | "capsule" => &["radius", "height", "segments"],
            "sphere" => &["radius", "segments"],
            "torus" => &["radius", "tube", "segments"],
            "tube" => &["radius", "wall", "height", "segments", "taper"],
            "wedge" | "pyramid" | "prism" => &["size"],
            "stairs" => &["size", "steps"],
            "arch" => &["size", "wall", "segments"],
            _ => &[],
        }
    }

    /// Every field that gives some shape its size or is an option of
    /// one, in the order they are read.
    pub const SIZE_FIELDS: [&'static str; 10] = [
        "size",
        "radius",
        "height",
        "segments",
        "tube",
        "wall",
        "steps",
        "bevel",
        "bevel_steps",
        "taper",
    ];

    /// The names of the faces of a shape of this name, as a recipe's
    /// `colors` can name them. `bevel` is there only once the shape has
    /// one.
    pub fn face_words(name: &str) -> &'static [&'static str] {
        match name {
            "box" => &[
                "top", "bottom", "left", "right", "front", "back", "sides", "bevel",
            ],
            "cylinder" => &["top", "bottom", "side", "bevel"],
            "sphere" | "torus" => &["side"],
            "cone" => &["side", "bottom"],
            "wedge" => &["slope", "bottom", "back", "left", "right"],
            "capsule" => &["side", "top", "bottom"],
            "tube" => &["side", "inside", "top", "bottom"],
            "pyramid" => &["front", "back", "left", "right", "bottom", "sides"],
            "prism" => &["left", "right", "front", "back", "bottom"],
            "stairs" => &["top", "front", "back", "left", "right", "bottom"],
            "arch" => &["front", "back", "left", "right", "top", "bottom", "inside"],
            _ => &[],
        }
    }
}

/// One of the three directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Left and right.
    X,
    /// Down and up.
    Y,
    /// Back and front.
    Z,
}

/// A colour for one face: `top = "grass"`.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceColor {
    /// The face's name.
    pub face: String,
    /// The palette name.
    pub color: String,
    /// Where it was written.
    pub loc: Loc,
}

/// Where a part goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// By hand: turned, then moved.
    Free {
        /// Where its centre goes.
        move_by: [f64; 3],
        /// Degrees around x, then y, then z.
        rotate: [f64; 3],
    },
    /// By sockets.
    Attached {
        /// Its own socket.
        attach: Named,
        /// The part it hangs from.
        to_part: String,
        /// That part's socket.
        to_socket: String,
        /// Where `to` was written.
        to_loc: Loc,
        /// Degrees to twist around the join: `twist`.
        twist: f64,
    },
}

impl Placement {
    /// A part that stays where it is.
    pub fn none() -> Self {
        Placement::Free {
            move_by: [0.0; 3],
            rotate: [0.0; 3],
        }
    }
}

/// A named socket, as the recipe declared it.
#[derive(Debug, Clone, PartialEq)]
pub struct SocketDef {
    /// Its name.
    pub name: String,
    /// Where it was written.
    pub loc: Loc,
    /// How its place is given.
    pub kind: SocketKind,
}

/// The three ways to say where a socket is.
#[derive(Debug, Clone, PartialEq)]
pub enum SocketKind {
    /// On a side of the box around the part.
    On {
        /// Which side.
        side: Side,
        /// How far it is slid across that side: -1 to 1 for x, y and z.
        shift: [f64; 3],
        /// The direction it faces, when that isn't straight out of the side.
        facing: Option<[f64; 3]>,
    },
    /// On a face of a shaped box, by the face's name: at the middle of
    /// the faces with that name, facing out of them.
    OnFace {
        /// The face's name.
        face: String,
        /// How far it is slid across the face: -1 to 1 for x, y and z,
        /// shares of how wide, tall and deep the face is.
        shift: [f64; 3],
        /// The direction it faces, when that isn't straight out.
        facing: Option<[f64; 3]>,
        /// Where `on` was written, for a name that turns out to be no
        /// face.
        on_loc: Loc,
    },
    /// At an exact point, in the part's own units.
    At {
        /// The point.
        at: [f64; 3],
        /// The direction it faces.
        facing: [f64; 3],
    },
    /// A socket of a part inside this one, passed on under a new name.
    From {
        /// The inner part.
        part: String,
        /// Its socket.
        socket: String,
    },
}

/// A side of the box around a part, or its middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// +y.
    Top,
    /// -y.
    Bottom,
    /// -x.
    Left,
    /// +x.
    Right,
    /// +z: the way the model faces.
    Front,
    /// -z.
    Back,
    /// The middle of the box.
    Center,
}

impl Side {
    /// Every side, in the order they are listed in messages. These are
    /// also the names of the seven sockets every part has built in.
    pub const ALL: [Side; 7] = [
        Side::Top,
        Side::Bottom,
        Side::Left,
        Side::Right,
        Side::Front,
        Side::Back,
        Side::Center,
    ];

    /// The side's name.
    pub fn name(self) -> &'static str {
        match self {
            Side::Top => "top",
            Side::Bottom => "bottom",
            Side::Left => "left",
            Side::Right => "right",
            Side::Front => "front",
            Side::Back => "back",
            Side::Center => "center",
        }
    }

    /// The side with this name.
    pub fn from_name(name: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|side| side.name() == name)
    }

    /// The side across from this one. The middle is its own.
    pub fn opposite(self) -> Side {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Front => Side::Back,
            Side::Back => Side::Front,
            Side::Center => Side::Center,
        }
    }

    /// Where the side is from the middle of the box: one step along one
    /// direction. The middle itself is `[0, 0, 0]`.
    pub fn offset(self) -> [f64; 3] {
        match self {
            Side::Top => [0.0, 1.0, 0.0],
            Side::Bottom => [0.0, -1.0, 0.0],
            Side::Left => [-1.0, 0.0, 0.0],
            Side::Right => [1.0, 0.0, 0.0],
            Side::Front => [0.0, 0.0, 1.0],
            Side::Back => [0.0, 0.0, -1.0],
            Side::Center => [0.0, 0.0, 0.0],
        }
    }

    /// The direction a socket on this side faces: straight out. The
    /// middle faces up.
    pub fn facing(self) -> [f64; 3] {
        match self {
            Side::Center => [0.0, 1.0, 0.0],
            side => side.offset(),
        }
    }
}

/// The words a recipe can write in `facing`.
const FACING_WORDS: [(&str, [f64; 3]); 6] = [
    ("up", [0.0, 1.0, 0.0]),
    ("down", [0.0, -1.0, 0.0]),
    ("left", [-1.0, 0.0, 0.0]),
    ("right", [1.0, 0.0, 0.0]),
    ("front", [0.0, 0.0, 1.0]),
    ("back", [0.0, 0.0, -1.0]),
];

/// The fields a part can have, in the order they are listed in messages.
pub(crate) const PART_FIELDS: [&str; 28] = [
    "name",
    "shape",
    "use",
    "file",
    "parts",
    "size",
    "radius",
    "tube",
    "wall",
    "height",
    "segments",
    "steps",
    "bevel",
    "bevel_steps",
    "taper",
    "color",
    "changes",
    "colors",
    "attach",
    "to",
    "twist",
    "rotate",
    "move",
    "scale",
    "mirror",
    "limits",
    "sockets",
    "note",
];

/// The fields a change can have.
const CHANGE_FIELDS: [&str; 9] = [
    "pull", "inset", "move", "turn", "scale", "cut", "by", "at", "name",
];

/// The most degrees a turn can be, either way.
const TURN_RANGE: (f64, f64) = (-36_000.0, 36_000.0);

/// "a cylinder", "an arch".
fn a_shape(name: &str) -> String {
    if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {name}")
    } else {
        format!("a {name}")
    }
}

/// The fields at the top of a recipe.
pub(crate) const TOP_FIELDS: [&str; 11] = [
    "version",
    "name",
    "description",
    "note",
    "palette",
    "define",
    "parts",
    "sockets",
    "origin",
    "poses",
    "animations",
];

/// Names kept for later phases, and what each is for.
const RESERVED: [(&str, &str); 3] = [
    ("bones", "rigs"),
    ("colliders", "collision shapes"),
    ("lods", "simpler versions of a model for far away"),
];

/// The most colours a palette can hold.
pub const MAX_PALETTE: usize = 256;
/// The smallest and biggest size, radius or height, in metres.
const SIZE_RANGE: (f64, f64) = (0.001, 1000.0);
/// How far a part can be moved, in metres.
const MOVE_RANGE: (f64, f64) = (-10_000.0, 10_000.0);
/// The smallest and biggest scale.
const SCALE_RANGE: (f64, f64) = (0.001, 1000.0);
/// The most flat sides a round shape can have.
const MAX_SEGMENTS: f64 = 64.0;
/// The narrowest and widest a top can be, as a share of the bottom.
const TAPER_RANGE: (f64, f64) = (0.01, 10.0);

/// True for a name made of letters, digits, `_` and `-`.
pub fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Reads a recipe from its tree.
///
/// `file` is the file's name for messages; `default_name` is the model's
/// name when the recipe doesn't give one. Problems are added to `issues`.
/// A recipe always comes back, however broken, so that later checks can
/// still run on what could be read.
pub fn decode(root: &Node, file: &str, default_name: &str, issues: &mut Vec<Issue>) -> Recipe {
    let mut decoder = Decoder {
        file,
        issues,
        faces_ok: false,
    };
    decoder.recipe(root, default_name)
}

/// Reads a palette file: a plain table of `name = "#RRGGBB"`.
pub fn decode_palette_file(root: &Node, file: &str, issues: &mut Vec<Issue>) -> Vec<PaletteColor> {
    let mut decoder = Decoder {
        file,
        issues,
        faces_ok: false,
    };
    decoder.palette_table(root, "")
}

/// Whether a part is an entry of a list or a definition.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    /// In a `parts` list: it has a place.
    InList,
    /// Under `define`: it has no place until it is used.
    Defined,
}

/// The fields of one table, ticked off as they are read, so that the
/// ones left over can be reported as unknown.
struct Fields<'a> {
    entries: &'a [Entry],
    taken: Vec<bool>,
    /// The table's own path: `parts[3]`. Empty for the top of the file.
    path: String,
    /// Where the table starts.
    pos: Pos,
}

impl<'a> Fields<'a> {
    fn take(&mut self, key: &str) -> Option<&'a Entry> {
        let at = self.entries.iter().position(|entry| entry.key == key)?;
        self.taken[at] = true;
        Some(&self.entries[at])
    }

    /// The path of one of this table's fields.
    fn sub(&self, key: &str) -> String {
        join(&self.path, key)
    }
}

/// `parts[3]` and `color` give `parts[3].color`.
fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

struct Decoder<'a> {
    file: &'a str,
    issues: &'a mut Vec<Issue>,
    /// True while the sockets of a part that may be a shaped box (one
    /// with `changes`, or one made from a definition) are read: there
    /// `on` can name one of its faces.
    faces_ok: bool,
}

impl Decoder<'_> {
    fn loc(&self, path: &str, pos: Pos) -> Loc {
        Loc {
            file: self.file.to_owned(),
            pos,
            path: path.to_owned(),
        }
    }

    fn error(
        &mut self,
        code: Code,
        path: &str,
        pos: Pos,
        message: impl Into<String>,
    ) -> &mut Issue {
        let issue = Issue::error(code, path, message).in_file(self.file).at(pos);
        self.issues.push(issue);
        self.issues.last_mut().expect("just pushed")
    }

    // ---- small readers ------------------------------------------------------

    /// The node as a table, or a `type_mismatch`.
    fn table<'a>(&mut self, node: &'a Node, path: &str, example: &str) -> Option<Fields<'a>> {
        match &node.value {
            Value::Table(entries) => Some(Fields {
                entries,
                taken: vec![false; entries.len()],
                path: path.to_owned(),
                pos: node.pos,
            }),
            other => {
                let message = format!("this needs {example}, not {}", other.describe());
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(other.to_got());
                None
            }
        }
    }

    /// Reports every field that wasn't read: reserved names with a "not
    /// yet", the rest as unknown, with the closest known name.
    fn finish(&mut self, fields: Fields<'_>, known: &[&str]) {
        for (entry, taken) in fields.entries.iter().zip(&fields.taken) {
            if *taken {
                continue;
            }
            let path = fields.sub(&entry.key);
            if let Some((_, purpose)) = RESERVED.iter().find(|(name, _)| *name == entry.key) {
                let message = format!(
                    "\"{}\" is reserved for {purpose} and isn't available yet: remove it",
                    entry.key
                );
                self.error(Code::Reserved, &path, entry.key_pos, message);
                continue;
            }
            let message = format!("there is no field \"{}\" here", entry.key);
            let issue = Issue::error(Code::UnknownField, path, message)
                .in_file(self.file)
                .at(entry.key_pos)
                .got(entry.key.as_str())
                .one_of(&entry.key, known.iter().copied());
            self.issues.push(issue);
        }
    }

    fn text(&mut self, node: &Node, path: &str, example: &str) -> Option<String> {
        match &node.value {
            Value::Str(text) => Some(text.clone()),
            other => {
                let message = format!(
                    "this needs text in quotes, like \"{example}\", not {}",
                    other.describe()
                );
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(other.to_got());
                None
            }
        }
    }

    fn number(&mut self, node: &Node, path: &str, example: &str) -> Option<f64> {
        match node.value.as_number() {
            Some(number) => Some(number),
            None => {
                let message = format!(
                    "this needs a number, like {example}, not {}",
                    node.value.describe()
                );
                self.error(Code::TypeMismatch, path, node.pos, message).got =
                    Some(node.value.to_got());
                None
            }
        }
    }

    /// A number that must lie between `min` and `max`.
    fn ranged(&mut self, node: &Node, path: &str, range: (f64, f64), example: &str) -> Option<f64> {
        let number = self.number(node, path, example)?;
        self.check_range(number, node.pos, path, range)
    }

    fn check_range(
        &mut self,
        number: f64,
        pos: Pos,
        path: &str,
        (min, max): (f64, f64),
    ) -> Option<f64> {
        if number < min || number > max {
            let size = if number < min { "too small" } else { "too big" };
            let message = format!("{} is {size}", plain(number));
            let issue = self.error(Code::OutOfRange, path, pos, message);
            issue.got = Some(number.into());
            issue.allowed = Some(crate::issue::Allowed::Range { min, max });
            None
        } else {
            Some(number)
        }
    }

    /// A list of exactly three numbers.
    fn three(
        &mut self,
        node: &Node,
        path: &str,
        range: (f64, f64),
        example: &str,
    ) -> Option<[f64; 3]> {
        let items = match &node.value {
            Value::Array(items) if items.len() == 3 => items,
            other => {
                let found = match other {
                    Value::Array(items) => format!("a list of {}", items.len()),
                    other => other.describe(),
                };
                let message = format!("this needs three numbers, like {example}, not {found}");
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(other.to_got());
                return None;
            }
        };
        let mut numbers = [0.0; 3];
        let mut good = true;
        for (at, item) in items.iter().enumerate() {
            match self.ranged(item, &format!("{path}[{at}]"), range, "0.5") {
                Some(number) => numbers[at] = number,
                None => good = false,
            }
        }
        good.then_some(numbers)
    }

    /// A name: letters, digits, `_` and `-`.
    fn name(&mut self, node: &Node, path: &str) -> Option<String> {
        let text = self.text(node, path, "left_arm")?;
        if is_name(&text) {
            Some(text)
        } else {
            let message =
                format!("\"{text}\" can't be a name: use letters, digits, \"_\" and \"-\" only");
            self.error(Code::TypeMismatch, path, node.pos, message).got = Some(text.into());
            None
        }
    }

    /// `"part.socket"` as its two halves.
    fn part_socket(&mut self, node: &Node, path: &str) -> Option<(String, String)> {
        let text = self.text(node, path, "body.neck")?;
        match text.split_once('.') {
            Some((part, socket)) if is_name(part) && is_name(socket) => {
                Some((part.to_owned(), socket.to_owned()))
            }
            _ => {
                let message = format!(
                    "\"{text}\" should be a part's name, a dot, and a socket's name, like \"body.neck\""
                );
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(text.into());
                None
            }
        }
    }

    // ---- the recipe -----------------------------------------------------------

    fn recipe(&mut self, root: &Node, default_name: &str) -> Recipe {
        let mut recipe = Recipe {
            file: self.file.to_owned(),
            name: default_name.to_owned(),
            description: None,
            palette: PaletteDef::Inline(Vec::new()),
            define: Vec::new(),
            parts: Vec::new(),
            sockets: Vec::new(),
            origin: None,
            poses: Vec::new(),
            animations: Vec::new(),
        };
        let Some(mut fields) = self.table(root, "", "a recipe: a table of fields") else {
            return recipe;
        };

        match fields.take("version") {
            None => {
                self.error(
                    Code::MissingField,
                    "version",
                    fields.pos,
                    "a recipe needs a version: add `version = 0` at the top",
                );
            }
            Some(entry) => match entry.node.value {
                Value::Int(crate::RECIPE_VERSION) => {}
                Value::Int(other) => {
                    let message = format!(
                        "this program reads recipe version {}, not {other}",
                        crate::RECIPE_VERSION
                    );
                    self.error(Code::UnsupportedVersion, "version", entry.node.pos, message)
                        .got = Some(other.into());
                }
                ref other => {
                    let message = format!(
                        "the version is the whole number {}, not {}",
                        crate::RECIPE_VERSION,
                        other.describe()
                    );
                    self.error(Code::TypeMismatch, "version", entry.node.pos, message)
                        .got = Some(other.to_got());
                }
            },
        }

        if let Some(entry) = fields.take("name")
            && let Some(name) = self.text(&entry.node, "name", "robot")
        {
            if name.trim().is_empty() {
                self.error(
                    Code::TypeMismatch,
                    "name",
                    entry.node.pos,
                    "the name is empty",
                );
            } else {
                recipe.name = name;
            }
        }
        if let Some(entry) = fields.take("description") {
            recipe.description = self.text(&entry.node, "description", "A small robot.");
        }
        // A note is for people: anything goes.
        fields.take("note");

        if let Some(entry) = fields.take("palette") {
            recipe.palette = match &entry.node.value {
                Value::Str(path) => PaletteDef::File(Named {
                    text: path.clone(),
                    loc: self.loc("palette", entry.node.pos),
                }),
                _ => PaletteDef::Inline(self.palette_table(&entry.node, "palette")),
            };
        }

        if let Some(entry) = fields.take("define")
            && let Some(defines) = self.table(&entry.node, "define", "a table of parts")
        {
            for define in defines.entries {
                let path = join("define", &define.key);
                if !is_name(&define.key) {
                    let message = format!(
                        "\"{}\" can't be a name: use letters, digits, \"_\" and \"-\" only",
                        define.key
                    );
                    self.error(Code::TypeMismatch, &path, define.key_pos, message);
                }
                let part = self.part(&define.node, &path, Place::Defined);
                recipe.define.push(Define {
                    name: define.key.clone(),
                    loc: self.loc(&path, define.key_pos),
                    part,
                });
            }
        }

        match fields.take("parts") {
            None => {
                self.error(
                    Code::MissingField,
                    "parts",
                    fields.pos,
                    "a recipe needs at least one part: add a [[parts]] section",
                );
            }
            Some(entry) => recipe.parts = self.part_list(&entry.node, "parts"),
        }

        if let Some(entry) = fields.take("sockets") {
            recipe.sockets = self.sockets(&entry.node, "sockets");
        }

        if let Some(entry) = fields.take("origin")
            && let Some(text) = self.text(&entry.node, "origin", "bottom")
        {
            let socket = match text.split_once('.') {
                None if is_name(&text) => Some(SocketRef::Own(text.clone())),
                Some((part, socket)) if is_name(part) && is_name(socket) => {
                    Some(SocketRef::Part(part.to_owned(), socket.to_owned()))
                }
                _ => {
                    let message = format!(
                        "\"{text}\" should be a socket's name, like \"bottom\", or a part and its socket, like \"trunk.bottom\""
                    );
                    self.error(Code::TypeMismatch, "origin", entry.node.pos, message);
                    None
                }
            };
            recipe.origin = socket.map(|socket| Origin {
                socket,
                loc: self.loc("origin", entry.node.pos),
            });
        }

        if let Some(entry) = fields.take("poses") {
            recipe.poses = self.poses(&entry.node, "poses");
        }
        if let Some(entry) = fields.take("animations") {
            recipe.animations = self.animations(&entry.node, "animations");
        }

        self.finish(fields, &TOP_FIELDS);
        recipe
    }

    /// Reads `poses`: a table of poses by name, each a table of parts
    /// with their turn (three numbers), or `{ turn = ..., move = ... }`.
    fn poses(&mut self, node: &Node, path: &str) -> Vec<PoseDef> {
        let Some(table) = self.table(node, path, "a table of poses by name, like [poses.wave]")
        else {
            return Vec::new();
        };
        let mut poses = Vec::new();
        for pose in table.entries {
            let pose_path = join(path, &pose.key);
            if pose.key == REST {
                self.error(
                    Code::Reserved,
                    &pose_path,
                    pose.key_pos,
                    "\"rest\" is the model as the recipe builds it, and is always there: it can't be written as a pose. Give this pose another name",
                );
                continue;
            }
            if !is_name(&pose.key) {
                let message = format!(
                    "\"{}\" can't be a name: use letters, digits, \"_\" and \"-\" only",
                    pose.key
                );
                self.error(Code::TypeMismatch, &pose_path, pose.key_pos, message);
                continue;
            }
            let Some(lines) = self.table(
                &pose.node,
                &pose_path,
                "a table of parts and how each is turned, like left_arm = [0, 0, -150]",
            ) else {
                continue;
            };
            let mut parts = Vec::new();
            for line in lines.entries {
                let part_path = join(&pose_path, &line.key);
                let (turn, move_by) = match &line.node.value {
                    Value::Table(_) => {
                        let Some(mut fields) =
                            self.table(&line.node, &part_path, "{ turn = [0, 30, 0] }")
                        else {
                            continue;
                        };
                        let turn = fields.take("turn").map(|entry| {
                            self.three(
                                &entry.node,
                                &join(&part_path, "turn"),
                                TURN_RANGE,
                                "[0, 30, 0]",
                            )
                        });
                        let move_by = fields.take("move").map(|entry| {
                            self.three(
                                &entry.node,
                                &join(&part_path, "move"),
                                MOVE_RANGE,
                                "[0, 0.2, 0]",
                            )
                        });
                        if turn.is_none() && move_by.is_none() {
                            self.error(
                                Code::MissingField,
                                &part_path,
                                line.node.pos,
                                "a part in a pose needs \"turn\", \"move\" or both, like { turn = [0, 30, 0] }",
                            );
                        }
                        self.finish(fields, &["turn", "move"]);
                        match (turn, move_by) {
                            (Some(None), _) | (_, Some(None)) | (None, None) => continue,
                            (turn, move_by) => (
                                turn.flatten().unwrap_or([0.0; 3]),
                                move_by.flatten().unwrap_or([0.0; 3]),
                            ),
                        }
                    }
                    Value::Array(_) => {
                        match self.three(&line.node, &part_path, TURN_RANGE, "[0, 30, 0]") {
                            Some(turn) => (turn, [0.0; 3]),
                            None => continue,
                        }
                    }
                    other => {
                        let message = format!(
                            "a part in a pose is its turn, three numbers in degrees like [0, 30, 0], or {{ turn = ..., move = ... }}, not {}",
                            other.describe()
                        );
                        self.error(Code::TypeMismatch, &part_path, line.node.pos, message)
                            .got = Some(other.to_got());
                        continue;
                    }
                };
                parts.push(PosePart {
                    part: line.key.clone(),
                    loc: self.loc(&part_path, line.key_pos),
                    turn,
                    move_by,
                });
            }
            poses.push(PoseDef {
                name: pose.key.clone(),
                loc: self.loc(&pose_path, pose.key_pos),
                parts,
            });
        }
        poses
    }

    /// `true` or `false`.
    fn yes_or_no(&mut self, node: &Node, path: &str) -> Option<bool> {
        match &node.value {
            Value::Bool(yes) => Some(*yes),
            other => {
                let message = format!("this needs true or false, not {}", other.describe());
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(other.to_got());
                None
            }
        }
    }

    /// Reads `animations`: a table of animations by name, each with its
    /// `keys` (a pose and the time it is reached), and `loop` and
    /// `smooth` if they are written.
    fn animations(&mut self, node: &Node, path: &str) -> Vec<AnimationDef> {
        let Some(table) = self.table(
            node,
            path,
            "a table of animations by name, like [animations.walk]",
        ) else {
            return Vec::new();
        };
        let mut animations = Vec::new();
        for animation in table.entries {
            let own_path = join(path, &animation.key);
            if !is_name(&animation.key) {
                let message = format!(
                    "\"{}\" can't be a name: use letters, digits, \"_\" and \"-\" only",
                    animation.key
                );
                self.error(Code::TypeMismatch, &own_path, animation.key_pos, message);
                continue;
            }
            let Some(mut fields) = self.table(
                &animation.node,
                &own_path,
                "a table with the animation's keys, like keys = [{ at = 0, pose = \"rest\" }]",
            ) else {
                continue;
            };
            let looping = fields
                .take("loop")
                .and_then(|entry| self.yes_or_no(&entry.node, &join(&own_path, "loop")))
                .unwrap_or(false);
            let smooth = fields
                .take("smooth")
                .and_then(|entry| self.yes_or_no(&entry.node, &join(&own_path, "smooth")))
                .unwrap_or(true);
            let keys = match fields.take("keys") {
                Some(entry) => self.keys(&entry.node, &join(&own_path, "keys")),
                None => Vec::new(),
            };
            self.finish(fields, &ANIMATION_FIELDS);
            animations.push(AnimationDef {
                name: animation.key.clone(),
                loc: self.loc(&own_path, animation.key_pos),
                looping,
                smooth,
                keys,
            });
        }
        animations
    }

    /// Reads an animation's `keys`: a list of `{ at = seconds, pose =
    /// "name" }`. The first is at 0 and each is later than the one
    /// before.
    fn keys(&mut self, node: &Node, path: &str) -> Vec<KeyDef> {
        let Value::Array(items) = &node.value else {
            let message = format!(
                "this needs a list of keys, like [{{ at = 0, pose = \"rest\" }}], not {}",
                node.value.describe()
            );
            self.error(Code::TypeMismatch, path, node.pos, message).got = Some(node.value.to_got());
            return Vec::new();
        };
        let mut keys: Vec<KeyDef> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let Some(mut fields) = self.table(
                item,
                &item_path,
                "a key, like { at = 0.4, pose = \"wave\" }",
            ) else {
                continue;
            };
            let at = match fields.take("at") {
                Some(entry) => self.ranged(
                    &entry.node,
                    &join(&item_path, "at"),
                    (0.0, LONGEST_ANIMATION),
                    "0.4",
                ),
                None => {
                    self.error(
                        Code::MissingField,
                        &item_path,
                        item.pos,
                        "a key needs \"at\", its time in seconds, like at = 0.4",
                    );
                    None
                }
            };
            let pose = match fields.take("pose") {
                Some(entry) => self.text(&entry.node, &join(&item_path, "pose"), "wave"),
                None => {
                    self.error(
                        Code::MissingField,
                        &item_path,
                        item.pos,
                        "a key needs \"pose\", the name of the pose it reaches, like pose = \"wave\"",
                    );
                    None
                }
            };
            self.finish(fields, &["at", "pose"]);
            let (Some(at), Some(pose)) = (at, pose) else {
                continue;
            };
            match keys.last() {
                None if at != 0.0 => {
                    let message = format!(
                        "an animation starts with a key at 0, and this first key is at {}. Put it at 0, or add a key at 0 before it",
                        plain(at)
                    );
                    let issue =
                        self.error(Code::OutOfRange, &join(&item_path, "at"), item.pos, message);
                    issue.got = Some(at.into());
                    issue.suggestion = Some("at = 0".to_owned());
                }
                Some(before) if at <= before.at => {
                    let message = format!(
                        "each key is later than the one before it: this one is at {} and the one before is at {}",
                        plain(at),
                        plain(before.at)
                    );
                    self.error(Code::OutOfRange, &join(&item_path, "at"), item.pos, message)
                        .got = Some(at.into());
                    continue;
                }
                _ => {}
            }
            keys.push(KeyDef {
                at,
                pose,
                loc: self.loc(&item_path, item.pos),
            });
        }
        keys
    }

    /// A table of `name = "#RRGGBB"`.
    fn palette_table(&mut self, node: &Node, path: &str) -> Vec<PaletteColor> {
        let Some(fields) = self.table(
            node,
            path,
            "a table of colours, or the path of a palette file in quotes",
        ) else {
            return Vec::new();
        };
        let mut colors = Vec::new();
        for entry in fields.entries {
            let color_path = join(path, &entry.key);
            if !is_name(&entry.key) {
                let message = format!(
                    "\"{}\" can't be a colour's name: use letters, digits, \"_\" and \"-\" only",
                    entry.key
                );
                self.error(Code::TypeMismatch, &color_path, entry.key_pos, message);
                continue;
            }
            let Some(text) = self.text(&entry.node, &color_path, "#B07A3F") else {
                continue;
            };
            match parse_hex(&text) {
                Ok(rgb) => colors.push(PaletteColor {
                    name: entry.key.clone(),
                    rgb,
                    loc: self.loc(&color_path, entry.key_pos),
                    own: true,
                }),
                Err(HexError::SeeThrough) => {
                    let message = format!(
                        "\"{text}\" is a see-through colour, which isn't available yet: write it as \"#RRGGBB\""
                    );
                    self.error(Code::Unsupported, &color_path, entry.node.pos, message)
                        .got = Some(text.into());
                }
                Err(HexError::NotHex) => {
                    let message =
                        format!("\"{text}\" isn't a colour: write \"#RRGGBB\", like \"#B07A3F\"");
                    self.error(Code::TypeMismatch, &color_path, entry.node.pos, message)
                        .got = Some(text.into());
                }
            }
        }
        if colors.len() > MAX_PALETTE {
            let message = format!(
                "a palette holds at most {MAX_PALETTE} colours; this one has {}",
                colors.len()
            );
            self.error(Code::OutOfRange, path, node.pos, message);
            colors.truncate(MAX_PALETTE);
        }
        colors
    }

    /// A `parts` list.
    fn part_list(&mut self, node: &Node, path: &str) -> Vec<PartDef> {
        let items = match &node.value {
            Value::Array(items) => items,
            other => {
                let message = format!(
                    "this needs a list of parts ([[parts]] sections in TOML), not {}",
                    other.describe()
                );
                self.error(Code::TypeMismatch, path, node.pos, message);
                return Vec::new();
            }
        };
        if items.is_empty() {
            self.error(
                Code::MissingField,
                path,
                node.pos,
                "this list of parts is empty: it needs at least one part",
            );
        }
        let mut parts: Vec<PartDef> = Vec::new();
        for (at, item) in items.iter().enumerate() {
            let part = self.part(item, &format!("{path}[{at}]"), Place::InList);
            if let Some(name) = &part.name
                && let Some(first) = parts
                    .iter()
                    .position(|other| other.name.as_ref() == Some(name))
            {
                let message = format!(
                    "two parts are called \"{name}\" ({path}[{first}] is the other): give one another name"
                );
                self.error(
                    Code::DuplicateName,
                    &join(&part.loc.path, "name"),
                    part.loc.pos,
                    message,
                );
            }
            parts.push(part);
        }
        parts
    }

    fn part(&mut self, node: &Node, path: &str, place: Place) -> PartDef {
        let mut part = PartDef {
            loc: self.loc(path, node.pos),
            name: None,
            source: Source::Broken,
            color: None,
            colors: Vec::new(),
            placement: Placement::none(),
            scale: [1.0; 3],
            mirror: None,
            sockets: Vec::new(),
            trim: Trim::default(),
            changes: Vec::new(),
            limits: Limits::Free,
        };
        let Some(mut fields) = self.table(node, path, "a part: a table of fields") else {
            return part;
        };

        // ---- the name
        if let Some(entry) = fields.take("name") {
            match place {
                Place::InList => part.name = self.name(&entry.node, &fields.sub("name")),
                Place::Defined => {
                    self.error(
                        Code::Conflict,
                        &fields.sub("name"),
                        entry.key_pos,
                        "a defined part is named by its place under `define`: remove \"name\"",
                    );
                }
            }
        }

        // ---- where the shape comes from: exactly one of four fields
        let sources: Vec<&Entry> = ["shape", "use", "file", "parts"]
            .into_iter()
            .filter_map(|key| fields.take(key))
            .collect();
        let shape_name = match sources.as_slice() {
            [] => {
                self.error(
                    Code::MissingField,
                    path,
                    fields.pos,
                    "a part needs one of \"shape\", \"use\", \"file\" or \"parts\" to say what it is, like shape = \"box\"",
                );
                None
            }
            [only] => {
                let (source, shape_name) = self.source(only, &fields);
                part.source = source;
                shape_name
            }
            [first, rest @ ..] => {
                for extra in rest {
                    let message = format!(
                        "a part gets its shape from one place: it has \"{}\" already, so remove \"{}\"",
                        first.key, extra.key
                    );
                    self.error(
                        Code::Conflict,
                        &fields.sub(&extra.key),
                        extra.key_pos,
                        message,
                    );
                }
                None
            }
        };

        // ---- the shape's sizes
        let size_fields = shape_name.as_deref().map(Shape::size_fields);
        let mut sizes: [Option<&Entry>; 10] = [None; 10];
        for (slot, key) in sizes.iter_mut().zip(Shape::SIZE_FIELDS) {
            let Some(entry) = fields.take(key) else {
                continue;
            };
            match size_fields {
                Some(allowed) if allowed.contains(&key) => *slot = Some(entry),
                Some(allowed) => {
                    let shape = shape_name.as_deref().unwrap_or_default();
                    let message = match key {
                        "bevel" | "bevel_steps" => format!(
                            "a {shape} can't have \"{key}\": only a box and a cylinder have edges to cut"
                        ),
                        "taper" => format!(
                            "a {shape} can't have \"taper\": only a box, a cylinder and a tube can be made narrower at the top"
                        ),
                        _ => {
                            let own: Vec<&str> = allowed
                                .iter()
                                .copied()
                                .filter(|field| !["bevel", "bevel_steps", "taper"].contains(field))
                                .collect();
                            format!(
                                "a {shape} has no \"{key}\": its size is given by {}",
                                quoted_list(&own)
                            )
                        }
                    };
                    self.error(Code::UnknownField, &fields.sub(key), entry.key_pos, message);
                }
                None if sources.len() == 1 && sources[0].key != "shape" => {
                    let message = format!(
                        "\"{key}\" belongs to a built-in shape; this part gets its shape from \"{}\". To change its size, use \"scale\"",
                        sources[0].key
                    );
                    self.error(Code::Conflict, &fields.sub(key), entry.key_pos, message);
                }
                // The shape itself is missing or wrong, and that is
                // already reported.
                None => {}
            }
        }
        if let Some(name) = shape_name.as_deref() {
            let (shape, trim) = self.shape(name, sizes, &fields);
            part.source = Source::Shape(shape);
            part.trim = trim;
        }

        // ---- changes to the shape
        if let Some(entry) = fields.take("changes") {
            let changes_path = fields.sub("changes");
            part.changes = self.changes(&entry.node, &changes_path);
            if !part.changes.is_empty() && !self.can_be_shaped(&part, &changes_path, entry) {
                part.changes.clear();
            }
        }

        // ---- colour
        if let Some(entry) = fields.take("color") {
            let color_path = fields.sub("color");
            if let Some(text) = self.text(&entry.node, &color_path, "wood") {
                part.color = Some(Named {
                    text,
                    loc: self.loc(&color_path, entry.node.pos),
                });
            }
        }
        if let Some(entry) = fields.take("colors") {
            let colors_path = fields.sub("colors");
            if let Some(table) = self.table(
                &entry.node,
                &colors_path,
                "a table of face names and colours, like { top = \"grass\" }",
            ) {
                for face in table.entries {
                    let face_path = join(&colors_path, &face.key);
                    if let Some(color) = self.text(&face.node, &face_path, "grass") {
                        part.colors.push(FaceColor {
                            face: face.key.clone(),
                            color,
                            loc: self.loc(&face_path, face.node.pos),
                        });
                    }
                }
            }
        }

        // ---- where it goes
        part.placement = self.placement(&mut fields, place);

        if let Some(entry) = fields.take("scale") {
            let scale_path = fields.sub("scale");
            if place == Place::Defined {
                self.defined_has_no(&scale_path, entry, "scale");
            } else if let Some(scale) = self.scale(&entry.node, &scale_path) {
                part.scale = scale;
            }
        }
        if let Some(entry) = fields.take("mirror") {
            let mirror_path = fields.sub("mirror");
            if place == Place::Defined {
                self.defined_has_no(&mirror_path, entry, "mirror");
            } else if let Some(text) = self.text(&entry.node, &mirror_path, "x") {
                part.mirror = match text.as_str() {
                    "x" => Some(Axis::X),
                    "y" => Some(Axis::Y),
                    "z" => Some(Axis::Z),
                    other => {
                        let message = format!(
                            "a part is mirrored across \"x\" (left and right), \"y\" (up and down) or \"z\" (front and back), not \"{other}\""
                        );
                        let issue = Issue::error(Code::TypeMismatch, mirror_path, message)
                            .in_file(self.file)
                            .at(entry.node.pos)
                            .got(other)
                            .one_of(&other.to_lowercase(), ["x", "y", "z"]);
                        self.issues.push(issue);
                        None
                    }
                };
            }
        }

        if let Some(entry) = fields.take("limits") {
            let limits_path = fields.sub("limits");
            if place == Place::Defined {
                self.defined_has_no(&limits_path, entry, "limits");
            } else if let Some(limits) = self.limits(&entry.node, &limits_path) {
                part.limits = limits;
            }
        }

        if let Some(entry) = fields.take("sockets") {
            self.faces_ok = !part.changes.is_empty() || matches!(part.source, Source::Use(_));
            part.sockets = self.sockets(&entry.node, &fields.sub("sockets"));
            self.faces_ok = false;
        }
        fields.take("note");

        self.finish(fields, &PART_FIELDS);
        part
    }

    /// Whether the part can have `changes`: only a box, with no bevel,
    /// can. Says why not.
    fn can_be_shaped(&mut self, part: &PartDef, path: &str, entry: &Entry) -> bool {
        let who = match &part.name {
            Some(name) => format!("\"{name}\""),
            None => "this part".to_owned(),
        };
        let (code, message) = match &part.source {
            Source::Shape(Shape::Box { .. }) if part.trim.is_bevelled() => (
                Code::Conflict,
                "a box with \"changes\" can't have a \"bevel\" in this version: remove one of them"
                    .to_owned(),
            ),
            Source::Shape(Shape::Box { .. }) | Source::Broken => return true,
            Source::Shape(shape) => (
                Code::Unsupported,
                format!(
                    "only a box can be shaped in this version: {who} is {}",
                    a_shape(shape.name())
                ),
            ),
            Source::Use(named) => (
                Code::Conflict,
                format!(
                    "a part made from a definition is shaped in the definition: move \"changes\" to [define.{}]",
                    named.text
                ),
            ),
            Source::Parts(_) => (
                Code::Unsupported,
                format!("only a box can be shaped in this version: {who} is made of other parts"),
            ),
            Source::File(_) | Source::Recipe(_) | Source::Mesh(_) => (
                Code::Unsupported,
                format!("only a box can be shaped in this version: {who} comes from a file"),
            ),
        };
        self.error(code, path, entry.key_pos, message);
        false
    }

    /// The list of changes to a shape.
    fn changes(&mut self, node: &Node, path: &str) -> Vec<ChangeDef> {
        let Value::Array(items) = &node.value else {
            let message = format!(
                "this needs a list of changes, like [{{ pull = \"top\", by = 0.5 }}], not {}",
                node.value.describe()
            );
            self.error(Code::TypeMismatch, path, node.pos, message).got = Some(node.value.to_got());
            return Vec::new();
        };
        let mut changes = Vec::new();
        for (at, item) in items.iter().enumerate() {
            let item_path = format!("{path}[{at}]");
            if let Some(change) = self.change(item, &item_path) {
                changes.push(ChangeDef {
                    loc: self.loc(&item_path, item.pos),
                    change,
                });
            }
        }
        changes
    }

    /// One change: `{ pull = "top", by = 0.5 }`.
    fn change(&mut self, node: &Node, path: &str) -> Option<Change> {
        let mut fields = self.table(
            node,
            path,
            "a change: a table like { pull = \"top\", by = 0.5 }",
        )?;
        let actions: Vec<&Entry> = Change::WORDS
            .iter()
            .filter_map(|word| fields.take(word))
            .collect();
        let action = match actions.as_slice() {
            [] => {
                // What else it says is fine: only the action is missing.
                fields.take("by");
                fields.take("at");
                fields.take("name");
                self.error(
                    Code::MissingField,
                    path,
                    fields.pos,
                    "a change needs one of \"pull\", \"inset\", \"move\", \"turn\", \"scale\" or \"cut\" to say what it does, like { pull = \"top\", by = 0.5 }",
                );
                self.finish(fields, &CHANGE_FIELDS);
                return None;
            }
            [only] => *only,
            [first, rest @ ..] => {
                for extra in rest {
                    let message = format!(
                        "a change does one thing: this one is a \"{}\" already, so make the \"{}\" a change of its own",
                        first.key, extra.key
                    );
                    self.error(
                        Code::Conflict,
                        &fields.sub(&extra.key),
                        extra.key_pos,
                        message,
                    );
                }
                fields.take("by");
                fields.take("at");
                fields.take("name");
                self.finish(fields, &CHANGE_FIELDS);
                return None;
            }
        };
        let word = action.key.as_str();
        if word == "cut" {
            return self.cut(action, fields, path);
        }
        let faces = self.face_list(&action.node, &fields.sub(word), word);
        let by_path = fields.sub("by");
        let by_entry = fields.take("by");
        if by_entry.is_none() {
            let example = match word {
                "pull" => "0.5",
                "inset" => "0.1",
                "move" => "[0, 0.2, 0]",
                "turn" => "[0, 15, 0]",
                _ => "0.5",
            };
            let what = match word {
                "move" => "how far along x, y and z, in metres",
                "turn" => "degrees around x, y and z",
                "scale" => "how much bigger or smaller: one number, or one for each of x, y and z",
                _ => "how far, in metres",
            };
            let message = format!(
                "a {word} needs \"by\": {what}, like {{ {word} = \"top\", by = {example} }}"
            );
            self.error(Code::MissingField, path, fields.pos, message);
        }
        let name = match fields.take("name") {
            Some(entry) if matches!(word, "move" | "turn" | "scale") => {
                let message = format!(
                    "a {word} makes no new faces, so it has nothing to name: remove \"name\""
                );
                self.error(Code::Conflict, &fields.sub("name"), entry.key_pos, message);
                None
            }
            Some(entry) => self.name(&entry.node, &fields.sub("name")),
            None => None,
        };
        self.finish(fields, &CHANGE_FIELDS);
        let faces = faces?;
        let by = by_entry?;
        let by_pos = by.node.pos;
        match word {
            "pull" | "inset" => {
                let by = self.change_amount(by, &by_path, word)?;
                Some(if word == "pull" {
                    Change::Pull { faces, by, name }
                } else {
                    Change::Inset { faces, by, name }
                })
            }
            "move" => {
                let by = self.three(&by.node, &by_path, MOVE_RANGE, "[0, 0.2, 0]")?;
                self.changes_something(by != [0.0; 3], &by_path, by_pos, word)?;
                Some(Change::Move { faces, by })
            }
            "turn" => {
                let by = self.three(&by.node, &by_path, TURN_RANGE, "[0, 15, 0]")?;
                self.changes_something(by != [0.0; 3], &by_path, by_pos, word)?;
                Some(Change::Turn { faces, by })
            }
            _ => {
                let by_node = &by.node;
                let by = match &by_node.value {
                    Value::Array(_) => self.three(by_node, &by_path, SCALE_RANGE, "[1, 0.5, 1]")?,
                    _ => [self.ranged(by_node, &by_path, SCALE_RANGE, "0.5")?; 3],
                };
                self.changes_something(by != [1.0; 3], &by_path, by_node.pos, word)?;
                Some(Change::Scale { faces, by })
            }
        }
    }

    /// A cut: `{ cut = "right", at = 0.8 }`.
    fn cut(&mut self, action: &Entry, mut fields: Fields<'_>, path: &str) -> Option<Change> {
        let side_path = fields.sub("cut");
        let toward = match self.text(&action.node, &side_path, "right") {
            Some(word) => match Side::from_name(&word).filter(|side| *side != Side::Center) {
                Some(side) => Some(side),
                None => {
                    let message = format!(
                        "a cut faces one of the box's sides: \"top\", \"bottom\", \"left\", \"right\", \"front\" or \"back\", not \"{word}\""
                    );
                    let issue = Issue::error(Code::TypeMismatch, side_path, message)
                        .in_file(self.file)
                        .at(action.node.pos)
                        .got(word.as_str())
                        .one_of(&word, ["top", "bottom", "left", "right", "front", "back"]);
                    self.issues.push(issue);
                    None
                }
            },
            None => None,
        };
        let at_path = fields.sub("at");
        let at = match fields.take("at") {
            Some(entry) => match self.number(&entry.node, &at_path, "0.8") {
                Some(at) if (0.0..1.0).contains(&at) => Some(at),
                Some(at) => {
                    let side = toward.map_or("the side", Side::name);
                    let message = format!(
                        "the cut towards \"{side}\" at {} is outside the part: \"at\" goes from 0 (the middle) to just under 1 (the side)",
                        plain(at)
                    );
                    let issue = self.error(Code::OutOfRange, &at_path, entry.node.pos, message);
                    issue.got = Some(at.into());
                    issue.allowed = Some(crate::issue::Allowed::Range { min: 0.0, max: 1.0 });
                    None
                }
                None => None,
            },
            None => {
                self.error(
                    Code::MissingField,
                    path,
                    fields.pos,
                    "a cut needs \"at\": how far from the middle (0) to the side (just under 1), like { cut = \"right\", at = 0.8 }",
                );
                None
            }
        };
        let name = fields
            .take("name")
            .and_then(|entry| self.name(&entry.node, &fields.sub("name")));
        self.finish(fields, &CHANGE_FIELDS);
        Some(Change::Cut {
            toward: toward?,
            at: at?,
            name,
        })
    }

    /// A move, turn or scale that changes nothing is a mistake: it is
    /// surely not what was meant.
    fn changes_something(&mut self, does: bool, path: &str, pos: Pos, word: &str) -> Option<()> {
        if does {
            return Some(());
        }
        let message = match word {
            "scale" => {
                "a scale of 1 changes nothing: above 1 makes the faces bigger, below 1 smaller"
            }
            "turn" => "a turn of [0, 0, 0] changes nothing: give the degrees around x, y and z",
            _ => "a move of [0, 0, 0] changes nothing: give how far along x, y and z",
        };
        self.error(Code::OutOfRange, path, pos, message);
        None
    }

    /// A change's `by`, checked for what the change can take.
    fn change_amount(&mut self, entry: &Entry, path: &str, word: &str) -> Option<f64> {
        let by = self.number(&entry.node, path, "0.5")?;
        let pos = entry.node.pos;
        match word {
            "pull" if by == 0.0 => {
                let issue = self.error(
                    Code::OutOfRange,
                    path,
                    pos,
                    "a pull of 0 changes nothing: pull out with a number above 0, or in with one below 0",
                );
                issue.got = Some(by.into());
                None
            }
            "pull" => self.check_range(by, pos, path, (-SIZE_RANGE.1, SIZE_RANGE.1)),
            _ if by <= 0.0 => {
                let issue = self.error(
                    Code::OutOfRange,
                    path,
                    pos,
                    "an inset is measured in from the edges: it needs a number above 0",
                );
                issue.got = Some(by.into());
                None
            }
            _ => self.check_range(by, pos, path, (0.0, SIZE_RANGE.1)),
        }
    }

    /// The faces a change is made to: one name, or a list of them.
    fn face_list(&mut self, node: &Node, path: &str, word: &str) -> Option<Vec<String>> {
        let items: Vec<(&Node, String)> = match &node.value {
            Value::Str(_) => vec![(node, path.to_owned())],
            Value::Array(items) if !items.is_empty() => items
                .iter()
                .enumerate()
                .map(|(at, item)| (item, format!("{path}[{at}]")))
                .collect(),
            other => {
                let found = match other {
                    Value::Array(_) => "an empty list".to_owned(),
                    other => other.describe(),
                };
                let message = format!(
                    "this needs a face's name, like \"top\", or a list of them, like [\"top\", \"front\"], not {found}"
                );
                self.error(Code::TypeMismatch, path, node.pos, message).got = Some(other.to_got());
                return None;
            }
        };
        let mut faces = Vec::new();
        let mut good = true;
        for (item, item_path) in items {
            let Some(text) = self.text(item, &item_path, "top") else {
                good = false;
                continue;
            };
            if text.contains('+') && word == "move" {
                // An edge or a corner: faces' names joined with `+`.
                if text.split('+').all(is_name) {
                    faces.push(text);
                } else {
                    let message = format!(
                        "\"{text}\" isn't an edge or a corner: write the names of the faces that meet there, joined with \"+\", like \"top+front\""
                    );
                    self.error(Code::TypeMismatch, &item_path, item.pos, message)
                        .got = Some(text.into());
                    good = false;
                }
            } else if text.contains('+') {
                let what = if text.matches('+').count() == 1 {
                    "an edge"
                } else {
                    "a corner"
                };
                let message =
                    format!("\"{text}\" is {what}: a {word} is made to faces, like \"top\"");
                self.error(Code::TypeMismatch, &item_path, item.pos, message)
                    .got = Some(text.into());
                good = false;
            } else if !is_name(&text) {
                let message = format!(
                    "\"{text}\" can't be a face's name: use letters, digits, \"_\" and \"-\" only"
                );
                self.error(Code::TypeMismatch, &item_path, item.pos, message)
                    .got = Some(text.into());
                good = false;
            } else {
                faces.push(text);
            }
        }
        good.then_some(faces)
    }

    fn defined_has_no(&mut self, path: &str, entry: &Entry, field: &str) {
        let message = format!(
            "a defined part has no place or size of its own: put \"{field}\" on the part that uses it"
        );
        self.error(Code::Conflict, path, entry.key_pos, message);
    }

    /// Reads `limits`: `"fixed"`, or a table of the directions the part
    /// may turn around, each with two angles or `"free"`.
    fn limits(&mut self, node: &Node, path: &str) -> Option<Limits> {
        const EXAMPLE: &str =
            "\"fixed\" (it never moves), or the directions it may turn, like { x = [-30, 40] }";
        let table = match &node.value {
            Value::Str(word) if word == "fixed" => return Some(Limits::Fixed),
            Value::Str(word) => {
                let message = format!("\"limits\" is {EXAMPLE}, not \"{word}\"");
                let issue = Issue::error(Code::TypeMismatch, path, message)
                    .in_file(self.file)
                    .at(node.pos)
                    .got(word.as_str())
                    .one_of(word, ["fixed"]);
                self.issues.push(issue);
                return None;
            }
            _ => self.table(node, path, EXAMPLE)?,
        };
        let mut turns = [Turns::No; 3];
        let mut good = true;
        for entry in table.entries {
            let axis_path = join(path, &entry.key);
            let Some(axis) = Limits::AXES.iter().position(|axis| *axis == entry.key) else {
                let message = format!(
                    "a part turns around \"x\" (a nod), \"y\" (a turn) or \"z\" (a tilt), not \"{}\"",
                    entry.key
                );
                let issue = Issue::error(Code::UnknownField, axis_path, message)
                    .in_file(self.file)
                    .at(entry.key_pos)
                    .got(entry.key.as_str())
                    .one_of(&entry.key.to_lowercase(), Limits::AXES);
                self.issues.push(issue);
                good = false;
                continue;
            };
            let how = "two angles in degrees, like [-30, 40], or \"free\" for any amount";
            turns[axis] = match &entry.node.value {
                Value::Str(word) if word == "free" => Turns::Any,
                Value::Array(items) if items.len() == 2 => {
                    let low = self.ranged(&items[0], &axis_path, TURN_RANGE, "-30");
                    let high = self.ranged(&items[1], &axis_path, TURN_RANGE, "40");
                    match low.zip(high) {
                        Some((low, high)) if low <= 0.0 && high >= 0.0 => Turns::Between(low, high),
                        Some((low, high)) => {
                            let message = format!(
                                "the part stands at 0 when the model is at rest, so its limits must have 0 between them: the first 0 or less and the second 0 or more, not [{}, {}]",
                                short(low),
                                short(high)
                            );
                            self.error(Code::OutOfRange, &axis_path, entry.node.pos, message);
                            good = false;
                            continue;
                        }
                        None => {
                            good = false;
                            continue;
                        }
                    }
                }
                Value::Array(items) => {
                    let message = format!("a limit is {how}, not a list of {}", items.len());
                    self.error(Code::TypeMismatch, &axis_path, entry.node.pos, message);
                    good = false;
                    continue;
                }
                other => {
                    let message = format!("a limit is {how}, not {}", other.describe());
                    self.error(Code::TypeMismatch, &axis_path, entry.node.pos, message)
                        .got = Some(other.to_got());
                    good = false;
                    continue;
                }
            };
        }
        good.then_some(Limits::Turns(turns))
    }

    /// Reads the one field that says where a part's shape comes from.
    /// For `shape`, the shape's name comes back too; its sizes are read
    /// by the caller.
    fn source(&mut self, entry: &Entry, fields: &Fields<'_>) -> (Source, Option<String>) {
        let path = fields.sub(&entry.key);
        match entry.key.as_str() {
            "shape" => {
                let Some(name) = self.text(&entry.node, &path, "box") else {
                    return (Source::Broken, None);
                };
                if Shape::NAMES.contains(&name.as_str()) {
                    (Source::Broken, Some(name))
                } else {
                    let message = format!("there is no shape \"{name}\"");
                    let issue = Issue::error(Code::UnknownShape, path, message)
                        .in_file(self.file)
                        .at(entry.node.pos)
                        .got(name.as_str())
                        .one_of(&name, Shape::NAMES);
                    self.issues.push(issue);
                    (Source::Broken, None)
                }
            }
            "use" => match self.name(&entry.node, &path) {
                Some(text) => (
                    Source::Use(Named {
                        text,
                        loc: self.loc(&path, entry.node.pos),
                    }),
                    None,
                ),
                None => (Source::Broken, None),
            },
            "file" => match self.text(&entry.node, &path, "wheel.toml") {
                Some(text) => (
                    Source::File(Named {
                        text,
                        loc: self.loc(&path, entry.node.pos),
                    }),
                    None,
                ),
                None => (Source::Broken, None),
            },
            _ => (Source::Parts(self.part_list(&entry.node, &path)), None),
        }
    }

    /// A whole number between `fewest` and `most`.
    fn whole(&mut self, entry: &Entry, path: &str, (fewest, most): (f64, f64)) -> Option<usize> {
        match entry.node.value {
            Value::Int(count) => self
                .check_range(count as f64, entry.node.pos, path, (fewest, most))
                .map(|count| count as usize),
            ref other => {
                let message = format!(
                    "this needs a whole number, like 12, not {}",
                    other.describe()
                );
                self.error(Code::TypeMismatch, path, entry.node.pos, message)
                    .got = Some(other.to_got());
                None
            }
        }
    }

    /// Says that a size doesn't go with another, and gives the nearest
    /// one that does, so that the rest of the recipe is still checked.
    fn too_big(
        &mut self,
        entry: Option<&Entry>,
        path: &str,
        got: f64,
        below: f64,
        why: &str,
    ) -> f64 {
        if let Some(entry) = entry {
            let message = format!("{} is too big: {why}", plain(got));
            let issue = self.error(Code::OutOfRange, path, entry.node.pos, message);
            issue.got = Some(got.into());
        }
        below * 0.99
    }

    /// Builds a shape from its name and its size fields, each already
    /// known to belong to that shape. A missing field takes its default.
    fn shape(
        &mut self,
        name: &str,
        sizes: [Option<&Entry>; 10],
        fields: &Fields<'_>,
    ) -> (Shape, Trim) {
        let [
            size_entry,
            radius_entry,
            height_entry,
            segments,
            tube_entry,
            wall_entry,
            steps,
            bevel_entry,
            bevel_steps,
            taper,
        ] = sizes;
        let size = size_entry
            .and_then(|entry| {
                self.three(&entry.node, &fields.sub("size"), SIZE_RANGE, "[1, 2, 0.5]")
            })
            .unwrap_or([1.0; 3]);
        let radius = radius_entry
            .and_then(|entry| self.ranged(&entry.node, &fields.sub("radius"), SIZE_RANGE, "0.5"))
            .unwrap_or(if name == "capsule" { 0.25 } else { 0.5 });
        let mut height = height_entry
            .and_then(|entry| self.ranged(&entry.node, &fields.sub("height"), SIZE_RANGE, "1.5"))
            .unwrap_or(1.0);
        let (fewest, default) = match name {
            "sphere" => (4.0, 12),
            // The round top of an arch's doorway.
            "arch" => (2.0, 8),
            _ => (3.0, 16),
        };
        let segments = segments
            .and_then(|entry| self.whole(entry, &fields.sub("segments"), (fewest, MAX_SEGMENTS)))
            .unwrap_or(default);
        let steps = steps
            .and_then(|entry| self.whole(entry, &fields.sub("steps"), (1.0, MAX_SEGMENTS)))
            .unwrap_or(4);
        let mut tube = tube_entry
            .and_then(|entry| self.ranged(&entry.node, &fields.sub("tube"), SIZE_RANGE, "0.15"))
            .unwrap_or(0.15);
        let mut wall = wall_entry
            .and_then(|entry| self.ranged(&entry.node, &fields.sub("wall"), SIZE_RANGE, "0.1"))
            .unwrap_or(if name == "arch" { 0.2 } else { 0.1 });

        // ---- sizes that must fit each other
        match name {
            "capsule" if height < radius * 2.0 => {
                let entry = height_entry.or(radius_entry);
                let key = if height_entry.is_some() {
                    "height"
                } else {
                    "radius"
                };
                if let Some(entry) = entry {
                    let message = format!(
                        "a capsule's height includes its two round ends, so it is at least twice its radius: {} or more",
                        plain(radius * 2.0)
                    );
                    let issue =
                        self.error(Code::OutOfRange, &fields.sub(key), entry.node.pos, message);
                    issue.got = Some(height.into());
                }
                height = radius * 2.0;
            }
            "torus" if tube >= radius => {
                tube = self.too_big(
                    tube_entry.or(radius_entry),
                    &fields.sub(if tube_entry.is_some() { "tube" } else { "radius" }),
                    tube,
                    radius,
                    &format!(
                        "the ring would close its own hole. \"tube\" must be less than \"radius\" ({})",
                        plain(radius)
                    ),
                );
            }
            "tube" if wall >= radius => {
                wall = self.too_big(
                    wall_entry.or(radius_entry),
                    &fields.sub(if wall_entry.is_some() {
                        "wall"
                    } else {
                        "radius"
                    }),
                    wall,
                    radius,
                    &format!(
                        "the tube would have no hole. \"wall\" must be less than \"radius\" ({})",
                        plain(radius)
                    ),
                );
            }
            "arch" => {
                let most = (size[0] / 2.0).min(size[1]);
                if wall >= most {
                    wall = self.too_big(
                        wall_entry.or(size_entry),
                        &fields.sub(if wall_entry.is_some() { "wall" } else { "size" }),
                        wall,
                        most,
                        &format!(
                            "there would be no doorway. \"wall\" must be less than half the arch's width and less than its height ({})",
                            plain(most)
                        ),
                    );
                }
            }
            _ => {}
        }

        // ---- the options
        let mut trim = Trim::default();
        if let Some(entry) = bevel_entry {
            let path = fields.sub("bevel");
            if let Some(bevel) = self.ranged(&entry.node, &path, (0.0, SIZE_RANGE.1), "0.05") {
                // Each edge can lose just under half of the shortest
                // side it runs between.
                let most = if name == "box" {
                    size[0].min(size[1]).min(size[2]) / 2.0
                } else {
                    radius.min(height / 2.0)
                };
                trim.bevel = if bevel >= most {
                    self.too_big(
                        Some(entry),
                        &path,
                        bevel,
                        most,
                        &format!(
                            "nothing would be left of the {name}'s smallest side. \"bevel\" must be less than {}",
                            plain(most)
                        ),
                    )
                } else {
                    bevel
                };
            }
        }
        if let Some(entry) = bevel_steps {
            trim.bevel_steps = self
                .whole(entry, &fields.sub("bevel_steps"), (1.0, 4.0))
                .unwrap_or(1);
        }
        if let Some(entry) = taper {
            let path = fields.sub("taper");
            trim.taper = match &entry.node.value {
                Value::Array(items) if items.len() == 2 => {
                    let mut both = [1.0; 2];
                    for (at, item) in items.iter().enumerate() {
                        if let Some(number) =
                            self.ranged(item, &format!("{path}[{at}]"), TAPER_RANGE, "0.5")
                        {
                            both[at] = number;
                        }
                    }
                    both
                }
                Value::Array(items) => {
                    let message = format!(
                        "this needs one number, like 0.5, or two, for x and z, like [0.5, 1], not a list of {}",
                        items.len()
                    );
                    self.error(Code::TypeMismatch, &path, entry.node.pos, message)
                        .got = Some(entry.node.value.to_got());
                    [1.0; 2]
                }
                _ => self
                    .ranged(&entry.node, &path, TAPER_RANGE, "0.5")
                    .map_or([1.0; 2], |all| [all; 2]),
            };
        }

        let shape = match name {
            "box" => Shape::Box { size },
            "cylinder" => Shape::Cylinder {
                radius,
                height,
                segments,
            },
            "sphere" => Shape::Sphere { radius, segments },
            "cone" => Shape::Cone {
                radius,
                height,
                segments,
            },
            "capsule" => Shape::Capsule {
                radius,
                height,
                segments,
            },
            "torus" => Shape::Torus {
                radius,
                tube,
                segments,
            },
            "tube" => Shape::Tube {
                radius,
                wall,
                height,
                segments,
            },
            "pyramid" => Shape::Pyramid { size },
            "prism" => Shape::Prism { size },
            "stairs" => Shape::Stairs { size, steps },
            "arch" => Shape::Arch {
                size,
                wall,
                segments,
            },
            _ => Shape::Wedge { size },
        };
        (shape, trim)
    }

    /// `attach`, `to` and `twist`, or `move` and `rotate`.
    fn placement(&mut self, fields: &mut Fields<'_>, place: Place) -> Placement {
        let attach = fields.take("attach");
        let to = fields.take("to");
        let twist = fields.take("twist");
        let rotate = fields.take("rotate");
        let move_by = fields.take("move");

        if place == Place::Defined {
            for (entry, field) in [
                (attach, "attach"),
                (to, "to"),
                (twist, "twist"),
                (rotate, "rotate"),
                (move_by, "move"),
            ] {
                if let Some(entry) = entry {
                    self.defined_has_no(&fields.sub(field), entry, field);
                }
            }
            return Placement::none();
        }

        match (attach, to) {
            (None, None) => {
                if let Some(entry) = twist {
                    // Without a join there is nothing to twist around.
                    let degrees = entry.node.value.as_number().map_or("90".to_owned(), plain);
                    let message = format!(
                        "this part isn't joined to anything, so there is nothing to twist it around: to turn it around the upright axis, write rotate = [0, {degrees}, 0]"
                    );
                    self.error(Code::Conflict, &fields.sub("twist"), entry.key_pos, message)
                        .suggestion = Some(format!("rotate = [0, {degrees}, 0]"));
                }
                let move_by = move_by
                    .and_then(|entry| {
                        self.three(&entry.node, &fields.sub("move"), MOVE_RANGE, "[0, 1.5, 0]")
                    })
                    .unwrap_or([0.0; 3]);
                let rotate = rotate
                    .and_then(|entry| {
                        let path = fields.sub("rotate");
                        if let Some(degrees) = entry.node.value.as_number() {
                            let message = format!(
                                "a part that isn't attached is turned around x, y and z: write rotate = [0, {}, 0] to turn it around the upright axis",
                                plain(degrees)
                            );
                            self.error(Code::TypeMismatch, &path, entry.node.pos, message)
                                .got = Some(degrees.into());
                            return None;
                        }
                        self.three(&entry.node, &path, (-36_000.0, 36_000.0), "[0, 90, 0]")
                    })
                    .unwrap_or([0.0; 3]);
                Placement::Free { move_by, rotate }
            }
            (Some(attach), Some(to)) => {
                if let Some(entry) = move_by {
                    self.error(
                        Code::Conflict,
                        &fields.sub("move"),
                        entry.key_pos,
                        "an attached part goes where its sockets say: remove \"move\", or move the socket it attaches to (see \"shift\")",
                    );
                }
                if let Some(entry) = rotate {
                    // Phase 0 wrote the twist as `rotate = 90`.
                    let (message, fix) = match entry.node.value.as_number() {
                        Some(degrees) => (
                            format!(
                                "a joined part turns around its join with \"twist\": write twist = {}",
                                plain(degrees)
                            ),
                            format!("twist = {}", plain(degrees)),
                        ),
                        None => (
                            "a joined part is turned by its join, not by \"rotate\": to turn it further around the join, write twist = 90".to_owned(),
                            "twist = 90".to_owned(),
                        ),
                    };
                    self.error(
                        Code::Conflict,
                        &fields.sub("rotate"),
                        entry.key_pos,
                        message,
                    )
                    .suggestion = Some(fix);
                }
                let twist = twist
                    .and_then(|entry| {
                        let path = fields.sub("twist");
                        self.ranged(&entry.node, &path, (-36_000.0, 36_000.0), "90")
                    })
                    .unwrap_or(0.0);
                let attach_path = fields.sub("attach");
                let to_path = fields.sub("to");
                let attach_name = self.name(&attach.node, &attach_path);
                let target = self.part_socket(&to.node, &to_path);
                match (attach_name, target) {
                    (Some(text), Some((to_part, to_socket))) => Placement::Attached {
                        attach: Named {
                            text,
                            loc: self.loc(&attach_path, attach.node.pos),
                        },
                        to_part,
                        to_socket,
                        to_loc: self.loc(&to_path, to.node.pos),
                        twist,
                    },
                    _ => Placement::none(),
                }
            }
            (Some(attach), None) => {
                let hint = match &attach.node.value {
                    Value::Str(socket) => match opposite(socket) {
                        Some(other) => format!(", like to = \"body.{other}\""),
                        None => ", like to = \"body.neck\"".to_owned(),
                    },
                    _ => String::new(),
                };
                let message = format!(
                    "\"attach\" names this part's socket; add \"to\" to say which socket of another part it joins{hint}"
                );
                self.error(
                    Code::MissingField,
                    &fields.sub("to"),
                    attach.key_pos,
                    message,
                );
                Placement::none()
            }
            (None, Some(to)) => {
                let hint = match &to.node.value {
                    Value::Str(target) => target
                        .split_once('.')
                        .and_then(|(_, socket)| opposite(socket))
                        .map(|other| format!(", like attach = \"{other}\""))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let message = format!(
                    "\"to\" names the other part's socket; add \"attach\" to say which socket of this part joins it{hint}"
                );
                self.error(
                    Code::MissingField,
                    &fields.sub("attach"),
                    to.key_pos,
                    message,
                );
                Placement::none()
            }
        }
    }

    /// One number, or three: how much bigger along x, y and z.
    fn scale(&mut self, node: &Node, path: &str) -> Option<[f64; 3]> {
        let negative = |value: &Value| value.as_number().is_some_and(|number| number <= 0.0);
        let any_negative = match &node.value {
            Value::Array(items) => items.iter().any(|item| negative(&item.value)),
            other => negative(other),
        };
        if any_negative {
            let issue = self.error(
                Code::OutOfRange,
                path,
                node.pos,
                "a scale must be above zero. To flip a part, use mirror = \"x\"",
            );
            issue.allowed = Some(crate::issue::Allowed::Range {
                min: SCALE_RANGE.0,
                max: SCALE_RANGE.1,
            });
            return None;
        }
        match &node.value {
            Value::Array(_) => self.three(node, path, SCALE_RANGE, "[1, 2, 1]"),
            _ => self
                .ranged(node, path, SCALE_RANGE, "2, or [1, 2, 1]")
                .map(|all| [all; 3]),
        }
    }

    /// A `sockets` table.
    fn sockets(&mut self, node: &Node, path: &str) -> Vec<SocketDef> {
        let Some(table) = self.table(
            node,
            path,
            "a table of sockets, like neck = { on = \"top\" }",
        ) else {
            return Vec::new();
        };
        let mut sockets = Vec::new();
        for entry in table.entries {
            let socket_path = join(path, &entry.key);
            if !is_name(&entry.key) {
                let message = format!(
                    "\"{}\" can't be a socket's name: use letters, digits, \"_\" and \"-\" only",
                    entry.key
                );
                self.error(Code::TypeMismatch, &socket_path, entry.key_pos, message);
                continue;
            }
            if Side::from_name(&entry.key).is_some() {
                let message = format!(
                    "every part already has a socket called \"{}\": give this one another name",
                    entry.key
                );
                self.error(Code::DuplicateName, &socket_path, entry.key_pos, message);
                continue;
            }
            if let Some(kind) = self.socket(&entry.node, &socket_path) {
                sockets.push(SocketDef {
                    name: entry.key.clone(),
                    loc: self.loc(&socket_path, entry.key_pos),
                    kind,
                });
            }
        }
        sockets
    }

    fn socket(&mut self, node: &Node, path: &str) -> Option<SocketKind> {
        let mut fields = self.table(
            node,
            path,
            "a socket, like { on = \"top\" } or { at = [0, 1, 0], facing = \"up\" }",
        )?;
        let on = fields.take("on");
        let at = fields.take("at");
        let from = fields.take("from");
        let shift = fields.take("shift");
        let facing = fields.take("facing");

        let kind = match (on, at, from) {
            (Some(on), None, None) => self.socket_on(on, shift, facing, &fields),
            (None, Some(at), None) => {
                if let Some(shift) = shift {
                    self.error(
                        Code::Conflict,
                        &fields.sub("shift"),
                        shift.key_pos,
                        "\"shift\" slides a socket placed with \"on\"; with \"at\" the point is already exact: remove \"shift\"",
                    );
                }
                let point = self.three(&at.node, &fields.sub("at"), MOVE_RANGE, "[0, 0.5, 0]");
                let facing = match facing {
                    Some(entry) => self.facing(&entry.node, &fields.sub("facing")),
                    None => Some([0.0, 1.0, 0.0]),
                };
                point
                    .zip(facing)
                    .map(|(at, facing)| SocketKind::At { at, facing })
            }
            (None, None, Some(from)) => {
                for (extra, field) in [(shift, "shift"), (facing, "facing")] {
                    if let Some(extra) = extra {
                        let message = format!(
                            "a socket passed on with \"from\" keeps its place and direction: remove \"{field}\""
                        );
                        self.error(Code::Conflict, &fields.sub(field), extra.key_pos, message);
                    }
                }
                self.part_socket(&from.node, &fields.sub("from"))
                    .map(|(part, socket)| SocketKind::From { part, socket })
            }
            (None, None, None) => {
                self.error(
                    Code::MissingField,
                    path,
                    fields.pos,
                    "a socket needs \"on\" (a side), \"at\" (a point) or \"from\" (a socket of an inner part), like { on = \"top\" }",
                );
                None
            }
            _ => {
                let given: Vec<&Entry> = [on, at, from].into_iter().flatten().collect();
                let message = format!(
                    "a socket's place is given one way: it has \"{}\" already, so remove \"{}\"",
                    given[0].key, given[1].key
                );
                self.error(
                    Code::Conflict,
                    &fields.sub(&given[1].key),
                    given[1].key_pos,
                    message,
                );
                None
            }
        };
        self.finish(fields, &["on", "shift", "facing", "at", "from"]);
        kind
    }

    fn socket_on(
        &mut self,
        on: &Entry,
        shift: Option<&Entry>,
        facing: Option<&Entry>,
        fields: &Fields<'_>,
    ) -> Option<SocketKind> {
        let on_path = fields.sub("on");
        let side_name = self.text(&on.node, &on_path, "top")?;
        if self.faces_ok && Side::from_name(&side_name).is_none() && is_name(&side_name) {
            // A face of a shaped box, by name: checked once the shape is
            // built.
            let shift = match shift {
                None => Some([0.0; 3]),
                Some(entry) => self.three(
                    &entry.node,
                    &fields.sub("shift"),
                    (-1.0, 1.0),
                    "[0.5, 0, 0]",
                ),
            };
            let facing = match facing {
                None => Some(None),
                Some(entry) => self.facing(&entry.node, &fields.sub("facing")).map(Some),
            };
            return shift.zip(facing).map(|(shift, facing)| SocketKind::OnFace {
                face: side_name.clone(),
                shift,
                facing,
                on_loc: self.loc(&on_path, on.node.pos),
            });
        }
        let Some(side) = Side::from_name(&side_name) else {
            let message = format!("there is no side \"{side_name}\"");
            let issue = Issue::error(Code::TypeMismatch, on_path, message)
                .in_file(self.file)
                .at(on.node.pos)
                .got(side_name.as_str())
                .one_of(&side_name, Side::ALL.map(Side::name));
            self.issues.push(issue);
            return None;
        };
        let shift = match shift {
            None => Some([0.0; 3]),
            Some(entry) => {
                let shift_path = fields.sub("shift");
                let shift = self.three(&entry.node, &shift_path, (-1.0, 1.0), "[0.5, 0, 0]");
                // A socket on a side slides across it, not in or out.
                let outwards = side.offset();
                match shift {
                    Some(shift) if (0..3).any(|at| outwards[at] != 0.0 && shift[at] != 0.0) => {
                        let direction =
                            ["x", "y", "z"][(0..3).find(|&at| outwards[at] != 0.0).unwrap_or(0)];
                        let message = format!(
                            "a socket on the {side_name} slides across that side: its {direction} number must stay 0. For a point off the surface, use \"at\""
                        );
                        self.error(Code::Conflict, &shift_path, entry.node.pos, message);
                        None
                    }
                    other => other,
                }
            }
        };
        let facing = match facing {
            None => Some(None),
            Some(entry) => self.facing(&entry.node, &fields.sub("facing")).map(Some),
        };
        shift.zip(facing).map(|(shift, facing)| SocketKind::On {
            side,
            shift,
            facing,
        })
    }

    /// A direction: a word, or three numbers that aren't all zero.
    fn facing(&mut self, node: &Node, path: &str) -> Option<[f64; 3]> {
        match &node.value {
            Value::Str(word) => match FACING_WORDS.iter().find(|(name, _)| name == word) {
                Some((_, direction)) => Some(*direction),
                None => {
                    let message = format!("there is no direction \"{word}\"");
                    let issue = Issue::error(Code::TypeMismatch, path, message)
                        .in_file(self.file)
                        .at(node.pos)
                        .got(word.as_str())
                        .one_of(word, FACING_WORDS.map(|(name, _)| name));
                    self.issues.push(issue);
                    None
                }
            },
            Value::Array(_) => {
                let direction = self.three(node, path, (-1000.0, 1000.0), "[0, 1, 0]")?;
                if direction == [0.0; 3] {
                    self.error(
                        Code::OutOfRange,
                        path,
                        node.pos,
                        "a direction can't be [0, 0, 0]: it has to point somewhere",
                    );
                    return None;
                }
                Some(direction)
            }
            other => {
                let message = format!(
                    "a direction is a word like \"up\" or three numbers like [0, 1, 0], not {}",
                    other.describe()
                );
                self.error(Code::TypeMismatch, path, node.pos, message);
                None
            }
        }
    }
}

/// The socket on the other side: `top` for `bottom`, `left` for `right`.
/// Used to make a helpful example when half a join is missing.
fn opposite(socket: &str) -> Option<&'static str> {
    Some(match socket {
        "top" => "bottom",
        "bottom" => "top",
        "left" => "right",
        "right" => "left",
        "front" => "back",
        "back" => "front",
        _ => return None,
    })
}

/// `["radius", "height"]` as `"radius" and "height"`.
fn quoted_list(words: &[&str]) -> String {
    let quoted: Vec<String> = words.iter().map(|word| format!("\"{word}\"")).collect();
    match quoted.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [first @ .., last] => format!("{} and {last}", first.join(", ")),
    }
}

/// A number the way a person writes it: `3`, not `3.0`.
fn plain(number: f64) -> String {
    if number == number.trunc() && number.abs() < 1e15 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

enum HexError {
    /// `#RRGGBBAA` or `#RGBA`: not in this phase.
    SeeThrough,
    /// Not a colour at all.
    NotHex,
}

/// Reads `"#RRGGBB"` or `"#RGB"`, capitals or not.
fn parse_hex(text: &str) -> Result<[u8; 3], HexError> {
    let digits = text.strip_prefix('#').ok_or(HexError::NotHex)?;
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(HexError::NotHex);
    }
    let value = |digit: u8| (digit as char).to_digit(16).unwrap_or(0) as u8;
    let bytes = digits.as_bytes();
    match bytes.len() {
        6 => Ok([
            value(bytes[0]) * 16 + value(bytes[1]),
            value(bytes[2]) * 16 + value(bytes[3]),
            value(bytes[4]) * 16 + value(bytes[5]),
        ]),
        // "#F80" is short for "#FF8800".
        3 => Ok([
            value(bytes[0]) * 17,
            value(bytes[1]) * 17,
            value(bytes[2]) * 17,
        ]),
        4 | 8 => Err(HexError::SeeThrough),
        _ => Err(HexError::NotHex),
    }
}

/// The red, green and blue of a colour written `#RRGGBB` or `#RGB`.
/// `None` for anything else.
pub fn rgb_of(text: &str) -> Option<[u8; 3]> {
    parse_hex(text).ok()
}

/// A colour as `#RRGGBB`.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
