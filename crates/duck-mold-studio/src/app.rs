//! The Studio's screen.
//!
//! [`StudioApp::ui`] draws the whole screen into the space it is given,
//! so it can be the whole window (as here) or one panel of another app.
//! It holds nothing about the model: that is all in
//! [`Studio`](duck_mold_view::studio::Studio), and this only shows it.

use std::path::{Path, PathBuf};

use duck_mold_core::tools;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::{FaceTool, Mode, Studio};
use eframe::egui::{self, Color32, Rect, RichText, Sense, pos2};

use crate::actions::{self, Action};
use crate::icons::{self, Icon};
use crate::recent::Recent;
use crate::{gallery, library, palette, properties, theme, tour, view};

/// The Studio's window: a file that is open, or the start page.
pub struct StudioApp {
    studio: Option<Studio>,
    /// What the start page says went wrong, if something did.
    notice: Option<String>,
    /// A crash last time, for the start page to tell about.
    crashed: Option<crate::crash::Crash>,
    /// Whether the list of problems is open above the status bar.
    problems_open: bool,
    /// Whether the file had mistakes last frame: the list opens by
    /// itself when a new one appears.
    had_mistakes: bool,
    /// A note over the picture, for what matters: a file written.
    note: Option<Note>,
    /// What the parts list is filtered by: only parts whose path has it.
    parts_filter: String,
    /// The interface size chosen, in percent: `None` is the size the
    /// system asks for.
    interface: Option<u32>,
    view: view::View,
    /// The window's title as it was last set.
    titled: String,
    /// The selection the parts list last showed, to scroll to a new one.
    shown_selected: Option<String>,
    /// What is being typed in the properties panel.
    draft: properties::Draft,
    /// What is being typed or picked in the palette panel.
    colours: palette::Draft,
    /// The library's small pictures.
    thumbs: library::Thumbs,
    /// The recipes opened lately, for the start page.
    recent: Recent,
    /// Which of Help's windows is open, if one is.
    showing: Option<Showing>,
    /// The start page's pictures.
    gallery: gallery::Gallery,
    /// Which of the first run's tips is showing, if one is.
    tour: Option<usize>,
    /// Where the Add button is on the screen, for the tip about it.
    add_button: Option<Rect>,
    /// Where a copy of an example goes: the person's own
    /// `Documents\Duck Mold` folder.
    models_folder: PathBuf,
    /// Set when Tab was pressed while nothing was being typed: taken
    /// before egui could move the focus with it.
    tab: bool,
    /// The part a definition is being made from, and the name being
    /// typed for it, while the window that asks is open.
    defining: Option<(String, String)>,
    /// The timeline, in Pose mode.
    timeline: crate::timeline::Timeline,
}

/// The folder a person's own models go in: `Duck Mold` in their
/// Documents folder, wherever the system says it is (on Windows, the
/// known folder, which may have been moved to OneDrive or another
/// drive).
fn own_models_folder() -> PathBuf {
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Documents")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Duck Mold")
}

/// One of Help's windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Keys,
    About,
}

impl Default for StudioApp {
    fn default() -> Self {
        Self::new()
    }
}

impl StudioApp {
    /// The start page: nothing is open yet.
    pub fn new() -> StudioApp {
        StudioApp {
            studio: None,
            notice: None,
            crashed: None,
            problems_open: false,
            had_mistakes: false,
            note: None,
            parts_filter: String::new(),
            interface: None,
            view: view::View::default(),
            titled: String::new(),
            shown_selected: None,
            draft: properties::Draft::default(),
            timeline: crate::timeline::Timeline::default(),
            colours: palette::Draft::default(),
            thumbs: library::Thumbs::default(),
            recent: Recent::nowhere(),
            showing: None,
            gallery: gallery::Gallery::default(),
            models_folder: own_models_folder(),
            tab: false,
            defining: None,
            tour: None,
            add_button: None,
        }
    }

    /// Remembers the files that are opened in `recent`, and shows them on
    /// the start page.
    pub fn remembering(mut self, recent: Recent) -> StudioApp {
        self.recent = recent;
        self.crashed = self.recent.folder().and_then(crate::crash::take_unseen);
        self.interface = self.recent.interface_size();
        if let Some(studio) = &mut self.studio {
            self.recent.opened(studio.path());
            studio.hide_again(self.recent.hidden_in(studio.path()));
        }
        self.keep_history();
        self
    }

    /// Keeps the open recipe's undo history on disk, in the folder the
    /// recent list is kept in.
    fn keep_history(&mut self) {
        if let (Some(studio), Some(folder)) = (&mut self.studio, self.recent.folder()) {
            studio.keep_history_in(folder.join("undo"));
        }
    }

    /// With a file open: a recipe to edit, or (with `look_only`, or for a
    /// model file) only to look at.
    pub fn with_file(path: impl Into<PathBuf>, look_only: bool) -> StudioApp {
        let mut app = StudioApp::new();
        app.open(path.into(), look_only);
        app
    }

    /// Opens a file, in place of whatever was open.
    pub fn open(&mut self, path: PathBuf, look_only: bool) {
        if path.is_file() {
            self.recent.opened(&path);
            let mut studio = Studio::open(path, look_only);
            studio.hide_again(self.recent.hidden_in(studio.path()));
            self.studio = Some(studio);
            self.keep_history();
            self.notice = None;
            // The first model a person opens to work on gets the tips.
            self.tour = (!look_only && !self.recent.tips_seen()).then_some(0);
        } else {
            self.notice = Some(format!("There is no file \"{}\".", path.display()));
        }
        self.view.reset();
        // The start page's pictures of the models opened lately are
        // drawn again next time: this one may be about to change.
        self.gallery.forget_recent();
    }

    /// Puts copies of the examples in `folder` instead of the person's
    /// own Documents folder (for the tests).
    pub fn keeping_models_in(mut self, folder: impl Into<PathBuf>) -> StudioApp {
        self.models_folder = folder.into();
        self
    }

    /// Where a copy of an example goes.
    pub fn models_folder(&self) -> &Path {
        &self.models_folder
    }

    /// Opens a copy of an example to play with: the copy goes in the
    /// person's own models folder, as `robot.toml`, or `robot_2.toml` if
    /// that is taken. The example itself, inside the program, can't be
    /// spoiled.
    pub fn open_example(&mut self, name: &str) {
        let folder = self.models_folder.clone();
        if let Err(error) = std::fs::create_dir_all(&folder) {
            self.notice = Some(format!(
                "Couldn't make the folder \"{}\" for your models: {error}",
                folder.display()
            ));
            return;
        }
        let path = (1..)
            .map(|count| {
                if count == 1 {
                    folder.join(format!("{name}.toml"))
                } else {
                    folder.join(format!("{name}_{count}.toml"))
                }
            })
            .find(|path| !path.exists())
            .expect("there is always a name that isn't taken");
        match tools::new_from_example(&path, name) {
            Ok(_) => self.open(path, false),
            Err(failure) => {
                self.notice = failure
                    .report()
                    .errors
                    .first()
                    .map(|issue| issue.message.clone());
            }
        }
    }

    /// Makes a new recipe (one box) at `path` and opens it.
    pub fn start_new(&mut self, path: &Path) {
        match tools::new_file(path) {
            Ok(_) => self.open(path.to_path_buf(), false),
            Err(failure) => {
                self.notice = failure
                    .report()
                    .errors
                    .first()
                    .map(|issue| issue.message.clone());
            }
        }
    }

    /// What is open, if a file is.
    pub fn studio(&self) -> Option<&Studio> {
        self.studio.as_ref()
    }

    /// The same, to drive it.
    pub fn studio_mut(&mut self) -> Option<&mut Studio> {
        self.studio.as_mut()
    }

    /// The timeline as it was last drawn: where its keys are, for tests.
    pub fn timeline(&self) -> &crate::timeline::Timeline {
        &self.timeline
    }

    /// Where on the screen the viewport was last drawn, and how many
    /// pixels its picture has. The tests use it to click on a part.
    pub fn viewport(&self) -> Option<(Rect, [usize; 2])> {
        self.view.place()
    }

    /// What the window's title should be.
    pub fn title(&self) -> String {
        match &self.studio {
            Some(studio) if studio.look_only() => format!(
                "{} (looking only) - Duck Mold Studio {}",
                studio.title(),
                env!("CARGO_PKG_VERSION")
            ),
            Some(studio) => format!(
                "{} - Duck Mold Studio {}",
                studio.title(),
                env!("CARGO_PKG_VERSION")
            ),
            None => format!("Duck Mold Studio {}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Draws the whole screen into `ui`.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Everything bigger or smaller, as chosen.
        let zoom = self.interface.map_or(1.0, |percent| percent as f32 / 100.0);
        if (ui.ctx().zoom_factor() - zoom).abs() > 0.001 {
            ui.ctx().set_zoom_factor(zoom);
        }
        // For a note, if the Studio crashes.
        match &self.studio {
            Some(studio) => crate::crash::remember(Some(studio.path()), studio.status()),
            None => crate::crash::remember(None, ""),
        }
        let Some(studio) = &mut self.studio else {
            self.start_page(ui);
            return;
        };

        // Tab shapes the selected box. Taken before anything is drawn, so
        // that egui doesn't also move the focus from widget to widget with
        // it (unless something is being typed: then it is the typing's).
        if !ui.ctx().egui_wants_keyboard_input()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
        {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
            self.tab = true;
        }

        // Twice a second, see if the file was saved by something else.
        let seconds = f64::from(ui.input(|input| input.stable_dt)).min(1.0);
        studio.tick(seconds);
        // Which parts are hidden is remembered for next time.
        self.recent.keep_hidden(studio.path(), studio.hidden());
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
        // A playing animation moves every frame.
        if studio.is_playing() {
            ui.ctx().request_repaint();
        }

        // What a menu or a key asked for, carried out at the end.
        let mut asked: Option<Action> = None;
        egui::Panel::top("duck_mold_bar").show(ui, |ui| {
            ui.add_space(3.0);
            // One row on a wide window; it wraps to two on a narrow one.
            ui.horizontal_wrapped(|ui| {
                if let Some(duck) = self.gallery.example(ui.ctx(), "duck") {
                    ui.add(
                        egui::Image::new(duck)
                            .fit_to_exact_size(egui::vec2(22.0, 22.0))
                            .corner_radius(4.0),
                    );
                }
                ui.label(RichText::new(studio.title()).strong());
                if studio.look_only() {
                    ui.label(RichText::new("looking only").weak())
                        .on_hover_text("Nothing can be changed here. Open the recipe in the Studio to edit it.");
                }
                ui.separator();
                let recent: Vec<PathBuf> = self
                    .recent
                    .files()
                    .into_iter()
                    .map(Path::to_path_buf)
                    .collect();
                if let Some(action) = actions::menus(ui, studio, &recent, self.interface) {
                    asked = Some(action);
                }
                if !studio.look_only() {
                    ui.separator();
                    // What each would do is in its tip, and in the
                    // status line once it is done.
                    let back = studio.can_undo().map(str::to_owned);
                    let stops = studio.undo_stops();
                    let undo = ui
                        .add_enabled_ui(back.is_some(), |ui| {
                            icons::button(ui, Icon::Undo, "Undo", false)
                        })
                        .inner
                        .on_hover_text(match &back {
                            Some(title) => format!("Undo: {title} (Ctrl+Z)"),
                            None => "Nothing to undo".to_owned(),
                        })
                        .on_disabled_hover_text(match stops {
                            Some(why) => format!("{why}."),
                            None => "Nothing to undo".to_owned(),
                        });
                    if undo.clicked() {
                        asked = Some(Action::Undo);
                    }
                    let again = studio.can_redo().map(str::to_owned);
                    let redo = ui
                        .add_enabled_ui(again.is_some(), |ui| {
                            icons::button(ui, Icon::Redo, "Redo", false)
                        })
                        .inner
                        .on_hover_text(match &again {
                            Some(title) => format!("Redo: {title} (Ctrl+Y)"),
                            None => "Nothing to redo".to_owned(),
                        });
                    if redo.clicked() {
                        asked = Some(Action::Redo);
                    }
                    ui.separator();
                    // A new part that is joined to nothing: it stands on
                    // the ground where the view looks, ready to be moved.
                    let thumbs = &mut self.thumbs;
                    let add = icons::labelled(ui, Icon::Add, "Add", false);
                    egui::Popup::menu(&add).show(|ui| {
                        ui.label(RichText::new("A new part, on the ground").weak());
                        if let Some(made) = view::choices(ui, studio, thumbs) {
                            asked = Some(Action::AddFree(made));
                            ui.close();
                        }
                    });
                    self.add_button = Some(add.rect);
                    add.on_hover_text(
                        "Add a part that stands on the ground, joined to nothing (Shift+A). To add one joined to another part, click a dot on that part.",
                    );
                }
                // At the right: the version, and what to do with the model.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).weak())
                        .on_hover_text("The version of Duck Mold Studio");
                    // A mesh file is only looked at: nothing to export.
                    if studio.is_recipe() {
                        ui.separator();
                        if icons::button(ui, Icon::Picture, "Render picture", false)
                            .on_hover_text("Render picture: write a picture of the model (four views) next to the recipe")
                            .clicked()
                        {
                            asked = Some(Action::RenderPicture);
                        }
                        let export = ui
                            .add(theme::primary_button("Export .glb"))
                            .on_hover_text("Write the model as a .glb file next to the recipe, for a game (Ctrl+E)");
                        export.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, "Export")
                        });
                        if export.clicked() {
                            asked = Some(Action::Export);
                        }
                    }
                });
            });
            ui.add_space(3.0);
        });

        // One line at the bottom: the problems, and what was done last.
        // A new mistake opens the list of problems by itself.
        let mistakes = studio.has_mistakes();
        if mistakes && !self.had_mistakes {
            self.problems_open = true;
        }
        self.had_mistakes = mistakes;
        egui::Panel::bottom("duck_mold_status")
            .exact_size(STATUS_HEIGHT)
            .show(ui, |ui| status_bar(ui, studio, &mut self.problems_open));
        if self.problems_open {
            egui::Panel::bottom("duck_mold_problems")
                .resizable(true)
                .default_size(120.0)
                .min_size(60.0)
                .show(ui, |ui| problems(ui, studio));
        }

        // The timeline, in Pose mode, above the problems.
        if studio.mode() == Mode::Pose {
            egui::Panel::bottom("duck_mold_timeline")
                .resizable(false)
                .show(ui, |ui| {
                    crate::timeline::show(ui, studio, &mut self.timeline)
                });
        }

        egui::Panel::left("duck_mold_parts")
            .resizable(true)
            .default_size(230.0)
            .min_size(140.0)
            .show(ui, |ui| {
                // The library sits under the parts, in the same column.
                if !studio.look_only() {
                    // As tall as it needs for its pictures and the
                    // animations list; it can be dragged taller.
                    let third = (ui.available_height() / 3.0).clamp(150.0, 320.0);
                    egui::Panel::bottom("duck_mold_library")
                        .resizable(true)
                        .default_size(third)
                        .min_size(120.0)
                        .show(ui, |ui| library::panel(ui, studio, &mut self.thumbs));
                }
                if let Some(action) = outliner(
                    ui,
                    studio,
                    &mut self.shown_selected,
                    &mut self.thumbs,
                    &mut self.parts_filter,
                ) {
                    // From the list there is no point of a surface.
                    self.view.clear_menu_spot();
                    asked = Some(action);
                }
            });

        // The right column: one column that scrolls, so nothing in it is
        // ever cut off. The model or the selected part, then the palette
        // (not in Pose mode, where it does nothing).
        egui::Panel::right("duck_mold_properties")
            .resizable(true)
            .default_size(280.0)
            .min_size(220.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        properties::show(ui, studio, &mut self.draft);
                        if studio.mode() != Mode::Pose {
                            ui.add_space(12.0);
                            palette::panel(ui, studio, &mut self.colours);
                        }
                    });
            });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            // A recipe deleted or moved while open: the model stays, and
            // the bar offers to keep it.
            if let Some(why) = studio.missing()
                && let Some(action) = missing_bar(ui, &why, studio.look_only())
            {
                asked = Some(action);
            }
            if let Some(action) = view::show(ui, studio, &mut self.view, &mut self.thumbs) {
                asked = Some(action);
            }
        });
        // A note over the top right of the picture, for a few seconds.
        let now = ui.input(|input| input.time);
        if let Some(note) = &self.note {
            if now - note.since > NOTE_SECONDS {
                self.note = None;
            } else if let Some((rect, _)) = self.view.place() {
                if show_note(ui.ctx(), note, rect) {
                    self.note = None;
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
        }

        // Ctrl+Z undoes and Ctrl+Y (or Ctrl+Shift+Z) redoes, unless a
        // text field is being typed in: there they belong to the typing.
        if !studio.look_only() && !ui.ctx().egui_wants_keyboard_input() {
            let (undo, redo) = ui.input_mut(|input| {
                let redo = input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                    || input.consume_key(
                        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                        egui::Key::Z,
                    );
                let undo = !redo && input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z);
                (undo, redo)
            });
            if undo {
                asked = Some(Action::Undo);
            }
            if redo {
                asked = Some(Action::Redo);
            }
            // The tools' keys, as in other 3D programs. Shaping has
            // tools of its own.
            let key = |key: egui::Key| {
                ui.input(|input| input.modifiers.is_none() && input.key_pressed(key))
            };
            if studio.mode() == Mode::Pose {
                for (pressed, tool) in [
                    (egui::Key::R, duck_mold_view::studio::PoseTool::Turn),
                    (egui::Key::G, duck_mold_view::studio::PoseTool::Move),
                ] {
                    if key(pressed) {
                        studio.pose_tool = tool;
                    }
                }
            } else if studio.mode() == Mode::Face {
                for (pressed, tool) in [
                    (egui::Key::T, FaceTool::Pull),
                    (egui::Key::G, FaceTool::Move),
                    (egui::Key::R, FaceTool::Turn),
                    (egui::Key::S, FaceTool::Scale),
                    (egui::Key::I, FaceTool::Inset),
                    (egui::Key::K, FaceTool::Cut),
                ] {
                    if key(pressed) {
                        studio.face_tool = tool;
                    }
                }
            } else if studio.mode() == Mode::Corner {
                if key(egui::Key::G) {
                    studio.face_tool = FaceTool::Move;
                }
            } else if studio.mode() == Mode::Edge {
                for (pressed, tool) in [
                    (egui::Key::G, FaceTool::Move),
                    (egui::Key::K, FaceTool::Cut),
                ] {
                    if key(pressed) {
                        studio.face_tool = tool;
                    }
                }
            } else {
                for (pressed, tool) in [
                    (egui::Key::G, Tool::Move),
                    (egui::Key::R, Tool::Rotate),
                    (egui::Key::S, Tool::Scale),
                    (egui::Key::T, Tool::Size),
                    (egui::Key::P, Tool::Paint),
                ] {
                    if key(pressed) {
                        studio.tool = tool;
                    }
                }
            }
            // Tab: shape the selected box, or back to parts.
            if std::mem::take(&mut self.tab) {
                asked = Some(Action::Shape);
            }
            if key(egui::Key::E) {
                asked = Some(Action::EdgeMode);
            }
            if key(egui::Key::C) {
                asked = Some(Action::CornerMode);
            }
            if key(egui::Key::O) {
                asked = Some(Action::PoseMode);
            }
            if key(egui::Key::Space) {
                asked = Some(Action::Play);
            }
            if let Some(action) = actions::keys(ui) {
                asked = Some(action);
            }
        }
        // Looking only, the keys that change nothing still work.
        if studio.look_only() && !ui.ctx().egui_wants_keyboard_input() {
            asked = asked.or(actions::keys(ui).filter(|action| {
                matches!(
                    action,
                    Action::Frame
                        | Action::Hide
                        | Action::ShowAll
                        | Action::SelectAll
                        | Action::LookFrom(_)
                        | Action::Wireframe
                        | Action::Open
                        | Action::NewModel
                        | Action::Export
                )
            }));
        }
        // What has to wait until the file is let go of.
        let mut open_another = false;
        let mut new_model = false;
        let mut to_start = false;
        let mut save_as = false;
        let mut open_file: Option<PathBuf> = None;
        let mut open_example: Option<&'static str> = None;
        if let Some(action) = asked {
            // Only what changes nothing is done when looking only.
            let allowed = !studio.look_only()
                || matches!(
                    action,
                    Action::Frame
                        | Action::Play
                        | Action::Hide
                        | Action::ShowAll
                        | Action::SelectAll
                        | Action::LookFrom(_)
                        | Action::Grid
                        | Action::Wireframe
                        | Action::Shadow
                        | Action::HitShapes
                        | Action::FarVersion(_)
                        | Action::Open
                        | Action::NewModel
                        | Action::Export
                        | Action::RenderPicture
                        | Action::StartPage
                        | Action::InterfaceSize(_)
                        | Action::ShowKeys
                        | Action::ShowTips
                        | Action::About
                        | Action::OpenFile(_)
                        | Action::OpenExample(_)
                );
            match action {
                _ if !allowed => {}
                Action::Undo => {
                    let _ = studio.undo();
                }
                Action::Redo => {
                    let _ = studio.redo();
                }
                Action::LookFrom(from) => studio.look_from(from),
                Action::Grid => studio.grid = !studio.grid,
                Action::Wireframe => studio.wire = !studio.wire,
                Action::Shadow => studio.shadow = !studio.shadow,
                Action::HitShapes => studio.hits = !studio.hits,
                Action::FarVersion(far) => studio.far = far,
                Action::NewModel => new_model = true,
                Action::Open => open_another = true,
                Action::Export => {
                    note_of(&mut self.note, studio.export(), studio, now);
                }
                Action::RenderPicture => {
                    note_of(&mut self.note, studio.render_picture(), studio, now);
                }
                Action::StartPage => to_start = true,
                Action::InterfaceSize(size) => {
                    self.interface = size;
                    self.recent.keep_interface_size(size);
                }
                Action::SaveAgain => {
                    note_of(&mut self.note, studio.save_again(), studio, now);
                }
                Action::SaveAs => save_as = true,
                Action::OpenFile(file) => open_file = Some(file),
                Action::OpenExample(name) => open_example = Some(name),
                Action::ShowTips => self.tour = Some(0),
                Action::ShowKeys => self.showing = Some(Showing::Keys),
                Action::About => self.showing = Some(Showing::About),
                Action::Duplicate => {
                    let _ = studio.duplicate_selected(false);
                }
                Action::MirrorCopy => {
                    let _ = studio.duplicate_selected(true);
                }
                Action::Rename => {
                    if studio.selection().len() == 1 {
                        self.draft.rename();
                    }
                }
                Action::Remove if studio.mode() == Mode::Pose => {
                    studio.say_why(
                        "In Pose mode a part isn't removed: \"Back to rest\" takes it out of the pose. To remove the part, go back to parts (O)",
                    );
                }
                Action::Remove if studio.mode() != Mode::Part => {
                    studio.say_why(
                        "Faces can't be removed: a shape must stay closed. To take a change back, Undo, or remove it from the Changes list",
                    );
                }
                Action::Remove => {
                    if !studio.selection().is_empty() {
                        self.draft.remove(studio);
                    }
                }
                Action::Shape => {
                    // What it did, or why not, is in the status line.
                    let _ = studio.toggle_face_mode();
                }
                Action::FaceTool(tool) => studio.face_tool = tool,
                Action::EdgeMode => {
                    // What it did, or why not, is in the status line.
                    let _ = studio.toggle_edge_mode();
                }
                Action::CornerMode => {
                    let _ = studio.toggle_corner_mode();
                }
                Action::PoseMode => {
                    // What it did, or why not, is in the status line.
                    let _ = studio.toggle_pose_mode();
                }
                Action::PoseTool(tool) => studio.pose_tool = tool,
                Action::Play => {
                    // In Part mode with none chosen yet: the first.
                    if studio.animation().is_none()
                        && let Some(first) = studio.animation_names().first().cloned()
                    {
                        studio.choose_animation(Some(&first));
                    }
                    studio.toggle_play();
                }
                Action::PoseBackToRest => {
                    if let Some(part) = studio.selected().map(str::to_owned) {
                        let _ = studio.pose_back_to_rest(&part);
                    }
                }
                Action::MakeDefinition => {
                    if let Some(part) = studio.selected().map(str::to_owned) {
                        let own = part.rsplit('/').next().unwrap_or(&part).to_owned();
                        let name = duck_mold_core::edit::unused_name(
                            own.trim_start_matches('#'),
                            &studio.definitions(),
                        );
                        self.defining = Some((part, name));
                    }
                }
                Action::SaveToLibrary => {
                    if let Some(part) = studio.selected().map(str::to_owned) {
                        // What it did, or why not, is in the status line.
                        let _ = studio.save_to_library(&part);
                    }
                }
                Action::Unjoin => {
                    let _ = studio.unjoin_selected();
                }
                Action::JoinTo(to) => {
                    if let Some(part) = studio.selected().map(str::to_owned) {
                        let _ = studio.apply(&duck_mold_core::edit::Edit::Join {
                            part,
                            to,
                            attach: None,
                        });
                    }
                }
                Action::AddHere(made) => {
                    // At the point the menu was opened at; from the
                    // list, on the part's top.
                    let _ = match self.view.take_menu_spot() {
                        Some((part, spot)) => studio.add_to_surface(&part, &spot, &made),
                        None => match studio.selected().map(str::to_owned) {
                            Some(part) => studio.add_to_socket(&part, "top", &made),
                            None => studio.add_free(&made),
                        },
                    };
                }
                Action::AddFree(made) => {
                    let _ = studio.add_free(&made);
                }
                Action::OpenAdd => self.view.add_on_ground(),
                Action::Frame => studio.frame(),
                Action::Hide => {
                    studio.hide_selected();
                }
                Action::ShowAll => {
                    studio.show_all();
                }
                Action::SelectAll => studio.select_all(),
                Action::TidySockets => {
                    let _ = studio.apply(&duck_mold_core::edit::Edit::TidySockets);
                }
            }
        }

        // Escape lets go of the selection. With nothing selected, it
        // closes a window that is only for looking, as `preview` always
        // did.
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.problems_open {
                self.problems_open = false;
            } else if self.view.is_adding() {
                self.view.stop_adding();
            } else if studio.dragging().is_some() || studio.grabbed().is_some() {
                // The viewport lets go of what it was dragging.
            } else if studio.escape() {
                // A step back: the faces, Face mode, then the selection.
            } else if studio.look_only() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        // The first run's tips, one at a time.
        if let Some(tip) = self.tour
            && let Some(viewport) = self.view.place()
        {
            tour::prepare(studio, tip);
            match tour::show(ui.ctx(), studio, tip, viewport, self.add_button) {
                tour::Answer::Reading => {}
                tour::Answer::Next if tip + 1 < tour::TIPS.len() => self.tour = Some(tip + 1),
                tour::Answer::Next | tour::Answer::Skip => {
                    self.tour = None;
                    self.recent.saw_tips();
                }
            }
        }
        self.help_window(ui);
        self.definition_window(ui);
        if open_another && let Some(path) = pick_file() {
            self.open(path, false);
        }
        if new_model && let Some(path) = pick_new_file() {
            self.start_new(&path);
        }
        if save_as
            && let Some(studio) = &mut self.studio
            && let Some(path) = pick_save_as(&studio.title())
        {
            note_of(&mut self.note, studio.save_as(&path), studio, now);
            self.recent.opened(&path);
        }
        if to_start {
            self.studio = None;
            self.view.reset();
        }
        if let Some(file) = open_file {
            self.open(file, false);
        }
        if let Some(name) = open_example {
            self.open_example(name);
        }
    }

    /// The list of keys, or what the program is: Help's two windows.
    /// The window that asks for a definition's name.
    fn definition_window(&mut self, ui: &mut egui::Ui) {
        let Some((part, name)) = &mut self.defining else {
            return;
        };
        let mut open = true;
        let mut make = false;
        let mut cancel = false;
        egui::Window::new("Make a definition")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label(format!(
                    "What \"{part}\" is made of becomes a definition the recipe can use again: Add > Defined in this recipe."
                ));
                ui.horizontal(|ui| {
                    let label = ui.label("Called");
                    let field = ui
                        .add(egui::TextEdit::singleline(name).desired_width(140.0))
                        .labelled_by(label.id);
                    if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        make = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!name.is_empty(), theme::primary_button("Make"))
                        .clicked()
                    {
                        make = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }
        if make && let Some(studio) = &mut self.studio {
            let (part, name) = (part.clone(), name.clone());
            // What it did, or why not, is in the status line.
            let _ = studio.apply(&duck_mold_core::edit::Edit::Define { part, name });
            open = false;
        }
        if !open {
            self.defining = None;
        }
    }

    fn help_window(&mut self, ui: &mut egui::Ui) {
        let Some(showing) = self.showing else {
            return;
        };
        let mut open = true;
        let title = match showing {
            Showing::Keys => "The keys",
            Showing::About => "About Duck Mold Studio",
        };
        // Never taller than the window: the keys scroll.
        let room = (ui.ctx().content_rect().height() - 120.0).max(160.0);
        let duck = self.gallery.example(ui.ctx(), "duck");
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| match showing {
                Showing::Keys => {
                    egui::ScrollArea::vertical()
                        .max_height(room)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            egui::Grid::new("duck_mold_keys")
                                .num_columns(2)
                                .spacing([16.0, 4.0])
                                .show(ui, |ui| {
                                    for (key, does) in actions::KEYS {
                                        ui.label(RichText::new(key).strong());
                                        ui.label(does);
                                        ui.end_row();
                                    }
                                });
                        });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Keys don't fire while you are typing in a field.").weak(),
                    );
                }
                Showing::About => {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        if let Some(duck) = duck {
                            ui.add(
                                egui::Image::new(duck)
                                    .fit_to_exact_size(egui::vec2(64.0, 64.0))
                                    .corner_radius(8.0),
                            );
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Duck Mold Studio").size(22.0).strong());
                            ui.horizontal(|ui| {
                                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                                if ui
                                    .small_button("Copy version")
                                    .on_hover_text("Copy it, to put in a message about a problem")
                                    .clicked()
                                {
                                    ui.ctx().copy_text(format!(
                                        "Duck Mold Studio {}",
                                        env!("CARGO_PKG_VERSION")
                                    ));
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                    ui.label("Make 3D models for Duck Forge games from parts that join at sockets.");
                    ui.add_space(4.0);
                    ui.label(
                        "A model is a recipe: a text file. The Studio changes that file every time you change something, so there is nothing to save, and an AI agent can read and change it too.",
                    );
                    ui.add_space(6.0);
                    ui.hyperlink_to("How to use it (the guide)", GUIDE);
                    ui.hyperlink_to("Duck Mold on GitHub", WEBSITE);
                }
            });
        if !open || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.showing = None;
        }
    }

    /// The page shown when no file is open: the name, the two ways to
    /// begin, the models opened lately and the examples, each with its
    /// picture, and how it works in three lines.
    fn start_page(&mut self, ui: &mut egui::Ui) {
        let mut open = false;
        let mut new = false;
        let mut again: Option<PathBuf> = None;
        let mut forget: Option<PathBuf> = None;
        let mut example: Option<&'static str> = None;
        let recent: Vec<PathBuf> = self
            .recent
            .all_files()
            .into_iter()
            .map(Path::to_path_buf)
            .collect();
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Up to 1400 points wide: on a big screen the examples
                    // spread out, and "How it works" sits beside them.
                    let wide = ui.available_width().min(START_PAGE_WIDTH);
                    let margin = ((ui.available_width() - wide) / 2.0).max(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(wide - 24.0);
                            ui.add_space(24.0);
                            // ---- the duck and the name
                            ui.horizontal(|ui| {
                                if let Some(duck) = self.gallery.example(ui.ctx(), "duck") {
                                    ui.add(
                                        egui::Image::new(duck)
                                            .fit_to_exact_size(egui::vec2(56.0, 56.0))
                                            .corner_radius(8.0),
                                    );
                                }
                                ui.vertical(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Duck Mold Studio").size(28.0).strong(),
                                    );
                                    ui.label("Build 3D models from parts, for Duck Forge games.");
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "version {}",
                                                env!("CARGO_PKG_VERSION")
                                            ))
                                            .weak(),
                                        );
                                    },
                                );
                            });
                            ui.add_space(16.0);
                            // ---- the two ways to begin
                            ui.horizontal(|ui| {
                                if ui
                                    .add(theme::primary_button("New model"))
                                    .on_hover_text("Starts a recipe with one box in it (Ctrl+N)")
                                    .clicked()
                                {
                                    new = true;
                                }
                                if ui
                                    .button("Open a model")
                                    .on_hover_text("Open a recipe or a model file (Ctrl+O)")
                                    .clicked()
                                {
                                    open = true;
                                }
                            });
                            if let Some(notice) = &self.notice {
                                ui.add_space(10.0);
                                ui.colored_label(theme::BAD, notice);
                            }
                            if let Some(crash) = self.crashed.clone() {
                                ui.add_space(10.0);
                                match crashed_notice(ui, &crash) {
                                    Some(CrashAnswer::Open(file)) => {
                                        self.crashed = None;
                                        again = Some(file);
                                    }
                                    Some(CrashAnswer::ShowNote) => {
                                        crate::system::open(&crash.note);
                                    }
                                    Some(CrashAnswer::Dismiss) => self.crashed = None,
                                    None => {}
                                }
                            }

                            // ---- the models opened lately
                            if !recent.is_empty() {
                                ui.add_space(20.0);
                                ui.label(RichText::new("Opened lately").strong());
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    for file in &recent {
                                        let name = file.file_name().map_or_else(String::new, |name| {
                                            name.to_string_lossy().into_owned()
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(108.0);
                                            if !file.is_file() {
                                                // Deleted or moved since: said, and
                                                // it can be taken off the list.
                                                ui.add_enabled_ui(false, |ui| {
                                                    tile(ui, None, &name, 96.0)
                                                })
                                                .response
                                                .on_disabled_hover_text(format!(
                                                    "{}\nisn't there any more",
                                                    file.display()
                                                ));
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new("not found").weak(),
                                                    );
                                                    if ui
                                                        .small_button("Forget")
                                                        .on_hover_text("Take it off this list")
                                                        .clicked()
                                                    {
                                                        forget = Some(file.clone());
                                                    }
                                                });
                                                return;
                                            }
                                            let picture =
                                                self.gallery.recent(ui.ctx(), file).cloned();
                                            if tile(ui, picture.as_ref(), &name, 96.0)
                                                .on_hover_text(file.display().to_string())
                                                .clicked()
                                            {
                                                again = Some(file.clone());
                                            }
                                            // A model that doesn't build says why.
                                            if let Some(mistake) =
                                                self.gallery.recent_mistake(file)
                                            {
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(mistake)
                                                            .color(theme::BAD)
                                                            .small(),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(mistake);
                                            }
                                        });
                                    }
                                });
                            }

                            // ---- the examples
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new("Examples: click one to open a copy of your own")
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "The copy goes in {}. Change it as you like: the example stays as it is.",
                                    self.models_folder.display()
                                ))
                                .weak()
                                .small(),
                            );
                            ui.add_space(4.0);
                            let beside = wide >= HOW_BESIDE_FROM;
                            let examples_width = if beside {
                                wide - 24.0 - HOW_WIDTH - 24.0
                            } else {
                                wide - 24.0
                            };
                            // As many across as fit, from three to six.
                            let gap = 8.0;
                            let across = (((examples_width + gap) / (CARD_SMALLEST + gap)).floor()
                                as usize)
                                .clamp(3, 6);
                            let card = (examples_width - gap * (across - 1) as f32) / across as f32;
                            let side = (card - 12.0).clamp(96.0, 220.0);
                            let how = |ui: &mut egui::Ui| {
                                ui.label(RichText::new("How it works").strong());
                                ui.add_space(2.0);
                                for line in [
                                    "1. A model is made of parts: boxes, balls, tubes, arches...",
                                    "2. Parts stand free, or join at dots, so they stay together.",
                                    "3. Everything you do is written to a recipe file as you go. Export turns it into the model file for your game.",
                                ] {
                                    ui.label(line);
                                }
                            };
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(examples_width);
                                    for row in tools::EXAMPLES.chunks(across) {
                                        ui.horizontal_top(|ui| {
                                            ui.spacing_mut().item_spacing.x = gap;
                                            for known in row {
                                                let picture = self
                                                    .gallery
                                                    .example(ui.ctx(), known.name)
                                                    .cloned();
                                                if example_card(ui, picture.as_ref(), known.name, known.about, side)
                                                    .on_hover_text(known.about)
                                                    .clicked()
                                                {
                                                    example = Some(known.name);
                                                }
                                            }
                                        });
                                        ui.add_space(gap);
                                    }
                                });
                                if beside {
                                    ui.add_space(24.0);
                                    ui.vertical(|ui| {
                                        ui.set_width(HOW_WIDTH);
                                        how(ui);
                                    });
                                }
                            });
                            if !beside {
                                // ---- how it works, under them
                                ui.add_space(12.0);
                                how(ui);
                            }
                            ui.add_space(24.0);
                        });
                    });
                });
        });
        // The same keys as with a model open.
        if !ui.ctx().egui_wants_keyboard_input() {
            ui.input_mut(|input| {
                new |= input.consume_key(egui::Modifiers::COMMAND, egui::Key::N);
                open |= input.consume_key(egui::Modifiers::COMMAND, egui::Key::O);
            });
        }
        if let Some(file) = again {
            self.open(file, false);
        }
        if let Some(file) = forget {
            self.recent.forget(&file);
        }
        if let Some(name) = example {
            self.open_example(name);
        }
        if open && let Some(path) = pick_file() {
            self.open(path, false);
        }
        if new && let Some(path) = pick_new_file() {
            self.start_new(&path);
        }
    }
}

/// How wide the start page may be, in points.
pub const START_PAGE_WIDTH: f32 = 1400.0;
/// From how wide "How it works" sits beside the examples, not under them.
pub const HOW_BESIDE_FROM: f32 = 1100.0;
/// How wide "How it works" is beside the examples.
pub const HOW_WIDTH: f32 = 280.0;
/// How narrow an example's card may be before fewer fit across.
pub const CARD_SMALLEST: f32 = 150.0;

/// An example on the start page: its picture, its name, and what it
/// shows in a line or two under it. The whole card is one button.
fn example_card(
    ui: &mut egui::Ui,
    picture: Option<&egui::TextureHandle>,
    name: &str,
    about: &str,
    side: f32,
) -> egui::Response {
    // What it shows: two lines at most, with "..." if it goes on.
    let mut job = egui::text::LayoutJob::simple(
        about.to_owned(),
        egui::FontId::proportional(theme::SMALLEST_TEXT),
        ui.visuals().weak_text_color(),
        side,
    );
    job.wrap.max_rows = 2;
    let about_text = ui.painter().layout_job(job);
    let about_height = about_text.size().y;
    let size = egui::vec2(side + 12.0, side + 30.0 + about_height + 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, visuals.weak_bg_fill);
        if response.hovered() || response.has_focus() {
            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.5, theme::DUCK_YELLOW),
                egui::StrokeKind::Inside,
            );
        }
        let frame = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(side, side));
        if let Some(picture) = picture {
            egui::Image::new(picture)
                .corner_radius(4.0)
                .paint_at(ui, frame);
        }
        painter.text(
            pos2(frame.left(), frame.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(14.0),
            visuals.text_color(),
        );
        painter.galley(
            pos2(frame.left(), frame.bottom() + 24.0),
            about_text,
            ui.visuals().weak_text_color(),
        );
    }
    response
}

/// A tile of the start page: a picture with a name under it, to click.
/// With no picture (a model that doesn't build), the tile is plain.
fn tile(
    ui: &mut egui::Ui,
    picture: Option<&egui::TextureHandle>,
    name: &str,
    side: f32,
) -> egui::Response {
    let size = egui::vec2(side + 12.0, side + 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, visuals.weak_bg_fill);
        if response.hovered() || response.has_focus() {
            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.5, theme::DUCK_YELLOW),
                egui::StrokeKind::Inside,
            );
        }
        let frame = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(side, side));
        match picture {
            Some(picture) => {
                egui::Image::new(picture)
                    .corner_radius(4.0)
                    .paint_at(ui, frame);
            }
            None => {
                painter.rect_filled(frame, 4.0, ui.visuals().extreme_bg_color);
                painter.text(
                    frame.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    egui::FontId::proportional(side * 0.4),
                    ui.visuals().weak_text_color(),
                );
            }
        }
        // The name, cut short if it is too long for the tile.
        let mut shown = name.to_owned();
        let font = egui::FontId::proportional(13.0);
        let fits = |text: &str| {
            painter
                .layout_no_wrap(text.to_owned(), font.clone(), visuals.text_color())
                .size()
                .x
                <= side + 4.0
        };
        if !fits(&shown) {
            while shown.chars().count() > 1 && !fits(&format!("{shown}...")) {
                shown.pop();
            }
            shown.push_str("...");
        }
        painter.text(
            egui::pos2(rect.center().x, frame.bottom() + 12.0),
            egui::Align2::CENTER_CENTER,
            shown,
            font,
            visuals.text_color(),
        );
    }
    response
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // A recipe dropped on the window is opened.
        let dropped = ui.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_path_buf())
        });
        if let Some(path) = dropped {
            self.open(path, false);
        }
        StudioApp::ui(self, ui);
        // (Kept again after the frame, in case the window closes next.)
        if let Some(studio) = &self.studio {
            self.recent.keep_hidden(studio.path(), studio.hidden());
        }
        let title = self.title();
        if title != self.titled {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.titled = title;
        }
    }
}

/// Asks which file to open.
fn pick_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open a model")
        .add_filter(
            "Recipes and models",
            &["toml", "json", "obj", "gltf", "glb"],
        )
        .add_filter("Recipes", &["toml", "json"])
        .pick_file()
}

/// What was clicked in the notice of a crash.
enum CrashAnswer {
    Open(PathBuf),
    ShowNote,
    Dismiss,
}

/// The start page's word about a crash last time.
fn crashed_notice(ui: &mut egui::Ui, crash: &crate::crash::Crash) -> Option<CrashAnswer> {
    let mut answer = None;
    egui::Frame::new()
        .fill(theme::WARNING_FILL)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::icon(ui, Icon::Warning, theme::LOOK);
                ui.label(RichText::new("Duck Mold Studio closed unexpectedly last time.").strong());
            });
            if let Some(file) = &crash.file {
                let name = file
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                ui.label(format!(
                    "Your model was saved up to its last change: {name}."
                ));
            }
            ui.horizontal(|ui| {
                if let Some(file) = &crash.file
                    && ui.button("Open it").clicked()
                {
                    answer = Some(CrashAnswer::Open(file.clone()));
                }
                if ui
                    .button("Show the crash note")
                    .on_hover_text(crash.note.display().to_string())
                    .clicked()
                {
                    answer = Some(CrashAnswer::ShowNote);
                }
                if ui.button("Don't show again").clicked() {
                    answer = Some(CrashAnswer::Dismiss);
                }
            });
        });
    answer
}

/// Asks where a recipe that was deleted or moved should be saved.
fn pick_save_as(name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save the model as")
        .add_filter("Recipe", &["toml", "json"])
        .set_file_name(name)
        .save_file()
}

/// The bar over the picture when the recipe isn't there any more:
/// save it again, save it somewhere else, or close it. Gives the one
/// clicked.
fn missing_bar(ui: &mut egui::Ui, why: &str, look_only: bool) -> Option<Action> {
    let mut asked = None;
    egui::Frame::new()
        .fill(theme::WARNING_FILL)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::icon(ui, Icon::Warning, theme::LOOK);
                ui.label(RichText::new(format!("{why}.")).strong());
            });
            ui.horizontal(|ui| {
                if !look_only {
                    if ui
                        .button("Save it again")
                        .on_hover_text(
                            "Write the model back where it was, as the Studio last saw it.",
                        )
                        .clicked()
                    {
                        asked = Some(Action::SaveAgain);
                    }
                    if ui
                        .button("Save as...")
                        .on_hover_text("Write the model somewhere else, and go on working there.")
                        .clicked()
                    {
                        asked = Some(Action::SaveAs);
                    }
                }
                if ui
                    .button("Close it")
                    .on_hover_text("Go back to the start page. The model is not saved.")
                    .clicked()
                {
                    asked = Some(Action::StartPage);
                }
            });
        });
    asked
}

/// Asks where a new recipe should go.
fn pick_new_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("New model")
        .add_filter("Recipe", &["toml"])
        .set_file_name("model.toml")
        .save_file()
}

/// The parts, as a tree. A click selects one, Shift+click adds one to
/// the selection or takes it out, and a right-click opens the menu. The
/// eye at the end of a line hides the part, or shows it again. The
/// selected ones are marked, and the one selected last is scrolled to
/// when it was selected somewhere else. Gives what the menu asked for.
fn outliner(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    shown_selected: &mut Option<String>,
    thumbs: &mut library::Thumbs,
    filter: &mut String,
) -> Option<Action> {
    ui.add_space(4.0);
    ui.label(RichText::new("Parts").strong());
    if let Some(headline) = studio.headline() {
        ui.label(RichText::new(headline).weak().small());
    }
    // A long list can be filtered by name.
    let all = studio.outline();
    if all.len() > FILTER_FROM {
        ui.add(
            egui::TextEdit::singleline(filter)
                .hint_text("Find a part")
                .desired_width(f32::INFINITY),
        );
    } else {
        filter.clear();
    }
    ui.separator();
    let selected = studio.selected().map(str::to_owned);
    let newly_selected = selected != *shown_selected;
    let shift = ui.input(|input| input.modifiers.shift);
    let mut clicked = None;
    let mut menu_for = None;
    let mut eye = None;
    let mut asked = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| {
            let wanted = filter.trim().to_lowercase();
            let rows: Vec<_> = all
                .into_iter()
                .filter(|row| wanted.is_empty() || row.path.to_lowercase().contains(&wanted))
                .collect();
            // In Pose mode, the parts the pose turns are marked.
            let posed = if studio.mode() == Mode::Pose {
                studio.posed_parts()
            } else {
                Vec::new()
            };
            if rows.is_empty() && !wanted.is_empty() {
                ui.label(RichText::new(format!("No part is called \"{}\".", filter.trim())).weak());
            } else if rows.is_empty() {
                ui.label(RichText::new("No parts to show.").weak());
            }
            for row in rows {
                let is_selected = studio.is_selected(&row.path);
                ui.horizontal(|ui| {
                    ui.add_space(row.depth as f32 * 14.0);
                    // The eye first, at the right; the name and what the
                    // part is take the room that is left.
                    // The eye shows on the line the mouse is on, and stays
                    // shut on a hidden part.
                    let line_hovered = ui.rect_contains_pointer(ui.max_rect());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (mark, icon, tip) = if row.hidden {
                            ("show", Icon::EyeShut, "Hidden: click to show it again")
                        } else {
                            ("hide", Icon::Eye, "Click to hide it. It stays in the model")
                        };
                        if icons::quiet_button(ui, icon, mark, row.hidden || line_hovered)
                            .on_hover_text(format!("{tip} ({})", row.path))
                            .clicked()
                        {
                            eye = Some((row.path.clone(), !row.hidden));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let turned = posed.contains(&row.path);
                            let words = if row.hidden {
                                RichText::new(&row.name).weak().strikethrough()
                            } else if turned {
                                RichText::new(format!("{} *", row.name)).color(theme::DUCK_YELLOW)
                            } else {
                                RichText::new(&row.name)
                            };
                            let name =
                                ui.selectable_label(is_selected, words)
                                    .on_hover_text(format!(
                                        "{}: {} triangles",
                                        row.path, row.triangles
                                    ));
                            if name.clicked() {
                                clicked = Some(row.path.clone());
                            }
                            if name.secondary_clicked() {
                                menu_for = Some(row.path.clone());
                            }
                            name.context_menu(|ui| {
                                if let Some(action) = actions::part_menu(ui, studio, thumbs) {
                                    asked = Some(action);
                                }
                            });
                            if selected.as_deref() == Some(row.path.as_str()) && newly_selected {
                                name.scroll_to_me(None);
                            }
                            ui.add(
                                egui::Label::new(RichText::new(&row.kind).weak().small())
                                    .truncate(),
                            );
                        });
                    });
                });
            }
        });
    *shown_selected = selected;
    if let Some(path) = menu_for
        && !studio.is_selected(&path)
    {
        // A right-click on a part that isn't selected selects it first.
        studio.select(Some(&path));
    }
    if let Some(path) = clicked {
        if shift {
            studio.toggle(&path);
        } else if studio.selection() == [path.as_str()] {
            // Clicking the only selected part again lets go of it.
            studio.select(None);
        } else {
            studio.select(Some(&path));
        }
    }
    if let Some((path, hidden)) = eye {
        studio.set_hidden(&path, hidden);
    }
    asked
}

/// What is wrong with the file, or that nothing is.
/// Where the guide for people is.
pub const GUIDE: &str = "https://github.com/mrguymrguy647-del/Duck-Mold#readme";

/// Where Duck Mold is made.
pub const WEBSITE: &str = "https://github.com/mrguymrguy647-del/Duck-Mold";

/// How many parts a model has before its list gets a box to find one.
pub const FILTER_FROM: usize = 12;

/// How tall the status bar is, in points.
pub const STATUS_HEIGHT: f32 = 26.0;

/// How long a note over the picture stays, in seconds.
pub const NOTE_SECONDS: f64 = 4.0;

/// A note over the picture: what was written, and where.
pub struct Note {
    text: String,
    file: Option<PathBuf>,
    /// When it was shown, in the window's seconds.
    since: f64,
}

/// A note for a file written, or for why it wasn't.
fn note_of(note: &mut Option<Note>, done: Result<String, String>, studio: &Studio, now: f64) {
    *note = Some(match done {
        Ok(said) => Note {
            text: said,
            file: studio.last_written().map(Path::to_path_buf),
            since: now,
        },
        Err(why) => Note {
            text: why,
            file: None,
            since: now,
        },
    });
}

/// Draws the note in the top right of the picture. True if it was
/// closed.
fn show_note(ctx: &egui::Context, note: &Note, picture: Rect) -> bool {
    let mut closed = false;
    egui::Area::new(egui::Id::unique("duck_mold_note"))
        .order(egui::Order::Foreground)
        // At the top of the picture, in the middle: the corners have the
        // tools and the gizmo.
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(picture.center().x, picture.top() + 10.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(360.0);
                ui.horizontal(|ui| {
                    ui.label(&note.text);
                    if let Some(file) = &note.file
                        && ui
                            .button("Show in folder")
                            .on_hover_text(file.display().to_string())
                            .clicked()
                    {
                        crate::system::show_in_folder(file);
                        closed = true;
                    }
                });
            });
        });
    closed
}

/// The words on the problems button: none, warnings, or mistakes.
pub fn problems_words(studio: &Studio) -> (Color32, String) {
    let problems = studio.problems();
    let mistakes = problems.iter().filter(|problem| problem.is_error).count();
    let warnings = problems.len() - mistakes;
    let count = |count: usize, word: &str| {
        if count == 1 {
            format!("1 {word}")
        } else {
            format!("{count} {word}s")
        }
    };
    match (mistakes, warnings) {
        (0, 0) => (theme::GOOD, "No problems".to_owned()),
        (0, warnings) => (theme::LOOK, count(warnings, "warning")),
        (mistakes, 0) => (theme::BAD, count(mistakes, "mistake")),
        (mistakes, warnings) => (
            theme::BAD,
            format!(
                "{}, {}",
                count(mistakes, "mistake"),
                count(warnings, "warning")
            ),
        ),
    }
}

/// The line at the bottom: the problems, as a button that opens their
/// list, and what the Studio did last.
fn status_bar(ui: &mut egui::Ui, studio: &Studio, open: &mut bool) {
    ui.horizontal_centered(|ui| {
        let (colour, words) = problems_words(studio);
        let mark = if studio.has_mistakes() {
            Icon::Mistake
        } else if studio.problems().is_empty() {
            Icon::Check
        } else {
            Icon::Warning
        };
        icons::icon(ui, mark, colour);
        let button = ui
            .add(
                egui::Button::new(RichText::new(&words).color(colour))
                    .selected(*open)
                    .frame_when_inactive(false),
            )
            .on_hover_text(if *open {
                "Close the list of problems (Escape)"
            } else {
                "Open the list of problems"
            });
        if button.clicked() {
            *open = !*open;
        }
        // The far-away version the picture shows, and how many
        // triangles it has.
        if let Some(far) = studio.far_drawn() {
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Showing Far {}: {} triangles",
                    studio.far,
                    far.triangle_count()
                ))
                .color(theme::LOOK),
            )
            .on_hover_text("View > Far version > The model itself shows the model again");
        }
        ui.separator();
        ui.add(egui::Label::new(RichText::new(studio.status()).weak()).truncate());
    });
}

/// The list of problems, opened above the status bar. A click on one
/// about a part selects that part.
fn problems(ui: &mut egui::Ui, studio: &mut Studio) {
    let problems = studio.problems().to_vec();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Problems").strong());
        if problems.is_empty() {
            ui.colored_label(theme::GOOD, "None");
        } else if studio.has_mistakes() && studio.model().is_none() {
            ui.colored_label(theme::BAD, "This file can't be built yet:");
        } else if studio.has_mistakes() {
            ui.colored_label(
                theme::BAD,
                "The file has mistakes. The picture is of the last version that worked.",
            );
        }
    });
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for problem in &problems {
                let (colour, word) = if problem.is_error {
                    (theme::BAD, "mistake")
                } else {
                    (theme::LOOK, "warning")
                };
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(colour, word);
                    match &problem.part {
                        // A problem about a part selects it when clicked.
                        Some(part) => {
                            let line = ui
                                .add(egui::Label::new(&problem.text).sense(Sense::click()))
                                .on_hover_text(format!("Click to select {part}"));
                            if line.clicked() {
                                studio.select(Some(part));
                            }
                        }
                        None => {
                            ui.label(&problem.text);
                        }
                    }
                });
            }
        });
}
