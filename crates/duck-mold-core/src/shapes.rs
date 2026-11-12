//! The five built-in shapes: box, cylinder, sphere, cone and wedge.
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
use crate::recipe::Shape;

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
}

/// The names of a shape's groups of faces, in the order [`build`] numbers
/// them.
pub fn face_names(shape: &Shape) -> &'static [&'static str] {
    match shape {
        Shape::Box { .. } => &["top", "bottom", "left", "right", "front", "back"],
        Shape::Cylinder { .. } => &["top", "bottom", "side"],
        Shape::Sphere { .. } => &["side"],
        Shape::Cone { .. } => &["side", "bottom"],
        Shape::Wedge { .. } => &["slope", "bottom", "back", "left", "right"],
    }
}

/// The groups a face name stands for. Usually one; a box's `sides` is
/// short for its left, right, front and back.
pub fn face_groups(shape: &Shape, name: &str) -> Option<Vec<usize>> {
    if matches!(shape, Shape::Box { .. }) && name == "sides" {
        return Some(vec![2, 3, 4, 5]);
    }
    face_names(shape)
        .iter()
        .position(|face| *face == name)
        .map(|group| vec![group])
}

/// Every face name a recipe can use for this shape, for messages.
pub fn face_words(shape: &Shape) -> Vec<&'static str> {
    let mut words = face_names(shape).to_vec();
    if matches!(shape, Shape::Box { .. }) {
        words.push("sides");
    }
    words
}

/// Builds a shape's faces.
pub fn build(shape: &Shape) -> Vec<Polygon> {
    let mut polygons = match *shape {
        Shape::Box { size } => box_faces(size.into()),
        Shape::Cylinder {
            radius,
            height,
            segments,
        } => cylinder_faces(radius, height, segments),
        Shape::Sphere { radius, segments } => sphere_faces(radius, segments),
        Shape::Cone {
            radius,
            height,
            segments,
        } => cone_faces(radius, height, segments),
        Shape::Wedge { size } => wedge_faces(size.into()),
    };
    face_outwards(&mut polygons);
    polygons
}

/// Makes every face of a solid shape point away from its inside, by
/// reversing the corners of any that doesn't. All five shapes bulge
/// outwards everywhere, so "away from the middle of the corners" is
/// always "outwards".
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
            let groups = face_groups(&boxed, name).unwrap();
            assert_eq!(groups.len(), 1);
            let polygon = polygons
                .iter()
                .find(|polygon| polygon.group == groups[0])
                .unwrap();
            assert_eq!(polygon.normal().unwrap(), facing, "{name}");
        }
        assert_eq!(face_groups(&boxed, "sides"), Some(vec![2, 3, 4, 5]));
        assert_eq!(face_groups(&boxed, "lid"), None);
        assert_eq!(face_words(&boxed).last(), Some(&"sides"));

        let wedge = Shape::Wedge { size: [1.0; 3] };
        let polygons = build(&wedge);
        let slope = &polygons[face_groups(&wedge, "slope").unwrap()[0]];
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
            assert!(face_groups(&shape, "sides").is_none() || matches!(shape, Shape::Box { .. }));
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
}
