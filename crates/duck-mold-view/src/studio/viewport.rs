//! The Studio's viewport: the camera, the picture, and finding what is
//! under the pointer.
//!
//! The picture is drawn by Duck Forge's CPU renderer, so a model looks
//! here as it does in a game. As in `render`, Duck Mold turns the model
//! for the view itself, with its own sine and cosine, and leaves Duck
//! Forge's camera looking straight ahead: the same view gives the same
//! picture on every computer, which lets the tests keep pictures of it.
//!
//! Finding the part under the pointer follows a line from the eye
//! through the pointer to the first triangle it meets, with the same
//! numbers the picture was drawn with, so the two always agree.

use std::ops::Range;

use duck_mold_core::math::{Bounds, Vec3, sin_cos_degrees, vec3};
use duck_mold_core::model::{Model, Part};
use duckforge::prelude::{Camera3D, Canvas, Color, Lighting, Mesh, Rect, Transform, Vec2, vec2};

use crate::render::{BACKGROUND, png_of};

/// How much the camera sees, top to bottom, in degrees.
pub const FIELD_OF_VIEW: f64 = 40.0;
/// Nothing nearer the eye than this is drawn or picked, in metres.
const NEAR: f64 = 0.1;
/// How far a one-pixel drag turns the view, in degrees.
const TURN_PER_PIXEL: f64 = 0.5;
/// How far the view can tilt up or down, short of straight up or down.
const MOST_TILT: f64 = 85.0;

/// The grid's lines every metre, and the lighter ones every 10 cm.
const GRID: Color = Color::rgb(0x46, 0x4e, 0x5e);
const GRID_FINE: Color = Color::rgb(0x33, 0x39, 0x46);
/// The line along x (left to right) and the one along z (front to back)
/// through the recipe's own zero.
const AXIS_X: Color = Color::rgb(0x9a, 0x45, 0x50);
const AXIS_Z: Color = Color::rgb(0x45, 0x62, 0x9a);
/// The ring on the ground at the model's zero point.
const ZERO_POINT: Color = Color::rgb(0xe8, 0xe8, 0xf0);
/// Duck yellow: the box round the selected part, and the socket under
/// the pointer.
const CHOSEN: Color = Color::rgb(0xff, 0xcc, 0x2e);
const MARKER: Color = Color::rgb(0x6f, 0xd6, 0xc8);
const MARKER_BEHIND: Color = Color::rgb(0x4a, 0x7f, 0x7a);
const MARKER_EDGE: Color = Color::rgb(0x14, 0x18, 0x20);
/// A face nobody coloured: the same grey a `.glb` gives it.
const UNCOLOURED: [u8; 3] = [180, 180, 180];

/// A picture: pixels, row by row from the top, each `0x00RRGGBB`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// How wide, in pixels.
    pub width: usize,
    /// How tall, in pixels.
    pub height: usize,
    /// The pixels.
    pub pixels: Vec<u32>,
}

impl Picture {
    /// The picture as a PNG file.
    pub fn to_png(&self) -> Vec<u8> {
        png_of(self.width, self.height, &self.pixels)
    }

    /// One pixel, as `0xRRGGBB`.
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        self.pixels[y * self.width + x]
    }
}

/// Where the camera is: it circles a point, looking at it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    /// How far round from the model's front, in degrees, towards its
    /// right.
    pub round: f64,
    /// How far above level, in degrees.
    pub above: f64,
    /// How far from the point it looks at, in metres.
    pub distance: f64,
    /// The point it looks at.
    pub target: Vec3,
}

/// A side to look at the model from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewpoint {
    /// Straight at its front.
    Front,
    /// Straight at its right side.
    Side,
    /// Straight down on it.
    Top,
    /// From the front right and a little above: how the Studio opens.
    ThreeQuarter,
}

impl Viewpoint {
    /// All four, in the order of their keys: 1, 2, 3, 4.
    pub const ALL: [Viewpoint; 4] = [
        Viewpoint::Front,
        Viewpoint::Side,
        Viewpoint::Top,
        Viewpoint::ThreeQuarter,
    ];

    /// Its name on a button.
    pub fn name(self) -> &'static str {
        match self {
            Viewpoint::Front => "Front",
            Viewpoint::Side => "Side",
            Viewpoint::Top => "Top",
            Viewpoint::ThreeQuarter => "3/4",
        }
    }
}

impl Orbit {
    /// From the front right and a little above, with all of `bounds` in
    /// view.
    pub fn framing(bounds: Option<Bounds>) -> Orbit {
        let mut orbit = Orbit {
            round: 30.0,
            above: 20.0,
            distance: 4.0,
            target: Vec3::ZERO,
        };
        if let Some(bounds) = bounds {
            orbit.frame(bounds);
        }
        orbit
    }

    /// Looks at the middle of `bounds` from far enough to see all of it,
    /// without turning.
    pub fn frame(&mut self, bounds: Bounds) {
        let (sin_half, _) = sin_cos_degrees(FIELD_OF_VIEW / 2.0);
        self.target = bounds.center();
        self.distance = (bounds.half().length().max(0.05) / sin_half) * 1.15;
    }

    /// Looks from one of the four named sides, at the same point and
    /// from as far as before.
    pub fn look_from(&mut self, from: Viewpoint) {
        (self.round, self.above) = match from {
            Viewpoint::Front => (0.0, 0.0),
            Viewpoint::Side => (90.0, 0.0),
            Viewpoint::Top => (0.0, 90.0),
            Viewpoint::ThreeQuarter => (30.0, 20.0),
        };
    }

    /// Turns the view as a drag of the mouse does: dragging right turns
    /// the model to the right, dragging down tilts it towards you.
    pub fn turn(&mut self, right: f64, down: f64) {
        self.round = (self.round - right * TURN_PER_PIXEL).rem_euclid(360.0);
        self.above = (self.above + down * TURN_PER_PIXEL).clamp(-MOST_TILT, MOST_TILT);
    }

    /// Comes closer (`steps` above 0, as rolling the wheel away from you)
    /// or goes further.
    pub fn zoom(&mut self, steps: f64) {
        self.distance = (self.distance * 0.85f64.powf(steps)).clamp(0.05, 500.0);
    }

    /// Slides the view as a drag does: what was under the pointer stays
    /// under it. `height` is the picture's height in pixels.
    pub fn pan(&mut self, right: f64, down: f64, height: usize) {
        let (right_way, up_way, _) = self.axes();
        let metres = self.distance / focal(height);
        self.target = self.target - right_way * (right * metres) + up_way * (down * metres);
    }

    /// The directions in the model that point right, up and away from the
    /// camera in the picture.
    fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let (sin_round, cos_round) = sin_cos_degrees(self.round);
        let (sin_above, cos_above) = sin_cos_degrees(self.above);
        let right = vec3(-cos_round, 0.0, sin_round);
        let up = vec3(-sin_above * sin_round, cos_above, -sin_above * cos_round);
        let away = vec3(-cos_above * sin_round, -sin_above, -cos_above * cos_round);
        (right, up, away)
    }

    /// A point of the model as the camera sees it: x to the right, y up,
    /// z away from the eye, which is at zero.
    pub fn to_view(&self, point: Vec3) -> Vec3 {
        let (right, up, away) = self.axes();
        let from_target = point - self.target;
        vec3(
            from_target.dot(right),
            from_target.dot(up),
            from_target.dot(away) + self.distance,
        )
    }

    /// The other way: a point the camera sees, in the model.
    pub fn from_view(&self, seen: Vec3) -> Vec3 {
        let (right, up, away) = self.axes();
        self.target + right * seen.x + up * seen.y + away * (seen.z - self.distance)
    }

    /// Where the eye is, in the model.
    pub fn eye(&self) -> Vec3 {
        self.from_view(Vec3::ZERO)
    }

    /// Where a point of the model is in a picture `width` x `height`, in
    /// pixels from its top left corner. `None` if it is behind the eye.
    pub fn on_screen(&self, point: Vec3, width: usize, height: usize) -> Option<(f64, f64)> {
        let seen = self.to_view(point);
        if seen.z < NEAR {
            return None;
        }
        let focal = focal(height);
        Some((
            width as f64 / 2.0 + seen.x / seen.z * focal,
            height as f64 / 2.0 - seen.y / seen.z * focal,
        ))
    }
}

/// How many pixels one metre covers at one metre from the eye.
pub(crate) fn focal(height: usize) -> f64 {
    let (sin_half, cos_half) = sin_cos_degrees(FIELD_OF_VIEW / 2.0);
    (height as f64 / 2.0) * cos_half / sin_half
}

/// A socket shown in the viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    /// The part it belongs to, by path: `left_arm/hand`.
    pub part: String,
    /// Its name on that part: `bottom`.
    pub socket: String,
    /// Where it is in the model.
    pub at: Vec3,
    /// The way it faces.
    pub facing: Vec3,
}

/// How near the pointer must be to a marker to be on it, in pixels.
pub const MARKER_REACH: f64 = 8.0;

/// Something drawn over the model: a piece of a handle. Places are where
/// the picture shows them (the model's own places, moved by
/// [`Look::offset`] already).
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// A line one pixel thick: the outline of a face.
    Thin {
        /// Where it starts.
        from: Vec3,
        /// Where it ends.
        to: Vec3,
        /// Red, green and blue.
        color: [u8; 3],
    },
    /// A line, three pixels thick.
    Line {
        /// Where it starts.
        from: Vec3,
        /// Where it ends.
        to: Vec3,
        /// Red, green and blue.
        color: [u8; 3],
    },
    /// A line with an arrow's head at `to`.
    Arrow {
        /// Where it starts.
        from: Vec3,
        /// Where its head is.
        to: Vec3,
        /// Red, green and blue.
        color: [u8; 3],
    },
    /// A filled circle.
    Disc {
        /// Its middle.
        at: Vec3,
        /// How big it is on the screen, in pixels.
        radius: f32,
        /// Red, green and blue.
        color: [u8; 3],
    },
    /// A filled square.
    Block {
        /// Its middle.
        at: Vec3,
        /// From its middle to a side on the screen, in pixels.
        half: f32,
        /// Red, green and blue.
        color: [u8; 3],
    },
}

/// What to draw.
pub struct Look<'a> {
    /// The model, if there is one to show.
    pub model: Option<&'a Model>,
    /// Where the camera is.
    pub orbit: &'a Orbit,
    /// How far the model is moved sideways in the picture, so that the
    /// grid's lines are the recipe's own measurements.
    pub offset: Vec3,
    /// The selected parts: brightened, with the box round each drawn.
    pub selected: &'a [&'a Part],
    /// Faces to brighten besides those of the selected parts: the face
    /// the Paint tool is over.
    pub lit: &'a [Range<usize>],
    /// The faces that aren't drawn: those of hidden parts.
    pub hidden: &'a [Range<usize>],
    /// When set, only these faces are drawn in full: the rest are faded
    /// towards the background (Face mode, shaping one part).
    pub focus: Option<Range<usize>>,
    /// Whether only the outline of each face is drawn, so that the
    /// model can be seen through.
    pub wire: bool,
    /// Whether the ground is darkened under the model.
    pub shadow: bool,
    /// The sockets to show.
    pub markers: &'a [Marker],
    /// Which of `markers` the pointer is on.
    pub hot: Option<usize>,
    /// The handles, drawn over everything.
    pub marks: &'a [Mark],
    /// Whether to draw the grid on the ground.
    pub grid: bool,
}

fn forge(point: Vec3) -> duckforge::prelude::Vec3 {
    duckforge::prelude::vec3(point.x as f32, point.y as f32, point.z as f32)
}

fn paint([red, green, blue]: [u8; 3]) -> Color {
    Color::rgb(red, green, blue)
}

/// Draws the viewport, `width` x `height` pixels.
pub fn draw(look: &Look<'_>, width: usize, height: usize) -> Picture {
    let (width, height) = (width.max(1), height.max(1));
    let mut canvas = Canvas::new(width, height);
    canvas.clear(BACKGROUND);
    let orbit = look.orbit;
    let offset = look.offset;
    // Duck Forge's camera stays at zero, looking straight ahead: the
    // model is brought to it.
    let camera = Camera3D {
        position: duckforge::prelude::vec3(0.0, 0.0, 0.0),
        yaw: 0.0,
        pitch: 0.0,
        fov: (FIELD_OF_VIEW as f32).to_radians(),
    };
    // The two ends of a line on the screen, with what is behind the eye
    // cut off. `None` if all of it is.
    let ends = |canvas: &Canvas, from: Vec3, to: Vec3| -> Option<(Vec2, Vec2)> {
        let (mut a, mut b) = (orbit.to_view(from), orbit.to_view(to));
        if a.z < NEAR && b.z < NEAR {
            return None;
        }
        if a.z < NEAR {
            std::mem::swap(&mut a, &mut b);
        }
        if b.z < NEAR {
            let along = (a.z - NEAR) / (a.z - b.z);
            b = a + (b - a) * along;
        }
        let (from, _) = camera.project(forge(a), canvas)?;
        let (to, _) = camera.project(forge(b), canvas)?;
        Some((from, to))
    };
    let line = |canvas: &mut Canvas, from: Vec3, to: Vec3, color: Color| {
        if let Some((from, to)) = ends(canvas, from, to) {
            canvas.draw_line(from, to, color);
        }
    };
    let thick = |canvas: &mut Canvas, from: Vec3, to: Vec3, color: Color| {
        if let Some((from, to)) = ends(canvas, from, to) {
            // Three lines side by side, across the way it runs.
            let along = to - from;
            let across = if along.x.abs() > along.y.abs() {
                vec2(0.0, 1.0)
            } else {
                vec2(1.0, 0.0)
            };
            canvas.draw_line(from - across, to - across, color);
            canvas.draw_line(from, to, color);
            canvas.draw_line(from + across, to + across, color);
        }
    };

    // The shadow: the ground darkened straight under the model, darkest
    // where something stands on it. Only seen from above the ground.
    if look.shadow
        && orbit.above > 1.0
        && let Some(model) = look.model
        && let Some(shade) = Shade::under(model, look.hidden)
    {
        let spot = |canvas: &Canvas, x: f64, z: f64| -> Option<Vec2> {
            let seen = orbit.to_view(vec3(x, shade.ground, z) + offset);
            if seen.z < NEAR {
                return None;
            }
            camera.project(forge(seen), canvas).map(|(at, _)| at)
        };
        let [red, green, blue] = [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b];
        for row in 0..shade.deep {
            for column in 0..shade.across {
                let dark = shade.dark[row * shade.across + column];
                if dark < 0.02 {
                    continue;
                }
                let (x, z) = (
                    shade.x + column as f64 * shade.cell,
                    shade.z + row as f64 * shade.cell,
                );
                let corners = [
                    spot(&canvas, x, z),
                    spot(&canvas, x + shade.cell, z),
                    spot(&canvas, x + shade.cell, z + shade.cell),
                    spot(&canvas, x, z + shade.cell),
                ];
                let [Some(a), Some(b), Some(c), Some(d)] = corners else {
                    continue;
                };
                let keep = 1.0 - SHADOW_DEPTH * dark;
                let color = Color::rgb(
                    (f32::from(red) * keep) as u8,
                    (f32::from(green) * keep) as u8,
                    (f32::from(blue) * keep) as u8,
                );
                canvas.fill_triangle(a, b, c, color);
                canvas.fill_triangle(a, c, d, color);
            }
        }
    }

    if look.grid {
        // The ground: light lines every 10 cm near the model, heavier
        // ones every metre, a red line along x and a blue one along z
        // through the recipe's own zero. It reaches two metres past the
        // model each way, to whole metres.
        let bounds = look.model.and_then(Model::bounds);
        let (low, high) = match bounds {
            Some(bounds) => (bounds.min + offset, bounds.max + offset),
            None => (vec3(-1.0, 0.0, -1.0), vec3(1.0, 0.0, 1.0)),
        };
        let reach = |low: f64, high: f64| {
            let low = (low.min(0.0).floor() as i32 - 2).max(-60);
            let high = (high.max(0.0).ceil() as i32 + 2).min(60);
            (low, high)
        };
        let (x0, x1) = reach(low.x, high.x);
        let (z0, z1) = reach(low.z, high.z);
        let (fx0, fx1, fz0, fz1) = (f64::from(x0), f64::from(x1), f64::from(z0), f64::from(z1));
        // The fine lines, only where they aren't too many to see apart.
        if (x1 - x0) <= 12 && (z1 - z0) <= 12 {
            for tenth in (x0 * 10)..=(x1 * 10) {
                if tenth % 10 != 0 {
                    let x = f64::from(tenth) / 10.0;
                    line(&mut canvas, vec3(x, 0.0, fz0), vec3(x, 0.0, fz1), GRID_FINE);
                }
            }
            for tenth in (z0 * 10)..=(z1 * 10) {
                if tenth % 10 != 0 {
                    let z = f64::from(tenth) / 10.0;
                    line(&mut canvas, vec3(fx0, 0.0, z), vec3(fx1, 0.0, z), GRID_FINE);
                }
            }
        }
        for x in x0..=x1 {
            if x != 0 {
                let x = f64::from(x);
                line(&mut canvas, vec3(x, 0.0, fz0), vec3(x, 0.0, fz1), GRID);
            }
        }
        for z in z0..=z1 {
            if z != 0 {
                let z = f64::from(z);
                line(&mut canvas, vec3(fx0, 0.0, z), vec3(fx1, 0.0, z), GRID);
            }
        }
        line(
            &mut canvas,
            vec3(fx0, 0.0, 0.0),
            vec3(fx1, 0.0, 0.0),
            AXIS_X,
        );
        line(
            &mut canvas,
            vec3(0.0, 0.0, fz0),
            vec3(0.0, 0.0, fz1),
            AXIS_Z,
        );
        // The model's zero point: where a game holds it.
        if look.model.is_some()
            && let Some((x, y)) = orbit.on_screen(offset, width, height)
        {
            let centre = vec2(x as f32, y as f32);
            canvas.fill_circle(centre, 5.0, ZERO_POINT);
            canvas.fill_circle(centre, 3.0, BACKGROUND);
        }
    }

    if let Some(model) = look.model {
        let lit: Vec<Range<usize>> = look
            .selected
            .iter()
            .map(|part| part.faces.clone())
            .chain(look.lit.iter().cloned())
            .collect();
        if look.wire {
            // Wireframe: the outline of every face, in the face's own
            // colour made lighter, and nothing filled in.
            for (at, face) in model.faces.iter().enumerate() {
                if look.hidden.iter().any(|hidden| hidden.contains(&at)) {
                    continue;
                }
                let mut rgb = face
                    .color
                    .and_then(|color| model.palette.get(color))
                    .map_or(UNCOLOURED, |color| color.rgb);
                if look
                    .focus
                    .as_ref()
                    .is_some_and(|focus| !focus.contains(&at))
                {
                    rgb = faded(rgb);
                }
                let bright = lit.iter().any(|lit| lit.contains(&at));
                let lighter = |channel: u8| {
                    let once = channel + (255 - channel) / 3;
                    if bright {
                        once + (255 - once) / 2
                    } else {
                        once
                    }
                };
                let color = Color::rgb(lighter(rgb[0]), lighter(rgb[1]), lighter(rgb[2]));
                for (corner, &from) in face.corners.iter().enumerate() {
                    let to = face.corners[(corner + 1) % face.corners.len()];
                    line(&mut canvas, from + offset, to + offset, color);
                }
            }
        } else {
            let mesh = mesh_of(model, orbit, offset, &lit, look.hidden, look.focus.as_ref());
            // The light comes from over the viewer's left shoulder,
            // whichever way the model is turned.
            canvas.set_lighting(Lighting {
                sun_direction: duckforge::prelude::vec3(-0.35, 0.75, -1.0),
                ambient: 0.45,
                ..Lighting::default()
            });
            canvas.draw_mesh(&mesh, &Transform::default(), &camera);
        }
    }

    for bounds in look.selected.iter().filter_map(|part| part.bounds) {
        let corner = |x: bool, y: bool, z: bool| {
            vec3(
                if x { bounds.max.x } else { bounds.min.x },
                if y { bounds.max.y } else { bounds.min.y },
                if z { bounds.max.z } else { bounds.min.z },
            ) + offset
        };
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            line(&mut canvas, corner(false, a, b), corner(true, a, b), CHOSEN);
            line(&mut canvas, corner(a, false, b), corner(a, true, b), CHOSEN);
            line(&mut canvas, corner(a, b, false), corner(a, b, true), CHOSEN);
        }
    }

    let eye = orbit.eye();
    for (at, marker) in look.markers.iter().enumerate() {
        let place = marker.at + offset;
        let Some((x, y)) = orbit.on_screen(place, width, height) else {
            continue;
        };
        let centre: Vec2 = vec2(x as f32, y as f32);
        let hot = look.hot == Some(at);
        let faces_us = marker.facing.dot(eye - place) >= 0.0;
        let (radius, color) = match (hot, faces_us) {
            (true, _) => (6.0, CHOSEN),
            (false, true) => (4.0, MARKER),
            (false, false) => (3.0, MARKER_BEHIND),
        };
        canvas.fill_circle(centre, radius + 1.5, MARKER_EDGE);
        canvas.fill_circle(centre, radius, color);
    }

    for mark in look.marks {
        match mark {
            Mark::Thin { from, to, color } => line(&mut canvas, *from, *to, paint(*color)),
            Mark::Line { from, to, color } => thick(&mut canvas, *from, *to, paint(*color)),
            Mark::Arrow { from, to, color } => {
                thick(&mut canvas, *from, *to, paint(*color));
                if let Some((tail, tip)) = ends(&canvas, *from, *to) {
                    let along = tip - tail;
                    let long = (along.x * along.x + along.y * along.y).sqrt();
                    if long > 1.0 {
                        let along = along * (1.0 / long);
                        let across = vec2(-along.y, along.x);
                        canvas.fill_triangle(
                            tip + along * 11.0,
                            tip + across * 6.0,
                            tip - across * 6.0,
                            paint(*color),
                        );
                    }
                }
            }
            Mark::Disc { at, radius, color } => {
                if let Some((x, y)) = orbit.on_screen(*at, width, height) {
                    let centre = vec2(x as f32, y as f32);
                    canvas.fill_circle(centre, radius + 1.5, MARKER_EDGE);
                    canvas.fill_circle(centre, *radius, paint(*color));
                }
            }
            Mark::Block { at, half, color } => {
                if let Some((x, y)) = orbit.on_screen(*at, width, height) {
                    let (x, y) = (x as f32, y as f32);
                    let edge = half + 1.5;
                    canvas.fill_rect(
                        Rect::new(x - edge, y - edge, edge * 2.0, edge * 2.0),
                        MARKER_EDGE,
                    );
                    canvas.fill_rect(
                        Rect::new(x - half, y - half, half * 2.0, half * 2.0),
                        paint(*color),
                    );
                }
            }
        }
    }

    Picture {
        width,
        height,
        pixels: canvas.pixels().to_vec(),
    }
}

/// How much of the ground's brightness the darkest shadow takes away.
const SHADOW_DEPTH: f32 = 0.72;
/// How many cells the shadow has along the model's longer side.
const SHADOW_CELLS: usize = 96;

/// The shadow under a model: a grid of square cells on the ground, each
/// with how dark it is, from 0 (open ground) to 1 (something stands
/// right on it).
struct Shade {
    /// The corner of the grid with the least x and z, in the model.
    x: f64,
    z: f64,
    /// The side of a cell, in metres.
    cell: f64,
    across: usize,
    deep: usize,
    dark: Vec<f32>,
    /// How high the ground is: the model's lowest point.
    ground: f64,
}

impl Shade {
    /// Works the shadow out: every face darkens the cells straight under
    /// it, the more the nearer it is to the ground, and the edges are
    /// then softened.
    fn under(model: &Model, hidden: &[Range<usize>]) -> Option<Shade> {
        let bounds = model.bounds()?;
        let size = bounds.size();
        let long = size.x.max(size.z);
        if long <= 0.0 {
            return None;
        }
        // Room all round for the soft edge.
        let cell = long * 1.2 / SHADOW_CELLS as f64;
        let margin = cell * 6.0;
        let (x, z) = (bounds.min.x - margin, bounds.min.z - margin);
        let across = (((size.x + margin * 2.0) / cell).ceil() as usize).clamp(1, 160);
        let deep = (((size.z + margin * 2.0) / cell).ceil() as usize).clamp(1, 160);
        let ground = bounds.min.y;
        // A face this high above the ground casts the palest shadow.
        let tall = size.y.max(long * 0.25).max(1e-6);
        let mut dark = vec![0.0f32; across * deep];
        for (at, face) in model.faces.iter().enumerate() {
            if hidden.iter().any(|hidden| hidden.contains(&at)) {
                continue;
            }
            let low = face
                .corners
                .iter()
                .fold(f64::INFINITY, |low, corner| low.min(corner.y));
            let weight = (1.0 - ((low - ground) / tall) * 0.7).clamp(0.3, 1.0) as f32;
            for [a, b, c] in face.triangles() {
                // The cells whose middles are under the triangle.
                let column = |x_at: f64| ((x_at - x) / cell).floor().max(0.0) as usize;
                let row = |z_at: f64| ((z_at - z) / cell).floor().max(0.0) as usize;
                let (first_column, last_column) = (
                    column(a.x.min(b.x).min(c.x)),
                    column(a.x.max(b.x).max(c.x)).min(across - 1),
                );
                let (first_row, last_row) = (
                    row(a.z.min(b.z).min(c.z)),
                    row(a.z.max(b.z).max(c.z)).min(deep - 1),
                );
                // Which side of each edge a point is on, seen from above.
                let side = |p: (f64, f64), from: Vec3, to: Vec3| {
                    (to.x - from.x) * (p.1 - from.z) - (to.z - from.z) * (p.0 - from.x)
                };
                for row_at in first_row..=last_row {
                    for column_at in first_column..=last_column {
                        let middle = (
                            x + (column_at as f64 + 0.5) * cell,
                            z + (row_at as f64 + 0.5) * cell,
                        );
                        let sides = [side(middle, a, b), side(middle, b, c), side(middle, c, a)];
                        let inside = sides.iter().all(|side| *side >= 0.0)
                            || sides.iter().all(|side| *side <= 0.0);
                        let cell_at = &mut dark[row_at * across + column_at];
                        if inside && weight > *cell_at {
                            *cell_at = weight;
                        }
                    }
                }
            }
        }
        // Softened: each cell becomes the average of itself and the
        // cells round it, three times over.
        for _ in 0..3 {
            let before = dark.clone();
            for row_at in 0..deep {
                for column_at in 0..across {
                    let mut sum = 0.0;
                    let mut count = 0.0;
                    for down in row_at.saturating_sub(1)..=(row_at + 1).min(deep - 1) {
                        for along in column_at.saturating_sub(1)..=(column_at + 1).min(across - 1) {
                            sum += before[down * across + along];
                            count += 1.0;
                        }
                    }
                    dark[row_at * across + column_at] = sum / count;
                }
            }
        }
        Some(Shade {
            x,
            z,
            cell,
            across,
            deep,
            dark,
            ground,
        })
    }
}

/// The model as a Duck Forge mesh, already turned for the camera. The
/// faces in `lit` are brightened; those in `hidden` are left out.
fn mesh_of(
    model: &Model,
    orbit: &Orbit,
    offset: Vec3,
    lit: &[Range<usize>],
    hidden: &[Range<usize>],
    focus: Option<&Range<usize>>,
) -> Mesh {
    let mut mesh = Mesh::new();
    for (at, face) in model.faces.iter().enumerate() {
        if hidden.iter().any(|hidden| hidden.contains(&at)) {
            continue;
        }
        let mut rgb = face
            .color
            .and_then(|color| model.palette.get(color))
            .map_or(UNCOLOURED, |color| color.rgb);
        if lit.iter().any(|lit| lit.contains(&at)) {
            rgb = rgb.map(|channel| channel + (255 - channel) / 3);
        }
        if focus.is_some_and(|focus| !focus.contains(&at)) {
            rgb = faded(rgb);
        }
        let color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        let first = mesh.vertices.len();
        for &corner in &face.corners {
            mesh.add_vertex(forge(orbit.to_view(corner + offset)));
        }
        for step in 1..face.corners.len().saturating_sub(1) {
            mesh.add_triangle(first, first + step, first + step + 1, color);
        }
    }
    mesh
}

/// A colour most of the way to the background: a part that isn't being
/// shaped.
fn faded(rgb: [u8; 3]) -> [u8; 3] {
    let back = [BACKGROUND.r, BACKGROUND.g, BACKGROUND.b];
    let mut out = [0; 3];
    for at in 0..3 {
        out[at] = ((u16::from(rgb[at]) + 2 * u16::from(back[at])) / 3) as u8;
    }
    out
}

/// What the pointer is on.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Which of the model's faces.
    pub face: usize,
    /// The part that face belongs to, by path.
    pub part: String,
    /// The point of the face under the pointer, in the model.
    pub point: Vec3,
    /// The way the face points, 1 long.
    pub normal: Vec3,
}

/// The line from the eye through the pointer at (`x`, `y`), in pixels
/// from the top left of a picture `width` x `height`: as the camera sees
/// it, with z = 1.
pub fn ray_through((width, height): (usize, usize), (x, y): (f64, f64)) -> Vec3 {
    let focal = focal(height);
    vec3(
        (x - width as f64 / 2.0) / focal,
        -(y - height as f64 / 2.0) / focal,
        1.0,
    )
}

/// Finds the face under the pointer at (`x`, `y`), in pixels from the top
/// left of a picture `width` x `height`. Faces in `skip` are looked
/// through. `offset` is how far the picture shows the model moved
/// ([`Look::offset`]); the point that comes back is in the model.
pub fn pick(
    model: &Model,
    orbit: &Orbit,
    offset: Vec3,
    size: (usize, usize),
    at: (f64, f64),
    skip: &[Range<usize>],
) -> Option<Hit> {
    // The line from the eye through the pointer, as the camera sees it.
    let ray = ray_through(size, at);
    let mut nearest: Option<(f64, usize)> = None;
    for (at, face) in model.faces.iter().enumerate() {
        if skip.iter().any(|skip| skip.contains(&at)) {
            continue;
        }
        let seen: Vec<Vec3> = face
            .corners
            .iter()
            .map(|&c| orbit.to_view(c + offset))
            .collect();
        for step in 1..seen.len().saturating_sub(1) {
            if let Some(along) = crossing(ray, [seen[0], seen[step], seen[step + 1]])
                && along >= NEAR
                && nearest.is_none_or(|(best, _)| along < best)
            {
                nearest = Some((along, at));
            }
        }
    }
    let (along, face) = nearest?;
    let part = model
        .leaves()
        .into_iter()
        .find(|leaf| leaf.faces.contains(&face))?;
    Some(Hit {
        face,
        part: part.path.clone(),
        point: orbit.from_view(ray * along) - offset,
        normal: model.faces[face].normal().unwrap_or(Vec3::UP),
    })
}

/// How far along a line from zero in direction `ray` it goes through a
/// triangle, in multiples of `ray`. `None` if it misses.
fn crossing(ray: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f64> {
    let (ab, ac) = (b - a, c - a);
    let p = ray.cross(ac);
    let det = ab.dot(p);
    if det.abs() < 1e-12 {
        return None; // the line runs along the triangle
    }
    let from_a = Vec3::ZERO - a;
    let u = from_a.dot(p) / det;
    let q = from_a.cross(ab);
    let v = ray.dot(q) / det;
    if u < 0.0 || v < 0.0 || u + v > 1.0 {
        return None;
    }
    let along = ac.dot(q) / det;
    (along > 0.0).then_some(along)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_goes_to_the_camera_and_back() {
        let orbit = Orbit {
            round: 37.0,
            above: 22.0,
            distance: 5.0,
            target: vec3(0.3, 1.0, -0.2),
        };
        let point = vec3(1.0, 2.0, 3.0);
        let back = orbit.from_view(orbit.to_view(point));
        assert!((back - point).length() < 1e-12);
        // The target is straight ahead, `distance` away.
        assert!((orbit.to_view(orbit.target) - vec3(0.0, 0.0, 5.0)).length() < 1e-12);
        // And in the middle of the picture.
        assert_eq!(orbit.on_screen(orbit.target, 200, 100), Some((100.0, 50.0)));
    }

    #[test]
    fn from_the_front_the_models_left_is_on_the_right() {
        let orbit = Orbit {
            round: 0.0,
            above: 0.0,
            distance: 5.0,
            target: Vec3::ZERO,
        };
        // The camera is in front (+z), looking back at the model.
        assert!((orbit.eye() - vec3(0.0, 0.0, 5.0)).length() < 1e-12);
        let (x, _) = orbit.on_screen(vec3(-1.0, 0.0, 0.0), 200, 100).unwrap();
        assert!(x > 100.0);
        let (_, y) = orbit.on_screen(vec3(0.0, 1.0, 0.0), 200, 100).unwrap();
        assert!(y < 50.0, "up is up");
    }

    #[test]
    fn dragging_turns_tilts_and_slides() {
        let mut orbit = Orbit::framing(None);
        orbit.turn(20.0, 10.0);
        assert_eq!((orbit.round, orbit.above), (20.0, 25.0));
        orbit.turn(0.0, 1000.0);
        assert_eq!(orbit.above, 85.0);
        let before = orbit.distance;
        orbit.zoom(1.0);
        assert!((orbit.distance - before * 0.85).abs() < 1e-12);
        // Sliding 10 pixels right keeps the old target 10 pixels right of
        // the middle.
        let old = orbit.target;
        orbit.pan(10.0, 0.0, 300);
        let (x, y) = orbit.on_screen(old, 400, 300).unwrap();
        assert!(
            (x - 210.0).abs() < 1e-6 && (y - 150.0).abs() < 1e-6,
            "{x} {y}"
        );
    }
}
