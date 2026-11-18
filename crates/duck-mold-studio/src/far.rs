//! Far-away versions in the properties panel: the model's line when
//! nothing is selected, and a part's "keep whole" box.
//!
//! A game draws a simpler version of a model far off: Far 1 with half
//! the segments, Far 2 with a quarter and the small parts left out. Each
//! change here is one `set-lods` (the model's) or `set-lod` (a part's)
//! edit, the same as `duck-mold edit` makes. View > Far version shows
//! them in the picture.

use duck_mold_core::edit::Edit;
use duck_mold_core::model::Model;
use duck_mold_core::recipe::Lods;
use duck_mold_view::studio::Studio;
use eframe::egui::{self, RichText};

/// The choices of the model's line, in the list's order.
#[derive(Clone, Copy, PartialEq)]
enum Choice {
    None,
    Auto,
    /// Two distances the recipe gives: kept as they are.
    At([f64; 2]),
}

/// The model's line: whether it has far-away versions. Gives the
/// `set-lods` edit when it is changed.
pub fn model_line(ui: &mut egui::Ui, studio: &Studio) -> Option<Edit> {
    let model = studio.model()?;
    let now = match model.lod_setting {
        None => Choice::None,
        Some(Lods::Auto) => Choice::Auto,
        Some(Lods::At(at)) => Choice::At(at),
    };
    let word = |choice: Choice| match choice {
        Choice::None => "none".to_owned(),
        Choice::Auto => "auto".to_owned(),
        Choice::At([near, far]) => format!("at {near} m and {far} m"),
    };
    let mut chosen = now;
    ui.add_space(8.0);
    let label = ui
        .label(RichText::new("Far versions").strong())
        .on_hover_text(
            "Simpler versions a game draws when the model is far off, so a crowd \
             of them stays fast. View > Far version shows them",
        );
    ui.add_enabled_ui(!studio.look_only(), |ui| {
        egui::ComboBox::from_id_salt("duck_mold_model_far_versions")
            .selected_text(word(now))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut chosen, Choice::None, "none");
                ui.selectable_value(&mut chosen, Choice::Auto, "auto");
                if let Choice::At(_) = now {
                    ui.selectable_value(&mut chosen, now, word(now));
                }
            })
            .response
            .labelled_by(label.id);
    });
    ui.label(RichText::new(summary(model)).weak().small());
    (chosen != now).then_some(Edit::SetLods {
        lods: match chosen {
            Choice::None => None,
            Choice::Auto => Some(Lods::Auto),
            Choice::At(at) => Some(Lods::At(at)),
        },
    })
}

/// What the model has, in a few words.
fn summary(model: &Model) -> String {
    if model.far.is_empty() {
        return "None: a game draws every triangle however far off it is.".to_owned();
    }
    let each: Vec<String> = model
        .far
        .iter()
        .enumerate()
        .map(|(at, far)| {
            format!(
                "Far {} from {} m: {} triangles",
                at + 1,
                far.distance,
                far.triangles()
            )
        })
        .collect();
    format!("{}.", each.join(". "))
}

/// A part's box: kept whole in the far-away versions, or simplified
/// like the rest. Only for a model that has them. Gives the `set-lod`
/// edit when it is changed.
pub fn part_line(ui: &mut egui::Ui, studio: &Studio, path: &str) -> Option<Edit> {
    let model = studio.model()?;
    if model.far.is_empty() {
        return None;
    }
    let now = model.part(path)?.lod_keep;
    let mut keep = now;
    ui.add_space(4.0);
    ui.add_enabled_ui(!studio.look_only(), |ui| {
        ui.checkbox(&mut keep, "Keep whole when far").on_hover_text(
            "Never simplified or left out of the far-away versions: for small \
                 parts that matter, like a character's eyes",
        );
    });
    (keep != now).then(|| Edit::SetLod {
        part: path.to_owned(),
        keep,
    })
}
