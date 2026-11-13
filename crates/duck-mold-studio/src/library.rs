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
use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};

/// The library's small pictures, as the screen draws them.
#[derive(Default)]
pub struct Thumbs {
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
        ui.label(RichText::new("Library").strong());
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
    });
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
