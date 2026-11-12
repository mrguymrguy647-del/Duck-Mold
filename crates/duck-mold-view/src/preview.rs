//! `duck-mold preview`: a window that shows a model, to look at it from
//! every side while the recipe is being written.
//!
//! - Drag with the left mouse button to orbit; roll the wheel to zoom;
//!   Escape quits.
//! - Twice a second it checks whether the recipe, or any file the recipe
//!   uses, has been saved (by its time and size). If so, it builds the
//!   model again. If the new version has mistakes, they are shown in the
//!   window (and printed), and the last good model stays up, so a
//!   half-typed recipe never blanks it.
//! - Nothing else: no editing, no menus.
//!
//! Duck Forge runs the window, as it does for a game, and draws with the
//! same CPU renderer as `render`. [`Preview`] is an ordinary Duck Forge
//! [`Game`], so the tests drive it without a window, through
//! [`duckforge::Simulation`].

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use duck_mold_core::inspect::inspect;
use duck_mold_core::load::display_path;
use duck_mold_core::math::Vec3 as MoldVec3;
use duck_mold_core::tools;
use duckforge::prelude::{
    Camera3D, Canvas, Color, Config, Context, Game, Key, Lighting, Mesh, MouseButton, Renderer,
    Transform, Vec3, vec2, vec3,
};

use crate::render::{BACKGROUND, forge_mesh};

/// Whether this program was built with the window (the `preview`
/// feature, on unless built with `--no-default-features`).
pub const HAS_WINDOW: bool = cfg!(feature = "preview");

/// How often the files are checked, in seconds.
const CHECK_EVERY: f32 = 0.5;
/// How much the camera sees, top to bottom, in degrees.
const FIELD_OF_VIEW: f32 = 40.0;
/// How far a one-pixel drag turns the view, in radians.
const TURN_PER_PIXEL: f32 = 0.01;
/// How close and how far the wheel can zoom: the distance at which the
/// whole model fits, times this.
const ZOOM: (f32, f32) = (0.25, 6.0);
/// How far the view can tilt up or down, short of straight up or down.
const MOST_PITCH: f32 = 1.5;

const TEXT: Color = Color::rgb(0xe6, 0xe9, 0xef);
const QUIET: Color = Color::rgb(0x9a, 0xa1, 0xad);
const ERROR: Color = Color::rgb(0xff, 0x7a, 0x6e);
const WARNING: Color = Color::rgb(0xf2, 0xc8, 0x4b);

/// A file being watched: where it is, and its time and size when it was
/// last read (`None` if it couldn't be found).
struct Watched {
    path: PathBuf,
    stamp: Option<(SystemTime, u64)>,
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// The model being shown.
struct Shown {
    mesh: Mesh,
    /// The middle of its box.
    middle: Vec3,
    /// Half the length of its box's diagonal: a ball this big holds it.
    radius: f32,
    /// Its name, size and triangle count.
    headline: String,
}

/// One line of text over the model: a mistake, or a warning.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The words.
    pub text: String,
    /// True for a mistake, false for a warning.
    pub is_error: bool,
}

/// The preview window's game: the model, the view, and the files to
/// watch.
pub struct Preview {
    path: PathBuf,
    shown_as: String,
    watched: Vec<Watched>,
    model: Option<Shown>,
    lines: Vec<Line>,
    /// Turned round the model, in radians: 0 looks at its front.
    yaw: f32,
    /// Tilted up or down, in radians: positive looks down from above.
    pitch: f32,
    /// How far away, as a multiple of the distance at which it all fits.
    zoom: f32,
    since_check: f32,
    builds: usize,
}

impl Preview {
    /// Builds the model in `path` (a recipe, or a model file) and gets
    /// ready to show it, from the front right and a little above.
    pub fn open(path: impl Into<PathBuf>) -> Preview {
        let path = path.into();
        let mut preview = Preview {
            shown_as: display_path(&path),
            path,
            watched: Vec::new(),
            model: None,
            lines: Vec::new(),
            yaw: 30f32.to_radians(),
            pitch: 20f32.to_radians(),
            zoom: 1.0,
            since_check: 0.0,
            builds: 0,
        };
        preview.build();
        preview
    }

    /// The window: titled after the file, resizable, drawn on the CPU.
    pub fn config(&self) -> Config {
        Config {
            title: format!("{} - Duck Mold preview", self.shown_as),
            width: 480,
            height: 360,
            scale: 2,
            renderer: Renderer::Cpu,
            resizable: true,
            ..Config::default()
        }
    }

    /// How many times the model has been built (or tried to be).
    pub fn builds(&self) -> usize {
        self.builds
    }

    /// The mistakes and warnings shown over the model.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The line under the model: its name, size and triangle count.
    /// `None` until a version of it has built.
    pub fn headline(&self) -> Option<&str> {
        self.model.as_ref().map(|model| model.headline.as_str())
    }

    /// The view: turned (radians), tilted (radians), and zoomed (1 is
    /// where the whole model fits).
    pub fn view(&self) -> (f32, f32, f32) {
        (self.yaw, self.pitch, self.zoom)
    }

    /// The files watched for changes.
    pub fn watched(&self) -> Vec<&Path> {
        self.watched
            .iter()
            .map(|file| file.path.as_path())
            .collect()
    }

    /// Builds the model again from the files, keeps it if it built, and
    /// notes every file read, to watch.
    fn build(&mut self) {
        self.builds += 1;
        let (built, files) = tools::build_watching(&self.path);
        self.watched = files
            .into_iter()
            .map(|path| Watched {
                stamp: stamp(&path),
                path,
            })
            .collect();
        match built {
            Ok(built) => {
                let report = inspect(&built.model, &built.source, built.warnings);
                let bounds = built.model.bounds();
                let (middle, radius) = match bounds {
                    Some(bounds) => (bounds.center(), bounds.half().length()),
                    None => (MoldVec3::ZERO, 1.0),
                };
                self.model = Some(Shown {
                    mesh: forge_mesh(&built.model, |point| point),
                    middle: vec3(middle.x as f32, middle.y as f32, middle.z as f32),
                    radius: (radius as f32).max(0.01),
                    headline: report.headline(),
                });
                self.lines = report
                    .warnings
                    .iter()
                    .map(|warning| Line {
                        text: warning.to_string(),
                        is_error: false,
                    })
                    .collect();
            }
            Err(failure) => {
                // The last good model stays up; the mistakes go over it.
                self.lines = failure
                    .report()
                    .to_string()
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| Line {
                        text: line.to_owned(),
                        is_error: true,
                    })
                    .collect();
            }
        }
        for line in &self.lines {
            eprintln!("{}", line.text);
        }
    }

    /// True if a watched file has been saved, made or removed since it
    /// was read.
    fn changed(&self) -> bool {
        self.watched
            .iter()
            .any(|file| stamp(&file.path) != file.stamp)
    }

    /// Where the camera is: round the model's middle, far enough back to
    /// see all of it, times the zoom.
    fn camera(&self, model: &Shown) -> Camera3D {
        let fits = model.radius / (FIELD_OF_VIEW.to_radians() / 2.0).sin();
        let away = vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        );
        let mut camera =
            Camera3D::looking_at(model.middle + away * (fits * self.zoom), model.middle);
        camera.fov = FIELD_OF_VIEW.to_radians();
        camera
    }
}

impl Game for Preview {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.was_pressed(Key::Escape) {
            ctx.quit();
        }
        if ctx.input.is_mouse_down(MouseButton::Left) {
            let drag = ctx.input.mouse_delta();
            // Dragging right turns the model to the right: the camera goes
            // the other way round it.
            self.yaw -= drag.x * TURN_PER_PIXEL;
            self.pitch = (self.pitch + drag.y * TURN_PER_PIXEL).clamp(-MOST_PITCH, MOST_PITCH);
        }
        let scroll = ctx.input.scroll();
        if scroll != 0.0 {
            // Rolling up (away from you) comes closer.
            self.zoom = (self.zoom * 0.85f32.powf(scroll)).clamp(ZOOM.0, ZOOM.1);
        }
        self.since_check += ctx.dt();
        if self.since_check >= CHECK_EVERY {
            self.since_check = 0.0;
            if self.changed() {
                self.build();
            }
        }
        ctx.idle_ok(); // waiting for the files is what it is for
        ctx.report("builds", self.builds);
        ctx.report("mistakes", self.lines.iter().filter(|l| l.is_error).count());
    }

    fn draw(&self, canvas: &mut Canvas) {
        canvas.clear(BACKGROUND);
        let (width, height) = (canvas.width() as f32, canvas.height() as f32);
        if let Some(model) = &self.model {
            let camera = self.camera(model);
            // The light comes from over the viewer's shoulder, whichever
            // way the model has been turned.
            let towards_camera = camera.position - model.middle;
            canvas.set_lighting(Lighting {
                sun_direction: towards_camera + vec3(0.0, towards_camera.length() * 0.6, 0.0),
                ambient: 0.4,
                ..Lighting::default()
            });
            canvas.draw_mesh(&model.mesh, &Transform::default(), &camera);
            let below = height - Canvas::line_height(1) * 2.0 - 2.0;
            canvas.draw_text(&model.headline, vec2(4.0, below), 1, TEXT);
        } else {
            canvas.draw_text("nothing to show yet", vec2(4.0, height / 2.0), 1, QUIET);
        }
        canvas.draw_text(
            "drag: orbit   wheel: zoom   esc: quit   saved changes show here",
            vec2(4.0, height - Canvas::line_height(1) - 2.0),
            1,
            QUIET,
        );
        // Mistakes and warnings at the top, wrapped to fit, as many lines
        // as fit in the top half.
        let most = ((height / 2.0) / Canvas::line_height(1)) as usize;
        let rows = self.lines.iter().flat_map(|line| {
            let color = if line.is_error { ERROR } else { WARNING };
            wrap(&line.text, width - 8.0)
                .into_iter()
                .map(move |row| (row, color))
        });
        for (at, (row, color)) in rows.take(most).enumerate() {
            canvas.draw_text(
                &row,
                vec2(4.0, 4.0 + at as f32 * Canvas::line_height(1)),
                1,
                color,
            );
        }
    }
}

/// `text` in lines that fit `room` pixels at scale 1, broken at spaces
/// (and inside a word too long for a line on its own).
fn wrap(text: &str, room: f32) -> Vec<String> {
    let fits = |line: &str| Canvas::text_width(line, 1) <= room;
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in text.split(' ') {
        let longer = if row.is_empty() {
            word.to_owned()
        } else {
            format!("{row} {word}")
        };
        if fits(&longer) {
            row = longer;
            continue;
        }
        if !row.is_empty() {
            rows.push(std::mem::take(&mut row));
        }
        // A word too long for a whole line is cut into pieces.
        let mut rest = word.to_owned();
        while !fits(&rest) {
            let mut piece = String::new();
            for c in rest.chars() {
                if !fits(&format!("{piece}{c}")) {
                    break;
                }
                piece.push(c);
            }
            if piece.is_empty() {
                break; // not even one letter fits
            }
            rest = rest[piece.len()..].to_owned();
            rows.push(piece);
        }
        row = rest;
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

/// `duck-mold preview`: opens the window and runs until it is closed.
pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        return Err(format!(
            "can't read \"{}\": there is no such file",
            display_path(path)
        )
        .into());
    }
    if !HAS_WINDOW {
        return Err(
            "this duck-mold was built without its preview window (with --no-default-features). \
             Build it with the default features to open one, or use duck-mold render to save \
             pictures"
                .into(),
        );
    }
    let preview = Preview::open(path);
    duckforge::run(preview.config(), preview).map_err(|error| {
        // Duck Forge's advice is for games; this is the preview's.
        let first = error.to_string();
        let first = first.lines().next().unwrap_or_default().to_owned();
        format!("{first} On a computer without a screen, use duck-mold render to save pictures.")
            .into()
    })
}
