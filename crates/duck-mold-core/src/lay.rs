//! Laying an image on a shape: where each corner of each face is on
//! the picture.
//!
//! One rule for flat faces: **the image is fitted to the rectangle round
//! the faces it was given to, as you see them looking straight at them
//! from outside.** Faces that face the same way share one copy; faces
//! that face different ways get a copy each. For a box that is the paper
//! cross a cardboard box unfolds into: upright on the four sides, the
//! top as seen from above with the front edge at the bottom, the bottom
//! as seen from below with the front edge at the top.
//!
//! Round faces are wrapped instead: once round a cylinder like the label
//! on a tin (the middle of the image at the front, its two ends meeting
//! at the back), round a ball like a map of the world.
//!
//! Everything is worked out from the faces' own corners, in the part's
//! own space, with arithmetic that gives the same answer on every
//! computer ([`crate::math::turn_of`] for angles).

use std::collections::BTreeMap;

use crate::math::{Vec3, turn_of, vec3};
use crate::recipe::{Shape, Side, Trim};
use crate::shapes::Polygon;

/// How an image lies on a group of faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lie {
    /// Flat, seen straight on from this side of the part, whichever way
    /// the face itself has been tilted since: a box's faces, and every
    /// face a change made from them.
    From(Side),
    /// Flat, seen straight on from the way the face itself faces: a
    /// wedge's slope, a pyramid's sides.
    Flat,
    /// Wrapped once round the upright line through the middle: the side
    /// of a cylinder, a cone, a capsule, a tube.
    Round,
    /// Wrapped like a map of the world: a sphere.
    Ball,
    /// Once round the ring and once round its tube: a torus.
    Ring,
    /// Along the inside of an arch's doorway, from one foot over the
    /// top to the other.
    Tunnel,
    /// By the file's own layout: a mesh from a file. Not laid here.
    Own,
    /// These faces never take an image: bevelled edges.
    Never,
}

/// How an image lies on each group of a shape's faces, in the order
/// [`crate::shapes::face_names`] names them, a bevel's faces last.
pub fn lies_of(shape: &Shape, trim: &Trim) -> Vec<Lie> {
    use Lie::{Ball, Flat, From, Never, Ring, Round, Tunnel};
    let mut lies = match shape {
        Shape::Box { .. } => vec![
            From(Side::Top),
            From(Side::Bottom),
            From(Side::Left),
            From(Side::Right),
            From(Side::Front),
            From(Side::Back),
        ],
        Shape::Cylinder { .. } => vec![From(Side::Top), From(Side::Bottom), Round],
        Shape::Sphere { .. } => vec![Ball],
        Shape::Torus { .. } => vec![Ring],
        Shape::Cone { .. } => vec![Round, From(Side::Bottom)],
        Shape::Wedge { .. } => vec![Flat; 5],
        Shape::Capsule { .. } => vec![Round, From(Side::Top), From(Side::Bottom)],
        Shape::Tube { .. } => vec![Round, Round, From(Side::Top), From(Side::Bottom)],
        Shape::Pyramid { .. } | Shape::Prism { .. } => vec![Flat; 5],
        Shape::Stairs { .. } => vec![Flat; 6],
        Shape::Arch { .. } => vec![Flat, Flat, Flat, Flat, Flat, Flat, Tunnel],
    };
    if trim.is_bevelled() && matches!(shape, Shape::Box { .. } | Shape::Cylinder { .. }) {
        lies.push(Never);
    }
    lies
}

/// Which faces lie together: one copy of the image for each of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Together {
    /// Everything seen from this side.
    From(u8),
    /// Everything that faces this way (its direction, to a millionth).
    Facing([i64; 3]),
    /// A group of round faces: which kind (1 round, 2 a ball, 3 a ring,
    /// 4 a tunnel), and the group.
    Curved(u8, usize),
}

/// The two directions of a flat image: which way is up on it, and which
/// way is to the right, for someone looking at the face from outside.
fn frame_from(side: Side) -> (Vec3, Vec3) {
    let (out, up) = match side {
        // Seen from above, the front edge at the bottom of the image.
        Side::Top => (vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, -1.0)),
        // Seen from below, the front edge at the top.
        Side::Bottom => (vec3(0.0, -1.0, 0.0), vec3(0.0, 0.0, 1.0)),
        Side::Left => (vec3(-1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)),
        Side::Right => (vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)),
        Side::Back => (vec3(0.0, 0.0, -1.0), vec3(0.0, 1.0, 0.0)),
        Side::Front | Side::Center => (vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0)),
    };
    (up, out.cross(up))
}

/// The same for a face that faces `out`: up is up, as far as the face
/// allows; a face that faces straight up or down is as a box's.
fn frame_facing(out: Vec3) -> (Vec3, Vec3) {
    if out.y > 0.999_999 {
        return frame_from(Side::Top);
    }
    if out.y < -0.999_999 {
        return frame_from(Side::Bottom);
    }
    let up = vec3(0.0, 1.0, 0.0);
    let up = (up - out * up.dot(out)).normalized().unwrap_or(up);
    (up, out.cross(up))
}

/// A place on an image, tidied: a face that should end exactly on the
/// image's edge does.
fn tidy(value: f64) -> f64 {
    (value * 1e9).round() / 1e9 + 0.0
}

/// How far `value` is from `low` to `high`, 0 to 1; a half where the two
/// are the same.
fn share(value: f64, low: f64, high: f64) -> f64 {
    if high - low > 1e-12 {
        (value - low) / (high - low)
    } else {
        0.5
    }
}

/// How far round the upright line a point is, as a place across an
/// image wrapped round it: 0.5 at the front, 0.25 on the model's right,
/// 0.75 on its left, 0 (and 1) at the back.
fn across_round(x: f64, z: f64) -> f64 {
    let across = tidy(0.5 - turn_of(x, z));
    if across >= 1.0 { 0.0 } else { across }
}

/// `value`, a place on something that comes round again every 1, moved
/// by whole turns to be within a half of `near`.
fn beside(mut value: f64, near: f64) -> f64 {
    while value - near > 0.5 {
        value -= 1.0;
    }
    while near - value > 0.5 {
        value += 1.0;
    }
    value
}

/// Works out where a face's corners are on an image.
type Placer<'a> = Box<dyn Fn(&Polygon) -> Vec<[f64; 2]> + 'a>;

/// Lays one image on some faces of a part.
///
/// `faces` are the part's faces in its own space and `lies` how an image
/// lies on each. `shown` says which faces show the image. `fits` says
/// which faces the image is fitted to: the rectangle is the one round
/// those (all the faces a name stands for; all the part's faces for the
/// part's own image), so the image stays put when one of them is given
/// another image.
///
/// For each face, where its corners are on the image; `None` for a face
/// that doesn't show it.
pub fn lay(
    faces: &[Polygon],
    lies: &[Lie],
    shown: &[bool],
    fits: &[bool],
) -> Vec<Option<Vec<[f64; 2]>>> {
    let together: Vec<Option<Together>> = faces
        .iter()
        .zip(lies)
        .map(|(face, lie)| match lie {
            Lie::From(side) => Some(Together::From(*side as u8)),
            Lie::Flat => face.normal().map(|out| {
                Together::Facing([
                    (out.x * 1e6).round() as i64,
                    (out.y * 1e6).round() as i64,
                    (out.z * 1e6).round() as i64,
                ])
            }),
            Lie::Round => Some(Together::Curved(1, face.group)),
            Lie::Ball => Some(Together::Curved(2, face.group)),
            Lie::Ring => Some(Together::Curved(3, face.group)),
            Lie::Tunnel => Some(Together::Curved(4, face.group)),
            Lie::Own | Lie::Never => None,
        })
        .collect();

    // What each copy is measured by: the faces it is fitted to that lie
    // this way or, if there are none, the faces that show it.
    let mut measured: BTreeMap<Together, Vec<usize>> = BTreeMap::new();
    for (at, key) in together.iter().enumerate() {
        if let Some(key) = key
            && shown[at]
        {
            measured.entry(*key).or_default();
        }
    }
    for (key, list) in &mut measured {
        let lying = |which: &[bool]| -> Vec<usize> {
            (0..faces.len())
                .filter(|&at| which[at] && together[at] == Some(*key))
                .collect()
        };
        *list = lying(fits);
        if list.is_empty() {
            *list = lying(shown);
        }
    }

    let mut laid: Vec<Option<Vec<[f64; 2]>>> = vec![None; faces.len()];
    for (key, list) in &measured {
        let corners = || {
            list.iter()
                .flat_map(|&at| faces[at].corners.iter().copied())
        };
        let range = |of: &dyn Fn(Vec3) -> f64| {
            corners()
                .map(of)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
                    (low.min(value), high.max(value))
                })
        };
        let place: Placer = match *key {
            Together::From(_) | Together::Facing(_) => {
                let (up, right) = match *key {
                    Together::From(side) => frame_from(side_of(side)),
                    _ => frame_facing(faces[list[0]].normal().unwrap_or(vec3(0.0, 0.0, 1.0))),
                };
                let (left_edge, right_edge) = range(&|corner| corner.dot(right));
                let (bottom_edge, top_edge) = range(&|corner| corner.dot(up));
                Box::new(move |face: &Polygon| {
                    face.corners
                        .iter()
                        .map(|corner| {
                            [
                                tidy(share(corner.dot(right), left_edge, right_edge)),
                                tidy(1.0 - share(corner.dot(up), bottom_edge, top_edge)),
                            ]
                        })
                        .collect()
                })
            }
            Together::Curved(1, _) => {
                let (low, high) = range(&|corner| corner.y);
                Box::new(move |face: &Polygon| {
                    round(face, &|corner| tidy(1.0 - share(corner.y, low, high)))
                })
            }
            Together::Curved(2, _) => Box::new(|face: &Polygon| {
                round(face, &|corner| {
                    let out = (corner.x * corner.x + corner.z * corner.z).sqrt();
                    tidy(2.0 * turn_of(out, corner.y))
                })
            }),
            Together::Curved(3, _) => {
                let (inner, outer) =
                    range(&|corner| (corner.x * corner.x + corner.z * corner.z).sqrt());
                let middle = (inner + outer) / 2.0;
                Box::new(move |face: &Polygon| ring(face, middle))
            }
            Together::Curved(_, _) => {
                let tunnel = Tunnel::of(corners());
                Box::new(move |face: &Polygon| tunnel.place(face))
            }
        };
        for (at, face) in faces.iter().enumerate() {
            if shown[at] && together[at] == Some(*key) {
                laid[at] = Some(place(face));
            }
        }
    }
    laid
}

/// The side a number stands for (its place in the list of sides).
fn side_of(number: u8) -> Side {
    Side::ALL
        .into_iter()
        .find(|side| *side as u8 == number)
        .unwrap_or(Side::Front)
}

/// The middle of a face's corners.
fn middle_of(face: &Polygon) -> Vec3 {
    let sum = face
        .corners
        .iter()
        .fold(Vec3::ZERO, |sum, &corner| sum + corner);
    sum * (1.0 / face.corners.len().max(1) as f64)
}

/// A face wrapped round the upright line: across by how far round each
/// corner is, down by `down`. A face that lies across the image's two
/// ends (at the back) carries on past the end rather than jumping to
/// the other one; a corner on the line itself (a cone's point) is where
/// the face's middle is.
fn round(face: &Polygon, down: &dyn Fn(Vec3) -> f64) -> Vec<[f64; 2]> {
    let middle = middle_of(face);
    let near = across_round(middle.x, middle.z);
    let reach = face
        .corners
        .iter()
        .fold(0.0f64, |reach, corner| reach.max(corner.length()));
    face.corners
        .iter()
        .map(|&corner| {
            let out = (corner.x * corner.x + corner.z * corner.z).sqrt();
            let across = if out <= 1e-9 * reach.max(1e-9) {
                near
            } else {
                tidy(beside(across_round(corner.x, corner.z), near))
            };
            [across, down(corner)]
        })
        .collect()
}

/// A face of a ring lying flat: across by how far round the ring, down
/// by how far round its tube (the outside of the ring at the middle of
/// the image, its top a quarter of the way down).
fn ring(face: &Polygon, middle_radius: f64) -> Vec<[f64; 2]> {
    let down_of = |corner: Vec3| {
        let out = (corner.x * corner.x + corner.z * corner.z).sqrt();
        let down = tidy(0.5 - turn_of(corner.y, out - middle_radius));
        if down >= 1.0 { 0.0 } else { down }
    };
    let near = down_of(middle_of(face));
    round(face, &|corner| tidy(beside(down_of(corner), near)))
}

/// The inside of an arch's doorway, measured along it.
struct Tunnel {
    /// How far along the doorway each of its corners is, 0 to 1, by the
    /// corner's place across and up (in millionths).
    along: BTreeMap<(i64, i64), f64>,
    /// The front and the back.
    front: f64,
    back: f64,
}

impl Tunnel {
    fn key(corner: Vec3) -> (i64, i64) {
        (
            (corner.x * 1e6).round() as i64,
            (corner.y * 1e6).round() as i64,
        )
    }

    fn of(corners: impl Iterator<Item = Vec3>) -> Tunnel {
        let corners: Vec<Vec3> = corners.collect();
        let floor = corners.iter().fold(f64::INFINITY, |low, c| low.min(c.y));
        let front = corners
            .iter()
            .fold(f64::NEG_INFINITY, |high, c| high.max(c.z));
        let back = corners.iter().fold(f64::INFINITY, |low, c| low.min(c.z));
        // The doorway's outline, each corner once, in order round it:
        // from the foot on the model's right, over the top, to the foot
        // on its left.
        let mut outline: Vec<((i64, i64), f64, f64, f64)> = Vec::new();
        for corner in &corners {
            let key = Tunnel::key(*corner);
            if !outline.iter().any(|(known, ..)| *known == key) {
                outline.push((key, turn_of(corner.y - floor, corner.x), corner.x, corner.y));
            }
        }
        outline.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let mut along = BTreeMap::new();
        let mut so_far = 0.0;
        let mut lengths = Vec::with_capacity(outline.len());
        for (at, point) in outline.iter().enumerate() {
            if at > 0 {
                let before = outline[at - 1];
                so_far += ((point.2 - before.2).powi(2) + (point.3 - before.3).powi(2)).sqrt();
            }
            lengths.push(so_far);
        }
        for (point, length) in outline.iter().zip(lengths) {
            along.insert(point.0, tidy(share(length, 0.0, so_far)));
        }
        Tunnel { along, front, back }
    }

    fn place(&self, face: &Polygon) -> Vec<[f64; 2]> {
        face.corners
            .iter()
            .map(|&corner| {
                [
                    self.along.get(&Tunnel::key(corner)).copied().unwrap_or(0.5),
                    tidy(1.0 - share(corner.z, self.back, self.front)),
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    /// For each face, where its corners are on the image.
    type Laid = Vec<Option<Vec<[f64; 2]>>>;

    fn laid_whole(shape: &Shape) -> (Vec<Polygon>, Laid) {
        let trim = Trim::default();
        let faces = shapes::build_trimmed(shape, &trim);
        let by_group = lies_of(shape, &trim);
        let lies: Vec<Lie> = faces.iter().map(|face| by_group[face.group]).collect();
        let all = vec![true; faces.len()];
        let laid = lay(&faces, &lies, &all, &all);
        (faces, laid)
    }

    /// Where on the image the corner of a face nearest to `point` is.
    fn at(face: &Polygon, laid: &[[f64; 2]], point: Vec3) -> [f64; 2] {
        let nearest = face
            .corners
            .iter()
            .enumerate()
            .min_by(|a, b| (*a.1 - point).length().total_cmp(&(*b.1 - point).length()))
            .unwrap()
            .0;
        laid[nearest]
    }

    #[test]
    fn a_boxs_faces_are_the_paper_cross() {
        let (faces, laid) = laid_whole(&Shape::Box {
            size: [2.0, 1.0, 4.0],
        });
        let face = |group: usize| {
            let at = faces.iter().position(|face| face.group == group).unwrap();
            (&faces[at], laid[at].as_ref().unwrap())
        };
        // The front, seen from the front: the model's right (+x) is on
        // the viewer's left, and up is up.
        let (front, on) = face(4);
        assert_eq!(at(front, on, vec3(1.0, 0.5, 2.0)), [0.0, 0.0]);
        assert_eq!(at(front, on, vec3(-1.0, 0.5, 2.0)), [1.0, 0.0]);
        assert_eq!(at(front, on, vec3(1.0, -0.5, 2.0)), [0.0, 1.0]);
        // The back, seen from behind: the model's left is on the left.
        let (back, on) = face(5);
        assert_eq!(at(back, on, vec3(-1.0, 0.5, -2.0)), [0.0, 0.0]);
        assert_eq!(at(back, on, vec3(1.0, -0.5, -2.0)), [1.0, 1.0]);
        // The model's left side (-x), seen from there: the front is on
        // the viewer's left.
        let (left, on) = face(2);
        assert_eq!(at(left, on, vec3(-1.0, 0.5, 2.0)), [0.0, 0.0]);
        assert_eq!(at(left, on, vec3(-1.0, -0.5, -2.0)), [1.0, 1.0]);
        // Its right side: the back is on the viewer's left.
        let (right, on) = face(3);
        assert_eq!(at(right, on, vec3(1.0, 0.5, -2.0)), [0.0, 0.0]);
        assert_eq!(at(right, on, vec3(1.0, -0.5, 2.0)), [1.0, 1.0]);
        // The top, from above, the front edge at the bottom of the image.
        let (top, on) = face(0);
        assert_eq!(at(top, on, vec3(1.0, 0.5, -2.0)), [0.0, 0.0]);
        assert_eq!(at(top, on, vec3(-1.0, 0.5, 2.0)), [1.0, 1.0]);
        // The bottom, from below, the front edge at the top of the image.
        let (bottom, on) = face(1);
        assert_eq!(at(bottom, on, vec3(1.0, -0.5, 2.0)), [0.0, 0.0]);
        assert_eq!(at(bottom, on, vec3(-1.0, -0.5, -2.0)), [1.0, 1.0]);
    }

    #[test]
    fn a_cylinders_side_is_a_label_with_its_middle_at_the_front() {
        let shape = Shape::Cylinder {
            radius: 0.5,
            height: 2.0,
            segments: 8,
        };
        let (faces, laid) = laid_whole(&shape);
        let mut seen_front = false;
        for (face, on) in faces.iter().zip(&laid) {
            if face.group != 2 {
                continue;
            }
            let on = on.as_ref().unwrap();
            let middle = middle_of(face);
            let across: Vec<f64> = on.iter().map(|at| at[0]).collect();
            let (low, high) = across
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| {
                    (l.min(v), h.max(v))
                });
            // Each of the 8 flat sides is an eighth of the image wide.
            assert!((high - low - 0.125).abs() < 1e-9, "{low} {high}");
            // Top edge of the image at the top.
            for (corner, at) in face.corners.iter().zip(on) {
                assert_eq!(at[1], if corner.y > 0.0 { 0.0 } else { 1.0 });
            }
            if middle.z > 0.49 {
                // The side that faces front has the image's middle.
                assert!((low - 0.4375).abs() < 1e-9 && (high - 0.5625).abs() < 1e-9);
                // The model's right (+x) is on the viewer's left.
                let right = at(face, on, vec3(1.0, 1.0, 0.5));
                assert!((right[0] - 0.4375).abs() < 1e-9);
                seen_front = true;
            }
            if middle.z < -0.49 {
                // The side at the back lies across the image's two ends:
                // it carries on past one end, in one piece.
                assert!(low < 0.0 || high > 1.0, "{low} {high}");
            }
        }
        assert!(seen_front);
    }

    #[test]
    fn a_sphere_is_a_map_of_the_world() {
        let (faces, laid) = laid_whole(&Shape::Sphere {
            radius: 1.0,
            segments: 8,
        });
        for (face, on) in faces.iter().zip(&laid) {
            let on = on.as_ref().unwrap();
            for (corner, at) in face.corners.iter().zip(on) {
                // The top of the image at the top of the ball.
                let down = (1.0 - corner.y) / 2.0;
                assert!((at[1] - 0.5).abs() <= 0.5 + 1e-9);
                assert!((at[1] < 0.5) == (down < 0.5) || corner.y.abs() < 1e-9);
            }
            // No face is wider than an eighth of the image.
            let across: Vec<f64> = on.iter().map(|at| at[0]).collect();
            let wide = across.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                - across.iter().cloned().fold(f64::INFINITY, f64::min);
            assert!(wide <= 0.125 + 1e-9, "{wide}");
        }
    }

    #[test]
    fn every_shape_gives_every_face_a_place_on_the_image() {
        let shapes = [
            Shape::Box { size: [1.0; 3] },
            Shape::Cylinder {
                radius: 0.5,
                height: 1.0,
                segments: 16,
            },
            Shape::Sphere {
                radius: 0.5,
                segments: 12,
            },
            Shape::Cone {
                radius: 0.5,
                height: 1.0,
                segments: 16,
            },
            Shape::Wedge { size: [1.0; 3] },
            Shape::Capsule {
                radius: 0.25,
                height: 1.0,
                segments: 16,
            },
            Shape::Torus {
                radius: 0.5,
                tube: 0.15,
                segments: 16,
            },
            Shape::Tube {
                radius: 0.5,
                wall: 0.1,
                height: 1.0,
                segments: 16,
            },
            Shape::Pyramid { size: [1.0; 3] },
            Shape::Prism { size: [1.0; 3] },
            Shape::Stairs {
                size: [1.0; 3],
                steps: 4,
            },
            Shape::Arch {
                size: [1.0; 3],
                wall: 0.2,
                segments: 8,
            },
        ];
        for shape in shapes {
            let (faces, laid) = laid_whole(&shape);
            for (face, on) in faces.iter().zip(&laid) {
                let on = on.as_ref().unwrap_or_else(|| panic!("{}", shape.name()));
                assert_eq!(on.len(), face.corners.len());
                for at in on {
                    // On the image, or within one face of its ends.
                    assert!(
                        at[0] > -0.26 && at[0] < 1.26 && at[1] > -0.26 && at[1] < 1.26,
                        "{}: {at:?}",
                        shape.name()
                    );
                }
                // The face isn't squeezed to nothing on the image.
                let spread = on
                    .iter()
                    .map(|at| (at[0] - on[0][0]).abs() + (at[1] - on[0][1]).abs())
                    .fold(0.0, f64::max);
                assert!(spread > 1e-6, "{}: group {}", shape.name(), face.group);
            }
        }
    }

    #[test]
    fn stairs_risers_share_one_image_up_the_whole_flight() {
        let (faces, laid) = laid_whole(&Shape::Stairs {
            size: [1.0, 1.0, 1.0],
            steps: 4,
        });
        // Group 1 is `front`: the risers. Together they cover the image
        // from top to bottom, a quarter each.
        let mut downs: Vec<(f64, f64)> = Vec::new();
        for (face, on) in faces.iter().zip(&laid) {
            if face.group == 1 {
                let on = on.as_ref().unwrap();
                let low = on.iter().map(|at| at[1]).fold(f64::INFINITY, f64::min);
                let high = on.iter().map(|at| at[1]).fold(f64::NEG_INFINITY, f64::max);
                downs.push((low, high));
            }
        }
        downs.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(
            downs,
            vec![(0.0, 0.25), (0.25, 0.5), (0.5, 0.75), (0.75, 1.0)]
        );
    }

    #[test]
    fn an_image_fitted_to_some_faces_stays_put_when_fewer_show_it() {
        let faces = shapes::build(&Shape::Stairs {
            size: [1.0, 1.0, 1.0],
            steps: 4,
        });
        let lies = vec![Lie::Flat; faces.len()];
        let risers: Vec<bool> = faces.iter().map(|face| face.group == 1).collect();
        let all = lay(&faces, &lies, &risers, &risers);
        // Only the first riser shows it; it is still fitted to all four.
        let first = risers.iter().position(|riser| *riser).unwrap();
        let mut one = vec![false; faces.len()];
        one[first] = true;
        let alone = lay(&faces, &lies, &one, &risers);
        assert_eq!(alone[first], all[first]);
        assert!(
            alone
                .iter()
                .enumerate()
                .all(|(at, laid)| at == first || laid.is_none())
        );
    }
}
