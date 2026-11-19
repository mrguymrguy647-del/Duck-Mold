//! Everything Duck Mold Studio does, with no window.
//!
//! The Studio's window (the `duck-mold-studio` crate) only shows what is
//! here and passes clicks on, so all of it can be tested with no screen,
//! and driven by a script exactly as the window drives it.
//!
//! **The recipe is still the model.** A [`Studio`] keeps no model of its
//! own to edit: it reads the recipe file, builds it, and shows the
//! result. Every change it makes is an [`Edit`](duck_mold_core::edit::Edit)
//! written to the file at once (the same edits as `duck-mold edit`),
//! after which it reads and builds the file again. When another program
//! saves the file, it does the same.

mod edges;
mod group;
pub mod handles;
pub mod history;
pub mod images;
pub mod library;
mod paint;
mod pose;
mod shape;
pub mod shelf;
mod sockets;
mod timeline;
pub mod viewport;

pub use edges::CutAim;
pub use pose::{POSE_STEP, PoseTool};
pub use shape::{FaceTool, Mode};
pub use sockets::Clicked;
pub use timeline::{KEY_STEP, KeyMark};

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use duck_mold_core::edit::{self, Edit, Facts};
use duck_mold_core::inspect::{Inspection, PartReport, inspect};
use duck_mold_core::issue::Issue;
use duck_mold_core::load::display_path;
use duck_mold_core::math::{Bounds, Vec3};
use duck_mold_core::model::{Model, Part};
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::{self, Failure};

use crate::render;
use viewport::{Look, Orbit, Picture};

/// How often the files are checked for changes from outside, in seconds.
pub const CHECK_EVERY: f64 = 0.5;

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

/// The model being shown: the last version of the file that built.
struct Shown {
    model: Model,
    report: Inspection,
}

/// One line of the problems panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// The words: `line 70: the part "x" touches nothing...`.
    pub text: String,
    /// True for a mistake (nothing was built), false for a warning.
    pub is_error: bool,
    /// The part it is about, by path, when it is about one: clicking the
    /// line selects it.
    pub part: Option<String>,
}

/// One line of the outliner: a part, and how deep in the tree it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The part's path: `left_arm/hand`.
    pub path: String,
    /// Its own name: `hand`.
    pub name: String,
    /// What it is: `box`, `arm (assembly)`, `parts/lamp.gltf (mesh)`.
    pub kind: String,
    /// How many parts it is inside.
    pub depth: usize,
    /// How many triangles it has.
    pub triangles: usize,
    /// Whether it is hidden in the picture (itself, or what it is in).
    pub hidden: bool,
}

/// The Studio: one open file, the view of it, and what is known about it.
pub struct Studio {
    path: PathBuf,
    shown_as: String,
    /// True when nothing can be edited: `duck-mold preview`, and files
    /// that aren't recipes.
    look_only: bool,
    /// The file's text as it was last read.
    text: String,
    watched: Vec<Watched>,
    shown: Option<Shown>,
    problems: Vec<Problem>,
    /// Where the camera is.
    pub orbit: Orbit,
    /// Whether the grid on the ground is drawn.
    pub grid: bool,
    /// Whether the model is drawn as outlines only, to see through it.
    pub wire: bool,
    /// Whether the ground is darkened under the model. In the Studio
    /// only: `render`'s pictures have no shadow.
    pub shadow: bool,
    /// Whether the model's hit shapes are outlined over it (the Hits
    /// switch), as `render --colliders` draws them.
    pub hits: bool,
    /// Which version of the model the picture shows (View > Far
    /// version): 0 for the model itself, 1 or 2 for a far-away version.
    /// Only the picture changes, and only outside Pose mode.
    pub far: usize,
    /// That far-away version as a game draws it, kept until the model
    /// is built again: the build it is of, which version, and it.
    far_shown: std::cell::RefCell<Option<(usize, usize, Rc<Model>)>>,
    /// The part that is selected, by path. When several are, this is
    /// the one selected last: the one the panel shows.
    pub(crate) selected: Option<String>,
    /// The other selected parts, by path, in the order they were added.
    pub(crate) also: Vec<String>,
    /// The parts that aren't drawn, by path. Only the picture knows:
    /// the recipe doesn't, and a hidden part is exported like any other.
    pub(crate) hidden: Vec<String>,
    status: String,
    since_check: f64,
    builds: usize,
    /// Where the pointer is in the viewport, in pixels of a picture
    /// `view` big.
    pointer: Option<(f64, f64)>,
    view: (usize, usize),
    /// The part being dragged to another socket, by path.
    dragging: Option<String>,
    /// The changes that can be undone, oldest first, and those that were
    /// undone and can be made again.
    undo: Vec<history::Step>,
    redo: Vec<history::Step>,
    /// Where the undo history is kept on disk, if it is.
    history_file: Option<PathBuf>,
    /// What one picture keeps for the next: its canvas, the shadow.
    kept: std::cell::RefCell<viewport::Kept>,
    /// The file the Studio wrote last (a `.glb`, a picture, the recipe
    /// saved again), for the window to offer to show in its folder.
    written: Option<PathBuf>,
    /// The `.glb` files exported and not yet asked for (see
    /// [`Studio::take_exported`]).
    exported: Vec<PathBuf>,
    /// The recipe's `export` line, as of the last build that worked.
    export_line: Option<String>,
    /// True when the file isn't there any more (deleted or moved): the
    /// model stays, and nothing is changed until it is saved again.
    gone: bool,
    /// Text another program has just saved, that doesn't build: shown
    /// once it has stayed the same for one more check. A program that
    /// writes a file in pieces isn't shown as a mistake for a moment.
    settling: Option<String>,
    /// The same for the other files a recipe names (an image being
    /// saved by a picture program): how they were when they last didn't
    /// build.
    settling_files: Option<Vec<Option<(SystemTime, u64)>>>,
    library: library::Library,
    /// The animation library's files, as last read.
    shelf: shelf::Shelf,
    /// The palette colour the Paint tool paints with.
    pub(crate) paint: Option<String>,
    /// The image the Paint tool paints with instead, when one is picked:
    /// its path from the recipe's folder, or `Some(None)` for "no
    /// image". `None`: it paints with the colour.
    pub(crate) paint_image: Option<Option<String>>,
    /// Which tool is in hand: what the handles on the selected part are.
    pub tool: handles::Tool,
    /// A handle being dragged, with the edit it would make.
    grab: Option<handles::Grab>,
    /// What the recipe would build with that edit: shown while the drag
    /// lasts. Nothing is written until the handle is let go.
    preview: Option<Model>,
    /// Parts, or the faces of the selected box (Tab).
    mode: Mode,
    /// The tool in hand in Face mode.
    pub face_tool: FaceTool,
    /// The faces selected in Face mode, by their own names.
    faces: Vec<String>,
    /// In Pose mode: the pose the model stands in (`rest`, or one of the
    /// recipe's), what a drag turns the parts in.
    pose: String,
    /// The model as that pose puts it.
    posed: Option<Model>,
    /// What the pose gets wrong that the model at rest doesn't: for the
    /// problems panel.
    pose_problems: Vec<Problem>,
    /// What a drag does in Pose mode.
    pub pose_tool: PoseTool,
    /// A ring or arrow in hand in Pose mode.
    pose_grab: Option<pose::PoseGrab>,
    /// The animation chosen on the timeline, or with the view's Play
    /// button.
    animation: Option<String>,
    /// The time shown on the timeline, in seconds from the animation's
    /// start. `None`: the pose chosen is shown.
    at_time: Option<f64>,
    /// True while the animation chosen plays.
    playing: bool,
    /// Whether the poses of the keys before and after are drawn faintly.
    ghosts: bool,
}

impl Studio {
    /// Opens a file: a recipe to edit, or a model file to look at. With
    /// `look_only`, a recipe is only looked at too (`duck-mold preview`).
    pub fn open(path: impl Into<PathBuf>, look_only: bool) -> Studio {
        let path = path.into();
        let is_recipe = Syntax::from_path(&path).is_some();
        let mut studio = Studio {
            shown_as: display_path(&path),
            path,
            look_only: look_only || !is_recipe,
            text: String::new(),
            watched: Vec::new(),
            shown: None,
            problems: Vec::new(),
            orbit: Orbit::framing(None),
            grid: true,
            wire: false,
            shadow: true,
            hits: false,
            far: 0,
            far_shown: std::cell::RefCell::new(None),
            selected: None,
            also: Vec::new(),
            hidden: Vec::new(),
            status: String::new(),
            since_check: 0.0,
            builds: 0,
            pointer: None,
            view: (1, 1),
            dragging: None,
            undo: Vec::new(),
            redo: Vec::new(),
            history_file: None,
            kept: Default::default(),
            written: None,
            exported: Vec::new(),
            export_line: None,
            gone: false,
            settling: None,
            settling_files: None,
            library: library::Library::default(),
            shelf: shelf::Shelf::default(),
            paint: None,
            paint_image: None,
            tool: handles::Tool::Move,
            grab: None,
            preview: None,
            mode: Mode::Part,
            face_tool: FaceTool::Pull,
            faces: Vec::new(),
            pose: duck_mold_core::recipe::REST.to_owned(),
            posed: None,
            pose_problems: Vec::new(),
            pose_tool: PoseTool::Turn,
            pose_grab: None,
            animation: None,
            at_time: None,
            playing: false,
            ghosts: true,
        };
        studio.reload();
        studio.read_library();
        studio.read_shelf();
        studio.frame();
        studio
    }

    /// The file that is open.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's name, for the window's title.
    pub fn title(&self) -> String {
        self.path.file_name().map_or_else(
            || self.shown_as.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Works `edits` out on the recipe's text, in memory: for a drag's
    /// preview, or to see whether an edit would work. The model shown is
    /// handed on when the text has no mistakes, so it isn't built again.
    pub(crate) fn try_edits(
        &self,
        syntax: Syntax,
        edits: &[Edit],
    ) -> Result<edit::Applied, Failure> {
        let built = self.model().filter(|_| !self.has_mistakes());
        edit::apply_many_built(&self.text, syntax, &self.path, &self.shown_as, edits, built)
    }

    /// True for a recipe (`.toml`, `.json`); false for a mesh file
    /// (`.glb`, `.gltf`, `.obj`), which is only looked at.
    pub fn is_recipe(&self) -> bool {
        Syntax::from_path(&self.path).is_some()
    }

    /// True when nothing can be edited.
    pub fn look_only(&self) -> bool {
        self.look_only
    }

    /// The file's text as it was last read.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The model being shown: the last version of the file that built.
    pub fn model(&self) -> Option<&Model> {
        self.shown.as_ref().map(|shown| &shown.model)
    }

    /// `inspect`'s report on that model.
    pub fn report(&self) -> Option<&Inspection> {
        self.shown.as_ref().map(|shown| &shown.report)
    }

    /// The model's name, size and triangle count, in one line.
    pub fn headline(&self) -> Option<String> {
        self.report().map(Inspection::headline)
    }

    /// The mistakes and warnings of the file as it is now, and in Pose
    /// mode what the pose gets wrong that the model at rest doesn't.
    pub fn problems(&self) -> Vec<Problem> {
        self.problems
            .iter()
            .chain(&self.pose_problems)
            .cloned()
            .collect()
    }

    /// True if the file as it is now has mistakes (and what is shown is
    /// an older version of it).
    pub fn has_mistakes(&self) -> bool {
        self.problems.iter().any(|problem| problem.is_error)
    }

    /// What the Studio last did, in a few words, for the status line.
    pub fn status(&self) -> &str {
        &self.status
    }

    pub(super) fn say(&mut self, status: impl Into<String>) {
        self.status = self.shorten(&status.into());
    }

    /// Says why something the window was asked for wasn't done, in the
    /// status line.
    pub fn say_why(&mut self, why: &str) {
        self.say(why.to_owned());
    }

    /// The same words with files named from the recipe's folder:
    /// `parts/lamp.gltf`, not the whole way to it from the top of the
    /// disk. That is how the recipe itself names them.
    pub fn shorten(&self, words: &str) -> String {
        match self.path.parent() {
            Some(folder) if !folder.as_os_str().is_empty() => {
                words.replace(&format!("{}/", display_path(folder)), "")
            }
            _ => words.to_owned(),
        }
    }

    /// How many times the file has been built.
    pub fn builds(&self) -> usize {
        self.builds
    }

    /// The files watched for changes.
    pub fn watched(&self) -> Vec<&Path> {
        self.watched
            .iter()
            .map(|file| file.path.as_path())
            .collect()
    }

    /// Reads the file and builds it again. If it has mistakes, they
    /// become the problems and the last good model stays.
    pub fn reload(&mut self) {
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let (built, files) = tools::build_watching(&self.path);
        self.take_build(text, built, files);
    }

    /// Shows what was built from `text`.
    fn take_build(
        &mut self,
        text: String,
        built: Result<tools::Built, Failure>,
        files: Vec<PathBuf>,
    ) {
        self.builds += 1;
        self.text = text;
        self.watched = files
            .into_iter()
            .map(|path| Watched {
                stamp: stamp(&path),
                path,
            })
            .collect();
        match built {
            Ok(built) => {
                self.export_line.clone_from(&built.export);
                let mut report = inspect(&built.model, &built.source, built.warnings);
                report.materials = built.materials;
                self.problems = report
                    .warnings
                    .iter()
                    .map(|warning| problem(warning, &built.model))
                    .collect();
                self.shown = Some(Shown {
                    model: built.model,
                    report,
                });
            }
            Err(failure) => {
                let report = failure.report();
                self.problems = report
                    .errors
                    .iter()
                    .chain(&report.warnings)
                    .map(|issue| match self.model() {
                        Some(model) => problem(issue, model),
                        None => Problem {
                            text: line_and_message(issue),
                            is_error: issue.is_error(),
                            part: None,
                        },
                    })
                    .collect();
            }
        }
        let problems = std::mem::take(&mut self.problems);
        self.problems = problems
            .into_iter()
            .map(|problem| Problem {
                text: self.shorten(&problem.text),
                ..problem
            })
            .collect();
        // A selected part that is gone is no longer selected, nor a
        // hidden one hidden.
        let also = std::mem::take(&mut self.also);
        self.also = also
            .into_iter()
            .filter(|path| self.part(path).is_some())
            .collect();
        if let Some(selected) = &self.selected
            && self.part(selected).is_none()
        {
            self.selected = self.also.pop();
        }
        let hidden = std::mem::take(&mut self.hidden);
        self.hidden = hidden
            .into_iter()
            .filter(|path| self.part(path).is_some())
            .collect();
        self.keep_face_mode();
        // A pose that is gone (renamed or removed elsewhere) is no longer
        // the one shown.
        if !self.pose_names().contains(&self.pose) {
            self.pose = duck_mold_core::recipe::REST.to_owned();
        }
        self.keep_timeline();
        self.refresh_pose();
    }

    /// True if a watched file has been saved, made or removed since it
    /// was read.
    fn files_changed(&self) -> bool {
        self.watched
            .iter()
            .any(|file| stamp(&file.path) != file.stamp)
    }

    /// Lets `seconds` pass. Twice a second the files are checked, and the
    /// model is built again if one has changed. True if it was.
    pub fn tick(&mut self, seconds: f64) -> bool {
        // A playing animation moves on.
        self.advance(seconds);
        self.since_check += seconds;
        if self.since_check < CHECK_EVERY {
            return false;
        }
        self.since_check = 0.0;
        let library = self.check_library();
        let shelf = self.check_shelf();
        self.check_files() || library || shelf
    }

    /// Checks the files now. True if one had changed, and the model was
    /// built again.
    pub fn check_files(&mut self) -> bool {
        if !self.files_changed() {
            self.settling = None;
            self.settling_files = None;
            return false;
        }
        if !self.path.is_file() {
            // Deleted or moved: the model stays, and the window offers
            // to save it again.
            if self.gone {
                return false;
            }
            self.gone = true;
            if let Some(why) = self.missing() {
                self.say(why);
            }
            return true;
        }
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        // The other files as they are now (an image a picture program is
        // saving), to tell whether they have stopped changing.
        let stamps: Vec<Option<(SystemTime, u64)>> =
            self.watched.iter().map(|file| stamp(&file.path)).collect();
        let (built, files) = tools::build_watching(&self.path);
        if built.is_err() && text != self.text && self.settling.as_ref() != Some(&text) {
            // It may be half written: looked at again next check.
            self.settling = Some(text);
            return false;
        }
        if built.is_err() && text == self.text && self.settling_files.as_ref() != Some(&stamps) {
            // The recipe is as it was, so it is another file that
            // changed and doesn't build: an image half saved, perhaps.
            // Looked at again next check.
            self.settling_files = Some(stamps);
            return false;
        }
        self.settling = None;
        self.settling_files = None;
        self.gone = false;
        let before = std::mem::take(&mut self.text);
        self.take_build(text, built, files);
        self.changed_outside(before);
        true
    }

    /// Why nothing can be changed: the file isn't there any more.
    pub fn missing(&self) -> Option<String> {
        self.gone
            .then(|| format!("{} isn't there any more (deleted or moved)", self.title()))
    }

    /// Writes the recipe the Studio last saw back where it was, after it
    /// was deleted or moved.
    pub fn save_again(&mut self) -> Result<String, String> {
        let path = self.path.clone();
        self.save_as(&path)
    }

    /// Writes the recipe the Studio last saw to `path`, and goes on
    /// working there. The undo history comes along.
    pub fn save_as(&mut self, path: &Path) -> Result<String, String> {
        if self.look_only {
            let why = "This window is only for looking: nothing was changed".to_owned();
            self.say(why.clone());
            return Err(why);
        }
        if let Err(failure) = edit::swap_in(path, None, &self.text) {
            let why = capital(&first_error(&failure));
            self.say(why.clone());
            return Err(why);
        }
        if path != self.path {
            let folder = self
                .history_file
                .take()
                .and_then(|file| file.parent().map(Path::to_path_buf));
            self.path = path.to_path_buf();
            self.shown_as = display_path(path);
            if let Some(folder) = folder {
                self.history_file = Some(history::history_file(&folder, path));
            }
        }
        self.gone = false;
        self.written = Some(self.path.clone());
        self.reload();
        let said = format!("Saved {}", self.title());
        self.say(said.clone());
        Ok(said)
    }

    /// Waits a moment, at most a second, for a file another program is
    /// saving to settle. False if it hasn't.
    fn settle(&mut self) -> bool {
        for _ in 0..10 {
            self.check_files();
            if self.settling.is_none() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    /// A click with Hits on: the part whose hit shape is first on the ray
    /// from the eye through the pointer is selected, as a game's click
    /// would find it, and the status line says what it hit; a click a
    /// game's would miss selects nothing, and says why.
    fn click_hit_shape(&mut self, size: (usize, usize), at: (f64, f64)) -> Option<&str> {
        self.hover(size, Some(at));
        let (eye, way) = self.pointer_ray(at);
        let found = self
            .shown_model()
            .and_then(|model| crate::ray::first_hit(model, eye - self.offset(), way));
        self.also.clear();
        let said = match &found {
            Some(hit) if !hit.part.is_empty() => {
                self.selected = Some(hit.part.clone());
                format!(
                    "A game's click here hits: {} (its {}), {:.1} m away",
                    hit.part, hit.shape, hit.distance
                )
            }
            Some(hit) => {
                self.selected = None;
                format!(
                    "A game's click here hits the {} round the whole model, {:.1} m away",
                    hit.shape, hit.distance
                )
            }
            None => {
                self.selected = None;
                match self.part_under(size, at) {
                    Some(part) => {
                        format!("A game's click here hits nothing: {part} has no hit shape")
                    }
                    None => "A game's click here hits nothing".to_owned(),
                }
            }
        };
        self.say(said);
        self.selected.as_deref()
    }

    /// The part with this path.
    pub fn part(&self, path: &str) -> Option<&Part> {
        fn find<'a>(parts: &'a [Part], path: &str) -> Option<&'a Part> {
            parts.iter().find_map(|part| {
                if part.path == path {
                    Some(part)
                } else {
                    find(&part.parts, path)
                }
            })
        }
        find(&self.model()?.parts, path)
    }

    /// The parts as the outliner lists them: the tree, top to bottom.
    pub fn outline(&self) -> Vec<Row> {
        fn walk(parts: &[PartReport], depth: usize, rows: &mut Vec<Row>) {
            for part in parts {
                let kind = match (&part.uses, &part.file) {
                    (Some(uses), _) => format!("{uses} ({})", part.kind),
                    (None, Some(file)) => format!("{file} ({})", part.kind),
                    (None, None) => part.kind.clone(),
                };
                rows.push(Row {
                    name: part
                        .name
                        .rsplit('/')
                        .next()
                        .unwrap_or(&part.name)
                        .to_owned(),
                    path: part.name.clone(),
                    kind,
                    depth,
                    triangles: part.triangles,
                    hidden: false,
                });
                walk(&part.parts, depth + 1, rows);
            }
        }
        let mut rows = Vec::new();
        if let Some(report) = self.report() {
            walk(&report.parts, 0, &mut rows);
        }
        for row in &mut rows {
            row.kind = self.shorten(&row.kind);
            row.hidden = self.is_hidden(&row.path);
        }
        rows
    }

    /// The path of the part that is selected, if one is.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Selects the part with this path, or nothing. True if that is now
    /// what is selected; false if there is no such part (and the
    /// selection is as it was).
    pub fn select(&mut self, path: Option<&str>) -> bool {
        let before = self.selected.clone();
        let done = match path {
            None => {
                self.selected = None;
                self.also.clear();
                true
            }
            Some(path) if self.part(path).is_some() => {
                self.selected = Some(path.to_owned());
                self.also.clear();
                true
            }
            Some(_) => false,
        };
        // Another part in Face mode: its faces, if it can be shaped.
        if self.is_shaping() && self.selected != before {
            self.faces.clear();
            if self.selected.is_none() {
                self.mode = Mode::Part;
            } else {
                self.keep_face_mode();
            }
        }
        done
    }

    /// A click in the viewport, at (`x`, `y`) pixels from the top left of
    /// a picture `width` x `height`: selects the part under the pointer,
    /// or nothing if the pointer is on none.
    ///
    /// A part made of parts is selected whole first. Clicking again, on
    /// one of the parts inside what is selected, selects that one: so
    /// the first click on a robot's hand selects its arm, and the second
    /// the hand.
    pub fn click(&mut self, size: (usize, usize), at: (f64, f64)) -> Option<&str> {
        // With Hits on, what a game's click here would hit: the first
        // hit shape on the ray, as the model stands.
        if self.hits {
            return self.click_hit_shape(size, at);
        }
        // (Picked from what is shown: in Pose mode, the model in its
        // pose.)
        let hit = self.shown_model().and_then(|model| {
            viewport::pick(model, &self.orbit, self.offset(), size, at, &self.unseen())
        });
        self.also.clear();
        let Some(hit) = hit else {
            self.selected = None;
            return None;
        };
        // The parts the face is in, from the outermost in.
        fn around<'a>(parts: &'a [Part], face: usize, chain: &mut Vec<&'a Part>) {
            if let Some(part) = parts.iter().find(|part| part.faces.contains(&face)) {
                chain.push(part);
                around(&part.parts, face, chain);
            }
        }
        let mut chain = Vec::new();
        if let Some(model) = self.model() {
            around(&model.parts, hit.face, &mut chain);
        }
        let already = chain
            .iter()
            .position(|part| Some(part.path.as_str()) == self.selected.as_deref());
        let chosen = match already {
            // One further in, if there is one; the innermost stays.
            Some(at) => chain.get(at + 1).or(chain.get(at)),
            None => chain.first(),
        };
        self.selected = chosen.map(|part| part.path.clone());
        self.selected.as_deref()
    }

    /// What the recipe says about the selected part, for the properties
    /// panel: its name, what it is made of, its sizes, colour, join and
    /// twist, and which other parts an edit to it would change. `None`
    /// with nothing selected, or for a part that can't be edited here (a
    /// part inside another file, or any part of a model file).
    pub fn facts(&self) -> Option<Facts> {
        let syntax = Syntax::from_path(&self.path)?;
        edit::describe(&self.text, syntax, self.model(), self.selected.as_deref()?)
    }

    /// The names of the recipe's palette colours, in the recipe's order,
    /// each with its colour as `[red, green, blue]`.
    pub fn palette(&self) -> Vec<(String, [u8; 3])> {
        self.model().map_or_else(Vec::new, |model| {
            model
                .palette
                .iter()
                .map(|color| (color.name.clone(), color.rgb))
                .collect()
        })
    }

    /// Makes one change to the recipe: the same edit, by the same
    /// function, as `duck-mold edit`. The file is written at once and
    /// built again. Says what was done, or why nothing was.
    ///
    /// The selection follows the part: a renamed part stays selected, and
    /// a part that was added or copied becomes the selection.
    pub fn apply(&mut self, change: &Edit) -> Result<String, String> {
        self.apply_all(std::slice::from_ref(change))
    }

    /// Makes several changes together: worked out one after the other,
    /// checked once, written once, and one step to undo. If one of them
    /// can't be made, none is.
    pub fn apply_all(&mut self, changes: &[Edit]) -> Result<String, String> {
        self.apply_as(changes, None)
    }

    /// [`Studio::apply_all`], said in the words given instead of the
    /// edits' own: for many edits that are one thing to the person
    /// ("Moved 3 parts").
    pub(crate) fn apply_as(
        &mut self,
        changes: &[Edit],
        say: Option<String>,
    ) -> Result<String, String> {
        if self.look_only {
            let why = "This window is only for looking: nothing was changed".to_owned();
            self.say(why.clone());
            return Err(why);
        }
        // What the file holds now is what is edited, not what was read a
        // moment ago: if another program has just saved it, that is seen
        // first, once it has finished saving.
        if !self.settle() {
            let why = format!("Waiting for {} to finish saving...", self.title());
            self.say(why.clone());
            return Err(why);
        }
        if let Some(why) = self.missing() {
            let why = format!("Not changed: {why}");
            self.say(why.clone());
            return Err(why);
        }
        let before = self.text.clone();
        let parts_before: Vec<String> = self.outline().into_iter().map(|row| row.path).collect();
        // A socket the Studio made, left with nothing on it by these
        // changes, goes in the same step.
        let mut all = changes.to_vec();
        all.extend(self.sockets_left_unused(changes));
        let changes = all.as_slice();
        // The model shown is handed on (when the file has no mistakes),
        // so it isn't built again; the report is worked out by reload.
        let built = self
            .model()
            .filter(|_| !self.has_mistakes())
            .map(|model| (self.text.as_str(), model));
        match edit::edit_file_built(&self.path, changes, false, built, false) {
            Ok(edited) => {
                // The selection follows the last change that moves it.
                let mut follow = changes
                    .iter()
                    .rev()
                    .find_map(|change| self.selection_after(change));
                let selected_before = self.selection_owned();
                self.reload();
                let mut copies = Vec::new();
                if changes
                    .iter()
                    .any(|change| matches!(change, Edit::Duplicate { .. }))
                {
                    // The copies are the parts that weren't there before
                    // (and not the parts inside them).
                    let new: Vec<String> = self
                        .outline()
                        .into_iter()
                        .map(|row| row.path)
                        .filter(|path| !parts_before.contains(path))
                        .collect();
                    copies = new
                        .iter()
                        .filter(|path| {
                            path.rsplit_once('/')
                                .is_none_or(|(around, _)| !new.iter().any(|other| other == around))
                        })
                        .cloned()
                        .collect();
                    follow = copies.pop();
                }
                if let Some(path) = follow {
                    self.select(Some(&path));
                    // Several copies are all selected.
                    self.also = copies;
                } else if selected_before.len() > 1 {
                    // Several parts changed together stay selected.
                    let mut still: Vec<String> = selected_before
                        .into_iter()
                        .filter(|path| self.part(path).is_some())
                        .collect();
                    self.selected = still.pop();
                    self.also = still;
                }
                let mut said = say.unwrap_or_else(|| edited.edit.summary.clone());
                if !edited.edit.also_changes.is_empty() {
                    said.push_str(&format!(
                        " (also changed: {})",
                        edited.edit.also_changes.join(", ")
                    ));
                }
                // Capital first, as the other things the Studio says.
                let said = capital(&said);
                if self.text != before {
                    self.edited(before, &said);
                }
                self.say(said.clone());
                Ok(said)
            }
            Err(failure) => {
                let why = format!("Not changed: {}", first_error(&failure));
                self.say(why.clone());
                Err(why)
            }
        }
    }

    /// The `remove-socket` edits for the sockets the Studio made
    /// (`spot`, `spot_2`...) that `changes` would leave with nothing on
    /// them: the socket a removed part sat on, or one a part was moved
    /// away from.
    fn sockets_left_unused(&self, changes: &[Edit]) -> Vec<Edit> {
        let Some(syntax) = Syntax::from_path(&self.path) else {
            return Vec::new();
        };
        // The socket each part these changes take away is on now.
        let mut candidates: Vec<String> = Vec::new();
        for change in changes {
            let part = match change {
                Edit::Remove { part, .. } | Edit::Join { part, .. } | Edit::Unjoin { part } => part,
                _ => continue,
            };
            let mut leaving = vec![part.clone()];
            if matches!(
                change,
                Edit::Remove {
                    with_attached: true,
                    ..
                }
            ) {
                leaving.extend(self.hanging_from(part));
            }
            for path in leaving {
                let Some(to) = self.part(&path).and_then(|part| part.attached_to.clone()) else {
                    continue;
                };
                let Some((target, socket)) = to.rsplit_once('.') else {
                    continue;
                };
                if !edit::is_studio_socket(socket) {
                    continue;
                }
                // The part it sits on is in the same list.
                let on = match path.rsplit_once('/') {
                    Some((around, _)) => format!("{around}/{target}.{socket}"),
                    None => format!("{target}.{socket}"),
                };
                if !candidates.contains(&on) {
                    candidates.push(on);
                }
            }
        }
        // Each is kept only if, after the changes, it can go: nothing
        // else is on it.
        let mut tidy: Vec<Edit> = Vec::new();
        for socket in candidates {
            let mut tried = changes.to_vec();
            tried.extend(tidy.iter().cloned());
            tried.push(Edit::RemoveSocket {
                socket: socket.clone(),
            });
            if self.try_edits(syntax, &tried).is_ok() {
                tidy.push(Edit::RemoveSocket { socket });
            }
        }
        tidy
    }

    /// How far the picture shows the model moved sideways. A model's zero
    /// point is the middle of its underside, so adding a part on one
    /// side moves every other part the other way. The picture takes that
    /// back: the grid's lines are the recipe's own measurements, and a
    /// part with `move = [1, 0, 2]` stands over line 1 and line 2. Up and
    /// down nothing is taken back: the ground is under the lowest part.
    pub fn offset(&self) -> Vec3 {
        let zero = self
            .preview
            .as_ref()
            .or(self.model())
            .map_or(Vec3::ZERO, |model| model.zero);
        duck_mold_core::math::vec3(-zero.x, 0.0, -zero.z)
    }

    /// Where a point of the model is in the viewport, in pixels of the
    /// picture the pointer was last measured in.
    pub fn on_screen(&self, point: Vec3) -> Option<(f64, f64)> {
        self.orbit
            .on_screen(point + self.offset(), self.view.0, self.view.1)
    }

    /// The far-away version the picture shows, if View > Far version
    /// asks for one and the model has it: not while a handle is held or
    /// in Pose mode, which show the model itself.
    pub fn far_drawn(&self) -> Option<Rc<Model>> {
        if self.far == 0 || self.preview.is_some() || self.posed_model().is_some() {
            return None;
        }
        let model = self.model()?;
        let version = model.far.get(self.far - 1)?;
        let mut kept = self.far_shown.borrow_mut();
        match kept.as_ref() {
            Some((builds, far, drawn)) if *builds == self.builds && *far == self.far => {
                Some(drawn.clone())
            }
            _ => {
                let drawn = Rc::new(version.drawn());
                *kept = Some((self.builds, self.far, drawn.clone()));
                Some(drawn)
            }
        }
    }

    /// The model as the viewport shows it: what a drag would build, while
    /// a handle is held, and otherwise what the file builds.
    pub fn shown_model(&self) -> Option<&Model> {
        self.preview
            .as_ref()
            .or(self.posed_model())
            .or(self.model())
    }

    /// The part to select once an edit is made, where that isn't the one
    /// selected already.
    fn selection_after(&self, change: &Edit) -> Option<String> {
        // A path next to another: `left_arm/hand` and `claw` give
        // `left_arm/claw`.
        let beside = |path: &str, name: &str| match path.rsplit_once('/') {
            Some((around, _)) => format!("{around}/{name}"),
            None => name.to_owned(),
        };
        match change {
            Edit::Rename { part, new_name } => {
                let selected = self.selected.as_deref()?;
                if selected == part {
                    Some(beside(part, new_name))
                } else {
                    // A part inside the renamed one keeps its place there.
                    let inside = selected.strip_prefix(part.as_str())?.strip_prefix('/')?;
                    Some(format!("{}/{inside}", beside(part, new_name)))
                }
            }
            Edit::Add { name, to, .. } => match to {
                Some(to) => {
                    let (target, _) = to.rsplit_once('.')?;
                    Some(beside(target, name))
                }
                // A free part is in the model's own list.
                None => Some(name.clone()),
            },
            _ => None,
        }
    }

    /// Turns the camera to look from one of the four named sides, with
    /// what is selected (or the whole model) in view.
    pub fn look_from(&mut self, from: viewport::Viewpoint) {
        self.orbit.look_from(from);
        self.frame();
        self.say(match from {
            viewport::Viewpoint::Front => "Looking at the front",
            viewport::Viewpoint::Side => "Looking at the right side",
            viewport::Viewpoint::Top => "Looking down from the top",
            viewport::Viewpoint::ThreeQuarter => "Looking from the front right",
        });
    }

    /// Brings the whole model back into view (or the selected part, if
    /// one is), without turning the camera.
    pub fn frame(&mut self) {
        let bounds = self
            .selection()
            .into_iter()
            .filter_map(|path| self.part(path)?.bounds)
            .reduce(|all, one| Bounds {
                min: all.min.min(one.min),
                max: all.max.max(one.max),
            })
            .or_else(|| self.model().and_then(Model::bounds));
        if let Some(bounds) = bounds {
            let offset = self.offset();
            self.orbit.frame(Bounds {
                min: bounds.min + offset,
                max: bounds.max + offset,
            });
        }
    }

    /// Draws the viewport, `width` x `height` pixels.
    pub fn picture(&self, width: usize, height: usize) -> Picture {
        let shaping = self.mode != Mode::Part;
        let posing = self.mode == Mode::Pose;
        // In Face mode there are no sockets to show: the faces are.
        let mut markers = if shaping { Vec::new() } else { self.markers() };
        // What the pointer is on is drawn bigger: a marker, or the point
        // of a surface where a socket would be made. (Only when the
        // picture is the size the pointer was measured in.)
        let aim = if self.view == (width, height) && !shaping {
            self.aim(&markers)
        } else {
            None
        };
        let hot = match aim {
            Some(sockets::Aim::Marker(at)) => Some(at),
            Some(sockets::Aim::Surface {
                part, at, facing, ..
            }) => {
                markers.push(viewport::Marker {
                    part,
                    socket: String::new(),
                    at,
                    facing,
                });
                Some(markers.len() - 1)
            }
            None => None,
        };
        let far = self.far_drawn();
        let model = match &far {
            Some(far) => Some(far.as_ref()),
            None => self.shown_model(),
        };
        // The selected parts as they are in what is shown.
        // (In Face mode the box isn't lit whole: its faces are.)
        let selected: Vec<&Part> = self
            .selection()
            .into_iter()
            .filter(|path| (!shaping || posing) && !self.is_hidden(path))
            .filter_map(|path| part_in(&model?.parts, path))
            .collect();
        let mut marks = if self.view == (width, height) {
            self.face_marks()
        } else {
            Vec::new()
        };
        marks.extend(self.ghost_marks());
        marks.extend(self.marks(width, height));
        let hidden = self.unseen_in(model);
        let lit = if self.view == (width, height) {
            let mut lit = self.paint_aim();
            lit.extend(self.face_lit());
            lit
        } else {
            Vec::new()
        };
        // The model as built changes only with the file: what is kept
        // for it (its shadow) stays until then, through a drag too (the
        // shadow catches up when the part is let go). A pose is worked
        // out afresh.
        let model_key = (self.preview.is_some() || self.posed_model().is_none())
            .then_some(self.builds as u64 * 4 + far.as_ref().map_or(0, |_| self.far as u64));
        viewport::draw_kept(
            &mut self.kept.borrow_mut(),
            model_key,
            &Look {
                model,
                orbit: &self.orbit,
                offset: self.offset(),
                selected: &selected,
                lit: &lit,
                hidden: &hidden,
                focus: self.face_focus(),
                markers: &markers,
                hot,
                marks: &marks,
                grid: self.grid,
                wire: self.wire,
                shadow: self.shadow,
                hits: self.hits,
            },
            width,
            height,
        )
    }

    /// Writes the model as a `.glb` next to the recipe, as `duck-mold
    /// export` does. Says where it went.
    pub fn export(&mut self) -> Result<String, String> {
        let exported = tools::export_file(&self.path, None);
        let done = match &exported {
            // Named as the recipe names it, when it does.
            Ok(exported) => Ok(format!(
                "Wrote {} ({})",
                match self.export_line() {
                    Some(line) => line.to_owned(),
                    None => self.shorten(&exported.output),
                },
                tools::size_in_words(exported.bytes)
            )),
            Err(failure) => Err(self.shorten(&first_error(failure))),
        };
        self.say(match &done {
            Ok(said) => said.clone(),
            Err(why) => format!("Not exported: {why}"),
        });
        if let Ok(exported) = exported {
            self.written = Some(exported.path.clone());
            self.exported.push(exported.path);
        }
        done
    }

    /// Exports as [`Studio::export`] does, after a change, without a
    /// word when it works: the status line keeps saying what the
    /// change was. If it can't be written, the status line says why.
    pub fn export_after_change(&mut self) -> Result<PathBuf, String> {
        match tools::export_file(&self.path, None) {
            Ok(exported) => {
                self.exported.push(exported.path.clone());
                Ok(exported.path)
            }
            Err(failure) => {
                let why = self.shorten(&first_error(&failure));
                self.say(format!("Not exported: {why}"));
                Err(why)
            }
        }
    }

    /// The `.glb` files written since this was last asked: what a
    /// program that shows the Studio inside its own window (Duck
    /// Forge's) passes on to a running game.
    pub fn take_exported(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.exported)
    }

    /// Where the recipe's `export` line says its `.glb` belongs, as
    /// the line has it (from the recipe's folder). `None` if it has no
    /// such line, or this isn't a recipe.
    pub fn export_line(&self) -> Option<&str> {
        self.export_line.as_deref()
    }

    /// Where Export writes: where the `export` line says, or else
    /// beside the recipe.
    pub fn export_path(&self) -> PathBuf {
        tools::export_path(&self.path, self.export_line())
    }

    /// Writes the four-view picture next to the recipe, as `duck-mold
    /// render` does. Says where it went.
    pub fn render_picture(&mut self) -> Result<String, String> {
        let done = render::render_file(&self.path, None, render::DEFAULT_SIZE, None, None, false)
            .map(|rendered| {
                self.shorten(&format!(
                    "Wrote {} ({} x {} pixels, {})",
                    rendered.output,
                    rendered.width,
                    rendered.height,
                    tools::size_in_words(rendered.bytes)
                ))
            })
            .map_err(|failure| self.shorten(&first_error(&failure)));
        self.say(match &done {
            Ok(said) => said.clone(),
            Err(why) => format!("No picture: {why}"),
        });
        if done.is_ok() {
            self.written = Some(self.path.with_extension("png"));
        }
        done
    }

    /// The file the Studio wrote last: a `.glb` exported, a picture
    /// rendered, or the recipe saved again.
    pub fn last_written(&self) -> Option<&Path> {
        self.written.as_deref()
    }
}

/// The part with this path among `parts` and the parts inside them.
pub(super) fn part_in<'a>(parts: &'a [Part], path: &str) -> Option<&'a Part> {
    parts.iter().find_map(|part| {
        if part.path == path {
            Some(part)
        } else {
            part_in(&part.parts, path)
        }
    })
}

/// The first mistake of a failure, as one line.
pub(crate) fn first_error(failure: &Failure) -> String {
    let report = failure.report();
    report
        .errors
        .first()
        .map_or_else(|| "it didn't work".to_owned(), line_and_message)
}

/// `line 70: the part "x" touches nothing`.
fn line_and_message(issue: &Issue) -> String {
    let mut text = match issue.line {
        Some(line) => format!("line {line}: {}", issue.message),
        None => issue.message.clone(),
    };
    if let Some(suggestion) = &issue.suggestion {
        text.push_str(&format!(" (did you mean {suggestion}?)"));
    }
    text
}

/// A mistake or warning as a line of the problems panel, with the part
/// it is about: the part written on the line it names.
fn problem(issue: &Issue, model: &Model) -> Problem {
    fn on_line<'a>(parts: &'a [Part], file: &str, line: usize) -> Option<&'a Part> {
        parts.iter().find_map(|part| {
            on_line(&part.parts, file, line)
                .or_else(|| (part.loc.pos.line == line && part.loc.file == file).then_some(part))
        })
    }
    Problem {
        text: line_and_message(issue),
        is_error: issue.is_error(),
        part: issue
            .line
            .and_then(|line| on_line(&model.parts, &issue.file, line))
            .map(|part| part.path.clone()),
    }
}

/// The same words, starting with a capital letter.
fn capital(words: &str) -> String {
    let mut letters = words.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}
