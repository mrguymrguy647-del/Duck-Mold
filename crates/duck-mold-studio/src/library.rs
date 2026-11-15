//! The parts library on the screen: the files of the library's folder,
//! each with its small picture.
//!
//! What is in the library, and its pictures, come from
//! [`Studio::library`]. This shows them: in a panel under the parts
//! list, and in the "add a part here" list of the viewport, where
//! choosing one is an `add --file` edit.

use std::collections::HashMap;

use duck_mold_view::studio::Studio;
use duck_mold_view::studio::library::Item;
use duck_mold_view::studio::shelf::{self, Shelved};
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};

/// Which list the library panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shown {
    /// The parts library.
    #[default]
    Parts,
    /// The animation library.
    Animations,
}

/// The library's small pictures, as the screen draws them, and what the
/// panel shows.
#[derive(Default)]
pub struct Thumbs {
    /// Which list the panel shows.
    pub shown: Shown,
    /// The animation the panel's Save button saves, if one was picked
    /// there, and the timeline's animation when it was.
    pub to_save: Option<(String, Option<String>)>,
    /// Which reading of the library they were made from.
    reads: usize,
    textures: HashMap<String, TextureHandle>,
    /// The Add menu's pictures of the shapes.
    pub shapes: crate::gallery::ShapePictures,
}

impl Thumbs {
    /// The picture of an item, made the first time it is asked for.
    fn texture(
        &mut self,
        ctx: &egui::Context,
        studio: &Studio,
        item: &Item,
    ) -> Option<&TextureHandle> {
        if self.reads != studio.library_reads() {
            self.textures.clear();
            self.reads = studio.library_reads();
        }
        let picture = item.picture.as_ref()?;
        Some(
            self.textures
                .entry(item.written.clone())
                .or_insert_with(|| {
                    let image = ColorImage::new(
                        [picture.width, picture.height],
                        picture
                            .pixels
                            .iter()
                            .map(|pixel| {
                                Color32::from_rgb(
                                    (pixel >> 16) as u8,
                                    (pixel >> 8) as u8,
                                    *pixel as u8,
                                )
                            })
                            .collect(),
                    );
                    ctx.load_texture(
                        format!("duck_mold_library_{}", item.written),
                        image,
                        TextureOptions::LINEAR,
                    )
                }),
        )
    }
}

/// A button for one library item: its picture and its name. A file that
/// can't be built is shown without a picture, and can't be chosen; its
/// tip says what is wrong with it.
pub fn item_button(
    ui: &mut egui::Ui,
    studio: &Studio,
    thumbs: &mut Thumbs,
    item: &Item,
    side: f32,
) -> egui::Response {
    let ctx = ui.ctx().clone();
    match (thumbs.texture(&ctx, studio, item), &item.problem) {
        (Some(texture), _) => {
            let image =
                egui::Image::from_texture(texture).fit_to_exact_size(egui::vec2(side, side));
            ui.add(egui::Button::image_and_text(image, &item.name))
                .on_hover_text(&item.written)
        }
        (None, problem) => ui
            .add_enabled(false, egui::Button::new(&item.name))
            .on_disabled_hover_text(format!(
                "{} can't be used: {}",
                item.written,
                problem.as_deref().unwrap_or("it can't be read")
            )),
    }
}

/// The library panel: what is in the folder, and a button to choose
/// another folder.
pub fn panel(ui: &mut egui::Ui, studio: &mut Studio, thumbs: &mut Thumbs) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut thumbs.shown,
            Shown::Parts,
            RichText::new("Library").strong(),
        )
        .on_hover_text("The parts library: recipes and model files to add to a socket");
        ui.selectable_value(
            &mut thumbs.shown,
            Shown::Animations,
            RichText::new("Animations").strong(),
        )
        .on_hover_text("The animation library: starters, and animations saved from other models");
        if thumbs.shown == Shown::Parts {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Folder")
                    .on_hover_text("Choose the folder of parts to show here")
                    .clicked()
                    && let Some(folder) = rfd::FileDialog::new()
                        .set_title("The folder of parts")
                        .pick_folder()
                {
                    studio.set_library_folder(Some(folder));
                }
            });
        }
    });
    if thumbs.shown == Shown::Animations {
        animations(ui, studio, thumbs);
        return;
    }
    let folder = studio.library_folder();
    ui.label(
        RichText::new(folder.file_name().map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ))
        .weak()
        .small(),
    )
    .on_hover_text(folder.display().to_string());
    ui.separator();
    let items = studio.library().to_vec();
    egui::ScrollArea::vertical()
        .id_salt("duck_mold_library_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if items.is_empty() {
                ui.label(
                    RichText::new(
                        "No parts here yet. Put recipes or model files (.obj, .gltf, .glb) in this folder.",
                    )
                    .weak(),
                );
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for item in &items {
                    // Looking at them is all the panel does: they are
                    // chosen in the list a click on a socket opens.
                    let _ = item_button(ui, studio, thumbs, item, 40.0);
                }
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new("To add one: click a socket in the picture, then choose it.")
                    .weak()
                    .small(),
            );
        });
}

/// The animation library: the starters, the animations saved in the
/// folder `animations` beside the recipe, and a button to save one of
/// the model's own there.
fn animations(ui: &mut egui::Ui, studio: &mut Studio, thumbs: &mut Thumbs) {
    ui.label(
        RichText::new("Add copies one into the model, with its poses.")
            .weak()
            .small(),
    );
    ui.separator();
    let shelf = studio.animation_library();
    let own = studio.animation_names();
    egui::ScrollArea::vertical()
        .id_salt("duck_mold_animations_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (starters, saved): (Vec<_>, Vec<_>) = shelf
                .iter()
                .partition(|shelved| shelved.source == "starter");
            ui.label(RichText::new("Starters").small().strong());
            for shelved in starters {
                shelf_row(ui, studio, shelved);
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Saved, in {}/", shelf::FOLDER))
                    .small()
                    .strong(),
            );
            if saved.is_empty() {
                ui.label(
                    RichText::new("None yet. Save one of this model's below; another model can then take it.")
                        .weak()
                        .small(),
                );
            }
            for shelved in saved {
                shelf_row(ui, studio, shelved);
            }
            if own.is_empty() || studio.look_only() {
                return;
            }
            ui.add_space(4.0);
            ui.separator();
            let timeline = studio.animation().map(str::to_owned);
            // A pick here holds until the timeline moves to another.
            if thumbs
                .to_save
                .as_ref()
                .is_some_and(|(_, then)| *then != timeline)
            {
                thumbs.to_save = None;
            }
            let chosen = thumbs
                .to_save
                .as_ref()
                .map(|(name, _)| name.clone())
                .filter(|name| own.contains(name))
                .or_else(|| studio.animation().map(str::to_owned))
                .unwrap_or_else(|| own[0].clone());
            ui.horizontal(|ui| {
                let mut picked = chosen.clone();
                egui::ComboBox::from_id_salt("duck_mold_save_animation")
                    .selected_text(&picked)
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for name in &own {
                            ui.selectable_value(&mut picked, name.clone(), name);
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::ComboBox,
                            true,
                            "Animation to save",
                        )
                    });
                // Only a choice made here is kept: otherwise it follows
                // the timeline's animation.
                if picked != chosen {
                    thumbs.to_save = Some((picked.clone(), timeline.clone()));
                }
                let save = ui.button("Save");
                save.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, "Save the animation")
                });
                if save
                    .on_hover_text(format!(
                        "Write \"{picked}\" and its poses to {}/{picked}.toml (duck-mold save-animation)",
                        shelf::FOLDER
                    ))
                    .clicked()
                {
                    let _ = studio.save_to_animation_library(&picked);
                }
            });
        });
}

/// One animation of the library: its name and an Add button, with what
/// it is and what it does below.
fn shelf_row(ui: &mut egui::Ui, studio: &mut Studio, shelved: &Shelved) {
    let missing = studio.missing_parts(shelved);
    let none_fit = !shelved.parts.is_empty() && missing.len() == shelved.parts.len();
    ui.horizontal(|ui| {
        ui.label(RichText::new(&shelved.name).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let why_not = if let Some(problem) = &shelved.problem {
                Some(format!("{} can't be used: {problem}", shelved.source))
            } else if none_fit {
                Some(format!(
                    "This model has none of the parts it moves: {}. Name parts so to use it",
                    shelved.parts.join(", ")
                ))
            } else if studio.look_only() {
                Some("This window is only for looking".to_owned())
            } else {
                None
            };
            let name = format!("Add {}", shelved.name);
            let button = ui.add_enabled(why_not.is_none(), egui::Button::new("Add").small());
            button.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, why_not.is_none(), &name)
            });
            match why_not {
                Some(why) => {
                    button.on_disabled_hover_text(why);
                }
                None => {
                    let tip = if missing.is_empty() {
                        format!("Copy \"{}\" into the model, with its poses", shelved.name)
                    } else {
                        format!(
                            "Copy \"{}\" into the model, leaving out what it does to {}, which it doesn't have",
                            shelved.name,
                            missing.join(", ")
                        )
                    };
                    if button.on_hover_text(tip).clicked() {
                        let _ = studio.add_from_library(shelved);
                    }
                }
            }
        });
    });
    let mut about = shelved.about.clone();
    if shelved.source != "starter" {
        about = format!("{} {about}", shelved.source);
    }
    if !missing.is_empty() && !none_fit {
        about.push_str(&format!(" Not here: {}.", missing.join(", ")));
    }
    ui.label(RichText::new(about.trim()).weak().small());
}
