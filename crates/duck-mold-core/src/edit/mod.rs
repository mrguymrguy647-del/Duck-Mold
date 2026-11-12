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
mod spot;

pub use spot::{Spot, spot};

use std::path::Path;

use serde::Serialize;

use crate::assemble::assemble;
use crate::inspect::{Inspection, inspect};
use crate::issue::{Code, Issue, Report};
use crate::load::{display_path, load_text_at};
use crate::model::{Model, Part};
use crate::recipe::{PART_FIELDS, Shape, Side, rgb_of};
use crate::source::{Node, Syntax, Value};
use crate::tools::Failure;
use doc::{Doc, Step, TreePath, Val, key, with};

/// What a new part is made of.
#[derive(Debug, Clone, PartialEq)]
pub enum Made {
    /// One of the five shapes: `box`, `cylinder`, `sphere`, `cone`,
    /// `wedge`.
    Shape(String),
    /// A part the recipe defines: `use = "arm"`.
    Use(String),
    /// A recipe or a mesh file: `file = "parts/lamp.gltf"`.
    File(String),
}

/// The sizes a shape can have. Only those that are given are written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sizes {
    /// `size = [x, y, z]`, for a box or a wedge.
    pub size: Option<[f64; 3]>,
    /// `radius`, for a cylinder, a sphere or a cone.
    pub radius: Option<f64>,
    /// `height`, for a cylinder or a cone.
    pub height: Option<f64>,
    /// `segments`: how many flat sides a round shape has.
    pub segments: Option<u32>,
}

impl Sizes {
    fn is_empty(&self) -> bool {
        *self == Sizes::default()
    }

    fn fields(&self) -> Vec<(String, Val)> {
        let mut fields = Vec::new();
        if let Some(size) = self.size {
            fields.push(("size".to_owned(), Val::three(size)));
        }
        if let Some(radius) = self.radius {
            fields.push(("radius".to_owned(), Val::Num(radius)));
        }
        if let Some(height) = self.height {
            fields.push(("height".to_owned(), Val::Num(height)));
        }
        if let Some(segments) = self.segments {
            fields.push(("segments".to_owned(), Val::Num(f64::from(segments))));
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
}

/// One change to a recipe. A part is named as `inspect` prints it:
/// `head`, `left_arm/hand`, or `#2` for a part with no name.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Adds a part, joined to a socket.
    Add {
        /// The new part's name.
        name: String,
        /// What it is made of.
        made: Made,
        /// The socket it joins: `head.top`.
        to: String,
        /// Its own socket to join by. Without one: the side opposite the
        /// socket it joins (`bottom` on a `top`), otherwise `bottom`.
        attach: Option<String>,
        /// Its sizes, for a shape.
        sizes: Sizes,
        /// Its palette colour.
        color: Option<String>,
        /// Degrees to twist it around the join.
        twist: Option<f64>,
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
    /// Moves a socket that was declared on a side of a part.
    SetSocket {
        /// The socket: `body.badge`.
        socket: String,
        /// Another side.
        on: Option<String>,
        /// A new slide across the side.
        shift: Option<[f64; 3]>,
    },
}

impl Edit {
    /// The name of every kind of edit, as the command line and the agent
    /// guide write it. A test checks each has a command and is in the
    /// guide.
    pub const COMMANDS: [&'static str; 10] = [
        "add",
        "remove",
        "rename",
        "join",
        "set",
        "duplicate",
        "add-color",
        "set-color",
        "add-socket",
        "set-socket",
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
            Edit::AddSocket { .. } => "add-socket",
            Edit::SetSocket { .. } => "set-socket",
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
    Some(match shape {
        "box" => (
            &["size"],
            &["top", "bottom", "left", "right", "front", "back", "sides"],
        ),
        "wedge" => (&["size"], &["slope", "bottom", "back", "left", "right"]),
        "cylinder" => (
            &["radius", "height", "segments"],
            &["top", "bottom", "side"],
        ),
        "cone" => (&["radius", "height", "segments"], &["side", "bottom"]),
        "sphere" => (&["radius", "segments"], &["side"]),
        _ => return None,
    })
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

fn run(doc: &mut Doc, edit: &Edit) -> Result<Outcome, Stop> {
    match edit {
        Edit::Add {
            name,
            made,
            to,
            attach,
            sizes,
            color,
            twist,
        } => {
            check_name(name)?;
            let (target, _) = split_socket(to)?;
            let found = find(doc, target)?;
            let (target_name, socket) = sibling(doc, &found.list, to)?;
            if names_in(doc, &found.list).contains(name) {
                return Err(problem(
                    Code::DuplicateName,
                    format!("there is already a part called \"{name}\" there: choose another name"),
                )
                .got(name.as_str()));
            }
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
            fields.extend(sizes.fields());
            if let Some(color) = color {
                fields.push(("color".to_owned(), text(color)));
            }
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
            going.sort_unstable();
            for at in going.into_iter().rev() {
                doc.remove_item(&found.list, at).map_err(cant)?;
            }
            let summary = match count {
                1 => format!("removed \"{part}\""),
                2 => format!("removed \"{part}\" and the part that hung from it"),
                more => format!(
                    "removed \"{part}\" and the {} parts that hung from it",
                    more - 1
                ),
            };
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
                for field in ["size", "radius", "height", "segments"] {
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
            }
            for (name, value) in sizes.fields() {
                made_changed = true;
                said.push(format!("{name} {}", value.write(doc.syntax)));
                set(doc, &table, &name, value)?;
            }
            if let Some(color) = color {
                made_changed = true;
                set(doc, &table, "color", text(color))?;
                said.push(format!("colour {color}"));
            }
            for (face, color) in face_colors {
                made_changed = true;
                let colors = with(&table, key("colors"));
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
            if said.is_empty() {
                return Err(problem(
                    Code::MissingField,
                    "nothing to set: give at least one of --shape, --size, --radius, --height, --segments, --color, --face-color, --twist",
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
        Edit::AddSocket {
            part,
            name,
            on,
            shift,
        } => {
            check_name(name)?;
            check_side(on)?;
            let found = find(doc, part)?;
            let table = found.path();
            let mut socket = vec![("on".to_owned(), text(on))];
            if *shift != [0.0; 3] {
                socket.push(("shift".to_owned(), Val::three(*shift)));
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
                        .got(name.as_str()));
                    }
                    doc.set_field(&sockets, name, &Val::Table(socket), &[])
                        .map_err(cant)?;
                }
                None => {
                    doc.add_table(
                        &table,
                        "sockets",
                        &[(name.clone(), Val::Table(socket))],
                        &PART_FIELDS,
                    )
                    .map_err(cant)?;
                }
            }
            Ok(Outcome {
                summary: format!("added the socket \"{name}\" on the {on} of \"{part}\""),
                touched: Some(found),
            })
        }
        Edit::SetSocket { socket, on, shift } => {
            let (part, name) = split_socket(socket)?;
            if let Some(on) = on {
                check_side(on)?;
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
                        "the socket \"{name}\" isn't on a side of the part (it is written with \"at\" or \"from\"), so it can't be slid"
                    ),
                ));
            }
            if on.is_none() && shift.is_none() {
                return Err(problem(
                    Code::MissingField,
                    "nothing to set: give --shift, --on, or both",
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
            Ok(Outcome {
                summary: format!("moved the socket {part}.{name}"),
                touched: Some(found),
            })
        }
    }
}

fn check_side(on: &str) -> Result<(), Stop> {
    let sides: Vec<&str> = Side::ALL.map(Side::name).to_vec();
    if sides.contains(&on) {
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
/// offers beside the five shapes when a part is added.
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
    /// The socket it is joined to: `body.neck`.
    pub to: Option<String>,
    /// Its own socket that joins there.
    pub attach: Option<String>,
    /// Degrees it is twisted around its join.
    pub twist: f64,
    /// Set when the part is inside a definition: every edit to it
    /// changes the other parts named here too.
    pub inside: Option<Shared>,
    /// Set when the part is made with `use`: an edit to its shape, sizes
    /// or colours changes the definition, and so the other parts named
    /// here.
    pub uses: Option<Shared>,
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
        let round = if shape.as_deref() == Some("sphere") {
            12
        } else {
            16
        };
        for field in fields {
            match *field {
                "size" => sizes.size = Some(three(made_node, "size").unwrap_or([1.0; 3])),
                "radius" => sizes.radius = Some(number(made_node, "radius").unwrap_or(0.5)),
                "height" => sizes.height = Some(number(made_node, "height").unwrap_or(1.0)),
                _ => {
                    sizes.segments = Some(
                        number(made_node, "segments").map_or(round, |count| count.max(0.0) as u32),
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
        to: text_of(own, "to").map(str::to_owned),
        attach: text_of(own, "attach").map(str::to_owned),
        twist: number(own, "twist").unwrap_or(0.0),
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
    // The invisible mark some Windows editors put first stays first.
    let (mark, body) = match text.strip_prefix('\u{feff}') {
        Some(body) => ("\u{feff}", body),
        None => ("", text),
    };
    let (old_model, old_issues) = build(text, syntax, path, display);
    let mut doc = Doc::parse(body, syntax)
        .map_err(|_| Failure::Invalid(Report::from_issues(old_issues.clone())))?;
    let outcome =
        run(&mut doc, edit).map_err(|stop| Failure::Usage(Box::new(stop.0.in_file(display))))?;
    let new_text = format!("{mark}{}", doc.text);
    let (model, issues) = build(&new_text, syntax, path, display);
    let before = Report::from_issues(old_issues).errors.len();
    let report = Report::from_issues(issues.clone());
    if report.errors.len() > before {
        return Err(Failure::Invalid(report));
    }
    let part = match edit {
        Edit::Add { to, .. } => to.rsplit_once('.').map_or("", |(part, _)| part),
        Edit::Remove { part, .. }
        | Edit::Rename { part, .. }
        | Edit::Join { part, .. }
        | Edit::Set { part, .. }
        | Edit::Duplicate { part, .. }
        | Edit::AddSocket { part, .. } => part,
        Edit::SetSocket { socket, .. } => socket.rsplit_once('.').map_or("", |(part, _)| part),
        Edit::AddColor { .. } | Edit::SetColor { .. } => "",
    };
    let shared = outcome
        .touched
        .as_ref()
        .and_then(|found| share(found, old_model.as_ref(), part));
    Ok(Applied {
        lines: changed_lines(text, &new_text),
        text: new_text,
        summary: outcome.summary,
        shared,
        model,
        issues,
    })
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
        let applied = apply(&before, syntax, path, &display, edit)?;
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
                command: edit.command(),
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
