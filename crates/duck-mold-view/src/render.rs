//! `duck-mold render`: one PNG with four views of a model.
//!
//! The views are laid out two by two, each a square:
//!
//! ```text
//! front         | right side
//!                 |
//! ----------------+----------------
//! back            | three-quarter
//!                 |
//! ---------------------------------
//! robot: 0.90 x 1.83 x 0.39 m, 384 triangles
//! ```
//!
//! *front* looks the model in the face; *right side* looks at it from its
//! right, so it faces right in the picture; *back* is from behind; and
//! *three-quarter* is from the front right, 45° round and 30° above. All
//! four are drawn from the same distance with the same zoom, so the model
//! is the same size in each. The camera is far away and zoomed in (a 20°
//! view, where Duck Forge games usually use 60°), so the views look
//! nearly flat, like drawings, and sizes can be compared by eye.

use std::path::Path;

use duck_mold_core::inspect::{
    AnimationInspection, Inspection, inspect, inspect_animation, inspect_pose,
};
use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::load::display_path;
use duck_mold_core::math::Bounds;
use duck_mold_core::math::{Vec3, sin_cos_degrees, vec3};
use duck_mold_core::model::Model;
use duck_mold_core::motion;
use duck_mold_core::motion::{Animation, Motion};
use duck_mold_core::tools::{self, Failure};
use duckforge::prelude::{Camera3D, Canvas, Color, Mesh, Rect, Transform, vec2};
use serde::Serialize;

/// The background: a dark blue-grey, so that the light grey of a face
/// nobody coloured still shows.
pub const BACKGROUND: Color = Color::rgb(0x2a, 0x2f, 0x3a);
/// The lines between the views.
const LINE: Color = Color::rgb(0x45, 0x4c, 0x5a);
/// The words.
const TEXT: Color = Color::rgb(0xe6, 0xe9, 0xef);
/// A face nobody coloured: the same grey a `.glb` gives it.
const UNCOLOURED: [u8; 3] = [180, 180, 180];

/// How much the camera sees, top to bottom, in degrees. Narrow, so the
/// views look nearly flat.
const FIELD_OF_VIEW: f64 = 20.0;
/// Room around the model: the ball around it fills 1 / 1.1 of the view.
const MARGIN: f64 = 1.1;

/// How big each view is, in pixels, unless `--size` says otherwise.
pub const DEFAULT_SIZE: usize = 256;
/// The smallest view `--size` allows: below it the names don't fit.
pub const SMALLEST_SIZE: usize = 128;
/// The largest view `--size` allows: the whole picture is then 4097
/// pixels wide.
pub const LARGEST_SIZE: usize = 2048;

/// One of the four views: where the camera is, seen from the model.
struct View {
    /// Its name in `--json`.
    name: &'static str,
    /// Its name in the picture.
    label: &'static str,
    /// How far round the camera is from the model's front, in degrees,
    /// towards the model's right.
    round: f64,
    /// How far above level the camera is, in degrees.
    above: f64,
}

const VIEWS: [View; 4] = [
    View {
        name: "front",
        label: "front",
        round: 0.0,
        above: 0.0,
    },
    View {
        name: "right",
        label: "right side",
        round: 90.0,
        above: 0.0,
    },
    View {
        name: "back",
        label: "back",
        round: 180.0,
        above: 0.0,
    },
    View {
        name: "three_quarter",
        label: "three-quarter",
        round: 45.0,
        above: 30.0,
    },
];

impl View {
    /// Which way the camera is from the model, 1 long.
    fn from(&self) -> Vec3 {
        let (sin_round, cos_round) = sin_cos_degrees(self.round);
        let (sin_above, cos_above) = sin_cos_degrees(self.above);
        vec3(cos_above * sin_round, sin_above, cos_above * cos_round)
    }

    /// The directions in the model that end up pointing right, up and
    /// away from the camera in the picture. They are worked out from the
    /// two angles directly, so every computer gets the same numbers.
    fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let (sin_round, cos_round) = sin_cos_degrees(self.round);
        let (sin_above, cos_above) = sin_cos_degrees(self.above);
        let right = vec3(-cos_round, 0.0, sin_round);
        let up = vec3(-sin_above * sin_round, cos_above, -sin_above * cos_round);
        let away = -self.from();
        (right, up, away)
    }
}

/// Where one view is in the picture, for `--json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewPlace {
    /// `front`, `right`, `back` or `three_quarter`.
    pub name: &'static str,
    /// The direction from the model to the camera, in the model's own
    /// directions (x right, y up, z forward): `[0, 0, 1]` is in front.
    pub from: [f64; 3],
    /// The view's left edge, in pixels from the picture's left edge.
    pub left: usize,
    /// The view's top edge, in pixels from the picture's top edge.
    pub top: usize,
    /// How wide and tall it is, in pixels.
    pub size: usize,
}

/// The finished picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    /// How wide, in pixels.
    pub width: usize,
    /// How tall, in pixels.
    pub height: usize,
    /// The pixels, row by row from the top, each `0x00RRGGBB`.
    pub pixels: Vec<u32>,
    /// Where each view is.
    pub views: Vec<ViewPlace>,
}

/// Pixels (row by row from the top, each `0x00RRGGBB`) as a PNG file: 8
/// bits each of red, green and blue.
pub fn png_of(width: usize, height: usize, pixels: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixels.len() * 3);
    for &pixel in pixels {
        let [_, r, g, b] = pixel.to_be_bytes();
        bytes.extend_from_slice(&[r, g, b]);
    }
    let mut png = Vec::new();
    let width = u32::try_from(width).expect("a picture is far narrower than that");
    let height = u32::try_from(height).expect("a picture is far shorter than that");
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("writing to memory can't fail");
    writer
        .write_image_data(&bytes)
        .expect("there is one pixel for each place in the picture");
    writer.finish().expect("writing to memory can't fail");
    png
}

impl Sheet {
    /// The picture as a PNG file: 8 bits each of red, green and blue.
    pub fn to_png(&self) -> Vec<u8> {
        png_of(self.width, self.height, &self.pixels)
    }

    /// The colour of one pixel, as `0xRRGGBB`.
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        self.pixels[y * self.width + x]
    }
}

/// Draws the four views of `model`, each `size` pixels square, with
/// `caption` under them.
///
/// A line one pixel wide runs between the views and above the caption,
/// so the picture is `2 * size + 1` pixels wide.
pub fn render_sheet(model: &Model, caption: &str, size: usize) -> Sheet {
    let scale = text_scale(size);
    let pad = 2 * scale as usize;
    let strip = Canvas::line_height(scale) as usize + 2 * pad;
    let grid = 2 * size + 1; // two views and the line between them
    let (width, height) = (grid, grid + 1 + strip);

    let mut sheet = Canvas::new(width, height);
    sheet.clear(BACKGROUND);
    let mut views = Vec::with_capacity(VIEWS.len());
    for (at, view) in VIEWS.iter().enumerate() {
        let (left, top) = ((at % 2) * (size + 1), (at / 2) * (size + 1));
        let picture = draw_view(model, view, size);
        for (index, &pixel) in picture.pixels().iter().enumerate() {
            let (x, y) = (left + index % size, top + index / size);
            sheet.set_pixel(x as i32, y as i32, Color::from_hex(pixel));
        }
        sheet.draw_text(
            view.label,
            vec2((left + pad) as f32, (top + pad) as f32),
            scale,
            TEXT,
        );
        views.push(ViewPlace {
            name: view.name,
            from: view.from().to_array(),
            left,
            top,
            size,
        });
    }
    // Lines between the views, and above the caption.
    let (grid, size_f) = (grid as f32, size as f32);
    sheet.fill_rect(Rect::new(size_f, 0.0, 1.0, grid), LINE);
    sheet.fill_rect(Rect::new(0.0, size_f, grid, 1.0), LINE);
    sheet.fill_rect(Rect::new(0.0, grid, grid, 1.0), LINE);

    let caption = fit(caption, width - 2 * pad, scale);
    let caption_top = height - strip + (strip - Canvas::text_height(scale) as usize) / 2;
    sheet.draw_text(&caption, vec2(pad as f32, caption_top as f32), scale, TEXT);

    Sheet {
        width,
        height,
        pixels: sheet.pixels().to_vec(),
        views,
    }
}

/// How big the words are: Duck Forge's font is 5 pixels tall, times this.
fn text_scale(size: usize) -> u32 {
    u32::try_from((size / 128).max(1)).unwrap_or(1)
}

/// `text`, cut short with `...` if it is wider than `room` pixels.
fn fit(text: &str, room: usize, scale: u32) -> String {
    let fits = |text: &str| Canvas::text_width(text, scale) <= room as f32;
    if fits(text) {
        return text.to_owned();
    }
    let mut kept: Vec<char> = text.chars().collect();
    while !kept.is_empty() {
        kept.pop();
        let shorter = format!("{}...", kept.iter().collect::<String>().trim_end());
        if fits(&shorter) {
            return shorter;
        }
    }
    String::new()
}

/// Draws one view, `size` pixels square.
///
/// The model is moved so the middle of its box is at the origin, shrunk
/// so the ball around that box has a radius of 1, and turned so the
/// view's right, up and away become Duck Forge's x, y and z. The camera
/// then always stands at the same place, looking straight ahead, and
/// Duck Forge does no turning of its own.
fn draw_view(model: &Model, view: &View, size: usize) -> Canvas {
    let Some(bounds) = model.bounds() else {
        let mut canvas = Canvas::new(size, size);
        canvas.clear(BACKGROUND);
        return canvas; // nothing to draw
    };
    draw_view_of(model, view, size, bounds.center(), bounds.half().length())
}

/// Draws one view, `size` pixels square, of what is within `radius` of
/// `middle`: several pictures drawn with the same two show the model
/// from the same place, at the same size.
fn draw_view_of(model: &Model, view: &View, size: usize, middle: Vec3, radius: f64) -> Canvas {
    let mut canvas = Canvas::new(size, size);
    canvas.clear(BACKGROUND);
    let shrink = if radius > 0.0 { 1.0 / radius } else { 1.0 };
    let (right, up, away) = view.axes();
    let mesh = forge_mesh(model, |point| {
        let from_middle = (point - middle) * shrink;
        vec3(
            from_middle.dot(right),
            from_middle.dot(up),
            from_middle.dot(away),
        )
    });
    let (sin_half, _) = sin_cos_degrees(FIELD_OF_VIEW / 2.0);
    // Far enough back for a ball of radius 1 (with the margin) to fit.
    let distance = MARGIN / sin_half;
    let camera = Camera3D {
        position: duckforge::prelude::vec3(0.0, 0.0, -(distance as f32)),
        yaw: 0.0,
        pitch: 0.0,
        fov: (FIELD_OF_VIEW as f32).to_radians(),
    };
    canvas.draw_mesh(&mesh, &Transform::default(), &camera);
    canvas
}

/// One small picture of a model, `size` pixels square: the three-quarter
/// view of `render`'s sheet, with no caption. The Studio's parts library
/// shows these. Each pixel is `0x00RRGGBB`, row by row from the top.
pub fn render_small(model: &Model, size: usize) -> Vec<u32> {
    let view = &VIEWS[VIEWS.len() - 1];
    draw_view(model, view, size.max(1)).pixels().to_vec()
}

/// The model as a Duck Forge mesh, each corner moved by `place` first.
///
/// Every face keeps corners of its own, and its triangles keep the
/// model's order and winding, which is already Duck Forge's: clockwise
/// seen from the front.
pub fn forge_mesh(model: &Model, place: impl Fn(Vec3) -> Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    for face in &model.faces {
        let [r, g, b] = face
            .color
            .and_then(|color| model.palette.get(color))
            .map_or(UNCOLOURED, |color| color.rgb);
        let color = Color::rgb(r, g, b);
        let first = mesh.vertices.len();
        for &corner in &face.corners {
            let at = place(corner);
            mesh.add_vertex(duckforge::prelude::vec3(
                at.x as f32,
                at.y as f32,
                at.z as f32,
            ));
        }
        for at in 1..face.corners.len().saturating_sub(1) {
            mesh.add_triangle(first, first + at, first + at + 1, color);
        }
    }
    mesh
}

/// What `duck-mold render` did: the report on the model, and the picture
/// it wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rendered {
    /// The same report `inspect` gives.
    #[serde(flatten)]
    pub inspection: Inspection,
    /// The picture that was written, as it was named.
    pub output: String,
    /// How big the file is.
    pub bytes: usize,
    /// How wide the picture is, in pixels.
    pub width: usize,
    /// How tall the picture is, in pixels.
    pub height: usize,
    /// Where each of the four views is in the picture.
    pub views: Vec<ViewPlace>,
}

/// `duck-mold render`: builds a file (a recipe, or a model to look at:
/// `.obj` or `.glb`) and writes the four views as a PNG. Without
/// `output`, the picture goes next to the file, with the same name and
/// the ending `.png`. Nothing is written if the recipe has mistakes.
pub fn render_file(
    path: &Path,
    output: Option<&Path>,
    size: usize,
    pose: Option<&str>,
) -> Result<Rendered, Failure> {
    if !(SMALLEST_SIZE..=LARGEST_SIZE).contains(&size) {
        return Err(Failure::Usage(Box::new(
            Issue::error(
                Code::OutOfRange,
                "size",
                if size < SMALLEST_SIZE {
                    format!("--size {size} is too small for the views' names to fit")
                } else {
                    format!("--size {size} is bigger than a view can be")
                },
            )
            .got(size)
            .range(SMALLEST_SIZE as f64, LARGEST_SIZE as f64)
            .suggest(format!("--size {DEFAULT_SIZE}")),
        )));
    }
    let built = tools::build(path)?;
    let mut inspection = inspect(&built.model, &built.source, built.warnings);
    // In a pose, the picture and the report are of the model as the pose
    // puts it.
    let model = match pose {
        None => built.model,
        Some(name) => {
            let pose = tools::pose_named(&built.model, name, &built.source)?;
            let posed = motion::posed(&built.model, &pose.joints);
            inspection = inspect_pose(&inspection, &built.model, &posed, &pose, &built.source);
            posed
        }
    };
    inspection.materials = built.materials;
    let sheet = render_sheet(&model, &inspection.headline(), size);
    let png = sheet.to_png();
    let output = match (output, pose) {
        (Some(output), _) => output.to_path_buf(),
        (None, None) => path.with_extension("png"),
        // `robot.wave.png`, beside `robot.png`.
        (None, Some(pose)) => path.with_extension(format!("{pose}.png")),
    };
    tools::write_file(&output, &png)?;
    Ok(Rendered {
        inspection,
        output: display_path(&output),
        bytes: png.len(),
        width: sheet.width,
        height: sheet.height,
        views: sheet.views,
    })
}

// ---- an animation as a strip of frames -------------------------------------------------

/// How many frames `render --animation` draws, unless `--frames` says
/// otherwise.
pub const DEFAULT_FRAMES: usize = 8;
/// The fewest and the most frames a strip can have.
pub const FRAMES_RANGE: (usize, usize) = (2, 60);
/// The widest a strip can be, in pixels.
const WIDEST_STRIP: usize = 32_768;

/// Where one frame is in a strip, for `--json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FramePlace {
    /// The frame's time, in seconds from the animation's start.
    pub at: f64,
    /// The frame's left edge, in pixels from the picture's left edge.
    pub left: usize,
    /// The frame's top edge, in pixels from the picture's top edge.
    pub top: usize,
    /// How wide and tall it is, in pixels.
    pub size: usize,
}

/// A finished strip of frames.
#[derive(Debug, Clone, PartialEq)]
pub struct Strip {
    /// How wide, in pixels.
    pub width: usize,
    /// How tall, in pixels.
    pub height: usize,
    /// The pixels, row by row from the top, each `0x00RRGGBB`.
    pub pixels: Vec<u32>,
    /// The view the frames are drawn from: `three_quarter`, `front`...
    pub view: &'static str,
    /// Where each frame is.
    pub frames: Vec<FramePlace>,
}

impl Strip {
    /// The picture as a PNG file.
    pub fn to_png(&self) -> Vec<u8> {
        png_of(self.width, self.height, &self.pixels)
    }
}

/// The view with this name, as `--view` writes it: `front`, `right`,
/// `back` or `three-quarter`.
fn view_named(name: &str) -> Option<&'static View> {
    let name = name.replace('-', "_");
    VIEWS.iter().find(|view| view.name == name)
}

/// The times of a strip's frames: evenly spaced, from the animation's
/// start. One that plays once ends on its last moment; one that loops
/// stops a step before, because its end is its start again.
pub fn strip_times(animation: &Animation, frames: usize) -> Vec<f64> {
    let length = animation.length();
    let steps = if animation.looping {
        frames
    } else {
        frames.saturating_sub(1)
    }
    .max(1);
    (0..frames)
        .map(|frame| length * frame as f64 / steps as f64)
        .collect()
}

/// Draws an animation as a strip: `frames` pictures of the model side
/// by side, evenly spaced in time, all from the same place and at the
/// same size, each with its time in its corner, and `caption` under
/// them.
pub fn render_strip(
    model: &Model,
    animation: &Animation,
    caption: &str,
    size: usize,
    frames: usize,
    view: &str,
) -> Strip {
    let view = view_named(view).unwrap_or(&VIEWS[VIEWS.len() - 1]);
    let moving = Motion::of(model, animation);
    // The camera takes in the model wherever the animation puts it, so
    // that it never leaves the picture: every frame of it, not only the
    // ones drawn.
    let mut all: Option<Bounds> = model.bounds();
    for at in motion::frame_times(animation.length()) {
        let posed = motion::posed(model, &moving.joints_at(at));
        all = match (all, posed.bounds()) {
            (Some(all), Some(more)) => Some(all.with(&more)),
            (all, more) => all.or(more),
        };
    }
    let (middle, radius) = all.map_or((Vec3::ZERO, 1.0), |bounds| {
        (bounds.center(), bounds.half().length())
    });

    let scale = text_scale(size);
    let pad = 2 * scale as usize;
    let under = Canvas::line_height(scale) as usize + 2 * pad;
    let row = frames * size + frames.saturating_sub(1); // the frames and the lines between
    let (width, height) = (row, size + 1 + under);
    let mut sheet = Canvas::new(width, height);
    sheet.clear(BACKGROUND);
    let mut places = Vec::with_capacity(frames);
    for (frame, at) in strip_times(animation, frames).into_iter().enumerate() {
        let left = frame * (size + 1);
        let posed = motion::posed(model, &moving.joints_at(at));
        let picture = draw_view_of(&posed, view, size, middle, radius);
        for (index, &pixel) in picture.pixels().iter().enumerate() {
            let (x, y) = (left + index % size, index / size);
            sheet.set_pixel(x as i32, y as i32, Color::from_hex(pixel));
        }
        sheet.draw_text(
            &time_label(at),
            vec2((left + pad) as f32, pad as f32),
            scale,
            TEXT,
        );
        if frame > 0 {
            sheet.fill_rect(Rect::new((left - 1) as f32, 0.0, 1.0, size as f32), LINE);
        }
        places.push(FramePlace {
            at,
            left,
            top: 0,
            size,
        });
    }
    sheet.fill_rect(Rect::new(0.0, size as f32, width as f32, 1.0), LINE);
    let caption = fit(caption, width - 2 * pad, scale);
    let caption_top = height - under + (under - Canvas::text_height(scale) as usize) / 2;
    sheet.draw_text(&caption, vec2(pad as f32, caption_top as f32), scale, TEXT);
    Strip {
        width,
        height,
        pixels: sheet.pixels().to_vec(),
        view: view.name,
        frames: places,
    }
}

/// A frame's time as it is written in its corner: `0.4 s`.
fn time_label(at: f64) -> String {
    let rounded = (at * 100.0).round() / 100.0 + 0.0;
    format!("{rounded} s")
}

/// What `duck-mold render --animation` did: the report on the
/// animation, and the picture it wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RenderedAnimation {
    /// The same report `inspect --animation` gives.
    #[serde(flatten)]
    pub inspection: AnimationInspection,
    /// The picture that was written, as it was named.
    pub output: String,
    /// How big the file is.
    pub bytes: usize,
    /// How wide the picture is, in pixels.
    pub width: usize,
    /// How tall the picture is, in pixels.
    pub height: usize,
    /// The view the frames are drawn from.
    pub view: &'static str,
    /// Where each frame is in the picture, and its time.
    pub strip: Vec<FramePlace>,
}

/// A mistake in what `render` was asked for.
fn usage(code: Code, field: &str, message: String) -> Issue {
    Issue::error(code, field, message)
}

/// `duck-mold render --animation`: builds a recipe and writes one of
/// its animations as a strip of frames. Without `output`, the picture
/// goes next to the recipe, named after both: `robot.walk.strip.png`.
pub fn render_animation_file(
    path: &Path,
    output: Option<&Path>,
    size: usize,
    animation: &str,
    frames: usize,
    view: Option<&str>,
) -> Result<RenderedAnimation, Failure> {
    let fail = |issue: Issue| Err(Failure::Usage(Box::new(issue)));
    if !(SMALLEST_SIZE..=LARGEST_SIZE).contains(&size) {
        return fail(
            usage(
                Code::OutOfRange,
                "size",
                if size < SMALLEST_SIZE {
                    format!("--size {size} is too small for the times to fit")
                } else {
                    format!("--size {size} is bigger than a frame can be")
                },
            )
            .got(size)
            .range(SMALLEST_SIZE as f64, LARGEST_SIZE as f64)
            .suggest(format!("--size {DEFAULT_SIZE}")),
        );
    }
    let (fewest, most) = FRAMES_RANGE;
    if !(fewest..=most).contains(&frames) {
        return fail(
            usage(
                Code::OutOfRange,
                "frames",
                format!("a strip has from {fewest} to {most} frames, not {frames}"),
            )
            .got(frames)
            .range(fewest as f64, most as f64)
            .suggest(format!("--frames {DEFAULT_FRAMES}")),
        );
    }
    if frames * (size + 1) > WIDEST_STRIP {
        return fail(
            usage(
                Code::OutOfRange,
                "frames",
                format!(
                    "{frames} frames of {size} pixels make a picture wider than {WIDEST_STRIP} pixels: ask for fewer frames or a smaller --size"
                ),
            )
            .got(frames),
        );
    }
    let view = view.unwrap_or("three-quarter");
    if view_named(view).is_none() {
        return fail(
            usage(
                Code::TypeMismatch,
                "view",
                format!("there is no view \"{view}\""),
            )
            .got(view)
            .one_of(view, ["front", "right", "back", "three-quarter"]),
        );
    }
    let built = tools::build(path)?;
    let moving = tools::animation_named(&built.model, animation, &built.source)?.clone();
    let rest = inspect(&built.model, &built.source, built.warnings);
    let inspection = inspect_animation(&rest, &built.model, &moving, &built.source);
    let strip = render_strip(
        &built.model,
        &moving,
        &inspection.headline(),
        size,
        frames,
        view,
    );
    let png = strip.to_png();
    let output = match output {
        Some(output) => output.to_path_buf(),
        // `robot.walk.strip.png`: a pose of the same name is
        // `robot.walk.png`.
        None => path.with_extension(format!("{animation}.strip.png")),
    };
    tools::write_file(&output, &png)?;
    Ok(RenderedAnimation {
        inspection,
        output: display_path(&output),
        bytes: png.len(),
        width: strip.width,
        height: strip.height,
        view: strip.view,
        strip: strip.frames,
    })
}
