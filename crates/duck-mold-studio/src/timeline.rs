//! The timeline, along the bottom in Pose mode, and the Play button in
//! the view.
//!
//! The timeline shows the chosen animation's keys, each a diamond with
//! its pose's name, along a ruler of seconds. A click on a key goes to
//! it; a drag of a key moves it (it snaps to 0.05 s and can't pass its
//! neighbours); a drag anywhere else moves through time. A right-click
//! on a key has Remove, another pose, and its events: a name to type and
//! Add event, and Remove for each; a key's events show as small flags
//! under it, with their names. Play (Space) plays it; Loop,
//! Smooth and Ghosts are switches; + Key adds the pose chosen at the
//! time shown. Every change to the file is one of the key commands.
//!
//! The ruler zooms and scrolls: Ctrl and the mouse wheel (or `-` and
//! `+` with the pointer on it, or the − and + buttons) zoom about the
//! pointer; the wheel, or a drag of the bar under it, scrolls; Fit shows
//! the whole animation again, as it is at first.

use duck_mold_view::studio::{KeyMark, Studio};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};

use crate::theme;

/// How tall the ruler is, in points.
const RULER: f32 = 74.0;
/// Room at either end of the ruler.
const ENDS: f32 = 24.0;
/// How near the pointer must be to a key to be on it.
const KEY_REACH: f32 = 9.0;
/// How tall the bar under the ruler that scrolls it is.
const BAR: f32 = 8.0;
/// The most the ruler zooms in: forty times, a fortieth of the
/// animation across it.
const MOST_ZOOM: f64 = 40.0;
/// How much one press of − or + zooms.
const ZOOM_STEP: f64 = 1.5;

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
    /// The name of an event being typed in a key's menu.
    event_name: String,
    /// How far the ruler is zoomed in: 1 shows the whole animation.
    zoom: f64,
    /// The time at the ruler's left edge, when zoomed in.
    scroll: f64,
    /// The animation the zoom is for: another starts fitted.
    zoomed_for: Option<String>,
}

impl Timeline {
    /// The stretch of time the ruler shows: where it starts, and how
    /// long it is.
    fn window(&self) -> Option<(f64, f64)> {
        let (_, length) = self.placed?;
        let length = length.max(0.1);
        let span = length / self.zoom.max(1.0);
        Some((self.scroll.clamp(0.0, length - span), span))
    }

    /// Where a time is along the ruler, as it was last drawn.
    fn x_of(&self, seconds: f64) -> Option<f32> {
        let (rect, _) = self.placed?;
        let (start, span) = self.window()?;
        let across = rect.width() - 2.0 * ENDS;
        Some(rect.left() + ENDS + across * ((seconds - start) / span) as f32)
    }

    /// The time at a point along the ruler.
    fn time_at(&self, x: f32) -> Option<f64> {
        let (rect, length) = self.placed?;
        let (start, span) = self.window()?;
        let across = (rect.width() - 2.0 * ENDS).max(1.0);
        Some((start + f64::from((x - rect.left() - ENDS) / across) * span).clamp(0.0, length))
    }

    /// Zooms by `factor` (above 1 in, below 1 out), keeping the time at
    /// `x` where it is on the ruler.
    fn zoom_about(&mut self, factor: f64, x: f32) {
        let (Some((rect, length)), Some(anchor)) = (self.placed, self.time_at(x)) else {
            return;
        };
        let length = length.max(0.1);
        self.zoom = (self.zoom * factor).clamp(1.0, MOST_ZOOM);
        let span = length / self.zoom;
        let across = (rect.width() - 2.0 * ENDS).max(1.0);
        let share = f64::from((x - rect.left() - ENDS) / across).clamp(0.0, 1.0);
        self.scroll = (anchor - share * span).clamp(0.0, length - span);
    }

    /// Moves the ruler along by `seconds` (later if above 0).
    fn scroll_by(&mut self, seconds: f64) {
        if let (Some((_, length)), Some((start, span))) = (self.placed, self.window()) {
            self.scroll = (start + seconds).clamp(0.0, (length.max(0.1) - span).max(0.0));
        }
    }

    /// Shows the whole animation, as it is at first.
    pub fn fit(&mut self) {
        self.zoom = 1.0;
        self.scroll = 0.0;
    }

    /// How far it is zoomed in (1: the whole animation), and the time
    /// at its left edge: for tests.
    pub fn zoomed(&self) -> (f64, f64) {
        (
            self.zoom.max(1.0),
            self.window().map_or(0.0, |(start, _)| start),
        )
    }

    /// Where a key's diamond is on the screen, as the ruler was last
    /// drawn: for tests.
    pub fn key_on_screen(&self, at: f64) -> Option<Pos2> {
        let (rect, _) = self.placed?;
        Some(pos2(self.x_of(at)?, rect.top() + RULER * 0.4))
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
            if ui
                .add_enabled(editable, egui::Button::new("Fit to ground"))
                .on_hover_text(
                    "Stand each pose this animation uses on the ground: the body moves up or down until the lowest point touches it. A leap is left alone",
                )
                .clicked()
            {
                done = Some(studio.fit_ground());
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
            ui.separator();
            let middle = timeline
                .placed
                .map_or(0.0, |(rect, _)| rect.center().x);
            if ui
                .button("-")
                .on_hover_text("Zoom out (-, or Ctrl and the mouse wheel, on the ruler)")
                .clicked()
            {
                timeline.zoom_about(1.0 / ZOOM_STEP, middle);
            }
            if ui
                .button("+")
                .on_hover_text("Zoom in (+, or Ctrl and the mouse wheel, on the ruler)")
                .clicked()
            {
                timeline.zoom_about(ZOOM_STEP, middle);
            }
            if ui
                .button("Fit")
                .on_hover_text("Show the whole animation")
                .clicked()
            {
                timeline.fit();
            }
        }
    });
    if timeline.zoomed_for.as_deref() != studio.animation() {
        timeline.zoomed_for = studio.animation().map(str::to_owned);
        timeline.fit();
    }

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
    // half; zoomed in far, every hundredth, a number every tenth.
    let (_, span) = timeline.window().unwrap_or((0.0, length));
    let fine = span < 0.6;
    let (per_second, every_long) = if fine { (100.0, 10) } else { (10.0, 5) };
    let ticks = (length * per_second).round() as usize;
    let step = if !fine && ticks > 100 && span > 10.0 {
        10
    } else {
        1
    };
    for tick in (0..=ticks).step_by(step) {
        let at = tick as f64 / per_second;
        let Some(x) = timeline.x_of(at) else { continue };
        if x < rect.left() - 1.0 || x > rect.right() + 1.0 {
            continue;
        }
        let long = tick % every_long == 0;
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
                egui::FontId::proportional(theme::SMALLEST_TEXT),
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
        let middle = pos2(x, rect.top() + RULER * 0.4);
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
            egui::FontId::proportional(theme::SMALLEST_TEXT),
            color,
        );
        // Its events: a small flag under it, and their names.
        if !key.events.is_empty() {
            let pole = middle + vec2(0.0, size + 2.0);
            let flag = theme::EVENT_FLAG;
            painter.line_segment([pole, pole + vec2(0.0, 11.0)], Stroke::new(1.0, flag));
            painter.add(egui::Shape::convex_polygon(
                vec![pole, pole + vec2(7.0, 2.5), pole + vec2(0.0, 5.0)],
                flag,
                Stroke::NONE,
            ));
            painter.text(
                pole + vec2(9.0, 0.0),
                egui::Align2::LEFT_TOP,
                key.events.join(", "),
                egui::FontId::proportional(theme::SMALLEST_TEXT),
                flag,
            );
        }
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
            Some((key.number, pos2(x, rect.top() + RULER * 0.4)))
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
    // Zooming and scrolling, with the pointer on the ruler.
    if let Some(pointer) = response.hover_pos() {
        let (zoom, wheel, out, inward) = ui.input(|input| {
            (
                input.zoom_delta(),
                input.smooth_scroll_delta,
                input.key_pressed(egui::Key::Minus),
                input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals),
            )
        });
        if zoom != 1.0 {
            timeline.zoom_about(f64::from(zoom), pointer.x);
        } else if wheel != egui::Vec2::ZERO {
            let (_, span) = timeline.window().unwrap_or((0.0, length));
            let across = f64::from((rect.width() - 2.0 * ENDS).max(1.0));
            timeline.scroll_by(-f64::from(wheel.x + wheel.y) / across * span);
        }
        if out {
            timeline.zoom_about(1.0 / ZOOM_STEP, pointer.x);
        }
        if inward {
            timeline.zoom_about(ZOOM_STEP, pointer.x);
        }
    }

    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        timeline.menu_for = key_under(pointer);
        timeline.event_name.clear();
    }
    if editable {
        let poses = studio.pose_names();
        let events_of = |number: usize| {
            keys.iter()
                .find(|key| key.number == number)
                .map_or_else(Vec::new, |key| key.events.clone())
        };
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
            let shared = studio.sharing_pose(number);
            let is_rest = keys
                .iter()
                .any(|key| key.number == number && key.pose == "rest");
            if ui
                .add_enabled(
                    !shared.is_empty() || is_rest,
                    egui::Button::new("Make this key's pose its own"),
                )
                .on_hover_text(if shared.is_empty() && !is_rest {
                    "No other key uses this pose: a drag already changes only this key".to_owned()
                } else {
                    format!(
                        "A copy of the pose for this key alone, so a drag changes only it. Now also used by: {}",
                        if is_rest && shared.is_empty() {
                            "the model as built (rest can't be changed)".to_owned()
                        } else {
                            shared
                                .iter()
                                .map(|(animation, at)| format!("{animation} key {at}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    )
                })
                .clicked()
            {
                done = Some(studio.own_pose(number));
                ui.close();
            }
            // Events: a name to type and Add event, and each one there
            // with Remove.
            ui.separator();
            ui.label(
                RichText::new("Events: heard by a game when this key is passed")
                    .weak()
                    .small(),
            );
            let mut add = false;
            ui.horizontal(|ui| {
                let label = ui.label("New event");
                let typed = ui
                    .add(
                        egui::TextEdit::singleline(&mut timeline.event_name)
                            .hint_text("footstep")
                            .desired_width(110.0),
                    )
                    .labelled_by(label.id);
                add |= typed.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                add |= ui.button("Add event").clicked();
            });
            if add {
                done = Some(studio.add_event(number, &timeline.event_name));
                if matches!(done, Some(Ok(_))) {
                    timeline.event_name.clear();
                    ui.close();
                }
            }
            for name in events_of(number) {
                if ui.button(format!("Remove \"{name}\"")).clicked() {
                    done = Some(studio.remove_event(number, &name));
                    ui.close();
                }
            }
        });
    }

    // ---- the bar under the ruler: where the part shown is in the whole
    // animation; drag it to scroll.
    let (bar, dragged) = ui.allocate_exact_size(vec2(width, BAR), Sense::drag());
    let painter = ui.painter_at(bar);
    painter.rect_filled(bar, 3.0, Color32::from_black_alpha(60));
    if let Some((start, span)) = timeline.window()
        && span < length
    {
        let left = bar.left() + bar.width() * (start / length) as f32;
        let right = bar.left() + bar.width() * ((start + span) / length) as f32;
        painter.rect_filled(
            Rect::from_min_max(
                pos2(left, bar.top() + 1.0),
                pos2(right.max(left + 6.0), bar.bottom() - 1.0),
            ),
            3.0,
            Color32::from_white_alpha(110),
        );
        if dragged.dragged() {
            timeline.scroll_by(f64::from(dragged.drag_delta().x / bar.width()) * length);
        }
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
            // Until one is chosen the list says so: what it shows is what
            // Pose mode's timeline shows too.
            let chosen = studio.animation().map(str::to_owned);
            let list = egui::ComboBox::from_id_salt("duck_mold_view_animation")
                .width(110.0)
                .selected_text(chosen.clone().unwrap_or_else(|| "choose one".to_owned()))
                .show_ui(ui, |ui| {
                    for name in &names {
                        if ui
                            .selectable_label(chosen.as_ref() == Some(name), name)
                            .clicked()
                        {
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
                // With none chosen, the first plays.
                if studio.animation().is_none() {
                    studio.choose_animation(Some(&names[0]));
                }
                studio.toggle_play();
            }
        });
    });
}
