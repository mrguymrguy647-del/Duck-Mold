//! Measuring a model: checks in words and numbers.
//!
//! [`inspect`] looks at a built [`Model`] and reports what an agent needs
//! to know to tell whether it came out right without seeing a picture:
//! its size, how many triangles it has, its parts, which colours it uses,
//! whether it rests on the ground, which parts float (touch nothing) and
//! which parts overlap.
//!
//! Two parts *touch* when their surfaces come within a millimetre
//! ([`TOUCH`]). Starting from the first part, everything that can be
//! reached by touching is the body; any other part floats. One part
//! *overlaps* another when some of it is more than a millimetre inside
//! the other.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::issue::{Code, Issue, in_file_order};
use crate::math::{Bounds, Vec3, tidy, vec3};
use crate::model::{Model, Part};
use crate::recipe::{Limits, Turns, hex};
use crate::shaping::{Shaped, facing_word};

/// How close two surfaces must be to count as touching, and how deep one
/// part must be inside another to count as overlapping: a millimetre.
pub const TOUCH: f64 = 0.001;

/// The most pairs of triangles that are compared exactly for one pair of
/// parts. Beyond it, the two parts are compared by their boxes only.
const PAIR_BUDGET: usize = 50_000_000;

/// About how many point-against-triangle tests are spent on finding how
/// deep one part is inside another. A part with more sample points than
/// this allows has every n-th one tested. Ordinary low-poly parts are far
/// below it and are tested in full.
const SAMPLE_BUDGET: usize = 500_000;

/// The whole report on a model. As JSON it is what `--json` prints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Inspection {
    /// Always true: a model that has errors is never inspected.
    pub ok: bool,
    /// The whole report in one line.
    pub summary: String,
    /// The file the model came from, as it was named.
    pub source: String,
    /// The model's name.
    pub name: String,
    /// Width (x), height (y) and depth (z), in metres.
    pub size: [f64; 3],
    /// The box around the model.
    pub bounds: Corners,
    /// How many triangles.
    pub triangles: usize,
    /// How many corners a `.glb` stores for it.
    pub vertices: usize,
    /// Whether it stands on y = 0.
    pub ground: Ground,
    /// Every palette colour, with how many triangles it paints.
    pub palette: Vec<ColorUse>,
    /// How many triangles nobody coloured (they are plain grey).
    #[serde(skip_serializing_if = "is_zero")]
    pub unpainted_triangles: usize,
    /// For a mesh file: its materials, with how many triangles each has.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<MaterialUse>,
    /// The parts, as a tree.
    pub parts: Vec<PartReport>,
    /// The parts that touch nothing.
    pub floating: Vec<Floating>,
    /// The pairs of parts where one is inside the other.
    pub overlapping: Vec<Overlap>,
    /// Every named socket, where it is: only with `--sockets`
    /// ([`socket_report`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sockets: Vec<SocketReport>,
    /// Every shaped part's faces, by name: only with `--faces`
    /// ([`face_report`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shaped: Vec<ShapedReport>,
    /// The tree the model moves by, every part with what it turns about
    /// and hangs from: only with `--joins` ([`join_report`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<JoinReport>,
    /// The names of the model's poses, in the recipe's order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub poses: Vec<String>,
    /// For a report on the model in one of its poses (`--pose`): which
    /// pose, and what the pose gets wrong that the model at rest
    /// doesn't.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose: Option<PoseReport>,
    /// Things that look like mistakes.
    pub warnings: Vec<Issue>,
    /// Always empty: a model that has errors is never inspected.
    pub errors: Vec<Issue>,
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// A box, by its two opposite corners.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Corners {
    /// The smallest x, y and z.
    pub min: [f64; 3],
    /// The biggest x, y and z.
    pub max: [f64; 3],
}

/// Whether the model stands on the ground.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ground {
    /// True when the lowest point is within a millimetre of y = 0.
    pub rests_on_ground: bool,
    /// The height of the lowest point.
    pub lowest_y: f64,
}

/// One palette colour and how much of the model it paints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColorUse {
    /// The colour's name.
    pub name: String,
    /// The colour as `#RRGGBB`.
    pub hex: String,
    /// How many triangles have it.
    pub triangles: usize,
}

/// One named socket, for `inspect --sockets`: the names a game can ask a
/// `.glb` for.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SocketReport {
    /// `top`, `head.top`, `left_arm/hand.bottom`.
    pub name: String,
    /// Where it is, in metres.
    pub at: [f64; 3],
    /// The way it faces, 1 long.
    pub facing: [f64; 3],
}

/// Every named socket of the model ([`Model::named_sockets`]), as the
/// `.glb` holds them.
pub fn socket_report(model: &Model) -> Vec<SocketReport> {
    model
        .named_sockets()
        .into_iter()
        .map(|socket| SocketReport {
            name: socket.name,
            at: socket.at.to_array(),
            facing: socket.facing.to_array(),
        })
        .collect()
}

/// What `inspect --pose` adds to the report: the report itself is of
/// the model as it stands in the pose.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PoseReport {
    /// The pose's name.
    pub name: String,
    /// The parts the pose turns or moves.
    pub moves: Vec<String>,
    /// What the pose asks of a part that its limits don't allow, each as
    /// a sentence: `turns "head" 95 around y: its limit is -80 to 80`.
    pub limits_broken: Vec<String>,
    /// The pairs of `overlapping` that don't overlap when the model is
    /// at rest: what the pose pushed into each other.
    pub new_overlapping: Vec<[String; 2]>,
    /// The parts of `floating` that don't float when the model is at
    /// rest.
    pub new_floating: Vec<String>,
}

/// The report on a model in one of its poses. `rest` is the report on
/// the model as it is built; `posed` is the model in the pose.
pub fn inspect_pose(
    rest: &Inspection,
    model: &Model,
    posed: &Model,
    pose: &crate::motion::Pose,
    source: &str,
) -> Inspection {
    let mut inspection = inspect(posed, source, Vec::new());
    // A pose may well leave the ground (a jump): the report's `ground`
    // says so, and that is no warning.
    inspection
        .warnings
        .retain(|warning| warning.code != Code::OffGround);
    let was = |a: &str, b: &str| {
        rest.overlapping
            .iter()
            .any(|old| (old.a == a && old.b == b) || (old.a == b && old.b == a))
    };
    inspection.pose = Some(PoseReport {
        name: pose.name.clone(),
        moves: pose
            .joints
            .iter()
            .filter(|joint| !joint.is_rest())
            .map(|joint| joint.part.clone())
            .collect(),
        limits_broken: crate::motion::limits_broken(model, &pose.joints)
            .iter()
            .map(crate::motion::Broken::describe)
            .collect(),
        new_overlapping: inspection
            .overlapping
            .iter()
            .filter(|overlap| !was(&overlap.a, &overlap.b))
            .map(|overlap| [overlap.a.clone(), overlap.b.clone()])
            .collect(),
        new_floating: inspection
            .floating
            .iter()
            .filter(|float| !rest.floating.iter().any(|old| old.part == float.part))
            .map(|float| float.part.clone())
            .collect(),
    });
    inspection.summary = format!("in the pose \"{}\", {}", pose.name, inspection.summary);
    inspection
}

/// One part in the tree the model moves by, for `inspect --joins`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JoinReport {
    /// The part, by path: `left_arm/elbow`.
    pub part: String,
    /// The part it hangs from: turning that one carries this one along.
    /// `None`: it hangs from the model itself.
    pub parent: Option<String>,
    /// The socket it is joined to, as the recipe wrote it. `None`: it
    /// isn't joined, and turns about its own middle.
    pub joined_to: Option<String>,
    /// The point it turns about, in the model.
    pub pivot: [f64; 3],
    /// How far it may turn: `"free"`, `"fixed"`, or the directions it
    /// turns around, each with two angles or `"free"`.
    pub limits: serde_json::Value,
    /// The same in a few words: `x -30..40, y -80..80`.
    #[serde(skip)]
    pub limits_words: String,
    /// How deep it is in the tree: 0 hangs from the model.
    #[serde(skip)]
    pub depth: usize,
}

/// `limits` as data: `"free"`, `"fixed"`, or `{ "x": [-30, 40], "z":
/// "free" }` with the directions that turn.
pub fn limits_json(limits: Limits) -> serde_json::Value {
    match limits {
        Limits::Free => "free".into(),
        Limits::Fixed => "fixed".into(),
        Limits::Turns(turns) => serde_json::Value::Object(
            Limits::AXES
                .iter()
                .zip(turns)
                .filter_map(|(axis, turns)| {
                    let value = match turns {
                        Turns::No => return None,
                        Turns::Any => "free".into(),
                        Turns::Between(low, high) => serde_json::json!([low, high]),
                    };
                    Some(((*axis).to_owned(), value))
                })
                .collect(),
        ),
    }
}

/// The tree a model moves by: every part, under the part it hangs from,
/// in the recipe's order.
pub fn join_report(model: &Model) -> Vec<JoinReport> {
    let all = model.all_parts();
    fn under(all: &[&Part], parent: Option<&str>, depth: usize, into: &mut Vec<JoinReport>) {
        for part in all.iter().filter(|part| part.parent.as_deref() == parent) {
            into.push(JoinReport {
                part: part.path.clone(),
                parent: part.parent.clone(),
                joined_to: part.attached_to.clone(),
                pivot: part.pivot.to_array(),
                limits: limits_json(part.limits),
                limits_words: part.limits.describe(),
                depth,
            });
            // (A tree can't go round: a ring of joins is a mistake, and a
            // model with mistakes is never inspected.)
            under(all, Some(&part.path), depth + 1, into);
        }
    }
    let mut joins = Vec::new();
    under(&all, None, 0, &mut joins);
    joins
}

/// The faces of one shaped part, for `inspect --faces`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShapedReport {
    /// The part: `chair`, `kitchen/chair`.
    pub part: String,
    /// The file its changes are written in.
    pub file: String,
    /// How many changes were made to it.
    pub changes: usize,
    /// How many faces it has.
    pub face_count: usize,
    /// Each of its faces' own names, once.
    pub faces: Vec<FaceReport>,
    /// The names that stand for several faces: `sides`, a pull's name,
    /// the old name of an inset face.
    pub names: Vec<NameReport>,
}

/// One face name of a shaped part.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FaceReport {
    /// The name.
    pub name: String,
    /// How many faces have it as their own name: usually 1.
    pub count: usize,
    /// The way it faces: `up`, `down`, `left`, `right`, `front`, `back`
    /// (the nearest of them).
    pub facing: String,
    /// For a single face: how wide and tall it is, in metres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 2]>,
    /// The faces next to it: the ones it shares an edge with.
    pub next_to: Vec<String>,
}

/// A name that stands for several faces.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NameReport {
    /// The name.
    pub name: String,
    /// The faces' own names.
    pub faces: Vec<String>,
}

/// The faces of every shaped part of a model, by name.
pub fn face_report(model: &Model) -> Vec<ShapedReport> {
    fn gather(parts: &[Part], into: &mut Vec<ShapedReport>) {
        for part in parts {
            if let Some(shaped) = &part.shaped {
                into.push(shaped_report(part, shaped));
            }
            gather(&part.parts, into);
        }
    }
    let mut reports = Vec::new();
    gather(&model.parts, &mut reports);
    reports
}

fn shaped_report(part: &Part, shaped: &Shaped) -> ShapedReport {
    let own = shaped.own_names();
    let faces = own
        .iter()
        .map(|name| {
            let these: Vec<usize> = (0..shaped.faces.len())
                .filter(|&face| shaped.faces[face].name == *name)
                .collect();
            let facing = these.iter().fold(Vec3::ZERO, |sum, &face| {
                sum + shaped.normal(face).unwrap_or(Vec3::ZERO) * shaped.area(face)
            });
            let mut next_to: Vec<String> = Vec::new();
            for &face in &these {
                for other in shaped.neighbours(face) {
                    let other = &shaped.faces[other].name;
                    if other != name && !next_to.contains(other) {
                        next_to.push(other.clone());
                    }
                }
            }
            FaceReport {
                name: name.clone(),
                count: these.len(),
                facing: facing_word(facing).to_owned(),
                size: match these.as_slice() {
                    [face] => Some(
                        shaped
                            .extent(*face)
                            .map(|n| (n * 10_000.0).round() / 10_000.0 + 0.0),
                    ),
                    _ => None,
                },
                next_to,
            }
        })
        .collect();
    let names = shaped
        .names()
        .into_iter()
        .filter(|name| !own.contains(name))
        .map(|name| {
            let mut faces: Vec<String> = Vec::new();
            for face in &shaped.faces {
                if face.answers_to(&name) && !faces.contains(&face.name) {
                    faces.push(face.name.clone());
                }
            }
            NameReport { name, faces }
        })
        .collect();
    ShapedReport {
        part: part.path.clone(),
        file: part.loc.file.clone(),
        changes: part.kind.changes,
        face_count: shaped.faces.len(),
        faces,
        names,
    }
}

/// One material of a mesh file and how many triangles have it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterialUse {
    /// The material's name: a face name for the recipe's `colors`.
    pub name: String,
    /// How many triangles have it.
    pub triangles: usize,
}

/// One part in the report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PartReport {
    /// Its name from the top of the model: `left_arm/hand`.
    pub name: String,
    /// What it is made of: `box`, `cylinder`, `sphere`, `cone`, `wedge`,
    /// `assembly` or `mesh`.
    pub kind: String,
    /// The defined part it uses, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses: Option<String>,
    /// The file it comes from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// How many changes were made to its shape: pulls, insets...
    #[serde(skip_serializing_if = "is_zero")]
    pub changes: usize,
    /// How many triangles, the parts inside it included.
    pub triangles: usize,
    /// Width, height and depth of the box around it.
    pub size: [f64; 3],
    /// The box around it.
    pub bounds: Corners,
    /// The names of the colours on it.
    pub colors: Vec<String>,
    /// The socket it is joined to, as the recipe wrote it.
    pub attached_to: Option<String>,
    /// True if it was flipped by `mirror`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mirrored: bool,
    /// How far it may turn when the model is posed, if the recipe gives
    /// it `limits`: `"fixed"`, or the directions it turns around.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<serde_json::Value>,
    /// The same in a few words.
    #[serde(skip)]
    pub limits_words: String,
    /// The parts inside it.
    pub parts: Vec<PartReport>,
}

/// A part that touches nothing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Floating {
    /// The part.
    pub part: String,
    /// The nearest part it doesn't touch.
    pub nearest: String,
    /// How far apart they are.
    pub gap: f64,
}

/// Two parts, one of them inside the other.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Overlap {
    /// The part that pokes in.
    pub a: String,
    /// The part it pokes into.
    pub b: String,
    /// How deep, measured at the corners, edge middles and face middles
    /// of each part: an estimate, not an exact volume.
    pub depth: f64,
    /// True when `a` is entirely inside `b`: nobody will ever see it.
    pub hidden: bool,
}

/// Measures a model.
///
/// `source` is the file it came from, for the report. `warnings` are the
/// ones already found while reading and building it; the report adds its
/// own (floating parts, hidden parts, a model off the ground).
pub fn inspect(model: &Model, source: &str, mut warnings: Vec<Issue>) -> Inspection {
    let bounds = model.bounds().unwrap_or(Bounds {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    });
    let size = bounds.size().tidied();
    let triangles = model.triangle_count();

    // ---- colours
    let mut palette: Vec<ColorUse> = model
        .palette
        .iter()
        .map(|color| ColorUse {
            name: color.name.clone(),
            hex: hex(color.rgb),
            triangles: 0,
        })
        .collect();
    let mut unpainted_triangles = 0;
    for face in &model.faces {
        match face.color {
            Some(color) => palette[color].triangles += face.triangle_count(),
            None => unpainted_triangles += face.triangle_count(),
        }
    }

    // ---- the ground
    let lowest_y = tidy(bounds.min.y);
    let ground = Ground {
        rests_on_ground: lowest_y.abs() <= TOUCH,
        lowest_y,
    };
    if !ground.rests_on_ground {
        warnings.push(
            Issue::warning(
                Code::OffGround,
                "",
                format!("the model {}", ground_words(&ground)),
            )
            .in_file(source),
        );
    }

    // ---- floating and overlapping parts
    let leaves = model.leaves();
    let solids: Vec<Solid> = leaves.iter().map(|leaf| Solid::of(model, leaf)).collect();
    let ground_y = model.bounds().map(|all| all.min.y);
    let (floating, overlapping, by_box) = compare(&solids, ground_y);
    for float in &floating {
        let part = leaves.iter().find(|leaf| leaf.path == float.part);
        warnings.push(warning_at(
            part,
            source,
            Code::Floating,
            format!(
                "the part \"{}\" touches nothing: it floats {} from \"{}\"",
                float.part,
                length(float.gap),
                float.nearest
            ),
        ));
    }
    for overlap in overlapping.iter().filter(|overlap| overlap.hidden) {
        let part = leaves.iter().find(|leaf| leaf.path == overlap.a);
        warnings.push(warning_at(
            part,
            source,
            Code::Hidden,
            format!(
                "the part \"{}\" is hidden inside \"{}\": nobody will see it",
                overlap.a, overlap.b
            ),
        ));
    }
    if by_box {
        warnings.push(
            Issue::warning(
                Code::Skipped,
                "",
                "some parts have so many triangles that they were compared by the boxes around them only: \"floating\" and \"overlapping\" may miss something",
            )
            .in_file(source),
        );
    }

    in_file_order(&mut warnings);

    let used = palette.iter().filter(|color| color.triangles > 0).count();
    let summary = format!(
        "{}, {}, {}, {}, {}, {}",
        headline(&model.name, size.to_array(), triangles),
        count(leaves.len(), "part"),
        count(used, "colour"),
        ground_words(&ground),
        match floating.len() {
            0 => "nothing floating".to_owned(),
            n => format!("{} floating", count(n, "part")),
        },
        match overlapping.len() {
            0 => "nothing overlapping".to_owned(),
            n => count(n, "overlap"),
        },
    );

    Inspection {
        ok: true,
        summary,
        source: source.to_owned(),
        name: model.name.clone(),
        size: size.to_array(),
        bounds: corners(&bounds),
        triangles,
        vertices: model.vertex_count(),
        ground,
        palette,
        unpainted_triangles,
        materials: Vec::new(),
        parts: model
            .parts
            .iter()
            .map(|part| part_report(model, part))
            .collect(),
        floating,
        overlapping,
        sockets: Vec::new(),
        shaped: Vec::new(),
        joins: Vec::new(),
        poses: model.poses.iter().map(|pose| pose.name.clone()).collect(),
        pose: None,
        warnings,
        errors: Vec::new(),
    }
}

/// A warning that points at the recipe line of a part, when the part
/// came from a recipe.
fn warning_at(part: Option<&&Part>, source: &str, code: Code, message: String) -> Issue {
    match part {
        Some(part) if !part.loc.file.is_empty() => part.loc.warning(code, message),
        _ => Issue::warning(code, "", message).in_file(source),
    }
}

fn corners(bounds: &Bounds) -> Corners {
    Corners {
        min: bounds.min.tidied().to_array(),
        max: bounds.max.tidied().to_array(),
    }
}

fn part_report(model: &Model, part: &Part) -> PartReport {
    let faces = &model.faces[part.faces.clone()];
    let bounds = part.bounds.unwrap_or(Bounds {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    });
    let used: BTreeSet<usize> = faces.iter().filter_map(|face| face.color).collect();
    PartReport {
        name: part.path.clone(),
        kind: part.kind.shape.clone(),
        uses: part.kind.uses.clone(),
        file: part.kind.file.clone(),
        changes: part.kind.changes,
        triangles: faces.iter().map(|face| face.triangle_count()).sum(),
        size: bounds.size().tidied().to_array(),
        bounds: corners(&bounds),
        colors: used
            .into_iter()
            .map(|color| model.palette[color].name.clone())
            .collect(),
        attached_to: part.attached_to.clone(),
        mirrored: part.mirrored,
        limits: (part.limits != Limits::Free).then(|| limits_json(part.limits)),
        limits_words: part.limits.describe(),
        parts: part
            .parts
            .iter()
            .map(|inner| part_report(model, inner))
            .collect(),
    }
}

// ---- words --------------------------------------------------------------------

/// A length for people: at least two decimals, at most three: `0.40`,
/// `1.02`, `0.125`.
fn length(metres: f64) -> String {
    let rounded = (metres * 1000.0).round() / 1000.0 + 0.0;
    let text = format!("{rounded:.3}");
    match text.strip_suffix('0') {
        Some(shorter) => shorter.to_owned(),
        None => text,
    }
}

/// Said after a part that floats or overlaps in a pose and doesn't when
/// the model is at rest.
const NOT_AT_REST: &str = "   (not so at rest)";

/// The start of the summary: `robot: 0.9 x 1.53 x 0.36 m, 384 triangles`.
fn headline(name: &str, size: [f64; 3], triangles: usize) -> String {
    let [x, y, z] = size.map(length);
    format!(
        "{name}: {x} x {y} x {z} m, {}",
        count(triangles, "triangle")
    )
}

/// `1 part`, `3 parts`.
fn count(number: usize, word: &str) -> String {
    if number == 1 {
        format!("1 {word}")
    } else {
        format!("{number} {word}s")
    }
}

fn ground_words(ground: &Ground) -> String {
    if ground.rests_on_ground {
        "rests on the ground".to_owned()
    } else if ground.lowest_y > 0.0 {
        format!("floats {} above the ground", length(ground.lowest_y))
    } else {
        format!("sinks {} into the ground", length(-ground.lowest_y))
    }
}

impl Inspection {
    /// The start of [`Inspection::summary`]: the model's name, size and
    /// triangle count, as `robot: 0.9 x 1.53 x 0.36 m, 384 triangles`.
    /// `render` writes it under its pictures.
    pub fn headline(&self) -> String {
        match &self.pose {
            Some(pose) => headline(
                &format!("{}, \"{}\"", self.name, pose.name),
                self.size,
                self.triangles,
            ),
            None => headline(&self.name, self.size, self.triangles),
        }
    }

    /// The report for people, a few aligned lines.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let mut line = |label: &str, text: String| {
            out.push_str(&format!("  {label:<12} {text}\n"));
        };

        let [x, y, z] = self.size.map(length);
        line(
            "size",
            format!("{x} wide x {y} tall x {z} deep, in metres (x, y, z)"),
        );
        line(
            "triangles",
            format!("{}  ({} corners)", self.triangles, self.vertices),
        );
        line("ground", ground_words(&self.ground));

        let mut used: Vec<String> = self
            .palette
            .iter()
            .filter(|color| color.triangles > 0)
            .map(|color| format!("{} {}", color.name, color.triangles))
            .collect();
        let named = used.len();
        if self.unpainted_triangles > 0 {
            used.push(format!("(no colour) {}", self.unpainted_triangles));
        }
        line(
            "colours",
            if used.is_empty() {
                "none".to_owned()
            } else {
                format!("{named}:  {}  (triangles each)", used.join(", "))
            },
        );
        if !self.materials.is_empty() {
            let materials: Vec<String> = self
                .materials
                .iter()
                .map(|material| format!("{} {}", material.name, material.triangles))
                .collect();
            line(
                "materials",
                format!(
                    "{}:  {}  (triangles each)",
                    materials.len(),
                    materials.join(", ")
                ),
            );
        }

        // The tree of parts, in columns.
        let mut rows: Vec<[String; 4]> = Vec::new();
        fn gather(
            parts: &[PartReport],
            depth: usize,
            inside_mirrored: bool,
            rows: &mut Vec<[String; 4]>,
        ) {
            for part in parts {
                let own_name = part.name.rsplit('/').next().unwrap_or(&part.name);
                let kind = match part.changes {
                    0 => part.kind.clone(),
                    1 => format!("{}, 1 change", part.kind),
                    changes => format!("{}, {changes} changes", part.kind),
                };
                let label = match (&part.uses, &part.file) {
                    (Some(uses), _) => format!("{uses} ({kind})"),
                    (None, Some(file)) => format!("{file} ({kind})"),
                    (None, None) => kind,
                };
                let mut notes = Vec::new();
                if let Some(socket) = &part.attached_to {
                    notes.push(format!("on {socket}"));
                }
                // Said once, on the part that was flipped, not again on
                // every part inside it.
                if part.mirrored != inside_mirrored {
                    notes.push("mirrored".to_owned());
                }
                if part.limits.is_some() {
                    notes.push(match part.limits_words.as_str() {
                        words @ ("fixed" | "doesn't turn") => words.to_owned(),
                        words => format!("turns {words}"),
                    });
                }
                rows.push([
                    format!("{}{own_name}", "  ".repeat(depth)),
                    label,
                    count(part.triangles, "triangle"),
                    notes.join(", "),
                ]);
                gather(&part.parts, depth + 1, part.mirrored, rows);
            }
        }
        gather(&self.parts, 0, false, &mut rows);
        let width = |column: usize| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
        };
        let (names, labels, triangles) = (width(0), width(1), width(2));
        let leaves = {
            fn leaves(parts: &[PartReport]) -> usize {
                parts
                    .iter()
                    .map(|part| {
                        if part.parts.is_empty() {
                            1
                        } else {
                            leaves(&part.parts)
                        }
                    })
                    .sum()
            }
            leaves(&self.parts)
        };
        line("parts", leaves.to_string());
        for [name, label, count, notes] in &rows {
            let row =
                format!("    {name:<names$}   {label:<labels$}   {count:>triangles$}   {notes}");
            out.push_str(row.trim_end());
            out.push('\n');
        }

        let mut list = |label: &str, items: Vec<String>| {
            if items.is_empty() {
                out.push_str(&format!("  {label:<12} none\n"));
            }
            for (at, item) in items.iter().enumerate() {
                let label = if at == 0 { label } else { "" };
                out.push_str(&format!("  {label:<12} {item}\n"));
            }
        };
        list(
            "floating",
            self.floating
                .iter()
                .map(|float| {
                    let new = self
                        .pose
                        .as_ref()
                        .is_some_and(|pose| pose.new_floating.contains(&float.part));
                    format!(
                        "{} floats {} from {}{}",
                        float.part,
                        length(float.gap),
                        float.nearest,
                        if new { NOT_AT_REST } else { "" }
                    )
                })
                .collect(),
        );
        list(
            "overlapping",
            self.overlapping
                .iter()
                .map(|overlap| {
                    let new = self.pose.as_ref().is_some_and(|pose| {
                        pose.new_overlapping
                            .iter()
                            .any(|[a, b]| *a == overlap.a && *b == overlap.b)
                    });
                    let new = if new { NOT_AT_REST } else { "" };
                    if overlap.hidden {
                        format!("{} is hidden inside {}{new}", overlap.a, overlap.b)
                    } else {
                        format!(
                            "{} is {} inside {}{new}",
                            overlap.a,
                            length(overlap.depth),
                            overlap.b
                        )
                    }
                })
                .collect(),
        );
        if !self.poses.is_empty() {
            list("poses", vec![self.poses.join(", ")]);
        }
        if let Some(pose) = &self.pose {
            list(
                "moves",
                if pose.moves.is_empty() {
                    vec!["nothing: every part is at rest".to_owned()]
                } else {
                    vec![pose.moves.join(", ")]
                },
            );
            list("limits", {
                if pose.limits_broken.is_empty() {
                    vec!["none broken".to_owned()]
                } else {
                    pose.limits_broken
                        .iter()
                        .map(|broken| format!("the pose {broken}"))
                        .collect()
                }
            });
        }
        if !self.sockets.is_empty() {
            let name_width = self
                .sockets
                .iter()
                .map(|socket| socket.name.chars().count())
                .max()
                .unwrap_or(0);
            let rows: Vec<String> = self
                .sockets
                .iter()
                .map(|socket| {
                    let [x, y, z] = socket.at.map(length);
                    format!(
                        "{:<name_width$}   at {x}, {y}, {z}   facing {}",
                        socket.name,
                        direction_words(socket.facing)
                    )
                })
                .collect();
            list("sockets", rows);
        }
        if !self.joins.is_empty() {
            out.push_str(&format!(
                "  {:<12} {}: what each part turns about, and how far\n",
                "joins",
                self.joins.len()
            ));
            let rows: Vec<[String; 3]> = self
                .joins
                .iter()
                .map(|join| {
                    [
                        format!("{}{}", "  ".repeat(join.depth), join.part),
                        match &join.joined_to {
                            Some(socket) => format!("on {socket}"),
                            None => "turns about its middle".to_owned(),
                        },
                        join.limits_words.clone(),
                    ]
                })
                .collect();
            let width = |column: usize| {
                rows.iter()
                    .map(|row| row[column].chars().count())
                    .max()
                    .unwrap_or(0)
            };
            let (names, pivots) = (width(0), width(1));
            for [name, pivot, limits] in &rows {
                out.push_str(&format!(
                    "    {name:<names$}   {pivot:<pivots$}   {limits}\n"
                ));
            }
        }
        for shaped in &self.shaped {
            out.push_str(&format!(
                "  {:<12} {}  ({}): box, {}, {}\n",
                "faces",
                shaped.part,
                shaped.file,
                count(shaped.changes, "change"),
                count(shaped.face_count, "face")
            ));
            let width = shaped
                .faces
                .iter()
                .map(|face| face.name.chars().count())
                .chain(shaped.names.iter().map(|name| name.name.chars().count()))
                .max()
                .unwrap_or(0);
            let sizes: Vec<String> = shaped
                .faces
                .iter()
                .map(|face| match face.size {
                    Some([wide, tall]) => format!("{wide:.2} x {tall:.2} m"),
                    None => count(face.count, "face"),
                })
                .collect();
            let size_width = sizes.iter().map(String::len).max().unwrap_or(0);
            for (face, size) in shaped.faces.iter().zip(&sizes) {
                out.push_str(&format!(
                    "    {:<width$}   faces {:<5}   {size:<size_width$}   next to: {}\n",
                    face.name,
                    face.facing,
                    face.next_to.join(", ")
                ));
            }
            for name in &shaped.names {
                out.push_str(&format!(
                    "    {:<width$}   stands for: {}\n",
                    name.name,
                    name.faces.join(", ")
                ));
            }
        }
        match &self.pose {
            Some(pose) => format!(
                "{}, in the pose \"{}\"  ({})\n{out}",
                self.name, pose.name, self.source
            ),
            None => format!("{}  ({})\n{out}", self.name, self.source),
        }
    }
}

/// `up`, `left`... for a direction straight along one of them, otherwise
/// its three numbers.
fn direction_words(facing: [f64; 3]) -> String {
    const WORDS: [(&str, [f64; 3]); 6] = [
        ("up", [0.0, 1.0, 0.0]),
        ("down", [0.0, -1.0, 0.0]),
        ("left", [-1.0, 0.0, 0.0]),
        ("right", [1.0, 0.0, 0.0]),
        ("front", [0.0, 0.0, 1.0]),
        ("back", [0.0, 0.0, -1.0]),
    ];
    match WORDS.iter().find(|(_, direction)| *direction == facing) {
        Some((word, _)) => (*word).to_owned(),
        None => {
            let [x, y, z] = facing.map(|n| format!("{}", (n * 1000.0).round() / 1000.0 + 0.0));
            format!("[{x}, {y}, {z}]")
        }
    }
}

// ---- geometry -----------------------------------------------------------------

/// One part's shape, prepared for comparing with the others.
struct Solid {
    name: String,
    triangles: Vec<[Vec3; 3]>,
    /// The box around each triangle.
    boxes: Vec<Bounds>,
    bounds: Bounds,
    /// True if the surface has no holes, so "inside" means something.
    closed: bool,
    /// Points to test for being inside another part: corners, the
    /// middles of edges and faces, and the part's own middle.
    samples: Vec<Vec3>,
}

impl Solid {
    fn of(model: &Model, part: &Part) -> Solid {
        let faces = &model.faces[part.faces.clone()];
        let triangles: Vec<[Vec3; 3]> = faces.iter().flat_map(|face| face.triangles()).collect();
        let boxes = triangles
            .iter()
            .map(|corners| Bounds::around(*corners).expect("a triangle has corners"))
            .collect();
        let bounds = part.bounds.unwrap_or(Bounds {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        });

        // Closed: every edge is walked once each way.
        let key = |point: Vec3| [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()];
        let mut edges = BTreeSet::new();
        let mut doubled = false;
        for face in faces {
            for (at, &corner) in face.corners.iter().enumerate() {
                let next = face.corners[(at + 1) % face.corners.len()];
                doubled |= !edges.insert((key(corner), key(next)));
            }
        }
        let closed = !doubled
            && !edges.is_empty()
            && edges.iter().all(|(from, to)| edges.contains(&(*to, *from)));

        let mut seen = BTreeSet::new();
        let mut samples = Vec::new();
        let mut add = |point: Vec3| {
            if seen.insert(key(point)) {
                samples.push(point);
            }
        };
        let mut sum = Vec3::ZERO;
        let mut corner_count = 0.0;
        for face in faces {
            let mut middle = Vec3::ZERO;
            for (at, &corner) in face.corners.iter().enumerate() {
                let next = face.corners[(at + 1) % face.corners.len()];
                add(corner);
                add((corner + next) * 0.5);
                middle = middle + corner;
                sum = sum + corner;
                corner_count += 1.0;
            }
            add(middle * (1.0 / face.corners.len() as f64));
        }
        let mut solid = Solid {
            name: part.path.clone(),
            triangles,
            boxes,
            bounds,
            closed,
            samples,
        };
        // The part's own middle catches a part put exactly on top of a
        // copy of itself, where every other sample is on the surface.
        if corner_count > 0.0 {
            let middle = sum * (1.0 / corner_count);
            if solid.closed && solid.contains(middle) {
                solid.samples.push(middle);
            }
        }
        solid
    }

    /// How far a point is from the surface.
    fn distance_to(&self, point: Vec3) -> f64 {
        self.triangles
            .iter()
            .map(|triangle| (point - closest_on_triangle(point, *triangle)).length())
            .fold(f64::INFINITY, f64::min)
    }

    /// True if the point is inside the surface: a line from it to far
    /// away crosses the surface an odd number of times. Three lines in
    /// three slanted directions vote, in case one of them happens to run
    /// exactly through an edge.
    fn contains(&self, point: Vec3) -> bool {
        const DIRECTIONS: [Vec3; 3] = [
            vec3(0.571_203_664, 0.683_917_180, 0.453_784_911),
            vec3(-0.613_533_988, 0.409_213_562, 0.677_457_216),
            vec3(0.307_129_995, -0.529_407_751, 0.791_884_560),
        ];
        let votes = DIRECTIONS
            .iter()
            .filter(|&&direction| {
                let crossings = self
                    .triangles
                    .iter()
                    .filter(|triangle| {
                        crossing(point, direction, **triangle).is_some_and(|along| along > 1e-12)
                    })
                    .count();
                crossings % 2 == 1
            })
            .count();
        votes >= 2
    }
}

/// How near the ground a part's lowest point must be for it to stand
/// there: a millimetre, as for the model.
const GROUND_NEAR: f64 = 0.001;

/// Compares every pair of parts. Returns the floating parts, the
/// overlapping pairs, and whether any pair was too big to compare
/// exactly. `ground` is the height of the model's lowest point: parts
/// standing there stand on the ground, and don't float.
fn compare(solids: &[Solid], ground: Option<f64>) -> (Vec<Floating>, Vec<Overlap>, bool) {
    let count = solids.len();
    let mut by_box = false;
    let mut overlapping = Vec::new();
    // Which parts are joined to which, by touching or overlapping.
    let mut group: Vec<usize> = (0..count).collect();
    fn root(group: &mut [usize], mut at: usize) -> usize {
        while group[at] != at {
            group[at] = group[group[at]];
            at = group[at];
        }
        at
    }

    for first in 0..count {
        for second in first + 1..count {
            let (a, b) = (&solids[first], &solids[second]);
            if a.bounds.gap_to(&b.bounds) > TOUCH {
                continue;
            }
            let (touching, exact) = match distance(a, b, TOUCH) {
                Some(gap) => (gap <= TOUCH, true),
                // Too many triangles: their boxes are within reach,
                // which has to do.
                None => (true, false),
            };
            by_box |= !exact;

            let (a_in_b, a_hidden) = depth_inside(a, b);
            let (b_in_a, b_hidden) = depth_inside(b, a);
            let depth = a_in_b.max(b_in_a);
            if depth > TOUCH {
                // The one that pokes in deeper is named first; a hidden
                // one always is.
                let a_first = if a_hidden != b_hidden {
                    a_hidden
                } else {
                    a_in_b >= b_in_a
                };
                let (inner, outer, hidden) = if a_first {
                    (a, b, a_hidden)
                } else {
                    (b, a, b_hidden)
                };
                overlapping.push(Overlap {
                    a: inner.name.clone(),
                    b: outer.name.clone(),
                    depth: tidy(depth),
                    hidden,
                });
            }
            if touching || depth > TOUCH {
                let (one, other) = (root(&mut group, first), root(&mut group, second));
                group[one] = other;
            }
        }
    }

    // What stands on the ground, and everything joined to it, stands.
    // So does everything joined to the first part (the body), if nothing
    // stands on the ground. The rest floats.
    let mut floating = Vec::new();
    if count > 0 {
        let on_ground = |solid: &Solid| {
            ground.is_some_and(|ground| (solid.bounds.min.y - ground).abs() <= GROUND_NEAR)
        };
        let mut standing: Vec<usize> = (0..count)
            .filter(|&at| on_ground(&solids[at]))
            .map(|at| root(&mut group, at))
            .collect();
        if standing.is_empty() {
            standing.push(root(&mut group, 0));
        }
        for at in 0..count {
            let own = root(&mut group, at);
            if standing.contains(&own) {
                continue;
            }
            // The nearest part that isn't joined to this one. Parts are
            // tried nearest box first, and the search stops once no box
            // can be nearer than the best found.
            let mut others: Vec<(f64, usize)> = (0..count)
                .filter(|&other| root(&mut group, other) != own)
                .map(|other| (solids[at].bounds.gap_to(&solids[other].bounds), other))
                .collect();
            others.sort_by(|a, b| a.partial_cmp(b).expect("gaps are numbers"));
            let mut best: Option<(f64, usize)> = None;
            for (box_gap, other) in others {
                if best.is_some_and(|(gap, _)| box_gap >= gap) {
                    break;
                }
                let gap = match distance(&solids[at], &solids[other], 0.0) {
                    Some(gap) => gap,
                    None => {
                        by_box = true;
                        box_gap
                    }
                };
                if best.is_none_or(|(least, _)| gap < least) {
                    best = Some((gap, other));
                }
            }
            if let Some((gap, other)) = best {
                floating.push(Floating {
                    part: solids[at].name.clone(),
                    nearest: solids[other].name.clone(),
                    gap: tidy(gap),
                });
            }
        }
    }
    (floating, overlapping, by_box)
}

/// How deep `inner` reaches into `outer`: the furthest that any of its
/// sample points is inside. Also whether all of it is inside.
fn depth_inside(inner: &Solid, outer: &Solid) -> (f64, bool) {
    if !outer.closed {
        return (0.0, false);
    }
    let reach = Bounds {
        min: outer.bounds.min - vec3(TOUCH, TOUCH, TOUCH),
        max: outer.bounds.max + vec3(TOUCH, TOUCH, TOUCH),
    };
    let within = |point: Vec3| {
        point.x >= reach.min.x
            && point.x <= reach.max.x
            && point.y >= reach.min.y
            && point.y <= reach.max.y
            && point.z >= reach.min.z
            && point.z <= reach.max.z
    };
    let near: Vec<Vec3> = inner
        .samples
        .iter()
        .copied()
        .filter(|&point| within(point))
        .collect();
    // Every sample outside the other's box is certainly outside it.
    let mut all_inside = near.len() == inner.samples.len();
    // For parts with very many triangles, look at every n-th sample only.
    let step = (near.len() * outer.triangles.len())
        .div_ceil(SAMPLE_BUDGET)
        .max(1);
    let mut deepest: f64 = 0.0;
    for &point in near.iter().step_by(step) {
        let distance = outer.distance_to(point);
        if distance <= TOUCH {
            // On the surface: neither in nor out.
            continue;
        }
        if outer.contains(point) {
            deepest = deepest.max(distance);
        } else {
            all_inside = false;
        }
    }
    (deepest, all_inside && deepest > TOUCH)
}

/// The smallest distance between two parts' surfaces. Stops early, with
/// whatever it has, once it finds one no bigger than `enough`. `None` if
/// the parts have too many triangles to compare.
fn distance(a: &Solid, b: &Solid, enough: f64) -> Option<f64> {
    // When the question is only "are they within `enough` of each
    // other?", triangles further than that from the other part's box
    // can't matter. When it is "exactly how far apart?", all can.
    let reach = if enough > 0.0 { enough } else { f64::INFINITY };
    let near = |solid: &Solid, other: &Solid| {
        solid
            .boxes
            .iter()
            .filter(|bounds| bounds.gap_to(&other.bounds) <= reach)
            .count()
    };
    if near(a, b).saturating_mul(near(b, a)) > PAIR_BUDGET {
        return None;
    }
    // Triangles in order, skipping any whose box is further away than
    // matters: than `reach`, or than the best distance found so far.
    let mut best = f64::INFINITY;
    for (triangle_a, box_a) in a.triangles.iter().zip(&a.boxes) {
        if box_a.gap_to(&b.bounds) > reach.min(best) {
            continue;
        }
        for (triangle_b, box_b) in b.triangles.iter().zip(&b.boxes) {
            if box_a.gap_to(box_b) > reach.min(best) {
                continue;
            }
            best = best.min(triangle_distance(*triangle_a, *triangle_b));
            if best <= enough {
                return Some(best);
            }
        }
    }
    Some(best)
}

/// The smallest distance between two triangles; 0 if they cross.
fn triangle_distance(a: [Vec3; 3], b: [Vec3; 3]) -> f64 {
    let edges = |t: [Vec3; 3]| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])];
    // An edge of one going through the other: they cross.
    for (triangle, other) in [(a, b), (b, a)] {
        for (from, to) in edges(triangle) {
            if crossing(from, to - from, other).is_some_and(|along| (0.0..=1.0).contains(&along)) {
                return 0.0;
            }
        }
    }
    // Otherwise the nearest points are a corner and a triangle, or two
    // edges.
    let mut best = f64::INFINITY;
    for (triangle, other) in [(a, b), (b, a)] {
        for corner in triangle {
            best = best.min((corner - closest_on_triangle(corner, other)).length());
        }
    }
    for (from_a, to_a) in edges(a) {
        for (from_b, to_b) in edges(b) {
            best = best.min(segment_distance(from_a, to_a, from_b, to_b));
        }
    }
    best
}

/// The point of a triangle nearest to `point`.
fn closest_on_triangle(point: Vec3, [a, b, c]: [Vec3; 3]) -> Vec3 {
    // Which of the triangle's regions the point is nearest: a corner, an
    // edge, or the inside. (Ericson, "Real-Time Collision Detection".)
    let (ab, ac, ap) = (b - a, c - a, point - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = point - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = point - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let total = va + vb + vc;
    if total == 0.0 {
        // A triangle with no area.
        return a;
    }
    a + ab * (vb / total) + ac * (vc / total)
}

/// The smallest distance between two straight pieces of line.
fn segment_distance(from_a: Vec3, to_a: Vec3, from_b: Vec3, to_b: Vec3) -> f64 {
    let (along_a, along_b, between) = (to_a - from_a, to_b - from_b, from_a - from_b);
    let (a, e, f) = (
        along_a.dot(along_a),
        along_b.dot(along_b),
        along_b.dot(between),
    );
    const NOTHING: f64 = 1e-24;
    // `s` and `t` say how far along each piece its nearest point is.
    let (s, t);
    if a <= NOTHING && e <= NOTHING {
        return between.length();
    }
    if a <= NOTHING {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = along_a.dot(between);
        if e <= NOTHING {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = along_a.dot(along_b);
            let denominator = a * e - b * b;
            let mut on_a = if denominator > NOTHING {
                ((b * f - c * e) / denominator).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut on_b = (b * on_a + f) / e;
            if on_b < 0.0 {
                on_b = 0.0;
                on_a = (-c / a).clamp(0.0, 1.0);
            } else if on_b > 1.0 {
                on_b = 1.0;
                on_a = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = on_a;
            t = on_b;
        }
    }
    ((from_a + along_a * s) - (from_b + along_b * t)).length()
}

/// Where the line from `origin` along `direction` goes through a
/// triangle, as a multiple of `direction`. `None` if it misses, or runs
/// alongside. (Möller and Trumbore's method.)
fn crossing(origin: Vec3, direction: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f64> {
    let (edge_1, edge_2) = (b - a, c - a);
    let h = direction.cross(edge_2);
    let determinant = edge_1.dot(h);
    if determinant.abs() <= 1e-12 * edge_1.length() * edge_2.length() * direction.length() {
        return None;
    }
    let s = origin - a;
    let u = s.dot(h) / determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(edge_1);
    let v = direction.dot(q) / determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(edge_2.dot(q) / determinant)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE: [Vec3; 3] = [
        vec3(0.0, 0.0, 0.0),
        vec3(2.0, 0.0, 0.0),
        vec3(0.0, 2.0, 0.0),
    ];

    #[test]
    fn the_nearest_point_of_a_triangle() {
        // Above the inside: straight down.
        assert_eq!(
            closest_on_triangle(vec3(0.5, 0.5, 3.0), TRIANGLE),
            vec3(0.5, 0.5, 0.0)
        );
        // Beyond a corner: the corner.
        assert_eq!(
            closest_on_triangle(vec3(-1.0, -1.0, 0.0), TRIANGLE),
            vec3(0.0, 0.0, 0.0)
        );
        assert_eq!(
            closest_on_triangle(vec3(5.0, -1.0, 1.0), TRIANGLE),
            vec3(2.0, 0.0, 0.0)
        );
        // Beside an edge: on the edge.
        assert_eq!(
            closest_on_triangle(vec3(1.0, -4.0, 0.0), TRIANGLE),
            vec3(1.0, 0.0, 0.0)
        );
        assert_eq!(
            closest_on_triangle(vec3(2.0, 2.0, 0.0), TRIANGLE),
            vec3(1.0, 1.0, 0.0)
        );
    }

    #[test]
    fn the_distance_between_two_pieces_of_line() {
        let (o, x, y, z) = (
            Vec3::ZERO,
            vec3(1.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            vec3(0.0, 0.0, 1.0),
        );
        // Crossing above each other.
        assert_eq!(segment_distance(o - x, x, y - z, y + z), 1.0);
        // End to end.
        assert_eq!(segment_distance(o, x, x * 3.0, x * 4.0), 2.0);
        // Side by side.
        assert_eq!(segment_distance(o, x, y * 2.0, x + y * 2.0), 2.0);
        // Touching.
        assert_eq!(segment_distance(o, x, x, x + y), 0.0);
        // A point and a line.
        assert_eq!(segment_distance(y, y, o - x, x), 1.0);
    }

    #[test]
    fn a_line_through_a_triangle() {
        let down = vec3(0.0, 0.0, -1.0);
        assert_eq!(crossing(vec3(0.5, 0.5, 3.0), down, TRIANGLE), Some(3.0));
        // Beside it.
        assert_eq!(crossing(vec3(3.0, 3.0, 3.0), down, TRIANGLE), None);
        // Alongside it.
        assert_eq!(
            crossing(vec3(0.5, 0.5, 3.0), vec3(1.0, 0.0, 0.0), TRIANGLE),
            None
        );
        // Behind the start: a negative distance.
        assert_eq!(crossing(vec3(0.5, 0.5, -3.0), down, TRIANGLE), Some(-3.0));
    }

    #[test]
    fn the_distance_between_two_triangles() {
        let lifted = TRIANGLE.map(|corner| corner + vec3(0.0, 0.0, 0.25));
        assert_eq!(triangle_distance(TRIANGLE, lifted), 0.25);
        // One standing through the other.
        let through = [
            vec3(0.5, 0.5, -1.0),
            vec3(0.5, 0.5, 1.0),
            vec3(0.6, 0.6, 1.0),
        ];
        assert_eq!(triangle_distance(TRIANGLE, through), 0.0);
        // Corner to corner.
        let away = TRIANGLE.map(|corner| corner + vec3(-5.0, 0.0, 0.0));
        assert_eq!(triangle_distance(TRIANGLE, away), 3.0);
        // Sharing an edge.
        let neighbour = [
            vec3(0.0, 0.0, 0.0),
            vec3(2.0, 0.0, 0.0),
            vec3(0.0, -2.0, 0.0),
        ];
        assert_eq!(triangle_distance(TRIANGLE, neighbour), 0.0);
    }

    #[test]
    fn lengths_are_written_for_people() {
        assert_eq!(length(0.4), "0.40");
        assert_eq!(length(1.02), "1.02");
        assert_eq!(length(0.125), "0.125");
        assert_eq!(length(12.0), "12.00");
        assert_eq!(length(0.0004), "0.00");
        assert_eq!(length(-0.0), "0.00");
        assert_eq!(count(1, "part"), "1 part");
        assert_eq!(count(0, "overlap"), "0 overlaps");
    }
}
