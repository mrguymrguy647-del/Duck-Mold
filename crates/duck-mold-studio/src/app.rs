//! The Studio's screen.
//!
//! [`StudioApp::ui`] draws the whole screen into the space it is given,
//! so it can be the whole window (as here) or one panel of another app.
//! It holds nothing about the model: that is all in
//! [`Studio`](duck_mold_view::studio::Studio), and this only shows it.

use std::path::{Path, PathBuf};

use duck_mold_core::tools;
use duck_mold_view::studio::Studio;
use eframe::egui::{self, Rect, RichText, Sense};

use crate::recent::Recent;
use crate::{library, properties, theme, view};

/// The Studio's window: a file that is open, or the start page.
pub struct StudioApp {
    studio: Option<Studio>,
    /// What the start page says went wrong, if something did.
    notice: Option<String>,
    view: view::View,
    /// The window's title as it was last set.
    titled: String,
    /// The selection the parts list last showed, to scroll to a new one.
    shown_selected: Option<String>,
    /// What is being typed in the properties panel.
    draft: properties::Draft,
    /// The library's small pictures.
    thumbs: library::Thumbs,
    /// The recipes opened lately, for the start page.
    recent: Recent,
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
            view: view::View::default(),
            titled: String::new(),
            shown_selected: None,
            draft: properties::Draft::default(),
            thumbs: library::Thumbs::default(),
            recent: Recent::nowhere(),
        }
    }

    /// Remembers the files that are opened in `recent`, and shows them on
    /// the start page.
    pub fn remembering(mut self, recent: Recent) -> StudioApp {
        self.recent = recent;
        if let Some(studio) = &self.studio {
            self.recent.opened(studio.path());
        }
        self
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
            self.studio = Some(Studio::open(path, look_only));
            self.notice = None;
        } else {
            self.notice = Some(format!("There is no file \"{}\".", path.display()));
        }
        self.view.reset();
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

    /// Where on the screen the viewport was last drawn, and how many
    /// pixels its picture has. The tests use it to click on a part.
    pub fn viewport(&self) -> Option<(Rect, [usize; 2])> {
        self.view.place()
    }

    /// What the window's title should be.
    pub fn title(&self) -> String {
        match &self.studio {
            Some(studio) => format!("{} - Duck Mold Studio", studio.title()),
            None => "Duck Mold Studio".to_owned(),
        }
    }

    /// Draws the whole screen into `ui`.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(studio) = &mut self.studio else {
            self.start_page(ui);
            return;
        };

        // Twice a second, see if the file was saved by something else.
        let seconds = f64::from(ui.input(|input| input.stable_dt)).min(1.0);
        studio.tick(seconds);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));

        let mut open_another = false;
        egui::Panel::top("duck_mold_bar").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(studio.title()).strong());
                if studio.look_only() {
                    ui.label(RichText::new("looking only").weak())
                        .on_hover_text("Nothing can be changed here. Open the recipe in the Studio to edit it.");
                }
                ui.separator();
                if ui
                    .button("Open")
                    .on_hover_text("Open another recipe or model file")
                    .clicked()
                {
                    open_another = true;
                }
                if !studio.look_only() {
                    // What each would do is in its tip, and in the
                    // status line once it is done.
                    let back = studio.can_undo().map(str::to_owned);
                    let undo = ui
                        .add_enabled(back.is_some(), egui::Button::new("Undo"))
                        .on_hover_text(match &back {
                            Some(title) => format!("Undo: {title} (Ctrl+Z)"),
                            None => "Nothing to undo".to_owned(),
                        });
                    if undo.clicked() {
                        let _ = studio.undo();
                    }
                    let again = studio.can_redo().map(str::to_owned);
                    let redo = ui
                        .add_enabled(again.is_some(), egui::Button::new("Redo"))
                        .on_hover_text(match &again {
                            Some(title) => format!("Redo: {title} (Ctrl+Y)"),
                            None => "Nothing to redo".to_owned(),
                        });
                    if redo.clicked() {
                        let _ = studio.redo();
                    }
                    ui.separator();
                }
                if ui
                    .button("Frame")
                    .on_hover_text("Bring the whole model back into view")
                    .clicked()
                {
                    studio.frame();
                }
                ui.toggle_value(&mut studio.grid, "Grid")
                    .on_hover_text("The grid on the ground: one square is one metre");
                ui.separator();
                if ui
                    .button("Export")
                    .on_hover_text("Write the model as a .glb file next to the recipe")
                    .clicked()
                {
                    let _ = studio.export();
                }
                if ui
                    .button("Render picture")
                    .on_hover_text("Write a picture of the model (four views) next to the recipe")
                    .clicked()
                {
                    let _ = studio.render_picture();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(studio.status()).weak());
                });
            });
            ui.add_space(2.0);
        });

        egui::Panel::bottom("duck_mold_problems")
            .resizable(true)
            .default_size(96.0)
            .min_size(56.0)
            .show(ui, |ui| problems(ui, studio));

        egui::Panel::left("duck_mold_parts")
            .resizable(true)
            .default_size(230.0)
            .min_size(140.0)
            .show(ui, |ui| {
                // The library sits under the parts, in the same column.
                if !studio.look_only() {
                    egui::Panel::bottom("duck_mold_library")
                        .resizable(true)
                        .default_size(170.0)
                        .min_size(80.0)
                        .show(ui, |ui| library::panel(ui, studio, &mut self.thumbs));
                }
                outliner(ui, studio, &mut self.shown_selected);
            });

        egui::Panel::right("duck_mold_properties")
            .resizable(true)
            .default_size(270.0)
            .min_size(220.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| properties::show(ui, studio, &mut self.draft));
            });

        egui::CentralPanel::no_frame().show(ui, |ui| {
            view::show(ui, studio, &mut self.view, &mut self.thumbs);
        });

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
                let _ = studio.undo();
            }
            if redo {
                let _ = studio.redo();
            }
        }

        // Escape lets go of the selection. With nothing selected, it
        // closes a window that is only for looking, as `preview` always
        // did.
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.view.is_adding() {
                self.view.stop_adding();
            } else if studio.dragging().is_some() {
                // The viewport lets go of the part it was dragging.
            } else if studio.selected().is_some() {
                studio.select(None);
            } else if studio.look_only() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if open_another && let Some(path) = pick_file() {
            self.open(path, false);
        }
    }

    /// The page shown when no file is open.
    fn start_page(&mut self, ui: &mut egui::Ui) {
        let mut open = false;
        let mut new = false;
        let mut again: Option<PathBuf> = None;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.25);
                ui.label(RichText::new("Duck Mold Studio").size(30.0).strong());
                ui.add_space(4.0);
                ui.label("Build a 3D model from parts that join at sockets.");
                ui.add_space(20.0);
                if ui.add(theme::primary_button("Open a model")).clicked() {
                    open = true;
                }
                ui.add_space(6.0);
                if ui
                    .button("New model")
                    .on_hover_text("Starts a recipe with one box in it")
                    .clicked()
                {
                    new = true;
                }
                if let Some(notice) = &self.notice {
                    ui.add_space(16.0);
                    ui.colored_label(theme::BAD, notice);
                }
                let recent: Vec<PathBuf> = self
                    .recent
                    .files()
                    .into_iter()
                    .map(Path::to_path_buf)
                    .collect();
                if !recent.is_empty() {
                    ui.add_space(24.0);
                    ui.label(RichText::new("Opened lately").weak());
                    ui.add_space(4.0);
                    for file in recent {
                        let name = file
                            .file_name()
                            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                        if ui
                            .button(name)
                            .on_hover_text(file.display().to_string())
                            .clicked()
                        {
                            again = Some(file);
                        }
                    }
                }
            });
        });
        if let Some(file) = again {
            self.open(file, false);
        }
        if open && let Some(path) = pick_file() {
            self.open(path, false);
        }
        if new && let Some(path) = pick_new_file() {
            self.start_new(&path);
        }
    }
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

/// Asks where a new recipe should go.
fn pick_new_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("New model")
        .add_filter("Recipe", &["toml"])
        .set_file_name("model.toml")
        .save_file()
}

/// The parts, as a tree. A click selects one; the selected one is
/// marked, and scrolled to when it was selected somewhere else.
fn outliner(ui: &mut egui::Ui, studio: &mut Studio, shown_selected: &mut Option<String>) {
    ui.add_space(4.0);
    ui.label(RichText::new("Parts").strong());
    if let Some(headline) = studio.headline() {
        ui.label(RichText::new(headline).weak().small());
    }
    ui.separator();
    let selected = studio.selected().map(str::to_owned);
    let newly_selected = selected != *shown_selected;
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let rows = studio.outline();
            if rows.is_empty() {
                ui.label(RichText::new("No parts to show.").weak());
            }
            for row in rows {
                let is_selected = selected.as_deref() == Some(row.path.as_str());
                ui.horizontal(|ui| {
                    ui.add_space(row.depth as f32 * 14.0);
                    let name = ui
                        .selectable_label(is_selected, &row.name)
                        .on_hover_text(format!("{}: {} triangles", row.path, row.triangles));
                    if name.clicked() {
                        clicked = Some(row.path.clone());
                    }
                    if is_selected && newly_selected {
                        name.scroll_to_me(None);
                    }
                    ui.label(RichText::new(&row.kind).weak().small());
                });
            }
        });
    *shown_selected = selected;
    if let Some(path) = clicked {
        // Clicking the selected part again lets go of it.
        if studio.selected() == Some(path.as_str()) {
            studio.select(None);
        } else {
            studio.select(Some(&path));
        }
    }
}

/// What is wrong with the file, or that nothing is.
fn problems(ui: &mut egui::Ui, studio: &mut Studio) {
    let problems = studio.problems().to_vec();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Problems").strong());
        if problems.is_empty() {
            ui.colored_label(theme::GOOD, "None");
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
