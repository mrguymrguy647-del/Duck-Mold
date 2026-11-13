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

pub mod handles;
pub mod history;
pub mod library;
mod sockets;
pub mod viewport;

pub use sockets::Clicked;

use std::path::{Path, PathBuf};
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
    /// The part that is selected, by path.
    pub(crate) selected: Option<String>,
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
    library: library::Library,
    /// Which tool is in hand: what the handles on the selected part are.
    pub tool: handles::Tool,
    /// A handle being dragged, with the edit it would make.
    grab: Option<handles::Grab>,
    /// What the recipe would build with that edit: shown while the drag
    /// lasts. Nothing is written until the handle is let go.
    preview: Option<Model>,
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
            selected: None,
            status: String::new(),
            since_check: 0.0,
            builds: 0,
            pointer: None,
            view: (1, 1),
            dragging: None,
            undo: Vec::new(),
            redo: Vec::new(),
            library: library::Library::default(),
            tool: handles::Tool::Move,
            grab: None,
            preview: None,
        };
        studio.reload();
        studio.read_library();
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

    /// The mistakes and warnings of the file as it is now.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
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

    /// The same words with files named from the recipe's folder:
    /// `parts/lamp.gltf`, not the whole way to it from the top of the
    /// disk. That is how the recipe itself names them.
    fn shorten(&self, words: &str) -> String {
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
        self.builds += 1;
        self.text = std::fs::read_to_string(&self.path).unwrap_or_default();
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
        // A selected part that is gone is no longer selected.
        if let Some(selected) = &self.selected
            && self.part(selected).is_none()
        {
            self.selected = None;
        }
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
        self.since_check += seconds;
        if self.since_check < CHECK_EVERY {
            return false;
        }
        self.since_check = 0.0;
        let library = self.check_library();
        self.check_files() || library
    }

    /// Checks the files now. True if one had changed, and the model was
    /// built again.
    pub fn check_files(&mut self) -> bool {
        if !self.files_changed() {
            return false;
        }
        let before = std::mem::take(&mut self.text);
        self.reload();
        self.changed_outside(before);
        true
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
        match path {
            None => {
                self.selected = None;
                true
            }
            Some(path) if self.part(path).is_some() => {
                self.selected = Some(path.to_owned());
                true
            }
            Some(_) => false,
        }
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
        let hit = self
            .model()
            .and_then(|model| viewport::pick(model, &self.orbit, self.offset(), size, at, None));
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
        if self.look_only {
            let why = "This window is only for looking: nothing was changed".to_owned();
            self.say(why.clone());
            return Err(why);
        }
        // What the file holds now is what is edited, not what was read a
        // moment ago: if another program has just saved it, that is seen
        // first.
        self.check_files();
        let before = self.text.clone();
        let parts_before: Vec<String> = self.outline().into_iter().map(|row| row.path).collect();
        match edit::edit_file_many(&self.path, changes, false) {
            Ok(edited) => {
                // The selection follows the last change that moves it.
                let mut follow = changes
                    .iter()
                    .rev()
                    .find_map(|change| self.selection_after(change));
                self.reload();
                if changes
                    .iter()
                    .any(|change| matches!(change, Edit::Duplicate { .. }))
                {
                    // The copy is the part that wasn't there before.
                    follow = self
                        .outline()
                        .into_iter()
                        .map(|row| row.path)
                        .find(|path| !parts_before.contains(path));
                }
                if let Some(path) = follow {
                    self.select(Some(&path));
                }
                let mut said = edited.edit.summary.clone();
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

    /// The model as the viewport shows it: what a drag would build, while
    /// a handle is held, and otherwise what the file builds.
    pub fn shown_model(&self) -> Option<&Model> {
        self.preview.as_ref().or(self.model())
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
            Edit::Add { name, to, .. } => {
                let (target, _) = to.rsplit_once('.')?;
                Some(beside(target, name))
            }
            _ => None,
        }
    }

    /// Brings the whole model back into view (or the selected part, if
    /// one is), without turning the camera.
    pub fn frame(&mut self) {
        let bounds = self
            .selected
            .as_deref()
            .and_then(|path| self.part(path))
            .and_then(|part| part.bounds)
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
        let mut markers = self.markers();
        // What the pointer is on is drawn bigger: a marker, or the point
        // of a surface where a socket would be made. (Only when the
        // picture is the size the pointer was measured in.)
        let aim = if self.view == (width, height) {
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
        let model = self.shown_model();
        // The selected part as it is in what is shown.
        let selected: Vec<&Part> = self
            .selected
            .as_deref()
            .and_then(|path| part_in(&model?.parts, path))
            .into_iter()
            .collect();
        let marks = self.marks(width, height);
        viewport::draw(
            &Look {
                model,
                orbit: &self.orbit,
                offset: self.offset(),
                selected: &selected,
                markers: &markers,
                hot,
                marks: &marks,
                grid: self.grid,
            },
            width,
            height,
        )
    }

    /// Writes the model as a `.glb` next to the recipe, as `duck-mold
    /// export` does. Says where it went.
    pub fn export(&mut self) -> Result<String, String> {
        let done = tools::export_file(&self.path, None)
            .map(|exported| {
                self.shorten(&format!(
                    "Wrote {} ({} bytes)",
                    exported.output, exported.bytes
                ))
            })
            .map_err(|failure| self.shorten(&first_error(&failure)));
        self.say(match &done {
            Ok(said) => said.clone(),
            Err(why) => format!("Not exported: {why}"),
        });
        done
    }

    /// Writes the four-view picture next to the recipe, as `duck-mold
    /// render` does. Says where it went.
    pub fn render_picture(&mut self) -> Result<String, String> {
        let done = render::render_file(&self.path, None, render::DEFAULT_SIZE)
            .map(|rendered| {
                self.shorten(&format!(
                    "Wrote {} ({} x {} pixels)",
                    rendered.output, rendered.width, rendered.height
                ))
            })
            .map_err(|failure| self.shorten(&first_error(&failure)));
        self.say(match &done {
            Ok(said) => said.clone(),
            Err(why) => format!("No picture: {why}"),
        });
        done
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
