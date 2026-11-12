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

use duck_mold_core::inspect::{Inspection, inspect};
use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::load::display_path;
use duck_mold_core::math::{Vec3, sin_cos_degrees, vec3};
use duck_mold_core::model::Model;
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
    let mut canvas = Canvas::new(size, size);
    canvas.clear(BACKGROUND);
    let Some(bounds) = model.bounds() else {
        return canvas; // nothing to draw
    };
    let middle = bounds.center();
    let radius = bounds.half().length();
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
pub fn render_file(path: &Path, output: Option<&Path>, size: usize) -> Result<Rendered, Failure> {
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
    inspection.materials = built.materials;
    let sheet = render_sheet(&built.model, &inspection.headline(), size);
    let png = sheet.to_png();
    let output = match output {
        Some(output) => output.to_path_buf(),
        None => path.with_extension("png"),
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
