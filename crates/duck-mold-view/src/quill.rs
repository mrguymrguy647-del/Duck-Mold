//! The round trip with Duck Quill: painting a part's image in a picture
//! program, and seeing the model change when it is saved.
//!
//! Nothing links the two programs but files. Duck Mold starts Duck Quill
//! on the image's PNG, with a guide to paint by and the face's colour as
//! paper; Duck Quill saves the PNG; Duck Mold, which watches the files a
//! recipe names, builds the model again. So any program that saves the
//! PNG does the same, and when Duck Quill isn't on the computer the
//! system's own picture program is started instead.
//!
//! This file finds Duck Quill, draws the guide, and starts the program.

use std::path::{Path, PathBuf};
use std::process::Command;

use duck_mold_core::math::Vec3;
use duck_mold_core::model::{Face, Model, Part};
use duck_mold_core::picture::write_png;
use duckforge::prelude::{Canvas, Color, vec2};

/// The program an image is painted in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum With {
    /// Duck Quill, at this place.
    DuckQuill(PathBuf),
    /// Whatever the system opens PNG files with.
    System,
}

/// Where Duck Mold keeps its settings: `%APPDATA%\Duck Mold` on Windows,
/// `~/.config/duck-mold` elsewhere. The Studio's list of recent files is
/// there too.
pub fn settings_folder() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Duck Mold"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|folder| folder.join("duck-mold"))
    }
}

/// Where Duck Quill keeps its settings, and its note of where its
/// program is (`program.txt`, written each time it starts).
pub fn duck_quill_settings() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Duck Quill"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|folder| folder.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|folder| folder.join("duck-quill"))
    }
}

/// The file in Duck Mold's settings folder that remembers what images
/// are painted with.
const REMEMBERED: &str = "paint-with.txt";

/// What was chosen before, read from Duck Mold's settings folder: Duck
/// Quill at a place, or the system's picture program. `None` if nothing
/// was chosen yet.
pub fn remembered(settings: &Path) -> Option<With> {
    let text = std::fs::read_to_string(settings.join(REMEMBERED)).ok()?;
    let line = text.lines().next()?.trim();
    match line.strip_prefix("duck-quill: ") {
        Some(path) => Some(With::DuckQuill(PathBuf::from(path))),
        None => (line == "system").then_some(With::System),
    }
}

/// Remembers what images are painted with, for next time. A failure to
/// write is passed over: it is only a convenience.
pub fn remember(settings: &Path, with: &With) {
    let line = match with {
        With::DuckQuill(path) => format!("duck-quill: {}\n", path.display()),
        With::System => "system\n".to_owned(),
    };
    let _ = std::fs::create_dir_all(settings);
    let _ = std::fs::write(settings.join(REMEMBERED), line);
}

/// The name of Duck Quill's program on this system.
pub fn duck_quill_name() -> &'static str {
    if cfg!(windows) {
        "DuckQuill.exe"
    } else {
        "DuckQuill"
    }
}

/// Finds Duck Quill's program. Looked for, in this order:
///
/// 1. the place chosen before (`chosen`), if the program is still there;
/// 2. Duck Quill's own note, `program.txt` in its settings folder
///    (`quill_settings`), which it writes each time it starts;
/// 3. beside Duck Mold: in its own folder, in a `Duck Quill` folder in
///    it, or in a `Duck Quill` folder beside it (`beside` is Duck Mold's
///    own folder);
/// 4. the folders of the `PATH` (`path`, as the system writes it).
///
/// `None` if it is in none of them.
pub fn find_duck_quill(
    chosen: Option<&Path>,
    quill_settings: Option<&Path>,
    beside: Option<&Path>,
    path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    if let Some(chosen) = chosen.filter(|path| path.is_file()) {
        return Some(chosen.to_path_buf());
    }
    if let Some(noted) = quill_settings
        .and_then(|folder| std::fs::read_to_string(folder.join("program.txt")).ok())
        .map(|text| PathBuf::from(text.trim()))
        .filter(|path| path.is_file())
    {
        return Some(noted);
    }
    // Duck Quill has been called by two names on systems without `.exe`.
    let names: &[&str] = if cfg!(windows) {
        &["DuckQuill.exe"]
    } else {
        &["DuckQuill", "duck-quill"]
    };
    let in_folder = |folder: &Path| {
        names
            .iter()
            .map(|name| folder.join(name))
            .find(|path| path.is_file())
    };
    if let Some(here) = beside {
        let mut folders = vec![here.to_path_buf(), here.join("Duck Quill")];
        if let Some(above) = here.parent() {
            folders.push(above.join("Duck Quill"));
        }
        if let Some(found) = folders.iter().find_map(|folder| in_folder(folder)) {
            return Some(found);
        }
    }
    path.and_then(|path| std::env::split_paths(path).find_map(|folder| in_folder(&folder)))
}

/// [`find_duck_quill`] on this computer, as it is now.
pub fn find_duck_quill_here(chosen: Option<&Path>) -> Option<PathBuf> {
    let own = std::env::current_exe().ok();
    let beside = own.as_deref().and_then(Path::parent);
    find_duck_quill(
        chosen,
        duck_quill_settings().as_deref(),
        beside,
        std::env::var_os("PATH").as_deref(),
    )
}

/// An image to be painted, and the program to paint it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintJob {
    /// The image's PNG file.
    pub image: PathBuf,
    /// The guide to paint by: a PNG of thin lines, the image's shape.
    /// Only Duck Quill draws it over the picture.
    pub guide: Option<PathBuf>,
    /// The colour of the face the image is on: Duck Quill's paper, so
    /// the picture looks as it will on the model.
    pub paper: [u8; 3],
    /// The program.
    pub with: With,
}

impl PaintJob {
    /// The program and the words it is started with, as a person would
    /// type them.
    pub fn words(&self) -> Vec<String> {
        match &self.with {
            With::DuckQuill(program) => {
                let mut words = vec![
                    program.display().to_string(),
                    self.image.display().to_string(),
                ];
                if let Some(guide) = &self.guide {
                    words.push("--guide".to_owned());
                    words.push(guide.display().to_string());
                }
                words.push("--paper".to_owned());
                words.push(format!(
                    "#{:02X}{:02X}{:02X}",
                    self.paper[0], self.paper[1], self.paper[2]
                ));
                words
            }
            With::System => {
                let opener = if cfg!(windows) {
                    vec!["cmd", "/C", "start", ""]
                } else if cfg!(target_os = "macos") {
                    vec!["open"]
                } else {
                    vec!["xdg-open"]
                };
                opener
                    .into_iter()
                    .map(str::to_owned)
                    .chain([self.image.display().to_string()])
                    .collect()
            }
        }
    }

    /// Starts the program, and doesn't wait for it. Says what was
    /// started, or why it couldn't be.
    pub fn start(&self) -> Result<String, String> {
        let words = self.words();
        let name = self
            .image
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let started = Command::new(&words[0])
            .args(&words[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        match (started, &self.with) {
            (Ok(_), With::DuckQuill(_)) => Ok(format!(
                "Opened {name} in Duck Quill. Save there (Ctrl+S) and the model shows it"
            )),
            (Ok(_), With::System) => Ok(format!(
                "Opened {name} in your picture program. Save there and the model shows it"
            )),
            (Err(error), With::DuckQuill(program)) => Err(format!(
                "Couldn't start Duck Quill ({}): {error}",
                program.display()
            )),
            (Err(error), With::System) => Err(format!(
                "Couldn't open {name} in a picture program: {error}. It is a plain PNG: open it from {}",
                self.image
                    .parent()
                    .map(|folder| folder.display().to_string())
                    .unwrap_or_default()
            )),
        }
    }
}

/// The colour of the guide's lines.
const GUIDE: Color = Color::rgb(0x3C, 0xD8, 0xF0);

/// Draws the guide for the image on some faces of a part: a see-through
/// PNG, several times the image's size (so its lines are thin and its
/// words readable when Duck Quill stretches it over the picture), with
///
/// - the outline of every face the image covers;
/// - a word at the image's top edge saying which way that is on the
///   part (`up`; `back` for a top seen from above), and `front` where a
///   wrapped image passes the part's front;
/// - a dotted outline where another part is joined on.
///
/// `faces` are the model's faces that show the image (their numbers);
/// `size` is the image's size in pixels. `None` if none of the faces
/// has a place on an image.
pub fn guide_png(model: &Model, part: &Part, faces: &[usize], size: [u32; 2]) -> Option<Vec<u8>> {
    let shown: Vec<&Face> = faces
        .iter()
        .filter_map(|&face| model.faces.get(face))
        .filter(|face| face.image.is_some())
        .collect();
    if shown.is_empty() {
        return None;
    }
    // Several times the image's size: about 512 pixels along its longer
    // side.
    let longer = size[0].max(size[1]).max(1);
    let times = (512 / longer).clamp(1, 32);
    let (wide, high) = ((size[0] * times) as usize, (size[1] * times) as usize);
    // Drawn on a canvas of one colour that nothing is drawn in, which
    // becomes see-through at the end.
    let nothing = Color::rgb(1, 2, 3);
    let mut canvas = Canvas::new(wide, high);
    canvas.clear(nothing);
    let on_guide = |at: [f64; 2]| vec2((at[0] * wide as f64) as f32, (at[1] * high as f64) as f32);
    // A line a few pixels thick.
    let thick = |canvas: &mut Canvas, from: [f64; 2], to: [f64; 2]| {
        let (from, to) = (on_guide(from), on_guide(to));
        for shift in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
            let by = vec2(shift.0, shift.1);
            canvas.draw_line(from + by, to + by, GUIDE);
        }
    };
    // The same, in dashes.
    let dashed = |canvas: &mut Canvas, from: [f64; 2], to: [f64; 2]| {
        let steps = 24;
        for step in (0..steps).step_by(2) {
            let at = |share: f64| {
                [
                    from[0] + (to[0] - from[0]) * share,
                    from[1] + (to[1] - from[1]) * share,
                ]
            };
            let (start, end) = (
                on_guide(at(f64::from(step) / f64::from(steps))),
                on_guide(at(f64::from(step + 1) / f64::from(steps))),
            );
            canvas.draw_line(start, end, GUIDE);
        }
    };

    // ---- the outline of every face
    for face in &shown {
        let on = &face.image.as_ref()?.at;
        for corner in 0..on.len() {
            thick(&mut canvas, on[corner], on[(corner + 1) % on.len()]);
        }
    }

    // ---- where other parts are joined on: the box round each, seen on
    // the face of this one it is nearest to.
    for other in model.all_parts() {
        if other.parent.as_deref() != Some(part.path.as_str()) || other.attached_to.is_none() {
            continue;
        }
        let Some(bounds) = other.bounds else { continue };
        let corners: Vec<Vec3> = (0..8)
            .map(|at| Vec3 {
                x: if at & 1 == 0 {
                    bounds.min.x
                } else {
                    bounds.max.x
                },
                y: if at & 2 == 0 {
                    bounds.min.y
                } else {
                    bounds.max.y
                },
                z: if at & 4 == 0 {
                    bounds.min.z
                } else {
                    bounds.max.z
                },
            })
            .collect();
        // The face it sits on: the one its nearest corner is closest to.
        let mut best: Option<(f64, [[f64; 2]; 2])> = None;
        for face in &shown {
            let Some(place) = OnFace::of(face) else {
                continue;
            };
            let gap = corners
                .iter()
                .map(|&corner| place.away(corner).abs())
                .fold(f64::INFINITY, f64::min);
            let places: Vec<[f64; 2]> = corners.iter().map(|&corner| place.at(corner)).collect();
            let low = [
                places.iter().map(|at| at[0]).fold(f64::INFINITY, f64::min),
                places.iter().map(|at| at[1]).fold(f64::INFINITY, f64::min),
            ];
            let top = [
                places
                    .iter()
                    .map(|at| at[0])
                    .fold(f64::NEG_INFINITY, f64::max),
                places
                    .iter()
                    .map(|at| at[1])
                    .fold(f64::NEG_INFINITY, f64::max),
            ];
            // Touching the face, and over the image.
            let over = low[0] < 1.0 && top[0] > 0.0 && low[1] < 1.0 && top[1] > 0.0;
            if gap <= 0.02 && over && best.as_ref().is_none_or(|(known, _)| gap < *known) {
                best = Some((gap, [low, top]));
            }
        }
        if let Some((_, [low, top])) = best {
            let clamp = |at: [f64; 2]| [at[0].clamp(0.0, 1.0), at[1].clamp(0.0, 1.0)];
            let (low, top) = (clamp(low), clamp(top));
            dashed(&mut canvas, low, [top[0], low[1]]);
            dashed(&mut canvas, [top[0], low[1]], top);
            dashed(&mut canvas, top, [low[0], top[1]]);
            dashed(&mut canvas, [low[0], top[1]], low);
        }
    }

    // ---- words: which way the image's top edge is on the part, and
    // where a wrapped image passes the part's front.
    let scale = (wide.min(high) / 96).clamp(1, 4) as u32;
    let word_at = |canvas: &mut Canvas, word: &str, middle: f32, top: f32| {
        let width = Canvas::text_width(word, scale);
        let left = (middle - width / 2.0).clamp(2.0, (wide as f32 - width - 2.0).max(2.0));
        canvas.draw_text(word, vec2(left, top), scale, GUIDE);
    };
    let directions = Directions::of(part);
    if let Some(up) = shown
        .iter()
        .find_map(|face| OnFace::of(face))
        .map(|place| place.up)
    {
        word_at(&mut canvas, directions.word(up), wide as f32 / 2.0, 3.0);
    }
    // A wrap: the faces face different ways round the part.
    let first = shown[0].normal();
    let wrapped = shown.iter().any(|face| match (face.normal(), first) {
        (Some(a), Some(b)) => a.dot(b) < 0.5,
        _ => false,
    });
    if wrapped {
        let front = shown
            .iter()
            .filter_map(|face| Some((face.normal()?.dot(directions.front), *face)))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((facing, face)) = front
            && facing > 0.7
        {
            let on = &face.image.as_ref()?.at;
            let middle = on.iter().map(|at| at[0]).sum::<f64>() / on.len() as f64;
            let below = Canvas::line_height(scale) + 6.0;
            word_at(&mut canvas, "front", (middle * wide as f64) as f32, below);
        }
    }

    let pixels: Vec<[u8; 4]> = canvas
        .pixels()
        .iter()
        .map(|&pixel| {
            if pixel == nothing.to_u32() {
                [0, 0, 0, 0]
            } else {
                let [_, r, g, b] = pixel.to_be_bytes();
                [r, g, b, 255]
            }
        })
        .collect();
    Some(write_png(wide as u32, high as u32, &pixels, false))
}

/// Where a point of the model is on a face's image: worked out from the
/// face's own corners and their places on the image.
struct OnFace {
    /// The face's first corner, and its place on the image.
    corner: Vec3,
    at: [f64; 2],
    /// How the place on the image changes for a step in the model along
    /// each of two directions in the face.
    across: (Vec3, [f64; 2]),
    along: (Vec3, [f64; 2]),
    /// The way the face faces.
    out: Vec3,
    /// The way in the model that is up on the image.
    up: Vec3,
}

impl OnFace {
    fn of(face: &Face) -> Option<OnFace> {
        let on = &face.image.as_ref()?.at;
        let out = face.normal()?;
        let corner = face.corners[0];
        // Two edges from the first corner that aren't in one line.
        let first = *face.corners.get(1)? - corner;
        let (second, second_at) = face.corners[2..]
            .iter()
            .zip(&on[2..])
            .map(|(&other, at)| (other - corner, *at))
            .find(|(edge, _)| first.cross(*edge).length() > 1e-12)?;
        let step = |at: [f64; 2]| [at[0] - on[0][0], at[1] - on[0][1]];
        let (a, b) = (step(on[1]), step(second_at));
        // Which way in the model the image's "up" (less down) is: solve
        // for the mix of the two edges that goes straight up the image.
        let det = a[0] * b[1] - a[1] * b[0];
        let up = if det.abs() > 1e-12 {
            // across * first + along * second moves (0, -1) on the image.
            let across = b[0] / det;
            let along = -a[0] / det;
            (first * across + second * along)
                .normalized()
                .unwrap_or(Vec3::ZERO)
        } else {
            Vec3::ZERO
        };
        Some(OnFace {
            corner,
            at: on[0],
            across: (first, a),
            along: (second, b),
            out,
            up,
        })
    }

    /// How far a point is in front of the face.
    fn away(&self, point: Vec3) -> f64 {
        (point - self.corner).dot(self.out)
    }

    /// Where a point, dropped straight onto the face, is on the image.
    fn at(&self, point: Vec3) -> [f64; 2] {
        let (first, a) = self.across;
        let (second, b) = self.along;
        let from = point - self.corner;
        // from = s * first + t * second (+ something straight out).
        let (ff, fs, ss) = (first.dot(first), first.dot(second), second.dot(second));
        let (pf, ps) = (from.dot(first), from.dot(second));
        let det = ff * ss - fs * fs;
        if det.abs() < 1e-18 {
            return self.at;
        }
        let s = (pf * ss - ps * fs) / det;
        let t = (ps * ff - pf * fs) / det;
        [
            self.at[0] + s * a[0] + t * b[0],
            self.at[1] + s * a[1] + t * b[1],
        ]
    }
}

/// The part's own six directions, in the model.
struct Directions {
    right: Vec3,
    up: Vec3,
    front: Vec3,
}

impl Directions {
    fn of(part: &Part) -> Directions {
        let turned = |direction: Vec3| {
            (part.place.apply(direction) - part.place.apply(Vec3::ZERO))
                .normalized()
                .unwrap_or(direction)
        };
        Directions {
            right: turned(Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            up: turned(Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
            front: turned(Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }),
        }
    }

    /// The word for the direction of the part that `way` is nearest to.
    fn word(&self, way: Vec3) -> &'static str {
        let ways = [
            (self.up, "up", "down"),
            (self.front, "front", "back"),
            (self.right, "right", "left"),
        ];
        let (axis, plus, minus) = ways
            .into_iter()
            .max_by(|a, b| way.dot(a.0).abs().total_cmp(&way.dot(b.0).abs()))
            .expect("three directions");
        if way.dot(axis) >= 0.0 { plus } else { minus }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir()
            .join("duck-mold-quill-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn program_in(folder: &Path) -> PathBuf {
        std::fs::create_dir_all(folder).unwrap();
        let program = folder.join(duck_quill_name());
        std::fs::write(&program, "a program").unwrap();
        program
    }

    #[test]
    fn duck_quill_is_looked_for_in_order() {
        let root = folder("find");
        let chosen = program_in(&root.join("chosen"));
        let noted = program_in(&root.join("noted"));
        let quill_settings = root.join("quill-settings");
        std::fs::create_dir_all(&quill_settings).unwrap();
        std::fs::write(
            quill_settings.join("program.txt"),
            format!("{}\n", noted.display()),
        )
        .unwrap();
        let mold = root.join("apps/Duck Mold");
        std::fs::create_dir_all(&mold).unwrap();
        let beside = program_in(&root.join("apps/Duck Quill"));
        let on_path = program_in(&root.join("bin"));
        let path = std::env::join_paths([root.join("empty"), root.join("bin")]).unwrap();

        let find = |chosen: Option<&Path>, settings: bool, mold: Option<&Path>, on_path: bool| {
            find_duck_quill(
                chosen,
                settings.then_some(quill_settings.as_path()),
                mold,
                on_path.then_some(path.as_os_str()),
            )
        };
        // The place chosen before comes first...
        assert_eq!(
            find(Some(&chosen), true, Some(&mold), true),
            Some(chosen.clone())
        );
        // ...then Duck Quill's own note of where it is...
        assert_eq!(find(None, true, Some(&mold), true), Some(noted.clone()));
        // ...then a "Duck Quill" folder beside Duck Mold's...
        assert_eq!(find(None, false, Some(&mold), true), Some(beside.clone()));
        // ...then the PATH.
        assert_eq!(find(None, false, None, true), Some(on_path.clone()));
        assert_eq!(find(None, false, None, false), None);
        // A place chosen before where it no longer is, is passed over.
        let gone = root.join("gone").join(duck_quill_name());
        assert_eq!(find(Some(&gone), true, None, false), Some(noted.clone()));
        // A note that leads nowhere is passed over too.
        std::fs::remove_file(&noted).unwrap();
        assert_eq!(find(None, true, None, true), Some(on_path));
    }

    #[test]
    fn what_was_chosen_is_remembered() {
        let settings = folder("remember").join("Duck Mold");
        assert_eq!(remembered(&settings), None);
        remember(&settings, &With::System);
        assert_eq!(remembered(&settings), Some(With::System));
        let program = PathBuf::from("C:/apps/Duck Quill/DuckQuill.exe");
        remember(&settings, &With::DuckQuill(program.clone()));
        assert_eq!(remembered(&settings), Some(With::DuckQuill(program)));
    }

    #[test]
    fn duck_quill_is_started_with_the_image_its_guide_and_the_faces_colour() {
        let job = PaintJob {
            image: PathBuf::from("models/textures/robot_face.png"),
            guide: Some(PathBuf::from("settings/guides/robot_face.png")),
            paper: [0x8A, 0x93, 0x9B],
            with: With::DuckQuill(PathBuf::from("apps/DuckQuill")),
        };
        assert_eq!(
            job.words(),
            [
                "apps/DuckQuill",
                "models/textures/robot_face.png",
                "--guide",
                "settings/guides/robot_face.png",
                "--paper",
                "#8A939B"
            ]
        );
        // The system's picture program gets the image alone.
        let plain = PaintJob {
            with: With::System,
            ..job
        };
        let words = plain.words();
        assert_eq!(words.last().unwrap(), "models/textures/robot_face.png");
        assert!(
            !words
                .iter()
                .any(|word| word == "--guide" || word == "--paper")
        );
    }
}
