//! The timeline, along the bottom in Pose mode, and the Play button in
//! the view.
//!
//! The timeline shows the chosen animation's keys, each a diamond with
//! its pose's name, along a ruler of seconds. A click on a key goes to
//! it; a drag of a key moves it (it snaps to 0.05 s and can't pass its
//! neighbours); a drag anywhere else moves through time. A right-click
//! on a key has Remove and another pose. Play (Space) plays it; Loop,
//! Smooth and Ghosts are switches; + Key adds the pose chosen at the
//! time shown. Every change to the file is one of the key commands.

use duck_mold_view::studio::{KeyMark, Studio};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};

use crate::theme;

/// How tall the ruler is, in points.
const RULER: f32 = 58.0;
/// Room at either end of the ruler.
const ENDS: f32 = 24.0;
/// How near the pointer must be to a key to be on it.
const KEY_REACH: f32 = 9.0;

/// What the timeline remembers between frames.
#[derive(Default)]
pub struct Timeline {
    /// Where the ruler was drawn, and how long the animation was then.
    placed: Option<(Rect, f64)>,
    /// A key being dragged: its number, and where it would go.
    dragging: Option<(usize, f64)>,
    /// A drag along the ruler that moves through time.
    scrubbing: bool,
    /// The key a right-click opened the menu for.
    menu_for: Option<usize>,
}

impl Timeline {
    /// Where a time is along the ruler, as it was last drawn.
    fn x_of(&self, seconds: f64) -> Option<f32> {
        let (rect, length) = self.placed?;
        let span = rect.width() - 2.0 * ENDS;
        Some(rect.left() + ENDS + span * (seconds / length.max(0.1)) as f32)
    }

    /// The time at a point along the ruler.
    fn time_at(&self, x: f32) -> Option<f64> {
        let (rect, length) = self.placed?;
        let span = (rect.width() - 2.0 * ENDS).max(1.0);
        Some((f64::from((x - rect.left() - ENDS) / span) * length.max(0.1)).clamp(0.0, length))
    }

    /// Where a key's diamond is on the screen, as the ruler was last
    /// drawn: for tests.
    pub fn key_on_screen(&self, at: f64) -> Option<Pos2> {
        let (rect, _) = self.placed?;
        Some(pos2(self.x_of(at)?, rect.top() + RULER * 0.45))
    }

    /// Where a time is on the ruler, on the screen: for tests.
    pub fn time_on_screen(&self, at: f64) -> Option<Pos2> {
        let (rect, _) = self.placed?;
        Some(pos2(self.x_of(at)?, rect.top() + RULER * 0.85))
    }
}

/// Seconds as the timeline writes them.
fn seconds(at: f64) -> String {
    let rounded = (at * 100.0).round() / 100.0 + 0.0;
    format!("{rounded} s")
}

/// The timeline, in Pose mode.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio, timeline: &mut Timeline) {
    let editable = !studio.look_only();
    let mut done: Option<Result<String, String>> = None;
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let label = ui.label(RichText::new("Animation").strong());
        let names = studio.animation_names();
        let chosen = studio.animation().map(str::to_owned);
        let list = egui::ComboBox::from_id_salt("duck_mold_animation")
            .selected_text(chosen.clone().unwrap_or_else(|| "none".to_owned()))
            .show_ui(ui, |ui| {
                if ui.selectable_label(chosen.is_none(), "none").clicked() {
                    studio.choose_animation(None);
                }
                for name in &names {
                    if ui
                        .selectable_label(chosen.as_deref() == Some(name.as_str()), name)
                        .clicked()
                    {
                        studio.choose_animation(Some(name));
                    }
                }
            });
        list.response.labelled_by(label.id);
        if ui
            .add_enabled(editable, egui::Button::new("New"))
            .on_hover_text("A new animation, starting in rest")
            .clicked()
        {
            done = Some(studio.new_animation());
        }
        let has = studio.animation().is_some();
        let play = if studio.is_playing() { "Stop" } else { "Play" };
        if ui
            .add_enabled(has, egui::Button::new(play))
            .on_hover_text("Play the animation in the view, or stop it (Space)")
            .clicked()
        {
            studio.toggle_play();
        }
        if let Some((looping, smooth)) = studio.animation_settings() {
            let mut looping_now = looping;
            if ui
                .add_enabled(editable, egui::Checkbox::new(&mut looping_now, "Loop"))
                .on_hover_text("It starts again after its last key")
                .changed()
            {
                done = Some(studio.set_looping(looping_now));
            }
            let mut smooth_now = smooth;
            if ui
                .add_enabled(editable, egui::Checkbox::new(&mut smooth_now, "Smooth"))
                .on_hover_text(
                    "It flows through each pose and starts and ends gently; off, it goes straight from pose to pose",
                )
                .changed()
            {
                done = Some(studio.set_smooth(smooth_now));
            }
        }
        let mut ghosts = studio.ghosts();
        if ui
            .checkbox(&mut ghosts, "Ghosts")
            .on_hover_text("The poses of the keys before and after, drawn faintly (blue before, orange after)")
            .changed()
        {
            studio.set_ghosts(ghosts);
        }
        if has {
            ui.label(
                RichText::new(format!(
                    "{} of {}",
                    seconds(studio.time().unwrap_or(0.0)),
                    seconds(studio.animation_length())
                ))
                .weak(),
            );
            if ui
                .add_enabled(editable, egui::Button::new("+ Key"))
                .on_hover_text(format!(
                    "A key here, with the pose chosen (\"{}\")",
                    studio.pose()
                ))
                .clicked()
            {
                done = Some(studio.add_key_here());
            }
        }
    });

    // ---- the ruler
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, RULER), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_black_alpha(90));
    let Some(_) = studio.animation() else {
        timeline.placed = None;
        ui.put(
            rect,
            egui::Label::new(
                RichText::new("Choose an animation, or make one with New, to see its keys here")
                    .color(Color32::from_white_alpha(140)),
            ),
        );
        finish(ui, studio, done);
        return;
    };
    let length = studio.animation_length();
    timeline.placed = Some((rect, length));
    let keys = studio.keys();
    let quiet = Color32::from_white_alpha(120);
    // Ticks: every tenth of a second, a longer one and a number every
    // half.
    let tenths = (length * 10.0).round() as usize;
    let step = if tenths > 100 { 10 } else { 1 };
    for tenth in (0..=tenths).step_by(step) {
        let at = tenth as f64 / 10.0;
        let Some(x) = timeline.x_of(at) else { continue };
        let long = tenth % 5 == 0;
        let top = rect.bottom() - if long { 14.0 } else { 7.0 };
        painter.line_segment(
            [pos2(x, top), pos2(x, rect.bottom() - 2.0)],
            Stroke::new(1.0, quiet),
        );
        if long {
            painter.text(
                pos2(x + 2.0, rect.bottom() - 15.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{at}"),
                egui::FontId::proportional(10.0),
                quiet,
            );
        }
    }
    // The keys, each a diamond with its pose's name above.
    let shown_at = |key: &KeyMark| match timeline.dragging {
        Some((number, at)) if number == key.number => at,
        _ => key.at,
    };
    let here = studio.key_here().map(|key| key.number);
    for key in &keys {
        let Some(x) = timeline.x_of(shown_at(key)) else {
            continue;
        };
        let middle = pos2(x, rect.top() + RULER * 0.45);
        let size = 6.0;
        let diamond = vec![
            middle + vec2(0.0, -size),
            middle + vec2(size, 0.0),
            middle + vec2(0.0, size),
            middle + vec2(-size, 0.0),
        ];
        let color = if here == Some(key.number) {
            theme::DUCK_YELLOW
        } else {
            Color32::from_rgb(0xE8, 0xE8, 0xF0)
        };
        painter.add(egui::Shape::convex_polygon(diamond, color, Stroke::NONE));
        painter.text(
            middle + vec2(0.0, -size - 2.0),
            egui::Align2::CENTER_BOTTOM,
            &key.pose,
            egui::FontId::proportional(11.0),
            color,
        );
    }
    // The time shown.
    if let Some(x) = studio.time().and_then(|at| timeline.x_of(at)) {
        painter.line_segment(
            [pos2(x, rect.top() + 2.0), pos2(x, rect.bottom() - 2.0)],
            Stroke::new(2.0, theme::NIGHT_INDIGO),
        );
    }

    // ---- the mouse on the ruler
    let spots: Vec<(usize, Pos2)> = keys
        .iter()
        .filter_map(|key| {
            let x = timeline.x_of(key.at)?;
            Some((key.number, pos2(x, rect.top() + RULER * 0.45)))
        })
        .collect();
    let key_under = |pointer: Pos2| {
        spots
            .iter()
            .filter_map(|(number, middle)| {
                let away = middle.distance(pointer);
                (away <= KEY_REACH).then_some((away, *number))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, number)| number)
    };
    let free = ui.input(|input| input.modifiers.command);
    if response.drag_started()
        && let Some(pressed) = ui.input(|input| input.pointer.press_origin())
    {
        match key_under(pressed).filter(|_| editable) {
            Some(number) => {
                let at = keys[number - 1].at;
                timeline.dragging = Some((number, at));
                studio.stop();
            }
            None => {
                timeline.scrubbing = true;
                studio.stop();
            }
        }
    }
    if let Some(pointer) = response.interact_pointer_pos()
        && response.dragged()
    {
        let at = timeline.time_at(pointer.x).unwrap_or(0.0);
        if let Some((number, _)) = timeline.dragging {
            let to = studio.key_time(number, at, free).unwrap_or(at);
            timeline.dragging = Some((number, to));
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if timeline.scrubbing {
            studio.set_time(at);
        }
    }
    if response.drag_stopped() {
        if let Some((number, to)) = timeline.dragging.take() {
            done = Some(studio.move_key(number, to, true));
        }
        timeline.scrubbing = false;
    }
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        studio.stop();
        match key_under(pointer) {
            Some(number) => studio.click_key(number),
            None => studio.set_time(timeline.time_at(pointer.x).unwrap_or(0.0)),
        }
    }
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        timeline.menu_for = key_under(pointer);
    }
    if editable {
        let poses = studio.pose_names();
        response.context_menu(|ui| {
            let Some(number) = timeline.menu_for else {
                ui.label(RichText::new("Right-click a key for its menu").weak());
                return;
            };
            ui.label(RichText::new(format!("Key {number}")).weak().small());
            if ui.button("Remove").clicked() {
                done = Some(studio.remove_key(number));
                ui.close();
            }
            ui.menu_button("Pose", |ui| {
                for pose in &poses {
                    if ui.button(pose).clicked() {
                        done = Some(studio.set_key_pose(number, pose));
                        ui.close();
                    }
                }
            });
        });
    }
    finish(ui, studio, done);
}

/// Says why the last change wasn't made, if it wasn't (the status line
/// says what was done).
fn finish(ui: &mut egui::Ui, _studio: &Studio, done: Option<Result<String, String>>) {
    if let Some(Err(why)) = done {
        ui.colored_label(theme::BAD, why);
    }
}

/// The Play button in the view, at its top right, in Part mode, for a
/// model that has animations: pick one, and Play (Space) plays it in the
/// view. Anything else done to the model stops it.
pub fn play_button(ui: &mut egui::Ui, studio: &mut Studio, within: Rect) {
    let names = studio.animation_names();
    if names.is_empty() {
        return;
    }
    let place = Rect::from_min_size(
        pos2(within.right() - 180.0, within.top() + 8.0),
        vec2(172.0, 24.0),
    );
    ui.painter_at(within)
        .rect_filled(place.expand(2.0), 4.0, Color32::from_black_alpha(150));
    ui.scope_builder(egui::UiBuilder::new().max_rect(place), |ui| {
        ui.horizontal(|ui| {
            let chosen = studio
                .animation()
                .map(str::to_owned)
                .unwrap_or_else(|| names[0].clone());
            let list = egui::ComboBox::from_id_salt("duck_mold_view_animation")
                .width(110.0)
                .selected_text(&chosen)
                .show_ui(ui, |ui| {
                    for name in &names {
                        if ui.selectable_label(*name == chosen, name).clicked() {
                            studio.choose_animation(Some(name));
                        }
                    }
                });
            list.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::ComboBox, true, "Animation to play")
            });
            let words = if studio.is_playing() { "Stop" } else { "Play" };
            if ui
                .button(words)
                .on_hover_text("Play the animation in the view, or stop it (Space). The model comes back to rest when it stops")
                .clicked()
            {
                if studio.animation().is_none() {
                    studio.choose_animation(Some(&chosen));
                }
                studio.toggle_play();
            }
        });
    });
}
