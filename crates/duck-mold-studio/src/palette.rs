//! The palette panel: the recipe's colours, to pick one to paint with,
//! change one, add one, or remove one.
//!
//! Each of those is an edit: `set-color`, `add-color`, `remove-color`.

use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::Tool;
use eframe::egui::{self, RichText, TextEdit};

use crate::theme;

/// What is being typed or picked in the palette panel.
#[derive(Default)]
pub struct Draft {
    /// The "new colour" form, when it is open: its name and colour.
    new: Option<(String, [u8; 3])>,
    /// A colour being changed with the colour picker: which, and to
    /// what, until the mouse is let go.
    recolor: Option<(String, [u8; 3])>,
    /// A colour that is in use and is to be removed, and the colour to
    /// paint with instead, while the panel asks.
    removing: Option<(String, String)>,
    /// Why the last change wasn't made.
    failed: Option<String>,
}

/// Draws the palette, and makes the edit a click asks for.
pub fn panel(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    ui.add_space(4.0);
    let palette = studio.palette();
    let edits = !studio.look_only();
    let mut done: Option<Result<String, String>> = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Palette").strong());
        ui.label(
            RichText::new(format!("{} colours", palette.len()))
                .weak()
                .small(),
        );
        if edits {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("New colour")
                    .on_hover_text("Add a colour to the palette")
                    .clicked()
                {
                    draft.new = Some((String::new(), [0xFF, 0xCC, 0x2E]));
                }
            });
        }
    });
    ui.separator();
    if edits {
        ui.label(
            RichText::new(
                "Click a colour to paint with it, then click a face. Shift+click paints the whole part.",
            )
            .weak()
            .small(),
        );
    }
    let in_hand = (studio.tool == Tool::Paint)
        .then(|| studio.paint_colour())
        .flatten();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if palette.is_empty() {
                ui.label(RichText::new("No colours yet.").weak());
            }
            for (name, rgb) in &palette {
                ui.horizontal(|ui| {
                    // The colour itself: a click opens the picker, and
                    // letting go changes it everywhere it is used.
                    let mut picked = match &draft.recolor {
                        Some((changing, to)) if changing == name => *to,
                        _ => *rgb,
                    };
                    ui.add_enabled_ui(edits, |ui| {
                        let square = ui
                            .color_edit_button_srgb(&mut picked)
                            .on_hover_text(format!(
                                "Change the colour \"{name}\" itself, everywhere it is used"
                            ));
                        if square.changed() {
                            draft.recolor = Some((name.clone(), picked));
                        }
                    });
                    if let Some((changing, to)) = &draft.recolor
                        && changing == name
                        && !ui.input(|input| input.pointer.any_down())
                    {
                        if to != rgb {
                            done = Some(studio.change_colour(name, *to));
                        }
                        draft.recolor = None;
                    }
                    // Its name: a click picks it to paint with.
                    let chosen = in_hand.as_deref() == Some(name.as_str());
                    let label = ui
                        .add_enabled(edits, egui::Button::selectable(chosen, name))
                        .on_hover_text(format!(
                            "Paint with \"{name}\" (#{:02X}{:02X}{:02X})",
                            rgb[0], rgb[1], rgb[2]
                        ));
                    if label.clicked() {
                        if chosen {
                            // Again puts the brush down.
                            studio.pick_colour(None);
                            studio.tool = Tool::Move;
                        } else {
                            studio.pick_colour(Some(name));
                        }
                    }
                    if edits {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let remove = egui::Button::new(RichText::new("remove").small().weak())
                                .frame(false);
                            if ui
                                .add(remove)
                                .on_hover_text(format!("Take \"{name}\" out of the palette"))
                                .clicked()
                            {
                                if studio.colour_in_use(name) {
                                    // Ask what to paint with instead.
                                    let other = palette
                                        .iter()
                                        .map(|(other, _)| other)
                                        .find(|other| *other != name)
                                        .cloned()
                                        .unwrap_or_default();
                                    draft.removing = Some((name.clone(), other));
                                } else {
                                    done = Some(studio.remove_colour(name, None));
                                }
                            }
                        });
                    }
                });
            }

            if let Some(why) = &draft.failed {
                ui.add_space(4.0);
                ui.colored_label(theme::BAD, why);
            }
        });

    // ---- a colour in use, to be removed: asked in a window of its own
    if let Some((name, other)) = &mut draft.removing {
        let mut close = false;
        let mut open = true;
        egui::Window::new("Remove a colour")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let others: Vec<&String> = palette
                    .iter()
                    .map(|(known, _)| known)
                    .filter(|known| *known != name)
                    .collect();
                if others.is_empty() {
                    ui.label(format!(
                        "\"{name}\" is in use, and it is the only colour. Add another first."
                    ));
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                    return;
                }
                ui.label(format!(
                    "\"{name}\" is in use. What it paints gets another colour:"
                ));
                ui.horizontal(|ui| {
                    let label = ui.label("Paint with");
                    egui::ComboBox::from_id_salt("duck_mold_replace_colour")
                        .selected_text(other.as_str())
                        .show_ui(ui, |ui| {
                            for known in &others {
                                ui.selectable_value(other, (*known).clone(), known.as_str());
                            }
                        })
                        .response
                        .labelled_by(label.id);
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Repaint and remove").clicked() {
                        done = Some(studio.remove_colour(name, Some(other)));
                        close = true;
                    }
                    if ui.button("Keep it").clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
            draft.removing = None;
        }
    }

    // ---- a new colour: also in a window
    if let Some((name, rgb)) = &mut draft.new {
        let mut close = false;
        let mut open = true;
        egui::Window::new("New colour")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    let label = ui.label("Called");
                    ui.add(
                        TextEdit::singleline(name)
                            .desired_width(120.0)
                            .hint_text("a name"),
                    )
                    .labelled_by(label.id);
                    ui.color_edit_button_srgb(rgb);
                });
                ui.label(
                    RichText::new(
                        "Letters, digits, _ and - only. Click the square to choose the colour.",
                    )
                    .weak()
                    .small(),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!name.is_empty(), theme::primary_button("Add to palette"))
                        .clicked()
                    {
                        done = Some(studio.add_colour(name, *rgb));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if close || !open {
            draft.new = None;
        }
    }

    match done {
        Some(Ok(_)) => draft.failed = None,
        Some(Err(why)) => draft.failed = Some(why),
        None => {}
    }
}
