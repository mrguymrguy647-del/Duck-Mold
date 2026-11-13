//! The Studio's screen.
//!
//! [`StudioApp::ui`] draws the whole screen into the space it is given,
//! so it can be the whole window (as here) or one panel of another app.
//! It holds nothing about the model: that is all in
//! [`Studio`](duck_mold_view::studio::Studio), and this only shows it.

use std::path::{Path, PathBuf};

use duck_mold_core::tools;
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::viewport::Viewpoint;
use eframe::egui::{self, Rect, RichText, Sense};

use crate::actions::{self, Action};
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
    /// Which of Help's windows is open, if one is.
    showing: Option<Showing>,
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
            view: view::View::default(),
            titled: String::new(),
            shown_selected: None,
            draft: properties::Draft::default(),
            thumbs: library::Thumbs::default(),
            recent: Recent::nowhere(),
            showing: None,
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

        // What a menu or a key asked for, carried out at the end.
        let mut asked: Option<Action> = None;
        egui::Panel::top("duck_mold_bar").show(ui, |ui| {
            ui.add_space(2.0);
            // One row on a wide window; it wraps to two on a narrow one.
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(studio.title()).strong());
                if studio.look_only() {
                    ui.label(RichText::new("looking only").weak())
                        .on_hover_text("Nothing can be changed here. Open the recipe in the Studio to edit it.");
                }
                ui.separator();
                if let Some(action) = actions::menus(ui, studio) {
                    asked = Some(action);
                }
                ui.separator();
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
                        asked = Some(Action::Undo);
                    }
                    let again = studio.can_redo().map(str::to_owned);
                    let redo = ui
                        .add_enabled(again.is_some(), egui::Button::new("Redo"))
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
                    ui.menu_button("Add", |ui| {
                        ui.label(RichText::new("A new part, on the ground").weak().small());
                        if let Some(made) = view::choices(ui, studio, thumbs) {
                            asked = Some(Action::AddFree(made));
                            ui.close();
                        }
                    })
                    .response
                    .on_hover_text(
                        "Add a part that stands on the ground, joined to nothing (Shift+A). To add one joined to another part, click a dot on that part.",
                    );
                    ui.separator();
                }
                // The four sides to look from.
                for (from, key) in Viewpoint::ALL.into_iter().zip(["1", "2", "3", "4"]) {
                    let tip = match from {
                        Viewpoint::Front => "Look at the front",
                        Viewpoint::Side => "Look at the right side",
                        Viewpoint::Top => "Look down from the top",
                        Viewpoint::ThreeQuarter => "Look from the front right, a little above",
                    };
                    if ui
                        .button(from.name())
                        .on_hover_text(format!("{tip} ({key})"))
                        .clicked()
                    {
                        asked = Some(Action::LookFrom(from));
                    }
                }
                if ui
                    .button("Frame")
                    .on_hover_text("Bring what is selected, or the whole model, into view (F)")
                    .clicked()
                {
                    asked = Some(Action::Frame);
                }
                ui.separator();
                ui.toggle_value(&mut studio.grid, "Grid")
                    .on_hover_text("The grid on the ground: one square is one metre");
                ui.toggle_value(&mut studio.wire, "Wire")
                    .on_hover_text("Wireframe: only the outline of each face, to see through the model (Z)");
                ui.toggle_value(&mut studio.shadow, "Shadow")
                    .on_hover_text("Darken the ground under the model, darkest where something stands on it");
                if studio.hidden_count() > 0
                    && ui
                        .button(format!("{} hidden", studio.hidden_count()))
                        .on_hover_text(
                            "Parts that aren't drawn. They are still in the model. Click to show them all (Shift+H)",
                        )
                        .clicked()
                {
                    asked = Some(Action::ShowAll);
                }
                ui.separator();
                if ui
                    .button("Export")
                    .on_hover_text("Write the model as a .glb file next to the recipe (Ctrl+E)")
                    .clicked()
                {
                    asked = Some(Action::Export);
                }
                if ui
                    .button("Render picture")
                    .on_hover_text("Write a picture of the model (four views) next to the recipe")
                    .clicked()
                {
                    asked = Some(Action::RenderPicture);
                }
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
                if let Some(action) =
                    outliner(ui, studio, &mut self.shown_selected, &mut self.thumbs)
                {
                    // From the list there is no point of a surface.
                    self.view.clear_menu_spot();
                    asked = Some(action);
                }
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
            if let Some(action) = view::show(ui, studio, &mut self.view, &mut self.thumbs) {
                asked = Some(action);
            }
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
                asked = Some(Action::Undo);
            }
            if redo {
                asked = Some(Action::Redo);
            }
            // The tools' keys, as in other 3D programs.
            for (key, tool) in [
                (egui::Key::G, Tool::Move),
                (egui::Key::R, Tool::Rotate),
                (egui::Key::S, Tool::Scale),
                (egui::Key::T, Tool::Size),
            ] {
                if ui.input(|input| input.modifiers.is_none() && input.key_pressed(key)) {
                    studio.tool = tool;
                }
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
        if let Some(action) = asked {
            // Only what changes nothing is done when looking only.
            let allowed = !studio.look_only()
                || matches!(
                    action,
                    Action::Frame
                        | Action::Hide
                        | Action::ShowAll
                        | Action::SelectAll
                        | Action::LookFrom(_)
                        | Action::Grid
                        | Action::Wireframe
                        | Action::Shadow
                        | Action::Open
                        | Action::NewModel
                        | Action::Export
                        | Action::RenderPicture
                        | Action::StartPage
                        | Action::ShowKeys
                        | Action::About
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
                Action::NewModel => new_model = true,
                Action::Open => open_another = true,
                Action::Export => {
                    let _ = studio.export();
                }
                Action::RenderPicture => {
                    let _ = studio.render_picture();
                }
                Action::StartPage => to_start = true,
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
                Action::Remove => {
                    if !studio.selection().is_empty() {
                        self.draft.remove(studio);
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
        self.help_window(ui);
        if open_another && let Some(path) = pick_file() {
            self.open(path, false);
        }
        if new_model && let Some(path) = pick_new_file() {
            self.start_new(&path);
        }
        if to_start {
            self.studio = None;
            self.view.reset();
        }
    }

    /// The list of keys, or what the program is: Help's two windows.
    fn help_window(&mut self, ui: &mut egui::Ui) {
        let Some(showing) = self.showing else {
            return;
        };
        let mut open = true;
        let title = match showing {
            Showing::Keys => "The keys",
            Showing::About => "About Duck Mold Studio",
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| match showing {
                Showing::Keys => {
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
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Keys don't fire while you are typing in a field.").weak(),
                    );
                }
                Showing::About => {
                    ui.label(
                        RichText::new(format!(
                            "Duck Mold Studio {}",
                            env!("CARGO_PKG_VERSION")
                        ))
                        .strong(),
                    );
                    ui.label("Make 3D models for Duck Forge games from parts that join at sockets.");
                    ui.add_space(4.0);
                    ui.label(
                        "A model is a recipe: a text file. The Studio changes that file every time you change something, so there is nothing to save, and an AI agent can read and change it too.",
                    );
                }
            });
        if !open || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.showing = None;
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
) -> Option<Action> {
    ui.add_space(4.0);
    ui.label(RichText::new("Parts").strong());
    if let Some(headline) = studio.headline() {
        ui.label(RichText::new(headline).weak().small());
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
        .show(ui, |ui| {
            let rows = studio.outline();
            if rows.is_empty() {
                ui.label(RichText::new("No parts to show.").weak());
            }
            for row in rows {
                let is_selected = studio.is_selected(&row.path);
                ui.horizontal(|ui| {
                    ui.add_space(row.depth as f32 * 14.0);
                    // The eye first, at the right; the name and what the
                    // part is take the room that is left.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (mark, tip) = if row.hidden {
                            ("show", "Hidden: click to show it again")
                        } else {
                            ("hide", "Click to hide it. It stays in the model")
                        };
                        let button =
                            egui::Button::new(RichText::new(mark).small().weak()).frame(row.hidden);
                        if ui
                            .add(button)
                            .on_hover_text(format!("{tip} ({})", row.path))
                            .clicked()
                        {
                            eye = Some((row.path.clone(), !row.hidden));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let words = if row.hidden {
                                RichText::new(&row.name).weak().strikethrough()
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
        // What was last done, at the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(studio.status()).weak());
        });
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
