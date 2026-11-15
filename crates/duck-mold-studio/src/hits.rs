//! Hit shapes in the properties panel: the selected part's own, and the
//! model's line when nothing is selected.
//!
//! A game asks a model which part a click or a bullet found by testing
//! each part's hit shape: a box, a ball or a capsule round it. Each
//! change here is one `set-collider` (a part's) or `set-colliders` (the
//! model's) edit, the same as `duck-mold edit` makes.

use duck_mold_core::edit::{ColliderTo, Edit};
use duck_mold_core::hit::fitted_numbers;
use duck_mold_core::model::Model;
use duck_mold_core::recipe::{ColliderDef, Colliders, HitKind};
use duck_mold_view::studio::Studio;
use eframe::egui::{self, DragValue, RichText};

/// The numbers of a part's hit shape, as they are being typed.
#[derive(Default)]
pub struct HitDraft {
    /// The part these were read for, and which build of the file.
    of: Option<(String, usize)>,
    center: [f64; 3],
    size: [f64; 3],
    radius: f64,
    length: f64,
}

/// The five choices for a part, in the list's order.
#[derive(Clone, Copy, PartialEq)]
enum Choice {
    /// No hit shape of its own: what the model's line gives it.
    Follow,
    /// This shape, fitted to it or with numbers of its own.
    Shape(HitKind),
    /// No hit shape at all.
    None,
}

/// How the model's line is said in the list.
const WHOLE: [(&str, Option<Colliders>); 5] = [
    ("none", None),
    ("each part", Some(Colliders::Parts)),
    ("one capsule", Some(Colliders::Whole(HitKind::Capsule))),
    ("one box", Some(Colliders::Whole(HitKind::Box))),
    ("one ball", Some(Colliders::Whole(HitKind::Sphere))),
];

fn word_for(setting: Option<Colliders>) -> &'static str {
    WHOLE
        .iter()
        .find(|(_, whole)| *whole == setting)
        .map_or("none", |(word, _)| word)
}

/// The model's line: whether it has hit shapes, and which. Gives the
/// `set-colliders` edit when it is changed.
pub fn model_line(ui: &mut egui::Ui, studio: &Studio) -> Option<Edit> {
    let model = studio.model()?;
    let now = model.hit_setting;
    let mut chosen = now;
    ui.add_space(8.0);
    let label = ui
        .label(RichText::new("Hit shapes").strong())
        .on_hover_text(
            "The shapes a game tests to know which part a click or a bullet found. \
             They aren't drawn: switch on Hits to see them",
        );
    ui.add_enabled_ui(!studio.look_only(), |ui| {
        egui::ComboBox::from_id_salt("duck_mold_model_hit_shapes")
            .selected_text(word_for(now))
            .show_ui(ui, |ui| {
                for (word, setting) in WHOLE {
                    ui.selectable_value(&mut chosen, setting, word);
                }
            })
            .response
            .labelled_by(label.id);
    });
    ui.label(RichText::new(summary(model)).weak().small());
    (chosen != now).then_some(Edit::SetColliders { colliders: chosen })
}

/// What the model has, in a few words.
fn summary(model: &Model) -> String {
    let count = model.colliders.len();
    match model.hit_setting {
        None if count == 0 => "None: a game can't tell which part was hit.".to_owned(),
        None => format!(
            "{count} of the parts that say their own. Choose \"each part\" to give every part one."
        ),
        Some(Colliders::Parts) => {
            let none = model.without_collider.len();
            if none == 0 {
                format!("{count}, one for each part.")
            } else {
                format!("{count}, one for each part but {none} that say none.")
            }
        }
        Some(Colliders::Whole(kind)) => format!(
            "One {} round the whole model{}.",
            kind.name(),
            if count > 1 {
                ", and the parts that say their own"
            } else {
                ""
            }
        ),
    }
}

/// The selected part's hit shape: its list, and for a shape of its own,
/// its numbers and Fit again. Gives the `set-collider` edit to make.
pub fn part_section(
    ui: &mut egui::Ui,
    studio: &Studio,
    path: &str,
    draft: &mut HitDraft,
) -> Option<Edit> {
    let model = studio.model()?;
    let part = model.part(path)?;
    let built = model
        .colliders
        .iter()
        .find(|collider| collider.part == path);
    let now = match &part.collider {
        ColliderDef::Unset => Choice::Follow,
        ColliderDef::None => Choice::None,
        ColliderDef::Fitted(kind) => Choice::Shape(*kind),
        ColliderDef::Own(own) => Choice::Shape(own.kind),
    };
    // The numbers to show: its own, or those it is fitted with.
    let numbers = match &part.collider {
        ColliderDef::Own(own) => Some(*own),
        ColliderDef::Fitted(kind) => fitted_numbers(model, part, *kind, None),
        _ => None,
    };
    let key = (path.to_owned(), studio.builds());
    if draft.of.as_ref() != Some(&key) {
        if let Some(numbers) = &numbers {
            draft.center = numbers.center;
            draft.size = numbers.size;
            draft.radius = numbers.radius;
            draft.length = numbers.length;
        }
        draft.of = Some(key);
    }

    let follow_words = match (model.hit_setting, built) {
        (Some(Colliders::Parts), Some(collider)) => {
            format!("fitted: {}", collider.shape.kind().name())
        }
        (Some(Colliders::Parts), None) => "fitted: none".to_owned(),
        (Some(Colliders::Whole(_)), _) => "the model's".to_owned(),
        (None, _) => "none (the model's)".to_owned(),
    };
    let word = |choice: Choice| match choice {
        Choice::Follow => follow_words.clone(),
        Choice::Shape(kind) => kind.name().to_owned(),
        Choice::None => "none".to_owned(),
    };

    let mut edit = None;
    ui.add_space(8.0);
    let label = ui
        .label(RichText::new("Hit shape").strong())
        .on_hover_text("The shape a game tests to know a click or a bullet found this part");
    let mut chosen = now;
    egui::ComboBox::from_id_salt("duck_mold_part_hit_shape")
        .selected_text(word(now))
        .show_ui(ui, |ui| {
            for choice in [
                Choice::Follow,
                Choice::Shape(HitKind::Box),
                Choice::Shape(HitKind::Sphere),
                Choice::Shape(HitKind::Capsule),
                Choice::None,
            ] {
                ui.selectable_value(&mut chosen, choice, word(choice));
            }
        })
        .response
        .labelled_by(label.id);
    if chosen != now {
        edit = Some(match chosen {
            Choice::Follow => ColliderTo::Fitted,
            Choice::None => ColliderTo::None,
            Choice::Shape(kind) => fitted(kind),
        });
    }

    // What it has now, in words.
    let said = match (now, built) {
        (Choice::None, _) => "None: a ray goes on to what is behind it.".to_owned(),
        (_, Some(collider)) => collider.shape.describe(),
        (Choice::Follow, None) if model.hit_setting.is_none() => {
            "None: the model's Hit shapes line is none.".to_owned()
        }
        (Choice::Follow, None) => "None: the whole model has one.".to_owned(),
        (_, None) => "None: it has nothing to fit one round.".to_owned(),
    };
    ui.label(RichText::new(said).weak().small());

    // The numbers of a shape it says for itself.
    if let (Choice::Shape(kind), Some(numbers)) = (now, &numbers) {
        let settled = |response: &egui::Response| response.drag_stopped() || response.lost_focus();
        let mut done = false;
        egui::Grid::new("duck_mold_hit_numbers")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                let label = ui.label("Middle").on_hover_text(
                    "Where its middle is, in metres, measured from the part's own middle point",
                );
                ui.horizontal(|ui| {
                    for (at, axis) in ["x", "y", "z"].into_iter().enumerate() {
                        let field = ui
                            .add(
                                DragValue::new(&mut draft.center[at])
                                    .speed(0.01)
                                    .max_decimals(4),
                            )
                            .labelled_by(label.id)
                            .on_hover_text(axis);
                        done |= settled(&field);
                    }
                });
                ui.end_row();
                match kind {
                    HitKind::Box => {
                        let label = ui
                            .label("Box size")
                            .on_hover_text("Width, height and depth, in metres");
                        ui.horizontal(|ui| {
                            for side in &mut draft.size {
                                done |= settled(&ui.add(metres(side)).labelled_by(label.id));
                            }
                        });
                        ui.end_row();
                    }
                    HitKind::Sphere => {
                        let label = ui.label("Ball radius");
                        done |= settled(&ui.add(metres(&mut draft.radius)).labelled_by(label.id));
                        ui.end_row();
                    }
                    HitKind::Capsule => {
                        let label = ui.label("Capsule radius");
                        done |= settled(&ui.add(metres(&mut draft.radius)).labelled_by(label.id));
                        ui.end_row();
                        let label = ui
                            .label("Length")
                            .on_hover_text("From end to end, round ends and all");
                        done |= settled(&ui.add(metres(&mut draft.length)).labelled_by(label.id));
                        ui.end_row();
                    }
                }
            });
        let typed = draft.center != numbers.center
            || match kind {
                HitKind::Box => draft.size != numbers.size,
                HitKind::Sphere => draft.radius != numbers.radius,
                HitKind::Capsule => {
                    draft.radius != numbers.radius || draft.length != numbers.length
                }
            };
        if done && typed {
            edit = Some(ColliderTo::Shape {
                kind,
                size: (kind == HitKind::Box).then_some(draft.size),
                radius: (kind != HitKind::Box).then_some(draft.radius),
                length: (kind == HitKind::Capsule).then_some(draft.length),
                center: Some(draft.center),
                along: numbers.along,
            });
        }
        if matches!(part.collider, ColliderDef::Own(_))
            && ui
                .button("Fit again")
                .on_hover_text("Forget these numbers: fit the shape round the part again")
                .clicked()
        {
            edit = Some(fitted(kind));
        }
    }
    edit.map(|to| Edit::SetCollider {
        part: path.to_owned(),
        to,
    })
}

/// A shape fitted to the part: no numbers of its own.
fn fitted(kind: HitKind) -> ColliderTo {
    ColliderTo::Shape {
        kind,
        size: None,
        radius: None,
        length: None,
        center: None,
        along: None,
    }
}

/// A field for a length in metres.
fn metres(value: &mut f64) -> DragValue<'_> {
    DragValue::new(value)
        .speed(0.01)
        .range(0.001..=1000.0)
        .max_decimals(4)
}
