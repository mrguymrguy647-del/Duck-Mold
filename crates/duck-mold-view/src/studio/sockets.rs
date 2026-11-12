//! Sockets in the Studio: the markers in the viewport, joining a new
//! part to one, and dragging a part to another.
//!
//! - The selected part's sockets show as small markers. Clicking one, or
//!   any point of the selected part's surface, is a place to add a part.
//! - Dragging the selected part shows the sockets of the parts it could
//!   join. Dropping it on one joins it there; dropping it on a part's
//!   surface makes a socket at that point and joins it there. A part
//!   that already sits on a socket of its own on that part slides: the
//!   socket is moved, not a new one made.
//!
//! All of it becomes `add`, `join`, `add-socket` and `set-socket` edits.

use duck_mold_core::edit::{self, Edit, Made, Sizes, Spot};
use duck_mold_core::math::Vec3;
use duck_mold_core::model::Part;

use super::Studio;
use super::viewport::{self, MARKER_REACH, Marker};

/// What a click in the viewport did, or found.
#[derive(Debug, Clone, PartialEq)]
pub enum Clicked {
    /// It was on nothing: nothing is selected now.
    Nothing,
    /// It selected this part.
    Selected(String),
    /// It was on a socket of the selected part: a part can be added
    /// there.
    Socket {
        /// The part, by path.
        part: String,
        /// The socket's name.
        socket: String,
    },
    /// It was on the selected part's surface: a socket can be made
    /// there, and a part added to it.
    Surface {
        /// The part, by path.
        part: String,
        /// Where on it.
        spot: Spot,
    },
}

/// What the pointer is on, that something can be joined to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Aim {
    /// One of the markers shown, by its place in the list.
    Marker(usize),
    /// A point of a part's surface.
    Surface {
        part: String,
        spot: Spot,
        at: Vec3,
        facing: Vec3,
    },
}

/// The list a part is in, and the path of whatever that list belongs to
/// (`None` for the model's own list).
fn around(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(around, _)| around)
}

fn own_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl Studio {
    /// Tells the Studio where the pointer is in the viewport, in pixels
    /// of a picture `size`, or that it has left.
    pub fn hover(&mut self, size: (usize, usize), at: Option<(f64, f64)>) {
        self.view = size;
        self.pointer = at;
    }

    /// The part being dragged, if one is.
    pub fn dragging(&self) -> Option<&str> {
        self.dragging.as_deref()
    }

    /// The parts in the same list as `path`.
    fn list_of(&self, path: &str) -> &[Part] {
        match around(path) {
            Some(around) => self.part(around).map(|part| part.parts.as_slice()),
            None => self.model().map(|model| model.parts.as_slice()),
        }
        .unwrap_or_default()
    }

    /// The parts `path` could be joined to: those in its list, but not
    /// itself, not one with no name, and not one that hangs from it.
    fn could_join(&self, path: &str) -> Vec<&Part> {
        let list = self.list_of(path);
        let mut hanging = vec![own_name(path).to_owned()];
        loop {
            let more: Vec<String> = list
                .iter()
                .filter(|part| !hanging.contains(&part.name))
                .filter(|part| {
                    part.attached_to
                        .as_deref()
                        .and_then(|to| to.rsplit_once('.'))
                        .is_some_and(|(target, _)| hanging.iter().any(|name| name == target))
                })
                .map(|part| part.name.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            hanging.extend(more);
        }
        list.iter()
            .filter(|part| !hanging.contains(&part.name) && !part.name.starts_with('#'))
            .collect()
    }

    /// The sockets to show: the selected part's, or, while a part is
    /// dragged, those of the parts it could join. The one in the middle
    /// of a part is left out: it is inside it.
    pub fn markers(&self) -> Vec<Marker> {
        if self.look_only {
            return Vec::new();
        }
        let of = |part: &Part| {
            part.sockets
                .iter()
                .filter(|socket| socket.name != "center")
                .map(|socket| Marker {
                    part: part.path.clone(),
                    socket: socket.name.clone(),
                    at: socket.at,
                    facing: socket.facing,
                })
                .collect::<Vec<_>>()
        };
        match (&self.dragging, &self.selected) {
            (Some(dragged), _) => self.could_join(dragged).into_iter().flat_map(of).collect(),
            (None, Some(selected)) if self.facts().is_some() => {
                self.part(selected).map(of).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    /// What the pointer is on: the nearest marker within reach, or else a
    /// point of a surface that a socket could be made on (the selected
    /// part's, or, while dragging, that of a part the dragged one could
    /// join).
    pub(super) fn aim(&self, markers: &[Marker]) -> Option<Aim> {
        let (x, y) = self.pointer?;
        let (width, height) = self.view;
        // A marker on the far side of its part is drawn, but small, and
        // isn't clicked: turn the view to reach it.
        let eye = self.orbit.eye();
        let near = markers
            .iter()
            .enumerate()
            .filter(|(_, marker)| marker.facing.dot(eye - marker.at) >= 0.0)
            .filter_map(|(at, marker)| {
                let (mx, my) = self.orbit.on_screen(marker.at, width, height)?;
                let away = ((mx - x).powi(2) + (my - y).powi(2)).sqrt();
                (away <= MARKER_REACH).then_some((away, at))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, at)) = near {
            return Some(Aim::Marker(at));
        }
        if self.look_only {
            return None;
        }
        let model = self.model()?;
        let selected = self.selected.as_deref()?;
        let target = match &self.dragging {
            Some(dragged) => {
                // Through the dragged part, to what is behind it.
                let skip = self.part(dragged)?.faces.clone();
                let hit = viewport::pick(model, &self.orbit, self.view, (x, y), Some(&skip))?;
                let onto = self
                    .could_join(dragged)
                    .into_iter()
                    .find(|part| part.faces.contains(&hit.face))?;
                (onto, hit)
            }
            None => {
                let hit = viewport::pick(model, &self.orbit, self.view, (x, y), None)?;
                let part = self.part(selected)?;
                // The selected part itself, when no part inside it would
                // be selected by the click instead.
                let innermost = !part
                    .parts
                    .iter()
                    .any(|inner| inner.faces.contains(&hit.face));
                if !part.faces.contains(&hit.face) || !innermost || self.facts().is_none() {
                    return None;
                }
                (part, hit)
            }
        };
        let (part, hit) = target;
        let spot = edit::spot(model, part, hit.point, hit.normal)?;
        Some(Aim::Surface {
            part: part.path.clone(),
            spot,
            at: hit.point,
            facing: hit.normal,
        })
    }

    /// A click in the viewport at (`x`, `y`) of a picture `size`. On a
    /// socket of the selected part, or on its surface, it finds a place
    /// to add a part, and selects nothing new. Anywhere else it selects,
    /// as [`Studio::click`] does.
    pub fn point_at(&mut self, size: (usize, usize), at: (f64, f64)) -> Clicked {
        self.hover(size, Some(at));
        let markers = self.markers();
        match self.aim(&markers) {
            Some(Aim::Marker(index)) => Clicked::Socket {
                part: markers[index].part.clone(),
                socket: markers[index].socket.clone(),
            },
            Some(Aim::Surface { part, spot, .. }) => Clicked::Surface { part, spot },
            None => match self.click(size, at) {
                Some(path) => Clicked::Selected(path.to_owned()),
                None => Clicked::Nothing,
            },
        }
    }

    /// A name for a new part next to `beside`: `base`, or `base_2`...
    fn new_part_name(&self, beside: &str, base: &str) -> String {
        let taken: Vec<String> = self
            .list_of(beside)
            .iter()
            .map(|part| part.name.clone())
            .collect();
        // Only letters, digits, `_` and `-` can be in a name.
        let base: String = base
            .chars()
            .map(|letter| {
                if letter.is_ascii_alphanumeric() || letter == '_' || letter == '-' {
                    letter
                } else {
                    '_'
                }
            })
            .collect();
        edit::unused_name(if base.is_empty() { "part" } else { &base }, &taken)
    }

    /// The edit that adds a part made of `made` to a socket. A new shape
    /// is about half as big as the part it goes on, and has its colour.
    fn adding(&self, part: &str, socket: &str, made: &Made) -> Edit {
        let base = match made {
            Made::Shape(shape) => shape.clone(),
            Made::Use(defined) => defined.clone(),
            Made::File(file) => std::path::Path::new(file).file_stem().map_or_else(
                || "part".to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            ),
        };
        let mut sizes = Sizes::default();
        let mut color = None;
        if let Made::Shape(shape) = made {
            let across = self
                .part(part)
                .and_then(|part| part.bounds)
                .map_or(1.0, |bounds| {
                    let size = bounds.size();
                    size.x.min(size.y).min(size.z)
                });
            // Half the part's smallest side, to two figures.
            let across = (across * 0.5).max(0.01);
            let unit = 10f64.powf(across.log10().floor() - 1.0);
            let across = ((across / unit).round() * unit * 1e6).round() / 1e6;
            let radius = ((across / 2.0) * 1e6).round() / 1e6;
            match shape.as_str() {
                "box" | "wedge" => sizes.size = Some([across; 3]),
                "sphere" => sizes.radius = Some(radius),
                _ => {
                    sizes.radius = Some(radius);
                    sizes.height = Some(across);
                }
            }
            let syntax = duck_mold_core::source::Syntax::from_path(&self.path);
            color = syntax
                .and_then(|syntax| edit::describe(&self.text, syntax, self.model(), part))
                .and_then(|facts| facts.color);
        }
        Edit::Add {
            name: self.new_part_name(part, &base),
            made: made.clone(),
            to: format!("{part}.{socket}"),
            attach: None,
            sizes,
            color,
            twist: None,
        }
    }

    /// A name for a new socket on `part`: `spot`, `spot_2`...
    fn new_socket_name(&self, part: &str) -> String {
        let taken: Vec<String> = self
            .part(part)
            .map(|part| {
                part.sockets
                    .iter()
                    .map(|socket| socket.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        edit::unused_name("spot", &taken)
    }

    /// Adds a part made of `made`, joined to a socket: an `add` edit. The
    /// new part is selected.
    pub fn add_to_socket(
        &mut self,
        part: &str,
        socket: &str,
        made: &Made,
    ) -> Result<String, String> {
        let add = self.adding(part, socket, made);
        self.apply(&add)
    }

    /// Adds a part made of `made` at a point of a part's surface: an
    /// `add-socket` edit for a socket there, then an `add` edit. One step
    /// to undo.
    pub fn add_to_surface(
        &mut self,
        part: &str,
        spot: &Spot,
        made: &Made,
    ) -> Result<String, String> {
        let socket = self.new_socket_name(part);
        let add = self.adding(part, &socket, made);
        self.apply_all(&[
            Edit::AddSocket {
                part: part.to_owned(),
                name: socket,
                on: spot.on.clone(),
                shift: spot.shift,
            },
            add,
        ])
    }

    /// Starts dragging the selected part, if the pointer is on it and it
    /// can be moved. True if a drag began.
    pub fn drag_start(&mut self, size: (usize, usize), at: (f64, f64)) -> bool {
        self.hover(size, Some(at));
        if self.look_only || self.facts().is_none() {
            return false;
        }
        let Some(selected) = self.selected.clone() else {
            return false;
        };
        if own_name(&selected).starts_with('#') {
            return false;
        }
        let on_it = self.model().is_some_and(|model| {
            viewport::pick(model, &self.orbit, size, at, None).is_some_and(|hit| {
                self.part(&selected)
                    .is_some_and(|part| part.faces.contains(&hit.face))
            })
        });
        if on_it {
            self.dragging = Some(selected);
        }
        on_it
    }

    /// Lets go of a drag without moving anything.
    pub fn drag_cancel(&mut self) {
        self.dragging = None;
    }

    /// Drops the dragged part where the pointer is.
    /// - On a socket: a `join` edit.
    /// - On a part's surface: the part slides there. If it sits on a
    ///   socket of its own on that part, that socket is moved
    ///   (`set-socket`); otherwise a socket is made there (`add-socket`)
    ///   and the part joined to it (`join`).
    /// - On nothing: nothing changes.
    ///
    /// `None` if nothing was being dragged.
    pub fn drag_end(&mut self) -> Option<Result<String, String>> {
        let dragged = self.dragging.clone()?;
        let markers = self.markers();
        let aim = self.aim(&markers);
        self.dragging = None;
        Some(match aim {
            Some(Aim::Marker(index)) => {
                let marker = &markers[index];
                self.apply(&Edit::Join {
                    part: dragged,
                    to: format!("{}.{}", marker.part, marker.socket),
                    attach: None,
                })
            }
            Some(Aim::Surface { part, spot, .. }) => self.slide(&dragged, &part, &spot),
            None => {
                let why = "Not moved: drop it on a socket, or on another part".to_owned();
                self.say(why.clone());
                Err(why)
            }
        })
    }

    /// Puts `dragged` at a point of `onto`'s surface.
    fn slide(&mut self, dragged: &str, onto: &str, spot: &Spot) -> Result<String, String> {
        // The socket it sits on now, if that is on the same part and
        // nothing else sits on it: then that socket is what moves.
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
        if let Some(socket) = its_own {
            let moved = self.apply(&Edit::SetSocket {
                socket: format!("{onto}.{socket}"),
                on: Some(spot.on.clone()),
                shift: Some(spot.shift),
            });
            // A built-in socket (`top`) can't be moved: make a new one.
            if moved.is_ok() {
                return moved;
            }
        }
        let socket = self.new_socket_name(onto);
        self.apply_all(&[
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
        ])
    }

    /// What can be added to a socket beside the five shapes: the parts
    /// the recipe defines.
    pub fn definitions(&self) -> Vec<String> {
        duck_mold_core::source::Syntax::from_path(&self.path)
            .map(|syntax| edit::definitions(&self.text, syntax))
            .unwrap_or_default()
    }
}
