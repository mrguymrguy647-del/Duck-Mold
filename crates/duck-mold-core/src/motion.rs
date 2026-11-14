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
    let turn = own_turn(part, joint.turn);
    let moved = own_move(part, joint.move_by);
    Transform {
        turn,
        shift: part.pivot + moved - turn * part.pivot,
    }
}

/// A part's turn (degrees around x, then y, then z, in the directions
/// of the list it is written in) as a turn in the model's directions.
pub fn own_turn(part: &Part, turn: [f64; 3]) -> Mat3 {
    let directions = directions(&part.axes);
    // The turn is written in the list's directions; in the model's it is
    // the same turn seen from there. (`directions` is square, so turning
    // its rows into columns undoes it.)
    directions * Mat3::from_rotate(turn) * directions.transposed()
}

/// A part's move (metres, in the directions of the list it is written
/// in) as a move in the model's directions.
pub fn own_move(part: &Part, move_by: [f64; 3]) -> Vec3 {
    part.axes * Vec3::from(move_by)
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

// ---- animations ------------------------------------------------------------------------

/// An animation of a model: poses, each with the time it is reached.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    /// Its name.
    pub name: String,
    /// Where the recipe wrote it.
    pub loc: Loc,
    /// True if it starts again after its last key.
    pub looping: bool,
    /// True if the motion flows through each pose and starts and ends
    /// gently; false if it goes straight from pose to pose at a steady
    /// speed.
    pub smooth: bool,
    /// The keys, each later than the one before; the first is at 0.
    pub keys: Vec<Key>,
}

/// One key of an animation.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    /// Seconds from the animation's start.
    pub at: f64,
    /// The pose that is reached then: one of the model's, or `rest`.
    pub pose: String,
    /// Where the recipe wrote it.
    pub loc: Loc,
}

impl Animation {
    /// How long it lasts, in seconds: the time of its last key.
    pub fn length(&self) -> f64 {
        self.keys.last().map_or(0.0, |key| key.at)
    }
}

/// How many times a second an animation is looked at: by `inspect
/// --animation`, and for the samples a `.glb` holds.
pub const FRAMES_A_SECOND: f64 = 30.0;

/// The times of an animation's frames: from 0, thirty a second, and its
/// very end.
pub fn frame_times(length: f64) -> Vec<f64> {
    let whole = (length * FRAMES_A_SECOND + 1e-9).floor().max(0.0) as usize;
    let mut times: Vec<f64> = (0..=whole)
        .map(|frame| frame as f64 / FRAMES_A_SECOND)
        .collect();
    let last = times.len() - 1;
    if length - times[last] > 1e-9 {
        times.push(length);
    } else {
        times[last] = length.max(0.0);
    }
    times
}

/// One part's way through an animation: its six numbers (the turn's
/// three, then the move's three) at every key, and how fast each is
/// changing there.
#[derive(Debug, Clone, PartialEq)]
struct Track {
    part: String,
    values: Vec<[f64; 6]>,
    slopes: Vec<[f64; 6]>,
}

/// An animation made ready to ask where every part is at any time.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    /// How long it lasts, in seconds.
    pub length: f64,
    /// True if it starts again after its end.
    pub looping: bool,
    smooth: bool,
    times: Vec<f64>,
    tracks: Vec<Track>,
}

impl Motion {
    /// `animation`, for `model`. A key whose pose the model doesn't have
    /// counts as `rest`.
    pub fn of(model: &Model, animation: &Animation) -> Motion {
        let poses: Vec<Pose> = animation
            .keys
            .iter()
            .map(|key| {
                model.pose(&key.pose).unwrap_or(Pose {
                    name: key.pose.clone(),
                    loc: Loc::default(),
                    joints: Vec::new(),
                })
            })
            .collect();
        let times: Vec<f64> = animation.keys.iter().map(|key| key.at).collect();
        let mut tracks = Vec::new();
        for part in model.all_parts() {
            let line = |pose: &Pose| {
                pose.joints
                    .iter()
                    .find(|joint| joint.part == part.path)
                    .map_or([0.0; 6], |joint| {
                        let ([x, y, z], [a, b, c]) = (joint.turn, joint.move_by);
                        [x, y, z, a, b, c]
                    })
            };
            let values: Vec<[f64; 6]> = poses.iter().map(line).collect();
            if values.iter().all(|value| *value == [0.0; 6]) {
                continue;
            }
            let slopes = if animation.smooth {
                slopes(&times, &values, animation.looping)
            } else {
                Vec::new()
            };
            tracks.push(Track {
                part: part.path.clone(),
                values,
                slopes,
            });
        }
        Motion {
            length: animation.length(),
            looping: animation.looping,
            smooth: animation.smooth,
            times,
            tracks,
        }
    }

    /// The parts the animation turns or moves, in the model's order.
    pub fn parts(&self) -> Vec<&str> {
        self.tracks
            .iter()
            .map(|track| track.part.as_str())
            .collect()
    }

    /// Where the animation has every part it moves, `at` seconds from
    /// its start. Before the start it is as at the start, and after the
    /// end as at the end.
    pub fn joints_at(&self, at: f64) -> Vec<Joint> {
        let Some(&end) = self.times.last() else {
            return Vec::new();
        };
        let at = at.clamp(0.0, end);
        // The key at or before `at`, and how far to the next (0 to 1).
        let before = self
            .times
            .iter()
            .rposition(|&time| time <= at)
            .unwrap_or(0)
            .min(self.times.len().saturating_sub(2));
        let after = (before + 1).min(self.times.len() - 1);
        let span = self.times[after] - self.times[before];
        let along = if span > 0.0 {
            (at - self.times[before]) / span
        } else {
            0.0
        };
        self.tracks
            .iter()
            .map(|track| {
                let (from, to) = (track.values[before], track.values[after]);
                let mut now = [0.0; 6];
                for (number, value) in now.iter_mut().enumerate() {
                    *value = if along <= 0.0 {
                        from[number]
                    } else if along >= 1.0 {
                        to[number]
                    } else if self.smooth {
                        curve(
                            from[number],
                            track.slopes[before][number] * span,
                            to[number],
                            track.slopes[after][number] * span,
                            along,
                        )
                    } else {
                        from[number] + (to[number] - from[number]) * along
                    };
                }
                let [x, y, z, a, b, c] = now;
                Joint {
                    part: track.part.clone(),
                    loc: Loc::default(),
                    turn: [x, y, z],
                    move_by: [a, b, c],
                }
            })
            .collect()
    }
}

/// A smooth way from `from` to `to` that leaves at the speed `leaving`
/// and arrives at the speed `arriving` (both measured over the whole
/// way), `along` it (0 to 1).
fn curve(from: f64, leaving: f64, to: f64, arriving: f64, along: f64) -> f64 {
    let (two, three) = (along * along, along * along * along);
    (2.0 * three - 3.0 * two + 1.0) * from
        + (three - 2.0 * two + along) * leaving
        + (-2.0 * three + 3.0 * two) * to
        + (three - two) * arriving
}

/// How fast each number is changing at every key of a smooth animation,
/// in units a second.
///
/// Where a number turns back (it went up and now comes down) or stays,
/// its speed there is 0: the arm comes to a stop at the top of its
/// swing. Otherwise it keeps going, at a speed between those of the two
/// sides, weighted so that the number never goes past a key's value on
/// the way to it: an animation whose poses keep within a part's limits
/// keeps within them all the way. At its two ends an animation starts
/// and stops gently (speed 0), unless it loops: then the end runs into
/// the start.
fn slopes(times: &[f64], values: &[[f64; 6]], looping: bool) -> Vec<[f64; 6]> {
    let count = times.len();
    let mut slopes = vec![[0.0; 6]; count];
    if count < 2 {
        return slopes;
    }
    // The steady speed over each stretch between two keys.
    let stretch = |at: usize, number: usize| {
        (values[at + 1][number] - values[at][number]) / (times[at + 1] - times[at])
    };
    let between = |before: usize, after: usize, number: usize| {
        let (slow, fast) = (stretch(before, number), stretch(after, number));
        if slow * fast <= 0.0 {
            return 0.0;
        }
        let (short, long) = (
            times[before + 1] - times[before],
            times[after + 1] - times[after],
        );
        let (first, second) = (2.0 * long + short, long + 2.0 * short);
        (first + second) / (first / slow + second / fast)
    };
    for (at, slope) in slopes.iter_mut().enumerate() {
        for (number, speed) in slope.iter_mut().enumerate() {
            *speed = if at > 0 && at < count - 1 {
                between(at - 1, at, number)
            } else if looping {
                // The last stretch runs into the first.
                between(count - 2, 0, number)
            } else {
                0.0
            };
        }
    }
    slopes
}

/// True if an animation ends where it starts: its last key puts every
/// part where its first does. (A wheel turned 360 is where it was at
/// 0.) An animation with fewer than two keys does.
pub fn ends_as_it_starts(model: &Model, animation: &Animation) -> bool {
    let (Some(first), Some(last)) = (animation.keys.first(), animation.keys.last()) else {
        return true;
    };
    if first.pose == last.pose {
        return true;
    }
    let joints = |name: &str| model.pose(name).map_or(Vec::new(), |pose| pose.joints);
    let start = placements(model, &joints(&first.pose));
    let end = placements(model, &joints(&last.pose));
    start.iter().zip(&end).all(|(one, other)| {
        let turns = one
            .turn
            .rows
            .iter()
            .flatten()
            .zip(other.turn.rows.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-6);
        turns && (one.shift - other.shift).length() < 1e-6
    })
}
