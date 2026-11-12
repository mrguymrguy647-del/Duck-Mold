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
use duckforge::prelude::{Camera3D, Canvas, Color, Lighting, Mesh, Transform, Vec2, vec2};

use crate::render::{BACKGROUND, png_of};

/// How much the camera sees, top to bottom, in degrees.
pub const FIELD_OF_VIEW: f64 = 40.0;
/// Nothing nearer the eye than this is drawn or picked, in metres.
const NEAR: f64 = 0.1;
/// How far a one-pixel drag turns the view, in degrees.
const TURN_PER_PIXEL: f64 = 0.5;
/// How far the view can tilt up or down, short of straight up or down.
const MOST_TILT: f64 = 85.0;

const GRID: Color = Color::rgb(0x3a, 0x41, 0x4e);
const AXIS: Color = Color::rgb(0x56, 0x5f, 0x70);
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
fn focal(height: usize) -> f64 {
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

/// What to draw.
pub struct Look<'a> {
    /// The model, if there is one to show.
    pub model: Option<&'a Model>,
    /// Where the camera is.
    pub orbit: &'a Orbit,
    /// The selected part: brightened, with the box round it drawn.
    pub selected: Option<&'a Part>,
    /// The sockets to show.
    pub markers: &'a [Marker],
    /// Which of `markers` the pointer is on.
    pub hot: Option<usize>,
    /// Whether to draw the grid on the ground.
    pub grid: bool,
}

fn forge(point: Vec3) -> duckforge::prelude::Vec3 {
    duckforge::prelude::vec3(point.x as f32, point.y as f32, point.z as f32)
}

/// Draws the viewport, `width` x `height` pixels.
pub fn draw(look: &Look<'_>, width: usize, height: usize) -> Picture {
    let (width, height) = (width.max(1), height.max(1));
    let mut canvas = Canvas::new(width, height);
    canvas.clear(BACKGROUND);
    let orbit = look.orbit;
    // Duck Forge's camera stays at zero, looking straight ahead: the
    // model is brought to it.
    let camera = Camera3D {
        position: duckforge::prelude::vec3(0.0, 0.0, 0.0),
        yaw: 0.0,
        pitch: 0.0,
        fov: (FIELD_OF_VIEW as f32).to_radians(),
    };
    let line = |canvas: &mut Canvas, from: Vec3, to: Vec3, color: Color| {
        let (mut a, mut b) = (orbit.to_view(from), orbit.to_view(to));
        if a.z < NEAR && b.z < NEAR {
            return;
        }
        // Cut the part behind the eye off.
        if a.z < NEAR {
            std::mem::swap(&mut a, &mut b);
        }
        if b.z < NEAR {
            let along = (a.z - NEAR) / (a.z - b.z);
            b = a + (b - a) * along;
        }
        if let (Some((from, _)), Some((to, _))) = (
            camera.project(forge(a), canvas),
            camera.project(forge(b), canvas),
        ) {
            canvas.draw_line(from, to, color);
        }
    };

    if look.grid {
        // One square to a metre, a little wider than the model.
        let reach = look.model.and_then(Model::bounds).map_or(2.0, |bounds| {
            let far = bounds.min.x.abs().max(bounds.max.x.abs());
            far.max(bounds.min.z.abs().max(bounds.max.z.abs()))
        });
        let half = (reach.ceil() as i32 + 1).clamp(2, 30);
        for at in -half..=half {
            let (at_f, half_f) = (f64::from(at), f64::from(half));
            let color = if at == 0 { AXIS } else { GRID };
            line(
                &mut canvas,
                vec3(at_f, 0.0, -half_f),
                vec3(at_f, 0.0, half_f),
                color,
            );
            line(
                &mut canvas,
                vec3(-half_f, 0.0, at_f),
                vec3(half_f, 0.0, at_f),
                color,
            );
        }
    }

    if let Some(model) = look.model {
        let lit = look.selected.map(|part| part.faces.clone());
        let mesh = mesh_of(model, orbit, lit.as_ref());
        // The light comes from over the viewer's left shoulder, whichever
        // way the model is turned.
        canvas.set_lighting(Lighting {
            sun_direction: duckforge::prelude::vec3(-0.35, 0.75, -1.0),
            ambient: 0.45,
            ..Lighting::default()
        });
        canvas.draw_mesh(&mesh, &Transform::default(), &camera);
    }

    if let Some(bounds) = look.selected.and_then(|part| part.bounds) {
        let corner = |x: bool, y: bool, z: bool| {
            vec3(
                if x { bounds.max.x } else { bounds.min.x },
                if y { bounds.max.y } else { bounds.min.y },
                if z { bounds.max.z } else { bounds.min.z },
            )
        };
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            line(&mut canvas, corner(false, a, b), corner(true, a, b), CHOSEN);
            line(&mut canvas, corner(a, false, b), corner(a, true, b), CHOSEN);
            line(&mut canvas, corner(a, b, false), corner(a, b, true), CHOSEN);
        }
    }

    let eye = orbit.eye();
    for (at, marker) in look.markers.iter().enumerate() {
        let Some((x, y)) = orbit.on_screen(marker.at, width, height) else {
            continue;
        };
        let centre: Vec2 = vec2(x as f32, y as f32);
        let hot = look.hot == Some(at);
        let faces_us = marker.facing.dot(eye - marker.at) >= 0.0;
        let (radius, color) = match (hot, faces_us) {
            (true, _) => (6.0, CHOSEN),
            (false, true) => (4.0, MARKER),
            (false, false) => (3.0, MARKER_BEHIND),
        };
        canvas.fill_circle(centre, radius + 1.5, MARKER_EDGE);
        canvas.fill_circle(centre, radius, color);
    }

    Picture {
        width,
        height,
        pixels: canvas.pixels().to_vec(),
    }
}

/// The model as a Duck Forge mesh, already turned for the camera. The
/// faces in `lit` are brightened.
fn mesh_of(model: &Model, orbit: &Orbit, lit: Option<&Range<usize>>) -> Mesh {
    let mut mesh = Mesh::new();
    for (at, face) in model.faces.iter().enumerate() {
        let mut rgb = face
            .color
            .and_then(|color| model.palette.get(color))
            .map_or(UNCOLOURED, |color| color.rgb);
        if lit.is_some_and(|lit| lit.contains(&at)) {
            rgb = rgb.map(|channel| channel + (255 - channel) / 3);
        }
        let color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        let first = mesh.vertices.len();
        for &corner in &face.corners {
            mesh.add_vertex(forge(orbit.to_view(corner)));
        }
        for step in 1..face.corners.len().saturating_sub(1) {
            mesh.add_triangle(first, first + step, first + step + 1, color);
        }
    }
    mesh
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

/// Finds the face under the pointer at (`x`, `y`), in pixels from the top
/// left of a picture `width` x `height`. Faces in `skip` are looked
/// through.
pub fn pick(
    model: &Model,
    orbit: &Orbit,
    (width, height): (usize, usize),
    (x, y): (f64, f64),
    skip: Option<&Range<usize>>,
) -> Option<Hit> {
    let focal = focal(height);
    // The line from the eye through the pointer, as the camera sees it.
    let ray = vec3(
        (x - width as f64 / 2.0) / focal,
        -(y - height as f64 / 2.0) / focal,
        1.0,
    );
    let mut nearest: Option<(f64, usize)> = None;
    for (at, face) in model.faces.iter().enumerate() {
        if skip.is_some_and(|skip| skip.contains(&at)) {
            continue;
        }
        let seen: Vec<Vec3> = face.corners.iter().map(|&c| orbit.to_view(c)).collect();
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
        point: orbit.from_view(ray * along),
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
