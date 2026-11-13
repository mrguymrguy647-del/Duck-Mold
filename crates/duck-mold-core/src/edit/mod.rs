//! Edits: the changes a person or an agent can make to a recipe without
//! typing it.
//!
//! Every edit is one [`Edit`] value. The command line turns its words
//! into one (`duck-mold edit robot.toml add hat --shape cone --to
//! head.top`), the Studio turns a click into one, and both call
//! [`edit_file`]. So whatever the Studio can do, an agent can do too, and
//! the other way round.
//!
//! An edit is gentle on the file: it changes only the characters it has
//! to ([`doc`] says how), so comments, order and layout stay. The new text
//! is built before it is written; an edit that would leave the recipe
//! with more mistakes than it had is refused, and nothing is written.
//!
//! The recipe is still the model: nothing here keeps a model. An edit
//! reads the file, changes its text, checks it and writes it back.

pub mod doc;
mod motion;
mod spot;

pub use motion::LimitsTo;
pub use spot::{Spot, spot};

use std::path::Path;

use serde::Serialize;

use crate::assemble::assemble;
use crate::inspect::{Inspection, inspect};
use crate::issue::{Code, Issue, Report};
use crate::load::{display_path, load_text_at};
use crate::math::{Bounds, Mat3, Vec3};
use crate::model::{Model, Part};
use crate::recipe::{Limits, PART_FIELDS, Shape, Side, rgb_of};
use crate::source::{Node, Syntax, Value};
use crate::tools::Failure;
use doc::{Doc, Step, TreePath, Val, key, with};

/// What a new part is made of.
#[derive(Debug, Clone, PartialEq)]
pub enum Made {
    /// One of the built-in shapes: `box`, `cylinder`, `torus`...
    Shape(String),
    /// A part the recipe defines: `use = "arm"`.
    Use(String),
    /// A recipe or a mesh file: `file = "parts/lamp.gltf"`.
    File(String),
}

/// The sizes a shape can have. Only those that are given are written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sizes {
    /// `size = [x, y, z]`, for a box, a wedge, a pyramid, a prism,
    /// stairs or an arch.
    pub size: Option<[f64; 3]>,
    /// `radius`, for a round shape.
    pub radius: Option<f64>,
    /// `tube`: how thick a torus's ring is.
    pub tube: Option<f64>,
    /// `wall`: how thick a tube's or an arch's wall is.
    pub wall: Option<f64>,
    /// `height`, for a cylinder, a cone, a capsule or a tube.
    pub height: Option<f64>,
    /// `segments`: how many flat sides a round shape has.
    pub segments: Option<u32>,
    /// `steps`: how many steps stairs have.
    pub steps: Option<u32>,
    /// `bevel`: how much is cut off the edges of a box or a cylinder.
    /// 0 takes the bevel away.
    pub bevel: Option<f64>,
    /// `bevel_steps`: how many flat cuts go round each edge.
    pub bevel_steps: Option<u32>,
    /// `taper`: how wide (x) and deep (z) the top of a box, a cylinder
    /// or a tube is, as a share of the bottom. 1 and 1 take it away.
    pub taper: Option<[f64; 2]>,
}

impl Sizes {
    fn is_empty(&self) -> bool {
        *self == Sizes::default()
    }

    /// The fields to write, in the order a recipe has them. A bevel of 0
    /// and a taper of 1 are no bevel and no taper: `None` says the
    /// field is to go.
    fn fields(&self) -> Vec<(String, Option<Val>)> {
        let mut fields = Vec::new();
        let mut put = |name: &str, value: Option<Val>| fields.push((name.to_owned(), value));
        if let Some(size) = self.size {
            put("size", Some(Val::three(size)));
        }
        if let Some(radius) = self.radius {
            put("radius", Some(Val::Num(radius)));
        }
        if let Some(tube) = self.tube {
            put("tube", Some(Val::Num(tube)));
        }
        if let Some(wall) = self.wall {
            put("wall", Some(Val::Num(wall)));
        }
        if let Some(height) = self.height {
            put("height", Some(Val::Num(height)));
        }
        if let Some(segments) = self.segments {
            put("segments", Some(Val::Num(f64::from(segments))));
        }
        if let Some(steps) = self.steps {
            put("steps", Some(Val::Num(f64::from(steps))));
        }
        if let Some(bevel) = self.bevel {
            if bevel == 0.0 {
                put("bevel", None);
                put("bevel_steps", None);
            } else {
                put("bevel", Some(Val::Num(bevel)));
            }
        }
        if let Some(steps) = self.bevel_steps
            && self.bevel != Some(0.0)
        {
            put(
                "bevel_steps",
                (steps > 1).then_some(Val::Num(f64::from(steps))),
            );
        }
        if let Some([across, deep]) = self.taper {
            put(
                "taper",
                if across == 1.0 && deep == 1.0 {
                    None
                } else if across == deep {
                    Some(Val::Num(across))
                } else {
                    Some(Val::List(vec![Val::Num(across), Val::Num(deep)]))
                },
            );
        }
        fields
    }
}

/// What `set` changes about a part. Only what is given changes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Changes {
    /// A new shape. Sizes the new shape doesn't have are dropped.
    pub shape: Option<String>,
    /// New sizes.
    pub sizes: Sizes,
    /// A new palette colour for the whole part.
    pub color: Option<String>,
    /// Colours for single faces: `("top", "plank")`.
    pub face_colors: Vec<(String, String)>,
    /// Degrees to twist it around its join.
    pub twist: Option<f64>,
    /// Where a free part's middle goes: `move = [x, y, z]`.
    pub move_to: Option<[f64; 3]>,
    /// How a free part is turned: `rotate = [x, y, z]`, in degrees.
    pub rotate: Option<[f64; 3]>,
    /// How much bigger the part is each way: `scale`. Written as one
    /// number when the three are alike.
    pub scale: Option<[f64; 3]>,
}

/// Where a new part that is joined to nothing goes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Free {
    /// Its middle: `move = [x, y, z]`. Without it, the part is stood on
    /// the ground.
    pub move_to: Option<[f64; 3]>,
    /// Where on the ground it is stood, when `move_to` isn't given: x
    /// and z, in the recipe's own measurements. Without it: on clear
    /// ground, from the middle of the model towards its front.
    pub stand: Option<[f64; 2]>,
    /// How it is turned: `rotate = [x, y, z]`.
    pub rotate: Option<[f64; 3]>,
    /// How much bigger it is each way: `scale`.
    pub scale: Option<[f64; 3]>,
}

/// One change to a recipe. A part is named as `inspect` prints it:
/// `head`, `left_arm/hand`, or `#2` for a part with no name.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Adds a part: joined to a socket, or free.
    Add {
        /// The new part's name.
        name: String,
        /// What it is made of.
        made: Made,
        /// The socket it joins: `head.top`. Without one the part is
        /// free: it is added to the model's own list, where `free` says.
        to: Option<String>,
        /// Its own socket to join by. Without one: the side opposite the
        /// socket it joins (`bottom` on a `top`), otherwise `bottom`.
        attach: Option<String>,
        /// Its sizes, for a shape.
        sizes: Sizes,
        /// Its palette colour.
        color: Option<String>,
        /// Degrees to twist it around the join.
        twist: Option<f64>,
        /// Where a part that joins nothing goes.
        free: Free,
    },
    /// Removes a part.
    Remove {
        /// The part.
        part: String,
        /// Also remove the parts that hang from it. Without this, a part
        /// that others hang from is not removed.
        with_attached: bool,
    },
    /// Renames a part, and everything that refers to it.
    Rename {
        /// The part.
        part: String,
        /// Its new name.
        new_name: String,
    },
    /// Moves a part's join to another socket.
    Join {
        /// The part.
        part: String,
        /// The socket to join: `body.neck`.
        to: String,
        /// Its own socket to join by, if that changes too.
        attach: Option<String>,
    },
    /// Changes a part's shape, sizes, colours or twist.
    Set {
        /// The part.
        part: String,
        /// What to change.
        changes: Changes,
    },
    /// Copies a part, straight after it.
    Duplicate {
        /// The part.
        part: String,
        /// The copy's name. Without one: the part's name with a number,
        /// or, for a mirror image, with `left` and `right` swapped.
        name: Option<String>,
        /// The socket the copy joins. Without one: the same socket, or,
        /// for a mirror image, that socket with `left` and `right`
        /// swapped.
        to: Option<String>,
        /// Make the copy a mirror image: a right arm from a left arm.
        mirror: bool,
    },
    /// Adds a colour to the recipe's palette.
    AddColor {
        /// Its name.
        name: String,
        /// `#RRGGBB`.
        hex: String,
    },
    /// Changes a colour that is in the recipe's palette.
    SetColor {
        /// Its name.
        name: String,
        /// `#RRGGBB`.
        hex: String,
    },
    /// Takes a colour out of the recipe's palette.
    RemoveColor {
        /// Its name.
        name: String,
        /// The colour everything painted with it gets instead. Without
        /// one, the edit is refused if anything is painted with it.
        replace_with: Option<String>,
    },
    /// Declares a socket on one side of a part.
    AddSocket {
        /// The part.
        part: String,
        /// The socket's name.
        name: String,
        /// The side: `top`, `bottom`, `left`, `right`, `front`, `back`.
        on: String,
        /// How far it is slid across the side, -1 to 1 each way.
        shift: [f64; 3],
    },
    /// Renames a socket a part declares, and everything that names it.
    RenameSocket {
        /// The socket: `body.spot`.
        socket: String,
        /// Its new name.
        new_name: String,
    },
    /// Removes a socket a part declares. Refused if anything uses it.
    RemoveSocket {
        /// The socket: `body.spot`.
        socket: String,
    },
    /// Removes the sockets the Studio made (`spot`, `spot_2`...) that
    /// nothing uses any more.
    TidySockets,
    /// Makes a joined part free, exactly where it stands.
    Unjoin {
        /// The part.
        part: String,
    },
    /// Moves a socket that was declared on a side of a part.
    SetSocket {
        /// The socket: `body.badge`.
        socket: String,
        /// Another side.
        on: Option<String>,
        /// A new slide across the side.
        shift: Option<[f64; 3]>,
        /// Which way it faces: `up`, `down`, `left`, `right`, `front`,
        /// `back`, or `out` (straight out of its side, as a socket faces
        /// when nothing says otherwise).
        facing: Option<String>,
    },
    /// Pulls faces of a box out the way they face (in, for a negative
    /// `by`), with new walls round them: a `pull` in its `changes`.
    Pull {
        /// The part.
        part: String,
        /// The faces, by name. Faces that touch are pulled as one piece.
        faces: Vec<String>,
        /// How far, in metres.
        by: f64,
        /// What the new walls are called.
        name: Option<String>,
    },
    /// Changes the amount of one of a box's changes.
    SetChange {
        /// The part.
        part: String,
        /// Which change, counted from 1.
        number: usize,
        /// Its new `by`, for a change that takes one number.
        by: Option<f64>,
        /// Its new `by`, for one that takes three (a move, a turn, a
        /// scale).
        by_xyz: Option<[f64; 3]>,
        /// Its new `at` (a cut's).
        at: Option<f64>,
    },
    /// Moves faces of a box by x, y and z metres, in its own directions:
    /// a `move` in its `changes`. Right after a move of the same faces,
    /// it adds to that one instead.
    MoveFace {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: Vec<String>,
        /// How far along x, y and z.
        by: [f64; 3],
    },
    /// Turns faces of a box about their middle: a `turn`. Right after a
    /// turn of the same faces, it adds to that one.
    TurnFace {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: Vec<String>,
        /// Degrees around x, y and z.
        by: [f64; 3],
    },
    /// Moves edges of a box (`top+front`: where those faces meet) by x,
    /// y and z metres: a `move` in its `changes`. Right after a move of
    /// the same edges, it adds to that one.
    MoveEdge {
        /// The part.
        part: String,
        /// The edges, by name: `top+front`.
        edges: Vec<String>,
        /// How far along x, y and z.
        by: [f64; 3],
    },
    /// Makes a definition from a part: what it is made of (its shape and
    /// sizes, changes, colours and own sockets) moves to
    /// `[define.<name>]`, and the part becomes `use = "<name>"` where it
    /// stands.
    Define {
        /// The part.
        part: String,
        /// The definition's name.
        name: String,
    },
    /// Moves corners of a box (`top+front+left`: where those faces
    /// meet) by x, y and z metres: a `move` in its `changes`. Right after
    /// a move of the same corners, it adds to that one.
    MoveCorner {
        /// The part.
        part: String,
        /// The corners, by name: `top+front+left`.
        corners: Vec<String>,
        /// How far along x, y and z.
        by: [f64; 3],
    },
    /// Cuts all the way round a box with a flat cut facing a side, `at`
    /// of the way from the middle to that side: a `cut` in its
    /// `changes`.
    Cut {
        /// The part.
        part: String,
        /// The side the cut faces: `right`.
        toward: String,
        /// From the middle (0) to the side (just under 1).
        at: f64,
        /// What the slices are called: `<face>_<name>`.
        name: Option<String>,
    },
    /// Makes faces of a box bigger or smaller about their middle: a
    /// `scale`. Right after a scale of the same faces, it multiplies
    /// that one.
    ScaleFace {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: Vec<String>,
        /// How much along x, y and z: 1 is as they are.
        by: [f64; 3],
    },
    /// Takes one of a box's changes away, or all of them.
    RemoveChange {
        /// The part.
        part: String,
        /// Which change, counted from 1. `None`: every one.
        number: Option<usize>,
    },
    /// Makes a smaller face inside each face of a box, `by` in from its
    /// edges: an `inset` in its `changes`.
    Inset {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: Vec<String>,
        /// How far in from the edges, in metres.
        by: f64,
        /// What the face in the middle is called.
        name: Option<String>,
    },
    /// Says how far a part may turn about its join when the model is
    /// posed: its `limits`.
    SetLimits {
        /// The part.
        part: String,
        /// What its limits become.
        limits: LimitsTo,
    },
    /// Turns or moves a part in a pose, or puts it back at rest there.
    /// Makes the pose if it isn't there.
    SetPose {
        /// The pose.
        pose: String,
        /// The part, by path.
        part: String,
        /// Degrees around x, then y, then z. `None`: as it is.
        turn: Option<[f64; 3]>,
        /// How far it is moved, in metres. `None`: as it is.
        move_by: Option<[f64; 3]>,
        /// Puts the part back at rest in the pose: its line goes.
        rest: bool,
    },
    /// Adds a pose: with every part at rest, or as a copy of another.
    AddPose {
        /// Its name.
        name: String,
        /// The pose to copy.
        from: Option<String>,
    },
    /// Renames a pose, and everything that names it.
    RenamePose {
        /// The pose.
        pose: String,
        /// Its new name.
        new_name: String,
    },
    /// Removes a pose.
    RemovePose {
        /// The pose.
        pose: String,
    },
    /// Makes the mirror image of a pose, left to right: what the left
    /// arm did, the right arm does.
    MirrorPose {
        /// The pose.
        pose: String,
        /// The name of the mirror image. Without one, the pose itself
        /// is mirrored.
        as_name: Option<String>,
    },
}

impl Edit {
    /// The name of every kind of edit, as the command line and the agent
    /// guide write it. A test checks each has a command and is in the
    /// guide.
    pub const COMMANDS: [&'static str; 32] = [
        "add",
        "remove",
        "rename",
        "join",
        "set",
        "duplicate",
        "add-color",
        "set-color",
        "remove-color",
        "add-socket",
        "set-socket",
        "rename-socket",
        "remove-socket",
        "tidy-sockets",
        "unjoin",
        "pull",
        "inset",
        "set-change",
        "remove-change",
        "move-face",
        "turn-face",
        "scale-face",
        "move-edge",
        "cut",
        "move-corner",
        "define",
        "set-limits",
        "set-pose",
        "add-pose",
        "rename-pose",
        "remove-pose",
        "mirror-pose",
    ];

    /// This edit's name: `add`, `set-color`...
    pub fn command(&self) -> &'static str {
        match self {
            Edit::Add { .. } => "add",
            Edit::Remove { .. } => "remove",
            Edit::Rename { .. } => "rename",
            Edit::Join { .. } => "join",
            Edit::Set { .. } => "set",
            Edit::Duplicate { .. } => "duplicate",
            Edit::AddColor { .. } => "add-color",
            Edit::SetColor { .. } => "set-color",
            Edit::RemoveColor { .. } => "remove-color",
            Edit::AddSocket { .. } => "add-socket",
            Edit::SetSocket { .. } => "set-socket",
            Edit::RenameSocket { .. } => "rename-socket",
            Edit::RemoveSocket { .. } => "remove-socket",
            Edit::TidySockets => "tidy-sockets",
            Edit::Unjoin { .. } => "unjoin",
            Edit::Pull { .. } => "pull",
            Edit::Inset { .. } => "inset",
            Edit::SetChange { .. } => "set-change",
            Edit::RemoveChange { .. } => "remove-change",
            Edit::MoveFace { .. } => "move-face",
            Edit::TurnFace { .. } => "turn-face",
            Edit::ScaleFace { .. } => "scale-face",
            Edit::MoveEdge { .. } => "move-edge",
            Edit::Cut { .. } => "cut",
            Edit::MoveCorner { .. } => "move-corner",
            Edit::Define { .. } => "define",
            Edit::SetLimits { .. } => "set-limits",
            Edit::SetPose { .. } => "set-pose",
            Edit::AddPose { .. } => "add-pose",
            Edit::RenamePose { .. } => "rename-pose",
            Edit::RemovePose { .. } => "remove-pose",
            Edit::MirrorPose { .. } => "mirror-pose",
        }
    }
}

/// A part found in the recipe's text.
#[derive(Debug, Clone, PartialEq)]
struct Found {
    /// The list it is in.
    list: TreePath,
    /// Its place in that list.
    at: usize,
    /// Its name, if it has one.
    name: Option<String>,
    /// The definition it is inside, if it is: `arm` for
    /// `left_arm/hand`, where `left_arm` uses `arm`.
    definition: Option<String>,
    /// Its path inside that definition: `hand`.
    inside: String,
}

impl Found {
    fn path(&self) -> TreePath {
        with(&self.list, Step::Index(self.at))
    }
}

/// Why an edit can't be carried out. (In a box, because an `Issue` is
/// big and most edits don't fail.)
struct Stop(Box<Issue>);

impl Stop {
    fn got(self, value: impl Into<serde_json::Value>) -> Stop {
        Stop(Box::new(self.0.got(value)))
    }

    fn one_of<I, S>(self, written: &str, options: I) -> Stop
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Stop(Box::new(self.0.one_of(written, options)))
    }
}

fn problem(code: Code, message: impl Into<String>) -> Stop {
    Stop(Box::new(Issue::error(code, "", message)))
}

fn boxed(stop: Stop) -> Box<Issue> {
    stop.0
}

fn cant(error: doc::Cant) -> Stop {
    problem(Code::Unsupported, error.0)
}

/// The text of a field of a table node.
fn text_of<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match &node.value {
        Value::Table(entries) => entries
            .iter()
            .find(|entry| entry.key == name)
            .and_then(|entry| match &entry.node.value {
                Value::Str(text) => Some(text.as_str()),
                _ => None,
            }),
        _ => None,
    }
}

fn has_field(node: &Node, name: &str) -> bool {
    matches!(&node.value, Value::Table(entries) if entries.iter().any(|e| e.key == name))
}

/// The names of the parts in a list, `#2` for one without.
fn names_in(doc: &Doc, list: &[Step]) -> Vec<String> {
    doc.items(list)
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(at, item)| text_of(item, "name").map_or_else(|| format!("#{at}"), str::to_owned))
        .collect()
}

/// Finds a part by the path `inspect` prints: `left_arm/hand`.
fn find(doc: &Doc, part: &str) -> Result<Found, Stop> {
    let segments: Vec<&str> = part.split('/').collect();
    let mut list: TreePath = vec![key("parts")];
    let mut definition = None;
    let mut inside_from = 0;
    for (depth, segment) in segments.iter().enumerate() {
        let names = names_in(doc, &list);
        let Some(at) = names.iter().position(|name| name == segment) else {
            let so_far = segments[..depth].join("/");
            let place = if so_far.is_empty() {
                "the recipe".to_owned()
            } else {
                format!("\"{so_far}\"")
            };
            return Err(problem(
                Code::UnknownPart,
                format!("there is no part \"{segment}\" in {place}"),
            )
            .got(*segment)
            .one_of(segment, names));
        };
        if depth + 1 == segments.len() {
            let item = &doc.items(&list).expect("the list was just read")[at];
            return Ok(Found {
                name: text_of(item, "name").map(str::to_owned),
                list,
                at,
                definition,
                inside: segments[inside_from..].join("/"),
            });
        }
        // Into the part: its own parts, or those of what it uses.
        let mut table = with(&list, Step::Index(at));
        let mut hops = 0;
        loop {
            let node = doc.node(&table).expect("the part is there");
            if has_field(node, "parts") {
                list = with(&table, key("parts"));
                break;
            }
            if let Some(file) = text_of(node, "file") {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "\"{}\" is inside \"{file}\", another file: open that file to edit it",
                        segments[depth + 1..].join("/")
                    ),
                ));
            }
            match text_of(node, "use") {
                Some(name) if hops < 32 => {
                    hops += 1;
                    definition = Some(name.to_owned());
                    inside_from = depth + 1;
                    table = vec![key("define"), key(name)];
                    if doc.node(&table).is_none() {
                        return Err(problem(
                            Code::UnknownPart,
                            format!("there is no defined part \"{name}\" to look inside"),
                        ));
                    }
                }
                _ => {
                    return Err(problem(
                        Code::UnknownPart,
                        format!("the part \"{segment}\" has no parts inside it"),
                    ));
                }
            }
        }
    }
    Err(problem(Code::UnknownPart, "no part was named"))
}

/// Where a part's shape, sizes and colours are written: its own table,
/// or, for a part made with `use`, the definition it uses (whose name
/// is given too). A definition's shape and colours are its own: the part
/// that uses it can't have others.
fn making(doc: &Doc, found: &Found) -> (TreePath, Option<String>) {
    let mut table = found.path();
    let mut definition = None;
    for _ in 0..32 {
        let used = doc
            .node(&table)
            .and_then(|node| text_of(node, "use"))
            .map(str::to_owned);
        match used {
            Some(name) if doc.node(&[key("define"), key(&name)]).is_some() => {
                table = vec![key("define"), key(&name)];
                definition = Some(name);
            }
            _ => break,
        }
    }
    (table, definition)
}

/// Splits `head.top` into the part and the socket.
fn split_socket(written: &str) -> Result<(&str, &str), Stop> {
    match written.rsplit_once('.') {
        Some((part, socket)) if !part.is_empty() && !socket.is_empty() => Ok((part, socket)),
        _ => Err(problem(
            Code::TypeMismatch,
            format!(
                "\"{written}\" doesn't name a socket: write the part, a dot and the socket, like body.neck"
            ),
        )
        .got(written)),
    }
}

fn check_name(name: &str) -> Result<(), Stop> {
    let fine = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if fine {
        Ok(())
    } else {
        Err(problem(
            Code::TypeMismatch,
            format!("\"{name}\" can't be a name: use letters, digits, \"_\" and \"-\" only"),
        )
        .got(name))
    }
}

fn check_hex(hex: &str) -> Result<(), Stop> {
    match rgb_of(hex) {
        Some(_) => Ok(()),
        None => Err(problem(
            Code::TypeMismatch,
            format!("\"{hex}\" isn't a colour: write it as #RRGGBB, like #F2C84B"),
        )
        .got(hex)),
    }
}

/// The side opposite a built-in socket: what a new part joins by.
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

/// `left_arm` to `right_arm` and back; the same text if it has neither.
fn swap_sides(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("left") {
            out.push_str("right");
            rest = after;
        } else if let Some(after) = rest.strip_prefix("right") {
            out.push_str("left");
            rest = after;
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// A name that isn't in `taken`: `hat_2`, `hat_3`...
fn free_name(base: &str, taken: &[String]) -> String {
    let stem = match base.rsplit_once('_') {
        Some((stem, number)) if number.parse::<u32>().is_ok() => stem,
        _ => base,
    };
    (2..)
        .map(|number| format!("{stem}_{number}"))
        .find(|name| !taken.contains(name))
        .expect("there is always a free number")
}

/// The size fields and face names a shape has.
fn shape_words(shape: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Shape::NAMES
        .contains(&shape)
        .then(|| (Shape::size_fields(shape), Shape::face_words(shape)))
}

/// Where a socket of the part `found` is named, besides where it is
/// declared: the `to` of every part joined to it, the part's own
/// `attach`, and the `from` of a socket that what the list belongs to
/// passes on. Each with what to call it in a message.
fn socket_uses(doc: &Doc, found: &Found, socket: &str) -> Vec<(TreePath, String)> {
    let mut uses = Vec::new();
    let items = doc.items(&found.list).unwrap_or_default();
    let own = with(&found.list, Step::Index(found.at));
    if let Some(item) = items.get(found.at)
        && text_of(item, "attach") == Some(socket)
    {
        uses.push((
            with(&own, key("attach")),
            "the part itself, which joins by it".to_owned(),
        ));
    }
    // Only a part with a name can be joined to, or passed on from.
    let Some(name) = &found.name else {
        return uses;
    };
    let full = format!("{name}.{socket}");
    for (at, item) in items.iter().enumerate() {
        if text_of(item, "to") == Some(full.as_str()) {
            let called = text_of(item, "name").map_or_else(|| format!("#{at}"), str::to_owned);
            uses.push((with(&with(&found.list, Step::Index(at)), key("to")), called));
        }
    }
    let owner = &found.list[..found.list.len().saturating_sub(1)];
    let passed = with(owner, key("sockets"));
    for entry in doc.entries(&passed).unwrap_or_default() {
        if text_of(&entry.node, "from") == Some(full.as_str()) {
            uses.push((
                with(&with(&passed, key(&entry.key)), key("from")),
                format!("the socket \"{}\", which passes it on", entry.key),
            ));
        }
    }
    uses
}

/// The sockets a part declares, by name.
fn declared_sockets(doc: &Doc, found: &Found) -> Vec<String> {
    doc.entries(&with(&found.path(), key("sockets")))
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect()
}

/// A declared socket of a part, or why there is none by that name.
fn declared(doc: &Doc, part: &str, name: &str) -> Result<Found, Stop> {
    let found = find(doc, part)?;
    let names = declared_sockets(doc, &found);
    if names.iter().any(|known| known == name) {
        return Ok(found);
    }
    Err(problem(
        Code::UnknownSocket,
        if Side::from_name(name).is_some() || name == "slope" {
            format!("\"{name}\" is built in: every part has it, and it can't be renamed or removed")
        } else {
            format!("\"{part}\" declares no socket called \"{name}\"")
        },
    )
    .got(name)
    .one_of(name, names))
}

/// The ways a socket on a side can face, as `set-socket --facing` takes
/// them: `out` is straight out of its side, the others are the words a
/// recipe writes in `facing`.
pub const FACINGS: [&str; 7] = ["out", "up", "down", "left", "right", "front", "back"];

/// Which way a declared socket faces, as one of [`FACINGS`], for a socket
/// on a side or a face; `None` for one written with `at` or `from`, or
/// with three numbers in `facing`.
fn facing_word(doc: &Doc, found: &Found, socket: &str) -> Option<String> {
    let place = with(&with(&found.path(), key("sockets")), key(socket));
    doc.entry(&place, "on")?;
    match doc.entry(&place, "facing") {
        None => Some("out".to_owned()),
        Some(entry) => match &entry.node.value {
            Value::Str(word) if FACINGS.contains(&word.as_str()) => Some(word.clone()),
            _ => None,
        },
    }
}

/// True for the names the Studio gives the sockets it makes: `spot`,
/// `spot_2`, `spot_3`...
pub fn is_studio_socket(name: &str) -> bool {
    match name.strip_prefix("spot") {
        Some("") => true,
        Some(rest) => rest
            .strip_prefix('_')
            .is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())),
        None => false,
    }
}

/// Every list of parts in the recipe: the model's own, each definition's,
/// and those of the parts inside them.
fn part_lists(doc: &Doc) -> Vec<TreePath> {
    fn inside(doc: &Doc, list: TreePath, into: &mut Vec<TreePath>) {
        let Some(items) = doc.items(&list) else {
            return;
        };
        let count = items.len();
        into.push(list.clone());
        for at in 0..count {
            inside(doc, with(&with(&list, Step::Index(at)), key("parts")), into);
        }
    }
    let mut lists = Vec::new();
    inside(doc, vec![key("parts")], &mut lists);
    let defined: Vec<String> = doc
        .entries(&[key("define")])
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect();
    for name in defined {
        inside(
            doc,
            vec![key("define"), key(&name), key("parts")],
            &mut lists,
        );
    }
    lists
}

/// Takes a socket out of a part's `sockets`, and the table with it if
/// that leaves it empty.
fn drop_socket(doc: &mut Doc, part: &[Step], name: &str) -> Result<(), Stop> {
    let sockets = with(part, key("sockets"));
    doc.remove_field(&sockets, name).map_err(cant)?;
    if doc.entries(&sockets).is_some_and(<[_]>::is_empty) {
        // An empty table left behind is harmless, so if it can't be
        // taken out as it is written, it stays.
        let _ = doc.remove_field(part, "sockets");
    }
    Ok(())
}

fn set(doc: &mut Doc, table: &[Step], name: &str, value: Val) -> Result<(), Stop> {
    doc.set_field(table, name, &value, &PART_FIELDS)
        .map_err(cant)
}

fn text(value: &str) -> Val {
    Val::Str(value.to_owned())
}

/// Where the sibling that `to` names is: `to` may be written as the
/// recipe has it (`body.neck`) or with the whole path
/// (`left_arm/hand.bottom`). Gives the sibling's own name and the socket.
fn sibling<'a>(doc: &Doc, list: &[Step], to: &'a str) -> Result<(String, &'a str), Stop> {
    let (target, socket) = split_socket(to)?;
    let found = if target.contains('/') {
        find(doc, target)?
    } else {
        let names = names_in(doc, list);
        match names.iter().position(|name| name == target) {
            Some(at) => Found {
                list: list.to_vec(),
                at,
                name: text_of(&doc.items(list).expect("read above")[at], "name").map(str::to_owned),
                definition: None,
                inside: String::new(),
            },
            None => {
                return Err(problem(
                    Code::UnknownPart,
                    format!("there is no part \"{target}\" in this list"),
                )
                .got(target)
                .one_of(target, names));
            }
        }
    };
    if found.list != list {
        return Err(problem(
            Code::Conflict,
            format!(
                "\"{target}\" is in another list of parts: a part joins a part of its own list"
            ),
        ));
    }
    match found.name {
        Some(name) => Ok((name, socket)),
        None => Err(problem(
            Code::UnknownPart,
            format!(
                "the part \"{target}\" has no name, and only a named part can be joined to: name it first, with rename"
            ),
        )),
    }
}

/// What an edit did, in words, and the part whose definition (if it is
/// inside one) says what else changes.
struct Outcome {
    summary: String,
    touched: Option<Found>,
}

/// A number as a recipe should hold it: to a tenth of a millimetre (or
/// of a degree's hundredth), and never `-0`.
fn tidy_number(value: f64, decimals: i32) -> f64 {
    let unit = 10f64.powi(decimals);
    (value * unit).round() / unit + 0.0
}

/// The flip and stretch a part's own `mirror` and `scale` ask for.
fn stretch_of(node: &Node) -> Mat3 {
    let Value::Table(entries) = &node.value else {
        return Mat3::IDENTITY;
    };
    let number = |node: &Node| match node.value {
        Value::Int(whole) => Some(whole as f64),
        Value::Float(number) => Some(number),
        _ => None,
    };
    let mut scale = [1.0; 3];
    if let Some(entry) = entries.iter().find(|entry| entry.key == "scale") {
        match &entry.node.value {
            Value::Array(items) if items.len() == 3 => {
                for (at, item) in items.iter().enumerate() {
                    scale[at] = number(item).unwrap_or(1.0);
                }
            }
            _ => scale = [number(&entry.node).unwrap_or(1.0); 3],
        }
    }
    match text_of(node, "mirror") {
        Some("x") => scale[0] = -scale[0],
        Some("y") => scale[1] = -scale[1],
        Some("z") => scale[2] = -scale[2],
        _ => {}
    }
    Mat3::stretch(scale.into())
}

/// The part of the built model with this path.
fn part_in<'a>(model: &'a Model, path: &str) -> Option<&'a Part> {
    fn find<'a>(parts: &'a [Part], path: &str) -> Option<&'a Part> {
        parts.iter().find_map(|part| {
            if part.path == path {
                Some(part)
            } else {
                find(&part.parts, path)
            }
        })
    }
    find(&model.parts, path)
}

fn run(doc: &mut Doc, edit: &Edit, model: Option<&Model>) -> Result<Outcome, Stop> {
    match edit {
        Edit::SetLimits { part, limits } => motion::set_limits(doc, part, *limits),
        Edit::SetPose {
            pose,
            part,
            turn,
            move_by,
            rest,
        } => motion::set_pose(doc, pose, part, *turn, *move_by, *rest),
        Edit::AddPose { name, from } => motion::add_pose(doc, name, from.as_deref()),
        Edit::RenamePose { pose, new_name } => motion::rename_pose(doc, pose, new_name),
        Edit::RemovePose { pose } => motion::remove_pose(doc, pose),
        Edit::MirrorPose { pose, as_name } => {
            motion::mirror_pose(doc, pose, as_name.as_deref(), model)
        }
        Edit::Add {
            name,
            made,
            to,
            attach,
            sizes,
            color,
            twist,
            free,
        } => {
            check_name(name)?;
            let (field, what) = match made {
                Made::Shape(shape) => ("shape", shape),
                Made::Use(defined) => ("use", defined),
                Made::File(file) => ("file", file),
            };
            if !sizes.is_empty() && !matches!(made, Made::Shape(_)) {
                return Err(problem(
                    Code::Conflict,
                    "sizes belong to a shape: a part made with --use or --file is sized by \"scale\" in the recipe",
                ));
            }
            let mut fields = vec![
                ("name".to_owned(), text(name)),
                (field.to_owned(), text(what)),
            ];
            fields.extend(
                sizes
                    .fields()
                    .into_iter()
                    .filter_map(|(name, value)| Some((name, value?))),
            );
            if let Some(color) = color {
                fields.push(("color".to_owned(), text(color)));
            }
            let taken = |list: &[Step]| {
                if names_in(doc, list).contains(name) {
                    Err(problem(
                        Code::DuplicateName,
                        format!(
                            "there is already a part called \"{name}\" there: choose another name"
                        ),
                    )
                    .got(name.as_str()))
                } else {
                    Ok(())
                }
            };
            let scale_field = |scale: [f64; 3]| {
                let scale = scale.map(|number| tidy_number(number, 4));
                if scale[0] == scale[1] && scale[1] == scale[2] {
                    Val::Num(scale[0])
                } else {
                    Val::three(scale)
                }
            };
            let Some(to) = to else {
                // A free part: in the model's own list, placed by `move`.
                if attach.is_some() || twist.is_some() {
                    return Err(problem(
                        Code::Conflict,
                        "--attach and --twist belong to a join: give --to as well, or leave them out for a part that stands free",
                    ));
                }
                let list: TreePath = vec![key("parts")];
                taken(&list)?;
                if let Some(turn) = free.rotate.filter(|turn| *turn != [0.0; 3]) {
                    fields.push((
                        "rotate".to_owned(),
                        Val::three(turn.map(|number| tidy_number(number, 2))),
                    ));
                }
                if let Some(at) = free.move_to {
                    fields.push((
                        "move".to_owned(),
                        Val::three(at.map(|number| tidy_number(number, 4))),
                    ));
                }
                if let Some(scale) = free.scale.filter(|scale| *scale != [1.0; 3]) {
                    fields.push(("scale".to_owned(), scale_field(scale)));
                }
                doc.append_item(&list, &fields).map_err(cant)?;
                return Ok(Outcome {
                    summary: format!("added \"{name}\" ({what}), standing free"),
                    touched: None,
                });
            };
            if *free != Free::default() && free.scale.is_none() {
                return Err(problem(
                    Code::Conflict,
                    "--move, --stand and --rotate are for a part that stands free: a part joined with --to goes where its socket is",
                ));
            }
            let (target, _) = split_socket(to)?;
            let found = find(doc, target)?;
            let (target_name, socket) = sibling(doc, &found.list, to)?;
            taken(&found.list)?;
            let attach = attach
                .as_deref()
                .or(opposite(socket))
                .unwrap_or("bottom")
                .to_owned();
            fields.push(("attach".to_owned(), text(&attach)));
            fields.push(("to".to_owned(), text(&format!("{target_name}.{socket}"))));
            if let Some(twist) = twist.filter(|twist| *twist != 0.0) {
                fields.push(("twist".to_owned(), Val::Num(twist)));
            }
            if let Some(scale) = free.scale.filter(|scale| *scale != [1.0; 3]) {
                fields.push(("scale".to_owned(), scale_field(scale)));
            }
            doc.append_item(&found.list, &fields).map_err(cant)?;
            Ok(Outcome {
                summary: format!("added \"{name}\" ({what}) on {target_name}.{socket}"),
                touched: Some(found),
            })
        }
        Edit::Remove {
            part,
            with_attached,
        } => {
            let found = find(doc, part)?;
            let names = names_in(doc, &found.list);
            let items = doc.items(&found.list).expect("the part was found");
            // The parts that hang from it, and from those.
            let mut going = vec![found.at];
            loop {
                let more: Vec<usize> = (0..items.len())
                    .filter(|at| !going.contains(at))
                    .filter(|&at| {
                        text_of(&items[at], "to")
                            .and_then(|to| to.rsplit_once('.'))
                            .is_some_and(|(target, _)| {
                                going.iter().any(|&gone| names[gone] == target)
                            })
                    })
                    .collect();
                if more.is_empty() {
                    break;
                }
                going.extend(more);
            }
            if going.len() > 1 && !with_attached {
                let hanging: Vec<String> = going[1..]
                    .iter()
                    .map(|&at| format!("\"{}\"", names[at]))
                    .collect();
                return Err(problem(
                    Code::Conflict,
                    format!(
                        "{} hang{} from \"{part}\": move them first, or remove them too with --with-attached",
                        hanging.join(", "),
                        if hanging.len() == 1 { "s" } else { "" }
                    ),
                ));
            }
            let count = going.len();
            // What goes, by the paths the poses know it by.
            let mut gone: Vec<String> = Vec::new();
            for &at in &going {
                let sibling = Found {
                    at,
                    name: Some(names[at].clone()),
                    inside: beside(&found.inside, &names[at]),
                    ..found.clone()
                };
                gone.extend(every_use(&sibling, model, &beside(part, &names[at])));
            }
            going.sort_unstable();
            for at in going.into_iter().rev() {
                doc.remove_item(&found.list, at).map_err(cant)?;
            }
            let forgotten = motion::poses_forget(doc, &gone)?;
            let mut summary = match count {
                1 => format!("removed \"{part}\""),
                2 => format!("removed \"{part}\" and the part that hung from it"),
                more => format!(
                    "removed \"{part}\" and the {} parts that hung from it",
                    more - 1
                ),
            };
            match forgotten {
                0 => {}
                1 => summary.push_str(", and its line in a pose"),
                lines => summary.push_str(&format!(", and {lines} lines of poses")),
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::Rename { part, new_name } => {
            check_name(new_name)?;
            let found = find(doc, part)?;
            let names = names_in(doc, &found.list);
            if names
                .iter()
                .enumerate()
                .any(|(at, name)| at != found.at && name == new_name)
            {
                return Err(problem(
                    Code::DuplicateName,
                    format!("there is already a part called \"{new_name}\" there"),
                )
                .got(new_name.as_str()));
            }
            set(doc, &found.path(), "name", text(new_name))?;
            if let Some(old) = &found.name {
                // Everything that names it: the parts joined to it, the
                // sockets passed on from it, and the model's zero point.
                let renamed = |written: &str| -> Option<String> {
                    let (target, socket) = written.rsplit_once('.')?;
                    (target == old).then(|| format!("{new_name}.{socket}"))
                };
                let mut changes: Vec<(TreePath, String)> = Vec::new();
                for (at, item) in doc
                    .items(&found.list)
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    if let Some(new) = text_of(item, "to").and_then(renamed) {
                        changes.push((with(&with(&found.list, Step::Index(at)), key("to")), new));
                    }
                }
                let owner: TreePath = found.list[..found.list.len() - 1].to_vec();
                let sockets = with(&owner, key("sockets"));
                for entry in doc.entries(&sockets).unwrap_or_default() {
                    if let Some(new) = text_of(&entry.node, "from").and_then(renamed) {
                        changes.push((with(&with(&sockets, key(&entry.key)), key("from")), new));
                    }
                }
                if owner.is_empty()
                    && let Some(new) = doc.text_at(&[], "origin").and_then(renamed)
                {
                    changes.push((vec![key("origin")], new));
                }
                for (path, new) in changes {
                    doc.set_value(&path, &text(&new)).map_err(cant)?;
                }
            }
            // The poses name it by its path: they follow.
            motion::poses_follow_rename(doc, &every_use(&found, model, part), new_name)?;
            Ok(Outcome {
                summary: format!("renamed \"{part}\" to \"{new_name}\""),
                touched: Some(found),
            })
        }
        Edit::Join { part, to, attach } => {
            let found = find(doc, part)?;
            let (target, socket) = sibling(doc, &found.list, to)?;
            let table = found.path();
            let joined_before = doc.entry(&table, "attach").is_some();
            match attach {
                Some(attach) => set(doc, &table, "attach", text(attach))?,
                None if !joined_before => {
                    set(
                        doc,
                        &table,
                        "attach",
                        text(opposite(socket).unwrap_or("bottom")),
                    )?;
                }
                None => {}
            }
            set(doc, &table, "to", text(&format!("{target}.{socket}")))?;
            // A part that was placed by hand is now placed by its join.
            for field in ["move", "rotate"] {
                doc.remove_field(&table, field).map_err(cant)?;
            }
            Ok(Outcome {
                summary: format!("joined \"{part}\" to {target}.{socket}"),
                touched: Some(found),
            })
        }
        Edit::Set { part, changes } => {
            let Changes {
                shape,
                sizes,
                color,
                face_colors,
                twist,
                move_to,
                rotate,
                scale,
            } = changes;
            let found = find(doc, part)?;
            // Twist is the part's own. What it is made of is written in
            // the definition, for a part that uses one.
            let own = found.path();
            let (table, uses) = making(doc, &found);
            let mut made_changed = false;
            let mut said = Vec::new();
            if let Some(shape) = shape {
                made_changed = true;
                let Some((size_fields, faces)) = shape_words(shape) else {
                    return Err(problem(
                        Code::UnknownShape,
                        format!("there is no shape \"{shape}\""),
                    )
                    .got(shape.as_str())
                    .one_of(shape, Shape::NAMES));
                };
                if doc.entry(&table, "shape").is_none() {
                    return Err(problem(
                        Code::Conflict,
                        format!(
                            "\"{part}\" isn't made from a shape (it uses a defined part, a file or parts of its own), so its shape can't be changed"
                        ),
                    ));
                }
                set(doc, &table, "shape", text(shape))?;
                // Sizes and face colours the new shape doesn't have go.
                for field in Shape::SIZE_FIELDS {
                    if !size_fields.contains(&field) {
                        doc.remove_field(&table, field).map_err(cant)?;
                    }
                }
                let colors = with(&table, key("colors"));
                let gone: Vec<String> = doc
                    .entries(&colors)
                    .unwrap_or_default()
                    .iter()
                    .map(|entry| entry.key.clone())
                    .filter(|face| !faces.contains(&face.as_str()))
                    .collect();
                for face in &gone {
                    doc.remove_field(&colors, face).map_err(cant)?;
                }
                if doc.entries(&colors).is_some_and(<[_]>::is_empty) {
                    doc.remove_field(&table, "colors").map_err(cant)?;
                }
                said.push(format!("shape {shape}"));
                // Only a box can be shaped: another shape loses its
                // changes (and Undo brings them back).
                let changes = doc
                    .items(&with(&table, key("changes")))
                    .map_or(0, <[_]>::len);
                if shape != "box" && doc.entry(&table, "changes").is_some() {
                    doc.remove_field(&table, "changes").map_err(cant)?;
                    said.push(match changes {
                        1 => "dropped 1 change: only a box can be shaped".to_owned(),
                        count => format!("dropped {count} changes: only a box can be shaped"),
                    });
                }
            }
            for (name, value) in sizes.fields() {
                made_changed = true;
                match value {
                    Some(value) => {
                        said.push(format!("{name} {}", value.write(doc.syntax)));
                        set(doc, &table, &name, value)?;
                    }
                    // No bevel, or no taper: the field goes.
                    None => {
                        if doc.entry(&table, &name).is_some() {
                            doc.remove_field(&table, &name).map_err(cant)?;
                        }
                        if name != "bevel_steps" {
                            said.push(format!("no {name}"));
                        } else if sizes.bevel != Some(0.0) {
                            // One cut is what a bevel has unsaid.
                            said.push("bevel_steps 1".to_owned());
                        }
                    }
                }
            }
            if let Some(color) = color {
                made_changed = true;
                set(doc, &table, "color", text(color))?;
                said.push(format!("colour {color}"));
            }
            for (face, color) in face_colors {
                made_changed = true;
                let colors = with(&table, key("colors"));
                if color == "none" {
                    // The face has no colour of its own again: it takes
                    // the part's.
                    if doc.entry(&colors, face).is_some() {
                        doc.remove_field(&colors, face).map_err(cant)?;
                        if doc.entries(&colors).is_some_and(<[_]>::is_empty) {
                            doc.remove_field(&table, "colors").map_err(cant)?;
                        }
                    }
                    said.push(format!("{face} no colour of its own"));
                    continue;
                }
                if doc.entries(&colors).is_some() {
                    doc.set_field(&colors, face, &text(color), &[])
                        .map_err(cant)?;
                } else {
                    set(
                        doc,
                        &table,
                        "colors",
                        Val::Table(vec![(face.clone(), text(color))]),
                    )?;
                }
                said.push(format!("{face} {color}"));
            }
            if let Some(twist) = twist {
                set(doc, &own, "twist", Val::Num(*twist))?;
                said.push(format!("twist {}", doc::number_text(*twist)));
            }
            // Where a free part is. A joined part is where its socket is.
            let joined = doc.text_at(&own, "to").map(str::to_owned);
            if let Some(to) = move_to {
                if let Some(joined) = &joined {
                    return Err(problem(
                        Code::Conflict,
                        format!(
                            "\"{part}\" is joined to {joined}, so it goes where that socket is. Slide the socket with set-socket, or make the part free first with unjoin"
                        ),
                    ));
                }
                let value = Val::three(to.map(|number| tidy_number(number, 4)));
                said.push(format!("move {}", value.write(doc.syntax)));
                set(doc, &own, "move", value)?;
            }
            if let Some(turn) = rotate {
                if let Some(joined) = &joined {
                    return Err(problem(
                        Code::Conflict,
                        format!(
                            "\"{part}\" is joined to {joined}, so it faces the way that socket does. Turn it around its join with --twist, or make it free first with unjoin"
                        ),
                    ));
                }
                let value = Val::three(turn.map(|number| tidy_number(number, 2)));
                said.push(format!("rotate {}", value.write(doc.syntax)));
                set(doc, &own, "rotate", value)?;
            }
            if let Some(scale) = scale {
                let scale = scale.map(|number| tidy_number(number, 4));
                let value = if scale[0] == scale[1] && scale[1] == scale[2] {
                    Val::Num(scale[0])
                } else {
                    Val::three(scale)
                };
                said.push(format!("scale {}", value.write(doc.syntax)));
                set(doc, &own, "scale", value)?;
            }
            if said.is_empty() {
                return Err(problem(
                    Code::MissingField,
                    "nothing to set: give at least one of --shape, --size, --radius, --tube, --wall, --height, --segments, --steps, --bevel, --bevel-steps, --taper, --color, --face-color, --twist, --move, --rotate, --scale",
                ));
            }
            // Changing what a definition is made of changes every part
            // that uses it.
            let touched = match uses {
                Some(definition) if made_changed => Found {
                    definition: Some(definition),
                    inside: String::new(),
                    ..found
                },
                _ => found,
            };
            Ok(Outcome {
                summary: format!("set \"{part}\": {}", said.join(", ")),
                touched: Some(touched),
            })
        }
        Edit::RenameSocket { socket, new_name } => {
            let (part, name) = split_socket(socket)?;
            check_name(new_name)?;
            let found = declared(doc, part, name)?;
            if Side::from_name(new_name).is_some() || new_name == "slope" {
                return Err(problem(
                    Code::DuplicateName,
                    format!("\"{new_name}\" is a built-in socket's name: choose another"),
                )
                .got(new_name.as_str()));
            }
            if declared_sockets(doc, &found).contains(new_name) {
                return Err(problem(
                    Code::DuplicateName,
                    format!("\"{part}\" already has a socket called \"{new_name}\""),
                )
                .got(new_name.as_str()));
            }
            // Everything that names it, first; then the socket itself.
            let uses = socket_uses(doc, &found, name);
            let own_name = found.name.clone().unwrap_or_default();
            for (path, _) in &uses {
                let last = path.last();
                let value = if matches!(last, Some(Step::Key(field)) if field == "attach") {
                    text(new_name)
                } else {
                    text(&format!("{own_name}.{new_name}"))
                };
                doc.set_value(path, &value).map_err(cant)?;
            }
            doc.rename_field(&with(&found.path(), key("sockets")), name, new_name)
                .map_err(cant)?;
            Ok(Outcome {
                summary: format!("renamed the socket \"{part}.{name}\" to \"{new_name}\""),
                touched: Some(found),
            })
        }
        Edit::RemoveSocket { socket } => {
            let (part, name) = split_socket(socket)?;
            let found = declared(doc, part, name)?;
            let uses = socket_uses(doc, &found, name);
            if !uses.is_empty() {
                let what: Vec<String> = uses.into_iter().map(|(_, called)| called).collect();
                return Err(problem(
                    Code::Conflict,
                    format!(
                        "the socket \"{part}.{name}\" is in use by: {}. Join that somewhere else first, or remove it",
                        what.join(", ")
                    ),
                )
                .got(name));
            }
            drop_socket(doc, &found.path(), name)?;
            Ok(Outcome {
                summary: format!("removed the socket \"{part}.{name}\""),
                touched: Some(found),
            })
        }
        Edit::TidySockets => {
            // Found first, removed after: removing one changes nothing
            // about where the others are.
            let mut unused: Vec<(TreePath, String, String)> = Vec::new();
            for list in part_lists(doc) {
                let count = doc.items(&list).map_or(0, <[_]>::len);
                for at in 0..count {
                    let part = with(&list, Step::Index(at));
                    let name = doc.text_at(&part, "name").map(str::to_owned);
                    let found = Found {
                        list: list.clone(),
                        at,
                        name: name.clone(),
                        definition: None,
                        inside: String::new(),
                    };
                    for socket in declared_sockets(doc, &found) {
                        if is_studio_socket(&socket) && socket_uses(doc, &found, &socket).is_empty()
                        {
                            let called = name.clone().unwrap_or_else(|| format!("#{at}"));
                            unused.push((
                                part.clone(),
                                socket.clone(),
                                format!("{called}.{socket}"),
                            ));
                        }
                    }
                }
            }
            for (part, socket, _) in &unused {
                drop_socket(doc, part, socket)?;
            }
            let names: Vec<String> = unused.into_iter().map(|(_, _, called)| called).collect();
            Ok(Outcome {
                summary: match names.len() {
                    0 => {
                        "found no sockets to tidy away: every \"spot\" socket is in use".to_owned()
                    }
                    1 => format!("removed 1 socket nothing uses: {}", names[0]),
                    count => format!("removed {count} sockets nothing uses: {}", names.join(", ")),
                },
                touched: None,
            })
        }
        Edit::Unjoin { part } => {
            let found = find(doc, part)?;
            let own = found.path();
            let Some(joined) = doc.text_at(&own, "to").map(str::to_owned) else {
                return Err(problem(
                    Code::Conflict,
                    format!("\"{part}\" isn't joined to anything: it is free already"),
                ));
            };
            let Some((model, placed)) =
                model.and_then(|model| Some((model, part_in(model, part)?)))
            else {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "the recipe has mistakes, so where \"{part}\" stands isn't known: fix them first"
                    ),
                ));
            };
            // Where it is in its own list. The model's list is shifted to
            // put the model's zero point at zero; a recipe measures from
            // its own zero.
            let mut at = placed.local.shift;
            if !part.contains('/') {
                at = at - model.zero;
            }
            // Its turn, without the flip and stretch it has of its own:
            // those stay in `mirror` and `scale`.
            let stretch = stretch_of(doc.node(&own).expect("the part was found"));
            let Some(unstretch) = stretch.inverse() else {
                return Err(problem(
                    Code::Unsupported,
                    format!("\"{part}\" is scaled to nothing, so it can't be placed"),
                ));
            };
            let turn = (placed.local.turn * unstretch).to_rotate();
            for field in ["attach", "to", "twist"] {
                doc.remove_field(&own, field).map_err(cant)?;
            }
            let turn = turn.map(|degrees| tidy_number(degrees, 2));
            if turn != [0.0; 3] {
                set(doc, &own, "rotate", Val::three(turn))?;
            }
            set(
                doc,
                &own,
                "move",
                Val::three(at.to_array().map(|number| tidy_number(number, 4))),
            )?;
            Ok(Outcome {
                summary: format!("unjoined \"{part}\" from {joined}: it is free, where it stood"),
                touched: Some(found),
            })
        }
        Edit::Duplicate {
            part,
            name,
            to,
            mirror,
        } => {
            let found = find(doc, part)?;
            let taken = names_in(doc, &found.list);
            let new_name = match (name, &found.name) {
                (Some(name), _) => {
                    check_name(name)?;
                    if taken.contains(name) {
                        return Err(problem(
                            Code::DuplicateName,
                            format!("there is already a part called \"{name}\" there"),
                        )
                        .got(name.as_str()));
                    }
                    Some(name.clone())
                }
                (None, Some(old)) => {
                    let swapped = swap_sides(old);
                    Some(if *mirror && swapped != *old && !taken.contains(&swapped) {
                        swapped
                    } else {
                        free_name(old, &taken)
                    })
                }
                (None, None) => None,
            };
            doc.copy_item(&found.list, found.at).map_err(cant)?;
            let copy = with(&found.list, Step::Index(found.at + 1));
            if let Some(new_name) = &new_name {
                set(doc, &copy, "name", text(new_name))?;
            }
            let mut where_to = String::new();
            let old_to = doc.text_at(&copy, "to").map(str::to_owned);
            match (to, &old_to) {
                (Some(to), _) => {
                    let (target, socket) = sibling(doc, &found.list, to)?;
                    if old_to.is_none() {
                        set(
                            doc,
                            &copy,
                            "attach",
                            text(opposite(socket).unwrap_or("bottom")),
                        )?;
                        for field in ["move", "rotate"] {
                            doc.remove_field(&copy, field).map_err(cant)?;
                        }
                    }
                    set(doc, &copy, "to", text(&format!("{target}.{socket}")))?;
                    where_to = format!(", on {target}.{socket}");
                }
                (None, Some(old_to)) if *mirror => {
                    let swapped = swap_sides(old_to);
                    if swapped != *old_to {
                        set(doc, &copy, "to", text(&swapped))?;
                        where_to = format!(", on {swapped}");
                    }
                }
                (None, Some(old_to)) => {
                    where_to = place_copy(doc, &found, &copy, part, old_to, model)?;
                }
                _ => {}
            }
            if *mirror {
                match doc.text_at(&copy, "mirror").map(str::to_owned) {
                    None => set(doc, &copy, "mirror", text("x"))?,
                    Some(axis) if axis == "x" => doc.remove_field(&copy, "mirror").map_err(cant)?,
                    Some(axis) => {
                        return Err(problem(
                            Code::Conflict,
                            format!(
                                "\"{part}\" is already mirrored across {axis}, so a left-to-right mirror image of it can't be written with one \"mirror\""
                            ),
                        ));
                    }
                }
                // And so do its limits: a turn around y or z goes the
                // other way.
                if let limits @ Limits::Turns(_) = motion::limits_at(doc, &copy)
                    && let Some(value) = motion::limits_val(limits.mirrored())
                {
                    set(doc, &copy, "limits", value)?;
                }
                // A twist seen in a mirror turns the other way.
                let twist = doc
                    .entry(&copy, "twist")
                    .and_then(|entry| entry.node.value.as_number())
                    .filter(|twist| *twist != 0.0);
                if let Some(twist) = twist {
                    set(doc, &copy, "twist", Val::Num(-twist))?;
                }
            }
            let shown = new_name.unwrap_or_else(|| format!("#{}", found.at + 1));
            Ok(Outcome {
                summary: format!(
                    "copied \"{part}\" as \"{shown}\"{}{where_to}",
                    if *mirror { ", mirrored" } else { "" }
                ),
                touched: Some(found),
            })
        }
        Edit::AddColor { name, hex } => {
            check_name(name)?;
            check_hex(hex)?;
            let palette = [key("palette")];
            match doc.node(&palette).map(|node| &node.value) {
                None => {
                    let fields = vec![(name.clone(), text(hex))];
                    doc.add_table(&[], "palette", &fields, &crate::recipe::TOP_FIELDS)
                        .map_err(cant)?;
                }
                Some(Value::Table(entries)) => {
                    if entries.iter().any(|entry| entry.key == *name) {
                        return Err(problem(
                            Code::DuplicateName,
                            format!(
                                "the palette already has a colour called \"{name}\": change it with set-color"
                            ),
                        )
                        .got(name.as_str()));
                    }
                    doc.set_field(&palette, name, &text(hex), &[])
                        .map_err(cant)?;
                }
                Some(Value::Str(file)) => {
                    return Err(problem(
                        Code::Unsupported,
                        format!(
                            "this recipe's palette is the file \"{file}\": add the colour there"
                        ),
                    ));
                }
                Some(_) => {
                    return Err(problem(
                        Code::TypeMismatch,
                        "the recipe's palette isn't a table",
                    ));
                }
            }
            Ok(Outcome {
                summary: format!("added the colour \"{name}\" ({hex})"),
                touched: None,
            })
        }
        Edit::SetColor { name, hex } => {
            check_hex(hex)?;
            let palette = [key("palette")];
            if let Some(file) = doc.text_at(&[], "palette") {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "this recipe's palette is the file \"{file}\": change the colour there"
                    ),
                ));
            }
            let names: Vec<String> = doc
                .entries(&palette)
                .unwrap_or_default()
                .iter()
                .map(|entry| entry.key.clone())
                .collect();
            if !names.contains(name) {
                return Err(problem(
                    Code::UnknownColor,
                    format!("there is no colour \"{name}\" in this recipe's palette"),
                )
                .got(name.as_str())
                .one_of(name, names));
            }
            doc.set_value(&with(&palette, key(name)), &text(hex))
                .map_err(cant)?;
            Ok(Outcome {
                summary: format!("changed the colour \"{name}\" to {hex}"),
                touched: None,
            })
        }
        Edit::RemoveColor { name, replace_with } => {
            let palette = [key("palette")];
            if let Some(file) = doc.text_at(&[], "palette") {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "this recipe's palette is the file \"{file}\": remove the colour there"
                    ),
                ));
            }
            let names: Vec<String> = doc
                .entries(&palette)
                .unwrap_or_default()
                .iter()
                .map(|entry| entry.key.clone())
                .collect();
            if !names.contains(name) {
                return Err(problem(
                    Code::UnknownColor,
                    format!("there is no colour \"{name}\" in this recipe's palette"),
                )
                .got(name.as_str())
                .one_of(name, names));
            }
            // Everything painted with it: a part's `color`, and a face's
            // colour in `colors`, anywhere in the recipe.
            let mut uses: Vec<(TreePath, String)> = Vec::new();
            fn painted(
                node: &Node,
                path: &mut TreePath,
                name: &str,
                into: &mut Vec<(TreePath, String)>,
            ) {
                match &node.value {
                    Value::Table(entries) => {
                        // What to call this table: a part's name, if it
                        // has one.
                        let called = entries
                            .iter()
                            .find(|entry| entry.key == "name")
                            .and_then(|entry| match &entry.node.value {
                                Value::Str(text) => Some(text.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| doc::show(path));
                        for entry in entries {
                            path.push(key(&entry.key));
                            match (&entry.node.value, entry.key.as_str()) {
                                (Value::Str(color), "color") if color == name => {
                                    into.push((path.clone(), called.clone()));
                                }
                                (Value::Table(faces), "colors") => {
                                    for face in faces {
                                        if matches!(&face.node.value, Value::Str(color) if color == name)
                                        {
                                            path.push(key(&face.key));
                                            into.push((
                                                path.clone(),
                                                format!("{called} ({})", face.key),
                                            ));
                                            path.pop();
                                        }
                                    }
                                }
                                _ => painted(&entry.node, path, name, into),
                            }
                            path.pop();
                        }
                    }
                    Value::Array(items) => {
                        for (at, item) in items.iter().enumerate() {
                            path.push(Step::Index(at));
                            painted(item, path, name, into);
                            path.pop();
                        }
                    }
                    _ => {}
                }
            }
            for top in ["parts", "define"] {
                if let Some(node) = doc.node(&[key(top)]) {
                    painted(node, &mut vec![key(top)], name, &mut uses);
                }
            }
            let mut summary = format!("removed the colour \"{name}\"");
            match (replace_with, uses.is_empty()) {
                (_, true) => {}
                (None, false) => {
                    let mut what: Vec<String> =
                        uses.iter().map(|(_, called)| called.clone()).collect();
                    what.dedup();
                    let others: Vec<&String> =
                        names.iter().filter(|other| *other != name).collect();
                    let hint = match others.first() {
                        Some(other) => {
                            format!("Say what to paint them instead, like --replace-with {other}")
                        }
                        None => "Add another colour first, to paint them with instead".to_owned(),
                    };
                    return Err(problem(
                        Code::Conflict,
                        format!(
                            "the colour \"{name}\" is in use: {} painted with it ({}). {hint}",
                            if what.len() == 1 {
                                "1 thing is".to_owned()
                            } else {
                                format!("{} things are", what.len())
                            },
                            what.join(", ")
                        ),
                    )
                    .got(name.as_str()));
                }
                (Some(other), false) => {
                    if other == name || !names.contains(other) {
                        return Err(problem(
                            Code::UnknownColor,
                            format!(
                                "\"{other}\" can't replace \"{name}\": it must be another colour of this recipe's palette"
                            ),
                        )
                        .got(other.as_str())
                        .one_of(other, names.iter().filter(|known| *known != name).cloned()));
                    }
                    for (path, _) in &uses {
                        doc.set_value(path, &text(other)).map_err(cant)?;
                    }
                    summary.push_str(&format!(
                        ", and painted {} with \"{other}\" instead",
                        if uses.len() == 1 {
                            "1 thing".to_owned()
                        } else {
                            format!("{} things", uses.len())
                        }
                    ));
                }
            }
            doc.remove_field(&palette, name).map_err(cant)?;
            Ok(Outcome {
                summary,
                touched: None,
            })
        }
        Edit::AddSocket {
            part,
            name,
            on,
            shift,
        } => {
            let found = add_socket(doc, part, name, on, *shift)?;
            Ok(Outcome {
                summary: format!("added the socket \"{name}\" on the {on} of \"{part}\""),
                touched: Some(found),
            })
        }
        Edit::Pull {
            part,
            faces,
            by,
            name,
        }
        | Edit::Inset {
            part,
            faces,
            by,
            name,
        } => {
            let pull = matches!(edit, Edit::Pull { .. });
            let word = if pull { "pull" } else { "inset" };
            let mut change = vec![
                (word.to_owned(), faces_val(faces)?),
                ("by".to_owned(), Val::Num(tidy_number(*by, 4))),
            ];
            if let Some(name) = name {
                check_name(name)?;
                change.push(("name".to_owned(), text(name)));
            }
            let (found, definition) = add_change(doc, part, change)?;
            let faces = crate::shaping::quoted(faces);
            let amount = doc::number_text(by.abs());
            let mut summary = if pull {
                let way = if *by < 0.0 { "in" } else { "out" };
                format!("pulled {faces} of \"{part}\" {way} by {amount}")
            } else {
                format!("inset {faces} of \"{part}\" by {amount}")
            };
            if let Some(name) = name {
                summary.push_str(&format!(", naming the new faces \"{name}\""));
            }
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::SetChange {
            part,
            number,
            by,
            by_xyz,
            at,
        } => {
            let (found, list, count, definition) = changes_of(doc, part)?;
            if *number == 0 || *number > count {
                return Err(problem(
                    Code::OutOfRange,
                    if count == 1 {
                        format!("\"{part}\" has 1 change: it is number 1")
                    } else {
                        format!(
                            "\"{part}\" has {count} changes: they are numbered from 1 to {count}"
                        )
                    },
                )
                .got(*number));
            }
            if by.is_none() && by_xyz.is_none() && at.is_none() {
                return Err(problem(Code::MissingField, "nothing to set: give --by"));
            }
            let change = with(&list, Step::Index(number - 1));
            let words = change_words(doc, &change);
            const ORDER: [&str; 3] = ["by", "at", "name"];
            let mut said = Vec::new();
            if let Some(by) = by_xyz {
                if doc.entry(&change, "by").is_none() {
                    return Err(problem(
                        Code::UnknownField,
                        format!("change {number} of \"{part}\" ({words}) has no \"by\""),
                    ));
                }
                let value = Val::three(by.map(|number| tidy_number(number, 4)));
                said.push(format!("by {}", value.write(doc.syntax)));
                doc.set_field(&change, "by", &value, &ORDER).map_err(cant)?;
            }
            for (field, value) in [("by", by), ("at", at)] {
                let Some(value) = value else {
                    continue;
                };
                if doc.entry(&change, field).is_none() {
                    return Err(problem(
                        Code::UnknownField,
                        format!("change {number} of \"{part}\" ({words}) has no \"{field}\""),
                    ));
                }
                let value = tidy_number(*value, 4);
                doc.set_field(&change, field, &Val::Num(value), &ORDER)
                    .map_err(cant)?;
                said.push(format!("{field} {}", doc::number_text(value)));
            }
            let mut summary = format!(
                "set change {number} of \"{part}\" ({words}): {}",
                said.join(", ")
            );
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::RemoveChange { part, number } => {
            let (found, list, count, definition) = changes_of(doc, part)?;
            let table = list[..list.len() - 1].to_vec();
            let mut summary = match number {
                None => {
                    doc.remove_field(&table, "changes").map_err(cant)?;
                    if count == 1 {
                        format!("took away the only change of \"{part}\": it is a plain box again")
                    } else {
                        format!(
                            "took away all {count} changes of \"{part}\": it is a plain box again"
                        )
                    }
                }
                Some(number) if *number == 0 || *number > count => {
                    return Err(problem(
                        Code::OutOfRange,
                        format!(
                            "\"{part}\" has {count} changes: they are numbered from 1 to {count}"
                        ),
                    )
                    .got(*number));
                }
                Some(number) => {
                    let change = with(&list, Step::Index(number - 1));
                    let words = change_words(doc, &change);
                    if count == 1 {
                        doc.remove_field(&table, "changes").map_err(cant)?;
                    } else {
                        doc.remove_item(&list, number - 1).map_err(cant)?;
                    }
                    format!("took away change {number} of \"{part}\" ({words})")
                }
            };
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::MoveFace { part, faces, by }
        | Edit::TurnFace { part, faces, by }
        | Edit::ScaleFace { part, faces, by } => {
            let (word, together, doing, done) = match edit {
                Edit::MoveFace { .. } => ("move", Together::Add, "moved", "moved"),
                Edit::TurnFace { .. } => ("turn", Together::Add, "turned", "turned"),
                _ => ("scale", Together::Times, "scaled", "scaled"),
            };
            let one = matches!(together, Together::Times);
            let (found, definition, joined) = add_or_join(doc, part, word, faces, *by, together)?;
            let names = crate::shaping::quoted(faces);
            let mut summary = format!(
                "{doing} {names} of \"{part}\" by {}",
                amount_words(*by, one)
            );
            match joined {
                None => {}
                Some(Some(all)) => summary.push_str(&format!(
                    " ({done} by {} in all, in one change)",
                    amount_words(all, one)
                )),
                Some(None) => summary.push_str(": back where it was, so the change is gone"),
            }
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::MoveEdge { part, edges, by }
        | Edit::MoveCorner {
            part,
            corners: edges,
            by,
        } => {
            let corner = matches!(edit, Edit::MoveCorner { .. });
            joined_val(edges, if corner { "corner" } else { "edge" })?;
            let (found, definition, joined) =
                add_or_join(doc, part, "move", edges, *by, Together::Add)?;
            let what = match (corner, edges.len()) {
                (true, 1) => "the corner",
                (true, _) => "the corners",
                (false, 1) => "the edge",
                (false, _) => "the edges",
            };
            let mut summary = format!(
                "moved {what} {} of \"{part}\" by {}",
                crate::shaping::quoted(edges),
                amount_words(*by, false)
            );
            match joined {
                None => {}
                Some(Some(all)) => summary.push_str(&format!(
                    " (moved by {} in all, in one change)",
                    amount_words(all, false)
                )),
                Some(None) => summary.push_str(": back where it was, so the change is gone"),
            }
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::Cut {
            part,
            toward,
            at,
            name,
        } => {
            let sides = ["top", "bottom", "left", "right", "front", "back"];
            if !sides.contains(&toward.as_str()) {
                return Err(problem(
                    Code::TypeMismatch,
                    format!("a cut faces one of the box's sides, not \"{toward}\""),
                )
                .got(toward.as_str())
                .one_of(toward, sides));
            }
            if !(0.0..1.0).contains(at) {
                return Err(problem(
                    Code::OutOfRange,
                    format!(
                        "the cut towards \"{toward}\" at {} is outside the part: \"at\" goes from 0 (the middle) to just under 1 (the side)",
                        doc::number_text(*at)
                    ),
                )
                .got(*at));
            }
            let mut change = vec![
                ("cut".to_owned(), text(toward)),
                ("at".to_owned(), Val::Num(tidy_number(*at, 4))),
            ];
            if let Some(name) = name {
                check_name(name)?;
                change.push(("name".to_owned(), text(name)));
            }
            let (found, definition) = add_change(doc, part, change)?;
            let mut summary = format!(
                "cut \"{part}\" towards \"{toward}\" at {}",
                doc::number_text(*at)
            );
            if let Some(name) = name {
                summary.push_str(&format!(", naming the slices \"{name}\""));
            }
            if let Some(definition) = definition {
                summary.push_str(&format!(" (in the definition \"{definition}\")"));
            }
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
        Edit::Define { part, name } => {
            check_name(name)?;
            let found = find(doc, part)?;
            if found.list != [key("parts")] || found.definition.is_some() {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "only a part of the model's own list can be made a definition in this version: \"{part}\" is inside another part"
                    ),
                ));
            }
            let table = found.path();
            let own = doc
                .node(&table)
                .ok_or_else(|| problem(Code::UnknownPart, "no such part"))?;
            if let Some(used) = text_of(own, "use") {
                return Err(problem(
                    Code::Conflict,
                    format!("\"{part}\" is made from the definition \"{used}\" already"),
                ));
            }
            if has_field(own, "parts") {
                return Err(problem(
                    Code::Unsupported,
                    format!(
                        "\"{part}\" is made of parts of its own: only a part made from a shape or a file can be made a definition in this version"
                    ),
                ));
            }
            if doc.node(&[key("define"), key(name)]).is_some() {
                return Err(problem(
                    Code::DuplicateName,
                    format!("there is a definition \"{name}\" already: choose another name"),
                )
                .got(name.as_str()));
            }
            let fields = made_of(doc, &table);
            for (field, _) in fields.iter().rev() {
                doc.remove_field(&table, field).map_err(cant)?;
            }
            set(doc, &table, "use", text(name))?;
            match doc.syntax {
                Syntax::Toml => {
                    let header = format!("define.{name}");
                    doc.add_block_before(&table, &header, &fields)
                        .map_err(cant)?;
                }
                Syntax::Json => {
                    let definition = Val::Table(fields);
                    if doc.entries(&[key("define")]).is_some() {
                        doc.set_field(&[key("define")], name, &definition, &[])
                            .map_err(cant)?;
                    } else {
                        doc.set_field(
                            &[],
                            "define",
                            &Val::Table(vec![(name.clone(), definition)]),
                            &crate::recipe::TOP_FIELDS,
                        )
                        .map_err(cant)?;
                    }
                }
            }
            let found = find(doc, part)?;
            Ok(Outcome {
                summary: format!(
                    "made \"{part}\" a definition, \"{name}\": it is in [define.{name}], and \"{part}\" uses it"
                ),
                touched: Some(found),
            })
        }
        Edit::SetSocket {
            socket,
            on,
            shift,
            facing,
        } => {
            let (part, name) = split_socket(socket)?;
            if let Some(on) = on {
                check_side(on)?;
            }
            if let Some(facing) = facing
                && !FACINGS.contains(&facing.as_str())
            {
                return Err(problem(
                    Code::TypeMismatch,
                    format!("there is no direction \"{facing}\""),
                )
                .got(facing.as_str())
                .one_of(facing, FACINGS));
            }
            let found = find(doc, part)?;
            let sockets = with(&found.path(), key("sockets"));
            let names: Vec<String> = doc
                .entries(&sockets)
                .unwrap_or_default()
                .iter()
                .map(|entry| entry.key.clone())
                .collect();
            if !names.iter().any(|known| known == name) {
                let issue = problem(
                    Code::UnknownSocket,
                    if Side::from_name(name).is_some() || name == "slope" {
                        format!(
                            "\"{name}\" is built in, and can't be moved: declare a socket of your own with add-socket"
                        )
                    } else {
                        format!("\"{part}\" declares no socket called \"{name}\"")
                    },
                );
                return Err(issue.got(name).one_of(name, names));
            }
            let place = with(&sockets, key(name));
            if doc.entry(&place, "on").is_none() {
                return Err(problem(
                    Code::Conflict,
                    format!(
                        "the socket \"{name}\" isn't on a side of the part (it is written with \"at\" or \"from\"), so it can't be slid or turned with set-socket: change it in the recipe"
                    ),
                ));
            }
            if on.is_none() && shift.is_none() && facing.is_none() {
                return Err(problem(
                    Code::MissingField,
                    "nothing to set: give --shift, --on, --facing, or more than one",
                ));
            }
            const ORDER: [&str; 3] = ["on", "shift", "facing"];
            if let Some(on) = on {
                doc.set_field(&place, "on", &text(on), &ORDER)
                    .map_err(cant)?;
            }
            if let Some(shift) = shift {
                doc.set_field(&place, "shift", &Val::three(*shift), &ORDER)
                    .map_err(cant)?;
            }
            match facing.as_deref() {
                Some("out") => {
                    if doc.entry(&place, "facing").is_some() {
                        doc.remove_field(&place, "facing").map_err(cant)?;
                    }
                }
                Some(facing) => {
                    doc.set_field(&place, "facing", &text(facing), &ORDER)
                        .map_err(cant)?;
                }
                None => {}
            }
            let summary = match (facing, on.is_some() || shift.is_some()) {
                (Some(facing), false) if facing == "out" => {
                    format!("turned the socket {part}.{name} to face straight out")
                }
                (Some(facing), false) => {
                    format!("turned the socket {part}.{name} to face {facing}")
                }
                (Some(facing), true) => {
                    format!("moved the socket {part}.{name}, facing {facing}")
                }
                (None, _) => format!("moved the socket {part}.{name}"),
            };
            Ok(Outcome {
                summary,
                touched: Some(found),
            })
        }
    }
}

/// Adds a change to the end of a part's `changes`, starting the list if
/// there is none. For a part made with `use`, the definition is
/// changed. Gives where it went, and the definition.
fn add_change(
    doc: &mut Doc,
    part: &str,
    change: Vec<(String, Val)>,
) -> Result<(Found, Option<String>), Stop> {
    let found = find(doc, part)?;
    let (table, definition) = making(doc, &found);
    let list = with(&table, key("changes"));
    let change = Val::Table(change);
    if doc.items(&list).is_some() {
        doc.push_item(&list, &change).map_err(cant)?;
    } else if doc.entry(&table, "changes").is_some() {
        return Err(problem(
            Code::TypeMismatch,
            format!(
                "\"{part}\" has \"changes\" that isn't a list: put it right in the recipe first"
            ),
        ));
    } else {
        doc.set_field(&table, "changes", &Val::Lines(vec![change]), &PART_FIELDS)
            .map_err(cant)?;
    }
    // Shaping a definition changes every part that uses it.
    let found = match &definition {
        Some(definition) => Found {
            definition: Some(definition.clone()),
            inside: String::new(),
            ..found
        },
        None => found,
    };
    Ok((found, definition))
}

/// What a part is made of, as fields to copy: its shape and sizes (or
/// its file), colours, changes and own sockets, as they are written.
const MADE_OF: [&str; 17] = [
    "shape",
    "file",
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
    "sockets",
    "note",
];

/// A value of the tree, to write again.
fn val_of(node: &Node) -> Option<Val> {
    Some(match &node.value {
        Value::Str(text) => Val::Str(text.clone()),
        Value::Int(whole) => Val::Num(*whole as f64),
        Value::Float(number) => Val::Num(*number),
        Value::Array(items) => Val::List(items.iter().filter_map(val_of).collect()),
        Value::Table(entries) => Val::Table(
            entries
                .iter()
                .filter_map(|entry| Some((entry.key.clone(), val_of(&entry.node)?)))
                .collect(),
        ),
        _ => return None,
    })
}

/// The fields of `table` that say what a part is made of, as written.
fn made_of(doc: &Doc, table: &[Step]) -> Vec<(String, Val)> {
    MADE_OF
        .iter()
        .filter_map(|field| {
            doc.entry(table, field)?;
            Some(((*field).to_owned(), doc.raw(&with(table, key(field)))?))
        })
        .collect()
}

/// How a move, a turn or a scale of the same faces right after another
/// is put together with it.
#[derive(Clone, Copy)]
enum Together {
    /// Moves and turns add up.
    Add,
    /// Scales multiply.
    Times,
}

/// Adds a move, turn or scale (`word`) of `faces` by `by` to a part's
/// changes, or, when the last change is the same of the same faces,
/// puts the two together in that one. Gives where it went, the
/// definition, and the amount in all if it was put together (`None`:
/// a new line; `Some(None)`: the two cancel out and the line went).
#[allow(clippy::type_complexity)]
fn add_or_join(
    doc: &mut Doc,
    part: &str,
    word: &str,
    faces: &[String],
    by: [f64; 3],
    together: Together,
) -> Result<(Found, Option<String>, Option<Option<[f64; 3]>>), Stop> {
    let faces_written = if faces.iter().any(|name| name.contains('+')) {
        let what = if faces[0].matches('+').count() == 1 {
            "edge"
        } else {
            "corner"
        };
        joined_val(faces, what)?
    } else {
        faces_val(faces)?
    };
    let found = find(doc, part)?;
    let (table, definition) = making(doc, &found);
    let list = with(&table, key("changes"));
    let count = doc.items(&list).map_or(0, <[_]>::len);
    let is_same = |doc: &Doc| -> Option<[f64; 3]> {
        let last = with(&list, Step::Index(count.checked_sub(1)?));
        let entries = doc.entries(&last)?;
        if entries.len() != 2
            || entries
                .iter()
                .any(|entry| entry.key != word && entry.key != "by")
        {
            return None;
        }
        let written = &doc.entry(&last, word)?.node.value;
        let same_faces = match (written, &faces_written) {
            (Value::Str(one), Val::Str(other)) => one == other,
            (Value::Array(items), Val::List(others)) => {
                items.len() == others.len()
                    && items.iter().zip(others).all(|(item, other)| {
                        matches!((&item.value, other), (Value::Str(a), Val::Str(b)) if a == b)
                    })
            }
            _ => false,
        };
        if !same_faces {
            return None;
        }
        match &doc.entry(&last, "by")?.node.value {
            Value::Array(items) if items.len() == 3 => {
                let numbers: Vec<f64> = items
                    .iter()
                    .filter_map(|item| item.value.as_number())
                    .collect();
                <[f64; 3]>::try_from(numbers).ok()
            }
            other => other.as_number().map(|all| [all; 3]),
        }
    };
    let found_for = |definition: &Option<String>, found: Found| match definition {
        Some(definition) => Found {
            definition: Some(definition.clone()),
            inside: String::new(),
            ..found
        },
        None => found,
    };
    if let Some(was) = is_same(doc) {
        let mut all = [0.0; 3];
        for at in 0..3 {
            all[at] = tidy_number(
                match together {
                    Together::Add => was[at] + by[at],
                    Together::Times => was[at] * by[at],
                },
                4,
            );
        }
        let last = with(&list, Step::Index(count - 1));
        let nothing = match together {
            Together::Add => all == [0.0; 3],
            Together::Times => all == [1.0; 3],
        };
        if nothing {
            // Back where it started: the line goes.
            if count == 1 {
                doc.remove_field(&table, "changes").map_err(cant)?;
            } else {
                doc.remove_item(&list, count - 1).map_err(cant)?;
            }
            return Ok((found_for(&definition, found), definition, Some(None)));
        }
        let value = match together {
            Together::Times if all[0] == all[1] && all[1] == all[2] => Val::Num(all[0]),
            _ => Val::three(all),
        };
        doc.set_field(&last, "by", &value, &["by"]).map_err(cant)?;
        return Ok((found_for(&definition, found), definition, Some(Some(all))));
    }
    let value = match together {
        Together::Times if by[0] == by[1] && by[1] == by[2] => Val::Num(tidy_number(by[0], 4)),
        _ => Val::three(by.map(|number| tidy_number(number, 4))),
    };
    let (found, definition) = add_change(
        doc,
        part,
        vec![(word.to_owned(), faces_written), ("by".to_owned(), value)],
    )?;
    Ok((found, definition, None))
}

/// `[0, 0.2, 0]`, or one number for three alike when `one` is set.
fn amount_words(by: [f64; 3], one: bool) -> String {
    if one && by[0] == by[1] && by[1] == by[2] {
        return doc::number_text(by[0]);
    }
    let [x, y, z] = by.map(doc::number_text);
    format!("[{x}, {y}, {z}]")
}

/// Where a part's changes are written, and how many there are, with the
/// definition they are in. Refused, saying so, if it has none.
fn changes_of(doc: &Doc, part: &str) -> Result<(Found, TreePath, usize, Option<String>), Stop> {
    let found = find(doc, part)?;
    let (table, definition) = making(doc, &found);
    let list = with(&table, key("changes"));
    let count = doc.items(&list).map_or(0, <[_]>::len);
    if count == 0 {
        return Err(problem(
            Code::UnknownField,
            format!("\"{part}\" has no changes: pull or inset one of its faces first"),
        ));
    }
    let found = match &definition {
        Some(definition) => Found {
            definition: Some(definition.clone()),
            inside: String::new(),
            ..found
        },
        None => found,
    };
    Ok((found, list, count, definition))
}

/// One change as it is written, in a few words: `pull "glass"`.
fn change_words(doc: &Doc, change: &[Step]) -> String {
    let Some(Value::Table(entries)) = doc.node(change).map(|node| &node.value) else {
        return "the change".to_owned();
    };
    for entry in entries {
        if entry.key == "by" || entry.key == "name" || entry.key == "at" {
            continue;
        }
        let faces = match &entry.node.value {
            Value::Str(face) => format!("\"{face}\""),
            Value::Array(items) => {
                let names: Vec<String> = items
                    .iter()
                    .filter_map(|item| match &item.value {
                        Value::Str(face) => Some(face.clone()),
                        _ => None,
                    })
                    .collect();
                crate::shaping::quoted(&names)
            }
            _ => String::new(),
        };
        return format!("{} {faces}", entry.key);
    }
    "the change".to_owned()
}

/// Edges or corners, as a change writes them: `top+front`, or a list.
fn joined_val(names: &[String], what: &str) -> Result<Val, Stop> {
    if names.is_empty() {
        return Err(problem(Code::MissingField, format!("no {what} was named")));
    }
    for name in names {
        let pieces: Vec<&str> = name.split('+').collect();
        let wanted = if what == "edge" { 2 } else { 3 };
        if pieces.len() < wanted || pieces.iter().any(|piece| check_name(piece).is_err()) {
            let example = if what == "edge" {
                "top+front"
            } else {
                "top+front+left"
            };
            return Err(problem(
                Code::TypeMismatch,
                format!(
                    "\"{name}\" isn't {}: write the names of the faces that meet there, joined with \"+\", like \"{example}\"",
                    if what == "edge" { "an edge" } else { "a corner" }
                ),
            )
            .got(name.as_str()));
        }
    }
    Ok(match names {
        [one] => text(one),
        many => Val::List(many.iter().map(|name| text(name)).collect()),
    })
}

/// The faces a change names, as it is written: one name, or a list.
fn faces_val(faces: &[String]) -> Result<Val, Stop> {
    if faces.is_empty() {
        return Err(problem(Code::MissingField, "no face was named"));
    }
    for face in faces {
        if face.contains('+') {
            return Err(problem(
                Code::TypeMismatch,
                format!("\"{face}\" is an edge or a corner: this works on faces, like \"top\""),
            )
            .got(face.as_str()));
        }
        check_name(face)?;
    }
    Ok(match faces {
        [one] => text(one),
        many => Val::List(many.iter().map(|face| text(face)).collect()),
    })
}

/// A side of a part (`top`...), or a face of a shaped box by its name:
/// that is checked when the recipe is built.
fn check_side(on: &str) -> Result<(), Stop> {
    let sides: Vec<&str> = Side::ALL.map(Side::name).to_vec();
    if sides.contains(&on) || crate::recipe::is_name(on) {
        Ok(())
    } else {
        Err(problem(
            Code::TypeMismatch,
            format!("\"{on}\" isn't a side of a part"),
        )
        .got(on)
        .one_of(on, sides))
    }
}

/// The parts that share a definition with a part: what else an edit to
/// it changes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shared {
    /// The definition's name.
    pub definition: String,
    /// Every other part that changes with it, by path.
    pub also_changes: Vec<String>,
}

/// Says whether a part comes from a definition, and names every other
/// part that an edit to it would change too. `None` for a part that is
/// written once. The Studio shows this before any edit is made.
pub fn shared(text: &str, syntax: Syntax, model: Option<&Model>, part: &str) -> Option<Shared> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = Doc::parse(text, syntax).ok()?;
    let found = find(&doc, part).ok()?;
    share(&found, model, part)
}

/// A path with its last name replaced: `left_arm/hand` and `elbow` give
/// `left_arm/elbow`.
fn beside(path: &str, name: &str) -> String {
    match path.rsplit_once('/') {
        Some((around, _)) => format!("{around}/{name}"),
        None => name.to_owned(),
    }
}

/// Every part of the model that the recipe's part `part` is: itself,
/// and, for a part inside a definition, the same part in every part
/// that uses the definition.
fn every_use(found: &Found, model: Option<&Model>, part: &str) -> Vec<String> {
    let mut all = vec![part.to_owned()];
    if let Some(shared) = share(found, model, part) {
        all.extend(shared.also_changes);
    }
    all
}

fn share(found: &Found, model: Option<&Model>, part: &str) -> Option<Shared> {
    let definition = found.definition.clone()?;
    fn users<'a>(parts: &'a [Part], definition: &str, into: &mut Vec<&'a Part>) {
        for part in parts {
            if part.kind.uses.as_deref() == Some(definition) {
                into.push(part);
            }
            users(&part.parts, definition, into);
        }
    }
    let mut using = Vec::new();
    if let Some(model) = model {
        users(&model.parts, &definition, &mut using);
    }
    let also_changes = using
        .iter()
        .map(|user| {
            if found.inside.is_empty() {
                user.path.clone()
            } else {
                format!("{}/{}", user.path, found.inside)
            }
        })
        .filter(|path| path != part)
        .collect();
    Some(Shared {
        definition,
        also_changes,
    })
}

/// The names of the parts the recipe defines, to `use`: what the Studio
/// offers beside the built-in shapes when a part is added.
pub fn definitions(text: &str, syntax: Syntax) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Ok(doc) = Doc::parse(text, syntax) else {
        return Vec::new();
    };
    doc.entries(&[key("define")])
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect()
}

/// A name that isn't taken yet: `base` itself, or `base_2`, `base_3`...
pub fn unused_name(base: &str, taken: &[String]) -> String {
    if taken.iter().any(|name| name == base) {
        free_name(base, taken)
    } else {
        base.to_owned()
    }
}

/// What the recipe says about one part: what the Studio's properties
/// panel shows, read from the file as it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    /// Its own name, if it has one.
    pub name: Option<String>,
    /// What it is made of. `None` for a part made of parts of its own.
    pub made: Option<Made>,
    /// The built-in shape it is, its own or its definition's. `None` for
    /// an assembly or a part from a file.
    pub shape: Option<String>,
    /// The sizes that shape has, with the default for each one the
    /// recipe leaves out. Empty when it isn't a shape.
    pub sizes: Sizes,
    /// Its palette colour, its own or its definition's.
    pub color: Option<String>,
    /// The colours its single faces have: `colors = { top = "grass" }`,
    /// its own or its definition's.
    pub face_colors: Vec<(String, String)>,
    /// The sockets it declares, by name, each with how many things use
    /// it (parts joined to it, its own join, a socket that passes it
    /// on).
    pub sockets: Vec<(String, usize)>,
    /// How far it may turn when the model is posed: its `limits`.
    pub limits: Limits,
    /// Which way each of `sockets` faces, as one of [`FACINGS`]: `None`
    /// for one written with `at` or `from`, or with three numbers.
    pub socket_facings: Vec<Option<String>>,
    /// The changes made to its shape, its own or its definition's, in
    /// order.
    pub changes: Vec<ChangeFact>,
    /// The socket it is joined to: `body.neck`.
    pub to: Option<String>,
    /// Its own socket that joins there.
    pub attach: Option<String>,
    /// Degrees it is twisted around its join.
    pub twist: f64,
    /// Where a free part's middle is: its `move`, measured from the
    /// recipe's own zero. (For a joined part: zero, and not used.)
    pub position: [f64; 3],
    /// How a free part is turned: its `rotate`, in degrees.
    pub rotation: [f64; 3],
    /// How much bigger it is each way: its `scale`, as three numbers.
    pub scale: [f64; 3],
    /// The direction it is flipped across, if it is: `x`, `y` or `z`.
    pub mirror: Option<String>,
    /// Set when the part is inside a definition: every edit to it
    /// changes the other parts named here too.
    pub inside: Option<Shared>,
    /// Set when the part is made with `use`: an edit to its shape, sizes
    /// or colours changes the definition, and so the other parts named
    /// here.
    pub uses: Option<Shared>,
}

/// One change to a shape, as the recipe writes it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeFact {
    /// What it does: `pull`, `inset`.
    pub word: String,
    /// The faces it names.
    pub faces: Vec<String>,
    /// Its `by`, when it is one number.
    pub by: Option<f64>,
    /// Its `by`, when it is three: a move, a turn, a scale each way.
    pub by_xyz: Option<[f64; 3]>,
    /// Its `at`: where a cut is.
    pub at: Option<f64>,
    /// The name it gives what it makes, if it has one.
    pub name: Option<String>,
}

/// Reads what the recipe says about a part. `None` if the text can't be
/// read, the part isn't in it, or it is inside another file.
pub fn describe(text: &str, syntax: Syntax, model: Option<&Model>, part: &str) -> Option<Facts> {
    fn number(node: &Node, name: &str) -> Option<f64> {
        let Value::Table(entries) = &node.value else {
            return None;
        };
        match entries.iter().find(|entry| entry.key == name)?.node.value {
            Value::Int(whole) => Some(whole as f64),
            Value::Float(number) => Some(number),
            _ => None,
        }
    }
    fn two(node: &Node, name: &str) -> Option<[f64; 2]> {
        let Value::Table(entries) = &node.value else {
            return None;
        };
        let Value::Array(items) = &entries.iter().find(|entry| entry.key == name)?.node.value
        else {
            return None;
        };
        let numbers: Vec<f64> = items
            .iter()
            .filter_map(|item| item.value.as_number())
            .collect();
        <[f64; 2]>::try_from(numbers).ok()
    }
    fn three(node: &Node, name: &str) -> Option<[f64; 3]> {
        let Value::Table(entries) = &node.value else {
            return None;
        };
        let Value::Array(items) = &entries.iter().find(|entry| entry.key == name)?.node.value
        else {
            return None;
        };
        let numbers: Vec<f64> = items
            .iter()
            .filter_map(|item| match item.value {
                Value::Int(whole) => Some(whole as f64),
                Value::Float(number) => Some(number),
                _ => None,
            })
            .collect();
        <[f64; 3]>::try_from(numbers).ok()
    }

    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = Doc::parse(text, syntax).ok()?;
    let found = find(&doc, part).ok()?;
    let own = doc.node(&found.path())?;
    let (made_at, via) = making(&doc, &found);
    let made_node = doc.node(&made_at)?;

    let made = if let Some(defined) = text_of(own, "use") {
        Some(Made::Use(defined.to_owned()))
    } else if let Some(file) = text_of(own, "file") {
        Some(Made::File(file.to_owned()))
    } else {
        text_of(own, "shape").map(|shape| Made::Shape(shape.to_owned()))
    };
    let shape = text_of(made_node, "shape")
        .filter(|shape| shape_words(shape).is_some())
        .map(str::to_owned);
    let mut sizes = Sizes::default();
    if let Some((fields, _)) = shape.as_deref().and_then(shape_words) {
        let shape = shape.as_deref().unwrap_or_default();
        let whole = |name: &str, default: u32| {
            Some(number(made_node, name).map_or(default, |count| count.max(0.0) as u32))
        };
        for field in fields {
            match *field {
                "size" => sizes.size = Some(three(made_node, "size").unwrap_or([1.0; 3])),
                "radius" => {
                    let default = if shape == "capsule" { 0.25 } else { 0.5 };
                    sizes.radius = Some(number(made_node, "radius").unwrap_or(default));
                }
                "tube" => sizes.tube = Some(number(made_node, "tube").unwrap_or(0.15)),
                "wall" => {
                    let default = if shape == "arch" { 0.2 } else { 0.1 };
                    sizes.wall = Some(number(made_node, "wall").unwrap_or(default));
                }
                "height" => sizes.height = Some(number(made_node, "height").unwrap_or(1.0)),
                "segments" => {
                    sizes.segments = whole(
                        "segments",
                        match shape {
                            "sphere" => 12,
                            "arch" => 8,
                            _ => 16,
                        },
                    );
                }
                "steps" => sizes.steps = whole("steps", 4),
                "bevel" => sizes.bevel = Some(number(made_node, "bevel").unwrap_or(0.0)),
                "bevel_steps" => sizes.bevel_steps = whole("bevel_steps", 1),
                _ => {
                    sizes.taper = Some(
                        number(made_node, "taper")
                            .map(|all| [all; 2])
                            .or_else(|| two(made_node, "taper"))
                            .unwrap_or([1.0; 2]),
                    );
                }
            }
        }
    }
    let inside = share(&found, model, part);
    let uses = via.and_then(|definition| {
        share(
            &Found {
                definition: Some(definition),
                inside: String::new(),
                ..found.clone()
            },
            model,
            part,
        )
    });
    Some(Facts {
        name: found.name.clone(),
        made,
        shape,
        sizes,
        color: text_of(made_node, "color").map(str::to_owned),
        face_colors: match &made_node.value {
            Value::Table(entries) => entries
                .iter()
                .find(|entry| entry.key == "colors")
                .and_then(|entry| match &entry.node.value {
                    Value::Table(faces) => Some(faces),
                    _ => None,
                })
                .map(|faces| {
                    faces
                        .iter()
                        .filter_map(|face| match &face.node.value {
                            Value::Str(color) => Some((face.key.clone(), color.clone())),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        },
        sockets: declared_sockets(&doc, &found)
            .into_iter()
            .map(|socket| {
                let uses = socket_uses(&doc, &found, &socket).len();
                (socket, uses)
            })
            .collect(),
        limits: motion::limits_at(&doc, &found.path()),
        socket_facings: declared_sockets(&doc, &found)
            .iter()
            .map(|socket| facing_word(&doc, &found, socket))
            .collect(),
        changes: doc
            .items(&with(&made_at, key("changes")))
            .unwrap_or_default()
            .iter()
            .filter_map(|item| {
                let Value::Table(entries) = &item.value else {
                    return None;
                };
                let action = entries
                    .iter()
                    .find(|entry| !["by", "at", "name"].contains(&entry.key.as_str()))?;
                let faces = match &action.node.value {
                    Value::Str(face) => vec![face.clone()],
                    Value::Array(items) => items
                        .iter()
                        .filter_map(|item| match &item.value {
                            Value::Str(face) => Some(face.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                Some(ChangeFact {
                    word: action.key.clone(),
                    faces,
                    by: number(item, "by"),
                    by_xyz: three(item, "by"),
                    at: number(item, "at"),
                    name: text_of(item, "name").map(str::to_owned),
                })
            })
            .collect(),
        to: text_of(own, "to").map(str::to_owned),
        attach: text_of(own, "attach").map(str::to_owned),
        twist: number(own, "twist").unwrap_or(0.0),
        position: three(own, "move").unwrap_or([0.0; 3]),
        rotation: three(own, "rotate").unwrap_or([0.0; 3]),
        scale: three(own, "scale")
            .or_else(|| number(own, "scale").map(|all| [all; 3]))
            .unwrap_or([1.0; 3]),
        mirror: text_of(own, "mirror").map(str::to_owned),
        inside,
        uses,
    })
}

/// A recipe's text after an edit, checked and built.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// The whole new text.
    pub text: String,
    /// What the edit did, in a few words.
    pub summary: String,
    /// The definition the edited part is inside, with every other part
    /// that changed with it.
    pub shared: Option<Shared>,
    /// The first and last line of the new text that differ from the old
    /// (the line it was at, twice, if lines were only taken out).
    /// `None` if nothing changed.
    pub lines: Option<[usize; 2]>,
    /// The model the new text builds. `None` if the recipe has mistakes
    /// (which it then had before the edit too).
    pub model: Option<Model>,
    /// Everything noticed while building the new text.
    pub issues: Vec<Issue>,
}

fn build(text: &str, syntax: Syntax, path: &Path, display: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded = load_text_at(text, syntax, path, display, &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, issues)
}

/// Works out an edit on a recipe's text, and checks the result. Reads no
/// file but those the recipe names, and writes nothing. `path` is the
/// recipe's file, for finding those; `display` is its name in messages.
///
/// Refused (with nothing changed) if the edit can't be carried out, or if
/// the recipe would have more mistakes after it than before.
pub fn apply(
    text: &str,
    syntax: Syntax,
    path: &Path,
    display: &str,
    edit: &Edit,
) -> Result<Applied, Failure> {
    apply_many(text, syntax, path, display, std::slice::from_ref(edit))
}

/// The part an edit is about, by path (nothing for a palette edit).
fn part_of(edit: &Edit) -> &str {
    match edit {
        Edit::Add { to, .. } => to
            .as_deref()
            .and_then(|to| to.rsplit_once('.'))
            .map_or("", |(part, _)| part),
        Edit::Remove { part, .. }
        | Edit::Rename { part, .. }
        | Edit::Join { part, .. }
        | Edit::Set { part, .. }
        | Edit::Duplicate { part, .. }
        | Edit::Unjoin { part }
        | Edit::AddSocket { part, .. }
        | Edit::Pull { part, .. }
        | Edit::Inset { part, .. }
        | Edit::SetChange { part, .. }
        | Edit::RemoveChange { part, .. }
        | Edit::MoveFace { part, .. }
        | Edit::TurnFace { part, .. }
        | Edit::ScaleFace { part, .. }
        | Edit::MoveEdge { part, .. }
        | Edit::MoveCorner { part, .. }
        | Edit::Define { part, .. }
        | Edit::SetLimits { part, .. }
        | Edit::SetPose { part, .. }
        | Edit::Cut { part, .. } => part,
        Edit::SetSocket { socket, .. }
        | Edit::RenameSocket { socket, .. }
        | Edit::RemoveSocket { socket } => socket.rsplit_once('.').map_or("", |(part, _)| part),
        Edit::AddColor { .. }
        | Edit::SetColor { .. }
        | Edit::RemoveColor { .. }
        | Edit::AddPose { .. }
        | Edit::RenamePose { .. }
        | Edit::RemovePose { .. }
        | Edit::MirrorPose { .. }
        | Edit::TidySockets => "",
    }
}

/// How far apart a new part stands from the others when no place is
/// given, and the steps it walks in to get clear: 10 cm.
pub const CLEAR_STEP: f64 = 0.1;

/// Where on the ground a new part stands clear of the others: from
/// `from` (x and z), a `step` at a time `towards` a flat direction (x
/// and z, any length), until its footprint, `half` as wide and deep as
/// it is plus a `step` round it, misses every one of `others` (boxes,
/// in the same measurements, standing on `ground`). A box wholly above
/// the new part's height doesn't count. The spot is a whole number of
/// `step`s. Gives up after 2000 steps, where it is.
pub fn clear_spot(
    others: &[Bounds],
    half: Vec3,
    ground: f64,
    from: [f64; 2],
    towards: [f64; 2],
    step: f64,
) -> [f64; 2] {
    let snapped =
        |x: f64, z: f64| [x, z].map(|number| ((number / step).round() * step * 1e6).round() / 1e6);
    let clear = |at: [f64; 2]| {
        others.iter().all(|other| {
            other.min.y - ground >= half.y * 2.0
                || at[0] + half.x + step <= other.min.x + 1e-9
                || at[0] - half.x - step >= other.max.x - 1e-9
                || at[1] + half.z + step <= other.min.z + 1e-9
                || at[1] - half.z - step >= other.max.z - 1e-9
        })
    };
    let flat = (towards[0] * towards[0] + towards[1] * towards[1]).sqrt();
    let towards = if flat > 1e-6 {
        [towards[0] / flat, towards[1] / flat]
    } else {
        [0.0, 1.0]
    };
    (0..2000)
        .map(|nearer| {
            let nearer = f64::from(nearer) * step;
            snapped(from[0] + towards[0] * nearer, from[1] + towards[1] * nearer)
        })
        .find(|at| clear(*at))
        .unwrap_or_else(|| snapped(from[0], from[1]))
}

/// Declares the socket `name` on `part`, on a side (or a face of a
/// shaped box), slid across it by `shift`: what `add-socket` does.
fn add_socket(
    doc: &mut Doc,
    part: &str,
    name: &str,
    on: &str,
    shift: [f64; 3],
) -> Result<Found, Stop> {
    check_name(name)?;
    check_side(on)?;
    let found = find(doc, part)?;
    let table = found.path();
    let mut socket = vec![("on".to_owned(), text(on))];
    if shift != [0.0; 3] {
        socket.push(("shift".to_owned(), Val::three(shift)));
    }
    let sockets = with(&table, key("sockets"));
    match doc.entries(&sockets) {
        Some(entries) => {
            if entries.iter().any(|entry| entry.key == *name) {
                return Err(problem(
                    Code::DuplicateName,
                    format!(
                        "\"{part}\" already has a socket called \"{name}\": move it with set-socket, or choose another name"
                    ),
                )
                .got(name));
            }
            doc.set_field(&sockets, name, &Val::Table(socket), &[])
                .map_err(cant)?;
        }
        None => {
            doc.add_table(
                &table,
                "sockets",
                &[(name.to_owned(), Val::Table(socket))],
                &PART_FIELDS,
            )
            .map_err(cant)?;
        }
    }
    Ok(found)
}

/// Where the copy of a joined part goes, when no socket is given: not
/// on top of the part it copies. In turn:
///
/// 1. the next socket of the same part that nothing is joined to and
///    that faces the same way, in the order they are written;
/// 2. a new socket (`spot`, `spot_2`...) on the same side, beside the
///    socket of the part it copies, 10 cm past it, if the side has room
///    and nothing else stands there;
/// 3. for a part in the model's own list, free, standing on clear ground
///    in front of the model.
///
/// Otherwise (a part inside another part with no room), on the same
/// socket. `copy` is the copy, already in `doc`; `old_to` is the socket
/// both are joined to. Says where, for the summary.
fn place_copy(
    doc: &mut Doc,
    found: &Found,
    copy: &[Step],
    part: &str,
    old_to: &str,
    model: Option<&Model>,
) -> Result<String, Stop> {
    let same = format!(", on {old_to}, the same socket: nothing else was free");
    let Some((target, socket)) = old_to.rsplit_once('.') else {
        return Ok(same);
    };
    let target_path = match part.rsplit_once('/') {
        Some((around, _)) => format!("{around}/{target}"),
        None => target.to_owned(),
    };
    let Some((model, aim, original)) =
        model.and_then(|model| Some((model, part_in(model, &target_path)?, part_in(model, part)?)))
    else {
        return Ok(same);
    };
    let Some(joined) = aim.sockets.iter().find(|known| known.name == socket) else {
        return Ok(same);
    };
    // 1. A free socket facing the same way.
    let taken: Vec<String> = doc
        .items(&found.list)
        .unwrap_or_default()
        .iter()
        .filter_map(|item| text_of(item, "to"))
        .filter_map(|to| to.rsplit_once('.'))
        .filter(|(on, _)| *on == target)
        .map(|(_, name)| name.to_owned())
        .collect();
    let built_in = |name: &str| name == "slope" || Side::ALL.iter().any(|side| side.name() == name);
    let declared: Vec<_> = aim
        .sockets
        .iter()
        .filter(|known| !built_in(&known.name))
        .collect();
    let after = declared
        .iter()
        .position(|known| known.name == socket)
        .map_or(0, |at| at + 1);
    let next = declared[after..]
        .iter()
        .chain(&declared[..after])
        .find(|known| !taken.contains(&known.name) && known.facing.dot(joined.facing) > 0.999);
    if let Some(next) = next {
        let to = format!("{target}.{}", next.name);
        set(doc, copy, "to", text(&to))?;
        return Ok(format!(", on {to}, the next free socket"));
    }
    // 2. A new socket beside, on the same side.
    let siblings: &[Part] = match part.rsplit_once('/') {
        Some((around, _)) => part_in(model, around).map_or(&[], |around| &around.parts),
        None => &model.parts,
    };
    if let (Some(own), Some(room)) = (original.bounds, aim.bounds) {
        let facing = joined.facing;
        let axes = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ]
        .map(Vec3::from);
        let beside = axes
            .into_iter()
            .filter(|axis| axis.dot(facing).abs() < 1e-3)
            .find_map(|axis| {
                let width = own
                    .size()
                    .dot(Vec3::from([axis.x.abs(), axis.y.abs(), axis.z.abs()]));
                let by = axis * (width + CLEAR_STEP);
                let at = joined.at + by;
                // Still on the side: inside the part's box across it.
                let across = |at: f64, low: f64, high: f64, along: f64| {
                    along.abs() > 0.5 || (at >= low - 1e-6 && at <= high + 1e-6)
                };
                let on_side = across(at.x, room.min.x, room.max.x, facing.x)
                    && across(at.y, room.min.y, room.max.y, facing.y)
                    && across(at.z, room.min.z, room.max.z, facing.z);
                let moved = Bounds {
                    min: own.min + by,
                    max: own.max + by,
                };
                let crowded = siblings
                    .iter()
                    .filter(|other| other.path != aim.path)
                    .any(|other| {
                        other.bounds.is_some_and(|other| {
                            moved.min.x < other.max.x - 1e-6
                                && moved.max.x > other.min.x + 1e-6
                                && moved.min.y < other.max.y - 1e-6
                                && moved.max.y > other.min.y + 1e-6
                                && moved.min.z < other.max.z - 1e-6
                                && moved.max.z > other.min.z + 1e-6
                        })
                    });
                (on_side && !crowded).then_some(at)
            })
            .and_then(|at| spot(model, aim, at, facing));
        if let Some(beside) = beside {
            let names: Vec<String> = aim.sockets.iter().map(|known| known.name.clone()).collect();
            let name = unused_name("spot", &names);
            add_socket(doc, &target_path, &name, &beside.on, beside.shift)?;
            let to = format!("{target}.{name}");
            set(doc, copy, "to", text(&to))?;
            return Ok(format!(", on {to}, a new socket beside {old_to}"));
        }
    }
    // 3. Free, in front of the model.
    if part.contains('/') {
        return Ok(same);
    }
    let Some(own) = original.bounds else {
        return Ok(same);
    };
    let stretch = stretch_of(doc.node(copy).expect("the copy was made"));
    let Some(unstretch) = stretch.inverse() else {
        return Ok(same);
    };
    let turn = (original.local.turn * unstretch)
        .to_rotate()
        .map(|degrees| tidy_number(degrees, 2));
    let ground = model.bounds().map_or(own.min.y, |all| all.min.y);
    let others: Vec<Bounds> = model
        .parts
        .iter()
        .filter_map(|other| other.bounds)
        .collect();
    let from = model.bounds().map_or(own.center(), |all| all.center());
    let spot = clear_spot(
        &others,
        own.half(),
        ground,
        [from.x, from.z],
        [0.0, 1.0],
        CLEAR_STEP,
    );
    let middle = own.center();
    let at = original.local.shift - model.zero
        + Vec3::from([spot[0] - middle.x, ground - own.min.y, spot[1] - middle.z]);
    for field in ["attach", "to", "twist"] {
        doc.remove_field(copy, field).map_err(cant)?;
    }
    if turn != [0.0; 3] {
        set(doc, copy, "rotate", Val::three(turn))?;
    }
    set(
        doc,
        copy,
        "move",
        Val::three(at.to_array().map(|number| tidy_number(number, 4))),
    )?;
    Ok(", standing free in front of the model: no socket beside it was free".to_owned())
}

/// Gives the new free part `name` the `move` that stands it on the
/// ground with its middle over `at` (x and z, in the recipe's own
/// measurements), or, with no `at`, on clear ground: from the middle of
/// the model towards its front until it stands clear of the other
/// parts ([`clear_spot`]). `built` is the recipe with the part in it at
/// zero; `before` is the model without it, whose ground is the ground.
/// If the recipe doesn't build, the part stays at zero.
fn stand_on_ground(
    doc: &mut Doc,
    name: &str,
    at: Option<[f64; 2]>,
    built: Option<&Model>,
    before: Option<&Model>,
) -> Result<(), Stop> {
    let Some(built) = built else {
        return Ok(());
    };
    let Some(bounds) = built
        .parts
        .iter()
        .find(|part| part.name == name)
        .and_then(|part| part.bounds)
    else {
        return Ok(());
    };
    // The box round the part, in the recipe's own measurements.
    let (low, middle) = (bounds.min - built.zero, bounds.center() - built.zero);
    // The ground is where the model stood before the part came. (A
    // first part makes its own ground.)
    let ground = before
        .and_then(|model| model.bounds().map(|all| all.min.y - model.zero.y))
        .unwrap_or(low.y);
    let at = at.unwrap_or_else(|| {
        let Some(model) = before else {
            return [0.0; 2];
        };
        let others: Vec<Bounds> = model
            .parts
            .iter()
            .filter_map(|part| part.bounds)
            .map(|other| Bounds {
                min: other.min - model.zero,
                max: other.max - model.zero,
            })
            .collect();
        let from = model
            .bounds()
            .map_or(Vec3::ZERO, |all| all.center() - model.zero);
        clear_spot(
            &others,
            bounds.half(),
            ground,
            [from.x, from.z],
            [0.0, 1.0],
            CLEAR_STEP,
        )
    });
    let to =
        [at[0] - middle.x, ground - low.y, at[1] - middle.z].map(|number| tidy_number(number, 4));
    let found = find(doc, name)?;
    set(doc, &found.path(), "move", Val::three(to))
}

/// [`apply`] for several edits, made one after the other on the same
/// text and checked once, together: the Studio's "one step".
pub fn apply_many(
    text: &str,
    syntax: Syntax,
    path: &Path,
    display: &str,
    edits: &[Edit],
) -> Result<Applied, Failure> {
    // The invisible mark some Windows editors put first stays first.
    let (mark, body) = match text.strip_prefix('\u{feff}') {
        Some(body) => ("\u{feff}", body),
        None => ("", text),
    };
    let (old_model, old_issues) = build(text, syntax, path, display);
    let mut doc = Doc::parse(body, syntax)
        .map_err(|_| Failure::Invalid(Report::from_issues(old_issues.clone())))?;
    let mut said = Vec::new();
    let mut shared = None;
    for edit in edits {
        let outcome = run(&mut doc, edit, old_model.as_ref())
            .map_err(|stop| Failure::Usage(Box::new(stop.0.in_file(display))))?;
        said.push(outcome.summary);
        // A new part that stands free, with no place given, is stood on
        // the ground.
        if let Edit::Add {
            name,
            to: None,
            free,
            ..
        } = edit
            && free.move_to.is_none()
        {
            let built = build(&format!("{mark}{}", doc.text), syntax, path, display).0;
            stand_on_ground(
                &mut doc,
                name,
                free.stand,
                built.as_ref(),
                old_model.as_ref(),
            )
            .map_err(|stop| Failure::Usage(Box::new(stop.0.in_file(display))))?;
        }
        if shared.is_none() {
            shared = outcome
                .touched
                .as_ref()
                .and_then(|found| share(found, old_model.as_ref(), part_of(edit)));
        }
    }
    let new_text = format!("{mark}{}", doc.text);
    let (model, issues) = build(&new_text, syntax, path, display);
    let before = Report::from_issues(old_issues).errors.len();
    let report = Report::from_issues(issues.clone());
    if report.errors.len() > before {
        return Err(Failure::Invalid(report));
    }
    Ok(Applied {
        lines: changed_lines(text, &new_text),
        text: new_text,
        summary: said.join(", then "),
        shared,
        model,
        issues,
    })
}

/// A part alone, as a recipe of its own: what it is made of (from its
/// definition, if it uses one), with the palette colours it uses, and
/// none of where it stands. What `duck-mold save-part` writes. `path`
/// is the recipe's file; the text is always TOML.
pub fn part_recipe(text: &str, syntax: Syntax, path: &Path, part: &str) -> Result<String, Failure> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let display = display_path(path);
    let usage = |stop: Stop| Failure::Usage(Box::new(stop.0.in_file(display.as_str())));
    let (model, issues) = build(text, syntax, path, &display);
    let Some(model) = model else {
        return Err(Failure::Invalid(Report::from_issues(issues)));
    };
    let doc = Doc::parse(text, syntax)
        .map_err(|_| Failure::Invalid(Report::from_issues(issues.clone())))?;
    let found = find(&doc, part).map_err(usage)?;
    let (table, _) = making(&doc, &found);
    let fields: Vec<(String, Val)> = match syntax {
        Syntax::Toml => made_of(&doc, &table),
        // JSON's tables aren't TOML's: written again from what they say.
        Syntax::Json => MADE_OF
            .iter()
            .filter_map(|field| {
                let node = &doc.entry(&table, field)?.node;
                Some(((*field).to_owned(), val_of(node)?))
            })
            .collect(),
    };
    if fields.iter().any(|(field, _)| field == "file") {
        return Err(usage(problem(
            Code::Unsupported,
            format!("\"{part}\" comes from a file already: copy that file to the library instead"),
        )));
    }
    if fields.iter().all(|(field, _)| field != "shape") {
        return Err(usage(problem(
            Code::Unsupported,
            format!(
                "\"{part}\" is made of parts of its own: only a part made from a shape can be saved alone in this version"
            ),
        )));
    }
    let name = found.name.clone().unwrap_or_else(|| "part".to_owned());
    // The colours it uses, as the model has them.
    let mut used: Vec<String> = Vec::new();
    let mut note = |color: &str| {
        if !used.iter().any(|known| known == color) {
            used.push(color.to_owned());
        }
    };
    let made = doc.node(&table);
    if let Some(color) = made.and_then(|node| text_of(node, "color")) {
        note(color);
    }
    if let Some(Value::Table(faces)) = made
        .and_then(|node| match &node.value {
            Value::Table(entries) => entries.iter().find(|entry| entry.key == "colors"),
            _ => None,
        })
        .map(|entry| &entry.node.value)
    {
        for face in faces {
            if let Value::Str(color) = &face.node.value {
                note(color);
            }
        }
    }
    let mut out = format!(
        "# Saved by `duck-mold save-part` from {}.\nversion = 0\nname = {}\n",
        display,
        Val::Str(name.clone()).write(Syntax::Toml)
    );
    let palette: Vec<String> = used
        .iter()
        .filter_map(|color| {
            let known = model.palette.iter().find(|known| known.name == *color)?;
            Some(format!("{} = \"{}\"", color, crate::recipe::hex(known.rgb)))
        })
        .collect();
    if !palette.is_empty() {
        out.push_str(&format!("\n[palette]\n{}\n", palette.join("\n")));
    }
    out.push_str(&format!(
        "\n[[parts]]\nname = {}\n",
        Val::Str(name).write(Syntax::Toml)
    ));
    for (field, value) in &fields {
        out.push_str(&format!("{field} = {}\n", value.write(Syntax::Toml)));
    }
    Ok(out.replace("\r\n", "\n"))
}

/// The first and last line of `new` that aren't in `old`, by taking away
/// the lines the two start and end with alike.
fn changed_lines(old: &str, new: &str) -> Option<[usize; 2]> {
    if old == new {
        return None;
    }
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let same_start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let same_end = old[same_start..]
        .iter()
        .rev()
        .zip(new[same_start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let first = same_start + 1;
    let last = (new.len() - same_end).max(first);
    Some([first, last])
}

/// What an edit did to a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditInfo {
    /// The edit's name: `add`, `set`...
    pub command: &'static str,
    /// What it did, in a few words.
    pub summary: String,
    /// The recipe, as it was named.
    pub file: String,
    /// False for `--dry-run`, and when the edit changed nothing.
    pub written: bool,
    /// The definition the edited part is inside, if it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    /// Every other part that changed with it, by path.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub also_changes: Vec<String>,
    /// The first and last line of the recipe that changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<[usize; 2]>,
}

/// An edit that was made (or, for a dry run, would be).
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    /// The report `inspect` gives for the recipe as it is now. `None` if
    /// the recipe has mistakes, which it had before the edit too.
    pub inspection: Option<Inspection>,
    /// Those mistakes.
    pub errors: Vec<Issue>,
    /// What the edit did.
    pub edit: EditInfo,
    /// The recipe's text before the edit.
    pub before: String,
    /// The recipe's text after it.
    pub after: String,
}

impl Edited {
    /// The report as `--json` prints it: `inspect`'s report with an
    /// `"edit"` field.
    pub fn to_json(&self) -> serde_json::Value {
        let mut value = match &self.inspection {
            Some(inspection) => serde_json::to_value(inspection).expect("a report is plain data"),
            None => serde_json::json!({ "ok": false, "errors": self.errors, "warnings": [] }),
        };
        value["edit"] = serde_json::to_value(&self.edit).expect("a report is plain data");
        value
    }
}

fn io_problem(path: &Path, what: &str, error: &std::io::Error) -> Failure {
    let reason = match error.kind() {
        std::io::ErrorKind::NotFound => "there is no such file",
        std::io::ErrorKind::PermissionDenied => "permission denied",
        _ => "it didn't work",
    };
    Failure::Usage(boxed(problem(
        Code::Io,
        format!("can't {what} \"{}\": {reason}", display_path(path)),
    )))
}

/// Writes a recipe by writing a new file beside it and swapping it in, so
/// that a program reading the recipe never sees half of it. `expected`
/// is the text the file should still hold: if it has changed meanwhile,
/// nothing is written and `Ok(false)` comes back.
pub fn swap_in(path: &Path, expected: Option<&str>, text: &str) -> Result<bool, Failure> {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".duck-mold-saving");
    let beside = path.with_file_name(name);
    std::fs::write(&beside, text).map_err(|error| io_problem(path, "write", &error))?;
    if let Some(expected) = expected {
        let now = std::fs::read_to_string(path).unwrap_or_default();
        if now != expected {
            let _ = std::fs::remove_file(&beside);
            return Ok(false);
        }
    }
    std::fs::rename(&beside, path).map_err(|error| {
        let _ = std::fs::remove_file(&beside);
        io_problem(path, "write", &error)
    })?;
    Ok(true)
}

/// `duck-mold edit`: makes one edit to a recipe file.
///
/// The file is read, the edit is worked out on its text and checked, and
/// the new text is swapped in. If another program saved the file between
/// the reading and the writing, the edit is worked out again on what is
/// there now, so both changes are kept. With `dry_run`, nothing is
/// written.
pub fn edit_file(path: &Path, edit: &Edit, dry_run: bool) -> Result<Edited, Failure> {
    edit_file_many(path, std::slice::from_ref(edit), dry_run)
}

/// [`edit_file`] for several edits made together: worked out one after
/// the other, checked once, and written once.
pub fn edit_file_many(path: &Path, edits: &[Edit], dry_run: bool) -> Result<Edited, Failure> {
    let display = display_path(path);
    let Some(syntax) = Syntax::from_path(path) else {
        return Err(Failure::Usage(boxed(problem(
            Code::Io,
            format!("\"{display}\" isn't a recipe: a recipe's name ends in .toml or .json"),
        ))));
    };
    let mut tries = 0;
    loop {
        tries += 1;
        let before =
            std::fs::read_to_string(path).map_err(|error| io_problem(path, "read", &error))?;
        let applied = apply_many(&before, syntax, path, &display, edits)?;
        let changed = applied.text != before;
        let written = if dry_run || !changed {
            false
        } else if swap_in(path, Some(&before), &applied.text)? {
            true
        } else if tries < 5 {
            continue; // someone else saved it meanwhile: start again
        } else {
            return Err(Failure::Usage(boxed(problem(
                Code::Io,
                format!(
                    "\"{display}\" keeps being changed by another program, so the edit wasn't made: try again"
                ),
            ))));
        };
        let report = Report::from_issues(applied.issues);
        let inspection = applied
            .model
            .as_ref()
            .map(|model| inspect(model, &display, report.warnings.clone()));
        let (definition, also_changes) = match applied.shared {
            Some(shared) => (Some(shared.definition), shared.also_changes),
            None => (None, Vec::new()),
        };
        return Ok(Edited {
            inspection,
            errors: report.errors,
            edit: EditInfo {
                command: edits.first().map_or("", Edit::command),
                summary: applied.summary,
                file: display,
                written,
                definition,
                also_changes,
                lines: applied.lines,
            },
            before,
            after: applied.text,
        });
    }
}
