//! The handles on the selected part: arrows to move it, rings to turn
//! it, boxes to scale it.
//!
//! A handle is a few lines drawn over the model. Pressing on one and
//! moving the pointer works out an edit: the same `set --move`, `set
//! --rotate`, `set --scale`, `set --twist`, `set-socket` or `join` an
//! agent would run. While the handle is held, the Studio shows what the
//! recipe *would* build with that edit (as `--dry-run` does) and writes
//! nothing. Letting go writes the edit once: one step to undo. Escape
//! lets go without writing.
//!
//! - **A free part** (one that isn't joined) is moved along the list's
//!   own x, y and z, turned around them, and scaled along its own sides.
//!   Dragging the part itself slides it across the ground.
//! - **A joined part** slides along the surface it sits on (its socket's
//!   `shift` changes) and twists around its join.
//!
//! Moves snap to the grid and turns to 15 degrees, unless the drag is
//! `free` (Ctrl is held).
//!
//! The handles know nothing of parts as such: they act on a middle and
//! three directions. Phase 4 will hand them a corner, an edge or a face.

use duck_mold_core::edit::{self, Changes, Edit, Facts};
use duck_mold_core::math::{Mat3, Vec3, vec3};
use duck_mold_core::model::Part;
use duck_mold_core::source::Syntax;

use super::shape::{FaceTool, Mode};
use super::viewport::{self, Mark};
use super::{Studio, first_error, part_in, sockets};

/// The tool in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Arrows: move the part.
    Move,
    /// Rings: turn it.
    Rotate,
    /// Boxes: make it bigger or smaller.
    Scale,
    /// Knobs on a shape's faces: change its own sizes.
    Size,
    /// A click colours a face.
    Paint,
}

/// What can be pressed on and dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    /// A move arrow: 0, 1, 2 for x, y, z (for a joined part: the two
    /// ways across the surface).
    Arrow(usize),
    /// A turn ring around x, y or z (for a joined part: around its join).
    Ring(usize),
    /// A scale box on one of the part's own three sides.
    Box(usize),
    /// The scale box in the middle: all three alike.
    Middle,
    /// A knob on a face of a shape (the Size tool).
    Knob(usize),
    /// The part itself.
    Body,
    /// In Face mode: the arrow out of the faces selected (Pull).
    Pull,
    /// In Face mode: the knob on an edge of a face selected (Inset).
    Inset,
}

/// Red, green and blue for x, y and z, as in other 3D tools.
pub(super) const AXIS_COLORS: [[u8; 3]; 3] =
    [[0xE5, 0x4B, 0x5F], [0x4C, 0xB8, 0x6A], [0x4A, 0x8C, 0xF0]];
/// Duck yellow: the handle under the pointer, or in hand.
pub(super) const HOT: [u8; 3] = [0xFF, 0xCC, 0x2E];
const MIDDLE: [u8; 3] = [0xE8, 0xE8, 0xF0];
/// The Inset knob: the colour of the sockets' dots.
const INSET: [u8; 3] = [0x6F, 0xD6, 0xC8];

/// How long an arrow is on the screen, in pixels, whatever the zoom.
pub(super) const LENGTH: f64 = 84.0;
/// How near the pointer must be to a handle to be on it, in pixels.
pub(super) const REACH: f64 = 9.0;
/// How many straight pieces a ring is drawn with.
pub(super) const RING_PIECES: usize = 48;
/// The step turns snap to, in degrees.
pub const TURN_STEP: f64 = 15.0;
/// The step scales snap to.
const SCALE_STEP: f64 = 0.1;
/// The step an inset snaps to, in metres: a frame is a few centimetres.
pub const INSET_STEP: f64 = 0.01;

/// One handle as it is drawn now.
struct Handle {
    grip: Grip,
    /// The line it is drawn along (two points), or the ring (many), in
    /// the picture's space.
    points: Vec<Vec3>,
    color: [u8; 3],
}

/// The join a part hangs by.
#[derive(Debug, Clone, PartialEq)]
struct Joined {
    /// The part it is joined to, by path.
    target: String,
    /// The socket on it.
    socket: String,
    /// Where that socket is in the model, and the way it faces.
    at: Vec3,
    facing: Vec3,
    /// The two ways across the surface there.
    across: [Vec3; 2],
}

/// The middle and directions the handles are drawn around.
#[derive(Debug, Clone, PartialEq)]
struct Gizmo {
    part: String,
    /// The middle of the handles, in the picture's space.
    centre: Vec3,
    /// How long an arrow is, in metres, to be [`LENGTH`] pixels.
    reach: f64,
    /// The x, y and z of the list the part is in, as directions of the
    /// model, and how long one of the list's units is along each.
    list: [Vec3; 3],
    units: [f64; 3],
    /// The part's own three sides, as directions of the model.
    own: [Vec3; 3],
    /// -1 if the list is a mirror image (inside a flipped part): turns
    /// there run the other way.
    sense: f64,
    joined: Option<Joined>,
    /// The Size tool's knobs, on the faces of a shape.
    knobs: Vec<Knob>,
    /// Set when several parts are selected: the handles hold them all.
    group: Option<Group>,
    /// Set in Face mode: the faces in hand, and their handles.
    face: Option<FaceGizmo>,
}

/// The handles of the faces selected in Face mode.
#[derive(Debug, Clone, PartialEq)]
struct FaceGizmo {
    /// The faces, by name.
    faces: Vec<String>,
    /// The way they are pulled out, in the picture, 1 long.
    out: Vec3,
    /// How many metres of the picture one metre of the part's own is,
    /// that way (its scale).
    out_unit: f64,
    /// The Inset knob: the middle of an edge of the first face.
    knob: Vec3,
    /// The way into that face from the edge, 1 long, and its unit.
    inward: Vec3,
    inward_unit: f64,
}

/// Several parts held together.
#[derive(Debug, Clone, PartialEq)]
struct Group {
    /// The parts the handles move, by an edit each: the free ones in the
    /// same list as the part selected last.
    moved: Vec<Member>,
    /// The selected parts that stay where they are: joined to a part
    /// that isn't selected, or in another list.
    stayed: Vec<String>,
}

/// One of the parts held together.
#[derive(Debug, Clone, PartialEq)]
struct Member {
    path: String,
    /// What the recipe said about it when the handles were made.
    facts: Facts,
    /// Where its `move` places it, in the model.
    at: Vec3,
}

/// Which of a shape's own sizes a knob changes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sized {
    /// One of the three numbers of `size`: 0, 1, 2 for width, height,
    /// depth.
    Side(usize),
    /// `height`.
    Height,
    /// `radius`.
    Radius,
    /// `tube`: how thick a ring is.
    Tube,
}

/// A knob of the Size tool: it sits on a face and pulls it out or in.
#[derive(Debug, Clone, PartialEq)]
struct Knob {
    /// Where it is, in the picture's space.
    at: Vec3,
    /// The way the face goes when it is pulled out, 1 long.
    out: Vec3,
    /// How many metres of the model one of the shape's own units is,
    /// that way (its `scale`, and that of what it is inside).
    unit: f64,
    sized: Sized,
    /// How far the part's middle has to move, in its list, for each unit
    /// the size grows, so that the opposite face stays where it is.
    /// Nothing for a radius, which grows all round.
    carry: Vec3,
}

/// A handle in hand.
pub(super) struct Grab {
    grip: Grip,
    gizmo: Gizmo,
    /// What the recipe said about the part when the drag began.
    facts: Facts,
    /// Where along the handle the pointer took hold.
    hold: f64,
    /// The pointer when the drag began, in pixels.
    pressed: (f64, f64),
    /// The point of the part the pointer took hold of (for the part
    /// itself), in the picture's space.
    point: Vec3,
    /// The edits the drag would make as it stands. Empty until the
    /// pointer has moved far enough to change something.
    edits: Vec<Edit>,
    /// What the drag has done, for the label by the pointer.
    label: String,
    /// Why the edit can't be made, if it can't.
    problem: Option<String>,
}

/// Where the line through `from` along `along` (1 long) comes nearest to
/// the line from `eye` along `ray`, as a distance along the first.
pub(super) fn nearest_on_line(from: Vec3, along: Vec3, eye: Vec3, ray: Vec3) -> Option<f64> {
    let between = from - eye;
    let (b, c) = (along.dot(ray), ray.dot(ray));
    let (d, e) = (along.dot(between), ray.dot(between));
    let bottom = c - b * b;
    (bottom.abs() > 1e-9).then(|| (b * e - c * d) / bottom)
}

/// Where the line from `eye` along `ray` meets the flat surface through
/// `point` that faces `facing`. `None` if it runs along it, or meets it
/// behind the eye.
pub(super) fn on_plane(point: Vec3, facing: Vec3, eye: Vec3, ray: Vec3) -> Option<Vec3> {
    let towards = ray.dot(facing);
    if towards.abs() < 1e-6 {
        return None;
    }
    let along = (point - eye).dot(facing) / towards;
    (along > 0.0).then(|| eye + ray * along)
}

/// The turn around `axis` that takes the direction `from` to `to`, in
/// degrees, counted the way [`Mat3::around`] counts.
pub(super) fn turn_between(axis: Vec3, from: Vec3, to: Vec3) -> f64 {
    let angle = from.cross(to).dot(axis).atan2(from.dot(to)).to_degrees();
    // Which way does `around` turn? Try a quarter turn and see.
    let tried = Mat3::around(axis, 90.0) * from;
    if from.cross(tried).dot(axis) < 0.0 {
        -angle
    } else {
        angle
    }
}

fn snapped(value: f64, step: f64, free: bool) -> f64 {
    if free {
        (value * 10_000.0).round() / 10_000.0
    } else {
        (value / step).round() * step
    }
}

/// A number for a label: no more decimals than it needs.
fn short(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0 + 0.0;
    format!("{rounded}")
}

impl Studio {
    /// The step moves and sizes snap to, in metres: a tenth of a grid
    /// square, or a hundredth when the view is close enough for those to
    /// be told apart.
    pub fn snap_step(&self) -> f64 {
        let pixels_per_metre = viewport::focal(self.view.1.max(1)) / self.orbit.distance;
        if pixels_per_metre * 0.1 > 60.0 {
            0.01
        } else {
            0.1
        }
    }

    /// What the bottom of the viewport says about snapping.
    pub fn snap_text(&self) -> String {
        if self.mode == Mode::Pose {
            return match self.pose_tool {
                super::PoseTool::Turn => {
                    format!("Snap {}°  (Ctrl: off)", short(super::pose::POSE_STEP))
                }
                super::PoseTool::Move => format!("Snap {} m  (Ctrl: off)", short(self.snap_step())),
            };
        }
        if self.mode != Mode::Part {
            return match self.face_tool {
                FaceTool::Pull | FaceTool::Move => {
                    format!("Snap {} m  (Ctrl: off)", short(self.snap_step()))
                }
                FaceTool::Inset => format!("Snap {} m  (Ctrl: off)", short(INSET_STEP)),
                FaceTool::Turn => format!("Snap {}°  (Ctrl: off)", short(TURN_STEP)),
                FaceTool::Scale => format!("Snap x {}  (Ctrl: off)", short(SCALE_STEP)),
                FaceTool::Cut => "Cuts in tenths  (Ctrl: free)".to_owned(),
            };
        }
        format!(
            "Snap {} m, {}°  (Ctrl: off)",
            short(self.snap_step()),
            short(TURN_STEP)
        )
    }

    /// The line from the eye through the pointer, in the picture's space:
    /// where the eye is, and the way the line runs.
    pub(super) fn pointer_ray(&self, at: (f64, f64)) -> (Vec3, Vec3) {
        let eye = self.orbit.eye();
        let through = self.orbit.from_view(viewport::ray_through(self.view, at));
        (eye, through - eye)
    }

    /// The middle and directions for the handles of the selected part.
    fn gizmo(&self) -> Option<Gizmo> {
        if self.look_only || self.mode == Mode::Pose {
            return None;
        }
        if self.mode != Mode::Part {
            return self.face_gizmo();
        }
        let path = self.selected.as_deref()?;
        let model = self.shown_model()?;
        let part = part_in(&model.parts, path)?;
        self.facts()?;
        let offset = self.offset();
        // The list the part is in: the model's own, or a part's.
        let around = path.rsplit_once('/').map(|(around, _)| around);
        let frame = match around {
            Some(around) => part_in(&model.parts, around)?.place.turn,
            None => Mat3::IDENTITY,
        };
        let mut list = [Vec3::ZERO; 3];
        let mut units = [1.0; 3];
        let mut own = [Vec3::ZERO; 3];
        for at in 0..3 {
            let column = frame.column(at);
            units[at] = column.length().max(1e-9);
            list[at] = column * (1.0 / units[at]);
            own[at] = part.place.turn.column(at).normalized()?;
        }
        if !self.also.is_empty() {
            // Several parts: the free ones in this list are moved, each
            // by its own edit. What is joined to one of them, or inside
            // one, goes along. The rest stay.
            let syntax = Syntax::from_path(&self.path)?;
            let mut moved = Vec::new();
            let mut stayed = Vec::new();
            for member in self.selection() {
                let same_list = member.rsplit_once('/').map(|(around, _)| around) == around;
                let named = !member.rsplit('/').next()?.starts_with('#');
                let held = part_in(&model.parts, member)
                    .zip(edit::describe(&self.text, syntax, self.model(), member))
                    .filter(|(_, facts)| same_list && named && facts.to.is_none());
                match held {
                    Some((held, facts)) => moved.push(Member {
                        path: member.to_owned(),
                        facts,
                        at: held.place.shift,
                    }),
                    None => stayed.push(member.to_owned()),
                }
            }
            let carried: Vec<String> = moved
                .iter()
                .flat_map(|member| self.hanging_from(&member.path))
                .collect();
            stayed.retain(|path| {
                !carried.contains(path)
                    && !moved.iter().any(|member| {
                        path.strip_prefix(member.path.as_str())
                            .is_some_and(|rest| rest.starts_with('/'))
                    })
            });
            let around_all = moved
                .iter()
                .filter_map(|member| part_in(&model.parts, &member.path)?.bounds)
                .reduce(|all, one| duck_mold_core::math::Bounds {
                    min: all.min.min(one.min),
                    max: all.max.max(one.max),
                })?;
            let centre = around_all.center() + offset;
            let away = self.orbit.to_view(centre).z.max(0.05);
            return Some(Gizmo {
                part: path.to_owned(),
                centre,
                reach: LENGTH * away / viewport::focal(self.view.1.max(1)),
                list,
                units,
                own: list,
                sense: if frame.determinant() < 0.0 { -1.0 } else { 1.0 },
                joined: None,
                knobs: Vec::new(),
                group: Some(Group { moved, stayed }),
                face: None,
            });
        }
        let joined = part.attached_to.as_deref().and_then(|to| {
            let (target, socket) = to.rsplit_once('.')?;
            let target_path = match around {
                Some(around) => format!("{around}/{target}"),
                None => target.to_owned(),
            };
            let onto = part_in(&model.parts, &target_path)?;
            let found = onto.sockets.iter().find(|known| known.name == socket)?;
            // The two of the target's own sides that lie flattest on the
            // surface there.
            let mut sides: Vec<Vec3> = (0..3)
                .filter_map(|at| onto.place.turn.column(at).normalized())
                .collect();
            sides.sort_by(|a, b| {
                a.dot(found.facing)
                    .abs()
                    .total_cmp(&b.dot(found.facing).abs())
            });
            let flat = |side: Vec3| (side - found.facing * side.dot(found.facing)).normalized();
            Some(Joined {
                target: target_path,
                socket: socket.to_owned(),
                at: found.at,
                facing: found.facing,
                across: [flat(*sides.first()?)?, flat(*sides.get(1)?)?],
            })
        });
        // A joined part is moved and turned about its join; a free one
        // is turned about its own middle (the point `move` places) and
        // moved and scaled about the middle of its box.
        let centre = match (&joined, self.tool) {
            (Some(joined), Tool::Move | Tool::Rotate) => joined.at,
            (None, Tool::Rotate) => part.place.shift,
            _ => part
                .bounds
                .map_or(part.place.shift, |bounds| bounds.center()),
        } + offset;
        let away = self.orbit.to_view(centre).z.max(0.05);
        let knobs = if self.tool == Tool::Size {
            self.knobs_of(part, offset)
        } else {
            Vec::new()
        };
        Some(Gizmo {
            part: path.to_owned(),
            centre,
            reach: LENGTH * away / viewport::focal(self.view.1.max(1)),
            list,
            units,
            own,
            sense: if frame.determinant() < 0.0 { -1.0 } else { 1.0 },
            joined,
            knobs,
            group: None,
            face: None,
        })
    }

    /// The handles in Face mode: around the faces selected, if any.
    fn face_gizmo(&self) -> Option<Gizmo> {
        if self.faces.is_empty() {
            return None;
        }
        let (part, shaped) = self.shaped_selected()?;
        let offset = self.offset();
        let (middle, facing) = self.faces_middle(&shaped)?;
        let way = part.place.turn * facing.normalized()?;
        let out_unit = way.length().max(1e-9);
        let out = way.normalized()?;
        let centre = part.place.apply(middle) + offset;
        // The Inset knob: on the middle of the first face's edge that is
        // nearest the pointer (so it is where the hand is).
        // (In Edge mode there is no face: the knob, never shown, is any
        // face's.)
        let first = shaped
            .faces_named(&self.faces[0])
            .first()
            .copied()
            .unwrap_or(0);
        let corners: Vec<Vec3> = shaped.faces[first]
            .corners
            .iter()
            .map(|&corner| shaped.corners[corner])
            .collect();
        let normal = shaped.normal(first)?;
        let count = corners.len();
        let seen = |point: Vec3| {
            self.orbit
                .on_screen(part.place.apply(point) + offset, self.view.0, self.view.1)
        };
        let edge = (0..count)
            .min_by(|&a, &b| {
                let away = |at: usize| {
                    let middle = (corners[at] + corners[(at + 1) % count]) * 0.5;
                    match (seen(middle), self.pointer) {
                        (Some(seen), Some(pointer)) => {
                            (seen.0 - pointer.0).powi(2) + (seen.1 - pointer.1).powi(2)
                        }
                        // Without a pointer: the edge lowest on the
                        // screen.
                        (Some(seen), None) => -seen.1,
                        (None, _) => f64::INFINITY,
                    }
                };
                away(a).total_cmp(&away(b))
            })
            .unwrap_or(0);
        let (from, to) = (corners[edge], corners[(edge + 1) % count]);
        let inward_own = normal.cross(to - from).normalized()?;
        let inward_way = part.place.turn * inward_own;
        let away = self.orbit.to_view(centre).z.max(0.05);
        // The part's own directions: a face moves, turns and scales
        // along them, as the recipe measures.
        let mut own = [Vec3::ZERO; 3];
        let mut units = [1.0; 3];
        for at in 0..3 {
            let column = part.place.turn.column(at);
            units[at] = column.length().max(1e-9);
            own[at] = column * (1.0 / units[at]);
        }
        Some(Gizmo {
            part: part.path.clone(),
            centre,
            reach: LENGTH * away / viewport::focal(self.view.1.max(1)),
            list: own,
            units,
            own,
            sense: if part.place.turn.determinant() < 0.0 {
                -1.0
            } else {
                1.0
            },
            joined: None,
            knobs: Vec::new(),
            group: None,
            face: Some(FaceGizmo {
                faces: self.faces.clone(),
                out,
                out_unit,
                knob: part.place.apply((from + to) * 0.5) + offset,
                inward: inward_way.normalized()?,
                inward_unit: inward_way.length().max(1e-9),
            }),
        })
    }

    /// The Size tool's knobs for a part that is one shape: one on each
    /// face whose place is one of the shape's own sizes.
    fn knobs_of(&self, part: &Part, offset: Vec3) -> Vec<Knob> {
        let Some(facts) = self.facts() else {
            return Vec::new();
        };
        let sizes = &facts.sizes;
        // A knob out along one of the shape's own directions, `reach` of
        // its own units from its middle.
        let knob = |which: usize, sign: f64, reach: f64, sized: Sized, carried: bool| {
            let mut own = [0.0; 3];
            own[which] = sign;
            let own: Vec3 = own.into();
            let way = part.place.turn * own;
            let unit = way.length();
            Some(Knob {
                at: part.place.apply(own * reach) + offset,
                out: way.normalized()?,
                unit,
                sized,
                carry: if carried {
                    part.local.turn * own * 0.5
                } else {
                    Vec3::ZERO
                },
            })
        };
        let mut knobs = Vec::new();
        if let Some(size) = sizes.size {
            // A box, or a shape measured like one: six sides.
            for (which, side) in size.into_iter().enumerate() {
                for sign in [1.0, -1.0] {
                    knobs.extend(knob(which, sign, side / 2.0, Sized::Side(which), true));
                }
            }
        }
        if let Some(height) = sizes.height {
            for sign in [1.0, -1.0] {
                knobs.extend(knob(1, sign, height / 2.0, Sized::Height, true));
            }
        }
        if let Some(radius) = sizes.radius {
            // On the surface: a ring's is its thickness further out.
            let reach = radius + sizes.tube.unwrap_or(0.0);
            knobs.extend(knob(0, 1.0, reach, Sized::Radius, false));
        }
        if let (Some(radius), Some(tube)) = (sizes.radius, sizes.tube) {
            // On top of the ring, over the middle of its thickness.
            let way = part.place.turn * Vec3::UP;
            if let Some(out) = way.normalized() {
                knobs.push(Knob {
                    at: part.place.apply(vec3(radius, tube, 0.0)) + offset,
                    out,
                    unit: way.length(),
                    sized: Sized::Tube,
                    carry: Vec3::ZERO,
                });
            }
        }
        knobs
    }

    /// The handles of the tool in hand, around `gizmo`.
    fn handles_of(&self, gizmo: &Gizmo) -> Vec<Handle> {
        let (centre, reach) = (gizmo.centre, gizmo.reach);
        let ring = |axis: Vec3| -> Vec<Vec3> {
            // Two directions across the axis, and a circle in them.
            let helper = if axis.y.abs() < 0.9 {
                Vec3::UP
            } else {
                vec3(1.0, 0.0, 0.0)
            };
            let Some(first) = axis.cross(helper).normalized() else {
                return Vec::new();
            };
            let second = axis.cross(first);
            (0..=RING_PIECES)
                .map(|piece| {
                    let angle = (piece as f64 / RING_PIECES as f64) * std::f64::consts::TAU;
                    centre + (first * angle.cos() + second * angle.sin()) * (reach * 0.85)
                })
                .collect()
        };
        let arrow = |along: Vec3| vec![centre + along * (reach * 0.3), centre + along * reach];
        let mut handles = Vec::new();
        if let Some(face) = &gizmo.face {
            match self.face_tool {
                FaceTool::Pull => handles.push(Handle {
                    grip: Grip::Pull,
                    points: vec![centre, centre + face.out * reach],
                    color: MIDDLE,
                }),
                FaceTool::Inset => handles.push(Handle {
                    grip: Grip::Inset,
                    points: vec![face.knob],
                    color: INSET,
                }),
                FaceTool::Move => {
                    for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                        handles.push(Handle {
                            grip: Grip::Arrow(at),
                            points: arrow(gizmo.own[at]),
                            color,
                        });
                    }
                }
                FaceTool::Turn => {
                    for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                        handles.push(Handle {
                            grip: Grip::Ring(at),
                            points: ring(gizmo.own[at]),
                            color,
                        });
                    }
                }
                FaceTool::Scale => {
                    for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                        handles.push(Handle {
                            grip: Grip::Box(at),
                            points: arrow(gizmo.own[at]),
                            color,
                        });
                    }
                    handles.push(Handle {
                        grip: Grip::Middle,
                        points: vec![centre],
                        color: MIDDLE,
                    });
                }
                // The Cut tool has a ring, but no handle: a click cuts.
                FaceTool::Cut => {}
            }
            return handles;
        }
        if gizmo.group.is_some() {
            // Several parts: arrows and rings along the list's own
            // directions, and one box for bigger or smaller all round.
            match self.tool {
                Tool::Move => {
                    for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                        handles.push(Handle {
                            grip: Grip::Arrow(at),
                            points: arrow(gizmo.list[at]),
                            color,
                        });
                    }
                }
                Tool::Rotate => {
                    for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                        handles.push(Handle {
                            grip: Grip::Ring(at),
                            points: ring(gizmo.list[at]),
                            color,
                        });
                    }
                }
                Tool::Scale => handles.push(Handle {
                    grip: Grip::Middle,
                    points: vec![centre],
                    color: MIDDLE,
                }),
                Tool::Size | Tool::Paint => {}
            }
            return handles;
        }
        match (self.tool, &gizmo.joined) {
            (Tool::Move, None) => {
                for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                    handles.push(Handle {
                        grip: Grip::Arrow(at),
                        points: arrow(gizmo.list[at]),
                        color,
                    });
                }
            }
            (Tool::Move, Some(joined)) => {
                for (at, across) in joined.across.iter().enumerate() {
                    handles.push(Handle {
                        grip: Grip::Arrow(at),
                        points: arrow(*across),
                        color: AXIS_COLORS[if at == 0 { 0 } else { 2 }],
                    });
                }
            }
            (Tool::Rotate, None) => {
                for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                    handles.push(Handle {
                        grip: Grip::Ring(at),
                        points: ring(gizmo.list[at]),
                        color,
                    });
                }
            }
            (Tool::Rotate, Some(joined)) => handles.push(Handle {
                grip: Grip::Ring(0),
                points: ring(joined.facing),
                color: AXIS_COLORS[1],
            }),
            (Tool::Scale, _) => {
                for (at, color) in AXIS_COLORS.into_iter().enumerate() {
                    handles.push(Handle {
                        grip: Grip::Box(at),
                        points: arrow(gizmo.own[at]),
                        color,
                    });
                }
                handles.push(Handle {
                    grip: Grip::Middle,
                    points: vec![centre],
                    color: MIDDLE,
                });
            }
            (Tool::Size, _) => {
                for (at, knob) in gizmo.knobs.iter().enumerate() {
                    handles.push(Handle {
                        grip: Grip::Knob(at),
                        points: vec![knob.at],
                        color: match knob.sized {
                            Sized::Side(which) => AXIS_COLORS[which],
                            Sized::Height | Sized::Tube => AXIS_COLORS[1],
                            Sized::Radius => AXIS_COLORS[0],
                        },
                    });
                }
            }
            (Tool::Paint, _) => {}
        }
        handles
    }

    /// Every handle that can be pressed on now.
    pub fn grips(&self) -> Vec<Grip> {
        if self.mode == Mode::Pose {
            return self.pose_grips();
        }
        self.gizmo().map_or_else(Vec::new, |gizmo| {
            self.handles_of(&gizmo)
                .into_iter()
                .map(|handle| handle.grip)
                .collect()
        })
    }

    /// A point of a handle to take hold of it by, where the picture
    /// shows it (the model's own place plus [`Studio::offset`]).
    pub fn grip_at(&self, grip: Grip) -> Option<Vec3> {
        if self.mode == Mode::Pose {
            return self.pose_grip_at(grip);
        }
        let gizmo = self.gizmo()?;
        let handle = self
            .handles_of(&gizmo)
            .into_iter()
            .find(|handle| handle.grip == grip)?;
        Some(match grip {
            // The box at the end of its line; the middle of an arrow.
            Grip::Box(_) => *handle.points.last()?,
            Grip::Arrow(_) | Grip::Pull => {
                let (from, to) = (*handle.points.first()?, *handle.points.last()?);
                from + (to - from) * 0.6
            }
            // The point of the ring nearest the eye.
            Grip::Ring(_) => {
                let eye = self.orbit.eye();
                *handle
                    .points
                    .iter()
                    .min_by(|a, b| (**a - eye).length().total_cmp(&(**b - eye).length()))?
            }
            _ => *handle.points.first()?,
        })
    }

    /// The same point on the screen: where to press, in pixels of the
    /// picture the pointer was last measured in.
    pub fn grip_point(&self, grip: Grip) -> Option<(f64, f64)> {
        self.orbit
            .on_screen(self.grip_at(grip)?, self.view.0, self.view.1)
    }

    /// The handle the pointer is on, and where along it.
    fn handle_under(&self, gizmo: &Gizmo, at: (f64, f64)) -> Option<Grip> {
        let (width, height) = self.view;
        let seen = |point: Vec3| self.orbit.on_screen(point, width, height);
        let away = |point: (f64, f64)| ((point.0 - at.0).powi(2) + (point.1 - at.1).powi(2)).sqrt();
        // How far the pointer is from a straight piece on the screen.
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
        self.handles_of(gizmo)
            .into_iter()
            .filter_map(|handle| {
                let distance = match handle.grip {
                    // Only the box itself, not the line it stands on.
                    Grip::Box(_) | Grip::Middle | Grip::Knob(_) | Grip::Inset => {
                        away(seen(*handle.points.last()?)?) - 2.0
                    }
                    _ => handle
                        .points
                        .windows(2)
                        .filter_map(|piece| Some(from_piece(seen(piece[0])?, seen(piece[1])?)))
                        .min_by(f64::total_cmp)?,
                };
                (distance <= REACH).then_some((distance, handle.grip))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, grip)| grip)
    }

    /// The pieces of the handles to draw over the model.
    pub(super) fn marks(&self, width: usize, height: usize) -> Vec<Mark> {
        if self.view != (width, height) {
            return Vec::new();
        }
        if self.mode == Mode::Pose {
            return self.pose_marks();
        }
        let Some(gizmo) = self.gizmo() else {
            return Vec::new();
        };
        let hot = match &self.grab {
            Some(grab) => Some(grab.grip),
            None => self
                .pointer
                .and_then(|pointer| self.handle_under(&gizmo, pointer)),
        };
        let mut marks = Vec::new();
        for handle in self.handles_of(&gizmo) {
            let color = if hot == Some(handle.grip) {
                HOT
            } else {
                handle.color
            };
            match handle.grip {
                Grip::Arrow(_) | Grip::Pull => marks.push(Mark::Arrow {
                    from: handle.points[0],
                    to: handle.points[1],
                    color,
                }),
                Grip::Inset => marks.push(Mark::Disc {
                    at: handle.points[0],
                    radius: 6.0,
                    color,
                }),
                Grip::Box(_) => {
                    marks.push(Mark::Line {
                        from: handle.points[0],
                        to: handle.points[1],
                        color,
                    });
                    marks.push(Mark::Block {
                        at: handle.points[1],
                        half: 5.0,
                        color,
                    });
                }
                Grip::Middle => marks.push(Mark::Block {
                    at: handle.points[0],
                    half: 6.0,
                    color,
                }),
                Grip::Ring(_) => {
                    for piece in handle.points.windows(2) {
                        marks.push(Mark::Line {
                            from: piece[0],
                            to: piece[1],
                            color,
                        });
                    }
                }
                Grip::Knob(_) => marks.push(Mark::Disc {
                    at: handle.points[0],
                    radius: 5.0,
                    color,
                }),
                Grip::Body => {}
            }
        }
        marks
    }

    /// Takes hold of what is under the pointer: a handle of the selected
    /// part, or (with the Move tool) the part itself. Says what was
    /// taken, or `None` if the pointer is on neither: then the drag
    /// turns the view.
    pub fn grab(&mut self, size: (usize, usize), at: (f64, f64)) -> Option<Grip> {
        self.hover(size, Some(at));
        self.grab = None;
        self.pose_grab = None;
        self.preview = None;
        self.dragging = None;
        if self.mode == Mode::Pose {
            return self.pose_grab_at(at);
        }
        let gizmo = self.gizmo()?;
        let facts = self.facts()?;
        let (eye, ray) = self.pointer_ray(at);
        let offset = self.offset();
        let (grip, hold, point) = match self.handle_under(&gizmo, at) {
            Some(grip) => {
                let hold = match grip {
                    Grip::Arrow(which) => {
                        let along = match &gizmo.joined {
                            Some(joined) => joined.across[which.min(1)],
                            None => gizmo.list[which],
                        };
                        nearest_on_line(gizmo.centre, along, eye, ray)?
                    }
                    Grip::Box(which) => nearest_on_line(gizmo.centre, gizmo.own[which], eye, ray)?,
                    Grip::Knob(which) => {
                        let knob = gizmo.knobs.get(which)?;
                        nearest_on_line(knob.at, knob.out, eye, ray)?
                    }
                    Grip::Pull => {
                        let face = gizmo.face.as_ref()?;
                        nearest_on_line(gizmo.centre, face.out, eye, ray)?
                    }
                    Grip::Inset => {
                        let face = gizmo.face.as_ref()?;
                        nearest_on_line(face.knob, face.inward, eye, ray)?
                    }
                    _ => 0.0,
                };
                (grip, hold, gizmo.centre)
            }
            None => {
                // The part itself, with the Move tool, if it has a name
                // (not in Face mode: there a drag off a handle turns the
                // view).
                if gizmo.face.is_some()
                    || self.tool != Tool::Move
                    || gizmo.part.rsplit('/').next()?.starts_with('#')
                {
                    return None;
                }
                let model = self.model()?;
                let hit = viewport::pick(model, &self.orbit, offset, size, at, &self.unseen())?;
                let on = |path: &str| {
                    self.part(path)
                        .is_some_and(|part| part.faces.contains(&hit.face))
                };
                match &gizmo.group {
                    // Any of the parts held together.
                    Some(group) => {
                        if !group.moved.iter().any(|member| on(&member.path)) {
                            return None;
                        }
                    }
                    None => {
                        if !on(&gizmo.part) {
                            return None;
                        }
                        self.dragging = Some(gizmo.part.clone());
                    }
                }
                (Grip::Body, 0.0, hit.point + offset)
            }
        };
        self.grab = Some(Grab {
            grip,
            gizmo,
            facts,
            hold,
            pressed: at,
            point,
            edits: Vec::new(),
            label: String::new(),
            problem: None,
        });
        Some(grip)
    }

    /// The handle in hand, if one is.
    pub fn grabbed(&self) -> Option<Grip> {
        self.grab
            .as_ref()
            .map(|grab| grab.grip)
            .or_else(|| self.pose_grabbed())
    }

    /// Why the drag in hand can't be made as it stands, if it can't:
    /// then its label is shown in red, and letting go writes nothing.
    pub fn drag_problem(&self) -> Option<&str> {
        self.grab.as_ref().and_then(|grab| grab.problem.as_deref())
    }

    /// What the drag has done so far, in a few words, to show by the
    /// pointer: `x 1.2 m`, `45°`.
    pub fn drag_label(&self) -> Option<&str> {
        if self.pose_grab_held() {
            return self.pose_drag_label();
        }
        self.grab
            .as_ref()
            .map(|grab| grab.label.as_str())
            .filter(|label| !label.is_empty())
    }

    /// Moves the handle in hand to the pointer. Works out the edit that
    /// would make, and shows what the recipe would then build. With
    /// `free`, nothing snaps.
    pub fn drag(&mut self, size: (usize, usize), at: (f64, f64), free: bool) {
        self.hover(size, Some(at));
        if self.pose_grab_held() {
            self.pose_drag_to(at, free);
            return;
        }
        let Some(grab) = &self.grab else {
            return;
        };
        let (eye, ray) = self.pointer_ray(at);
        let step = self.snap_step();
        let gizmo = &grab.gizmo;
        if gizmo.face.is_some() {
            let Some((edits, label)) = self.face_drag(grab, eye, ray, at, free) else {
                return;
            };
            self.hold_edits(edits, label);
            return;
        }
        if let Some(group) = &gizmo.group {
            let Some((edits, label)) = self.group_drag(grab, group, eye, ray, at, free) else {
                return;
            };
            self.hold_edits(edits, label);
            return;
        }
        let part = gizmo.part.clone();
        let set = |changes: Changes| Edit::Set {
            part: part.clone(),
            changes,
        };
        let axis_names = ["x", "y", "z"];
        let (edits, label): (Vec<Edit>, String) = match (grab.grip, &gizmo.joined) {
            // ---- a free part along one direction
            (Grip::Arrow(which), None) => {
                let Some(now) = nearest_on_line(gizmo.centre, gizmo.list[which], eye, ray) else {
                    return;
                };
                let mut to = grab.facts.position;
                // In steps from where it was, so a part that stood half a
                // step off the lines (a crate on a floor) stays so.
                to[which] += snapped((now - grab.hold) / gizmo.units[which], step, free);
                to[which] = (to[which] * 10_000.0).round() / 10_000.0 + 0.0;
                (
                    vec![set(Changes {
                        move_to: Some(to),
                        ..Changes::default()
                    })],
                    format!("{} {} m", axis_names[which], short(to[which])),
                )
            }
            // ---- a joined part across the surface it sits on
            (Grip::Arrow(which), Some(joined)) => {
                let across = joined.across[which.min(1)];
                let Some(now) = nearest_on_line(gizmo.centre, across, eye, ray) else {
                    return;
                };
                let slid = snapped(now - grab.hold, step, free);
                let to = joined.at + across * slid;
                let edits = self
                    .model()
                    .and_then(|model| {
                        let onto = part_in(&model.parts, &joined.target)?;
                        edit::spot(model, onto, to, joined.facing)
                    })
                    .map(|spot| self.slide_edits(&part, &joined.target, &spot))
                    .unwrap_or_default();
                (edits, format!("slid {} m", short(slid)))
            }
            // ---- a free part turned around one direction
            (Grip::Ring(which), None) => {
                let axis = gizmo.list[which];
                let Some(turned) = self.ring_turn(grab, axis, eye, ray, at) else {
                    return;
                };
                // Counted the way the recipe's `rotate` counts around this
                // direction, which for x is the other way round from
                // `Mat3::around`: try a quarter turn each way and see.
                let mut quarter = [0.0; 3];
                quarter[which] = 90.0;
                let mut unit = [0.0; 3];
                unit[which] = 1.0;
                let probe = vec3(0.3, 0.5, 0.7);
                let same = (Mat3::around(unit.into(), 90.0) * probe
                    - Mat3::from_rotate(quarter) * probe)
                    .length()
                    < 1e-6;
                let turned = turned * gizmo.sense * if same { 1.0 } else { -1.0 };
                let turned = snapped(turned, TURN_STEP, free);
                let mut by = [0.0; 3];
                by[which] = turned;
                let turn = Mat3::from_rotate(by) * Mat3::from_rotate(grab.facts.rotation);
                (
                    vec![set(Changes {
                        rotate: Some(turn.to_rotate()),
                        ..Changes::default()
                    })],
                    format!("{}°", short(turned)),
                )
            }
            // ---- a joined part twisted around its join
            (Grip::Ring(_), Some(joined)) => {
                let Some(turned) = self.ring_turn(grab, joined.facing, eye, ray, at) else {
                    return;
                };
                let turned = snapped(turned, TURN_STEP, free);
                // Kept between -180 and 180, as a person would write it.
                let mut twist = (grab.facts.twist + turned * gizmo.sense) % 360.0;
                if twist > 180.0 {
                    twist -= 360.0;
                } else if twist <= -180.0 {
                    twist += 360.0;
                }
                (
                    vec![set(Changes {
                        twist: Some(twist),
                        ..Changes::default()
                    })],
                    format!("twist {}°", short(twist)),
                )
            }
            // ---- bigger or smaller along one of its own sides
            (Grip::Box(which), _) => {
                let Some(now) = nearest_on_line(gizmo.centre, gizmo.own[which], eye, ray) else {
                    return;
                };
                if grab.hold.abs() < 1e-9 {
                    return;
                }
                let mut scale = grab.facts.scale;
                let least = if free { 0.01 } else { SCALE_STEP };
                scale[which] =
                    snapped(scale[which] * (now / grab.hold), SCALE_STEP, free).max(least);
                (
                    vec![set(Changes {
                        scale: Some(scale),
                        ..Changes::default()
                    })],
                    format!("{} x {}", axis_names[which], short(scale[which])),
                )
            }
            // ---- bigger or smaller all round: right and up grow it
            (Grip::Middle, _) => {
                let pulled = (at.0 - grab.pressed.0) - (at.1 - grab.pressed.1);
                let factor = (1.0 + pulled / 120.0).max(0.05);
                let least = if free { 0.01 } else { SCALE_STEP };
                let scale = grab
                    .facts
                    .scale
                    .map(|side| snapped(side * factor, SCALE_STEP, free).max(least));
                (
                    vec![set(Changes {
                        scale: Some(scale),
                        ..Changes::default()
                    })],
                    format!("x {}", short(factor)),
                )
            }
            // ---- the part itself
            (Grip::Body, joined) => {
                let markers = self.markers();
                match (self.aim(&markers), joined) {
                    // Dropped on a socket: joined there.
                    (Some(sockets::Aim::Marker(index)), _) => {
                        let marker = &markers[index];
                        (
                            vec![Edit::Join {
                                part: part.clone(),
                                to: format!("{}.{}", marker.part, marker.socket),
                                attach: None,
                            }],
                            format!("join {}.{}", marker.part, marker.socket),
                        )
                    }
                    // A joined part on a surface: slid there.
                    (
                        Some(sockets::Aim::Surface {
                            part: onto, spot, ..
                        }),
                        Some(_),
                    ) => (
                        self.slide_edits(&part, &onto, &spot),
                        format!("on the {} of {onto}", spot.on),
                    ),
                    // A free part: across the ground.
                    (_, None) => {
                        let up = gizmo.list[1];
                        let Some(now) = on_plane(grab.point, up, eye, ray) else {
                            return;
                        };
                        let moved = now - grab.point;
                        let mut to = grab.facts.position;
                        for which in [0, 2] {
                            to[which] += snapped(
                                moved.dot(gizmo.list[which]) / gizmo.units[which],
                                step,
                                free,
                            );
                            to[which] = (to[which] * 10_000.0).round() / 10_000.0 + 0.0;
                        }
                        (
                            vec![set(Changes {
                                move_to: Some(to),
                                ..Changes::default()
                            })],
                            format!("x {} m, z {} m", short(to[0]), short(to[2])),
                        )
                    }
                    (_, Some(_)) => (Vec::new(), String::new()),
                }
            }
            // (Face mode's handles are worked out above.)
            (Grip::Pull | Grip::Inset, _) => return,
            // ---- a face of a shape, pulled out or in
            (Grip::Knob(which), _) => {
                let Some(knob) = gizmo.knobs.get(which) else {
                    return;
                };
                let Some(now) = nearest_on_line(knob.at, knob.out, eye, ray) else {
                    return;
                };
                let was = &grab.facts.sizes;
                let (was_size, name) = match knob.sized {
                    Sized::Side(side) => (
                        was.size.map_or(1.0, |size| size[side]),
                        ["width", "height", "depth"][side],
                    ),
                    Sized::Height => (was.height.unwrap_or(1.0), "height"),
                    Sized::Radius => (was.radius.unwrap_or(0.5), "radius"),
                    Sized::Tube => (was.tube.unwrap_or(0.15), "tube"),
                };
                // Never down to nothing: a centimetre at the least.
                let grown = snapped((now - grab.hold) / knob.unit, step, free).max(0.01 - was_size);
                let is = ((was_size + grown) * 10_000.0).round() / 10_000.0;
                let mut sizes = edit::Sizes::default();
                match knob.sized {
                    Sized::Side(side) => {
                        let mut size = was.size.unwrap_or([1.0; 3]);
                        size[side] = is;
                        sizes.size = Some(size);
                    }
                    Sized::Height => sizes.height = Some(is),
                    Sized::Radius => sizes.radius = Some(is),
                    Sized::Tube => sizes.tube = Some(is),
                }
                // A free part's middle goes half as far, so the face
                // opposite stays where it is. A joined part is held by
                // its join.
                let carried = knob.carry * (is - was_size);
                let move_to = (grab.facts.to.is_none() && carried.length() > 1e-9).then(|| {
                    let at = Vec3::from(grab.facts.position) + carried;
                    at.to_array()
                        .map(|number| (number * 10_000.0).round() / 10_000.0 + 0.0)
                });
                (
                    vec![set(Changes {
                        sizes,
                        move_to,
                        ..Changes::default()
                    })],
                    format!("{name} {} m", short(is)),
                )
            }
        };
        self.hold_edits(edits, label);
    }

    /// The edits a drag of the handles makes when several parts are held
    /// together: one `set` for each part, and what to say by the
    /// pointer. Every part moves the same way; a turn or a new size
    /// also carries each part round, or away from, the middle of them
    /// all.
    fn group_drag(
        &self,
        grab: &Grab,
        group: &Group,
        eye: Vec3,
        ray: Vec3,
        at: (f64, f64),
        free: bool,
    ) -> Option<(Vec<Edit>, String)> {
        let gizmo = &grab.gizmo;
        let step = self.snap_step();
        let tidy = |number: f64| (number * 10_000.0).round() / 10_000.0 + 0.0;
        let signed = |number: f64| {
            if number > 0.0 {
                format!("+{}", short(number))
            } else {
                short(number)
            }
        };
        let axis_names = ["x", "y", "z"];
        // From the middle of them all to a part, measured in the list.
        let middle = gizmo.centre - self.offset();
        let arm = |member: &Member| {
            let across = member.at - middle;
            vec3(
                across.dot(gizmo.list[0]) / gizmo.units[0],
                across.dot(gizmo.list[1]) / gizmo.units[1],
                across.dot(gizmo.list[2]) / gizmo.units[2],
            )
        };
        let set = |member: &Member, changes: Changes| Edit::Set {
            part: member.path.clone(),
            changes,
        };
        let shifted = |member: &Member, by: Vec3| {
            let at = Vec3::from(member.facts.position) + by;
            Some(at.to_array().map(tidy))
        };
        Some(match grab.grip {
            Grip::Arrow(which) => {
                let now = nearest_on_line(gizmo.centre, gizmo.list[which], eye, ray)?;
                let by = snapped((now - grab.hold) / gizmo.units[which], step, free);
                let mut along = [0.0; 3];
                along[which] = by;
                (
                    group
                        .moved
                        .iter()
                        .map(|member| {
                            set(
                                member,
                                Changes {
                                    move_to: shifted(member, along.into()),
                                    ..Changes::default()
                                },
                            )
                        })
                        .collect(),
                    format!("{} {} m", axis_names[which], signed(by)),
                )
            }
            Grip::Body => {
                let now = on_plane(grab.point, gizmo.list[1], eye, ray)?;
                let moved = now - grab.point;
                let across = |which: usize| {
                    snapped(
                        moved.dot(gizmo.list[which]) / gizmo.units[which],
                        step,
                        free,
                    )
                };
                let by = vec3(across(0), 0.0, across(2));
                (
                    group
                        .moved
                        .iter()
                        .map(|member| {
                            set(
                                member,
                                Changes {
                                    move_to: shifted(member, by),
                                    ..Changes::default()
                                },
                            )
                        })
                        .collect(),
                    format!("x {} m, z {} m", signed(by.x), signed(by.z)),
                )
            }
            Grip::Ring(which) => {
                let turned = self.ring_turn(grab, gizmo.list[which], eye, ray, at)?;
                // Counted the way the recipe's `rotate` counts (see the
                // ring of one part).
                let mut quarter = [0.0; 3];
                quarter[which] = 90.0;
                let mut unit = [0.0; 3];
                unit[which] = 1.0;
                let probe = vec3(0.3, 0.5, 0.7);
                let same = (Mat3::around(unit.into(), 90.0) * probe
                    - Mat3::from_rotate(quarter) * probe)
                    .length()
                    < 1e-6;
                let turned = turned * gizmo.sense * if same { 1.0 } else { -1.0 };
                let turned = snapped(turned, TURN_STEP, free);
                let mut by = [0.0; 3];
                by[which] = turned;
                let turn = Mat3::from_rotate(by);
                (
                    group
                        .moved
                        .iter()
                        .map(|member| {
                            let out = arm(member);
                            set(
                                member,
                                Changes {
                                    rotate: Some(
                                        (turn * Mat3::from_rotate(member.facts.rotation))
                                            .to_rotate(),
                                    ),
                                    move_to: shifted(member, turn * out - out),
                                    ..Changes::default()
                                },
                            )
                        })
                        .collect(),
                    format!("{}°", short(turned)),
                )
            }
            Grip::Middle => {
                let pulled = (at.0 - grab.pressed.0) - (at.1 - grab.pressed.1);
                let least = if free { 0.01 } else { SCALE_STEP };
                let factor = snapped((1.0 + pulled / 120.0).max(0.05), SCALE_STEP, free).max(least);
                (
                    group
                        .moved
                        .iter()
                        .map(|member| {
                            set(
                                member,
                                Changes {
                                    scale: Some(
                                        member
                                            .facts
                                            .scale
                                            .map(|side| tidy(side * factor).max(0.001)),
                                    ),
                                    move_to: shifted(member, arm(member) * (factor - 1.0)),
                                    ..Changes::default()
                                },
                            )
                        })
                        .collect(),
                    format!("x {}", short(factor)),
                )
            }
            _ => return None,
        })
    }

    /// The edit a drag of a handle makes in Face mode, and what to say
    /// by the pointer.
    fn face_drag(
        &self,
        grab: &Grab,
        eye: Vec3,
        ray: Vec3,
        at: (f64, f64),
        free: bool,
    ) -> Option<(Vec<Edit>, String)> {
        let gizmo = &grab.gizmo;
        let face = gizmo.face.as_ref()?;
        let part = gizmo.part.clone();
        let faces = face.faces.clone();
        let step = self.snap_step();
        let tidy = |number: f64| (number * 10_000.0).round() / 10_000.0 + 0.0;
        let axis_names = ["x", "y", "z"];
        Some(match grab.grip {
            // ---- out or in
            Grip::Pull => {
                let now = nearest_on_line(gizmo.centre, face.out, eye, ray)?;
                let by = tidy(snapped((now - grab.hold) / face.out_unit, step, free));
                if by == 0.0 {
                    (Vec::new(), "pull 0 m".to_owned())
                } else {
                    (
                        vec![Edit::Pull {
                            part,
                            faces,
                            by,
                            name: None,
                        }],
                        format!(
                            "{} {} m",
                            if by < 0.0 { "in" } else { "out" },
                            short(by.abs())
                        ),
                    )
                }
            }
            // ---- a smaller face inside
            Grip::Inset => {
                let now = nearest_on_line(face.knob, face.inward, eye, ray)?;
                let by = tidy(snapped(
                    (now - grab.hold) / face.inward_unit,
                    INSET_STEP,
                    free,
                ));
                if by <= 0.0 {
                    (Vec::new(), "inset 0 m".to_owned())
                } else {
                    (
                        vec![Edit::Inset {
                            part,
                            faces,
                            by,
                            name: None,
                        }],
                        format!("inset {} m", short(by)),
                    )
                }
            }
            // ---- slid along one of the part's own directions
            Grip::Arrow(which) => {
                let now = nearest_on_line(gizmo.centre, gizmo.own[which], eye, ray)?;
                let moved = tidy(snapped((now - grab.hold) / gizmo.units[which], step, free));
                let mut by = [0.0; 3];
                by[which] = moved;
                let label = format!("{} {} m", axis_names[which], short(moved));
                if moved == 0.0 {
                    (Vec::new(), label)
                } else if self.mode == Mode::Edge
                    || (self.mode == Mode::Corner
                        && faces.iter().any(|name| name.matches('+').count() < 2))
                {
                    // (A corner whose faces have only two names between
                    // them is where those faces meet: an edge's name.)
                    (
                        vec![Edit::MoveEdge {
                            part,
                            edges: faces,
                            by,
                        }],
                        label,
                    )
                } else if self.mode == Mode::Corner {
                    (
                        vec![Edit::MoveCorner {
                            part,
                            corners: faces,
                            by,
                        }],
                        label,
                    )
                } else {
                    (vec![Edit::MoveFace { part, faces, by }], label)
                }
            }
            // ---- turned about their middle
            Grip::Ring(which) => {
                let turned = self.ring_turn(grab, gizmo.own[which], eye, ray, at)?;
                // Counted the way `rotate` and `turn` count: see the ring
                // of a part.
                let mut quarter = [0.0; 3];
                quarter[which] = 90.0;
                let mut unit = [0.0; 3];
                unit[which] = 1.0;
                let probe = vec3(0.3, 0.5, 0.7);
                let same = (Mat3::around(unit.into(), 90.0) * probe
                    - Mat3::from_rotate(quarter) * probe)
                    .length()
                    < 1e-6;
                let turned = turned * gizmo.sense * if same { 1.0 } else { -1.0 };
                let turned = tidy(snapped(turned, TURN_STEP, free));
                let mut by = [0.0; 3];
                by[which] = turned;
                let label = format!("{}°", short(turned));
                if turned == 0.0 {
                    (Vec::new(), label)
                } else {
                    (vec![Edit::TurnFace { part, faces, by }], label)
                }
            }
            // ---- bigger or smaller one way
            Grip::Box(which) => {
                let now = nearest_on_line(gizmo.centre, gizmo.own[which], eye, ray)?;
                if grab.hold.abs() < 1e-9 {
                    return None;
                }
                let least = if free { 0.01 } else { SCALE_STEP };
                let factor = tidy(snapped(now / grab.hold, SCALE_STEP, free).max(least));
                let mut by = [1.0; 3];
                by[which] = factor;
                let label = format!("{} x {}", axis_names[which], short(factor));
                if factor == 1.0 {
                    (Vec::new(), label)
                } else {
                    (vec![Edit::ScaleFace { part, faces, by }], label)
                }
            }
            // ---- bigger or smaller all round: right and up grow them
            Grip::Middle => {
                let pulled = (at.0 - grab.pressed.0) - (at.1 - grab.pressed.1);
                let least = if free { 0.01 } else { SCALE_STEP };
                let factor =
                    tidy(snapped((1.0 + pulled / 120.0).max(0.05), SCALE_STEP, free).max(least));
                let label = format!("x {}", short(factor));
                if factor == 1.0 {
                    (Vec::new(), label)
                } else {
                    (
                        vec![Edit::ScaleFace {
                            part,
                            faces,
                            by: [factor; 3],
                        }],
                        label,
                    )
                }
            }
            _ => return None,
        })
    }

    /// How far a ring has been turned, in degrees, counted as
    /// [`Mat3::around`] counts around `axis`.
    fn ring_turn(
        &self,
        grab: &Grab,
        axis: Vec3,
        eye: Vec3,
        ray: Vec3,
        at: (f64, f64),
    ) -> Option<f64> {
        let centre = grab.gizmo.centre;
        // Seen nearly edge on, the ring is a line: then left and right
        // turn it, half a degree to the pixel.
        let facing_us = (eye - centre).normalized()?.dot(axis).abs();
        if facing_us < 0.2 {
            return Some((at.0 - grab.pressed.0) * 0.5);
        }
        let (_, first_ray) = self.pointer_ray(grab.pressed);
        let from = on_plane(centre, axis, eye, first_ray)? - centre;
        let to = on_plane(centre, axis, eye, ray)? - centre;
        Some(turn_between(axis, from.normalized()?, to.normalized()?))
    }

    /// Keeps the edits a drag would make, and shows what they build.
    fn hold_edits(&mut self, edits: Vec<Edit>, label: String) {
        let Some(grab) = &mut self.grab else {
            return;
        };
        grab.label = label;
        if grab.edits == edits {
            return;
        }
        grab.edits = edits;
        grab.problem = None;
        self.preview = None;
        let edits = grab.edits.clone();
        if edits.is_empty() {
            return;
        }
        let Some(syntax) = Syntax::from_path(&self.path) else {
            return;
        };
        match edit::apply_many(&self.text, syntax, &self.path, &self.shown_as, &edits) {
            Ok(applied) => self.preview = applied.model,
            Err(failure) => {
                let why = self.shorten(&first_error(&failure));
                if let Some(grab) = &mut self.grab {
                    grab.problem = Some(why);
                }
            }
        }
    }

    /// The edits that put `dragged` at a point of `onto`'s surface. If it
    /// sits on a socket of its own there, that socket is moved;
    /// otherwise a socket is made and the part joined to it.
    pub(super) fn slide_edits(&self, dragged: &str, onto: &str, spot: &edit::Spot) -> Vec<Edit> {
        let own_name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
        let joined = self.part(dragged).and_then(|part| part.attached_to.clone());
        let its_own = joined.as_deref().and_then(|to| {
            let (target, socket) = to.rsplit_once('.')?;
            let alone = self
                .list_of(dragged)
                .iter()
                .filter(|part| part.attached_to.as_deref() == Some(to))
                .count()
                == 1;
            (target == own_name(onto) && alone).then(|| socket.to_owned())
        });
        if let (Some(socket), Some(syntax)) = (its_own, Syntax::from_path(&self.path)) {
            let moved = vec![Edit::SetSocket {
                socket: format!("{onto}.{socket}"),
                on: Some(spot.on.clone()),
                shift: Some(spot.shift),
                facing: None,
            }];
            // A built-in socket (`top`) can't be moved: then a new one
            // is made.
            if edit::apply_many(&self.text, syntax, &self.path, &self.shown_as, &moved).is_ok() {
                return moved;
            }
        }
        let socket = self.new_socket_name(onto);
        vec![
            Edit::AddSocket {
                part: onto.to_owned(),
                name: socket.clone(),
                on: spot.on.clone(),
                shift: spot.shift,
            },
            Edit::Join {
                part: dragged.to_owned(),
                to: format!("{onto}.{socket}"),
                attach: None,
            },
        ]
    }

    /// Lets go of the handle and makes the edit: written once, one step
    /// to undo. `None` if nothing was in hand, or the pointer never
    /// moved it.
    pub fn release(&mut self) -> Option<Result<String, String>> {
        if self.pose_grab_held() {
            return self.pose_let_go();
        }
        let grab = self.grab.take()?;
        let before = self.preview.take();
        self.dragging = None;
        if let Some(why) = grab.problem {
            let why = format!("Not changed: {why}");
            self.say(why.clone());
            return Some(Err(why));
        }
        if grab.edits.is_empty() {
            // A joined part let go over nothing: say what it can be
            // dropped on.
            if grab.grip == Grip::Body && grab.gizmo.joined.is_some() {
                let why = "Not moved: drop it on a socket, or on another part".to_owned();
                self.say(why.clone());
                return Some(Err(why));
            }
            return None;
        }
        // Where the part was shown, to see whether lifting it showed
        // anything.
        let height = |studio: &Studio| {
            studio
                .part(&grab.gizmo.part)
                .and_then(|part| part.bounds)
                .map(|bounds| bounds.min.y)
        };
        let was = height(self);
        drop(before);
        if let Some(group) = &grab.gizmo.group {
            // Several parts: said in a few words, with what stayed.
            let what = match grab.grip {
                Grip::Ring(_) => "Turned",
                Grip::Middle => "Scaled",
                _ => "Moved",
            };
            let mut said = format!("{what} {} parts: {}", group.moved.len(), grab.label);
            if !group.stayed.is_empty() {
                said.push_str(&format!(
                    ". Not moved: {} (joined to a part that isn't selected, or inside another part)",
                    group.stayed.join(", ")
                ));
            }
            return Some(self.apply_as(&grab.edits, Some(said)));
        }
        if grab.grip == Grip::Inset {
            // The new faces in the middle are what is in hand next.
            let before: Vec<String> = self
                .shaped_selected()
                .map(|(_, shaped)| shaped.own_names())
                .unwrap_or_default();
            let done = self.apply_all(&grab.edits);
            if done.is_ok()
                && let Some((_, shaped)) = self.shaped_selected()
            {
                let middles: Vec<String> = shaped
                    .own_names()
                    .into_iter()
                    .filter(|name| !before.contains(name) && !name.contains("_rim_"))
                    .collect();
                if !middles.is_empty() {
                    self.faces = middles;
                }
            }
            return Some(done);
        }
        let done = self.apply_all(&grab.edits);
        if done.is_ok()
            && grab.grip == Grip::Arrow(1)
            && grab.gizmo.joined.is_none()
            && let (Some(was), Some(is)) = (was, height(self))
            && was.abs() < 1e-6
            && is.abs() < 1e-6
        {
            // It was on the ground and still is: the ground is always
            // under the lowest part.
            let others = self.model().is_some_and(|model| model.leaves().len() > 1);
            let why = if others {
                "It is the lowest part, so it stays on the ground and the others come down instead"
            } else {
                "It is the only part, and a model always stands on the ground, so lifting it shows nothing"
            };
            let said = format!("{}. {why}", self.status());
            self.say(said.clone());
            return Some(Ok(said));
        }
        Some(done)
    }

    /// Lets go of the handle without changing anything.
    pub fn cancel_drag(&mut self) {
        self.grab = None;
        self.pose_grab = None;
        self.preview = None;
        self.dragging = None;
    }
}
