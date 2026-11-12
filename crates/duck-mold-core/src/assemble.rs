//! Building the model: parts, sockets, joins and nesting.
//!
//! [`assemble`] takes a recipe that has been read ([`crate::load`]) and
//! builds the [`Model`]. Every part is first built in its own space (a
//! [`Body`]): a shape's faces, or, for an assembly, the parts inside it.
//! Then it is placed in the list it belongs to, by `move` and `rotate` or
//! by joining one of its sockets to a socket of a part that is already
//! placed. The tree of placed parts is kept to the end, when it is
//! flattened into the model's faces.
//!
//! Colour is decided here too. Every face gets one palette colour, from
//! the nearest thing that names one: a face name in `colors` first, then
//! the part's own `color`, then the `color` of each part it is inside,
//! outwards (see [`Layer`]).
//!
//! Mistakes that need the built shapes are found here: a part, a socket,
//! a colour or a face that doesn't exist, a part that uses itself. As everywhere, all of
//! them are reported, and a part that couldn't be built is replaced by an
//! empty placeholder so that nothing is complained about twice.

use std::collections::{BTreeMap, BTreeSet};

use crate::issue::{Code, Issue};
use crate::load::Loaded;
use crate::math::{Bounds, Mat3, Transform, Vec3, vec3};
use crate::model::{Face, Model, Part, PartKind, Socket};
use crate::recipe::{
    Axis, Define, Loc, PaletteColor, PartDef, Placement, Recipe, Side, SocketDef, SocketKind,
    SocketRef, Source, rgb_of,
};
use crate::shapes::{self, Polygon};

/// The most triangles a model may have. Far beyond "game-ready"; it
/// stops a recipe that nests uses of uses from filling the memory.
pub const MAX_TRIANGLES: usize = 1_000_000;

/// How deep parts may be nested inside parts.
const MAX_DEPTH: usize = 32;

/// Builds the model. `None` if the recipe has any error, already found
/// or found here; the errors are in `issues`.
pub fn assemble(loaded: &Loaded, issues: &mut Vec<Issue>) -> Option<Model> {
    let recipe = &loaded.recipe;
    let mut assembler = Assembler {
        issues,
        palette: &loaded.palette,
        referenced: vec![false; loaded.palette.len()],
        paint: Vec::new(),
        entry: Vec::new(),
        using: Vec::new(),
        used: BTreeSet::new(),
        depth: 0,
        triangles: 0,
        over_budget: false,
    };
    let body = assembler.recipe_body(recipe);

    // Parts that are defined but never used: a warning, and they are
    // still built once, so that their own mistakes are found too.
    let scope = Scope::of(recipe);
    let unused: Vec<&Define> = recipe
        .define
        .iter()
        .filter(|define| !assembler.used.contains(&scope.key(&define.name)))
        .collect();
    for define in unused {
        assembler.issues.push(define.loc.warning(
            Code::Unused,
            format!("the part \"{}\" is defined but never used", define.name),
        ));
        assembler.entry.push(define.loc.clone());
        assembler.body(&define.part, &scope);
        assembler.entry.pop();
    }

    // Colours of the recipe's own palette that nothing names.
    for (color, referenced) in loaded.palette.iter().zip(&assembler.referenced) {
        if color.own && !referenced {
            assembler.issues.push(color.loc.warning(
                Code::Unused,
                format!("the colour \"{}\" is never used", color.name),
            ));
        }
    }

    if body.broken || assembler.issues.iter().any(Issue::is_error) {
        return None;
    }
    let mut faces = Vec::new();
    let parts = emit(&body.nodes, &Transform::IDENTITY, "", &mut faces);
    Some(Model {
        name: recipe.name.clone(),
        palette: loaded.palette.clone(),
        faces,
        parts,
        sockets: body.all_sockets(&Transform::IDENTITY),
    })
}

/// Which `define`s a `use` can name: those of the file it is written in.
struct Scope<'r> {
    file: &'r str,
    defines: &'r [Define],
}

impl<'r> Scope<'r> {
    fn of(recipe: &'r Recipe) -> Self {
        Scope {
            file: &recipe.file,
            defines: &recipe.define,
        }
    }

    /// A defined part's name together with its file, so that two files
    /// can each define an `arm`.
    fn key(&self, name: &str) -> (String, String) {
        (self.file.to_owned(), name.to_owned())
    }
}

/// A part's shape in its own space, before it is put anywhere.
struct Body {
    kind: PartKind,
    /// Its own faces, if it is a shape or a mesh.
    faces: Vec<Polygon>,
    /// The palette colour of each of `faces`, in the same order. `None`
    /// for a face nobody coloured.
    colors: Vec<Option<usize>>,
    /// The parts inside it, each already placed, if it is an assembly.
    nodes: Vec<Node>,
    /// The sockets it declares. The seven built-in ones are worked out
    /// from `bounds` when asked for.
    sockets: Vec<Socket>,
    /// The box around everything in it.
    bounds: Option<Bounds>,
    /// True if it couldn't be built. The reason is already reported, so
    /// nothing that touches it is reported again.
    broken: bool,
}

/// A part placed in the list it belongs to.
struct Node {
    name: String,
    /// Where the recipe wrote the part.
    loc: Loc,
    attached_to: Option<String>,
    /// From the part's own space to the space of its list.
    place: Transform,
    body: Body,
}

impl Body {
    fn broken() -> Body {
        Body {
            kind: PartKind {
                shape: "nothing".to_owned(),
                uses: None,
                file: None,
            },
            faces: Vec::new(),
            colors: Vec::new(),
            nodes: Vec::new(),
            sockets: Vec::new(),
            bounds: None,
            broken: true,
        }
    }

    /// A shape or a mesh: faces of its own.
    fn solid(
        shape: &str,
        file: Option<String>,
        faces: Vec<Polygon>,
        colors: Vec<Option<usize>>,
    ) -> Body {
        let mut body = Body {
            kind: PartKind {
                shape: shape.to_owned(),
                uses: None,
                file,
            },
            faces,
            colors,
            nodes: Vec::new(),
            sockets: Vec::new(),
            bounds: None,
            broken: false,
        };
        body.measure();
        body
    }

    /// An assembly: parts, already placed.
    fn assembly(nodes: Vec<Node>, file: Option<String>) -> Body {
        // An assembly none of whose parts could be built is itself not
        // built.
        let broken = nodes.iter().all(|node| node.body.broken);
        let mut body = Body {
            kind: PartKind {
                shape: "assembly".to_owned(),
                uses: None,
                file,
            },
            faces: Vec::new(),
            colors: Vec::new(),
            nodes,
            sockets: Vec::new(),
            bounds: None,
            broken,
        };
        body.measure();
        body
    }

    /// Calls `visit` with every corner in the body, moved by `place`.
    fn corners(&self, place: &Transform, visit: &mut dyn FnMut(Vec3)) {
        for polygon in &self.faces {
            for &corner in &polygon.corners {
                visit(place.apply(corner));
            }
        }
        for node in &self.nodes {
            node.body.corners(&node.place.then(place), visit);
        }
    }

    /// Works out the box around the body.
    fn measure(&mut self) {
        let mut points = Vec::new();
        self.corners(&Transform::IDENTITY, &mut |corner| points.push(corner));
        self.bounds = Bounds::around(points);
    }

    /// One of the body's sockets, in its own space: a built-in one, on
    /// the box around it, or one it declares.
    fn socket(&self, name: &str) -> Option<Socket> {
        if let Some(side) = Side::from_name(name) {
            let bounds = self.bounds?;
            return Some(Socket {
                name: name.to_owned(),
                at: bounds.center() + Vec3::from(side.offset()).times(bounds.half()),
                facing: side.facing().into(),
            });
        }
        self.sockets
            .iter()
            .find(|socket| socket.name == name)
            .cloned()
    }

    /// The names of all its sockets, declared ones first, for messages.
    fn socket_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .sockets
            .iter()
            .map(|socket| socket.name.clone())
            .collect();
        names.extend(Side::ALL.map(|side| side.name().to_owned()));
        names
    }

    /// All its sockets, built-in ones first, moved by `place`.
    fn all_sockets(&self, place: &Transform) -> Vec<Socket> {
        Side::ALL
            .into_iter()
            .filter_map(|side| self.socket(side.name()))
            .chain(self.sockets.iter().cloned())
            .map(|socket| Socket {
                name: socket.name,
                at: place.apply(socket.at).tidied(),
                facing: place.facing(socket.facing).tidied(),
            })
            .collect()
    }

    /// Moves everything in the body, without turning it.
    fn shift(&mut self, by: Vec3) {
        for polygon in &mut self.faces {
            for corner in &mut polygon.corners {
                *corner = *corner + by;
            }
        }
        for node in &mut self.nodes {
            node.place.shift = node.place.shift + by;
        }
        for socket in &mut self.sockets {
            socket.at = socket.at + by;
        }
        if let Some(bounds) = &mut self.bounds {
            bounds.min = bounds.min + by;
            bounds.max = bounds.max + by;
        }
    }
}

impl Node {
    /// One of the part's sockets, where it is in the part's list.
    fn socket(&self, name: &str) -> Option<Socket> {
        placed_socket(&self.body, &self.place, name)
    }
}

/// A body's socket after the body has been placed.
fn placed_socket(body: &Body, place: &Transform, name: &str) -> Option<Socket> {
    let socket = body.socket(name)?;
    Some(Socket {
        name: socket.name,
        at: place.apply(socket.at),
        facing: place.facing(socket.facing),
    })
}

/// The flip and stretch a part asks for with `mirror` and `scale`. Done
/// to the part before it is turned or moved.
fn mirror_and_scale(part: &PartDef) -> Mat3 {
    let flip = match part.mirror {
        None => vec3(1.0, 1.0, 1.0),
        Some(Axis::X) => vec3(-1.0, 1.0, 1.0),
        Some(Axis::Y) => vec3(1.0, -1.0, 1.0),
        Some(Axis::Z) => vec3(1.0, 1.0, -1.0),
    };
    Mat3::stretch(flip.times(part.scale.into()))
}

/// What trying to join a part gave.
enum Join {
    /// Where it goes.
    Place(Transform),
    /// The part it hangs from isn't placed yet: try again later.
    Wait,
}

/// What one part says about colour, kept while the things inside it are
/// built.
///
/// The layers of all the parts being built form a stack, outermost
/// first. A face looks through it from the nearest layer outwards: first
/// for its own name in `faces`, as far as the nearest assembly (face
/// names don't reach through an assembly: its inner parts have faces of
/// their own); then for a whole-part `color`, all the way out. So the
/// nearest setting wins, and a face name beats a whole-part colour.
struct Layer {
    /// The part's `color`.
    whole: Whole,
    /// Where `color` was written.
    whole_loc: Option<Loc>,
    /// True once a face took its colour from `whole`.
    painted: bool,
    /// The part's `colors`: face name, palette colour (`None` if the
    /// name isn't in the palette), and where it was written.
    faces: Vec<(String, Option<usize>, Loc)>,
    /// True once a shape or mesh looked at `faces`.
    reached: bool,
    /// True for an assembly.
    wall: bool,
}

/// A part's `color`.
#[derive(Clone, Copy)]
enum Whole {
    /// Not written.
    None,
    /// Written, but not a palette name. That is reported; the faces it
    /// would have painted are not also reported as having no colour.
    Unknown,
    /// This palette colour.
    Is(usize),
}

struct Assembler<'a> {
    issues: &'a mut Vec<Issue>,
    /// The model's palette.
    palette: &'a [PaletteColor],
    /// For each palette colour, whether anything names it.
    referenced: Vec<bool>,
    /// The colour layers of the parts being built, outermost first.
    paint: Vec<Layer>,
    /// The list entries being built, outermost first: where to point
    /// when a part has no colour.
    entry: Vec<Loc>,
    /// The defined parts being built right now, outermost first, to
    /// catch a part that uses itself.
    using: Vec<(String, String)>,
    /// Every defined part that was used.
    used: BTreeSet<(String, String)>,
    /// How deep inside other parts the part being built is.
    depth: usize,
    /// How many triangles have been built.
    triangles: usize,
    /// True once there are too many triangles: nothing more is built.
    over_budget: bool,
}

impl Assembler<'_> {
    /// Builds a whole recipe (the one on the command line, or one used
    /// as a part) in its own space, zero point where `origin` says.
    fn recipe_body(&mut self, recipe: &Recipe) -> Body {
        let scope = Scope::of(recipe);
        let nodes = self.list(&recipe.parts, &scope);
        let mut body = Body::assembly(nodes, None);
        self.declare_sockets(&mut body, &recipe.sockets);
        if body.broken {
            return body;
        }
        let origin = match &recipe.origin {
            // The default: the middle of the underside, so the model
            // stands on the ground.
            None => body.socket("bottom"),
            Some(origin) => match &origin.socket {
                SocketRef::Own(name) => {
                    let socket = body.socket(name);
                    if socket.is_none() {
                        let issue = origin
                            .loc
                            .error(
                                Code::UnknownSocket,
                                format!("the model has no socket \"{name}\""),
                            )
                            .got(name.as_str())
                            .one_of(name, body.socket_names());
                        self.issues.push(issue);
                    }
                    socket
                }
                SocketRef::Part(part, socket) => {
                    self.socket_of(&body.nodes, part, socket, &origin.loc)
                }
            },
        };
        if let Some(origin) = origin {
            body.shift(-origin.at);
        }
        body
    }

    /// Builds one part in its own space: its shape, then the sockets it
    /// declares.
    fn body(&mut self, part: &PartDef, scope: &Scope<'_>) -> Body {
        self.depth += 1;
        let mut body = if self.over_budget {
            Body::broken()
        } else if self.depth > MAX_DEPTH {
            self.issues.push(part.loc.error(
                Code::OutOfRange,
                format!("parts are nested more than {MAX_DEPTH} deep here"),
            ));
            Body::broken()
        } else {
            let layer = self.layer(part);
            self.paint.push(layer);
            let body = self.source(part, scope);
            let layer = self.paint.pop().expect("pushed above");
            if !body.broken {
                self.check_layer(layer);
            }
            body
        };
        self.depth -= 1;
        self.declare_sockets(&mut body, &part.sockets);
        body
    }

    /// Reads what a part says about colour.
    fn layer(&mut self, part: &PartDef) -> Layer {
        let whole = match &part.color {
            None => Whole::None,
            Some(named) => match self.color(&named.text, &named.loc) {
                Some(color) => Whole::Is(color),
                None => Whole::Unknown,
            },
        };
        let faces = part
            .colors
            .iter()
            .map(|face| {
                (
                    face.face.clone(),
                    self.color(&face.color, &face.loc),
                    face.loc.clone(),
                )
            })
            .collect();
        Layer {
            whole,
            whole_loc: part.color.as_ref().map(|named| named.loc.clone()),
            painted: false,
            faces,
            reached: false,
            wall: matches!(part.source, Source::Parts(_) | Source::Recipe(_)),
        }
    }

    /// Once a part is built: was what it said about colour any use?
    fn check_layer(&mut self, layer: Layer) {
        if let (Whole::Is(_), false, Some(loc)) = (layer.whole, layer.painted, &layer.whole_loc) {
            self.issues.push(loc.warning(
                Code::Unused,
                "this colour changes nothing: every face in this part already has a colour of its own",
            ));
        }
        if !layer.reached
            && let Some((_, _, loc)) = layer.faces.first()
        {
            self.issues.push(loc.error(
                Code::UnknownFace,
                "this part is made of other parts and has no faces of its own: put \"colors\" on the parts inside it (a plain \"color\" works here)",
            ));
        }
    }

    /// The palette colour with this name. Reports a name that isn't in
    /// the palette.
    fn color(&mut self, name: &str, loc: &Loc) -> Option<usize> {
        if let Some(at) = self.palette.iter().position(|color| color.name == name) {
            self.referenced[at] = true;
            return Some(at);
        }
        let issue = if name.starts_with('#') {
            // A colour code where a name belongs.
            let known =
                rgb_of(name).and_then(|rgb| self.palette.iter().find(|color| color.rgb == rgb));
            let message = match known {
                Some(color) => format!(
                    "\"{name}\" is not a palette name, but the palette has that colour as \"{}\"",
                    color.name
                ),
                None => format!(
                    "\"{name}\" is not a palette name. Add it to [palette] under a name (for example accent = \"{name}\") and write that name here"
                ),
            };
            let issue = loc.error(Code::UnknownColor, message).got(name);
            match known {
                Some(color) => issue.suggest(color.name.clone()),
                None => issue,
            }
        } else if self.palette.is_empty() {
            loc.error(
                Code::UnknownColor,
                format!(
                    "there is no colour \"{name}\": this recipe has no palette. Add a [palette] section with {name} = \"#RRGGBB\""
                ),
            )
            .got(name)
        } else {
            loc.error(
                Code::UnknownColor,
                format!("there is no colour \"{name}\" in the palette"),
            )
            .got(name)
            .one_of(name, self.palette.iter().map(|color| color.name.clone()))
        };
        self.issues.push(issue);
        None
    }

    /// Chooses a colour for every face of a shape or mesh that has just
    /// been built. `groups_of` says which groups of faces a face name
    /// stands for; `words` lists the face names there are, for messages.
    fn paint_faces(
        &mut self,
        faces: &[Polygon],
        groups_of: &dyn Fn(&str) -> Option<Vec<usize>>,
        words: &[String],
    ) -> Vec<Option<usize>> {
        // A colour for a group of faces. `Some(None)` is a colour that
        // was written but isn't in the palette.
        let mut by_group: BTreeMap<usize, Option<usize>> = BTreeMap::new();
        let mut problems = Vec::new();
        // Face names, from the nearest layer out to the nearest assembly.
        for layer in self.paint.iter_mut().rev() {
            if layer.wall {
                break;
            }
            layer.reached = true;
            for (face, color, loc) in &layer.faces {
                match groups_of(face) {
                    Some(groups) => {
                        for group in groups {
                            by_group.entry(group).or_insert(*color);
                        }
                    }
                    None => problems.push(
                        loc.error(
                            Code::UnknownFace,
                            format!("this part has no face called \"{face}\""),
                        )
                        .got(face.as_str())
                        .one_of(face, words.iter().cloned()),
                    ),
                }
            }
        }
        self.issues.append(&mut problems);

        // A whole-part colour for the faces that are left, from the
        // nearest layer that has one.
        let left = faces.iter().any(|face| !by_group.contains_key(&face.group));
        let mut whole = Whole::None;
        if left {
            for layer in self.paint.iter_mut().rev() {
                match layer.whole {
                    Whole::None => continue,
                    Whole::Unknown => whole = Whole::Unknown,
                    Whole::Is(color) => {
                        layer.painted = true;
                        whole = Whole::Is(color);
                    }
                }
                break;
            }
            if matches!(whole, Whole::None)
                && let Some(entry) = self.entry.last()
            {
                self.issues.push(entry.warning(
                    Code::NoColor,
                    "faces of this part have no colour and are built in plain grey: give the part a \"color\" from the palette",
                ));
            }
        }
        faces
            .iter()
            .map(|face| match by_group.get(&face.group) {
                Some(color) => *color,
                None => match whole {
                    Whole::Is(color) => Some(color),
                    Whole::None | Whole::Unknown => None,
                },
            })
            .collect()
    }

    fn source(&mut self, part: &PartDef, scope: &Scope<'_>) -> Body {
        match &part.source {
            Source::Shape(shape) => {
                let faces = shapes::build(shape);
                if !self.count(&faces, &part.loc) {
                    return Body::broken();
                }
                let words: Vec<String> = shapes::face_words(shape)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                let colors =
                    self.paint_faces(&faces, &|name| shapes::face_groups(shape, name), &words);
                Body::solid(shape.name(), None, faces, colors)
            }
            Source::Use(named) => {
                let Some(define) = scope
                    .defines
                    .iter()
                    .find(|define| define.name == named.text)
                else {
                    let issue = named
                        .loc
                        .error(
                            Code::UnknownPart,
                            format!("there is no defined part \"{}\" to use", named.text),
                        )
                        .got(named.text.as_str())
                        .one_of(
                            &named.text,
                            scope.defines.iter().map(|define| define.name.clone()),
                        );
                    self.issues.push(issue);
                    return Body::broken();
                };
                let key = scope.key(&named.text);
                if self.using.contains(&key) {
                    let mut chain: Vec<&str> = self
                        .using
                        .iter()
                        .skip_while(|open| **open != key)
                        .map(|(_, name)| name.as_str())
                        .collect();
                    chain.push(&named.text);
                    self.issues.push(named.loc.error(
                        Code::Cycle,
                        format!(
                            "the part \"{}\" ends up using itself ({})",
                            named.text,
                            chain.join(" > ")
                        ),
                    ));
                    return Body::broken();
                }
                self.used.insert(key.clone());
                self.using.push(key);
                let mut body = self.body(&define.part, scope);
                self.using.pop();
                body.kind.uses = Some(named.text.clone());
                body
            }
            Source::Parts(parts) => Body::assembly(self.list(parts, scope), None),
            Source::Recipe(recipe) => {
                let mut body = self.recipe_body(recipe);
                body.kind.file = Some(recipe.file.clone());
                body
            }
            Source::Mesh(mesh) => {
                let faces: Vec<Polygon> = mesh
                    .triangles
                    .iter()
                    .zip(&mesh.material_of)
                    .map(|(corners, &material)| Polygon {
                        corners: corners.map(Vec3::from).to_vec(),
                        group: material,
                    })
                    .collect();
                if !self.count(&faces, &part.loc) {
                    return Body::broken();
                }
                // A mesh's face names are its material names.
                let material = |name: &str| {
                    mesh.materials
                        .iter()
                        .position(|material| material == name)
                        .map(|at| vec![at])
                };
                let colors = self.paint_faces(&faces, &material, &mesh.materials);
                Body::solid("mesh", Some(mesh.file.clone()), faces, colors)
            }
            // A file that couldn't be read, or a part that couldn't be:
            // already reported.
            Source::File(_) | Source::Broken => Body::broken(),
        }
    }

    /// Adds to the count of triangles. False, with one error, once there
    /// are too many.
    fn count(&mut self, faces: &[Polygon], loc: &Loc) -> bool {
        self.triangles += faces.iter().map(Polygon::triangle_count).sum::<usize>();
        if self.triangles > MAX_TRIANGLES {
            self.over_budget = true;
            self.issues.push(loc.error(
                Code::OutOfRange,
                format!("the model has more than {MAX_TRIANGLES} triangles: it has to be simpler"),
            ));
            return false;
        }
        true
    }

    /// Works out where each declared socket is in the body's own space.
    fn declare_sockets(&mut self, body: &mut Body, sockets: &[SocketDef]) {
        for def in sockets {
            if body.sockets.iter().any(|socket| socket.name == def.name) {
                self.issues.push(def.loc.error(
                    Code::DuplicateName,
                    format!(
                        "the part being used already has a socket called \"{}\": give this one another name",
                        def.name
                    ),
                ));
                continue;
            }
            if body.broken {
                continue;
            }
            let placed = match &def.kind {
                SocketKind::On {
                    side,
                    shift,
                    facing,
                } => body.bounds.map(|bounds| {
                    let slide = Vec3::from(side.offset()) + Vec3::from(*shift);
                    let facing = facing.unwrap_or(side.facing());
                    (
                        bounds.center() + slide.times(bounds.half()),
                        Vec3::from(facing),
                    )
                }),
                SocketKind::At { at, facing } => Some((Vec3::from(*at), Vec3::from(*facing))),
                SocketKind::From { part, socket } => {
                    if body.nodes.is_empty() {
                        self.issues.push(def.loc.error(
                            Code::UnknownPart,
                            format!(
                                "\"from\" passes on a socket of a part inside this one, but this part is a single {} with no parts inside. Use \"on\" or \"at\"",
                                body.kind.shape
                            ),
                        ));
                        None
                    } else {
                        self.socket_of(&body.nodes, part, socket, &def.loc)
                            .map(|socket| (socket.at, socket.facing))
                    }
                }
            };
            if let Some((at, facing)) = placed
                && let Some(facing) = facing.normalized()
            {
                body.sockets.push(Socket {
                    name: def.name.clone(),
                    at,
                    facing,
                });
            }
        }
    }

    /// The socket `socket` of the part called `part` in `nodes`, where it
    /// is in that list. Reports a part or socket that doesn't exist.
    fn socket_of(&mut self, nodes: &[Node], part: &str, socket: &str, loc: &Loc) -> Option<Socket> {
        let Some(node) = nodes.iter().find(|node| node.name == part) else {
            let issue = loc
                .error(
                    Code::UnknownPart,
                    format!("there is no part \"{part}\" here"),
                )
                .got(part)
                .one_of(part, named(nodes.iter().map(|node| node.name.as_str())));
            self.issues.push(issue);
            return None;
        };
        if node.body.broken {
            return None;
        }
        let found = node.socket(socket);
        if found.is_none() {
            let issue = loc
                .error(
                    Code::UnknownSocket,
                    format!("the part \"{part}\" has no socket \"{socket}\""),
                )
                .got(socket)
                .one_of(socket, node.body.socket_names());
            self.issues.push(issue);
        }
        found
    }

    /// Builds every part of a list and puts each in its place.
    fn list(&mut self, parts: &[PartDef], scope: &Scope<'_>) -> Vec<Node> {
        let bodies: Vec<Body> = parts
            .iter()
            .map(|part| {
                self.entry.push(part.loc.clone());
                let body = self.body(part, scope);
                self.entry.pop();
                body
            })
            .collect();
        let names: Vec<String> = parts
            .iter()
            .enumerate()
            .map(|(at, part)| part.name.clone().unwrap_or_else(|| format!("#{at}")))
            .collect();

        // Parts placed by hand can go straight away. A joined part has to
        // wait until the part it hangs from is placed, so go round until
        // nothing more can be placed.
        let mut places: Vec<Option<Transform>> = vec![None; parts.len()];
        loop {
            let mut placed_one = false;
            for at in 0..parts.len() {
                if places[at].is_some() {
                    continue;
                }
                let place = match &parts[at].placement {
                    Placement::Free { move_by, rotate } => Join::Place(Transform {
                        turn: Mat3::from_rotate(*rotate) * mirror_and_scale(&parts[at]),
                        shift: (*move_by).into(),
                    }),
                    Placement::Attached { .. } => self.join(at, parts, &names, &bodies, &places),
                };
                if let Join::Place(place) = place {
                    places[at] = Some(place);
                    placed_one = true;
                }
            }
            if !placed_one {
                break;
            }
        }

        // Whatever is still waiting hangs in a ring: a from b, b from a.
        for at in 0..parts.len() {
            if places[at].is_some() {
                continue;
            }
            if let Placement::Attached { to_loc, .. } = &parts[at].placement {
                let chain = ring(at, parts, &names);
                self.issues.push(to_loc.error(
                    Code::Cycle,
                    format!(
                        "these parts hang from each other in a ring ({}): one of them must be attached to something else, or to nothing",
                        chain.join(" > ")
                    ),
                ));
            }
        }

        bodies
            .into_iter()
            .enumerate()
            .map(|(at, body)| Node {
                name: names[at].clone(),
                loc: parts[at].loc.clone(),
                attached_to: match &parts[at].placement {
                    Placement::Attached {
                        to_part, to_socket, ..
                    } => Some(format!("{to_part}.{to_socket}")),
                    Placement::Free { .. } => None,
                },
                place: places[at].unwrap_or(Transform {
                    turn: mirror_and_scale(&parts[at]),
                    shift: Vec3::ZERO,
                }),
                body,
            })
            .collect()
    }

    /// Works out where a joined part goes: its socket on the other
    /// part's socket, the two facing each other.
    fn join(
        &mut self,
        at: usize,
        parts: &[PartDef],
        names: &[String],
        bodies: &[Body],
        places: &[Option<Transform>],
    ) -> Join {
        let Placement::Attached {
            attach,
            to_part,
            to_socket,
            to_loc,
            rotate,
        } = &parts[at].placement
        else {
            return Join::Wait;
        };
        let before = mirror_and_scale(&parts[at]);
        // Where a part goes when its join can't be worked out. The
        // reason is reported, and no model comes out.
        let nowhere = Join::Place(Transform {
            turn: before,
            shift: Vec3::ZERO,
        });

        let Some(target) = names.iter().position(|name| name == to_part) else {
            let issue = to_loc
                .error(
                    Code::UnknownPart,
                    format!("there is no part \"{to_part}\" in this list"),
                )
                .got(to_part.as_str())
                .one_of(to_part, named(names.iter().map(String::as_str)));
            self.issues.push(issue);
            return nowhere;
        };
        if target == at {
            self.issues.push(to_loc.error(
                Code::Cycle,
                format!("the part \"{to_part}\" can't be attached to itself"),
            ));
            return nowhere;
        }
        let Some(target_place) = &places[target] else {
            return Join::Wait;
        };
        if bodies[target].broken || bodies[at].broken {
            return nowhere;
        }
        let Some(theirs) = placed_socket(&bodies[target], target_place, to_socket) else {
            let issue = to_loc
                .error(
                    Code::UnknownSocket,
                    format!("the part \"{to_part}\" has no socket \"{to_socket}\""),
                )
                .got(to_socket.as_str())
                .one_of(to_socket, bodies[target].socket_names());
            self.issues.push(issue);
            return nowhere;
        };
        let Some(mine) = bodies[at].socket(&attach.text) else {
            let issue = attach
                .loc
                .error(
                    Code::UnknownSocket,
                    format!("this part has no socket \"{}\"", attach.text),
                )
                .got(attach.text.as_str())
                .one_of(&attach.text, bodies[at].socket_names());
            self.issues.push(issue);
            return nowhere;
        };

        // My socket after mirroring and scaling, still in my own space.
        let my_point = before * mine.at;
        let my_facing = before.turn_facing(mine.facing);
        // Turn as little as possible until my socket faces theirs, then
        // twist around the join.
        let face_it = Mat3::turning(my_facing, -theirs.facing);
        let twist = Mat3::around(theirs.facing, *rotate);
        let turn = twist * face_it;
        Join::Place(Transform {
            turn: turn * before,
            shift: theirs.at - turn * my_point,
        })
    }
}

/// The names that a recipe gave, without the `#2` stand-ins of unnamed
/// parts: only named parts can be referred to.
fn named<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    names
        .filter(|name| !name.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// The ring a part hangs in, for a message: `a > b > a`.
fn ring(start: usize, parts: &[PartDef], names: &[String]) -> Vec<String> {
    let mut chain = vec![names[start].clone()];
    let mut at = start;
    // A ring can't be longer than the list.
    for _ in 0..parts.len() {
        let Placement::Attached { to_part, .. } = &parts[at].placement else {
            break;
        };
        chain.push(to_part.clone());
        if *to_part == names[start] {
            break;
        }
        match names.iter().position(|name| name == to_part) {
            Some(next) => at = next,
            None => break,
        }
    }
    chain
}

/// Flattens the tree of placed parts into the model's faces, and returns
/// the tree as the model keeps it.
fn emit(nodes: &[Node], outer: &Transform, prefix: &str, faces: &mut Vec<Face>) -> Vec<Part> {
    nodes
        .iter()
        .map(|node| {
            let place = node.place.then(outer);
            let path = if prefix.is_empty() {
                node.name.clone()
            } else {
                format!("{prefix}/{}", node.name)
            };
            let first = faces.len();
            for (polygon, &color) in node.body.faces.iter().zip(&node.body.colors) {
                let placed = polygon.transformed(&place);
                faces.push(Face {
                    corners: placed.corners.into_iter().map(Vec3::tidied).collect(),
                    color,
                });
            }
            let parts = emit(&node.body.nodes, &place, &path, faces);
            let own = first..faces.len();
            Part {
                name: node.name.clone(),
                path,
                loc: node.loc.clone(),
                kind: node.body.kind.clone(),
                attached_to: node.attached_to.clone(),
                mirrored: place.is_mirrored(),
                place,
                sockets: node.body.all_sockets(&place),
                bounds: Bounds::around(
                    faces[own.clone()]
                        .iter()
                        .flat_map(|face| face.corners.iter().copied()),
                ),
                faces: own,
                parts,
            }
        })
        .collect()
}
