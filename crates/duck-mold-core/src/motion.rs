//! Models that move: a pose turns parts about their joins.
//!
//! There are no bones. Every part turns about its pivot (the socket it
//! is joined to, or its own middle), and what hangs from it goes with
//! it: the parts joined to it, and, for a part made of parts, the parts
//! inside it. A pose says, for the parts that aren't at rest, how far
//! each is turned (degrees around x, then y, then z, in the directions
//! of the list the part is written in) and how far it is moved.
//!
//! Parts move rigidly: every part keeps its shape and size, and only
//! where it is changes.

use crate::math::{Bounds, Mat3, Transform, Vec3};
use crate::model::{Model, Part};
use crate::recipe::{Limits, Loc, Turns};

/// A pose of a model: the parts that aren't where they stand at rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    /// Its name.
    pub name: String,
    /// Where the recipe wrote it.
    pub loc: Loc,
    /// The parts it turns or moves.
    pub joints: Vec<Joint>,
}

/// One part in a pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    /// The part, by path: `left_arm/elbow`.
    pub part: String,
    /// Where the recipe wrote it.
    pub loc: Loc,
    /// Degrees around x, then y, then z, about the part's pivot, in the
    /// directions of the list the part is written in.
    pub turn: [f64; 3],
    /// How far it is moved from where it stands at rest, in metres, in
    /// the same directions.
    pub move_by: [f64; 3],
}

impl Joint {
    /// A part turned, and not moved.
    pub fn turned(part: &str, turn: [f64; 3]) -> Joint {
        Joint {
            part: part.to_owned(),
            loc: Loc::default(),
            turn,
            move_by: [0.0; 3],
        }
    }

    /// True if it says nothing: no turn and no move.
    pub fn is_rest(&self) -> bool {
        self.turn == [0.0; 3] && self.move_by == [0.0; 3]
    }
}

/// The directions a part's turns are measured in, as three directions
/// of the model, each 1 long and square to the others: the part's
/// [`axes`](Part::axes) without their stretch. (A mirror image stays a
/// mirror image.)
pub fn directions(axes: &Mat3) -> Mat3 {
    let x = axes.column(0).normalized();
    let y = axes.column(1);
    let z = axes.column(2);
    let Some(x) = x else {
        return Mat3::IDENTITY;
    };
    let Some(y) = (y - x * y.dot(x)).normalized() else {
        return Mat3::IDENTITY;
    };
    let Some(z) = (z - x * z.dot(x) - y * z.dot(y)).normalized() else {
        return Mat3::IDENTITY;
    };
    Mat3 {
        rows: [[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]],
    }
}

/// How a joint moves its part from where it stands at rest, in the
/// model, before what it hangs from moves it further: a turn about the
/// part's pivot, then a move.
pub fn own_motion(part: &Part, joint: &Joint) -> Transform {
    let directions = directions(&part.axes);
    // The turn is written in the list's directions; in the model's it is
    // the same turn seen from there. (`directions` is square, so turning
    // its rows into columns undoes it.)
    let turn = directions * Mat3::from_rotate(joint.turn) * directions.transposed();
    let moved = part.axes * Vec3::from(joint.move_by);
    Transform {
        turn,
        shift: part.pivot + moved - turn * part.pivot,
    }
}

/// Where a pose puts every part: for each part of the model, in the
/// order of [`Model::all_parts`], how to get from where a point of it is
/// at rest to where it is in the pose.
pub fn placements(model: &Model, joints: &[Joint]) -> Vec<Transform> {
    let all = model.all_parts();
    let own: Vec<Transform> = all
        .iter()
        .map(|part| {
            joints
                .iter()
                .find(|joint| joint.part == part.path)
                .map_or(Transform::IDENTITY, |joint| own_motion(part, joint))
        })
        .collect();
    let parent: Vec<Option<usize>> = all
        .iter()
        .map(|part| {
            let path = part.parent.as_deref()?;
            all.iter().position(|other| other.path == path)
        })
        .collect();
    // A part's own motion, then that of what it hangs from, and so on up
    // to the model. (No more steps than there are parts: a model can't
    // hang in a ring, but nothing here depends on that.)
    (0..all.len())
        .map(|at| {
            let mut whole = own[at];
            let mut above = parent[at];
            for _ in 0..all.len() {
                let Some(next) = above else {
                    break;
                };
                whole = whole.then(&own[next]);
                above = parent[next];
            }
            whole
        })
        .collect()
}

/// The model in a pose: every part where the pose puts it, with its
/// faces, sockets, pivot and box. Its shape and size are as they were.
pub fn posed(model: &Model, joints: &[Joint]) -> Model {
    if joints.iter().all(Joint::is_rest) {
        return model.clone();
    }
    let placed = placements(model, joints);
    let paths: Vec<String> = model
        .all_parts()
        .iter()
        .map(|part| part.path.clone())
        .collect();
    let place_of = |path: &str| {
        paths
            .iter()
            .position(|known| known == path)
            .map_or(Transform::IDENTITY, |at| placed[at])
    };
    let mut out = model.clone();
    // The faces: each belongs to one part with a shape of its own.
    for (at, part) in model.all_parts().iter().enumerate() {
        if !part.parts.is_empty() {
            continue;
        }
        for face in &mut out.faces[part.faces.clone()] {
            for corner in &mut face.corners {
                *corner = placed[at].apply(*corner).tidied();
            }
        }
    }
    // The parts themselves, in the same order.
    fn settle(
        parts: &mut [Part],
        faces: &[crate::model::Face],
        place_of: &dyn Fn(&str) -> Transform,
    ) {
        for part in parts {
            let own = place_of(&part.path);
            part.place = part.place.then(&own);
            part.pivot = own.apply(part.pivot).tidied();
            part.axes = own.turn * part.axes;
            for socket in &mut part.sockets {
                // A socket passed on from a part inside goes with that
                // part.
                let carrier = part
                    .carried
                    .iter()
                    .find(|(name, _)| *name == socket.name)
                    .map_or(own, |(_, inner)| place_of(inner));
                socket.at = carrier.apply(socket.at).tidied();
                socket.facing = carrier.facing(socket.facing).tidied();
            }
            part.bounds = Bounds::around(
                faces[part.faces.clone()]
                    .iter()
                    .flat_map(|face| face.corners.iter().copied()),
            );
            settle(&mut part.parts, faces, place_of);
        }
    }
    settle(&mut out.parts, &out.faces, &place_of);
    for socket in &mut out.sockets {
        if let Some((_, inner)) = model.carried.iter().find(|(name, _)| *name == socket.name) {
            let carrier = place_of(inner);
            socket.at = carrier.apply(socket.at).tidied();
            socket.facing = carrier.facing(socket.facing).tidied();
        }
    }
    out
}

/// One thing a pose asks of a part that its limits don't allow.
#[derive(Debug, Clone, PartialEq)]
pub struct Broken {
    /// The part.
    pub part: String,
    /// What is wrong.
    pub how: BrokenHow,
}

/// How a pose breaks a part's limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrokenHow {
    /// It is turned further around a direction (0 is x, 1 is y, 2 is z)
    /// than it may go.
    Turned {
        /// The direction.
        axis: usize,
        /// How far the pose turns it.
        degrees: f64,
        /// How far it may turn.
        allowed: Turns,
    },
    /// It is moved, and it is fixed.
    Moved,
}

impl Broken {
    /// In words, as a message's ending: `"head" 95 around y: its limit
    /// is -80 to 80`.
    pub fn describe(&self) -> String {
        let number = |degrees: f64| {
            let rounded = (degrees * 100.0).round() / 100.0 + 0.0;
            format!("{rounded}")
        };
        match self.how {
            BrokenHow::Moved => format!("moves \"{}\", which is fixed", self.part),
            BrokenHow::Turned {
                axis,
                degrees,
                allowed,
            } => {
                let axis = Limits::AXES[axis];
                let limit = match allowed {
                    Turns::Between(low, high) => {
                        format!("its limit is {} to {}", number(low), number(high))
                    }
                    // (`Any` allows everything; it is here for the match.)
                    Turns::No | Turns::Any => format!("it doesn't turn around {axis}"),
                };
                format!(
                    "turns \"{}\" {} around {axis}: {limit}",
                    self.part,
                    number(degrees)
                )
            }
        }
    }
}

/// What a pose asks of the model's parts that their limits don't allow.
pub fn limits_broken(model: &Model, joints: &[Joint]) -> Vec<Broken> {
    let mut broken = Vec::new();
    for joint in joints {
        let Some(part) = model.part(&joint.part) else {
            continue;
        };
        for (axis, allowed) in part.limits.turns().into_iter().enumerate() {
            // (A hundredth of a degree over is rounding, not a mistake.)
            if allowed.past(joint.turn[axis]) > 0.005 {
                broken.push(Broken {
                    part: joint.part.clone(),
                    how: BrokenHow::Turned {
                        axis,
                        degrees: joint.turn[axis],
                        allowed,
                    },
                });
            }
        }
        if !part.limits.may_move() && joint.move_by != [0.0; 3] {
            broken.push(Broken {
                part: joint.part.clone(),
                how: BrokenHow::Moved,
            });
        }
    }
    broken
}

/// The lowest point of a model: how far above the ground (y = 0) it is,
/// below it if negative.
pub fn lowest(model: &Model) -> f64 {
    model.bounds().map_or(0.0, |bounds| bounds.min.y)
}
