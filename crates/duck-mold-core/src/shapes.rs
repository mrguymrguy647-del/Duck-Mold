//! The built-in shapes: box, cylinder, sphere, cone, wedge, capsule,
//! torus, tube, pyramid, prism, stairs and arch.
//!
//! Each shape is a list of flat faces ([`Polygon`]s), centred on its own
//! middle, like Duck Forge's built-in meshes. Every face belongs to a
//! named group (`top`, `side`, `slope`...), which is what a recipe's
//! `colors = { top = "grass" }` names.
//!
//! A face's corners go clockwise seen from outside, which is Duck Forge's
//! order. In numbers: for corners `a`, `b`, `c`, the direction
//! `(b - a).cross(c - a)` points out of the shape.

use crate::math::{Transform, Vec3, sin_cos_degrees, vec3};
use crate::recipe::{Shape, Trim};

/// One flat face: three or more corners in a plane, clockwise seen from
/// outside.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    /// The corners.
    pub corners: Vec<Vec3>,
    /// Which named group of faces it belongs to: a place in the shape's
    /// list of face names.
    pub group: usize,
}

impl Polygon {
    /// How many triangles the face is cut into: two for a four-cornered
    /// face, and so on.
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
        // Adding up over all corners works for any flat face, also one
        // whose first three corners lie in a line.
        let mut sum = Vec3::ZERO;
        for (at, &corner) in self.corners.iter().enumerate() {
            let next = self.corners[(at + 1) % self.corners.len()];
            sum = sum + corner.cross(next);
        }
        sum.normalized()
    }

    /// The face moved, turned, stretched or flipped. A flip reverses the
    /// corners, so the face still points outwards.
    pub fn transformed(&self, transform: &Transform) -> Polygon {
        let mut corners: Vec<Vec3> = self
            .corners
            .iter()
            .map(|&corner| transform.apply(corner))
            .collect();
        if transform.is_mirrored() {
            corners.reverse();
        }
        Polygon {
            corners,
            group: self.group,
        }
    }

    /// Whether `point`, which is in the face's plane, is inside the face
    /// or on its edge (allowing `near` for rounding). The face must bulge
    /// nowhere inwards, as every face of the built-in shapes does.
    fn contains(&self, point: Vec3, normal: Vec3, near: f64) -> bool {
        (0..self.corners.len()).all(|at| {
            let start = self.corners[at];
            let edge = self.corners[(at + 1) % self.corners.len()] - start;
            // How far the point is inside this edge, times the edge's
            // length: negative outside.
            edge.cross(point - start).dot(normal) >= -near * edge.length()
        })
    }
}

/// Where a line coming straight in from the box around a shape first
/// touches the shape. See [`land`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Landing {
    /// The line starts on the surface already: nothing moves.
    Already,
    /// It touches the surface at `at`, where the surface faces `facing`
    /// (1 long). On an edge or a corner, `facing` is halfway between the
    /// faces that meet there.
    Touches {
        /// Where.
        at: Vec3,
        /// The way the surface faces there.
        facing: Vec3,
    },
    /// It passes beside the shape without touching it.
    Misses,
}

/// Follows a line from `start` in the direction `inward` (1 long) to
/// where it first touches one of `faces`.
///
/// This is how a socket on a side of a shape's box finds the shape's
/// surface: from the side, straight in. `size` is about how big the
/// shape is; points closer than a billionth of it count as touching, so
/// rounding never decides.
pub fn land(faces: &[Polygon], start: Vec3, inward: Vec3, size: f64) -> Landing {
    let near = 1e-9 * size.max(1e-9);
    // Every face the line goes through: how far along, and its normal.
    let mut hits: Vec<(f64, Vec3)> = Vec::new();
    for polygon in faces {
        let Some(normal) = polygon.normal() else {
            continue; // a face with no area
        };
        let towards = normal.dot(inward);
        if towards.abs() < 1e-12 {
            continue; // the line runs along the face, not through it
        }
        let along = normal.dot(polygon.corners[0] - start) / towards;
        if along < -near {
            continue; // behind the start
        }
        if polygon.contains(start + inward * along, normal, near) {
            hits.push((along, normal));
        }
    }
    let Some(first) = hits.iter().map(|&(along, _)| along).reduce(f64::min) else {
        return Landing::Misses;
    };
    if first <= near {
        return Landing::Already;
    }
    let sum = hits
        .iter()
        .filter(|&&(along, _)| along <= first + near)
        .fold(Vec3::ZERO, |sum, &(_, normal)| sum + normal);
    Landing::Touches {
        at: start + inward * first,
        facing: sum.normalized().unwrap_or(-inward),
    }
}

/// The names of a shape's groups of faces, in the order [`build`] numbers
/// them. A shape whose edges are cut has one more, `bevel`, after these.
pub fn face_names(shape: &Shape) -> &'static [&'static str] {
    match shape {
        Shape::Box { .. } => &["top", "bottom", "left", "right", "front", "back"],
        Shape::Cylinder { .. } => &["top", "bottom", "side"],
        Shape::Sphere { .. } | Shape::Torus { .. } => &["side"],
        Shape::Cone { .. } => &["side", "bottom"],
        Shape::Wedge { .. } => &["slope", "bottom", "back", "left", "right"],
        Shape::Capsule { .. } => &["side", "top", "bottom"],
        Shape::Tube { .. } => &["side", "inside", "top", "bottom"],
        Shape::Pyramid { .. } => &["front", "back", "left", "right", "bottom"],
        Shape::Prism { .. } => &["left", "right", "front", "back", "bottom"],
        Shape::Stairs { .. } => &["top", "front", "back", "left", "right", "bottom"],
        Shape::Arch { .. } => &["front", "back", "left", "right", "top", "bottom", "inside"],
    }
}

/// The name of the faces a bevel makes.
pub const BEVEL: &str = "bevel";

/// Whether the shape, with this trim, has faces called `bevel`.
fn has_bevel(shape: &Shape, trim: &Trim) -> bool {
    trim.is_bevelled() && matches!(shape, Shape::Box { .. } | Shape::Cylinder { .. })
}

/// The groups a face name stands for. Usually one; a box's `sides` is
/// short for its left, right, front and back, and a pyramid's for its
/// four slopes.
pub fn face_groups(shape: &Shape, trim: &Trim, name: &str) -> Option<Vec<usize>> {
    if name == "sides" {
        match shape {
            Shape::Box { .. } => return Some(vec![2, 3, 4, 5]),
            Shape::Pyramid { .. } => return Some(vec![0, 1, 2, 3]),
            _ => {}
        }
    }
    if name == BEVEL && has_bevel(shape, trim) {
        return Some(vec![face_names(shape).len()]);
    }
    face_names(shape)
        .iter()
        .position(|face| *face == name)
        .map(|group| vec![group])
}

/// Every face name a recipe can use for this shape, for messages.
pub fn face_words(shape: &Shape, trim: &Trim) -> Vec<&'static str> {
    let mut words = face_names(shape).to_vec();
    if matches!(shape, Shape::Box { .. } | Shape::Pyramid { .. }) {
        words.push("sides");
    }
    if has_bevel(shape, trim) {
        words.push(BEVEL);
    }
    words
}

/// The name of a group of faces of this shape, as [`build_trimmed`]
/// numbers them.
pub fn group_name(shape: &Shape, group: usize) -> &'static str {
    face_names(shape).get(group).copied().unwrap_or(BEVEL)
}

/// Builds a shape's faces, with sharp edges and straight sides.
pub fn build(shape: &Shape) -> Vec<Polygon> {
    build_trimmed(shape, &Trim::default())
}

/// Builds a shape's faces, with its edges cut and its top narrowed as
/// `trim` says.
pub fn build_trimmed(shape: &Shape, trim: &Trim) -> Vec<Polygon> {
    let bevel = trim.bevel.max(0.0);
    let steps = trim.bevel_steps.max(1);
    // Shapes that bulge outwards everywhere are turned to face outwards
    // by where their middle is; the others by going from face to face.
    let (mut polygons, bulges) = match *shape {
        Shape::Box { size } if bevel > 0.0 => (bevelled_box_faces(size.into(), bevel, steps), true),
        Shape::Box { size } => (box_faces(size.into()), true),
        Shape::Cylinder {
            radius,
            height,
            segments,
        } if bevel > 0.0 => (
            bevelled_cylinder_faces(radius, height, segments, bevel, steps),
            true,
        ),
        Shape::Cylinder {
            radius,
            height,
            segments,
        } => (cylinder_faces(radius, height, segments), true),
        Shape::Sphere { radius, segments } => (sphere_faces(radius, segments), true),
        Shape::Cone {
            radius,
            height,
            segments,
        } => (cone_faces(radius, height, segments), true),
        Shape::Wedge { size } => (wedge_faces(size.into()), true),
        Shape::Capsule {
            radius,
            height,
            segments,
        } => (capsule_faces(radius, height, segments), true),
        Shape::Torus {
            radius,
            tube,
            segments,
        } => (torus_faces(radius, tube, segments), false),
        Shape::Tube {
            radius,
            wall,
            height,
            segments,
        } => (tube_faces(radius, wall, height, segments), false),
        Shape::Pyramid { size } => (pyramid_faces(size.into()), true),
        Shape::Prism { size } => (prism_faces(size.into()), true),
        Shape::Stairs { size, steps } => (stairs_faces(size.into(), steps), false),
        Shape::Arch {
            size,
            wall,
            segments,
        } => (arch_faces(size.into(), wall, segments), false),
    };
    if bulges {
        face_outwards(&mut polygons);
    } else {
        face_outwards_all_round(&mut polygons);
    }
    if trim.is_tapered()
        && matches!(
            shape,
            Shape::Box { .. } | Shape::Cylinder { .. } | Shape::Tube { .. }
        )
    {
        polygons = tapered(polygons, trim.taper);
    }
    polygons
}

/// Makes the top of a shape narrower or wider than its bottom: each
/// corner moves towards or away from the upright line through the
/// middle, the more the higher it is. A face this bends is cut into
/// triangles, which are always flat.
fn tapered(polygons: Vec<Polygon>, [across, deep]: [f64; 2]) -> Vec<Polygon> {
    let heights = polygons
        .iter()
        .flat_map(|polygon| &polygon.corners)
        .map(|corner| corner.y);
    let (low, high) = heights.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), y| {
        (low.min(y), high.max(y))
    });
    let tall = (high - low).max(f64::MIN_POSITIVE);
    let moved = |corner: Vec3| {
        let up = (corner.y - low) / tall;
        vec3(
            corner.x * (1.0 + (across - 1.0) * up),
            corner.y,
            corner.z * (1.0 + (deep - 1.0) * up),
        )
    };
    let mut out = Vec::with_capacity(polygons.len());
    for polygon in polygons {
        let bent = Polygon {
            corners: polygon
                .corners
                .iter()
                .map(|&corner| moved(corner))
                .collect(),
            group: polygon.group,
        };
        let size = bent
            .corners
            .iter()
            .fold(0.0f64, |size, corner| size.max(corner.length()));
        let flat = bent.normal().is_some_and(|normal| {
            bent.corners
                .iter()
                .all(|&corner| normal.dot(corner - bent.corners[0]).abs() <= 1e-10 * size.max(1.0))
        });
        if flat || bent.corners.len() <= 3 {
            out.push(bent);
        } else {
            out.extend(
                bent.triangles()
                    .map(|corners| face(&corners, bent.group))
                    .filter(|triangle| triangle.normal().is_some()),
            );
        }
    }
    out
}

/// Makes every face of a closed shape point away from its inside, also
/// one with a hole through it or a hollow in it. One face is known to
/// point outwards: one at the corner furthest to the right. From there
/// it goes from face to face across the edges they share: two faces
/// that point the same way go along their shared edge in opposite
/// directions.
fn face_outwards_all_round(polygons: &mut [Polygon]) {
    use std::collections::BTreeMap;
    let key = |corner: Vec3| {
        let corner = corner.tidied();
        [corner.x.to_bits(), corner.y.to_bits(), corner.z.to_bits()]
    };
    // Each edge, by its two ends in order, with the faces along it and
    // whether each goes from the first end to the second.
    type Corner = [u64; 3];
    let mut edges: BTreeMap<(Corner, Corner), Vec<(usize, bool)>> = BTreeMap::new();
    for (at, polygon) in polygons.iter().enumerate() {
        for (corner, &from) in polygon.corners.iter().enumerate() {
            let to = polygon.corners[(corner + 1) % polygon.corners.len()];
            let (from, to) = (key(from), key(to));
            let forwards = from < to;
            let ends = if forwards { (from, to) } else { (to, from) };
            edges.entry(ends).or_default().push((at, forwards));
        }
    }
    // The face to start from.
    let mut start: Option<(usize, f64, f64)> = None;
    for (at, polygon) in polygons.iter().enumerate() {
        let Some(normal) = polygon.normal() else {
            continue;
        };
        let right = polygon
            .corners
            .iter()
            .fold(f64::NEG_INFINITY, |right, corner| right.max(corner.x));
        let better = match start {
            None => true,
            Some((_, best_right, best_facing)) => {
                right > best_right + 1e-9
                    || ((right - best_right).abs() <= 1e-9 && normal.x.abs() > best_facing)
            }
        };
        if better {
            start = Some((at, right, normal.x.abs()));
        }
    }
    let Some((start, _, _)) = start else {
        return;
    };
    // Whether each face is to be turned round; `None` until it is reached.
    let mut turned: Vec<Option<bool>> = vec![None; polygons.len()];
    turned[start] = Some(
        polygons[start]
            .normal()
            .is_some_and(|normal| normal.x < 0.0),
    );
    let mut waiting = vec![start];
    while let Some(at) = waiting.pop() {
        let this_turned = turned[at].unwrap_or(false);
        let corners = polygons[at].corners.clone();
        for (corner, &from) in corners.iter().enumerate() {
            let to = corners[(corner + 1) % corners.len()];
            let (from, to) = (key(from), key(to));
            let forwards = from < to;
            let ends = if forwards { (from, to) } else { (to, from) };
            for &(other, other_forwards) in &edges[&ends] {
                if other == at || turned[other].is_some() {
                    continue;
                }
                // As written, they should go opposite ways. If they do,
                // the other is turned exactly when this one is.
                let opposite = forwards != other_forwards;
                turned[other] = Some(if opposite { this_turned } else { !this_turned });
                waiting.push(other);
            }
        }
    }
    for (polygon, turned) in polygons.iter_mut().zip(turned) {
        if turned == Some(true) {
            polygon.corners.reverse();
        }
    }
}

/// Makes every face of a solid shape point away from its inside, by
/// reversing the corners of any that doesn't. This is for the shapes
/// that bulge outwards everywhere, where "away from the middle of the
/// corners" is always "outwards".
fn face_outwards(polygons: &mut [Polygon]) {
    let mut sum = Vec3::ZERO;
    let mut count = 0.0;
    for corner in polygons.iter().flat_map(|polygon| &polygon.corners) {
        sum = sum + *corner;
        count += 1.0;
    }
    let inside = sum * (1.0 / count);
    for polygon in polygons {
        let middle = polygon
            .corners
            .iter()
            .fold(Vec3::ZERO, |sum, &corner| sum + corner)
            * (1.0 / polygon.corners.len() as f64);
        if let Some(normal) = polygon.normal()
            && normal.dot(middle - inside) < 0.0
        {
            polygon.corners.reverse();
        }
    }
}

fn face(corners: &[Vec3], group: usize) -> Polygon {
    Polygon {
        corners: corners.to_vec(),
        group,
    }
}

/// A box: six four-cornered faces.
fn box_faces(size: Vec3) -> Vec<Polygon> {
    let h = size * 0.5;
    // The eight corners, named by where they are: left/right, bottom/top,
    // back/front.
    let corner = |x: f64, y: f64, z: f64| vec3(x * h.x, y * h.y, z * h.z);
    let (lbb, rbb, lbf, rbf) = (
        corner(-1.0, -1.0, -1.0),
        corner(1.0, -1.0, -1.0),
        corner(-1.0, -1.0, 1.0),
        corner(1.0, -1.0, 1.0),
    );
    let (ltb, rtb, ltf, rtf) = (
        corner(-1.0, 1.0, -1.0),
        corner(1.0, 1.0, -1.0),
        corner(-1.0, 1.0, 1.0),
        corner(1.0, 1.0, 1.0),
    );
    vec![
        face(&[ltb, ltf, rtf, rtb], 0), // top
        face(&[lbb, rbb, rbf, lbf], 1), // bottom
        face(&[lbb, lbf, ltf, ltb], 2), // left
        face(&[rbb, rtb, rtf, rbf], 3), // right
        face(&[lbf, rbf, rtf, ltf], 4), // front
        face(&[lbb, ltb, rtb, rbb], 5), // back
    ]
}

/// The corners of a ring of `segments` flat sides, at height `y`.
///
/// `radius` is measured to the middle of a flat side, not to a corner,
/// so that a cylinder of radius 0.5 is exactly 1 wide, and a part
/// attached to its side meets a flat face. The first flat side faces
/// right (+x).
fn ring(radius: f64, y: f64, segments: usize) -> Vec<Vec3> {
    let step = 360.0 / segments as f64;
    // How far out the corners are, for the flat sides to be at `radius`.
    let (_, cos_half) = sin_cos_degrees(step / 2.0);
    let corner_radius = radius / cos_half;
    (0..segments)
        .map(|at| {
            let (sin, cos) = sin_cos_degrees((at as f64 + 0.5) * step);
            vec3(corner_radius * cos, y, corner_radius * sin)
        })
        .collect()
}

/// An upright cylinder: a top, a bottom, and one four-cornered face per
/// segment round the side.
fn cylinder_faces(radius: f64, height: f64, segments: usize) -> Vec<Polygon> {
    let top = ring(radius, height / 2.0, segments);
    let bottom = ring(radius, -height / 2.0, segments);
    let mut polygons = vec![face(&top, 0), face(&bottom, 1)];
    for at in 0..segments {
        let next = (at + 1) % segments;
        polygons.push(face(&[bottom[at], bottom[next], top[next], top[at]], 2));
    }
    polygons
}

/// A cone, point at the top: one triangle per segment, and a base.
fn cone_faces(radius: f64, height: f64, segments: usize) -> Vec<Polygon> {
    let base = ring(radius, -height / 2.0, segments);
    let point = vec3(0.0, height / 2.0, 0.0);
    let mut polygons = Vec::with_capacity(segments + 1);
    for at in 0..segments {
        polygons.push(face(&[base[at], base[(at + 1) % segments], point], 0));
    }
    polygons.push(face(&base, 1));
    polygons
}

/// A ball: rings of faces from the top point to the bottom point, like
/// the lines on a globe. `segments` is how many faces go round its
/// middle; half as many rings go from top to bottom.
fn sphere_faces(radius: f64, segments: usize) -> Vec<Polygon> {
    let rings = (segments / 2).max(2);
    // The corners on ring `ring`, counted from the top point (ring 0) to
    // the bottom point (ring `rings`).
    let corner = |ring: usize, at: usize| {
        let (sin_down, cos_down) = sin_cos_degrees(180.0 * ring as f64 / rings as f64);
        let (sin_round, cos_round) =
            sin_cos_degrees(360.0 * (at % segments) as f64 / segments as f64);
        vec3(
            radius * sin_down * cos_round,
            radius * cos_down,
            radius * sin_down * sin_round,
        )
    };
    let top = vec3(0.0, radius, 0.0);
    let bottom = vec3(0.0, -radius, 0.0);
    let mut polygons = Vec::new();
    for at in 0..segments {
        polygons.push(face(&[top, corner(1, at), corner(1, at + 1)], 0));
    }
    for ring in 1..rings - 1 {
        for at in 0..segments {
            polygons.push(face(
                &[
                    corner(ring, at),
                    corner(ring + 1, at),
                    corner(ring + 1, at + 1),
                    corner(ring, at + 1),
                ],
                0,
            ));
        }
    }
    for at in 0..segments {
        polygons.push(face(
            &[bottom, corner(rings - 1, at + 1), corner(rings - 1, at)],
            0,
        ));
    }
    polygons
}

/// A ramp: a box cut corner to corner. Full height at the back, sloping
/// down to the front edge.
fn wedge_faces(size: Vec3) -> Vec<Polygon> {
    let h = size * 0.5;
    let front_left = vec3(-h.x, -h.y, h.z);
    let front_right = vec3(h.x, -h.y, h.z);
    let back_left = vec3(-h.x, -h.y, -h.z);
    let back_right = vec3(h.x, -h.y, -h.z);
    let top_left = vec3(-h.x, h.y, -h.z);
    let top_right = vec3(h.x, h.y, -h.z);
    vec![
        face(&[front_left, front_right, top_right, top_left], 0), // slope
        face(&[front_left, back_left, back_right, front_right], 1), // bottom
        face(&[back_left, top_left, top_right, back_right], 2),   // back
        face(&[front_left, top_left, back_left], 3),              // left
        face(&[front_right, back_right, top_right], 4),           // right
    ]
}

/// A box with its edges cut off: the six faces, each smaller by the
/// bevel all round; a strip along each of the twelve edges; and a patch
/// at each of the eight corners. With one step the strip is one flat
/// face and the patch one triangle; with more they are rounder.
fn bevelled_box_faces(size: Vec3, bevel: f64, steps: usize) -> Vec<Polygon> {
    const BEVEL_GROUP: usize = 6;
    let h = size * 0.5;
    // The box inside, whose corners the rounding goes about.
    let inner = vec3(h.x - bevel, h.y - bevel, h.z - bevel);
    // The point of the corner patch at (i, j, k) of `steps`, for the
    // corner whose signs are `sign`: i, j and k say how far towards x, y
    // and z it leans.
    let point = |sign: [f64; 3], i: usize, j: usize, k: usize| {
        let lean = |part: usize| sin_cos_degrees(90.0 * part as f64 / steps as f64).0;
        let way = vec3(lean(i), lean(j), lean(k))
            .normalized()
            .unwrap_or(Vec3::ZERO);
        vec3(
            sign[0] * (inner.x + bevel * way.x),
            sign[1] * (inner.y + bevel * way.y),
            sign[2] * (inner.z + bevel * way.z),
        )
    };
    let signs = [-1.0, 1.0];
    let mut polygons = Vec::new();
    // The six faces: top, bottom, left, right, front, back.
    for (group, (axis, side)) in [
        (1, 1.0),
        (1, -1.0),
        (0, -1.0),
        (0, 1.0),
        (2, 1.0),
        (2, -1.0),
    ]
    .into_iter()
    .enumerate()
    {
        // The other two directions, and the corners going round.
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        let corners: Vec<Vec3> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .into_iter()
            .map(|(sign_a, sign_b)| {
                let mut sign = [0.0; 3];
                sign[axis] = side;
                sign[a] = sign_a;
                sign[b] = sign_b;
                let mut lean = [0; 3];
                lean[axis] = steps;
                point(sign, lean[0], lean[1], lean[2])
            })
            .collect();
        polygons.push(face(&corners, group));
    }
    // The twelve edges: along each direction, at each pair of signs of
    // the other two.
    for axis in 0..3 {
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        for sign_a in signs {
            for sign_b in signs {
                for step in 0..steps {
                    let at = |end: f64, step: usize| {
                        let mut sign = [0.0; 3];
                        sign[axis] = end;
                        sign[a] = sign_a;
                        sign[b] = sign_b;
                        let mut lean = [0; 3];
                        lean[a] = step;
                        lean[b] = steps - step;
                        point(sign, lean[0], lean[1], lean[2])
                    };
                    polygons.push(face(
                        &[
                            at(1.0, step),
                            at(1.0, step + 1),
                            at(-1.0, step + 1),
                            at(-1.0, step),
                        ],
                        BEVEL_GROUP,
                    ));
                }
            }
        }
    }
    // The eight corners.
    for sign_x in signs {
        for sign_y in signs {
            for sign_z in signs {
                let sign = [sign_x, sign_y, sign_z];
                for i in 0..steps {
                    for j in 0..steps - i {
                        let k = steps - i - j;
                        polygons.push(face(
                            &[
                                point(sign, i, j, k),
                                point(sign, i + 1, j, k - 1),
                                point(sign, i, j + 1, k - 1),
                            ],
                            BEVEL_GROUP,
                        ));
                        if k >= 2 {
                            polygons.push(face(
                                &[
                                    point(sign, i + 1, j, k - 1),
                                    point(sign, i + 1, j + 1, k - 2),
                                    point(sign, i, j + 1, k - 1),
                                ],
                                BEVEL_GROUP,
                            ));
                        }
                    }
                }
            }
        }
    }
    polygons
}

/// A cylinder with the rims of its top and bottom cut off.
fn bevelled_cylinder_faces(
    radius: f64,
    height: f64,
    segments: usize,
    bevel: f64,
    steps: usize,
) -> Vec<Polygon> {
    const BEVEL_GROUP: usize = 3;
    // The rings from the top face down to the bottom face.
    let mut rings: Vec<(Vec<Vec3>, usize)> = Vec::new();
    for step in 0..=steps {
        let (sin, cos) = sin_cos_degrees(90.0 * step as f64 / steps as f64);
        rings.push((
            ring(
                radius - bevel + bevel * sin,
                height / 2.0 - bevel + bevel * cos,
                segments,
            ),
            BEVEL_GROUP,
        ));
    }
    // The side, below the last ring of the top's bevel.
    if let Some(last) = rings.last_mut() {
        last.1 = 2;
    }
    for step in (0..=steps).rev() {
        let (sin, cos) = sin_cos_degrees(90.0 * step as f64 / steps as f64);
        rings.push((
            ring(
                radius - bevel + bevel * sin,
                -(height / 2.0 - bevel + bevel * cos),
                segments,
            ),
            BEVEL_GROUP,
        ));
    }
    let mut polygons = vec![face(&rings[0].0, 0), face(&rings[rings.len() - 1].0, 1)];
    for pair in rings.windows(2) {
        let ((upper, group), (lower, _)) = (&pair[0], &pair[1]);
        for at in 0..segments {
            let next = (at + 1) % segments;
            polygons.push(face(
                &[lower[at], lower[next], upper[next], upper[at]],
                *group,
            ));
        }
    }
    polygons
}

/// An upright cylinder with half a ball on each end. `height` is from
/// the lowest point to the highest.
fn capsule_faces(radius: f64, height: f64, segments: usize) -> Vec<Polygon> {
    let rings = (segments / 4).max(2);
    let straight = (height / 2.0 - radius).max(0.0);
    // Ring `ring` of an end, counted from where the end meets the side
    // (0) towards its point; `up` is 1 for the top end, -1 for the bottom.
    let level = |ring_at: usize, up: f64| {
        let (sin, cos) = sin_cos_degrees(90.0 * ring_at as f64 / rings as f64);
        ring(radius * cos, up * (straight + radius * sin), segments)
    };
    let mut polygons = Vec::new();
    if straight > 0.0 {
        let (top, bottom) = (level(0, 1.0), level(0, -1.0));
        for at in 0..segments {
            let next = (at + 1) % segments;
            polygons.push(face(&[bottom[at], bottom[next], top[next], top[at]], 0));
        }
    }
    for (up, group) in [(1.0, 1), (-1.0, 2)] {
        for ring_at in 0..rings - 1 {
            let (near, far) = (level(ring_at, up), level(ring_at + 1, up));
            for at in 0..segments {
                let next = (at + 1) % segments;
                polygons.push(face(&[near[at], near[next], far[next], far[at]], group));
            }
        }
        let last = level(rings - 1, up);
        let point = vec3(0.0, up * (straight + radius), 0.0);
        for at in 0..segments {
            polygons.push(face(&[last[at], last[(at + 1) % segments], point], group));
        }
    }
    polygons
}

/// A ring lying flat. `segments` flat sides go round the hole; half as
/// many (in fours, so that its top, bottom, inside and outside are
/// exactly where the sizes say) go round the ring itself.
fn torus_faces(radius: f64, tube: f64, segments: usize) -> Vec<Polygon> {
    let around = (segments / 2).max(4).div_ceil(4) * 4;
    let rings: Vec<Vec<Vec3>> = (0..around)
        .map(|at| {
            let (sin, cos) = sin_cos_degrees(360.0 * at as f64 / around as f64);
            ring(radius + tube * cos, tube * sin, segments)
        })
        .collect();
    let mut polygons = Vec::with_capacity(around * segments);
    for (at, this) in rings.iter().enumerate() {
        let after = &rings[(at + 1) % around];
        for side in 0..segments {
            let next = (side + 1) % segments;
            polygons.push(face(&[this[side], this[next], after[next], after[side]], 0));
        }
    }
    polygons
}

/// An upright pipe: an outside, an inside, and a flat ring at each end.
fn tube_faces(radius: f64, wall: f64, height: f64, segments: usize) -> Vec<Polygon> {
    let inner = radius - wall;
    let (top, bottom) = (
        ring(radius, height / 2.0, segments),
        ring(radius, -height / 2.0, segments),
    );
    let (top_in, bottom_in) = (
        ring(inner, height / 2.0, segments),
        ring(inner, -height / 2.0, segments),
    );
    let mut polygons = Vec::with_capacity(segments * 4);
    for at in 0..segments {
        let next = (at + 1) % segments;
        polygons.push(face(&[bottom[at], bottom[next], top[next], top[at]], 0));
        polygons.push(face(
            &[bottom_in[at], bottom_in[next], top_in[next], top_in[at]],
            1,
        ));
        polygons.push(face(&[top[at], top[next], top_in[next], top_in[at]], 2));
        polygons.push(face(
            &[bottom[at], bottom[next], bottom_in[next], bottom_in[at]],
            3,
        ));
    }
    polygons
}

/// Four slopes meeting in a point above the middle of the base.
fn pyramid_faces(size: Vec3) -> Vec<Polygon> {
    let h = size * 0.5;
    let point = vec3(0.0, h.y, 0.0);
    let (lb, rb, lf, rf) = (
        vec3(-h.x, -h.y, -h.z),
        vec3(h.x, -h.y, -h.z),
        vec3(-h.x, -h.y, h.z),
        vec3(h.x, -h.y, h.z),
    );
    vec![
        face(&[lf, rf, point], 0),  // front
        face(&[rb, lb, point], 1),  // back
        face(&[lb, lf, point], 2),  // left
        face(&[rf, rb, point], 3),  // right
        face(&[lb, rb, rf, lf], 4), // bottom
    ]
}

/// A roof: a slope on the left and one on the right, meeting in a ridge
/// that runs from front to back, with a triangle at each end.
fn prism_faces(size: Vec3) -> Vec<Polygon> {
    let h = size * 0.5;
    let (lb, rb, lf, rf) = (
        vec3(-h.x, -h.y, -h.z),
        vec3(h.x, -h.y, -h.z),
        vec3(-h.x, -h.y, h.z),
        vec3(h.x, -h.y, h.z),
    );
    let (ridge_front, ridge_back) = (vec3(0.0, h.y, h.z), vec3(0.0, h.y, -h.z));
    vec![
        face(&[lb, lf, ridge_front, ridge_back], 0), // left
        face(&[rf, rb, ridge_back, ridge_front], 1), // right
        face(&[lf, rf, ridge_front], 2),             // front
        face(&[rb, lb, ridge_back], 3),              // back
        face(&[lb, rb, rf, lf], 4),                  // bottom
    ]
}

/// Steps going up from the front edge to the full height at the back.
///
/// Each side is one upright strip per step. A strip has a corner where
/// the step in front of it ends, so that every edge is shared by exactly
/// two faces.
fn stairs_faces(size: Vec3, steps: usize) -> Vec<Polygon> {
    let h = size * 0.5;
    let steps = steps.max(1);
    // The front of step `at` (the back of the one before), and its top.
    let front = |at: usize| h.z - size.z * at as f64 / steps as f64;
    let top = |at: usize| -h.y + size.y * (at + 1) as f64 / steps as f64;
    let mut polygons = Vec::with_capacity(steps * 5 + 1);
    for at in 0..steps {
        let (z_front, z_back, y) = (front(at), front(at + 1), top(at));
        let below = if at == 0 { -h.y } else { top(at - 1) };
        // The tread and the riser.
        polygons.push(face(
            &[
                vec3(-h.x, y, z_front),
                vec3(h.x, y, z_front),
                vec3(h.x, y, z_back),
                vec3(-h.x, y, z_back),
            ],
            0,
        ));
        polygons.push(face(
            &[
                vec3(-h.x, below, z_front),
                vec3(h.x, below, z_front),
                vec3(h.x, y, z_front),
                vec3(-h.x, y, z_front),
            ],
            1,
        ));
        // The two sides, from the back corner on the ground round.
        for (x, group) in [(-h.x, 3), (h.x, 4)] {
            let mut corners = vec![vec3(x, -h.y, z_back), vec3(x, -h.y, z_front)];
            if at > 0 {
                corners.push(vec3(x, below, z_front));
            }
            corners.push(vec3(x, y, z_front));
            corners.push(vec3(x, y, z_back));
            polygons.push(face(&corners, group));
        }
        // The ground under it.
        polygons.push(face(
            &[
                vec3(-h.x, -h.y, z_front),
                vec3(h.x, -h.y, z_front),
                vec3(h.x, -h.y, z_back),
                vec3(-h.x, -h.y, z_back),
            ],
            5,
        ));
    }
    polygons.push(face(
        &[
            vec3(-h.x, -h.y, -h.z),
            vec3(h.x, -h.y, -h.z),
            vec3(h.x, h.y, -h.z),
            vec3(-h.x, h.y, -h.z),
        ],
        2,
    ));
    polygons
}

/// A wall with a doorway through it from front to back. The doorway is
/// `wall` in from each side and its round top is `wall` below the top;
/// where the arch is too low for a half circle, the top is flatter.
///
/// The front and the back are cut into pieces that bulge nowhere
/// inwards: a leg at each side, and triangles from the two top corners
/// to the round top.
fn arch_faces(size: Vec3, wall: f64, segments: usize) -> Vec<Polygon> {
    let h = size * 0.5;
    let segments = segments.max(2);
    let across = h.x - wall;
    let rise = across.min(size.y - wall);
    // Where the round top begins.
    let spring = h.y - wall - rise;
    // The round top, from left to right, as (x, y).
    let curve: Vec<(f64, f64)> = (0..=segments)
        .map(|at| {
            let (sin, cos) = sin_cos_degrees(180.0 - 180.0 * at as f64 / segments as f64);
            (across * cos, spring + rise * sin)
        })
        .collect();
    // Exactly at the sides, whatever rounding says.
    let mut curve = curve;
    curve[0] = (-across, spring);
    curve[segments] = (across, spring);
    let (left, right) = (curve[0], curve[segments]);
    let middle = segments / 2;
    let mut polygons = Vec::new();
    // A flat piece of the front or the back, from its corners as (x, y).
    // Corners that are the same point are counted once.
    let mut flat = |corners: &[(f64, f64)], z: f64, group: usize| {
        let mut points: Vec<Vec3> = Vec::new();
        for &(x, y) in corners {
            let point = vec3(x, y, z);
            if points
                .last()
                .is_none_or(|last| (*last - point).length() > 1e-12)
                && points
                    .first()
                    .is_none_or(|first| (*first - point).length() > 1e-12)
            {
                points.push(point);
            }
        }
        if points.len() >= 3 {
            polygons.push(face(&points, group));
        }
    };
    for (z, group) in [(h.z, 0), (-h.z, 1)] {
        flat(
            &[(-h.x, -h.y), (-across, -h.y), left, (-h.x, h.y)],
            z,
            group,
        );
        for at in 0..middle {
            flat(&[curve[at], curve[at + 1], (-h.x, h.y)], z, group);
        }
        flat(&[curve[middle], (h.x, h.y), (-h.x, h.y)], z, group);
        for at in middle..segments {
            flat(&[curve[at], curve[at + 1], (h.x, h.y)], z, group);
        }
        flat(&[(across, -h.y), (h.x, -h.y), (h.x, h.y), right], z, group);
    }
    // A strip from the front to the back, along a line of the outline.
    let mut strip = |from: (f64, f64), to: (f64, f64), group: usize| {
        if (from.0 - to.0).abs() + (from.1 - to.1).abs() > 1e-12 {
            polygons.push(face(
                &[
                    vec3(from.0, from.1, h.z),
                    vec3(to.0, to.1, h.z),
                    vec3(to.0, to.1, -h.z),
                    vec3(from.0, from.1, -h.z),
                ],
                group,
            ));
        }
    };
    strip((-h.x, -h.y), (-h.x, h.y), 2); // left
    strip((h.x, -h.y), (h.x, h.y), 3); // right
    strip((-h.x, h.y), (h.x, h.y), 4); // top
    strip((-h.x, -h.y), (-across, -h.y), 5); // the left foot
    strip((across, -h.y), (h.x, -h.y), 5); // the right foot
    // The doorway: up its left side, over the top, down its right side.
    strip((-across, -h.y), left, 6);
    for pair in curve.windows(2) {
        strip(pair[0], pair[1], 6);
    }
    strip(right, (across, -h.y), 6);
    polygons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Bounds, Mat3};

    /// One of each shape, at sizes that aren't all the same.
    fn samples() -> Vec<Shape> {
        vec![
            Shape::Box {
                size: [1.0, 2.0, 0.5],
            },
            Shape::Cylinder {
                radius: 0.5,
                height: 2.0,
                segments: 16,
            },
            Shape::Cylinder {
                radius: 0.3,
                height: 1.0,
                segments: 3,
            },
            Shape::Sphere {
                radius: 0.75,
                segments: 12,
            },
            Shape::Sphere {
                radius: 1.0,
                segments: 4,
            },
            Shape::Sphere {
                radius: 1.0,
                segments: 7,
            },
            Shape::Cone {
                radius: 0.5,
                height: 1.5,
                segments: 16,
            },
            Shape::Cone {
                radius: 0.5,
                height: 1.0,
                segments: 4,
            },
            Shape::Wedge {
                size: [2.0, 1.0, 3.0],
            },
        ]
    }

    fn triangle_count(polygons: &[Polygon]) -> usize {
        polygons.iter().map(Polygon::triangle_count).sum()
    }

    fn bounds(polygons: &[Polygon]) -> Bounds {
        let tidy = polygons
            .iter()
            .flat_map(|polygon| &polygon.corners)
            .map(|corner| corner.tidied());
        Bounds::around(tidy).unwrap()
    }

    /// Duck Forge's own test for its meshes: every triangle's front
    /// faces away from the inside.
    fn assert_faces_outwards(polygons: &[Polygon], what: &str) {
        let corners: Vec<Vec3> = polygons.iter().flat_map(|p| p.corners.clone()).collect();
        let inside =
            corners.iter().fold(Vec3::ZERO, |sum, &c| sum + c) * (1.0 / corners.len() as f64);
        for polygon in polygons {
            for [a, b, c] in polygon.triangles() {
                let normal = (b - a).cross(c - a);
                let middle = (a + b + c) * (1.0 / 3.0);
                assert!(
                    normal.dot(middle - inside) > 0.0,
                    "{what}: a triangle faces inwards"
                );
            }
        }
    }

    #[test]
    fn every_shape_faces_outwards() {
        for shape in samples() {
            assert_faces_outwards(&build(&shape), &format!("{shape:?}"));
        }
    }

    #[test]
    fn every_face_is_flat() {
        for shape in samples() {
            for polygon in build(&shape) {
                let normal = polygon.normal().expect("a face has area");
                for corner in &polygon.corners {
                    let off = normal.dot(*corner - polygon.corners[0]);
                    assert!(off.abs() < 1e-12, "{shape:?}: a face isn't flat");
                }
            }
        }
    }

    #[test]
    fn every_shape_is_closed() {
        // Each edge is used by exactly two faces, in opposite directions:
        // no holes and no loose faces.
        for shape in samples() {
            let key = |p: Vec3| {
                let p = p.tidied();
                [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
            };
            let mut edges = std::collections::BTreeMap::new();
            for polygon in build(&shape) {
                for (at, &corner) in polygon.corners.iter().enumerate() {
                    let next = polygon.corners[(at + 1) % polygon.corners.len()];
                    *edges.entry((key(corner), key(next))).or_insert(0) += 1;
                }
            }
            for ((from, to), count) in &edges {
                assert_eq!(*count, 1, "{shape:?}: an edge is used twice the same way");
                assert_eq!(
                    edges.get(&(*to, *from)),
                    Some(&1),
                    "{shape:?}: an edge has no partner"
                );
            }
        }
    }

    #[test]
    fn triangle_counts_are_the_documented_ones() {
        let count = |shape: Shape| triangle_count(&build(&shape));
        assert_eq!(count(Shape::Box { size: [1.0; 3] }), 12);
        assert_eq!(count(Shape::Wedge { size: [1.0; 3] }), 8);
        for segments in [3, 4, 8, 16, 64] {
            // A cylinder: 4 x segments - 4. A cone: 2 x segments - 2.
            assert_eq!(
                count(Shape::Cylinder {
                    radius: 0.5,
                    height: 1.0,
                    segments
                }),
                4 * segments - 4
            );
            assert_eq!(
                count(Shape::Cone {
                    radius: 0.5,
                    height: 1.0,
                    segments
                }),
                2 * segments - 2
            );
        }
        for segments in [4, 8, 12, 16, 64] {
            // A sphere with an even count: segments x (segments - 2).
            assert_eq!(
                count(Shape::Sphere {
                    radius: 0.5,
                    segments
                }),
                segments * (segments - 2)
            );
        }
        // Duck Forge's sphere of 12 has the same 120.
        assert_eq!(
            count(Shape::Sphere {
                radius: 0.5,
                segments: 12
            }),
            120
        );
    }

    #[test]
    fn shapes_are_centred_and_the_size_they_say() {
        let size = |shape: Shape| {
            let bounds = bounds(&build(&shape));
            assert_eq!(bounds.center(), Vec3::ZERO, "{shape:?} is centred");
            bounds.size()
        };
        assert_eq!(
            size(Shape::Box {
                size: [1.0, 2.0, 0.5]
            }),
            vec3(1.0, 2.0, 0.5)
        );
        assert_eq!(
            size(Shape::Wedge {
                size: [2.0, 1.0, 3.0]
            }),
            vec3(2.0, 1.0, 3.0)
        );
        // A radius of 0.5 is exactly 1 wide, when the sides come in fours.
        for segments in [4, 8, 16, 32] {
            assert_eq!(
                size(Shape::Cylinder {
                    radius: 0.5,
                    height: 2.0,
                    segments
                }),
                vec3(1.0, 2.0, 1.0)
            );
            assert_eq!(
                size(Shape::Cone {
                    radius: 0.5,
                    height: 2.0,
                    segments
                }),
                vec3(1.0, 2.0, 1.0)
            );
            assert_eq!(
                size(Shape::Sphere {
                    radius: 0.5,
                    segments
                }),
                vec3(1.0, 1.0, 1.0)
            );
        }
        // The default sphere too.
        assert_eq!(
            size(Shape::Sphere {
                radius: 0.5,
                segments: 12
            }),
            vec3(1.0, 1.0, 1.0)
        );
    }

    #[test]
    fn a_four_sided_cone_is_a_square_pyramid() {
        // Like Duck Forge's pyramid: a square base, straight to x and z.
        let polygons = build(&Shape::Cone {
            radius: 0.5,
            height: 1.0,
            segments: 4,
        });
        let base = polygons.iter().find(|polygon| polygon.group == 1).unwrap();
        let mut corners: Vec<(f64, f64)> = base
            .corners
            .iter()
            .map(|c| (tidy_number(c.x), tidy_number(c.z)))
            .collect();
        corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            corners,
            [(-0.5, -0.5), (-0.5, 0.5), (0.5, -0.5), (0.5, 0.5)]
        );
    }

    fn tidy_number(value: f64) -> f64 {
        crate::math::tidy(value)
    }

    #[test]
    fn a_cylinder_has_a_flat_side_facing_each_way() {
        let polygons = build(&Shape::Cylinder {
            radius: 0.5,
            height: 1.0,
            segments: 16,
        });
        for facing in [
            vec3(1.0, 0.0, 0.0),
            vec3(-1.0, 0.0, 0.0),
            Vec3::FORWARD,
            -Vec3::FORWARD,
        ] {
            assert!(
                polygons
                    .iter()
                    .any(|polygon| (polygon.normal().unwrap() - facing).length() < 1e-9),
                "no flat side faces {facing:?}"
            );
        }
    }

    #[test]
    fn faces_are_grouped_by_name() {
        let boxed = Shape::Box {
            size: [1.0, 2.0, 3.0],
        };
        let polygons = build(&boxed);
        // Each named face points the way its name says. Front is +z.
        let expected = [
            ("top", Vec3::UP),
            ("bottom", -Vec3::UP),
            ("left", vec3(-1.0, 0.0, 0.0)),
            ("right", vec3(1.0, 0.0, 0.0)),
            ("front", Vec3::FORWARD),
            ("back", -Vec3::FORWARD),
        ];
        for (name, facing) in expected {
            let groups = face_groups(&boxed, &Trim::default(), name).unwrap();
            assert_eq!(groups.len(), 1);
            let polygon = polygons
                .iter()
                .find(|polygon| polygon.group == groups[0])
                .unwrap();
            assert_eq!(polygon.normal().unwrap(), facing, "{name}");
        }
        assert_eq!(
            face_groups(&boxed, &Trim::default(), "sides"),
            Some(vec![2, 3, 4, 5])
        );
        assert_eq!(face_groups(&boxed, &Trim::default(), "lid"), None);
        assert_eq!(face_words(&boxed, &Trim::default()).last(), Some(&"sides"));

        let wedge = Shape::Wedge { size: [1.0; 3] };
        let polygons = build(&wedge);
        let slope = &polygons[face_groups(&wedge, &Trim::default(), "slope").unwrap()[0]];
        // The slope faces up and to the front.
        let normal = slope.normal().unwrap();
        assert!(normal.y > 0.0 && normal.z > 0.0 && normal.x == 0.0);
        // Every face of every shape has a name.
        for shape in samples() {
            let names = face_names(&shape);
            assert!(
                build(&shape)
                    .iter()
                    .all(|polygon| polygon.group < names.len())
            );
            assert!(
                face_groups(&shape, &Trim::default(), "sides").is_none()
                    || matches!(shape, Shape::Box { .. })
            );
        }
    }

    #[test]
    fn moved_turned_scaled_and_mirrored_shapes_still_face_outwards() {
        let transforms = [
            // move
            Transform::moving(vec3(3.0, -1.0, 2.0)),
            // rotate
            Transform {
                turn: Mat3::from_rotate([30.0, 45.0, 60.0]),
                shift: Vec3::ZERO,
            },
            // scale, unevenly
            Transform {
                turn: Mat3::stretch(vec3(2.0, 0.5, 3.0)),
                shift: Vec3::ZERO,
            },
            // mirror, across each direction
            Transform {
                turn: Mat3::stretch(vec3(-1.0, 1.0, 1.0)),
                shift: Vec3::ZERO,
            },
            Transform {
                turn: Mat3::stretch(vec3(1.0, -1.0, 1.0)),
                shift: Vec3::ZERO,
            },
            Transform {
                turn: Mat3::stretch(vec3(1.0, 1.0, -1.0)),
                shift: Vec3::ZERO,
            },
            // all of them
            Transform {
                turn: Mat3::from_rotate([10.0, 20.0, 30.0]) * Mat3::stretch(vec3(-2.0, 1.0, 0.5)),
                shift: vec3(1.0, 2.0, 3.0),
            },
        ];
        for shape in samples() {
            for transform in &transforms {
                let moved: Vec<Polygon> = build(&shape)
                    .iter()
                    .map(|p| p.transformed(transform))
                    .collect();
                assert_faces_outwards(&moved, &format!("{shape:?} after {transform:?}"));
                assert_eq!(triangle_count(&moved), triangle_count(&build(&shape)));
            }
        }
    }

    #[test]
    fn a_mirrored_wedge_slopes_the_other_way() {
        let wedge = Shape::Wedge {
            size: [1.0, 1.0, 1.0],
        };
        let flip = Transform {
            turn: Mat3::stretch(vec3(1.0, 1.0, -1.0)),
            shift: Vec3::ZERO,
        };
        let flipped: Vec<Polygon> = build(&wedge).iter().map(|p| p.transformed(&flip)).collect();
        // The slope now faces up and to the back.
        let normal = flipped[0].normal().unwrap();
        assert!(normal.y > 0.0 && normal.z < 0.0);
    }

    #[test]
    fn building_twice_gives_the_same_numbers() {
        for shape in samples() {
            assert_eq!(build(&shape), build(&shape));
        }
    }

    // ---- the seven newer shapes, bevel and taper ---------------------------

    fn trim(bevel: f64, bevel_steps: usize, taper: [f64; 2]) -> Trim {
        Trim {
            bevel,
            bevel_steps,
            taper,
        }
    }

    /// The newer shapes, and the older ones with their edges cut or
    /// their top narrowed, each with the box it should fill.
    fn trimmed_samples() -> Vec<(Shape, Trim, [f64; 3])> {
        let plain = Trim::default();
        let capsule = |radius, height, segments| Shape::Capsule {
            radius,
            height,
            segments,
        };
        let torus = |radius, tube, segments| Shape::Torus {
            radius,
            tube,
            segments,
        };
        let tube = Shape::Tube {
            radius: 0.5,
            wall: 0.1,
            height: 1.0,
            segments: 16,
        };
        let stairs = |steps| Shape::Stairs {
            size: [1.0, 1.0, 2.0],
            steps,
        };
        let arch = |size, segments| Shape::Arch {
            size,
            wall: 0.2,
            segments,
        };
        let cube = Shape::Box {
            size: [1.0, 2.0, 0.5],
        };
        let can = Shape::Cylinder {
            radius: 0.5,
            height: 2.0,
            segments: 16,
        };
        vec![
            (capsule(0.25, 1.0, 16), plain, [0.5, 1.0, 0.5]),
            (capsule(0.5, 1.0, 12), plain, [1.0, 1.0, 1.0]),
            (capsule(0.25, 1.0, 4), plain, [0.5, 1.0, 0.5]),
            (torus(0.5, 0.15, 16), plain, [1.3, 0.3, 1.3]),
            (torus(1.0, 0.4, 8), plain, [2.8, 0.8, 2.8]),
            (tube, plain, [1.0, 1.0, 1.0]),
            (tube, trim(0.0, 1, [0.5, 0.5]), [1.0, 1.0, 1.0]),
            (tube, trim(0.0, 1, [0.5, 2.0]), [1.0, 1.0, 2.0]),
            (
                Shape::Pyramid {
                    size: [2.0, 1.0, 3.0],
                },
                plain,
                [2.0, 1.0, 3.0],
            ),
            (
                Shape::Prism {
                    size: [2.0, 1.0, 3.0],
                },
                plain,
                [2.0, 1.0, 3.0],
            ),
            (stairs(4), plain, [1.0, 1.0, 2.0]),
            (stairs(1), plain, [1.0, 1.0, 2.0]),
            (stairs(7), plain, [1.0, 1.0, 2.0]),
            (arch([2.0, 2.0, 0.5], 8), plain, [2.0, 2.0, 0.5]),
            (arch([2.0, 1.0, 0.5], 8), plain, [2.0, 1.0, 0.5]),
            (arch([2.0, 0.7, 0.5], 6), plain, [2.0, 0.7, 0.5]),
            (arch([1.0, 3.0, 1.0], 3), plain, [1.0, 3.0, 1.0]),
            (arch([1.0, 3.0, 1.0], 2), plain, [1.0, 3.0, 1.0]),
            (cube, trim(0.1, 1, [1.0; 2]), [1.0, 2.0, 0.5]),
            (cube, trim(0.1, 3, [1.0; 2]), [1.0, 2.0, 0.5]),
            (cube, trim(0.2, 4, [1.0; 2]), [1.0, 2.0, 0.5]),
            (cube, trim(0.0, 1, [0.5, 0.5]), [1.0, 2.0, 0.5]),
            // Cut edges and a narrowed top together: the widest place is
            // where the bevel ends, a little way up or down.
            (cube, trim(0.1, 2, [0.5, 1.5]), [0.975, 2.0, 0.7375]),
            (can, trim(0.1, 1, [1.0; 2]), [1.0, 2.0, 1.0]),
            (can, trim(0.2, 3, [1.0; 2]), [1.0, 2.0, 1.0]),
            (can, trim(0.0, 1, [0.3, 0.3]), [1.0, 2.0, 1.0]),
            (can, trim(0.1, 2, [0.5, 1.0]), [0.975, 2.0, 1.0]),
        ]
    }

    /// Six times the room inside a closed shape: above zero when its
    /// faces point outwards.
    fn room(polygons: &[Polygon]) -> f64 {
        polygons
            .iter()
            .flat_map(Polygon::triangles)
            .map(|[a, b, c]| a.dot(b.cross(c)))
            .sum()
    }

    #[test]
    fn the_newer_shapes_are_closed_flat_and_face_outwards() {
        // Which sign "outwards" has, from a shape known to be right.
        let outwards = room(&build(&Shape::Box { size: [1.0; 3] })).signum();
        for (shape, trim, _) in trimmed_samples() {
            let what = format!("{shape:?} {trim:?}");
            let polygons = build_trimmed(&shape, &trim);
            // Closed: each edge is used by two faces, in opposite
            // directions.
            let key = |p: Vec3| {
                let p = p.tidied();
                [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
            };
            let mut edges = std::collections::BTreeMap::new();
            for polygon in &polygons {
                for (at, &corner) in polygon.corners.iter().enumerate() {
                    let next = polygon.corners[(at + 1) % polygon.corners.len()];
                    assert_ne!(key(corner), key(next), "{what}: an edge with no length");
                    *edges.entry((key(corner), key(next))).or_insert(0) += 1;
                }
            }
            for ((from, to), count) in &edges {
                assert_eq!(*count, 1, "{what}: an edge is used twice the same way");
                assert_eq!(
                    edges.get(&(*to, *from)),
                    Some(&1),
                    "{what}: an edge has no partner"
                );
            }
            // Closed and all the same way round, so one number says
            // which way.
            assert!(room(&polygons) * outwards > 0.0, "{what}: faces inwards");
            for polygon in &polygons {
                // Flat.
                let normal = polygon.normal().expect("a face has area");
                for corner in &polygon.corners {
                    let off = normal.dot(*corner - polygon.corners[0]);
                    assert!(off.abs() < 1e-9, "{what}: a face isn't flat: {off}");
                }
                // Bulging nowhere inwards, which sockets count on, and
                // cut into triangles that all have area.
                let count = polygon.corners.len();
                let turn = |at: usize| {
                    let (a, b, c) = (
                        polygon.corners[at],
                        polygon.corners[(at + 1) % count],
                        polygon.corners[(at + 2) % count],
                    );
                    (b - a).cross(c - b).dot(normal)
                };
                let first = (0..count)
                    .map(turn)
                    .fold(0.0, |sum, turn| sum + turn)
                    .signum();
                for at in 0..count {
                    assert!(turn(at) * first > -1e-12, "{what}: a face bulges inwards");
                }
                for [a, b, c] in polygon.triangles() {
                    assert!(
                        (b - a).cross(c - a).length() > 1e-12,
                        "{what}: a triangle with no area"
                    );
                }
            }
        }
    }

    #[test]
    fn the_newer_shapes_are_centred_and_the_size_they_say() {
        for (shape, trim, size) in trimmed_samples() {
            let around = bounds(&build_trimmed(&shape, &trim));
            let (got, middle) = (around.size(), around.center());
            for (got, want) in [got.x, got.y, got.z].into_iter().zip(size) {
                assert!(
                    (got - want).abs() < 1e-9,
                    "{shape:?} {trim:?}: {got} for {want}"
                );
            }
            assert!(middle.length() < 1e-9, "{shape:?} {trim:?}: {middle:?}");
        }
    }

    #[test]
    fn the_newer_shapes_hold_as_much_as_they_should() {
        let volume = |shape: Shape| room(&build(&shape)).abs() / 6.0;
        let size = [2.0, 1.0, 3.0];
        assert!((volume(Shape::Pyramid { size }) - 2.0).abs() < 1e-9);
        assert!((volume(Shape::Prism { size }) - 3.0).abs() < 1e-9);
        // Four steps: a quarter, a half, three quarters and all of the
        // height, each over a quarter of the depth.
        assert!((volume(Shape::Stairs { size, steps: 4 }) - 6.0 * 2.5 / 4.0).abs() < 1e-9);
        // A square tube is a box with a box taken out.
        let square = Shape::Tube {
            radius: 0.5,
            wall: 0.1,
            height: 2.0,
            segments: 4,
        };
        assert!((volume(square) - (1.0 - 0.64) * 2.0).abs() < 1e-9);
        // An arch with a two-piece top is a wall with a pointed doorway.
        let pointed = Shape::Arch {
            size: [2.0, 2.0, 0.5],
            wall: 0.2,
            segments: 2,
        };
        // The doorway: 1.6 wide, 1.0 straight, and a point 0.8 higher.
        let doorway = 1.6 * 1.0 + 1.6 * 0.8 / 2.0;
        assert!((volume(pointed) - (4.0 - doorway) * 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_newer_faces_are_grouped_by_name() {
        for (shape, trim, _) in trimmed_samples() {
            let polygons = build_trimmed(&shape, &trim);
            let words = face_words(&shape, &trim);
            // Every name a recipe can use stands for at least one face.
            // (But a capsule no taller than it is wide is all ends.)
            let all_ends =
                matches!(shape, Shape::Capsule { radius, height, .. } if height <= radius * 2.0);
            for word in &words {
                if all_ends && *word == "side" {
                    continue;
                }
                let groups = face_groups(&shape, &trim, word).unwrap();
                assert!(
                    polygons
                        .iter()
                        .any(|polygon| groups.contains(&polygon.group)),
                    "{shape:?} {trim:?}: no face is called {word}"
                );
            }
            // And every face has a name.
            for polygon in &polygons {
                let name = group_name(&shape, polygon.group);
                assert!(words.contains(&name), "{shape:?} {trim:?}: {name}");
            }
            assert_eq!(
                words.contains(&BEVEL),
                trim.is_bevelled(),
                "{shape:?} {trim:?}"
            );
        }
        // What points which way.
        let facing = |shape: Shape, name: &str| -> Vec3 {
            let groups = face_groups(&shape, &Trim::default(), name).unwrap();
            build(&shape)
                .iter()
                .filter(|polygon| groups.contains(&polygon.group))
                .filter_map(Polygon::normal)
                .fold(Vec3::ZERO, |sum, normal| sum + normal)
                .normalized()
                .unwrap()
        };
        let size = [2.0, 1.0, 3.0];
        let roof = Shape::Prism { size };
        assert!(facing(roof, "left").x < -0.4 && facing(roof, "left").y > 0.4);
        assert!(facing(roof, "right").x > 0.4 && facing(roof, "right").y > 0.4);
        assert!((facing(roof, "front") - vec3(0.0, 0.0, 1.0)).length() < 1e-9);
        let steps = Shape::Stairs { size, steps: 3 };
        assert!((facing(steps, "top") - vec3(0.0, 1.0, 0.0)).length() < 1e-9);
        assert!((facing(steps, "front") - vec3(0.0, 0.0, 1.0)).length() < 1e-9);
        assert!((facing(steps, "left") - vec3(-1.0, 0.0, 0.0)).length() < 1e-9);
        let gate = Shape::Arch {
            size,
            wall: 0.2,
            segments: 8,
        };
        assert!((facing(gate, "back") - vec3(0.0, 0.0, -1.0)).length() < 1e-9);
        assert!((facing(gate, "top") - vec3(0.0, 1.0, 0.0)).length() < 1e-9);
        // The doorway's faces look into the doorway: down, on the whole.
        assert!(facing(gate, "inside").y < -0.9);
        let pipe = Shape::Tube {
            radius: 0.5,
            wall: 0.1,
            height: 1.0,
            segments: 16,
        };
        let inside: Vec<Polygon> = build(&pipe)
            .into_iter()
            .filter(|polygon| polygon.group == 1)
            .collect();
        for polygon in inside {
            let middle = polygon
                .corners
                .iter()
                .fold(Vec3::ZERO, |sum, &corner| sum + corner)
                * 0.25;
            assert!(polygon.normal().unwrap().dot(middle) < 0.0);
        }
    }

    #[test]
    fn the_newer_triangle_counts_are_the_documented_ones() {
        let count = |shape: Shape, trim: Trim| triangle_count(&build_trimmed(&shape, &trim));
        let plain = Trim::default();
        let size = [1.0; 3];
        assert_eq!(count(Shape::Pyramid { size }, plain), 6);
        assert_eq!(count(Shape::Prism { size }, plain), 8);
        // Stairs: 12 x steps. (One step is a box.)
        for steps in [1, 4, 9] {
            assert_eq!(count(Shape::Stairs { size, steps }, plain), 12 * steps);
        }
        for segments in [4, 8, 16] {
            let round = |radius, height| Shape::Capsule {
                radius,
                height,
                segments,
            };
            let rings = (segments / 4).max(2);
            // A capsule: its side, and two ends of `rings` rings each.
            assert_eq!(
                count(round(0.25, 1.0), plain),
                2 * segments + 2 * (2 * segments * (rings - 1) + segments)
            );
            let around = (segments / 2).max(4).div_ceil(4) * 4;
            assert_eq!(
                count(
                    Shape::Torus {
                        radius: 0.5,
                        tube: 0.15,
                        segments
                    },
                    plain
                ),
                2 * segments * around
            );
            assert_eq!(
                count(
                    Shape::Tube {
                        radius: 0.5,
                        wall: 0.1,
                        height: 1.0,
                        segments
                    },
                    plain
                ),
                8 * segments
            );
            // An arch: 4 x segments + 24.
            assert_eq!(
                count(
                    Shape::Arch {
                        size: [2.0, 2.0, 0.5],
                        wall: 0.2,
                        segments
                    },
                    plain
                ),
                4 * segments + 24
            );
        }
        // A bevelled box: 12 for its faces, 24 x steps along its edges,
        // 8 x steps x steps at its corners.
        for steps in 1..=4 {
            assert_eq!(
                count(Shape::Box { size }, trim(0.1, steps, [1.0; 2])),
                12 + 24 * steps + 8 * steps * steps
            );
        }
    }
}
