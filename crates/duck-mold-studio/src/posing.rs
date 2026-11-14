//! The panel in Pose mode, and a part's limits in either mode.
//!
//! In Pose mode the panel lists the model's poses (`rest` and the
//! recipe's), with New, Copy, Mirror, Rename and Remove; under them, the
//! selected part's turn and move in the pose chosen, as numbers to type,
//! "Back to rest", and its limits. Every change is one edit: `add-pose`,
//! `mirror-pose`, `rename-pose`, `remove-pose`, `set-pose`,
//! `set-limits`.

use duck_mold_core::edit::LimitsTo;
use duck_mold_core::recipe::{Limits, REST, Turns};
use duck_mold_view::studio::Studio;
use eframe::egui::{self, DragValue, RichText, TextEdit};

use crate::theme;

/// How a direction may turn, as the panel's list says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Any amount.
    Free,
    /// Between two angles.
    Between,
    /// Not at all.
    No,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Free, Kind::Between, Kind::No];

    fn words(self) -> &'static str {
        match self {
            Kind::Free => "any amount",
            Kind::Between => "between",
            Kind::No => "doesn't turn",
        }
    }
}

/// What is being typed in the Pose panel and the limits, until it
/// becomes an edit.
#[derive(Default)]
pub struct PoseDraft {
    /// The pose, part and build of the file these numbers were read for.
    of: Option<(String, String, usize)>,
    turn: [f64; 3],
    move_by: [f64; 3],
    /// The part and build the limits were read for.
    limits_of: Option<(String, usize)>,
    fixed: bool,
    kinds: [Option<Kind>; 3],
    ranges: [[f64; 2]; 3],
    /// The new name being typed for the pose chosen, while Rename is
    /// open.
    renaming: Option<String>,
    /// The animations whose keys use the pose, while Remove asks whether
    /// to take those keys too.
    confirm_remove: Option<Vec<String>>,
    /// Why the last change wasn't made.
    failed: Option<String>,
}

impl PoseDraft {
    fn fill_limits(&mut self, limits: Limits) {
        self.fixed = limits == Limits::Fixed;
        let turns = match limits {
            Limits::Fixed => [Turns::No; 3],
            other => other.turns(),
        };
        for (at, turns) in turns.into_iter().enumerate() {
            self.kinds[at] = Some(match turns {
                Turns::Any => Kind::Free,
                Turns::No => Kind::No,
                Turns::Between(..) => Kind::Between,
            });
            self.ranges[at] = match turns {
                Turns::Between(low, high) => [low, high],
                _ => [-45.0, 45.0],
            };
        }
    }

    /// The limits as the panel has them now.
    fn limits(&self) -> LimitsTo {
        if self.fixed {
            return LimitsTo::Fixed;
        }
        let kinds = self.kinds.map(|kind| kind.unwrap_or(Kind::Free));
        if kinds.iter().all(|kind| *kind == Kind::Free) {
            return LimitsTo::None;
        }
        LimitsTo::Turns([0, 1, 2].map(|at| {
            Some(match kinds[at] {
                Kind::Free => Turns::Any,
                Kind::No => Turns::No,
                Kind::Between => {
                    let [low, high] = self.ranges[at];
                    Turns::Between(low.min(high), low.max(high))
                }
            })
        }))
    }
}

/// A part's limits, to read and to change: fixed, or for each of x, y
/// and z any amount, between two angles, or not at all. Gives the
/// `set-limits` to make, if one is settled.
pub fn limits_section(
    ui: &mut egui::Ui,
    studio: &Studio,
    path: &str,
    draft: &mut PoseDraft,
    open: bool,
) -> Option<LimitsTo> {
    let limits = studio.limits_of(path)?;
    let key = (path.to_owned(), studio.builds());
    if draft.limits_of.as_ref() != Some(&key) {
        draft.fill_limits(limits);
        draft.limits_of = Some(key);
    }
    let mut settled = false;
    ui.add_space(6.0);
    // Folded in Part mode, where it is two short lines; open in Pose
    // mode.
    let header = egui::CollapsingHeader::new("Limits")
        .id_salt(("duck_mold_limits_header", open))
        .default_open(open)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "How far it may turn about its join when the model is posed, in degrees. A pose past them is a warning.",
                )
                .weak()
                .small(),
            );
            let fixed = ui
                .checkbox(&mut draft.fixed, "Fixed")
                .on_hover_text("It neither turns nor moves in any pose");
            settled |= fixed.changed();
            if draft.fixed {
                return;
            }
            egui::Grid::new(("duck_mold_limits", path))
                .num_columns(4)
                .spacing([6.0, 4.0])
                .show(ui, |ui| {
                    for (at, axis) in ["Around x", "Around y", "Around z"].into_iter().enumerate() {
                        ui.label(axis);
                        let kind = draft.kinds[at].get_or_insert(Kind::Free);
                        egui::ComboBox::from_id_salt(("duck_mold_limit_kind", at))
                            .width(96.0)
                            .selected_text(kind.words())
                            .show_ui(ui, |ui| {
                                for choice in Kind::ALL {
                                    if ui
                                        .selectable_label(*kind == choice, choice.words())
                                        .clicked()
                                        && *kind != choice
                                    {
                                        *kind = choice;
                                        settled = true;
                                    }
                                }
                            });
                        if *kind == Kind::Between {
                            for end in 0..2 {
                                let field = ui.add(
                                    DragValue::new(&mut draft.ranges[at][end])
                                        .speed(1.0)
                                        .range(-36_000.0..=36_000.0)
                                        .suffix("°"),
                                );
                                settled |= field.drag_stopped() || field.lost_focus();
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    // What they are, in words, folded or not.
    header.header_response.on_hover_text(limits_words(limits));
    ui.label(RichText::new(limits_words(limits)).weak().small());
    (settled && !studio.look_only()).then(|| draft.limits())
}

/// What a part's limits say, in words.
fn limits_words(limits: Limits) -> String {
    match limits {
        Limits::Free => "turns any way, any amount".to_owned(),
        Limits::Fixed => "fixed: it doesn't turn or move".to_owned(),
        Limits::Turns(turns) => {
            let said: Vec<String> = turns
                .iter()
                .zip(["x", "y", "z"])
                .filter_map(|(turns, axis)| match turns {
                    Turns::No => None,
                    Turns::Any => Some(format!("{axis} any amount")),
                    Turns::Between(low, high) => Some(format!("{axis} {low}..{high}°")),
                })
                .collect();
            if said.is_empty() {
                "doesn't turn (it may be moved)".to_owned()
            } else {
                format!("turns {}", said.join(", "))
            }
        }
    }
}

/// The panel in Pose mode.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut PoseDraft) {
    ui.add_space(4.0);
    ui.label(RichText::new("Poses").strong());
    ui.separator();
    let mut done: Option<Result<String, String>> = None;
    let shown = studio.pose_shown().map(str::to_owned);
    for name in studio.pose_names() {
        let chosen = shown.as_deref() == Some(name.as_str());
        let words = if name == REST {
            RichText::new("rest").italics()
        } else {
            RichText::new(&name)
        };
        let tip = if name == REST {
            "The model as it is built. A drag here starts a new pose".to_owned()
        } else {
            let keys = studio.keys_using(&name);
            if keys.is_empty() {
                format!("The pose \"{name}\"")
            } else {
                format!("The pose \"{name}\": keys of {} use it", keys.join(", "))
            }
        };
        if ui
            .selectable_label(chosen, words)
            .on_hover_text(tip)
            .clicked()
            && !chosen
        {
            studio.choose_pose(&name);
            draft.renaming = None;
            draft.confirm_remove = None;
        }
    }
    let editable = !studio.look_only();
    let a_pose = studio.pose() != REST;
    ui.add_space(4.0);
    ui.add_enabled_ui(editable, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("New")
                .on_hover_text("A new pose, with every part at rest")
                .clicked()
            {
                done = Some(studio.new_pose());
            }
            if ui
                .add_enabled(a_pose, egui::Button::new("Copy"))
                .on_hover_text("A copy of this pose, to change")
                .clicked()
            {
                done = Some(studio.copy_pose());
            }
            if ui
                .add_enabled(a_pose, egui::Button::new("Mirror"))
                .on_hover_text(
                    "The mirror image, left to right. step_left makes step_right; any other pose is mirrored where it is",
                )
                .clicked()
            {
                done = Some(studio.mirror_pose());
            }
            if ui
                .add_enabled(a_pose, egui::Button::new("Rename"))
                .clicked()
            {
                draft.renaming = Some(studio.pose().to_owned());
            }
            if ui
                .add_enabled(a_pose, egui::Button::new("Remove"))
                .clicked()
            {
                let keys = studio.keys_using(studio.pose());
                if keys.is_empty() {
                    done = Some(studio.remove_pose(false));
                } else {
                    draft.confirm_remove = Some(keys);
                }
            }
        });
    });
    // ---- renaming the pose
    if let Some(name) = &mut draft.renaming {
        let mut close = false;
        ui.horizontal(|ui| {
            let field = ui.add(TextEdit::singleline(name).desired_width(120.0));
            field.request_focus();
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if (ui.button("Rename").clicked() || enter) && !name.is_empty() {
                done = Some(studio.rename_pose(name));
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        if close {
            draft.renaming = None;
        }
    }
    // ---- removing a pose that keys use
    if let Some(animations) = draft.confirm_remove.clone() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(format!(
                "Keys of {} use \"{}\". Remove those keys too?",
                animations.join(", "),
                studio.pose()
            ));
            ui.horizontal(|ui| {
                if ui.button("Remove the pose and its keys").clicked() {
                    done = Some(studio.remove_pose(true));
                    draft.confirm_remove = None;
                }
                if ui.button("Keep it").clicked() {
                    draft.confirm_remove = None;
                }
            });
        });
    }

    // ---- the selected part, in the pose chosen
    ui.add_space(8.0);
    match studio.selected().map(str::to_owned) {
        None => {
            ui.label(RichText::new("Click a part to turn it.").weak());
        }
        Some(path) => {
            ui.label(RichText::new(&path).strong());
            ui.separator();
            let (turn, move_by) = studio.pose_line(&path);
            let key = (studio.pose().to_owned(), path.clone(), studio.builds());
            if draft.of.as_ref() != Some(&key) {
                draft.turn = turn;
                draft.move_by = move_by;
                draft.of = Some(key);
            }
            let mut settled = false;
            ui.add_enabled_ui(editable, |ui| {
                egui::Grid::new("duck_mold_pose_numbers")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Turn").on_hover_text(
                            "Degrees around x, then y, then z, about its join, in this pose",
                        );
                        ui.horizontal(|ui| {
                            for at in 0..3 {
                                let field = ui.add(
                                    DragValue::new(&mut draft.turn[at])
                                        .speed(1.0)
                                        .range(-36_000.0..=36_000.0)
                                        .suffix("°"),
                                );
                                settled |= field.drag_stopped() || field.lost_focus();
                            }
                        });
                        ui.end_row();
                        ui.label("Move").on_hover_text(
                            "How far it is moved from where it is at rest, in metres",
                        );
                        ui.horizontal(|ui| {
                            for at in 0..3 {
                                let field = ui.add(
                                    DragValue::new(&mut draft.move_by[at])
                                        .speed(0.01)
                                        .range(-10_000.0..=10_000.0)
                                        .max_decimals(3),
                                );
                                settled |= field.drag_stopped() || field.lost_focus();
                            }
                        });
                        ui.end_row();
                    });
                if settled && (draft.turn != turn || draft.move_by != move_by) {
                    done = Some(studio.set_pose_numbers(&path, draft.turn, draft.move_by));
                    draft.of = None;
                }
                if ui
                    .add_enabled(
                        a_pose && (turn != [0.0; 3] || move_by != [0.0; 3]),
                        egui::Button::new("Back to rest"),
                    )
                    .on_hover_text("Take this part's line out of the pose")
                    .clicked()
                {
                    done = Some(studio.pose_back_to_rest(&path));
                    draft.of = None;
                }
            });
            if let Some(limits) = limits_section(ui, studio, &path, draft, true) {
                done = Some(studio.set_limits_of(&path, limits));
                draft.limits_of = None;
            }
        }
    }
    match done {
        Some(Ok(_)) => draft.failed = None,
        Some(Err(why)) => draft.failed = Some(why),
        None => {}
    }
    if let Some(why) = &draft.failed {
        ui.add_space(6.0);
        ui.colored_label(theme::BAD, why);
    }
}
