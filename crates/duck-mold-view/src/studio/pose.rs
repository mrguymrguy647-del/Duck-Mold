//! Pose mode: turning a model's parts about their joins, in its poses.
//!
//! In Pose mode the model stands in a pose: `rest` (as the recipe builds
//! it) or one of the recipe's `[poses]`. A click selects a part, and
//! rings appear at its join, one for each direction its `limits` let it
//! turn. Dragging a ring turns the part, and what hangs from it follows.
//! The turn snaps to 5 degrees (Ctrl: free) and stops at the limit.
//! Letting go is one `set-pose` edit, the same an agent would run, and
//! one step to undo. The Move tool gives arrows instead, to slide the
//! part (a body that crouches, a drawer).
//!
//! A drag in `rest` doesn't change the model: `rest` is the model as it
//! is built. It starts a new pose instead.
//!
//! The rings turn exactly one of the pose's three numbers each. A turn
//! is around x, then y, then z, so the z ring turns around the part's
//! own z as its parent holds it, the y ring around y once z has turned,
//! and the x ring around x once y and z have.

use duck_mold_core::edit::{self, Edit};
use duck_mold_core::inspect::inspect_pose;
use duck_mold_core::math::{Mat3, Vec3, vec3};
use duck_mold_core::model::Model;
use duck_mold_core::motion::{self, Joint};
use duck_mold_core::recipe::{Limits, REST, Turns};

use super::handles::{
    AXIS_COLORS, Grip, HOT, LENGTH, REACH, RING_PIECES, nearest_on_line, on_plane, turn_between,
};
use super::shape::Mode;
use super::viewport::{self, Mark};
use super::{Problem, Studio};

/// What a drag does in Pose mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoseTool {
    /// Rings: turn the part about its join (R).
    Turn,
    /// Arrows: slide it (G).
    Move,
}

/// The step a turn snaps to, in degrees.
pub const POSE_STEP: f64 = 5.0;

/// The names of x, y and z, for labels.
const AXIS_NAMES: [&str; 3] = ["x", "y", "z"];

/// One handle of the selected part in Pose mode.
#[derive(Debug, Clone, PartialEq)]
struct PoseHandle {
    grip: Grip,
    /// Which of the pose's three numbers it changes.
    which: usize,
    /// The direction it turns around, or slides along, in the model, 1
    /// long.
    axis: Vec3,
    /// For an arrow: how many metres of the model one of the pose's
    /// units is, that way.
    unit: f64,
    /// For a ring: +1 if turning around `axis` the way [`Mat3::around`]
    /// counts raises the number, -1 if it lowers it.
    sense: f64,
}

/// The handles of the selected part, and where they are.
#[derive(Debug, Clone, PartialEq)]
struct PoseGizmo {
    part: String,
    /// The part's join as it stands now, in the picture's space.
    centre: Vec3,
    /// How long an arrow is, in metres, to be [`LENGTH`] pixels.
    reach: f64,
    /// Its turn and move in the pose, as written.
    turn: [f64; 3],
    move_by: [f64; 3],
    limits: Limits,
    handles: Vec<PoseHandle>,
}

/// A ring or an arrow in hand in Pose mode.
#[derive(Debug, Clone)]
pub(super) struct PoseGrab {
    gizmo: PoseGizmo,
    handle: PoseHandle,
    /// The pointer when the drag began, in pixels.
    pressed: (f64, f64),
    /// Where along an arrow the pointer took hold.
    hold: f64,
    /// The turn and move the drag would write, once it has changed
    /// something.
    now: Option<([f64; 3], [f64; 3])>,
    label: String,
}

impl Studio {
    /// Goes into Pose mode: the model stands in the pose chosen, and a
    /// drag turns the selected part (O).
    pub fn pose_mode(&mut self) -> Result<String, String> {
        if self.model().is_none() {
            let why = "There is no model to pose yet: the recipe has mistakes".to_owned();
            self.say(why.clone());
            return Err(why);
        }
        self.cancel_drag();
        self.mode = Mode::Pose;
        self.faces.clear();
        self.also.clear();
        if !self.pose_names().contains(&self.pose) {
            self.pose = REST.to_owned();
        }
        self.refresh_pose();
        let said = "Posing: click a part, then drag a ring to turn it. Pick the pose on the right. O goes back to parts".to_owned();
        self.say(said.clone());
        Ok(said)
    }

    /// O: into Pose mode, or back to Part mode.
    pub fn toggle_pose_mode(&mut self) -> Result<String, String> {
        if self.mode == Mode::Pose {
            self.part_mode();
            Ok(self.status().to_owned())
        } else {
            self.pose_mode()
        }
    }

    /// The model's poses, by name: `rest` first, then the recipe's, in
    /// its order.
    pub fn pose_names(&self) -> Vec<String> {
        std::iter::once(REST.to_owned())
            .chain(
                self.model()
                    .into_iter()
                    .flat_map(|model| model.poses.iter().map(|pose| pose.name.clone())),
            )
            .collect()
    }

    /// The pose the model stands in, in Pose mode: what a drag changes.
    pub fn pose(&self) -> &str {
        &self.pose
    }

    /// Stands the model in another pose. False if it has none by that
    /// name.
    pub fn choose_pose(&mut self, name: &str) -> bool {
        if !self.pose_names().iter().any(|known| known == name) {
            return false;
        }
        self.cancel_drag();
        self.pose = name.to_owned();
        self.refresh_pose();
        self.say(if name == REST {
            "The model as it is built: a drag starts a new pose".to_owned()
        } else {
            format!("The pose \"{name}\": drag a ring to change it")
        });
        true
    }

    /// The parts of the model, by path, that are turned or moved in the
    /// pose shown: marked in the list of parts.
    pub fn posed_parts(&self) -> Vec<String> {
        self.pose_joints()
            .into_iter()
            .filter(|joint| !joint.is_rest())
            .map(|joint| joint.part)
            .collect()
    }

    /// How a part is turned and moved in the pose chosen, as the recipe
    /// writes it: degrees around x, y and z, and metres.
    pub fn pose_line(&self, path: &str) -> ([f64; 3], [f64; 3]) {
        self.model()
            .and_then(|model| model.pose(&self.pose))
            .and_then(|pose| pose.joints.into_iter().find(|joint| joint.part == path))
            .map_or(([0.0; 3], [0.0; 3]), |joint| (joint.turn, joint.move_by))
    }

    /// How a part may turn: its `limits`, as built.
    pub fn limits_of(&self, path: &str) -> Option<Limits> {
        self.part(path).map(|part| part.limits)
    }

    /// The joints of what the model shows now in Pose mode: the pose
    /// chosen.
    pub(super) fn pose_joints(&self) -> Vec<Joint> {
        let Some(model) = self.model() else {
            return Vec::new();
        };
        model
            .pose(&self.pose)
            .map(|pose| pose.joints)
            .unwrap_or_default()
    }

    /// Works out the model as the pose puts it, and what the pose gets
    /// wrong that the model at rest doesn't. Called whenever the pose or
    /// the file changes.
    pub(super) fn refresh_pose(&mut self) {
        self.pose_problems.clear();
        let showing = self.mode == Mode::Pose;
        let Some(model) = self.model().filter(|_| showing) else {
            self.posed = None;
            return;
        };
        let joints = self.pose_joints();
        let posed = motion::posed(model, &joints);
        // A part that floats in the pose and doesn't at rest. (A limit
        // a pose breaks is a warning of the file already.)
        if self.mode == Mode::Pose
            && self.pose != REST
            && let (Some(rest), Some(pose)) = (self.report(), model.pose(&self.pose))
        {
            let report = inspect_pose(rest, model, &posed, &pose, &self.shown_as);
            let new = report
                .pose
                .map(|pose| pose.new_floating)
                .unwrap_or_default();
            if new.len() > 3 {
                // Most of the model away from what stands on the ground:
                // one line, not a line for every part.
                self.pose_problems.push(Problem {
                    text: format!(
                        "in the pose \"{}\", {} parts touch nothing that reaches the ground: {}, {}, {} and {} more",
                        self.pose,
                        new.len(),
                        new[0],
                        new[1],
                        new[2],
                        new.len() - 3
                    ),
                    is_error: false,
                    part: new.first().cloned(),
                });
            } else {
                for part in new {
                    let gap = report
                        .floating
                        .iter()
                        .find(|float| float.part == part)
                        .map_or(String::new(), |float| {
                            format!(" {} m from {}", short(float.gap), float.nearest)
                        });
                    self.pose_problems.push(Problem {
                        text: format!(
                            "in the pose \"{}\", \"{part}\" floats{gap}: it touches nothing",
                            self.pose
                        ),
                        is_error: false,
                        part: Some(part),
                    });
                }
            }
        }
        self.posed = Some(posed);
    }

    /// The handles of the selected part in Pose mode.
    fn pose_gizmo(&self) -> Option<PoseGizmo> {
        if self.mode != Mode::Pose || self.look_only {
            return None;
        }
        let path = self.selected.as_deref()?;
        let model = self.model()?;
        let part = model.part(path)?;
        let joints = self.pose_joints();
        let placed = motion::placements(model, &joints);
        let all = model.all_parts();
        let at = all.iter().position(|known| known.path == path)?;
        let parent = part
            .parent
            .as_deref()
            .and_then(|parent| all.iter().position(|known| known.path == parent))
            .map_or(Mat3::IDENTITY, |at| placed[at].turn);
        let (turn, move_by) = joints
            .iter()
            .find(|joint| joint.part == path)
            .map_or(([0.0; 3], [0.0; 3]), |joint| (joint.turn, joint.move_by));
        let centre = placed[at].apply(part.pivot) + self.offset();
        let away = self.orbit.to_view(centre).z.max(0.05);
        let reach = LENGTH * away / viewport::focal(self.view.1.max(1));
        let limits = part.limits;
        let directions = motion::directions(&part.axes);
        let mut handles = Vec::new();
        match self.pose_tool {
            PoseTool::Turn => {
                // The part's world turn with these three numbers, without
                // the parent's move.
                let world = |turn: [f64; 3]| {
                    parent * directions * Mat3::from_rotate(turn) * directions.transposed()
                };
                let axes = [
                    directions
                        * Mat3::around_z(turn[2])
                        * Mat3::around_y(turn[1])
                        * vec3(1.0, 0.0, 0.0),
                    directions * Mat3::around_z(turn[2]) * vec3(0.0, 1.0, 0.0),
                    directions * vec3(0.0, 0.0, 1.0),
                ];
                for (which, allowed) in limits.turns().into_iter().enumerate() {
                    if allowed == Turns::No {
                        continue;
                    }
                    let Some(axis) = (parent * axes[which]).normalized() else {
                        continue;
                    };
                    // Which way round raises the number: try a degree.
                    let mut nudged = turn;
                    nudged[which] += 1.0;
                    let after = world(nudged);
                    let before = world(turn);
                    let close = |other: Mat3| {
                        other
                            .rows
                            .iter()
                            .flatten()
                            .zip(after.rows.iter().flatten())
                            .map(|(a, b)| (a - b).abs())
                            .sum::<f64>()
                    };
                    let sense = if close(Mat3::around(axis, 1.0) * before)
                        <= close(Mat3::around(axis, -1.0) * before)
                    {
                        1.0
                    } else {
                        -1.0
                    };
                    handles.push(PoseHandle {
                        grip: Grip::Ring(which),
                        which,
                        axis,
                        unit: 1.0,
                        sense,
                    });
                }
            }
            PoseTool::Move if limits.may_move() => {
                for which in 0..3 {
                    let along = parent * part.axes.column(which);
                    let unit = along.length();
                    let Some(axis) = along.normalized() else {
                        continue;
                    };
                    handles.push(PoseHandle {
                        grip: Grip::Arrow(which),
                        which,
                        axis,
                        unit,
                        sense: 1.0,
                    });
                }
            }
            PoseTool::Move => {}
        }
        Some(PoseGizmo {
            part: path.to_owned(),
            centre,
            reach,
            turn,
            move_by,
            limits,
            handles,
        })
    }

    /// Why the selected part has no handles in Pose mode, if it has
    /// none: for the line under the picture.
    pub fn why_no_pose_handles(&self) -> Option<String> {
        let gizmo = self.pose_gizmo()?;
        if !gizmo.handles.is_empty() {
            return None;
        }
        Some(match (self.pose_tool, gizmo.limits) {
            (_, Limits::Fixed) => format!(
                "\"{}\" is fixed: its limits say it neither turns nor moves",
                gizmo.part
            ),
            (PoseTool::Move, _) => format!("\"{}\" can't be moved: it is fixed", gizmo.part),
            (PoseTool::Turn, _) => format!(
                "\"{}\" doesn't turn: its limits name no direction. G gives arrows to move it",
                gizmo.part
            ),
        })
    }

    /// The points each handle is drawn through: a ring's circle, an
    /// arrow's two ends.
    fn pose_handle_points(gizmo: &PoseGizmo, handle: &PoseHandle) -> Vec<Vec3> {
        let (centre, reach) = (gizmo.centre, gizmo.reach);
        match handle.grip {
            Grip::Ring(_) => {
                let helper = if handle.axis.y.abs() < 0.9 {
                    Vec3::UP
                } else {
                    vec3(1.0, 0.0, 0.0)
                };
                let Some(first) = handle.axis.cross(helper).normalized() else {
                    return Vec::new();
                };
                let second = handle.axis.cross(first);
                (0..=RING_PIECES)
                    .map(|piece| {
                        let angle = (piece as f64 / RING_PIECES as f64) * std::f64::consts::TAU;
                        centre + (first * angle.cos() + second * angle.sin()) * (reach * 0.85)
                    })
                    .collect()
            }
            _ => vec![
                centre + handle.axis * (reach * 0.3),
                centre + handle.axis * reach,
            ],
        }
    }

    /// The handle of the gizmo under the pointer, if one is.
    fn pose_handle_under(&self, gizmo: &PoseGizmo, at: (f64, f64)) -> Option<PoseHandle> {
        let (width, height) = self.view;
        let seen = |point: Vec3| self.orbit.on_screen(point, width, height);
        let away = |point: (f64, f64)| ((point.0 - at.0).powi(2) + (point.1 - at.1).powi(2)).sqrt();
        let from_piece = |a: (f64, f64), b: (f64, f64)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let long = dx * dx + dy * dy;
            let along = if long < 1e-9 {
                0.0
            } else {
                (((at.0 - a.0) * dx + (at.1 - a.1) * dy) / long).clamp(0.0, 1.0)
            };
            away((a.0 + dx * along, a.1 + dy * along))
        };
        gizmo
            .handles
            .iter()
            .filter_map(|handle| {
                let points = Self::pose_handle_points(gizmo, handle);
                let distance = points
                    .windows(2)
                    .filter_map(|piece| Some(from_piece(seen(piece[0])?, seen(piece[1])?)))
                    .min_by(f64::total_cmp)?;
                (distance <= REACH).then_some((distance, handle))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, handle)| handle.clone())
    }

    /// The handles that can be pressed on in Pose mode.
    pub(super) fn pose_grips(&self) -> Vec<Grip> {
        self.pose_gizmo().map_or_else(Vec::new, |gizmo| {
            gizmo.handles.iter().map(|handle| handle.grip).collect()
        })
    }

    /// A point of a handle to take hold of it by: the point of a ring
    /// nearest the eye, or the middle of an arrow.
    pub(super) fn pose_grip_at(&self, grip: Grip) -> Option<Vec3> {
        let gizmo = self.pose_gizmo()?;
        let handle = gizmo.handles.iter().find(|handle| handle.grip == grip)?;
        let points = Self::pose_handle_points(&gizmo, handle);
        match grip {
            Grip::Ring(_) => {
                // The point nearest the eye that is well clear of the
                // other rings on the screen, so a press there takes this
                // one.
                let eye = self.orbit.eye();
                let (width, height) = self.view;
                let clear = |point: Vec3| {
                    let Some(seen) = self.orbit.on_screen(point, width, height) else {
                        return false;
                    };
                    gizmo
                        .handles
                        .iter()
                        .filter(|other| other.grip != grip)
                        .all(|other| {
                            Self::pose_handle_points(&gizmo, other).iter().all(|on| {
                                self.orbit.on_screen(*on, width, height).is_none_or(|on| {
                                    ((on.0 - seen.0).powi(2) + (on.1 - seen.1).powi(2)).sqrt()
                                        > REACH * 2.0
                                })
                            })
                        })
                };
                let nearest = |list: Vec<Vec3>| {
                    list.into_iter()
                        .min_by(|a, b| (*a - eye).length().total_cmp(&(*b - eye).length()))
                };
                let clear_points: Vec<Vec3> = points
                    .iter()
                    .copied()
                    .filter(|point| clear(*point))
                    .collect();
                nearest(clear_points).or_else(|| nearest(points))
            }
            _ => {
                let (from, to) = (*points.first()?, *points.last()?);
                Some(from + (to - from) * 0.6)
            }
        }
    }

    /// The rings or arrows of the selected part, to draw over the model.
    pub(super) fn pose_marks(&self) -> Vec<Mark> {
        let Some(gizmo) = self
            .pose_grab
            .as_ref()
            .map(|grab| grab.gizmo.clone())
            .or_else(|| self.pose_gizmo())
        else {
            return Vec::new();
        };
        let hot = match &self.pose_grab {
            Some(grab) => Some(grab.handle.grip),
            None => self
                .pointer
                .and_then(|pointer| self.pose_handle_under(&gizmo, pointer))
                .map(|handle| handle.grip),
        };
        let mut marks = Vec::new();
        for handle in &gizmo.handles {
            let color = if hot == Some(handle.grip) {
                HOT
            } else {
                AXIS_COLORS[handle.which]
            };
            let points = Self::pose_handle_points(&gizmo, handle);
            match handle.grip {
                Grip::Ring(_) => {
                    for piece in points.windows(2) {
                        marks.push(Mark::Line {
                            from: piece[0],
                            to: piece[1],
                            color,
                        });
                    }
                }
                _ => marks.push(Mark::Arrow {
                    from: points[0],
                    to: points[1],
                    color,
                }),
            }
        }
        // The join itself, as a dot.
        marks.push(Mark::Disc {
            at: gizmo.centre,
            radius: 4.0,
            color: [0xE8, 0xE8, 0xF0],
        });
        marks
    }

    /// Takes hold of the ring or arrow under the pointer, in Pose mode.
    pub(super) fn pose_grab_at(&mut self, at: (f64, f64)) -> Option<Grip> {
        let gizmo = self.pose_gizmo()?;
        let handle = self.pose_handle_under(&gizmo, at)?;
        let hold = match handle.grip {
            Grip::Arrow(_) => {
                let (eye, ray) = self.pointer_ray(at);
                nearest_on_line(gizmo.centre, handle.axis, eye, ray)?
            }
            _ => 0.0,
        };
        let grip = handle.grip;
        self.pose_grab = Some(PoseGrab {
            gizmo,
            handle,
            pressed: at,
            hold,
            now: None,
            label: String::new(),
        });
        Some(grip)
    }

    /// Moves the ring or arrow in hand to the pointer: works out the new
    /// turn (or move), held at the part's limit, and shows the model
    /// with it.
    pub(super) fn pose_drag_to(&mut self, at: (f64, f64), free: bool) {
        let Some(grab) = &self.pose_grab else {
            return;
        };
        let (eye, ray) = self.pointer_ray(at);
        let gizmo = &grab.gizmo;
        let handle = &grab.handle;
        let which = handle.which;
        let mut turn = gizmo.turn;
        let mut move_by = gizmo.move_by;
        let label;
        match handle.grip {
            Grip::Ring(_) => {
                let centre = gizmo.centre;
                let Some(facing_us) = (eye - centre).normalized().map(|to| to.dot(handle.axis))
                else {
                    return;
                };
                // Seen nearly edge on, the ring is a line: then left and
                // right turn it, half a degree to the pixel.
                let turned = if facing_us.abs() < 0.2 {
                    (at.0 - grab.pressed.0) * 0.5
                } else {
                    let (_, first_ray) = self.pointer_ray(grab.pressed);
                    let (Some(from), Some(to)) = (
                        on_plane(centre, handle.axis, eye, first_ray),
                        on_plane(centre, handle.axis, eye, ray),
                    ) else {
                        return;
                    };
                    let (Some(from), Some(to)) =
                        ((from - centre).normalized(), (to - centre).normalized())
                    else {
                        return;
                    };
                    turn_between(handle.axis, from, to)
                };
                let wanted = gizmo.turn[which] + handle.sense * turned;
                let wanted = if free {
                    (wanted * 10.0).round() / 10.0
                } else {
                    (wanted / POSE_STEP).round() * POSE_STEP
                };
                let allowed = gizmo.limits.turns()[which];
                let held = allowed.kept(wanted);
                turn[which] = held + 0.0;
                label = if (held - wanted).abs() > 1e-9 {
                    format!("{} {}° (its limit)", AXIS_NAMES[which], short(held))
                } else {
                    format!("{} {}°", AXIS_NAMES[which], short(held))
                };
            }
            _ => {
                let Some(now) = nearest_on_line(gizmo.centre, handle.axis, eye, ray) else {
                    return;
                };
                let step = self.snap_step();
                let slid = (now - grab.hold) / handle.unit.max(1e-9);
                let slid = if free {
                    (slid * 10_000.0).round() / 10_000.0
                } else {
                    (slid / step).round() * step
                };
                move_by[which] =
                    ((gizmo.move_by[which] + slid) * 10_000.0).round() / 10_000.0 + 0.0;
                label = format!("{} {} m", AXIS_NAMES[which], short(move_by[which]));
            }
        }
        let part = gizmo.part.clone();
        let changed = turn != gizmo.turn || move_by != gizmo.move_by;
        let mut joints = self.pose_joints();
        match joints.iter_mut().find(|joint| joint.part == part) {
            Some(joint) => {
                joint.turn = turn;
                joint.move_by = move_by;
            }
            None => joints.push(Joint {
                part: part.clone(),
                loc: Default::default(),
                turn,
                move_by,
            }),
        }
        let preview = self.model().map(|model| motion::posed(model, &joints));
        if let Some(grab) = &mut self.pose_grab {
            grab.label = label;
            grab.now = changed.then_some((turn, move_by));
        }
        self.preview = preview;
    }

    /// Lets go of the ring or arrow: one `set-pose` edit, one step to
    /// undo. In `rest`, a new pose is made for it.
    pub(super) fn pose_let_go(&mut self) -> Option<Result<String, String>> {
        let grab = self.pose_grab.take()?;
        self.preview = None;
        let (turn, move_by) = grab.now?;
        let part = grab.gizmo.part.clone();
        let rest = turn == [0.0; 3] && move_by == [0.0; 3];
        // Only what is, or was, turned or moved is written.
        let (was_turn, was_move) = (grab.gizmo.turn, grab.gizmo.move_by);
        let set = |pose: &str| Edit::SetPose {
            pose: pose.to_owned(),
            part: part.clone(),
            turn: (!rest && (turn != [0.0; 3] || was_turn != [0.0; 3])).then_some(turn),
            move_by: (!rest && (move_by != [0.0; 3] || was_move != [0.0; 3])).then_some(move_by),
            rest,
        };
        if self.pose == REST {
            if rest {
                return None;
            }
            // `rest` is the model itself: a drag there starts a pose.
            let name = edit::unused_name("pose", &self.pose_names());
            let done = self.apply(&set(&name));
            if done.is_ok() {
                self.pose = name.clone();
                self.refresh_pose();
                let said = format!(
                    "Made the pose \"{name}\" (rest is the model as it is built): rename it on the right"
                );
                self.say(said.clone());
                return Some(Ok(said));
            }
            return Some(done);
        }
        let pose = self.pose.clone();
        Some(self.apply(&set(&pose)))
    }

    /// True while a ring or arrow is in hand in Pose mode.
    pub(super) fn pose_grab_held(&self) -> bool {
        self.pose_grab.is_some()
    }

    /// The ring or arrow in hand in Pose mode.
    pub(super) fn pose_grabbed(&self) -> Option<Grip> {
        self.pose_grab.as_ref().map(|grab| grab.handle.grip)
    }

    /// What the drag in hand has done so far, in Pose mode.
    pub(super) fn pose_drag_label(&self) -> Option<&str> {
        self.pose_grab
            .as_ref()
            .map(|grab| grab.label.as_str())
            .filter(|label| !label.is_empty())
    }

    /// Sets a part's turn and move in the pose chosen, typed in the
    /// panel: one `set-pose`. In `rest`, a new pose is made for it.
    pub fn set_pose_numbers(
        &mut self,
        part: &str,
        turn: [f64; 3],
        move_by: [f64; 3],
    ) -> Result<String, String> {
        let rest = turn == [0.0; 3] && move_by == [0.0; 3];
        let (was_turn, was_move) = self.pose_line(part);
        let pose = if self.pose == REST {
            if rest {
                return Ok(String::new());
            }
            edit::unused_name("pose", &self.pose_names())
        } else {
            self.pose.clone()
        };
        let done = self.apply(&Edit::SetPose {
            pose: pose.clone(),
            part: part.to_owned(),
            turn: (!rest && (turn != [0.0; 3] || was_turn != [0.0; 3])).then_some(turn),
            move_by: (!rest && (move_by != [0.0; 3] || was_move != [0.0; 3])).then_some(move_by),
            rest,
        });
        if done.is_ok() && pose != self.pose {
            self.pose = pose;
            self.refresh_pose();
        }
        done
    }

    /// Puts a part back at rest in the pose chosen.
    pub fn pose_back_to_rest(&mut self, part: &str) -> Result<String, String> {
        if self.pose == REST {
            return Ok(String::new());
        }
        let pose = self.pose.clone();
        self.apply(&Edit::SetPose {
            pose,
            part: part.to_owned(),
            turn: None,
            move_by: None,
            rest: true,
        })
    }

    /// New: a new pose, with every part at rest, chosen at once.
    pub fn new_pose(&mut self) -> Result<String, String> {
        let name = edit::unused_name("pose", &self.pose_names());
        self.pose_list_edit(
            &Edit::AddPose {
                name: name.clone(),
                from: None,
            },
            &name,
        )
    }

    /// Copy: a copy of the pose chosen, chosen at once.
    pub fn copy_pose(&mut self) -> Result<String, String> {
        let from = self.pose.clone();
        let name = edit::unused_name(&format!("{from}_copy"), &self.pose_names());
        self.pose_list_edit(
            &Edit::AddPose {
                name: name.clone(),
                from: Some(from),
            },
            &name,
        )
    }

    /// Mirror: the mirror image of the pose chosen, left to right. A
    /// pose named for one side (`step_left`) makes the other
    /// (`step_right`); any other is mirrored in place.
    pub fn mirror_pose(&mut self) -> Result<String, String> {
        let pose = self.pose.clone();
        if pose == REST {
            let why = "rest is the same both ways round: pick a pose to mirror".to_owned();
            self.say(why.clone());
            return Err(why);
        }
        let other = other_side(&pose);
        let as_name = (other != pose).then_some(other);
        let chosen = as_name.clone().unwrap_or_else(|| pose.clone());
        self.pose_list_edit(&Edit::MirrorPose { pose, as_name }, &chosen)
    }

    /// Rename: the pose chosen gets another name.
    pub fn rename_pose(&mut self, new_name: &str) -> Result<String, String> {
        let pose = self.pose.clone();
        self.pose_list_edit(
            &Edit::RenamePose {
                pose,
                new_name: new_name.to_owned(),
            },
            new_name,
        )
    }

    /// The animations whose keys use a pose, for the question Remove
    /// asks first.
    pub fn keys_using(&self, pose: &str) -> Vec<String> {
        self.model().map_or_else(Vec::new, |model| {
            model
                .animations
                .iter()
                .filter(|animation| animation.keys.iter().any(|key| key.pose == pose))
                .map(|animation| animation.name.clone())
                .collect()
        })
    }

    /// Remove: takes the pose chosen away (with the keys that use it,
    /// if `with_keys`), and goes back to `rest`.
    pub fn remove_pose(&mut self, with_keys: bool) -> Result<String, String> {
        let pose = self.pose.clone();
        self.pose_list_edit(&Edit::RemovePose { pose, with_keys }, REST)
    }

    /// One edit to the list of poses; then `chosen` is the pose shown.
    fn pose_list_edit(&mut self, change: &Edit, chosen: &str) -> Result<String, String> {
        let done = self.apply(change);
        if done.is_ok() {
            self.pose = chosen.to_owned();
            self.refresh_pose();
        }
        done
    }

    /// Sets the selected part's limits, typed in the panel: one
    /// `set-limits`.
    pub fn set_limits_of(&mut self, part: &str, limits: edit::LimitsTo) -> Result<String, String> {
        self.apply(&Edit::SetLimits {
            part: part.to_owned(),
            limits,
        })
    }

    /// The model the Studio shows in Pose mode, without a drag: the
    /// pose chosen.
    pub(super) fn posed_model(&self) -> Option<&Model> {
        if self.mode == Mode::Pose {
            self.posed.as_ref()
        } else {
            None
        }
    }
}

/// A name with `left` and `right` swapped: `step_left` gives
/// `step_right`.
fn other_side(name: &str) -> String {
    const SWAP: &str = "\u{0}";
    name.replace("left", SWAP)
        .replace("right", "left")
        .replace(SWAP, "right")
}

/// A number for a label: no more decimals than it needs.
fn short(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0 + 0.0;
    format!("{rounded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_other_side_of_a_name() {
        assert_eq!(other_side("step_left"), "step_right");
        assert_eq!(other_side("right_kick"), "left_kick");
        assert_eq!(other_side("wave"), "wave");
    }
}
