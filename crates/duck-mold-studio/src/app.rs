//! The Studio's screen.
//!
//! [`StudioApp::ui`] draws the whole screen into the space it is given,
//! so it can be the whole window (as here) or one panel of another app.
//! It holds nothing about the model: that is all in
//! [`Studio`](duck_mold_view::studio::Studio), and this only shows it.

use std::path::{Path, PathBuf};

use duck_mold_core::tools;
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::viewport::Orbit;
use eframe::egui::{
    self, Color32, ColorImage, PointerButton, Rect, RichText, Sense, TextureHandle, TextureOptions,
    pos2,
};

use crate::theme;

/// The most pixels the viewport is drawn with. On a very large or very
/// sharp screen the picture is drawn smaller and stretched, so turning
/// the model stays smooth.
const MOST_PIXELS: usize = 2_500_000;

/// How far the wheel must roll for one step of zoom, in points.
const WHEEL_STEP: f32 = 50.0;

/// What the viewport's picture was last drawn from. When none of it has
/// changed, the picture is still right and isn't drawn again.
#[derive(Clone, PartialEq)]
struct Drawn {
    builds: usize,
    orbit: Orbit,
    grid: bool,
    selected: Option<String>,
    size: [usize; 2],
}

/// The Studio's window: a file that is open, or the start page.
pub struct StudioApp {
    studio: Option<Studio>,
    /// What the start page says went wrong, if something did.
    notice: Option<String>,
    view: Option<TextureHandle>,
    drawn: Option<Drawn>,
    /// The window's title as it was last set.
    titled: String,
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
            view: None,
            drawn: None,
            titled: String::new(),
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
            self.studio = Some(Studio::open(path, look_only));
            self.notice = None;
        } else {
            self.notice = Some(format!("There is no file \"{}\".", path.display()));
        }
        self.drawn = None;
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
            .show(ui, |ui| outliner(ui, studio));

        egui::CentralPanel::no_frame().show(ui, |ui| {
            viewport(ui, studio, &mut self.view, &mut self.drawn);
        });

        if studio.look_only() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if open_another && let Some(path) = pick_file() {
            self.open(path, false);
        }
    }

    /// The page shown when no file is open.
    fn start_page(&mut self, ui: &mut egui::Ui) {
        let mut open = false;
        let mut new = false;
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
            });
        });
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

/// The parts, as a tree.
fn outliner(ui: &mut egui::Ui, studio: &Studio) {
    ui.add_space(4.0);
    ui.label(RichText::new("Parts").strong());
    if let Some(headline) = studio.headline() {
        ui.label(RichText::new(headline).weak().small());
    }
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let rows = studio.outline();
            if rows.is_empty() {
                ui.label(RichText::new("No parts to show.").weak());
            }
            for row in rows {
                ui.horizontal(|ui| {
                    ui.add_space(row.depth as f32 * 14.0);
                    ui.label(&row.name);
                    ui.label(RichText::new(&row.kind).weak().small());
                })
                .response
                .on_hover_text(format!("{}: {} triangles", row.path, row.triangles));
            }
        });
}

/// What is wrong with the file, or that nothing is.
fn problems(ui: &mut egui::Ui, studio: &Studio) {
    let problems = studio.problems();
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
            for problem in problems {
                let (colour, word) = if problem.is_error {
                    (theme::BAD, "mistake")
                } else {
                    (theme::LOOK, "warning")
                };
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(colour, word);
                    ui.label(&problem.text);
                });
            }
        });
}

/// The picture of the model, and the mouse on it: drag to orbit,
/// right-drag (or middle-drag, or Shift and drag) to pan, the wheel to
/// zoom.
fn viewport(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    view: &mut Option<TextureHandle>,
    drawn: &mut Option<Drawn>,
) {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    if rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }

    let moved = response.drag_delta();
    let (right, down) = (f64::from(moved.x), f64::from(moved.y));
    let shift = ui.input(|input| input.modifiers.shift);
    if response.dragged_by(PointerButton::Primary) && !shift {
        studio.orbit.turn(right, down);
    } else if response.dragged_by(PointerButton::Primary)
        || response.dragged_by(PointerButton::Secondary)
        || response.dragged_by(PointerButton::Middle)
    {
        studio.orbit.pan(right, down, rect.height() as usize);
    }
    if response.hovered() {
        let rolled = ui.input(|input| input.smooth_scroll_delta.y);
        if rolled != 0.0 {
            studio.orbit.zoom(f64::from(rolled / WHEEL_STEP));
        }
    }

    // The picture has one pixel for each of the screen's, unless that
    // would be too many to draw quickly.
    let sharpness = ui.ctx().pixels_per_point();
    let mut width = (rect.width() * sharpness).round() as usize;
    let mut height = (rect.height() * sharpness).round() as usize;
    while width * height > MOST_PIXELS {
        width /= 2;
        height /= 2;
    }
    let now = Drawn {
        builds: studio.builds(),
        orbit: studio.orbit,
        grid: studio.grid,
        selected: studio.selected().map(str::to_owned),
        size: [width.max(1), height.max(1)],
    };
    if drawn.as_ref() != Some(&now) || view.is_none() {
        let picture = studio.picture(now.size[0], now.size[1]);
        let image = ColorImage::new(
            now.size,
            picture
                .pixels
                .iter()
                .map(|pixel| {
                    Color32::from_rgb((pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8)
                })
                .collect(),
        );
        match view {
            Some(texture) => texture.set(image, TextureOptions::NEAREST),
            None => {
                *view = Some(ui.ctx().load_texture(
                    "duck_mold_viewport",
                    image,
                    TextureOptions::NEAREST,
                ));
            }
        }
        *drawn = Some(now);
    }
    if let Some(texture) = view {
        ui.painter_at(rect).image(
            texture.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}
