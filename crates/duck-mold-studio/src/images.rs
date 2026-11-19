//! Images in the Studio: the row of them under the palette, to pick one
//! to paint with; the lines in a part's properties, to see, choose or
//! take off its images; and the "New image" window.
//!
//! Each change is an edit: `set --image`, `set --face-image`, and
//! `duck-mold new-image` for a new one.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use duck_mold_core::edit::Facts;
use duck_mold_core::images::ImagePlan;
use duck_mold_core::picture::Picture;
use duck_mold_core::recipe::{NO_IMAGE, OWN_IMAGE};
use duck_mold_view::quill::{self, PaintJob, With};
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::images::{Brush, OwnImage};
use eframe::egui::{
    self, Color32, ColorImage, Rect, Response, RichText, Sense, Stroke, TextureHandle,
    TextureOptions, WidgetInfo,
};

use crate::theme;

/// What is open or kept for the images: the small pictures of them, and
/// the "New image" window.
#[derive(Default)]
pub struct Draft {
    /// Each image as a picture egui can draw, by the image's stamp (a
    /// number that changes when its pixels do).
    thumbs: HashMap<u64, TextureHandle>,
    /// The "New image" window, when it is open.
    new: Option<NewImage>,
    /// Why the last change wasn't made.
    failed: Option<String>,
    /// Painting an image in Duck Quill, or another picture program.
    pub painting: Painting,
}

/// What the Studio knows about painting images in another program.
#[derive(Default)]
pub struct Painting {
    /// Duck Mold's settings folder: where the program chosen is
    /// remembered and the guides are written. Without one (the tests),
    /// nothing is remembered and Duck Quill isn't looked for.
    pub settings: Option<PathBuf>,
    /// The program to use whatever is remembered or found: for tests.
    pub with: Option<With>,
    /// Whether programs are really started (not in the tests).
    pub start: bool,
    /// The last image handed to a program.
    pub last: Option<PaintJob>,
    /// The part (and face) waiting to be painted while the window that
    /// says Duck Quill can't be found is open.
    asking: Option<(String, Option<String>)>,
    /// Whether Help's "Duck Quill" window is open.
    pub help: bool,
}

impl Painting {
    /// The program images are painted in: the one chosen before, if it
    /// is still there, or Duck Quill where it is found. `None`: nothing
    /// was chosen and Duck Quill can't be found.
    pub fn program(&self) -> Option<With> {
        if let Some(with) = &self.with {
            return Some(with.clone());
        }
        let settings = self.settings.as_deref()?;
        match quill::remembered(settings) {
            Some(With::System) => Some(With::System),
            Some(With::DuckQuill(program)) => {
                quill::find_duck_quill_here(Some(&program)).map(With::DuckQuill)
            }
            None => quill::find_duck_quill_here(None).map(With::DuckQuill),
        }
    }

    /// What the button says: "Paint in Duck Quill", or, when the system's
    /// picture program was chosen instead, "Open in my picture program".
    pub fn words(&self) -> &'static str {
        match self.program() {
            Some(With::System) => "Open in my picture program",
            _ => "Paint in Duck Quill",
        }
    }

    /// Hands the image a part (or one face of it) shows to `with`, and
    /// remembers the choice. What was started, or why not, is said in
    /// the status bar.
    fn hand_over(&mut self, studio: &mut Studio, part: &str, face: Option<&str>, with: With) {
        let guides = match &self.settings {
            Some(settings) => settings.join("guides"),
            None => std::env::temp_dir().join("duck-mold-guides"),
        };
        let job = match studio.paint_job(part, face, with.clone(), &guides) {
            Ok(job) => job,
            Err(why) => {
                studio.say_why(&why);
                return;
            }
        };
        if let Some(settings) = &self.settings {
            quill::remember(settings, &with);
        }
        let said = if self.start {
            job.start()
        } else {
            Ok(format!("Would start: {}", job.words().join(" ")))
        };
        match said {
            Ok(said) => studio.say_why(&said),
            Err(why) => studio.say_why(&why),
        }
        self.last = Some(job);
    }

    /// "Paint in Duck Quill" for the image a part (or one face of it)
    /// shows. If Duck Quill can't be found and nothing else was chosen,
    /// a window asks what to do.
    pub fn paint(&mut self, studio: &mut Studio, part: &str, face: Option<&str>) {
        match self.program() {
            Some(with) => self.hand_over(studio, part, face, with),
            None => self.asking = Some((part.to_owned(), face.map(str::to_owned))),
        }
    }
}

/// Where Duck Quill can be had.
pub const GET_DUCK_QUILL: &str = "https://github.com/mrguymrguy647-del/duck-quill/releases";

/// Asks where Duck Quill's program is.
fn pick_duck_quill() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Where is Duck Quill?")
        .add_filter("Duck Quill", &["exe", ""])
        .pick_file()
}

/// The buttons both of the Duck Quill windows have: find it, get it, or
/// use the system's picture program. Gives the program chosen.
fn program_buttons(ui: &mut egui::Ui) -> Option<With> {
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Find DuckQuill.exe...")
            .on_hover_text("Show Duck Mold where Duck Quill's program is. It is remembered")
            .clicked()
            && let Some(program) = pick_duck_quill()
        {
            chosen = Some(With::DuckQuill(program));
        }
        ui.hyperlink_to("Get Duck Quill", GET_DUCK_QUILL)
            .on_hover_text("Duck Quill's download page");
    });
    if ui
        .button("Open in my picture program instead")
        .on_hover_text("Whatever this computer opens PNG files with. It is remembered")
        .clicked()
    {
        chosen = Some(With::System);
    }
    ui.label(
        RichText::new("The image is a plain PNG: any program that saves it works, and the model shows it at once.")
            .weak()
            .small(),
    );
    chosen
}

/// The windows about painting in another program: the one that says
/// Duck Quill can't be found, and Help's "Duck Quill".
pub fn painting_windows(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    let painting = &mut draft.painting;
    if let Some((part, face)) = painting.asking.clone() {
        let mut open = true;
        let mut chosen = None;
        egui::Window::new("Duck Quill isn't on this computer")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("Or Duck Mold can't find it. Duck Quill is the Duck painting program: it opens the image with a guide of the faces over it.");
                ui.add_space(4.0);
                chosen = program_buttons(ui);
            });
        if let Some(with) = chosen {
            painting.asking = None;
            painting.hand_over(studio, &part, face.as_deref(), with);
        } else if !open {
            painting.asking = None;
        }
    }
    if painting.help {
        let mut open = true;
        let mut chosen = None;
        let now = painting.program();
        egui::Window::new("Duck Quill")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("Images are painted in:");
                match &now {
                    Some(With::DuckQuill(program)) => {
                        ui.label(
                            RichText::new(format!("Duck Quill, at {}", program.display())).strong(),
                        );
                    }
                    Some(With::System) => {
                        ui.label(
                            RichText::new("the picture program this computer opens PNG files with")
                                .strong(),
                        );
                    }
                    None => {
                        ui.label(RichText::new("nothing yet: Duck Quill wasn't found").strong());
                    }
                }
                ui.add_space(4.0);
                chosen = program_buttons(ui);
            });
        if let Some(with) = chosen {
            if let Some(settings) = &painting.settings {
                quill::remember(settings, &with);
            }
            painting.with = painting.with.as_ref().map(|_| with.clone());
            studio.say_why(&match &with {
                With::DuckQuill(program) => {
                    format!("Images are painted in Duck Quill, at {}", program.display())
                }
                With::System => "Images are opened in this computer's picture program".to_owned(),
            });
        }
        if !open {
            painting.help = false;
        }
    }
}

/// How big a new image is to be.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Size {
    /// As worked out from what it covers.
    Fitting,
    /// So many pixels along its longer side.
    Long(u32),
}

/// The "New image" window: what the image is for, and how big.
struct NewImage {
    part: String,
    /// The face it is for; `None` for the whole part.
    face: Option<String>,
    /// The faces to choose from.
    faces: Vec<String>,
    /// What it would be, or why it can't be made.
    plan: Result<ImagePlan, String>,
    size: Size,
}

impl NewImage {
    fn size(&self, plan: &ImagePlan) -> [u32; 2] {
        match self.size {
            Size::Fitting => plan.size,
            Size::Long(long) => plan.with_long_side(long),
        }
    }
}

impl Draft {
    /// Opens the "New image" window for a part: for one face of it, or
    /// (`None`) for the whole part.
    pub fn open_new(&mut self, studio: &Studio, part: &str, face: Option<&str>) {
        let faces = studio
            .part(part)
            .map(|part| {
                let mut names: Vec<String> = Vec::new();
                for name in &part.face_names {
                    if name != "bevel" && !names.contains(name) {
                        names.push(name.clone());
                    }
                }
                names
            })
            .unwrap_or_default();
        self.new = Some(NewImage {
            part: part.to_owned(),
            face: face.map(str::to_owned),
            faces,
            plan: studio.plan_image(part, face),
            size: Size::Fitting,
        });
        self.failed = None;
    }

    /// Whether the "New image" window is open.
    pub fn is_new_open(&self) -> bool {
        self.new.is_some()
    }

    /// The small picture of an image, made once and kept until its
    /// pixels change.
    fn texture(&mut self, ctx: &egui::Context, picture: &Arc<Picture>) -> TextureHandle {
        // Only the pictures in use are kept: a model has a handful.
        if self.thumbs.len() > 64 {
            self.thumbs.clear();
        }
        self.thumbs
            .entry(picture.stamp)
            .or_insert_with(|| {
                let pixels: Vec<Color32> = picture
                    .pixels
                    .iter()
                    .map(|&[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a))
                    .collect();
                let image =
                    ColorImage::new([picture.width as usize, picture.height as usize], pixels);
                // Sharp pixels, as on the model.
                ctx.load_texture(
                    format!("duck_mold_image_{}", picture.stamp),
                    image,
                    TextureOptions::NEAREST,
                )
            })
            .clone()
    }

    /// Draws an image small, as a square button `side` across. `chosen`
    /// gives it a yellow edge.
    fn thumb(
        &mut self,
        ui: &mut egui::Ui,
        picture: &Arc<Picture>,
        side: f32,
        chosen: bool,
        label: &str,
    ) -> Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
        response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, chosen, label));
        if ui.is_rect_visible(rect) {
            let texture = self.texture(ui.ctx(), picture);
            ui.painter().rect_filled(rect, 3.0, Color32::from_gray(70));
            // The whole image, its proportions kept, in the middle.
            let (wide, high) = (picture.width as f32, picture.height as f32);
            let scale = ((side - 4.0) / wide).min((side - 4.0) / high);
            let inner =
                Rect::from_center_size(rect.center(), egui::vec2(wide * scale, high * scale));
            egui::Image::from_texture(&texture).paint_at(ui, inner);
            let edge = if chosen {
                Stroke::new(2.0, theme::DUCK_YELLOW)
            } else if response.hovered() {
                Stroke::new(1.0, Color32::from_gray(200))
            } else {
                Stroke::new(1.0, Color32::from_gray(100))
            };
            ui.painter()
                .rect_stroke(rect, 3.0, edge, egui::StrokeKind::Inside);
        }
        response
    }
}

/// The square that stands for "no image" in the palette's row: an empty
/// square with a line through it.
fn no_image_square(ui: &mut egui::Ui, side: f32, chosen: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, chosen, "No image"));
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 3.0, Color32::from_gray(45));
        let edge = if chosen {
            Stroke::new(2.0, theme::DUCK_YELLOW)
        } else if response.hovered() {
            Stroke::new(1.0, Color32::from_gray(200))
        } else {
            Stroke::new(1.0, Color32::from_gray(100))
        };
        ui.painter()
            .rect_stroke(rect, 3.0, edge, egui::StrokeKind::Inside);
        let inset = rect.shrink(6.0);
        ui.painter().line_segment(
            [inset.left_bottom(), inset.right_top()],
            Stroke::new(1.5, Color32::from_gray(170)),
        );
    }
    response
}

/// The row of the model's images under the palette: click one to paint
/// with it, the first square to take images off, `+` for a new one.
pub fn palette_row(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    let images = studio.images();
    let edits = !studio.look_only();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Images").strong());
        ui.label(
            RichText::new(match images.len() {
                1 => "1 image".to_owned(),
                count => format!("{count} images"),
            })
            .weak()
            .small(),
        );
        if edits {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let selected = studio.selected().map(str::to_owned);
                let button = ui.add_enabled(selected.is_some(), egui::Button::new("New image"));
                let button = button
                    .on_hover_text("Make a new image for the selected part, to paint on")
                    .on_disabled_hover_text("Select a part first: an image is made for a part");
                if button.clicked()
                    && let Some(part) = selected
                {
                    draft.open_new(studio, &part, None);
                }
            });
        }
    });
    ui.separator();
    if edits {
        ui.label(
            RichText::new(
                "Click an image to paint with it, then click a face. The first square takes a face's image off.",
            )
            .weak()
            .small(),
        );
    } else if images.is_empty() {
        ui.label(RichText::new("No images.").weak());
    }
    let in_hand = (studio.tool == Tool::Paint)
        .then(|| studio.brush())
        .flatten();
    let side = 36.0;
    ui.horizontal_wrapped(|ui| {
        if edits {
            let chosen = in_hand == Some(Brush::NoImage);
            let square = no_image_square(ui, side, chosen)
                .on_hover_text("No image: click a face to take its image off");
            if square.clicked() {
                if chosen {
                    studio.drop_image();
                    studio.tool = Tool::Move;
                } else {
                    studio.pick_image(None);
                }
            }
        }
        for image in &images {
            let chosen = in_hand == Some(Brush::Image(image.file.clone()));
            let square = draft
                .thumb(ui, &image.picture, side, chosen, image.name())
                .on_hover_text(format!(
                    "{} ({} x {} pixels){}",
                    image.file,
                    image.picture.width,
                    image.picture.height,
                    if edits { ": paint with it" } else { "" }
                ));
            if edits && square.clicked() {
                if chosen {
                    // Again puts it down.
                    studio.drop_image();
                    studio.tool = Tool::Move;
                } else {
                    studio.pick_image(Some(&image.file));
                }
            }
        }
    });
    if let Some(why) = &draft.failed {
        ui.add_space(4.0);
        ui.colored_label(theme::BAD, why);
    }
}

/// Asks which PNG to use.
fn pick_png() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Choose an image")
        .add_filter("PNG images", &["png"])
        .pick_file()
}

/// One line of a part's images: what it is on, the image, and the
/// buttons to choose another or take it off. Gives what was asked.
enum Asked {
    Nothing,
    Paint,
    Choose,
    TakeOff,
}

fn image_line(
    ui: &mut egui::Ui,
    studio: &Studio,
    draft: &mut Draft,
    what: &str,
    file: &str,
) -> Asked {
    let mut asked = Asked::Nothing;
    ui.horizontal(|ui| {
        ui.label(RichText::new(what).weak());
        let found = (file != NO_IMAGE)
            .then(|| studio.images().into_iter().find(|image| image.file == file))
            .flatten();
        if let Some(image) = &found {
            draft.thumb(ui, &image.picture, 28.0, false, &format!("{what} image"));
        }
        // The buttons at the right end; the file's name takes what room
        // is left, cut short with "..." if it doesn't fit.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::icons::quiet_button(
                ui,
                crate::icons::Icon::Cross,
                &format!("Take the image off {what}"),
                true,
            )
            .on_hover_text("Take this image off")
            .clicked()
            {
                asked = Asked::TakeOff;
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if file == NO_IMAGE {
                    ui.add(egui::Label::new("none").truncate()).on_hover_text(
                        "\"none\" is written here, so no image is shown on it: not the part's own, nor one its file brought",
                    );
                } else {
                    let name = file.rsplit('/').next().unwrap_or(file);
                    let tip = match &found {
                        Some(image) => format!(
                            "{file}: {} x {} pixels",
                            image.picture.width, image.picture.height
                        ),
                        None => file.to_owned(),
                    };
                    ui.add(egui::Label::new(name).truncate()).on_hover_text(tip);
                }
            });
        });
    });
    // Under it: paint it, or use another PNG.
    if file != NO_IMAGE {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let words = draft.painting.words();
            let button = ui.small_button(words);
            button.widget_info(|| {
                WidgetInfo::labeled(egui::Role::Button, true, format!("{words}: {what}"))
            });
            if button
                .on_hover_text(
                    "Opens this image in the painting program. Save there, and the model shows it at once",
                )
                .clicked()
            {
                asked = Asked::Paint;
            }
            let choose = ui.small_button("Choose...");
            choose.widget_info(|| {
                WidgetInfo::labeled(egui::Role::Button, true, format!("Choose another image for {what}"))
            });
            if choose.on_hover_text("Use another PNG here").clicked() {
                asked = Asked::Choose;
            }
        });
    }
    asked
}

/// A part's images in its properties: the whole part's and each face's,
/// with buttons to choose another PNG, take one off, or make a new one.
pub fn part_section(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    draft: &mut Draft,
    path: &str,
    facts: &Facts,
) {
    ui.add_space(8.0);
    ui.label(RichText::new("Images").strong()).on_hover_text(
        "A picture (a PNG file) on the whole part, or on single faces. It lies over the colour: the colour shows where the picture is see-through",
    );
    let mut done: Option<Result<String, String>> = None;
    // A part made from a mesh file can have pictures the file brought.
    // Taking the recipe's image off such a part shows them again.
    let own = studio.own_images(path, facts);
    let from_mesh = studio
        .model()
        .and_then(|model| model.part(path))
        .is_some_and(|part| part.kind.shape == "mesh");
    let off = from_mesh.then_some(OWN_IMAGE);
    if let Some(file) = &facts.image {
        match image_line(ui, studio, draft, "whole part", file) {
            Asked::Choose => {
                if let Some(chosen) = pick_png() {
                    done = Some(studio.use_image(path, None, &chosen));
                }
            }
            Asked::TakeOff => done = Some(studio.set_image(path, None, off)),
            Asked::Paint => draft.painting.paint(studio, path, None),
            Asked::Nothing => {}
        }
    }
    for (face, file) in &facts.face_images {
        match image_line(ui, studio, draft, face, file) {
            Asked::Choose => {
                if let Some(chosen) = pick_png() {
                    done = Some(studio.use_image(path, Some(face), &chosen));
                }
            }
            Asked::TakeOff => done = Some(studio.set_image(path, Some(face), off)),
            Asked::Paint => draft.painting.paint(studio, path, Some(face)),
            Asked::Nothing => {}
        }
    }
    for image in &own {
        match own_line(ui, draft, image) {
            Asked::Choose => {
                if let Some(chosen) = pick_png() {
                    done = Some(studio.use_image(path, Some(&image.face), &chosen));
                }
            }
            Asked::TakeOff => done = Some(studio.set_image(path, Some(&image.face), None)),
            Asked::Paint | Asked::Nothing => {}
        }
    }
    if facts.image.is_none() && facts.face_images.is_empty() && own.is_empty() {
        ui.label(RichText::new("None yet.").weak());
    }
    ui.horizontal(|ui| {
        if ui
            .button("New image...")
            .on_hover_text("Make a new image for this part, or for one face of it, to paint on")
            .clicked()
        {
            draft.open_new(studio, path, None);
        }
    });
    match done {
        Some(Ok(_)) => draft.failed = None,
        Some(Err(why)) => draft.failed = Some(why),
        None => {}
    }
    if let Some(why) = &draft.failed {
        ui.colored_label(theme::BAD, why);
    }
}

/// One line for a picture the part's mesh file brought with it: the
/// material, the picture, and buttons to use another or take it off.
fn own_line(ui: &mut egui::Ui, draft: &mut Draft, image: &OwnImage) -> Asked {
    let mut asked = Asked::Nothing;
    let what = image.face.as_str();
    ui.horizontal(|ui| {
        ui.label(RichText::new(what).weak());
        draft.thumb(ui, &image.picture, 28.0, false, &format!("{what} image"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::icons::quiet_button(
                ui,
                crate::icons::Icon::Cross,
                &format!("Take the image off {what}"),
                true,
            )
            .on_hover_text("Take this picture off: the material shows its colour")
            .clicked()
            {
                asked = Asked::TakeOff;
            }
            let choose = ui.small_button("Choose...");
            choose.widget_info(|| {
                WidgetInfo::labeled(
                    egui::Role::Button,
                    true,
                    format!("Choose another image for {what}"),
                )
            });
            if choose
                .on_hover_text("Use a PNG of your own here, laid on the way the file lays its own")
                .clicked()
            {
                asked = Asked::Choose;
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new("from the file").truncate())
                    .on_hover_text(format!(
                        "{}: {} x {} pixels. It came with the part's file",
                        image.picture.file, image.picture.width, image.picture.height
                    ));
            });
        });
    });
    asked
}

/// The "New image" window, when it is open.
pub fn windows(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    let Some(new) = &mut draft.new else {
        return;
    };
    let mut close = false;
    let mut open = true;
    let mut made: Option<Result<String, String>> = None;
    // Whether the new image is to be opened in the painting program.
    let mut paint_after = false;
    let mut replan = false;
    let title = match &new.face {
        Some(face) => format!("New image for {face} of \"{}\"", new.part),
        None => format!("New image for \"{}\"", new.part),
    };
    egui::Window::new("New image")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            ui.label(RichText::new(title).strong());
            ui.add_space(4.0);
            // ---- what it is for
            ui.horizontal(|ui| {
                let label = ui.label("For");
                let now = new.face.clone();
                egui::ComboBox::from_id_salt("duck_mold_new_image_for")
                    .selected_text(now.as_deref().unwrap_or("the whole part"))
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(now.is_none(), "the whole part")
                            .clicked()
                            && now.is_some()
                        {
                            new.face = None;
                            replan = true;
                        }
                        for face in &new.faces {
                            if ui
                                .selectable_label(now.as_deref() == Some(face), face)
                                .clicked()
                                && now.as_deref() != Some(face)
                            {
                                new.face = Some(face.clone());
                                replan = true;
                            }
                        }
                    })
                    .response
                    .labelled_by(label.id);
            });
            if replan {
                new.plan = studio.plan_image(&new.part, new.face.as_deref());
                new.size = Size::Fitting;
            }
            match new.plan.clone() {
                Err(why) => {
                    ui.colored_label(theme::BAD, why);
                    ui.add_space(4.0);
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                }
                Ok(plan) => {
                    egui::Grid::new("duck_mold_new_image")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("File").weak());
                            ui.label(&plan.file)
                                .on_hover_text("Beside the recipe: any picture program can open it");
                            ui.end_row();
                            ui.label(RichText::new("Covers").weak());
                            ui.label(format!(
                                "{:.2} x {:.2} m on the model",
                                plan.metres[0], plan.metres[1]
                            ));
                            ui.end_row();
                            ui.label(RichText::new("Size").weak());
                            ui.vertical(|ui| {
                                let fitting = match plan.like_the_others {
                                    Some(per_metre) => format!(
                                        "{} x {}: pixels the size of the model's other images ({} to the metre)",
                                        plan.size[0],
                                        plan.size[1],
                                        per_metre.round()
                                    ),
                                    None => format!(
                                        "{} x {}: a good size for this",
                                        plan.size[0], plan.size[1]
                                    ),
                                };
                                ui.radio_value(&mut new.size, Size::Fitting, fitting);
                                for (name, long) in
                                    [("Small", 16), ("Medium", 32), ("Large", 64), ("Fine", 128)]
                                {
                                    let [wide, high] = plan.with_long_side(long);
                                    ui.radio_value(
                                        &mut new.size,
                                        Size::Long(long),
                                        format!("{name}: {wide} x {high}"),
                                    );
                                }
                            });
                            ui.end_row();
                        });
                    ui.label(
                        RichText::new(
                            "It starts see-through all over: the part's colour shows until it is painted.",
                        )
                        .weak()
                        .small(),
                    );
                    ui.add_space(6.0);
                    let size = new.size(&plan);
                    let paint_words = match draft.painting.words() {
                        "Paint in Duck Quill" => "Make it and paint in Duck Quill",
                        _ => "Make it and open it in my picture program",
                    };
                    if ui.add(theme::primary_button(paint_words)).clicked() {
                        made = Some(studio.new_image(&plan, size));
                        paint_after = made.as_ref().is_some_and(Result::is_ok);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Just make it").clicked() {
                            made = Some(studio.new_image(&plan, size));
                        }
                        if ui
                            .button("Choose a PNG I already have...")
                            .on_hover_text("Use a picture that is already on this computer instead")
                            .clicked()
                            && let Some(chosen) = pick_png()
                        {
                            made = Some(studio.use_image(&new.part, new.face.as_deref(), &chosen));
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                }
            }
            if let Some(why) = &draft.failed {
                ui.colored_label(theme::BAD, why);
            }
        });
    let painted = paint_after.then(|| (new.part.clone(), new.face.clone()));
    match made {
        Some(Ok(_)) => {
            draft.failed = None;
            close = true;
        }
        Some(Err(why)) => draft.failed = Some(why),
        None => {}
    }
    if close || !open {
        draft.new = None;
    }
    if let Some((part, face)) = painted {
        draft.painting.paint(studio, &part, face.as_deref());
    }
}
