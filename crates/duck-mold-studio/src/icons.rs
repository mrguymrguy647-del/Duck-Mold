//! The Studio's icons, drawn with lines and shapes.
//!
//! They need no font or picture files, look the same on every system and
//! are sharp at every interface size. Each is drawn in a square, in one
//! colour, with lines a tenth of the square wide.

use std::f32::consts::{PI, TAU};

use eframe::egui::{
    self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, WidgetInfo, pos2, vec2,
};

use crate::theme;

/// Every icon the Studio draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    // ---- tools
    /// Move.
    Move,
    /// Rotate.
    Rotate,
    /// Scale.
    Scale,
    /// Size.
    Size,
    /// Paint.
    Paint,
    /// Pull.
    Pull,
    /// Inset.
    Inset,
    /// Cut.
    Cut,
    // ---- views
    /// Front.
    Front,
    /// Side.
    Side,
    /// Top.
    Top,
    /// Three quarter.
    ThreeQuarter,
    /// Frame.
    Frame,
    // ---- switches
    /// Grid.
    Grid,
    /// Wire.
    Wire,
    /// Shadow.
    Shadow,
    /// Hits.
    Hits,
    // ---- the top bar
    /// Undo.
    Undo,
    /// Redo.
    Redo,
    /// Add.
    Add,
    /// Export.
    Export,
    /// Picture.
    Picture,
    // ---- small marks
    /// Eye.
    Eye,
    /// Eye shut.
    EyeShut,
    /// Cross.
    Cross,
    /// Check.
    Check,
    /// Warning.
    Warning,
    /// Mistake.
    Mistake,
    /// Folder.
    Folder,
    /// Play.
    Play,
    /// Stop.
    Stop,
    /// Find.
    Find,
    // ---- shapes, for lists of parts
    /// Box.
    Box,
    /// Ball.
    Ball,
    /// Cylinder.
    Cylinder,
    /// Cone.
    Cone,
    /// Wedge.
    Wedge,
    /// Parts.
    Parts,
}

impl Icon {
    /// Every icon, in the order above.
    pub const ALL: [Icon; 38] = [
        Icon::Move,
        Icon::Rotate,
        Icon::Scale,
        Icon::Size,
        Icon::Paint,
        Icon::Pull,
        Icon::Inset,
        Icon::Cut,
        Icon::Front,
        Icon::Side,
        Icon::Top,
        Icon::ThreeQuarter,
        Icon::Frame,
        Icon::Grid,
        Icon::Wire,
        Icon::Shadow,
        Icon::Hits,
        Icon::Undo,
        Icon::Redo,
        Icon::Add,
        Icon::Export,
        Icon::Picture,
        Icon::Eye,
        Icon::EyeShut,
        Icon::Cross,
        Icon::Check,
        Icon::Warning,
        Icon::Mistake,
        Icon::Folder,
        Icon::Play,
        Icon::Stop,
        Icon::Find,
        Icon::Box,
        Icon::Ball,
        Icon::Cylinder,
        Icon::Cone,
        Icon::Wedge,
        Icon::Parts,
    ];

    /// The icon for a shape's name (`box`, `sphere`...), or the one for
    /// a part made of parts.
    pub fn for_shape(shape: &str) -> Icon {
        match shape {
            "box" | "stairs" | "arch" => Icon::Box,
            "sphere" | "torus" | "capsule" => Icon::Ball,
            "cylinder" | "tube" | "prism" => Icon::Cylinder,
            "cone" | "pyramid" => Icon::Cone,
            "wedge" => Icon::Wedge,
            _ => Icon::Parts,
        }
    }
}

/// Draws `icon` in `rect` (a square works best), in `color`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let side = rect.width().min(rect.height());
    let c = rect.center();
    // A point in the icon's own square, from (-1, -1) to (1, 1).
    let at = |x: f32, y: f32| c + vec2(x, y) * side * 0.42;
    let width = (side * 0.1).max(1.2);
    let line = Stroke::new(width, color);
    let thin = Stroke::new((width * 0.6).max(1.0), color);
    let seg = |a: Pos2, b: Pos2| painter.line_segment([a, b], line);
    let arrow_head = |tip: Pos2, from: Pos2| {
        let along = (tip - from).normalized();
        let across = vec2(-along.y, along.x);
        let size = side * 0.18;
        painter.add(Shape::convex_polygon(
            vec![
                tip,
                tip - along * size + across * size * 0.7,
                tip - along * size - across * size * 0.7,
            ],
            color,
            Stroke::NONE,
        ));
    };
    let arrow = |from: Pos2, to: Pos2| {
        let along = (to - from).normalized();
        seg(from, to - along * side * 0.12);
        arrow_head(to, from);
    };
    let arc = |middle: Pos2, radius: f32, start: f32, end: f32, stroke: Stroke| {
        let steps = 20;
        let points: Vec<Pos2> = (0..=steps)
            .map(|step| {
                let angle = start + (end - start) * step as f32 / steps as f32;
                middle + vec2(angle.cos(), angle.sin()) * radius
            })
            .collect();
        painter.add(Shape::line(points, stroke));
    };
    let polygon = |points: Vec<Pos2>, stroke: Stroke| {
        painter.add(Shape::closed_line(points, stroke));
    };
    // A cube drawn as three faces seen from the front right, from above.
    let cube = |lit: Option<usize>| {
        let top = vec![
            at(-0.15, -0.85),
            at(0.75, -0.55),
            at(0.15, -0.25),
            at(-0.75, -0.55),
        ];
        let left = vec![
            at(-0.75, -0.55),
            at(0.15, -0.25),
            at(0.15, 0.85),
            at(-0.75, 0.55),
        ];
        let right = vec![
            at(0.15, -0.25),
            at(0.75, -0.55),
            at(0.75, 0.55),
            at(0.15, 0.85),
        ];
        for (index, face) in [&top, &left, &right].into_iter().enumerate() {
            if lit == Some(index) {
                painter.add(Shape::convex_polygon(
                    face.clone(),
                    color.gamma_multiply(0.55),
                    Stroke::NONE,
                ));
            }
        }
        for face in [top, left, right] {
            polygon(face, thin);
        }
    };
    match icon {
        Icon::Move => {
            arrow(c, at(0.0, -1.0));
            arrow(c, at(0.0, 1.0));
            arrow(c, at(-1.0, 0.0));
            arrow(c, at(1.0, 0.0));
        }
        Icon::Rotate => {
            arc(c, side * 0.34, -PI * 0.9, PI * 0.55, line);
            let end = c + vec2((PI * 0.55).cos(), (PI * 0.55).sin()) * side * 0.34;
            arrow_head(
                end + vec2(-side * 0.12, -side * 0.02),
                end + vec2(side * 0.1, -side * 0.12),
            );
        }
        Icon::Scale => {
            polygon(
                vec![at(-0.9, -0.1), at(0.1, -0.1), at(0.1, 0.9), at(-0.9, 0.9)],
                thin,
            );
            arrow(at(-0.3, 0.3), at(0.9, -0.9));
        }
        Icon::Size => {
            seg(at(-0.9, -0.8), at(-0.9, 0.8));
            seg(at(0.9, -0.8), at(0.9, 0.8));
            arrow(c, at(-0.8, 0.0));
            arrow(c, at(0.8, 0.0));
        }
        Icon::Paint => {
            // A brush: a handle, and a filled tip.
            seg(at(0.85, -0.85), at(0.0, 0.0));
            painter.add(Shape::convex_polygon(
                vec![
                    at(0.1, -0.15),
                    at(0.2, 0.05),
                    at(-0.4, 0.75),
                    at(-0.85, 0.85),
                    at(-0.7, 0.4),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pull => {
            polygon(
                vec![at(-0.9, 0.3), at(0.9, 0.3), at(0.9, 0.9), at(-0.9, 0.9)],
                thin,
            );
            arrow(at(0.0, 0.3), at(0.0, -0.95));
        }
        Icon::Inset => {
            polygon(
                vec![at(-0.9, -0.9), at(0.9, -0.9), at(0.9, 0.9), at(-0.9, 0.9)],
                thin,
            );
            polygon(
                vec![
                    at(-0.45, -0.45),
                    at(0.45, -0.45),
                    at(0.45, 0.45),
                    at(-0.45, 0.45),
                ],
                line,
            );
        }
        Icon::Cut => {
            polygon(
                vec![at(-0.9, -0.6), at(0.9, -0.6), at(0.9, 0.6), at(-0.9, 0.6)],
                thin,
            );
            let dashes = 5;
            for dash in 0..dashes {
                let y0 = -0.95 + dash as f32 * 0.4;
                seg(at(0.0, y0), at(0.0, y0 + 0.22));
            }
        }
        Icon::Front => cube(Some(1)),
        Icon::Side => cube(Some(2)),
        Icon::Top => cube(Some(0)),
        Icon::ThreeQuarter => cube(None),
        Icon::Frame => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = at(x * 0.9, y * 0.9);
                seg(corner, at(x * 0.4, y * 0.9));
                seg(corner, at(x * 0.9, y * 0.4));
            }
            painter.circle_filled(c, side * 0.1, color);
        }
        Icon::Grid => {
            // A grid lying on the ground, seen at a slant.
            let ground = |u: f32, v: f32| at(-0.6 + u * 1.2 + (v - 0.5) * -0.5, -0.3 + v * 0.9);
            for step in 0..=3 {
                let t = step as f32 / 3.0;
                seg(ground(t, 0.0), ground(t, 1.0));
                seg(ground(0.0, t), ground(1.0, t));
            }
        }
        Icon::Wire => {
            let front = [at(-0.8, -0.4), at(0.4, -0.4), at(0.4, 0.8), at(-0.8, 0.8)];
            let back = front.map(|point| point + vec2(side * 0.17, -side * 0.17));
            polygon(front.to_vec(), thin);
            polygon(back.to_vec(), thin);
            for (a, b) in front.iter().zip(back.iter()) {
                painter.line_segment([*a, *b], thin);
            }
        }
        Icon::Shadow => {
            painter.add(Shape::ellipse_filled(
                at(0.0, 0.6),
                vec2(side * 0.38, side * 0.12),
                color.gamma_multiply(0.6),
            ));
            painter.circle_stroke(at(0.0, -0.25), side * 0.26, thin);
        }
        Icon::Hits => {
            // A dashed ring round a dot: a shape a game tests against.
            let dashes = 8;
            for dash in 0..dashes {
                let start = dash as f32 / dashes as f32 * TAU;
                arc(
                    c,
                    side * 0.36,
                    start,
                    start + TAU / dashes as f32 * 0.55,
                    line,
                );
            }
            painter.circle_filled(c, side * 0.1, color);
        }
        Icon::Undo | Icon::Redo => {
            // A hook: back along the top, round, and out along the
            // bottom. Redo is the same, the other way round.
            let flip = if icon == Icon::Undo { 1.0 } else { -1.0 };
            let point = |x: f32, y: f32| at(x * flip, y);
            let radius = 0.45;
            let middle = point(0.25, 0.05);
            let points: Vec<Pos2> = (0..=16)
                .map(|step| {
                    let angle = -PI / 2.0 + PI * step as f32 / 16.0;
                    middle + vec2(angle.cos() * flip, angle.sin()) * radius * side * 0.42
                })
                .collect();
            painter.add(Shape::line(points, line));
            let top = point(0.25, 0.05 - radius);
            let bottom = point(0.25, 0.05 + radius);
            seg(bottom, point(-0.45, 0.05 + radius));
            arrow(top, point(-0.85, 0.05 - radius));
        }
        Icon::Add => {
            seg(at(0.0, -0.85), at(0.0, 0.85));
            seg(at(-0.85, 0.0), at(0.85, 0.0));
        }
        Icon::Export => {
            polygon(
                vec![at(-0.8, -0.2), at(-0.8, 0.9), at(0.8, 0.9), at(0.8, -0.2)],
                thin,
            );
            arrow(at(0.0, 0.5), at(0.0, -0.95));
        }
        Icon::Picture => {
            polygon(
                vec![at(-0.9, -0.7), at(0.9, -0.7), at(0.9, 0.7), at(-0.9, 0.7)],
                thin,
            );
            painter.add(Shape::convex_polygon(
                vec![
                    at(-0.7, 0.55),
                    at(-0.15, -0.15),
                    at(0.25, 0.3),
                    at(0.45, 0.1),
                    at(0.7, 0.55),
                ],
                color,
                Stroke::NONE,
            ));
            painter.circle_filled(at(0.45, -0.35), side * 0.08, color);
        }
        Icon::Eye | Icon::EyeShut => {
            let top: Vec<Pos2> = (0..=16)
                .map(|step| {
                    let t = step as f32 / 16.0;
                    at(-0.95 + t * 1.9, -(t * PI).sin() * 0.55)
                })
                .collect();
            let bottom: Vec<Pos2> = top
                .iter()
                .map(|point| pos2(point.x, 2.0 * c.y - point.y))
                .collect();
            if icon == Icon::Eye {
                painter.add(Shape::line(top, thin));
                painter.add(Shape::line(bottom, thin));
                painter.circle_filled(c, side * 0.16, color);
            } else {
                painter.add(Shape::line(bottom, thin));
                for x in [-0.55, 0.0, 0.55] {
                    seg(at(x, 0.45), at(x * 1.2, 0.75));
                }
            }
        }
        Icon::Cross => {
            seg(at(-0.65, -0.65), at(0.65, 0.65));
            seg(at(0.65, -0.65), at(-0.65, 0.65));
        }
        Icon::Check => {
            painter.add(Shape::line(
                vec![at(-0.75, 0.05), at(-0.2, 0.6), at(0.8, -0.6)],
                line,
            ));
        }
        Icon::Warning => {
            painter.add(Shape::convex_polygon(
                vec![at(0.0, -0.95), at(0.95, 0.8), at(-0.95, 0.8)],
                color,
                Stroke::NONE,
            ));
            let ink = theme::INK_NAVY;
            painter.line_segment([at(0.0, -0.35), at(0.0, 0.25)], Stroke::new(width, ink));
            painter.circle_filled(at(0.0, 0.5), width * 0.6, ink);
        }
        Icon::Mistake => {
            painter.circle_filled(c, side * 0.42, color);
            let ink = theme::INK_NAVY;
            painter.line_segment([at(-0.4, -0.4), at(0.4, 0.4)], Stroke::new(width, ink));
            painter.line_segment([at(0.4, -0.4), at(-0.4, 0.4)], Stroke::new(width, ink));
        }
        Icon::Folder => {
            polygon(
                vec![
                    at(-0.9, -0.6),
                    at(-0.3, -0.6),
                    at(-0.15, -0.4),
                    at(0.9, -0.4),
                    at(0.9, 0.75),
                    at(-0.9, 0.75),
                ],
                thin,
            );
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![at(-0.55, -0.75), at(0.75, 0.0), at(-0.55, 0.75)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            painter.rect_filled(Rect::from_two_pos(at(-0.6, -0.6), at(0.6, 0.6)), 1.0, color);
        }
        Icon::Find => {
            painter.circle_stroke(at(-0.2, -0.2), side * 0.26, line);
            seg(at(0.25, 0.25), at(0.85, 0.85));
        }
        Icon::Box => cube(Some(0)),
        Icon::Ball => {
            painter.circle_stroke(c, side * 0.38, line);
            arc(c, side * 0.38, PI * 0.1, PI * 0.9, thin);
        }
        Icon::Cylinder => {
            painter.add(Shape::ellipse_stroke(
                at(0.0, -0.6),
                vec2(side * 0.3, side * 0.1),
                thin,
            ));
            seg(at(-0.72, -0.6), at(-0.72, 0.6));
            seg(at(0.72, -0.6), at(0.72, 0.6));
            arc(at(0.0, 0.6), side * 0.3, 0.0, PI, thin);
        }
        Icon::Cone => {
            seg(at(0.0, -0.9), at(-0.75, 0.6));
            seg(at(0.0, -0.9), at(0.75, 0.6));
            painter.add(Shape::ellipse_stroke(
                at(0.0, 0.6),
                vec2(side * 0.31, side * 0.1),
                thin,
            ));
        }
        Icon::Wedge => {
            polygon(vec![at(-0.9, 0.7), at(0.9, 0.7), at(0.9, -0.7)], thin);
            polygon(
                vec![at(0.9, -0.7), at(0.9, 0.7), at(0.55, 0.45), at(0.55, -0.85)],
                thin,
            );
        }
        Icon::Parts => {
            for (x, y) in [(-0.45, 0.35), (0.45, 0.35), (0.0, -0.45)] {
                painter.rect_stroke(
                    Rect::from_center_size(at(x, y), Vec2::splat(side * 0.32)),
                    1.0,
                    thin,
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
}

/// An icon on its own, the size of a line of text.
pub fn icon(ui: &mut Ui, icon: Icon, color: Color32) -> Response {
    let side = ui.text_style_height(&egui::TextStyle::Body);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    paint(ui.painter(), rect, icon, color);
    response
}

/// A square button showing only `icon`, called `label` (for its tip
/// and for screen readers and tests). `selected` draws it pressed, as a
/// switch that is on.
pub fn button(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    button_sized(ui, icon, label, selected, 26.0)
}

/// [`button`] at a given size, in points.
pub fn button_sized(ui: &mut Ui, icon: Icon, label: &str, selected: bool, side: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, enabled, selected, label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() || response.has_focus() {
            ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
        }
        let color = if !enabled {
            ui.visuals().weak_text_color()
        } else if selected {
            Color32::WHITE
        } else {
            visuals.text_color()
        };
        paint(ui.painter(), rect.shrink(side * 0.18), icon, color);
    }
    response
}

/// A small button that is only drawn while `shown` (its line is under
/// the mouse, say) or while the mouse is on it: the eye at the end of a
/// part's line, the × at the end of a colour's. It is there all the
/// time to be found, by keyboard and by tests, as `label`.
pub fn quiet_button(ui: &mut Ui, icon: Icon, label: &str, shown: bool) -> Response {
    let side = 20.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label));
    if ui.is_rect_visible(rect) && (shown || response.hovered() || response.has_focus()) {
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let color = if response.hovered() {
            ui.visuals().strong_text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        paint(ui.painter(), rect.shrink(3.0), icon, color);
    }
    response
}

/// A button with an icon before its words.
pub fn labelled(ui: &mut Ui, icon: Icon, words: &str, selected: bool) -> Response {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(words.to_owned(), font, Color32::WHITE);
    let side = galley.size().y.max(16.0);
    let padding = ui.spacing().button_padding;
    let size = vec2(side + 6.0 + galley.size().x, side) + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(egui::Role::Button, enabled, selected, words));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        ui.painter().rect_filled(rect, 4.0, visuals.weak_bg_fill);
        let color = if enabled {
            visuals.text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        let icon_rect = Rect::from_min_size(rect.min + padding, Vec2::splat(side));
        paint(ui.painter(), icon_rect.shrink(1.0), icon, color);
        let galley = ui.painter().layout_no_wrap(
            words.to_owned(),
            egui::TextStyle::Button.resolve(ui.style()),
            color,
        );
        ui.painter().galley(
            pos2(
                icon_rect.max.x + 6.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            color,
        );
    }
    response
}
