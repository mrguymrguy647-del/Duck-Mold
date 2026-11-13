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

use duck_mold_core::edit::{self, Edit, Free, Made, Sizes, Spot};
use duck_mold_core::math::Vec3;
use duck_mold_core::model::Part;
use duck_mold_core::source::Syntax;

use super::handles::{Grip, Tool};
use super::viewport::{self, MARKER_REACH, Marker};
use super::{Studio, part_in};

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

/// What a new part made of `made` is called, before a number makes the
/// name its own.
fn base_name(made: &Made) -> String {
    match made {
        Made::Shape(shape) => shape.clone(),
        Made::Use(defined) => defined.clone(),
        Made::File(file) => std::path::Path::new(file).file_stem().map_or_else(
            || "part".to_owned(),
            |stem| stem.to_string_lossy().into_owned(),
        ),
    }
}

/// `base` with only what a name can have: letters, digits, `_` and `-`.
fn clean_name(base: &str) -> String {
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
    if base.is_empty() {
        "part".to_owned()
    } else {
        base
    }
}

/// The sizes of a new shape about `across` big each way, to two figures.
/// Every size is given, so that none is left at a default that doesn't
/// fit the others.
fn sizes_for(shape: &str, across: f64) -> Sizes {
    let across = across.max(0.01);
    let unit = 10f64.powf(across.log10().floor() - 1.0);
    let tidy = |number: f64| ((number / unit).round().max(1.0) * unit * 1e6).round() / 1e6;
    let across = tidy(across);
    let radius = ((across / 2.0) * 1e6).round() / 1e6;
    let mut sizes = Sizes::default();
    match shape {
        "box" | "wedge" | "pyramid" | "prism" | "stairs" => sizes.size = Some([across; 3]),
        "arch" => {
            sizes.size = Some([across, across, tidy(across * 0.25)]);
            sizes.wall = Some(tidy(across * 0.15));
        }
        "sphere" => sizes.radius = Some(radius),
        "capsule" => {
            sizes.radius = Some(tidy(across * 0.25));
            sizes.height = Some(across);
        }
        "torus" => {
            sizes.radius = Some(tidy(across * 0.35));
            sizes.tube = Some(tidy(across * 0.15));
        }
        "tube" => {
            sizes.radius = Some(radius);
            sizes.wall = Some(tidy(across * 0.1));
            sizes.height = Some(across);
        }
        _ => {
            sizes.radius = Some(radius);
            sizes.height = Some(across);
        }
    }
    sizes
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
    pub(super) fn list_of(&self, path: &str) -> &[Part] {
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
            .filter(|(_, marker)| marker.facing.dot(eye - (marker.at + self.offset())) >= 0.0)
            .filter_map(|(at, marker)| {
                let (mx, my) = self
                    .orbit
                    .on_screen(marker.at + self.offset(), width, height)?;
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
        self.selected.as_deref()?;
        // Only while a part is dragged can a surface be aimed at. (A
        // click on the part itself is the start of a drag now; "add a
        // part here" is in the right-click menu.)
        let dragged = self.dragging.as_ref()?;
        // Through the dragged part, to what is behind it.
        let skip = self.part(dragged)?.faces.clone();
        let hit = viewport::pick(
            model,
            &self.orbit,
            self.offset(),
            self.view,
            (x, y),
            Some(&skip),
        )?;
        let part = self
            .could_join(dragged)
            .into_iter()
            .find(|part| part.faces.contains(&hit.face))?;
        let spot = edit::spot(model, part, hit.point, hit.normal)?;
        Some(Aim::Surface {
            part: part.path.clone(),
            spot,
            at: hit.point,
            facing: hit.normal,
        })
    }

    /// A click in the viewport at (`x`, `y`) of a picture `size`. On a
    /// socket of the selected part it finds a place to add a part, and
    /// selects nothing new. Anywhere else it selects, as
    /// [`Studio::click`] does.
    pub fn point_at(&mut self, size: (usize, usize), at: (f64, f64)) -> Clicked {
        self.hover(size, Some(at));
        let markers = self.markers();
        match self.aim(&markers) {
            Some(Aim::Marker(index)) => Clicked::Socket {
                part: markers[index].part.clone(),
                socket: markers[index].socket.clone(),
            },
            _ => match self.click(size, at) {
                Some(path) => Clicked::Selected(path.to_owned()),
                None => Clicked::Nothing,
            },
        }
    }

    /// The point of a part's surface under the pointer, written the way a
    /// socket is declared: for "add a part here". The part is the
    /// selected one if the pointer is on it, and otherwise the whole
    /// part the pointer is on.
    pub fn spot_at(&self, size: (usize, usize), at: (f64, f64)) -> Option<(String, Spot)> {
        let model = self.model()?;
        let hit = viewport::pick(model, &self.orbit, self.offset(), size, at, None)?;
        let part = self
            .selected
            .as_deref()
            .and_then(|path| self.part(path))
            .filter(|part| part.faces.contains(&hit.face))
            .or_else(|| {
                model
                    .parts
                    .iter()
                    .find(|part| part.faces.contains(&hit.face))
            })?;
        let spot = edit::spot(model, part, hit.point, hit.normal)?;
        Some((part.path.clone(), spot))
    }

    /// A name for a new part next to `beside`: `base`, or `base_2`...
    fn new_part_name(&self, beside: &str, base: &str) -> String {
        let taken: Vec<String> = self
            .list_of(beside)
            .iter()
            .map(|part| part.name.clone())
            .collect();
        edit::unused_name(&clean_name(base), &taken)
    }

    /// The edit that adds a part made of `made` to a socket. A new shape
    /// is about half as big as the part it goes on, and has its colour.
    fn adding(&self, part: &str, socket: &str, made: &Made) -> Edit {
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
            sizes = sizes_for(shape, across * 0.5);
            color = self.colour_of(part);
        }
        Edit::Add {
            name: self.new_part_name(part, &base_name(made)),
            made: made.clone(),
            to: Some(format!("{part}.{socket}")),
            attach: None,
            sizes,
            color,
            twist: None,
            free: Free::default(),
        }
    }

    /// The colour the recipe gives a part, if it gives one.
    fn colour_of(&self, part: &str) -> Option<String> {
        let syntax = Syntax::from_path(&self.path)?;
        edit::describe(&self.text, syntax, self.model(), part)?.color
    }

    /// Adds a part made of `made` that is joined to nothing: an `add`
    /// edit with no socket. It stands on the ground in the middle of
    /// what the view looks at, or, where something stands there already,
    /// as far towards the eye as it takes to be clear of it. A new shape
    /// is about half as big as the model's smallest side. The new part
    /// is selected, with the Move tool in hand.
    pub fn add_free(&mut self, made: &Made) -> Result<String, String> {
        let taken: Vec<String> = self.model().map_or_else(Vec::new, |model| {
            model.parts.iter().map(|part| part.name.clone()).collect()
        });
        let name = edit::unused_name(&clean_name(&base_name(made)), &taken);
        let mut sizes = Sizes::default();
        let mut color = None;
        if let Made::Shape(shape) = made {
            let across = self
                .model()
                .and_then(|model| model.bounds())
                .map_or(1.0, |bounds| {
                    let size = bounds.size();
                    size.x.min(size.y).min(size.z)
                });
            sizes = sizes_for(shape, across * 0.5);
            color = self
                .selected
                .clone()
                .and_then(|part| self.colour_of(&part))
                .or_else(|| self.palette().first().map(|(name, _)| name.clone()));
        }
        let add = |at: [f64; 2]| Edit::Add {
            name: name.clone(),
            made: made.clone(),
            to: None,
            attach: None,
            sizes: sizes.clone(),
            color: color.clone(),
            twist: None,
            free: Free {
                stand: Some(at),
                ..Free::default()
            },
        };
        // Sideways, the picture's measurements are the recipe's own.
        let step = self.snap_step();
        let snapped = |x: f64, z: f64| {
            [x, z].map(|number| ((number / step).round() * step * 1e6).round() / 1e6)
        };
        let middle = self.orbit.target;
        let mut at = snapped(middle.x, middle.z);
        // How much room it takes is known once it is built: try it, and
        // look.
        let tried = Syntax::from_path(&self.path).and_then(|syntax| {
            edit::apply_many(&self.text, syntax, &self.path, &self.shown_as, &[add(at)]).ok()
        });
        let room = tried
            .and_then(|tried| tried.model)
            .and_then(|model| part_in(&model.parts, &name).and_then(|part| part.bounds))
            .map(|bounds| bounds.half());
        if let (Some(room), Some(model)) = (room, self.model()) {
            let offset = self.offset();
            let others: Vec<_> = model.parts.iter().filter_map(|part| part.bounds).collect();
            let clear = |at: [f64; 2]| {
                others.iter().all(|other| {
                    let (low, high) = (other.min + offset, other.max + offset);
                    low.y >= room.y * 2.0
                        || at[0] + room.x + step <= low.x + 1e-9
                        || at[0] - room.x - step >= high.x - 1e-9
                        || at[1] + room.z + step <= low.z + 1e-9
                        || at[1] - room.z - step >= high.z - 1e-9
                })
            };
            // Towards the eye, along the ground.
            let eye = self.orbit.eye() - middle;
            let flat = (eye.x * eye.x + eye.z * eye.z).sqrt();
            let towards = if flat > 1e-6 {
                [eye.x / flat, eye.z / flat]
            } else {
                [0.0, 1.0]
            };
            if let Some(free) = (0..2000)
                .map(|nearer| {
                    let nearer = f64::from(nearer) * step;
                    snapped(
                        middle.x + towards[0] * nearer,
                        middle.z + towards[1] * nearer,
                    )
                })
                .find(|at| clear(*at))
            {
                at = free;
            }
        }
        let said = self.apply(&add(at))?;
        self.tool = Tool::Move;
        Ok(said)
    }

    /// A name for a new socket on `part`: `spot`, `spot_2`...
    pub(super) fn new_socket_name(&self, part: &str) -> String {
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

    /// Starts dragging the selected part itself, if the pointer is on it
    /// and it can be moved. True if a drag began. ([`Studio::grab`] also
    /// takes hold of handles; this is the part only.)
    pub fn drag_start(&mut self, size: (usize, usize), at: (f64, f64)) -> bool {
        match self.grab(size, at) {
            Some(Grip::Body) => true,
            _ => {
                self.cancel_drag();
                false
            }
        }
    }

    /// Lets go of a drag without moving anything.
    pub fn drag_cancel(&mut self) {
        self.cancel_drag();
    }

    /// Drops the dragged part where the pointer is.
    /// - On a socket: a `join` edit.
    /// - A joined part on a part's surface: it slides there. If it sits
    ///   on a socket of its own on that part, that socket is moved
    ///   (`set-socket`); otherwise a socket is made there (`add-socket`)
    ///   and the part joined to it (`join`).
    /// - A free part anywhere else: it has moved across the ground
    ///   (`set --move`).
    /// - A joined part on nothing: nothing changes.
    ///
    /// `None` if nothing was being dragged.
    pub fn drag_end(&mut self) -> Option<Result<String, String>> {
        if let Some(at) = self.pointer {
            self.drag(self.view, at, false);
        }
        self.release()
    }

    /// What can be added to a socket beside the built-in shapes: the parts
    /// the recipe defines.
    pub fn definitions(&self) -> Vec<String> {
        duck_mold_core::source::Syntax::from_path(&self.path)
            .map(|syntax| edit::definitions(&self.text, syntax))
            .unwrap_or_default()
    }
}
