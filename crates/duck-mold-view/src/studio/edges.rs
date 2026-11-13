//! Edge mode, and the Cut tool.
//!
//! In Edge mode (E) a click picks an edge of the selected box, by its
//! name: the faces that meet there, joined with `+` (`top+front`). The
//! Move arrows then move it: a `move-edge` edit.
//!
//! The Cut tool (K, in Face and Edge mode) shows a ring all the way
//! round the part where the pointer is: across the edge under it, flat,
//! facing the way that edge mostly runs. A click cuts there: a `cut`
//! edit, `at` a share of the way from the middle to that side, in
//! tenths (Ctrl: free).

use duck_mold_core::edit::Edit;
use duck_mold_core::math::Vec3;
use duck_mold_core::model::Part;
use duck_mold_core::recipe::Side;
use duck_mold_core::shaping::Shaped;

use super::Studio;
use super::shape::{FaceTool, Mode};
use super::viewport::Mark;

/// How near the pointer must be to an edge to pick it, in pixels.
const EDGE_REACH: f64 = 8.0;
/// How near it must be for the Cut tool to follow that edge.
const CUT_REACH: f64 = 40.0;
/// The edges' lines.
const LINE: [u8; 3] = [0xB8, 0xBE, 0xCC];
/// Duck yellow: what is selected.
const CHOSEN: [u8; 3] = [0xFF, 0xCC, 0x2E];
/// The edge under the pointer, and the cut ring.
const HOT: [u8; 3] = [0xFF, 0xE8, 0x9A];

/// Where a click of the Cut tool would cut.
#[derive(Debug, Clone, PartialEq)]
pub struct CutAim {
    /// The side the cut faces.
    pub toward: Side,
    /// How far from the middle (0) to that side (just under 1).
    pub at: f64,
    /// The ring round the part, as pieces of line where the picture
    /// shows them.
    pub ring: Vec<[Vec3; 2]>,
}

/// The distance from `point` to the piece of line from `a` to `b`, on
/// the screen.
fn to_piece(point: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let long = dx * dx + dy * dy;
    let along = if long < 1e-9 {
        0.0
    } else {
        (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / long).clamp(0.0, 1.0)
    };
    ((a.0 + dx * along - point.0).powi(2) + (a.1 + dy * along - point.1).powi(2)).sqrt()
}

impl Studio {
    /// A point of the box's own space, where the picture shows it.
    fn seen_point(&self, part: &Part, point: Vec3) -> Vec3 {
        part.place.apply(point) + self.offset()
    }

    /// The edges of the box being shaped that look this way (one of
    /// their two faces does), each with its name and its two ends where
    /// the picture shows them.
    fn seen_edges(&self) -> Vec<(String, [Vec3; 2])> {
        let Some((part, shaped)) = self.shaped_selected() else {
            return Vec::new();
        };
        let eye = self.orbit.eye();
        shaped
            .edges()
            .iter()
            .filter_map(|edge| {
                let [a, b] = edge
                    .corners
                    .map(|corner| self.seen_point(part, shaped.corners[corner]));
                let looks = edge.faces.iter().any(|&face| {
                    shaped.normal(face).is_some_and(|normal| {
                        part.place.facing(normal).dot(eye - (a + b) * 0.5) > 0.0
                    })
                });
                looks.then(|| (shaped.edge_name(edge), [a, b]))
            })
            .collect()
    }

    /// The edge of the box being shaped under the pointer, by name.
    pub fn edge_at(&self, size: (usize, usize), at: (f64, f64)) -> Option<String> {
        if self.mode != Mode::Edge {
            return None;
        }
        let seen = |point: Vec3| self.orbit.on_screen(point, size.0, size.1);
        self.seen_edges()
            .into_iter()
            .filter_map(|(name, [a, b])| Some((to_piece(at, seen(a)?, seen(b)?), name)))
            .filter(|(distance, _)| *distance <= EDGE_REACH)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, name)| name)
    }

    /// The corners of the box being shaped that look this way (one of
    /// the faces round them does), each with its name and where the
    /// picture shows it.
    fn seen_corners(&self) -> Vec<(String, Vec3)> {
        let Some((part, shaped)) = self.shaped_selected() else {
            return Vec::new();
        };
        let eye = self.orbit.eye();
        (0..shaped.corners.len())
            .filter_map(|corner| {
                let at = self.seen_point(part, shaped.corners[corner]);
                let looks = shaped.faces.iter().enumerate().any(|(face, known)| {
                    known.corners.contains(&corner)
                        && shaped
                            .normal(face)
                            .is_some_and(|normal| part.place.facing(normal).dot(eye - at) > 0.0)
                });
                looks.then(|| (shaped.corner_name(corner), at))
            })
            .collect()
    }

    /// The corner of the box being shaped under the pointer, by name.
    pub fn corner_at(&self, size: (usize, usize), at: (f64, f64)) -> Option<String> {
        if self.mode != Mode::Corner {
            return None;
        }
        self.seen_corners()
            .into_iter()
            .filter_map(|(name, point)| {
                let (x, y) = self.orbit.on_screen(point, size.0, size.1)?;
                Some((((x - at.0).powi(2) + (y - at.1).powi(2)).sqrt(), name))
            })
            .filter(|(distance, _)| *distance <= EDGE_REACH + 2.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, name)| name)
    }

    /// Corner mode's dots: every corner that looks this way; the one
    /// under the pointer, and those selected, bigger.
    pub(super) fn corner_marks(&self) -> Vec<Mark> {
        let hot = match (&self.grab, self.pointer) {
            (None, Some(pointer)) => self.corner_at(self.view, pointer),
            _ => None,
        };
        let mut small = Vec::new();
        let mut big = Vec::new();
        for (name, at) in self.seen_corners() {
            if self.faces.contains(&name) {
                big.push(Mark::Disc {
                    at,
                    radius: 5.0,
                    color: CHOSEN,
                });
            } else if hot.as_ref() == Some(&name) {
                big.push(Mark::Disc {
                    at,
                    radius: 5.0,
                    color: HOT,
                });
            } else {
                small.push(Mark::Disc {
                    at,
                    radius: 3.0,
                    color: LINE,
                });
            }
        }
        // The edges, thin, so the dots read as corners of something.
        let mut marks: Vec<Mark> = self
            .seen_edges()
            .into_iter()
            .map(|(_, [from, to])| Mark::Thin {
                from,
                to,
                color: LINE,
            })
            .collect();
        marks.extend(small);
        marks.extend(big);
        marks
    }

    /// Where on the screen a corner of the box being shaped is drawn.
    /// For tests.
    pub fn corner_point(&self, name: &str) -> Option<(f64, f64)> {
        let (_, at) = self
            .seen_corners()
            .into_iter()
            .find(|(known, _)| known == name)?;
        self.orbit.on_screen(at, self.view.0, self.view.1)
    }

    /// Edge mode's lines: every edge that looks this way, thin; the one
    /// under the pointer, and those selected, thick.
    pub(super) fn edge_marks(&self) -> Vec<Mark> {
        let hot = match (&self.grab, self.pointer) {
            (None, Some(pointer)) if self.face_tool != FaceTool::Cut => {
                self.edge_at(self.view, pointer)
            }
            _ => None,
        };
        let mut thin = Vec::new();
        let mut thick = Vec::new();
        for (name, [from, to]) in self.seen_edges() {
            if self.faces.contains(&name) {
                thick.push(Mark::Line {
                    from,
                    to,
                    color: CHOSEN,
                });
            } else if hot.as_ref() == Some(&name) {
                thick.push(Mark::Line {
                    from,
                    to,
                    color: HOT,
                });
            } else {
                thin.push(Mark::Thin {
                    from,
                    to,
                    color: LINE,
                });
            }
        }
        thin.extend(thick);
        thin
    }

    /// Where the Cut tool would cut, with the pointer at `at`: across the
    /// edge nearest it, flat, facing the box's own direction that edge
    /// mostly runs along. In tenths of the way from the middle to the
    /// side, unless `free`.
    pub fn cut_aim(&self, at: (f64, f64), free: bool) -> Option<CutAim> {
        if self.face_tool != FaceTool::Cut || self.mode == Mode::Part {
            return None;
        }
        let (part, shaped) = self.shaped_selected()?;
        let size = self.facts()?.sizes.size?;
        let (width, height) = self.view;
        let seen = |point: Vec3| self.orbit.on_screen(point, width, height);
        let eye = self.orbit.eye();
        // The edge nearest the pointer.
        let (edge, _) = shaped
            .edges()
            .into_iter()
            .filter_map(|edge| {
                let [a, b] = edge
                    .corners
                    .map(|corner| self.seen_point(part, shaped.corners[corner]));
                let looks = edge.faces.iter().any(|&face| {
                    shaped.normal(face).is_some_and(|normal| {
                        part.place.facing(normal).dot(eye - (a + b) * 0.5) > 0.0
                    })
                });
                if !looks {
                    return None;
                }
                Some((edge, to_piece(at, seen(a)?, seen(b)?)))
            })
            .filter(|(_, distance)| *distance <= CUT_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let [from, to] = edge.corners.map(|corner| shaped.corners[corner]);
        let run = to - from;
        let axis = [run.x.abs(), run.y.abs(), run.z.abs()]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(at, _)| at)?;
        // Where along the edge the pointer is.
        let (eye, ray) = {
            let eye = self.orbit.eye();
            let through = self
                .orbit
                .from_view(super::viewport::ray_through(self.view, at));
            (eye, through - eye)
        };
        let (a, b) = (self.seen_point(part, from), self.seen_point(part, to));
        let long = (b - a).length();
        let along = (b - a).normalized()?;
        let between = a - eye;
        let (bb, cc) = (along.dot(ray), ray.dot(ray));
        let (dd, ee) = (along.dot(between), ray.dot(between));
        let bottom = cc - bb * bb;
        let share = if bottom.abs() > 1e-12 {
            ((bb * ee - cc * dd) / bottom / long).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let point = from + run * share;
        let value = point.to_array()[axis];
        let half = size[axis] / 2.0;
        let sides = [
            [Side::Right, Side::Left],
            [Side::Top, Side::Bottom],
            [Side::Front, Side::Back],
        ];
        let toward = sides[axis][usize::from(value < 0.0)];
        let raw = (value.abs() / half.max(1e-9)).min(0.999);
        let at = if free {
            (raw * 1000.0).round() / 1000.0
        } else {
            ((raw * 10.0).round() / 10.0).min(0.9)
        };
        let place = at * half * if value < 0.0 { -1.0 } else { 1.0 };
        Some(CutAim {
            toward,
            at,
            ring: self.ring(part, &shaped, axis, place),
        })
    }

    /// Where the flat cut through `place` along the box's own `axis`
    /// meets the box's faces: pieces of line, where the picture shows
    /// them.
    fn ring(&self, part: &Part, shaped: &Shaped, axis: usize, place: f64) -> Vec<[Vec3; 2]> {
        let mut ring = Vec::new();
        for face in &shaped.faces {
            let points: Vec<Vec3> = face
                .corners
                .iter()
                .map(|&corner| shaped.corners[corner])
                .collect();
            let count = points.len();
            let mut crossing = Vec::new();
            for at in 0..count {
                let (p, q) = (points[at], points[(at + 1) % count]);
                let (dp, dq) = (p.to_array()[axis] - place, q.to_array()[axis] - place);
                if (dp < 0.0) != (dq < 0.0) && (dp - dq).abs() > 1e-12 {
                    crossing.push(p + (q - p) * (dp / (dp - dq)));
                }
            }
            if let [a, b] = crossing.as_slice() {
                ring.push([self.seen_point(part, *a), self.seen_point(part, *b)]);
            }
        }
        ring
    }

    /// The cut ring where the pointer is, to draw.
    pub(super) fn cut_marks(&self) -> Vec<Mark> {
        let Some(pointer) = self.pointer else {
            return Vec::new();
        };
        if self.grab.is_some() {
            return Vec::new();
        }
        self.cut_aim(pointer, false)
            .map(|aim| {
                aim.ring
                    .into_iter()
                    .map(|[from, to]| Mark::Line {
                        from,
                        to,
                        color: HOT,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// What the cut ring says by the pointer: `cut right 0.8`.
    pub fn cut_label(&self) -> Option<String> {
        let aim = self.cut_aim(self.pointer?, false)?;
        Some(format!(
            "cut {} {}",
            aim.toward.name(),
            (aim.at * 1000.0).round() / 1000.0
        ))
    }

    /// A click of the Cut tool: cuts where the ring is.
    pub fn cut_at(
        &mut self,
        size: (usize, usize),
        at: (f64, f64),
        free: bool,
    ) -> Result<String, String> {
        self.hover(size, Some(at));
        let Some(aim) = self.cut_aim(at, free) else {
            let why = "Nothing to cut here: point at an edge of the box".to_owned();
            self.say(why.clone());
            return Err(why);
        };
        let part = self.selected.clone().unwrap_or_default();
        self.apply(&Edit::Cut {
            part,
            toward: aim.toward.name().to_owned(),
            at: aim.at,
            name: None,
        })
    }

    /// Where on the screen an edge of the box being shaped is drawn: its
    /// middle. For tests.
    pub fn edge_point(&self, name: &str) -> Option<(f64, f64)> {
        let (_, [a, b]) = self
            .seen_edges()
            .into_iter()
            .find(|(known, _)| known == name)?;
        self.orbit
            .on_screen((a + b) * 0.5, self.view.0, self.view.1)
    }
}
