//! The first-run tour: five small tips, shown once, on the first model
//! a person opens. Each points at the thing it is about.

use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::{Grip, Tool};
use eframe::egui::{self, Color32, Pos2, Rect, RichText};

use crate::theme;

/// What each tip says.
pub const TIPS: [&str; 5] = [
    "Drag to look around. Roll the wheel to come closer.",
    "Click a part to select it.",
    "Drag an arrow to move it. R turns it, S sizes it.",
    "Dots are sockets. Click one to add a part that stays joined there.",
    "Add puts a new shape on the ground.",
];

/// What the person did with a tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Nothing yet.
    Reading,
    /// On to the next tip.
    Next,
    /// No more tips.
    Skip,
}

/// Where on the screen a point of the viewport's picture is.
fn in_window(viewport: (Rect, [usize; 2]), (x, y): (f64, f64)) -> Pos2 {
    let (rect, [width, height]) = viewport;
    rect.min
        + egui::vec2(
            x as f32 * rect.width() / width.max(1) as f32,
            y as f32 * rect.height() / height.max(1) as f32,
        )
}

/// Gets the Studio ready for a tip: the arrows and the dots need a part
/// to be selected, with the Move tool in hand. (Selecting changes
/// nothing in the model.)
pub fn prepare(studio: &mut Studio, tip: usize) {
    if tip >= 2 && !studio.look_only() {
        if studio.selected().is_none() {
            let first = studio.outline().first().map(|row| row.path.clone());
            if let Some(first) = first {
                studio.select(Some(&first));
            }
        }
        if tip == 2 {
            studio.tool = Tool::Move;
        }
    }
}

/// The thing a tip is about, as a point of the screen: the middle of
/// the picture, a part, an arrow, a dot, or the Add button.
fn target(
    studio: &Studio,
    tip: usize,
    viewport: (Rect, [usize; 2]),
    add_button: Option<Rect>,
) -> Option<Pos2> {
    let size = (viewport.1[0], viewport.1[1]);
    let seen = |point| {
        studio
            .orbit
            .on_screen(point, size.0, size.1)
            .map(|at| in_window(viewport, at))
    };
    match tip {
        // Nothing to point at: the whole picture.
        0 => None,
        1 => {
            // The selected part, or else the first.
            let path = studio
                .selected()
                .map(str::to_owned)
                .or_else(|| studio.outline().first().map(|row| row.path.clone()))?;
            let middle = studio.part(&path)?.bounds?.center();
            seen(middle + studio.offset())
        }
        2 => seen(studio.grip_at(Grip::Arrow(0))?),
        3 => {
            // The dot nearest the eye.
            let eye = studio.orbit.eye();
            let offset = studio.offset();
            studio
                .markers()
                .into_iter()
                .map(|marker| marker.at + offset)
                .min_by(|a, b| (*a - eye).length().total_cmp(&(*b - eye).length()))
                .and_then(seen)
        }
        _ => add_button.map(|rect| rect.center_bottom()),
    }
}

/// Shows tip `tip` (counted from 0), with a ring round what it is
/// about and a line to it. Says what the person did.
pub fn show(
    ctx: &egui::Context,
    studio: &Studio,
    tip: usize,
    viewport: (Rect, [usize; 2]),
    add_button: Option<Rect>,
) -> Answer {
    let Some(words) = TIPS.get(tip) else {
        return Answer::Skip;
    };
    let aim = target(studio, tip, viewport, add_button);
    // The tip stands beside what it is about: below and to the right,
    // or wherever there is room in the picture.
    let room = viewport.0.shrink(12.0);
    let size = egui::vec2(300.0, 92.0);
    let at = match aim {
        Some(aim) => {
            let mut at = aim + egui::vec2(36.0, 36.0);
            if at.x + size.x > room.right() {
                at.x = aim.x - 36.0 - size.x;
            }
            if at.y + size.y > room.bottom() {
                at.y = aim.y - 36.0 - size.y;
            }
            egui::pos2(
                at.x.clamp(room.left(), (room.right() - size.x).max(room.left())),
                at.y.clamp(room.top(), (room.bottom() - size.y).max(room.top())),
            )
        }
        None => egui::pos2(room.center().x - size.x / 2.0, room.top() + 24.0),
    };
    let mut answer = Answer::Reading;
    let shown = egui::Window::new("duck_mold_tip")
        .title_bar(false)
        .resizable(false)
        .fixed_pos(at)
        .fixed_size(size)
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(theme::INK_NAVY)
                .stroke(egui::Stroke::new(1.5, theme::DUCK_YELLOW)),
        )
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("Tip {} of {}", tip + 1, TIPS.len()))
                    .small()
                    .color(theme::DUCK_YELLOW),
            );
            ui.label(RichText::new(*words).color(Color32::WHITE));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let last = tip + 1 == TIPS.len();
                if ui
                    .add(theme::primary_button(if last { "Done" } else { "Next" }))
                    .clicked()
                {
                    answer = Answer::Next;
                }
                if !last && ui.button("Skip the tips").clicked() {
                    answer = Answer::Skip;
                }
            });
        });
    // A ring round what the tip is about, and a line from the tip to it.
    if let (Some(aim), Some(shown)) = (aim, shown) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("duck_mold_tip_pointer"),
        ));
        let stroke = egui::Stroke::new(2.0, theme::DUCK_YELLOW);
        painter.circle_stroke(aim, 14.0, stroke);
        let from = shown.response.rect.clamp(aim);
        let along = aim - from;
        if along.length() > 16.0 {
            painter.line_segment([from, aim - along.normalized() * 14.0], stroke);
        }
    }
    answer
}
