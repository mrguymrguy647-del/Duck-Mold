//! Hit shapes: the simple shapes (a box, a ball, a capsule) that stand
//! in for a model's parts when a game asks what was hit.
//!
//! A hit shape is fitted from a part's own corners, in the part's own
//! directions, so a turned part gets a turned box, and it moves with the
//! part's join when the model is posed. See `DESIGN.md`, section 22.

use crate::math::{Mat3, Transform, Vec3, tidy, vec3};
use crate::model::{Collider, HitShape, Model, Part};
use crate::motion::directions;
use crate::recipe::{ColliderDef, Colliders, HitKind, OwnHitShape};

/// How much of a part may be outside its own hit shape before it is a
/// warning: more than half of its corners, each further out than a tenth
/// of the part's size.
const OUTSIDE_SHARE: f64 = 0.5;

/// How much bigger than its part a hit shape of its own may be (by
/// volume, against the shape fitted to it) before it is a warning.
const TOO_BIG: f64 = 8.0;

/// What [`fit`] found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fitted {
    /// The hit shapes, at rest.
    pub colliders: Vec<Collider>,
    /// The parts with faces that have no hit shape (when the model's
    /// `colliders` is set), and whether they say so themselves.
    pub without: Vec<(String, bool)>,
    /// Hit shapes of their own that don't suit their part: the part's
    /// path and what is wrong, for a warning.
    pub warnings: Vec<(String, String)>,
}

/// The shape that suits a part, by what it is made of and how long it
/// is: a sphere for a sphere; a capsule for a cylinder, a capsule or a
/// cone that is clearly longer than it is wide (`long`); a box for
/// everything else (a flat disc, a wheel, included).
pub fn suits(shape: &str, long: bool) -> HitKind {
    match shape {
        "sphere" => HitKind::Sphere,
        "cylinder" | "capsule" | "cone" if long => HitKind::Capsule,
        _ => HitKind::Box,
    }
}

/// True if a part is clearly longer one way than any other: its longest
/// side at least 1.2 times its next.
fn is_long(corners: &[Vec3], frame: Frame) -> bool {
    let Some(first) = corners.first() else {
        return false;
    };
    let mut low = frame.in_frame(*first);
    let mut high = low;
    for corner in corners {
        let local = frame.in_frame(*corner);
        low = low.min(local);
        high = high.max(local);
    }
    let size = high - low;
    let mut sides = [size.x, size.y, size.z];
    sides.sort_by(|a, b| b.total_cmp(a));
    sides[0] >= sides[1] * 1.2
}

/// Works out a model's hit shapes, as `colliders` (the recipe's line)
/// and each part's own `collider` say.
pub fn fit(model: &Model, setting: Option<Colliders>) -> Fitted {
    let mut fitted = Fitted::default();
    if let Some(Colliders::Whole(kind)) = setting {
        let corners: Vec<Vec3> = model
            .faces
            .iter()
            .flat_map(|face| face.corners.iter().copied())
            .collect();
        if let Some(shape) = fit_points(kind, &corners, Frame::model(), Some(1)) {
            fitted.colliders.push(Collider {
                part: String::new(),
                shape,
                own: false,
            });
        }
    }
    let each_part = setting == Some(Colliders::Parts);
    walk(
        model,
        &model.parts,
        each_part,
        setting.is_some(),
        &mut fitted,
    );
    fitted
}

/// Fits the shapes of `parts` and the parts inside them.
fn walk(model: &Model, parts: &[Part], each_part: bool, asked: bool, fitted: &mut Fitted) {
    for part in parts {
        let corners = corners_of(model, part);
        let own_faces = part.parts.is_empty() && !corners.is_empty();
        match &part.collider {
            ColliderDef::None => {
                // No shape for it, nor for the parts inside it.
                if asked {
                    for path in solid_paths(part) {
                        fitted.without.push((path, true));
                    }
                }
                continue;
            }
            ColliderDef::Fitted(kind) => {
                if let Some(shape) = fit_points(*kind, &corners, Frame::of(part), None) {
                    fitted.colliders.push(Collider {
                        part: part.path.clone(),
                        shape,
                        own: false,
                    });
                }
                continue; // the parts inside it are covered by it
            }
            ColliderDef::Own(own) => {
                let shape = own_shape(own, &corners, Frame::of(part));
                check_own(part, own, &shape, &corners, fitted);
                fitted.colliders.push(Collider {
                    part: part.path.clone(),
                    shape,
                    own: true,
                });
                continue;
            }
            ColliderDef::Unset => {}
        }
        // With one shape round the whole model, no part is without one.
        if own_faces && each_part {
            let kind = suits(&part.kind.shape, is_long(&corners, Frame::of(part)));
            if let Some(shape) = fit_points(kind, &corners, Frame::of(part), None) {
                fitted.colliders.push(Collider {
                    part: part.path.clone(),
                    shape,
                    own: false,
                });
            }
        }
        walk(model, &part.parts, each_part, asked, fitted);
    }
}

/// The paths of a part and the parts inside it that have faces of their
/// own.
fn solid_paths(part: &Part) -> Vec<String> {
    if part.parts.is_empty() {
        return vec![part.path.clone()];
    }
    part.parts.iter().flat_map(solid_paths).collect()
}

/// Every corner of a part, the parts inside it included, in the model.
pub fn corners_of(model: &Model, part: &Part) -> Vec<Vec3> {
    model
        .faces
        .get(part.faces.clone())
        .unwrap_or(&[])
        .iter()
        .flat_map(|face| face.corners.iter().copied())
        .collect()
}

/// A part's own space without its stretch: its zero point and its three
/// directions, each 1 long.
#[derive(Debug, Clone, Copy)]
struct Frame {
    origin: Vec3,
    axes: Mat3,
}

impl Frame {
    fn model() -> Frame {
        Frame {
            origin: Vec3::ZERO,
            axes: Mat3::IDENTITY,
        }
    }

    fn of(part: &Part) -> Frame {
        Frame {
            origin: part.place.shift,
            axes: directions(&part.place.turn),
        }
    }

    /// A point of the model, in this frame.
    fn in_frame(&self, point: Vec3) -> Vec3 {
        let from = point - self.origin;
        vec3(
            from.dot(self.axes.column(0)),
            from.dot(self.axes.column(1)),
            from.dot(self.axes.column(2)),
        )
    }

    /// A point of this frame, in the model.
    fn out_of_frame(&self, point: Vec3) -> Vec3 {
        self.origin + self.axes * point
    }
}

/// A number kept to a millionth, so the same model always gives the
/// same shapes on every computer.
fn kept(number: f64) -> f64 {
    tidy((number * 1e6).round() / 1e6)
}

fn kept3(point: Vec3) -> Vec3 {
    vec3(kept(point.x), kept(point.y), kept(point.z))
}

/// A size kept to a millionth, never smaller than it was (a shape that
/// holds its part keeps holding it).
fn kept_up(number: f64) -> f64 {
    tidy(((number * 1e6) - 1e-3).ceil() / 1e6)
}

fn kept3_up(size: Vec3) -> Vec3 {
    vec3(kept_up(size.x), kept_up(size.y), kept_up(size.z))
}

/// The shape of `kind` that holds every one of `points`, lying in
/// `frame`'s directions. A capsule lies along its longest side, or
/// along `along` if that is given. `None` with no points.
fn fit_points(
    kind: HitKind,
    points: &[Vec3],
    frame: Frame,
    along: Option<usize>,
) -> Option<HitShape> {
    if points.is_empty() {
        return None;
    }
    // The box around them, in the frame.
    let local: Vec<Vec3> = points.iter().map(|point| frame.in_frame(*point)).collect();
    let mut low = local[0];
    let mut high = local[0];
    for point in &local {
        low = low.min(*point);
        high = high.max(*point);
    }
    let middle_local = (low + high) * 0.5;
    let half = (high - low) * 0.5;
    let center = frame.out_of_frame(middle_local);
    Some(match kind {
        HitKind::Box => HitShape::Box {
            center: kept3(center),
            axes: frame.axes,
            half: kept3_up(half),
        },
        HitKind::Sphere => {
            let radius = points
                .iter()
                .map(|point| (*point - center).length())
                .fold(0.0, f64::max);
            HitShape::Sphere {
                center: kept3(center),
                radius: kept_up(radius),
            }
        }
        HitKind::Capsule => {
            let axis = along.unwrap_or_else(|| longest(half));
            let direction = frame.axes.column(axis);
            capsule_around(points, center, direction)
        }
    })
}

/// Which of three numbers is biggest: 0, 1 or 2 (the first, on a tie).
fn longest(half: Vec3) -> usize {
    let numbers = [half.x, half.y, half.z];
    let mut best = 0;
    for at in 1..3 {
        if numbers[at] > numbers[best] + 1e-9 {
            best = at;
        }
    }
    best
}

/// The capsule along `direction` through `center` that fits `points`:
/// just thick enough to hold them all, and as long as they reach along
/// the line, round ends included. (So a cylinder's rim corners stand a
/// little outside the round ends: a capsule long enough to hold them
/// would reach well past the part.)
fn capsule_around(points: &[Vec3], center: Vec3, direction: Vec3) -> HitShape {
    let mut radius: f64 = 0.0;
    let mut reach: f64 = 0.0;
    for point in points {
        let from = *point - center;
        let along = from.dot(direction);
        radius = radius.max((from - direction * along).length());
        reach = reach.max(along.abs());
    }
    let line = (reach - radius).max(0.0);
    HitShape::Capsule {
        a: kept3(center - direction * line),
        b: kept3(center + direction * line),
        radius: kept_up(radius),
    }
}

/// A part's hit shape with numbers of its own: in its own directions,
/// measured from its own zero point.
fn own_shape(own: &OwnHitShape, corners: &[Vec3], frame: Frame) -> HitShape {
    let center = frame.out_of_frame(Vec3::from(own.center));
    match own.kind {
        HitKind::Box => HitShape::Box {
            center: kept3(center),
            axes: frame.axes,
            half: kept3(Vec3::from(own.size) * 0.5),
        },
        HitKind::Sphere => HitShape::Sphere {
            center: kept3(center),
            radius: kept(own.radius),
        },
        HitKind::Capsule => {
            let axis = own.along.unwrap_or_else(|| {
                // Along the part's longest side.
                let local: Vec<Vec3> = corners.iter().map(|c| frame.in_frame(*c)).collect();
                let mut low = local.first().copied().unwrap_or(Vec3::ZERO);
                let mut high = low;
                for point in &local {
                    low = low.min(*point);
                    high = high.max(*point);
                }
                longest((high - low) * 0.5)
            });
            let direction = frame.axes.column(axis);
            let reach = (own.length / 2.0 - own.radius).max(0.0);
            HitShape::Capsule {
                a: kept3(center - direction * reach),
                b: kept3(center + direction * reach),
                radius: kept(own.radius),
            }
        }
    }
}

/// The warnings for a hit shape of a part's own: most of the part
/// outside it, or the shape far bigger than the part.
fn check_own(
    part: &Part,
    own: &OwnHitShape,
    shape: &HitShape,
    corners: &[Vec3],
    fitted: &mut Fitted,
) {
    if corners.is_empty() {
        return;
    }
    // A corner a hair outside doesn't count: only one further out than a
    // tenth of the part's own size.
    let mut low = corners[0];
    let mut high = corners[0];
    for corner in corners {
        low = low.min(*corner);
        high = high.max(*corner);
    }
    let slack = (high - low).length() * 0.05;
    let outside = corners
        .iter()
        .filter(|corner| !shape.holds(**corner, slack))
        .count();
    if outside as f64 > corners.len() as f64 * OUTSIDE_SHARE {
        fitted.warnings.push((
            part.path.clone(),
            format!(
                "most of \"{}\" is outside its own hit shape ({} of its {} corners): a hit there would miss it. Make the {} bigger, or let it be fitted (collider = \"{}\")",
                part.path,
                outside,
                corners.len(),
                own.kind.name(),
                own.kind.name()
            ),
        ));
        return;
    }
    if let Some(fit) = fit_points(own.kind, corners, Frame::of(part), own.along) {
        let fitted_volume = fit.volume();
        if fitted_volume > 0.0 && shape.volume() > fitted_volume * TOO_BIG {
            fitted.warnings.push((
                part.path.clone(),
                format!(
                    "the hit shape of \"{}\" is more than {TOO_BIG} times as big as the part: a hit far from it would count as a hit on it",
                    part.path
                ),
            ));
        }
    }
}

impl HitShape {
    /// Which of the three shapes it is.
    pub fn kind(&self) -> HitKind {
        match self {
            HitShape::Box { .. } => HitKind::Box,
            HitShape::Sphere { .. } => HitKind::Sphere,
            HitShape::Capsule { .. } => HitKind::Capsule,
        }
    }

    /// Its middle.
    pub fn center(&self) -> Vec3 {
        match self {
            HitShape::Box { center, .. } | HitShape::Sphere { center, .. } => *center,
            HitShape::Capsule { a, b, .. } => (*a + *b) * 0.5,
        }
    }

    /// How much room it takes up, in cubic metres.
    pub fn volume(&self) -> f64 {
        use std::f64::consts::PI;
        match self {
            HitShape::Box { half, .. } => 8.0 * half.x * half.y * half.z,
            HitShape::Sphere { radius, .. } => 4.0 / 3.0 * PI * radius.powi(3),
            HitShape::Capsule { a, b, radius } => {
                PI * radius * radius * (*b - *a).length() + 4.0 / 3.0 * PI * radius.powi(3)
            }
        }
    }

    /// True if `point` is inside it, or within `slack` of it.
    pub fn holds(&self, point: Vec3, slack: f64) -> bool {
        self.distance_outside(point) <= slack
    }

    /// How far `point` is outside it: 0 if it is inside.
    pub fn distance_outside(&self, point: Vec3) -> f64 {
        match self {
            HitShape::Box { center, axes, half } => {
                let from = point - *center;
                let mut outside = Vec3::ZERO;
                let reach = [half.x, half.y, half.z];
                let mut each = [0.0; 3];
                for (at, item) in each.iter_mut().enumerate() {
                    let along = from.dot(axes.column(at));
                    *item = (along.abs() - reach[at]).max(0.0);
                }
                outside = outside + vec3(each[0], each[1], each[2]);
                outside.length()
            }
            HitShape::Sphere { center, radius } => ((point - *center).length() - radius).max(0.0),
            HitShape::Capsule { a, b, radius } => {
                (distance_to_line(point, *a, *b) - radius).max(0.0)
            }
        }
    }

    /// The same shape moved by a part's placement in a pose: a turn and
    /// a move, with no stretch.
    pub fn moved(&self, place: &Transform) -> HitShape {
        match self {
            HitShape::Box { center, axes, half } => HitShape::Box {
                center: place.apply(*center),
                axes: directions(&(place.turn * *axes)),
                half: *half,
            },
            HitShape::Sphere { center, radius } => HitShape::Sphere {
                center: place.apply(*center),
                radius: *radius,
            },
            HitShape::Capsule { a, b, radius } => HitShape::Capsule {
                a: place.apply(*a),
                b: place.apply(*b),
                radius: *radius,
            },
        }
    }

    /// Its outline, as short straight lines: a box's twelve edges, a
    /// sphere's three rings, a capsule's two rings at its ends, its four
    /// sides and its round ends. For drawing.
    pub fn outline(&self) -> Vec<(Vec3, Vec3)> {
        const PIECES: usize = 24;
        let ring = |center: Vec3, one: Vec3, two: Vec3, radius: f64, from: f64, to: f64| {
            let mut lines = Vec::new();
            let steps = PIECES;
            let point = |at: usize| {
                let angle = from + (to - from) * at as f64 / steps as f64;
                center + one * (radius * angle.cos()) + two * (radius * angle.sin())
            };
            for at in 0..steps {
                lines.push((point(at), point(at + 1)));
            }
            lines
        };
        use std::f64::consts::PI;
        match self {
            HitShape::Box { center, axes, half } => {
                let corner = |signs: [f64; 3]| {
                    *center
                        + axes.column(0) * (half.x * signs[0])
                        + axes.column(1) * (half.y * signs[1])
                        + axes.column(2) * (half.z * signs[2])
                };
                let mut lines = Vec::new();
                for axis in 0..3 {
                    let (one, two) = ((axis + 1) % 3, (axis + 2) % 3);
                    for first in [-1.0, 1.0] {
                        for second in [-1.0, 1.0] {
                            let mut start = [0.0; 3];
                            start[axis] = -1.0;
                            start[one] = first;
                            start[two] = second;
                            let mut end = start;
                            end[axis] = 1.0;
                            lines.push((corner(start), corner(end)));
                        }
                    }
                }
                lines
            }
            HitShape::Sphere { center, radius } => {
                let (x, y, z) = (
                    vec3(1.0, 0.0, 0.0),
                    vec3(0.0, 1.0, 0.0),
                    vec3(0.0, 0.0, 1.0),
                );
                let mut lines = ring(*center, x, y, *radius, 0.0, 2.0 * PI);
                lines.extend(ring(*center, y, z, *radius, 0.0, 2.0 * PI));
                lines.extend(ring(*center, z, x, *radius, 0.0, 2.0 * PI));
                lines
            }
            HitShape::Capsule { a, b, radius } => {
                let along = (*b - *a).normalized().unwrap_or(vec3(0.0, 1.0, 0.0));
                // Two directions square to the line.
                let helper = if along.y.abs() < 0.9 {
                    vec3(0.0, 1.0, 0.0)
                } else {
                    vec3(1.0, 0.0, 0.0)
                };
                let one = along
                    .cross(helper)
                    .normalized()
                    .unwrap_or(vec3(1.0, 0.0, 0.0));
                let two = along.cross(one);
                let mut lines = ring(*a, one, two, *radius, 0.0, 2.0 * PI);
                lines.extend(ring(*b, one, two, *radius, 0.0, 2.0 * PI));
                for side in [one, -one, two, -two] {
                    lines.push((*a + side * *radius, *b + side * *radius));
                }
                // The round ends, as two half rings each.
                for side in [one, two] {
                    lines.extend(ring(*b, side, along, *radius, 0.0, PI));
                    lines.extend(ring(*a, side, -along, *radius, 0.0, PI));
                }
                lines
            }
        }
    }

    /// For people: `box 0.6 x 0.7 x 0.36`, `sphere r 0.21`, `capsule r
    /// 0.09, 0.45 long`.
    pub fn describe(&self) -> String {
        let short = |number: f64| {
            let text = format!("{:.3}", number);
            let text = text.trim_end_matches('0').trim_end_matches('.');
            if text.is_empty() || text == "-" {
                "0".to_owned()
            } else {
                text.to_owned()
            }
        };
        match self {
            HitShape::Box { half, .. } => format!(
                "box {} x {} x {}",
                short(half.x * 2.0),
                short(half.y * 2.0),
                short(half.z * 2.0)
            ),
            HitShape::Sphere { radius, .. } => format!("sphere r {}", short(*radius)),
            HitShape::Capsule { a, b, radius } => format!(
                "capsule r {}, {} long",
                short(*radius),
                short((*b - *a).length() + 2.0 * radius)
            ),
        }
    }
}

/// How far `point` is from the straight line between `a` and `b`.
pub fn distance_to_line(point: Vec3, a: Vec3, b: Vec3) -> f64 {
    let line = b - a;
    let length = line.dot(line);
    let along = if length > 0.0 {
        ((point - a).dot(line) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point - (a + line * along)).length()
}

/// A part's fitted shape of `kind`, for the Studio's panel and for
/// `set-collider` with only some numbers: where it is, in the part's own
/// directions, measured from its own zero point, as a recipe would write
/// it.
pub fn fitted_numbers(
    model: &Model,
    part: &Part,
    kind: HitKind,
    along: Option<usize>,
) -> Option<OwnHitShape> {
    let corners = corners_of(model, part);
    let frame = Frame::of(part);
    let shape = fit_points(kind, &corners, frame, along)?;
    let local = |point: Vec3| kept3(frame.in_frame(point)).to_array();
    Some(match shape {
        HitShape::Box { center, half, .. } => OwnHitShape {
            kind,
            center: local(center),
            size: (kept3(half * 2.0)).to_array(),
            radius: 0.0,
            length: 0.0,
            along: None,
        },
        HitShape::Sphere { center, radius } => OwnHitShape {
            kind,
            center: local(center),
            size: [0.0; 3],
            radius,
            length: 0.0,
            along: None,
        },
        HitShape::Capsule { a, b, radius } => {
            let direction = frame.in_frame(b) - frame.in_frame(a);
            let axis = along.unwrap_or_else(|| {
                longest(vec3(
                    direction.x.abs(),
                    direction.y.abs(),
                    direction.z.abs(),
                ))
            });
            OwnHitShape {
                kind,
                center: local((a + b) * 0.5),
                size: [0.0; 3],
                radius,
                length: kept((b - a).length() + 2.0 * radius),
                along: Some(axis),
            }
        }
    })
}
