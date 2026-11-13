//! The properties panel: what the recipe says about the selected part,
//! to read and to change.
//!
//! Nothing here changes a model. Each field, when its value is settled
//! (Enter, a click elsewhere, the mouse let go), becomes one
//! [`Edit`], the same one `duck-mold edit` makes, and the Studio applies
//! it to the recipe file. The fields are then read from the file again.

use duck_mold_core::edit::{Changes, Edit, Facts, Made, Sizes};
use duck_mold_core::model::Part;
use duck_mold_core::recipe::Shape;
use duck_mold_view::studio::Studio;
use eframe::egui::{self, Color32, DragValue, RichText, TextEdit};

use crate::theme;

/// A colour being made, before it is added to the palette.
struct NewColor {
    name: String,
    rgb: [u8; 3],
}

/// What is being typed or dragged in the panel, until it becomes an edit.
#[derive(Default)]
pub struct Draft {
    /// The part these values were read for, and which build of the file.
    of: Option<(String, usize)>,
    name: String,
    size: [f64; 3],
    radius: f64,
    tube: f64,
    wall: f64,
    height: f64,
    segments: u32,
    steps: u32,
    bevel: f64,
    bevel_steps: u32,
    taper: [f64; 2],
    twist: f64,
    position: [f64; 3],
    rotation: [f64; 3],
    scale: [f64; 3],
    /// The "new colour" form, when it is open.
    new_color: Option<NewColor>,
    /// A palette colour being changed with the colour picker.
    recolor: Option<[u8; 3]>,
    /// How many parts hang from the selected one, while the Studio asks
    /// whether to remove them too.
    confirm_remove: Option<usize>,
    /// Why the last change wasn't made.
    failed: Option<String>,
    /// Set to put the typing in the Name field: F2, or "Rename" in a
    /// menu.
    focus_name: bool,
    /// The names of the part's sockets, as they are being typed.
    socket_names: Vec<String>,
}

impl Draft {
    fn fill(&mut self, facts: &Facts) {
        self.name = facts.name.clone().unwrap_or_default();
        self.size = facts.sizes.size.unwrap_or([1.0; 3]);
        self.radius = facts.sizes.radius.unwrap_or(0.5);
        self.tube = facts.sizes.tube.unwrap_or(0.15);
        self.wall = facts.sizes.wall.unwrap_or(0.1);
        self.height = facts.sizes.height.unwrap_or(1.0);
        self.segments = facts.sizes.segments.unwrap_or(16);
        self.steps = facts.sizes.steps.unwrap_or(4);
        self.bevel = facts.sizes.bevel.unwrap_or(0.0);
        self.bevel_steps = facts.sizes.bevel_steps.unwrap_or(1);
        self.taper = facts.sizes.taper.unwrap_or([1.0; 2]);
        self.twist = facts.twist;
        self.position = facts.position;
        self.rotation = facts.rotation;
        self.scale = facts.scale;
        self.recolor = None;
        self.confirm_remove = None;
        self.socket_names = facts.sockets.iter().map(|(name, _)| name.clone()).collect();
    }
}

fn hex_of(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

impl Draft {
    /// Puts the typing in the Name field the next time the panel is
    /// drawn.
    pub fn rename(&mut self) {
        self.focus_name = true;
    }

    /// Removes the selected parts; or, if parts that aren't selected
    /// hang from them, asks in the panel whether those go too.
    pub fn remove(&mut self, studio: &mut Studio) {
        match studio.hanging_from_selection() {
            0 => {
                if let Err(why) = studio.remove_selected(false) {
                    self.failed = Some(why);
                }
            }
            count => self.confirm_remove = Some(count),
        }
    }
}

/// A small square of a colour.
fn swatch(ui: &mut egui::Ui, rgb: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 2.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
}

/// A field for a length in metres.
fn metres(value: &mut f64) -> DragValue<'_> {
    DragValue::new(value)
        .speed(0.01)
        .range(0.001..=1000.0)
        .max_decimals(3)
}

/// The other parts in the selected part's list: the ones it can join.
fn siblings<'a>(studio: &'a Studio, path: &str) -> Vec<&'a Part> {
    let list = match path.rsplit_once('/') {
        Some((around, _)) => studio.part(around).map(|part| part.parts.as_slice()),
        None => studio.model().map(|model| model.parts.as_slice()),
    };
    list.unwrap_or_default()
        .iter()
        .filter(|part| part.path != path && !part.name.starts_with('#'))
        .collect()
}

/// Says that a part comes from a definition, and names every other part
/// an edit would change, before any edit is made.
fn definition_notice(ui: &mut egui::Ui, facts: &Facts) {
    let (shared, lead) = match (&facts.inside, &facts.uses) {
        (Some(inside), _) => (
            inside,
            format!(
                "Inside the definition \"{}\". Any change here",
                inside.definition
            ),
        ),
        (None, Some(uses)) => (
            uses,
            format!(
                "Made from the definition \"{}\". Changing its shape, size or colour",
                uses.definition
            ),
        ),
        (None, None) => return,
    };
    let words = if shared.also_changes.is_empty() {
        format!("{lead} changes the definition. No other part uses it.")
    } else {
        format!("{lead} also changes: {}.", shared.also_changes.join(", "))
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.colored_label(theme::LOOK, words);
    });
    ui.add_space(4.0);
}

/// Asks whether the parts that hang from the selected ones go too, when
/// "Remove" found some: in a window of its own, in the middle, so it is
/// seen wherever the panel is scrolled to.
fn confirm_remove(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    let Some(count) = draft.confirm_remove else {
        return;
    };
    let several = studio.selection().len() > 1;
    let mut open = true;
    let mut close = false;
    egui::Window::new("Remove")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            ui.label(match (count, several) {
                (1, false) => "Another part hangs from this one. Remove it too?".to_owned(),
                (1, true) => "Another part hangs from these. Remove it too?".to_owned(),
                (count, false) => {
                    format!("{count} other parts hang from this one. Remove them too?")
                }
                (count, true) => format!("{count} other parts hang from these. Remove them too?"),
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Remove them all").clicked() {
                    if let Err(why) = studio.remove_selected(true) {
                        draft.failed = Some(why);
                    }
                    close = true;
                }
                if ui.button("Keep them").clicked() {
                    close = true;
                }
            });
        });
    if close || !open {
        draft.confirm_remove = None;
    }
}

/// The panel when several parts are selected: what can be set for all
/// of them.
fn show_several(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft, count: usize) {
    ui.label(RichText::new(format!("{count} parts")).strong());
    ui.separator();
    let names: Vec<String> = studio.selection().into_iter().map(str::to_owned).collect();
    ui.label(RichText::new(names.join(", ")).weak().small());
    ui.add_space(4.0);
    if studio.look_only() {
        ui.label(RichText::new("Looking only: nothing can be changed here.").weak());
        return;
    }
    ui.label(
        RichText::new(
            "The handles move, turn and scale them together. Shift+click a part to add it or take it out.",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);
    let palette = studio.palette();
    let mut colour = None;
    ui.horizontal(|ui| {
        let label = ui.label("Colour");
        egui::ComboBox::from_id_salt("duck_mold_color_several")
            .selected_text("choose...")
            .show_ui(ui, |ui| {
                for (name, rgb) in &palette {
                    ui.horizontal(|ui| {
                        swatch(ui, *rgb);
                        if ui.selectable_label(false, name).clicked() {
                            colour = Some(name.clone());
                        }
                    });
                }
            })
            .response
            .labelled_by(label.id);
    });
    let mut done = None;
    if let Some(colour) = colour {
        done = Some(studio.colour_selected(&colour));
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Duplicate")
            .on_hover_text("A copy of each, beside them (Ctrl+D)")
            .clicked()
        {
            done = Some(studio.duplicate_selected(false));
        }
        if ui.button("Mirror copy").clicked() {
            done = Some(studio.duplicate_selected(true));
        }
        if ui
            .button("Unjoin")
            .on_hover_text("Make the joined ones free, right where they are")
            .clicked()
        {
            done = Some(studio.unjoin_selected());
        }
        if ui
            .button("Hide")
            .on_hover_text("Stop drawing them. They stay in the model (H)")
            .clicked()
        {
            studio.hide_selected();
        }
        if ui.button("Remove").on_hover_text("Delete").clicked() {
            draft.remove(studio);
        }
    });
    confirm_remove(ui, studio, draft);
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

/// Draws the panel for the selected part, and makes the edit when a
/// field's value is settled.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio, draft: &mut Draft) {
    ui.add_space(4.0);
    let count = studio.selection().len();
    if count > 1 {
        draft.of = None;
        show_several(ui, studio, draft, count);
        return;
    }
    let Some(path) = studio.selected().map(str::to_owned) else {
        ui.label(RichText::new("Properties").strong());
        ui.separator();
        ui.label(RichText::new("Click a part to select it.").weak());
        if studio.hidden_count() > 0 {
            ui.add_space(6.0);
            if ui
                .button(format!("Show the {} hidden", studio.hidden_count()))
                .on_hover_text("Draw every hidden part again (Shift+H)")
                .clicked()
            {
                studio.show_all();
            }
        }
        draft.of = None;
        draft.confirm_remove = None;
        return;
    };
    ui.label(RichText::new(&path).strong());
    ui.separator();

    let Some(facts) = studio.facts() else {
        // A part that can't be edited here: say what it is.
        if let Some(part) = studio.part(&path) {
            ui.label(part.kind.label());
            if let Some(bounds) = part.bounds {
                let size = bounds.size();
                ui.label(format!("{:.2} x {:.2} x {:.2} m", size.x, size.y, size.z));
            }
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("This part is inside another file. Open that file to change it.").weak(),
        );
        return;
    };
    let key = (path.clone(), studio.builds());
    if draft.of.as_ref() != Some(&key) {
        draft.fill(&facts);
        draft.of = Some(key);
    }
    if studio.look_only() {
        ui.label(RichText::new("Looking only: nothing can be changed here.").weak());
        ui.add_space(4.0);
    }

    definition_notice(ui, &facts);

    let set = |changes: Changes| Edit::Set {
        part: path.clone(),
        changes,
    };
    let sized = |sizes: Sizes| {
        set(Changes {
            sizes,
            ..Changes::default()
        })
    };
    // The edits to make, in order (a new colour is two: add it, use it).
    let mut edits: Vec<Edit> = Vec::new();
    let palette = studio.palette();

    ui.add_enabled_ui(!studio.look_only(), |ui| {
        egui::Grid::new("duck_mold_properties_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                // ---- name
                let label = ui.label("Name");
                let name = ui
                    .add(TextEdit::singleline(&mut draft.name).desired_width(150.0))
                    .labelled_by(label.id);
                if std::mem::take(&mut draft.focus_name) {
                    name.request_focus();
                }
                if name.lost_focus()
                    && !draft.name.is_empty()
                    && Some(&draft.name) != facts.name.as_ref()
                {
                    edits.push(Edit::Rename {
                        part: path.clone(),
                        new_name: draft.name.clone(),
                    });
                }
                ui.end_row();

                // ---- what it is made of
                let made_label = ui.label("Made of");
                match &facts.made {
                    Some(Made::Shape(shape)) => {
                        let list = egui::ComboBox::from_id_salt("duck_mold_shape")
                            .selected_text(shape)
                            .show_ui(ui, |ui| {
                                for name in Shape::NAMES {
                                    if ui.selectable_label(name == shape, name).clicked()
                                        && name != shape
                                    {
                                        edits.push(set(Changes {
                                            shape: Some(name.to_owned()),
                                            ..Changes::default()
                                        }));
                                    }
                                }
                            });
                        list.response.labelled_by(made_label.id);
                    }
                    Some(Made::Use(defined)) => {
                        let what = match &facts.shape {
                            Some(shape) => format!("{defined} (a defined {shape})"),
                            None => format!("{defined} (defined parts)"),
                        };
                        ui.label(what);
                    }
                    Some(Made::File(file)) => {
                        ui.label(file);
                    }
                    None => {
                        ui.label("parts of its own");
                    }
                }
                ui.end_row();

                // ---- sizes
                let settled =
                    |response: &egui::Response| response.drag_stopped() || response.lost_focus();
                if facts.sizes.size.is_some() {
                    let label = ui
                        .label("Size")
                        .on_hover_text("Width (x), height (y) and depth (z), in metres");
                    ui.horizontal(|ui| {
                        let mut done = false;
                        for (at, axis) in ["Width", "Height", "Depth"].into_iter().enumerate() {
                            let field = ui
                                .add(metres(&mut draft.size[at]))
                                .labelled_by(label.id)
                                .on_hover_text(format!("{axis}, in metres"));
                            done |= settled(&field);
                        }
                        if done && Some(draft.size) != facts.sizes.size {
                            edits.push(sized(Sizes {
                                size: Some(draft.size),
                                ..Sizes::default()
                            }));
                        }
                    });
                    ui.end_row();
                }
                if facts.sizes.radius.is_some() {
                    let label = ui.label("Radius");
                    let field = ui.add(metres(&mut draft.radius)).labelled_by(label.id);
                    if settled(&field) && Some(draft.radius) != facts.sizes.radius {
                        edits.push(sized(Sizes {
                            radius: Some(draft.radius),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }
                if facts.sizes.tube.is_some() {
                    let label = ui
                        .label("Tube")
                        .on_hover_text("How thick the ring is: from its middle to its surface");
                    let field = ui.add(metres(&mut draft.tube)).labelled_by(label.id);
                    if settled(&field) && Some(draft.tube) != facts.sizes.tube {
                        edits.push(sized(Sizes {
                            tube: Some(draft.tube),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }
                if facts.sizes.wall.is_some() {
                    let label = ui
                        .label("Wall")
                        .on_hover_text("How thick the wall is, in metres");
                    let field = ui.add(metres(&mut draft.wall)).labelled_by(label.id);
                    if settled(&field) && Some(draft.wall) != facts.sizes.wall {
                        edits.push(sized(Sizes {
                            wall: Some(draft.wall),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }
                if facts.sizes.height.is_some() {
                    let label = ui.label("Height");
                    let field = ui.add(metres(&mut draft.height)).labelled_by(label.id);
                    if settled(&field) && Some(draft.height) != facts.sizes.height {
                        edits.push(sized(Sizes {
                            height: Some(draft.height),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }
                if facts.sizes.segments.is_some() {
                    let label = ui
                        .label("Sides")
                        .on_hover_text("How many flat sides the round shape has");
                    let fewest = match facts.shape.as_deref() {
                        Some("sphere") => 4,
                        Some("arch") => 2,
                        _ => 3,
                    };
                    let field = ui
                        .add(DragValue::new(&mut draft.segments).range(fewest..=64))
                        .labelled_by(label.id);
                    if settled(&field) && Some(draft.segments) != facts.sizes.segments {
                        edits.push(sized(Sizes {
                            segments: Some(draft.segments),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }

                if facts.sizes.steps.is_some() {
                    let label = ui.label("Steps").on_hover_text("How many steps");
                    let field = ui
                        .add(DragValue::new(&mut draft.steps).range(1..=64))
                        .labelled_by(label.id);
                    if settled(&field) && Some(draft.steps) != facts.sizes.steps {
                        edits.push(sized(Sizes {
                            steps: Some(draft.steps),
                            ..Sizes::default()
                        }));
                    }
                    ui.end_row();
                }
                if facts.sizes.bevel.is_some() {
                    let label = ui.label("Bevel").on_hover_text(
                        "How much is cut off the edges, in metres (0 for sharp edges), and how many flat cuts go round each edge (1 to 4)",
                    );
                    ui.horizontal(|ui| {
                        let field = ui
                            .add(
                                DragValue::new(&mut draft.bevel)
                                    .speed(0.005)
                                    .range(0.0..=1000.0)
                                    .max_decimals(3),
                            )
                            .labelled_by(label.id)
                            .on_hover_text("How much is cut off, in metres");
                        if settled(&field) && Some(draft.bevel) != facts.sizes.bevel {
                            edits.push(sized(Sizes {
                                bevel: Some(draft.bevel),
                                ..Sizes::default()
                            }));
                        }
                        let cuts = ui
                            .add_enabled(
                                draft.bevel > 0.0,
                                DragValue::new(&mut draft.bevel_steps)
                                    .range(1..=4)
                                    .suffix(" cuts"),
                            )
                            .on_hover_text("How many flat cuts go round each edge");
                        if settled(&cuts) && Some(draft.bevel_steps) != facts.sizes.bevel_steps {
                            edits.push(sized(Sizes {
                                bevel_steps: Some(draft.bevel_steps),
                                ..Sizes::default()
                            }));
                        }
                    });
                    ui.end_row();
                }
                if facts.sizes.taper.is_some() {
                    let label = ui.label("Taper").on_hover_text(
                        "How wide (x) and deep (z) the top is, as a share of the bottom: 1 is straight sides, 0.5 is half as wide",
                    );
                    ui.horizontal(|ui| {
                        let mut done = false;
                        for (at, axis) in ["wide", "deep"].into_iter().enumerate() {
                            let field = ui
                                .add(
                                    DragValue::new(&mut draft.taper[at])
                                        .speed(0.01)
                                        .range(0.01..=10.0)
                                        .max_decimals(2),
                                )
                                .labelled_by(label.id)
                                .on_hover_text(format!("How {axis} the top is"));
                            done |= settled(&field);
                        }
                        if done && Some(draft.taper) != facts.sizes.taper {
                            edits.push(sized(Sizes {
                                taper: Some(draft.taper),
                                ..Sizes::default()
                            }));
                        }
                    });
                    ui.end_row();
                }

                // ---- where a free part is, and how it is turned
                if facts.to.is_none() {
                    let label = ui
                        .label("Position")
                        .on_hover_text("Where its middle is: x, y, z in metres");
                    ui.horizontal(|ui| {
                        let mut done = false;
                        for (at, axis) in ["x", "y", "z"].into_iter().enumerate() {
                            let field = ui
                                .add(
                                    DragValue::new(&mut draft.position[at])
                                        .speed(0.01)
                                        .range(-10_000.0..=10_000.0)
                                        .max_decimals(3),
                                )
                                .labelled_by(label.id)
                                .on_hover_text(axis);
                            done |= settled(&field);
                        }
                        if done && draft.position != facts.position {
                            edits.push(set(Changes {
                                move_to: Some(draft.position),
                                ..Changes::default()
                            }));
                        }
                    });
                    ui.end_row();

                    let label = ui
                        .label("Rotation")
                        .on_hover_text("Degrees around x, then y, then z");
                    ui.horizontal(|ui| {
                        let mut done = false;
                        for (at, axis) in ["x", "y", "z"].into_iter().enumerate() {
                            let field = ui
                                .add(
                                    DragValue::new(&mut draft.rotation[at])
                                        .speed(1.0)
                                        .range(-36_000.0..=36_000.0)
                                        .max_decimals(1)
                                        .suffix("°"),
                                )
                                .labelled_by(label.id)
                                .on_hover_text(axis);
                            done |= settled(&field);
                        }
                        if done && draft.rotation != facts.rotation {
                            edits.push(set(Changes {
                                rotate: Some(draft.rotation),
                                ..Changes::default()
                            }));
                        }
                    });
                    ui.end_row();
                }
                // ---- how much bigger it is, joined or not
                let label = ui
                    .label("Scale")
                    .on_hover_text("How many times bigger it is, each way: 1 is as it was made");
                ui.horizontal(|ui| {
                    let mut done = false;
                    for (at, axis) in ["x", "y", "z"].into_iter().enumerate() {
                        let field = ui
                            .add(
                                DragValue::new(&mut draft.scale[at])
                                    .speed(0.01)
                                    .range(0.001..=1000.0)
                                    .max_decimals(3),
                            )
                            .labelled_by(label.id)
                            .on_hover_text(axis);
                        done |= settled(&field);
                    }
                    if done && draft.scale != facts.scale {
                        edits.push(set(Changes {
                            scale: Some(draft.scale),
                            ..Changes::default()
                        }));
                    }
                });
                ui.end_row();

                // ---- colour
                let colour_label = ui.label("Colour");
                ui.horizontal(|ui| {
                    let now = facts.color.as_deref();
                    let now_rgb = now.and_then(|name| {
                        palette
                            .iter()
                            .find(|(known, _)| known == name)
                            .map(|(_, rgb)| *rgb)
                    });
                    let list = egui::ComboBox::from_id_salt("duck_mold_color")
                        .selected_text(now.unwrap_or("none"))
                        .show_ui(ui, |ui| {
                            for (name, rgb) in &palette {
                                ui.horizontal(|ui| {
                                    swatch(ui, *rgb);
                                    if ui.selectable_label(now == Some(name), name).clicked()
                                        && now != Some(name)
                                    {
                                        edits.push(set(Changes {
                                            color: Some(name.clone()),
                                            ..Changes::default()
                                        }));
                                    }
                                });
                            }
                            ui.separator();
                            if ui.selectable_label(false, "New colour...").clicked() {
                                draft.new_color = Some(NewColor {
                                    name: String::new(),
                                    rgb: [0xFF, 0xCC, 0x2E],
                                });
                            }
                        });
                    list.response.labelled_by(colour_label.id);
                    // The colour itself can be changed: that changes it
                    // everywhere it is used.
                    if let (Some(name), Some(rgb)) = (now, now_rgb) {
                        let mut picked = draft.recolor.unwrap_or(rgb);
                        let button = ui
                            .color_edit_button_srgb(&mut picked)
                            .on_hover_text(format!(
                                "Change the colour \"{name}\" itself, everywhere it is used"
                            ));
                        if button.changed() {
                            draft.recolor = Some(picked);
                        }
                        // Settled when the mouse is let go.
                        if let Some(picked) = draft.recolor
                            && !ui.input(|input| input.pointer.any_down())
                        {
                            if picked != rgb {
                                edits.push(Edit::SetColor {
                                    name: name.to_owned(),
                                    hex: hex_of(picked),
                                });
                            }
                            draft.recolor = None;
                        }
                    }
                });
                ui.end_row();

                // ---- the join
                let others = siblings(studio, &path);
                let joined = facts
                    .to
                    .as_deref()
                    .and_then(|to| to.rsplit_once('.'))
                    .map(|(part, socket)| (part.to_owned(), socket.to_owned()));
                ui.label("Joined to")
                    .on_hover_text("The part, and the socket on it, that this part sits on");
                ui.horizontal(|ui| {
                    let on = joined.as_ref().map(|(part, _)| part.as_str());
                    egui::ComboBox::from_id_salt("duck_mold_join_part")
                        .selected_text(on.unwrap_or("nothing"))
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for other in &others {
                                if ui
                                    .selectable_label(on == Some(other.name.as_str()), &other.name)
                                    .clicked()
                                    && on != Some(other.name.as_str())
                                {
                                    // The same socket there if it has
                                    // one, else its top.
                                    let socket = joined
                                        .as_ref()
                                        .map(|(_, socket)| socket.as_str())
                                        .filter(|socket| {
                                            other.sockets.iter().any(|known| known.name == *socket)
                                        })
                                        .unwrap_or("top");
                                    edits.push(Edit::Join {
                                        part: path.clone(),
                                        to: format!("{}.{socket}", other.name),
                                        attach: None,
                                    });
                                }
                            }
                        });
                    if let Some((on, socket)) = &joined {
                        let sockets = others
                            .iter()
                            .find(|other| other.name == *on)
                            .map(|other| other.sockets.as_slice())
                            .unwrap_or_default();
                        egui::ComboBox::from_id_salt("duck_mold_join_socket")
                            .selected_text(socket)
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for known in sockets {
                                    if ui
                                        .selectable_label(known.name == *socket, &known.name)
                                        .clicked()
                                        && known.name != *socket
                                    {
                                        edits.push(Edit::Join {
                                            part: path.clone(),
                                            to: format!("{on}.{}", known.name),
                                            attach: None,
                                        });
                                    }
                                }
                            });
                    }
                });
                ui.end_row();

                if let (Some(to), Some(attach)) = (&facts.to, &facts.attach) {
                    ui.label("By its")
                        .on_hover_text("The part's own socket that sits on that one");
                    let own = studio
                        .part(&path)
                        .map(|part| part.sockets.as_slice())
                        .unwrap_or_default();
                    egui::ComboBox::from_id_salt("duck_mold_join_own")
                        .selected_text(attach)
                        .show_ui(ui, |ui| {
                            for known in own {
                                if ui
                                    .selectable_label(known.name == *attach, &known.name)
                                    .clicked()
                                    && known.name != *attach
                                {
                                    edits.push(Edit::Join {
                                        part: path.clone(),
                                        to: to.clone(),
                                        attach: Some(known.name.clone()),
                                    });
                                }
                            }
                        });
                    ui.end_row();

                    // ---- twist
                    let label = ui
                        .label("Twist")
                        .on_hover_text("Degrees to turn the part around its join");
                    let field = ui
                        .add(
                            DragValue::new(&mut draft.twist)
                                .speed(1.0)
                                .range(-36000.0..=36000.0)
                                .max_decimals(1)
                                .suffix("°"),
                        )
                        .labelled_by(label.id);
                    if settled(&field) && draft.twist != facts.twist {
                        edits.push(set(Changes {
                            twist: Some(draft.twist),
                            ..Changes::default()
                        }));
                    }
                    ui.end_row();
                }
            });

        // ---- the sockets it declares
        if !facts.sockets.is_empty() && draft.socket_names.len() == facts.sockets.len() {
            ui.add_space(8.0);
            ui.label(RichText::new("Sockets").strong()).on_hover_text(
                "The sockets this part declares: named points other parts join to. Its seven built-in ones (top, bottom...) aren't listed",
            );
            for (at, (name, uses)) in facts.sockets.iter().enumerate() {
                ui.horizontal(|ui| {
                    let field = ui
                        .add(
                            TextEdit::singleline(&mut draft.socket_names[at]).desired_width(100.0),
                        )
                        .on_hover_text("Type a new name: everything joined to it follows");
                    if field.lost_focus()
                        && !draft.socket_names[at].is_empty()
                        && draft.socket_names[at] != *name
                    {
                        edits.push(Edit::RenameSocket {
                            socket: format!("{path}.{name}"),
                            new_name: draft.socket_names[at].clone(),
                        });
                    }
                    ui.label(
                        RichText::new(match uses {
                            0 => "not used".to_owned(),
                            1 => "1 use".to_owned(),
                            count => format!("{count} uses"),
                        })
                        .weak()
                        .small(),
                    );
                    let remove = ui
                        .add_enabled(
                            *uses == 0,
                            egui::Button::new(RichText::new("remove socket").small()),
                        )
                        .on_hover_text("Take this socket away")
                        .on_disabled_hover_text(
                            "Something is joined to it: move that away first, or remove it",
                        );
                    if remove.clicked() {
                        edits.push(Edit::RemoveSocket {
                            socket: format!("{path}.{name}"),
                        });
                    }
                });
            }
        }

        // ---- a new colour
        if let Some(new) = &mut draft.new_color {
            ui.add_space(6.0);
            let mut close = false;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(RichText::new("New colour").strong());
                ui.horizontal(|ui| {
                    let label = ui.label("Called");
                    ui.add(
                        TextEdit::singleline(&mut new.name)
                            .desired_width(110.0)
                            .hint_text("a name"),
                    )
                    .labelled_by(label.id);
                    ui.color_edit_button_srgb(&mut new.rgb);
                });
                ui.horizontal(|ui| {
                    let ready = !new.name.is_empty();
                    if ui
                        .add_enabled(ready, theme::primary_button("Add colour"))
                        .clicked()
                    {
                        edits.push(Edit::AddColor {
                            name: new.name.clone(),
                            hex: hex_of(new.rgb),
                        });
                        edits.push(set(Changes {
                            color: Some(new.name.clone()),
                            ..Changes::default()
                        }));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if close {
                draft.new_color = None;
            }
        }

        // ---- the whole part
        ui.add_space(8.0);
        let mut done = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("Duplicate")
                .on_hover_text(
                    "A copy. Of a joined part: on the same socket. Of a free part: beside it (Ctrl+D)",
                )
                .clicked()
            {
                done = Some(studio.duplicate_selected(false));
            }
            if ui
                .button("Mirror copy")
                .on_hover_text("A mirror image: a right arm from a left arm")
                .clicked()
            {
                done = Some(studio.duplicate_selected(true));
            }
            if facts.to.is_some()
                && ui
                    .button("Unjoin")
                    .on_hover_text("Make it free, right where it is, so it can be moved anywhere")
                    .clicked()
            {
                edits.push(Edit::Unjoin { part: path.clone() });
            }
            if ui
                .button("Hide")
                .on_hover_text("Stop drawing it. It stays in the model (H)")
                .clicked()
            {
                studio.hide_selected();
            }
            if ui.button("Remove").on_hover_text("Delete").clicked() {
                draft.remove(studio);
            }
        });
        match done {
            Some(Ok(_)) => draft.failed = None,
            Some(Err(why)) => draft.failed = Some(why),
            None => {}
        }
        confirm_remove(ui, studio, draft);
    });

    for edit in &edits {
        match studio.apply(edit) {
            Ok(_) => draft.failed = None,
            Err(why) => {
                draft.failed = Some(why);
                // What was typed goes back to what the file says.
                draft.of = None;
                break;
            }
        }
    }
    if let Some(why) = &draft.failed {
        ui.add_space(6.0);
        ui.colored_label(theme::BAD, why);
    }
}
