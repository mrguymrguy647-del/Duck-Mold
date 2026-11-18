//! The viewport: the picture of the model, and the mouse on it.
//!
//! - Drag turns the view; right-drag (or middle-drag, or Shift and drag)
//!   slides it; the wheel zooms.
//! - A click selects the part under the pointer. Shift+click adds a
//!   part to the selection, or takes it out. A right-click opens the
//!   menu for the part under the pointer.
//! - With a part selected, its sockets show as dots, and the handles of
//!   the tool in hand: arrows, rings or boxes. A click on a dot opens a
//!   short list of what to add there.
//! - A drag that starts on a handle, or on the selected part, moves,
//!   turns or sizes the part. While it lasts the picture shows what the
//!   recipe would build; the file is written when the mouse is let go.
//! - In Face mode (Tab, or the Face button above the tools) a click
//!   picks a face of the selected box, and the handles shape it: the
//!   Pull arrow, the Inset knob. A drag that would break the shape shows
//!   its label in red, and letting go writes nothing.
//!
//! The picture is drawn by Duck Forge's CPU renderer
//! ([`Studio::picture`]) and shown as one image.

use crate::actions::{self, Action};
use crate::icons::{self, Icon};
use crate::library::Thumbs;
use crate::theme;
use duck_mold_core::edit::{Made, Spot};
use duck_mold_core::recipe::Shape;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::viewport::Orbit;
use duck_mold_view::studio::{Clicked, FaceTool, Mode, Studio};
use eframe::egui::{
    self, Color32, ColorImage, PointerButton, Rect, RichText, Sense, TextureHandle, TextureOptions,
    pos2,
};

/// The most pixels the viewport is drawn with. On a very large or very
/// sharp screen the picture is drawn smaller and stretched, so turning
/// the model stays smooth.
const MOST_PIXELS: usize = 2_500_000;

/// How far the wheel must roll for one step of zoom, in points.
const WHEEL_STEP: f32 = 50.0;

/// What the viewport's picture was last drawn from. When none of it has
/// changed, the picture is still right and isn't drawn again.
#[derive(Clone, PartialEq)]
struct Drawn {
    builds: usize,
    orbit: Orbit,
    grid: bool,
    wire: bool,
    shadow: bool,
    hits: bool,
    selected: Vec<String>,
    hidden: Vec<String>,
    tool: Tool,
    mode: Mode,
    face_tool: FaceTool,
    faces: Vec<String>,
    size: [usize; 2],
    /// Where the pointer is, when that changes what is drawn (a part is
    /// selected, so markers show).
    pointer: Option<(i32, i32)>,
    dragging: bool,
    /// The pose shown, the time on the timeline (in milliseconds),
    /// whether it plays, and the ghosts.
    pose: String,
    time: Option<i64>,
    playing: bool,
    ghosts: bool,
}

/// Where a part is about to be added.
#[derive(Clone, PartialEq)]
enum Place {
    /// On a socket of a part.
    Socket { part: String, socket: String },
    /// On the ground, joined to nothing.
    Ground,
}

/// The "add a part here" list, while it is open.
struct Adding {
    place: Place,
    /// Where on the screen it was asked for.
    at: egui::Pos2,
}

/// The viewport's own state: its picture, and what the mouse is doing.
#[derive(Default)]
pub struct View {
    texture: Option<TextureHandle>,
    drawn: Option<Drawn>,
    /// Where on the screen it was last drawn.
    rect: Option<Rect>,
    /// True while the selected part is being dragged to a new place.
    moving: bool,
    adding: Option<Adding>,
    /// The point of a part's surface the right-click menu was opened
    /// at: where "Add a part here" adds.
    menu_spot: Option<(String, Spot)>,
    /// How long the last picture took, in milliseconds, as if it had been
    /// full size.
    last_frame_ms: f32,
    /// Every frame of a drag is a half-size draft, as on a slow
    /// computer: for tests.
    pub(crate) always_draft: bool,
}

/// Above how many milliseconds a frame is too slow while dragging, and
/// the next ones are drafts.
pub const DRAFT_ABOVE_MS: f32 = 12.0;

impl View {
    /// Where on the screen the viewport was last drawn, and how many
    /// pixels its picture has.
    pub fn place(&self) -> Option<(Rect, [usize; 2])> {
        Some((self.rect?, self.drawn.as_ref()?.size))
    }

    /// Forgets the picture, so it is drawn again (another file was
    /// opened).
    pub fn reset(&mut self) {
        self.drawn = None;
        self.moving = false;
        self.adding = None;
    }

    /// True while the "add a part here" list is open.
    pub fn is_adding(&self) -> bool {
        self.adding.is_some()
    }

    /// Closes the "add a part here" list.
    pub fn stop_adding(&mut self) {
        self.adding = None;
    }

    /// Opens the list of what can be added on the ground, in the middle
    /// of the viewport: Shift+A.
    pub fn add_on_ground(&mut self) {
        let at = self.rect.map_or(pos2(200.0, 120.0), |rect| {
            rect.center() - egui::vec2(90.0, 150.0)
        });
        self.adding = Some(Adding {
            place: Place::Ground,
            at,
        });
    }

    /// The point of a part the right-click menu was opened at, once.
    pub fn take_menu_spot(&mut self) -> Option<(String, Spot)> {
        self.menu_spot.take()
    }

    /// Forgets that point: the menu was opened from the parts list.
    pub fn clear_menu_spot(&mut self) {
        self.menu_spot = None;
    }
}

/// How many pixels the viewport's picture has: one for each of the
/// screen's, unless that would be too many to draw quickly.
fn picture_size(ui: &egui::Ui, rect: Rect) -> (usize, usize) {
    let sharpness = ui.ctx().pixels_per_point();
    let mut width = (rect.width() * sharpness).round() as usize;
    let mut height = (rect.height() * sharpness).round() as usize;
    while width * height > MOST_PIXELS {
        width /= 2;
        height /= 2;
    }
    (width.max(1), height.max(1))
}

/// Draws the viewport and handles the mouse on it. Gives what was asked
/// for in its right-click menu, if something was.
pub fn show(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    view: &mut View,
    thumbs: &mut Thumbs,
) -> Option<Action> {
    let rect = ui.available_rect_before_wrap();
    view.rect = Some(rect);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    if rect.width() < 2.0 || rect.height() < 2.0 {
        return None;
    }
    // While the picture is dragged (the view turned or slid, a handle
    // pulled) and the last frame was slow, it is drawn smaller and
    // stretched: a draft. The moment the drag stops it is full size.
    let slow = view.always_draft || view.last_frame_ms > DRAFT_ABOVE_MS;
    let draft = if (response.dragged() || view.moving) && slow {
        if view.always_draft || view.last_frame_ms > DRAFT_ABOVE_MS * 1.8 {
            0.5
        } else {
            0.75
        }
    } else {
        1.0
    };
    let full = picture_size(ui, rect);
    let size = if draft < 1.0 {
        (
            ((full.0 as f32 * draft) as usize).max(1),
            ((full.1 as f32 * draft) as usize).max(1),
        )
    } else {
        full
    };
    // A point of the screen in the picture's pixels at full size,
    // whatever size a draft is drawn at: the pointer is measured the same
    // way all through a drag, also when the frames turn into drafts
    // halfway through it.
    let in_picture = |pointer: egui::Pos2| {
        let across = f64::from((pointer.x - rect.min.x) / rect.width());
        let along = f64::from((pointer.y - rect.min.y) / rect.height());
        (across * full.0 as f64, along * full.1 as f64)
    };

    let pointer = response
        .hover_pos()
        .or_else(|| response.interact_pointer_pos());
    studio.hover(full, pointer.map(in_picture));

    // A drag that starts on a handle, or on the selected part, moves the
    // part. Any other drag turns or slides the view.
    let (shift, free) = ui.input(|input| (input.modifiers.shift, input.modifiers.command));
    if response.drag_started_by(PointerButton::Primary)
        && !shift
        && let Some(pressed) = ui.input(|input| input.pointer.press_origin())
    {
        view.moving = studio.grab(full, in_picture(pressed)).is_some();
        if view.moving {
            view.adding = None;
        }
    }
    if view.moving {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            studio.cancel_drag();
            view.moving = false;
        } else if response.drag_stopped() {
            // What it did, or why not, is in the status line.
            let _ = studio.release();
            view.moving = false;
        } else if let Some(pointer) = pointer {
            // Ctrl held: nothing snaps.
            studio.drag(full, in_picture(pointer), free);
        }
    } else {
        let moved = response.drag_delta();
        let (right, down) = (f64::from(moved.x), f64::from(moved.y));
        if response.dragged_by(PointerButton::Primary) && !shift {
            studio.orbit.turn(right, down);
        } else if response.dragged_by(PointerButton::Primary)
            || response.dragged_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Middle)
        {
            studio.orbit.pan(right, down, rect.height() as usize);
        }
    }

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        if studio.mode() == Mode::Pose {
            // Pose mode: the part under the pointer, as it stands in the
            // pose, is selected.
            studio.click(full, in_picture(pointer));
            view.adding = None;
        } else if studio.is_shaping() && studio.face_tool == FaceTool::Cut {
            // The Cut tool: cut where the ring is. What it did, or why
            // not, is in the status line.
            let _ = studio.cut_at(full, in_picture(pointer), free);
            view.adding = None;
        } else if studio.is_shaping() {
            // Face or Edge mode: the face (or edge) under the pointer is
            // selected; with Shift, added or taken out.
            studio.click_face(full, in_picture(pointer), shift);
            view.adding = None;
        } else if studio.tool == Tool::Paint && !studio.look_only() {
            // The Paint tool: the face under the pointer gets the colour
            // in hand; with Shift, the whole part does. What it did, or
            // why not, is in the status line.
            let _ = studio.paint_at(full, in_picture(pointer), shift);
            view.adding = None;
        } else if shift {
            // Shift+click: the part joins the selection, or leaves it.
            studio.click_adding(full, in_picture(pointer));
            view.adding = None;
        } else {
            view.adding = match studio.point_at(full, in_picture(pointer)) {
                Clicked::Socket { part, socket } => Some(Adding {
                    place: Place::Socket { part, socket },
                    at: pointer,
                }),
                Clicked::Selected(_) | Clicked::Nothing => None,
            };
        }
    }
    // A right-click (not a right-drag, which slides the view) opens the
    // menu for the part under the pointer. A part that isn't selected
    // becomes the selection first.
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let at = in_picture(pointer);
        view.menu_spot = None;
        if let Some(part) = studio.part_under(full, at) {
            if !studio.is_selected(&part) {
                studio.select(Some(&part));
            }
            view.menu_spot = studio.spot_at(full, at);
        }
        view.adding = None;
    }
    let mut asked = None;
    response.context_menu(|ui| asked = actions::part_menu(ui, studio, thumbs));
    if response.hovered() {
        let rolled = ui.input(|input| input.smooth_scroll_delta.y);
        if rolled != 0.0 {
            studio.orbit.zoom(f64::from(rolled / WHEEL_STEP));
        }
    }

    let shaping = studio.mode() != Mode::Part;
    let posing = studio.mode() == Mode::Pose;
    let shows_markers = (studio.selected().is_some() && !studio.look_only()) || shaping;
    let now = Drawn {
        builds: studio.builds(),
        orbit: studio.orbit,
        grid: studio.grid,
        wire: studio.wire,
        shadow: studio.shadow,
        hits: studio.hits,
        selected: studio.selection().into_iter().map(str::to_owned).collect(),
        hidden: studio.hidden().to_vec(),
        tool: studio.tool,
        mode: studio.mode(),
        face_tool: studio.face_tool,
        faces: studio.selected_faces().to_vec(),
        size: [size.0, size.1],
        pointer: pointer
            .filter(|_| shows_markers || studio.tool == Tool::Paint)
            .map(in_picture)
            .map(|(x, y)| (x.round() as i32, y.round() as i32)),
        dragging: studio.dragging().is_some(),
        pose: studio.pose().to_owned(),
        time: studio.time().map(|at| (at * 1000.0).round() as i64),
        playing: studio.is_playing(),
        ghosts: studio.ghosts(),
    };
    // While a handle is held the picture changes with every move.
    if view.drawn.as_ref() != Some(&now) || view.texture.is_none() || view.moving {
        let started = std::time::Instant::now();
        let picture = studio.picture(size.0, size.1);
        // A full frame's time decides whether drafts are needed; a
        // draft's, scaled up, whether they still are.
        let took = started.elapsed().as_secs_f32() * 1000.0;
        view.last_frame_ms = took / (draft * draft);
        let image = ColorImage::new(
            now.size,
            picture
                .pixels
                .iter()
                .map(|pixel| {
                    Color32::from_rgb((pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8)
                })
                .collect(),
        );
        match &mut view.texture {
            Some(texture) => texture.set(image, TextureOptions::NEAREST),
            None => {
                view.texture = Some(ui.ctx().load_texture(
                    "duck_mold_viewport",
                    image,
                    TextureOptions::NEAREST,
                ));
            }
        }
        view.drawn = Some(now);
    }
    if let Some(texture) = &view.texture {
        ui.painter_at(rect).image(
            texture.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // What the mouse does here, in a line at the bottom, and how moves
    // snap.
    let handles = (shows_markers && studio.facts().is_some() && !shaping)
        || (shaping && !posing && !studio.selected_faces().is_empty())
        || (posing && !studio.grips().is_empty());
    let why_not = studio.why_no_pose_handles();
    let hint = if view.moving {
        "Let go to make the change. Escape lets go without changing anything."
    } else if posing && studio.selected().is_none() {
        "Click a part to turn it in the pose. Pick the pose on the right. O goes back to parts."
    } else if let Some(why) = why_not.as_deref().filter(|_| posing) {
        why
    } else if posing && studio.pose_tool == duck_mold_view::studio::PoseTool::Move {
        "Drag an arrow to move the part in the pose: what hangs from it goes too."
    } else if posing {
        "Drag a ring to turn the part about its join. It stops at its limit. R: rings, G: arrows."
    } else if shaping && studio.face_tool == FaceTool::Cut {
        "Point at an edge: the ring shows where the cut goes, all the way round. Click to cut. Ctrl: not in tenths."
    } else if studio.mode() == Mode::Corner && studio.selected_faces().is_empty() {
        "Click a corner of the box (a dot) to move it. Shift+click adds a corner. C goes back to parts."
    } else if studio.mode() == Mode::Corner {
        "Drag an arrow to move the corner: the faces round it follow, bending if they must."
    } else if studio.mode() == Mode::Edge && studio.selected_faces().is_empty() {
        "Click an edge of the box to move it. Shift+click adds an edge. K cuts. E goes back to parts."
    } else if studio.mode() == Mode::Edge {
        "Drag an arrow to move the edge: the faces on both sides follow. Shift+click adds an edge."
    } else if shaping && studio.selected_faces().is_empty() {
        "Click a face of the box to shape it. Shift+click adds a face. Tab goes back to parts."
    } else if shaping && studio.face_tool == FaceTool::Inset {
        "Drag the knob in to make a smaller face inside: a frame round it. Shift+click adds a face."
    } else if shaping && studio.face_tool == FaceTool::Move {
        "Drag an arrow to slide the face: the faces round it stretch. Shift+click adds a face."
    } else if shaping && studio.face_tool == FaceTool::Turn {
        "Drag a ring to turn the face about its middle. Shift+click adds a face."
    } else if shaping && studio.face_tool == FaceTool::Scale {
        "Drag a box to make the face bigger or smaller one way, or the middle one for all. Shift+click adds a face."
    } else if shaping {
        "Drag the arrow to pull the face out, or push it in. Shift+click adds a face. G, R, S, I: other tools."
    } else if studio.tool == Tool::Paint && !studio.look_only() {
        "Click a face to colour it. Shift+click colours the whole part. Pick the colour in the palette."
    } else if studio.selection().len() > 1 {
        "Drag a handle, or one of the parts, to change them together. Shift+click adds or takes out a part."
    } else if handles {
        "Drag an arrow, a ring or a box, or the part itself. Click a dot to add a part there. Right-click for more."
    } else {
        "Drag to turn, right-drag to slide, roll the wheel to zoom. Click a part to select it."
    };
    let quiet = Color32::from_white_alpha(150);
    let painter = ui.painter_at(rect);
    // The snap on the right; the hint on the left, wrapped onto a second
    // line rather than running into it on a narrow viewport.
    let mut room = rect.width() - 16.0;
    if handles {
        let snap = painter.text(
            rect.right_bottom() + egui::vec2(-8.0, -6.0),
            egui::Align2::RIGHT_BOTTOM,
            studio.snap_text(),
            egui::FontId::proportional(12.0),
            quiet,
        );
        room -= snap.width() + 16.0;
    }
    let hint = painter.layout(
        hint.to_owned(),
        egui::FontId::proportional(12.0),
        quiet,
        room.max(120.0),
    );
    painter.galley(
        rect.left_bottom() + egui::vec2(8.0, -6.0 - hint.size().y),
        hint,
        quiet,
    );
    // Where the Cut tool would cut, beside the pointer.
    if let (Some(label), Some(pointer)) = (studio.cut_label(), pointer) {
        let at = pointer + egui::vec2(16.0, -18.0);
        let text = painter.layout_no_wrap(label, egui::FontId::proportional(13.0), Color32::WHITE);
        let back = Rect::from_min_size(at, text.size()).expand(4.0);
        painter.rect_filled(back, 4.0, Color32::from_black_alpha(200));
        painter.galley(at, text, Color32::WHITE);
    }
    // What the drag has done so far, beside the pointer: in red, with
    // why, when letting go would change nothing.
    if let (Some(label), Some(pointer)) = (studio.drag_label(), pointer) {
        let at = pointer + egui::vec2(16.0, -18.0);
        let (words, color) = match studio.drag_problem() {
            Some(why) => (
                format!("{label}: {why}"),
                Color32::from_rgb(0xFF, 0x7A, 0x7A),
            ),
            None => (label.to_owned(), Color32::WHITE),
        };
        let text = painter.layout(words, egui::FontId::proportional(13.0), color, 320.0);
        let back = Rect::from_min_size(at, text.size()).expand(4.0);
        painter.rect_filled(back, 4.0, Color32::from_black_alpha(200));
        painter.galley(at, text, color);
    }

    mode_row(ui, studio, rect);
    tool_strip(ui, studio, rect);
    if let Some(action) = view_corner(ui, studio, rect) {
        asked = Some(action);
    }
    if studio.mode() == Mode::Part {
        crate::timeline::play_button(ui, studio, rect);
    }
    add_list(ui, studio, view, thumbs, rect);
    asked
}

/// Where the tools start, under the mode row.
const TOOLS_TOP: f32 = 44.0;

/// Part and Face, above the tools: what a click picks.
fn mode_row(ui: &mut egui::Ui, studio: &mut Studio, within: Rect) {
    if studio.look_only() {
        return;
    }
    let modes = [
        (
            Mode::Part,
            "Part",
            "Click parts, and move, turn and size them (Tab)",
        ),
        (
            Mode::Face,
            "Face",
            "Shape the selected box: click its faces, then pull, move, turn, scale or inset them (Tab)",
        ),
        (
            Mode::Edge,
            "Edge",
            "The selected box's edges: click one and move it, or cut round the box (E)",
        ),
        (
            Mode::Corner,
            "Corner",
            "The selected box's corners: click one and move it (C)",
        ),
        (
            Mode::Pose,
            "Pose",
            "The model in its poses: click a part and turn it about its join (O)",
        ),
    ];
    for (at, (mode, name, tip)) in modes.into_iter().enumerate() {
        let widths = [44.0, 44.0, 44.0, 56.0, 48.0];
        let left: f32 = widths[..at].iter().sum::<f32>() + at as f32 * 4.0;
        let place = Rect::from_min_size(
            within.left_top() + egui::vec2(8.0 + left, 8.0),
            egui::vec2(widths[at], 24.0),
        );
        ui.painter_at(within)
            .rect_filled(place, 4.0, Color32::from_black_alpha(150));
        // Why it can't be chosen now, if it can't: said on the button,
        // where the person looks.
        let why_not = if studio.mode() == mode {
            None
        } else {
            match mode {
                Mode::Part => None,
                Mode::Face | Mode::Edge | Mode::Corner => studio.why_not_shaped(),
                Mode::Pose => studio.why_not_posed(),
            }
        };
        // On one line, however narrow the button.
        let button = egui::Button::selectable(studio.mode() == mode, name)
            .wrap_mode(egui::TextWrapMode::Extend);
        let clicked = ui
            .put(place, |ui: &mut egui::Ui| {
                ui.add_enabled(why_not.is_none(), button)
                    .on_hover_text(tip)
                    .on_disabled_hover_text(why_not.as_deref().unwrap_or(tip))
            })
            .clicked();
        if clicked && studio.mode() != mode {
            // Why not, if it can't, is in the status line.
            let _ = match mode {
                Mode::Part => {
                    studio.part_mode();
                    Ok(String::new())
                }
                Mode::Face => studio.face_mode(),
                Mode::Edge => studio.edge_mode(),
                Mode::Corner => studio.corner_mode(),
                Mode::Pose => studio.pose_mode(),
            };
        }
    }
}

/// The colours of the gizmo's three axes: x red, y green, z blue, as
/// the grid's lines are.
const GIZMO_X: Color32 = Color32::from_rgb(0xE5, 0x5A, 0x64);
const GIZMO_Y: Color32 = Color32::from_rgb(0x6C, 0xC2, 0x5A);
const GIZMO_Z: Color32 = Color32::from_rgb(0x5A, 0x8C, 0xE5);

/// Where the gizmo's middle is, from the picture's top right corner.
const GIZMO_AT: egui::Vec2 = egui::vec2(-46.0, 86.0);
const GIZMO_REACH: f32 = 30.0;
/// How wide the column of views under the gizmo is.
const VIEWS_WIDTH: f32 = 84.0;

/// The top right of the picture: the axis gizmo (a click on an axis
/// looks along it), the four views and Frame under it; and along the
/// bottom right the switches (Grid, Wire, Shadow, Hits) and how many
/// parts are hidden. Gives what was asked for.
fn view_corner(ui: &mut egui::Ui, studio: &mut Studio, within: Rect) -> Option<Action> {
    use duck_mold_view::studio::viewport::Viewpoint;
    let mut asked = None;
    let painter = ui.painter_at(within);

    // ---- the gizmo: x, y and z as the view sees them
    let middle = within.right_top() + GIZMO_AT;
    painter.circle_filled(middle, GIZMO_REACH + 12.0, Color32::from_black_alpha(110));
    let (round, above) = (
        studio.orbit.round.to_radians() as f32,
        studio.orbit.above.to_radians() as f32,
    );
    // How each axis lies on the screen, and how near the eye its end is.
    let axes = [
        (
            "x",
            GIZMO_X,
            egui::vec2(round.cos(), round.sin() * above.sin()),
            round.sin() * above.cos(),
            Viewpoint::Side,
        ),
        (
            "y",
            GIZMO_Y,
            egui::vec2(0.0, -above.cos()),
            above.sin(),
            Viewpoint::Top,
        ),
        (
            "z",
            GIZMO_Z,
            egui::vec2(-round.sin(), round.cos() * above.sin()),
            round.cos() * above.cos(),
            Viewpoint::Front,
        ),
    ];
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by(|a, b| axes[*a].3.total_cmp(&axes[*b].3));
    for at in order {
        let (name, colour, along, _, from) = axes[at];
        let end = middle + along * GIZMO_REACH;
        // The other way, faintly.
        painter.circle_filled(
            middle - along * GIZMO_REACH,
            3.0,
            colour.gamma_multiply(0.45),
        );
        painter.line_segment([middle, end], egui::Stroke::new(2.0, colour));
        let knob = Rect::from_center_size(end, egui::Vec2::splat(18.0));
        let response = ui.interact(knob, ui.scope_id().with(("gizmo", name)), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Look along {name}"))
        });
        painter.circle_filled(end, if response.hovered() { 9.0 } else { 8.0 }, colour);
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::proportional(12.0),
            theme::INK_NAVY,
        );
        if response
            .on_hover_text(format!("Look along {name}: {}", from.name()))
            .clicked()
        {
            asked = Some(Action::LookFrom(from));
        }
    }

    // ---- the four views and Frame, in a column under it
    let views = [
        (
            Viewpoint::Front,
            Icon::Front,
            "Front",
            "Look at the front (1)",
        ),
        (
            Viewpoint::Side,
            Icon::Side,
            "Side",
            "Look at the right side (2)",
        ),
        (
            Viewpoint::Top,
            Icon::Top,
            "Top",
            "Look down from the top (3)",
        ),
        (
            Viewpoint::ThreeQuarter,
            Icon::ThreeQuarter,
            "3/4",
            "Look from the front right, a little above (4)",
        ),
    ];
    let column = Rect::from_min_size(
        pos2(
            within.right() - 8.0 - VIEWS_WIDTH,
            middle.y + GIZMO_REACH + 20.0,
        ),
        egui::vec2(VIEWS_WIDTH, 5.0 * 28.0 + 8.0),
    );
    painter.rect_filled(column, 6.0, Color32::from_black_alpha(150));
    ui.scope_builder(egui::UiBuilder::new().max_rect(column.shrink(4.0)), |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.spacing_mut().button_padding = egui::vec2(4.0, 3.0);
        for (from, icon, name, tip) in views {
            if icons::labelled(ui, icon, name, false)
                .on_hover_text(tip)
                .clicked()
            {
                asked = Some(Action::LookFrom(from));
            }
        }
        if icons::labelled(ui, Icon::Frame, "Frame", false)
            .on_hover_text("Bring what is selected, or the whole model, into view (F)")
            .clicked()
        {
            asked = Some(Action::Frame);
        }
    });

    // ---- the switches, along the bottom right
    let switches = 4.0 * 30.0 + 4.0;
    let row = Rect::from_min_size(
        pos2(
            within.right() - 8.0 - switches,
            within.bottom() - 30.0 - 34.0,
        ),
        egui::vec2(switches, 30.0),
    );
    painter.rect_filled(row, 6.0, Color32::from_black_alpha(150));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row.shrink2(egui::vec2(2.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let switch = |ui: &mut egui::Ui, on: &mut bool, icon: Icon, name: &str, tip: &str| {
                if icons::button(ui, icon, name, *on).on_hover_text(tip).clicked() {
                    *on = !*on;
                }
            };
            switch(ui, &mut studio.grid, Icon::Grid, "Grid", "The grid on the ground: one square is one metre");
            switch(
                ui,
                &mut studio.wire,
                Icon::Wire,
                "Wire",
                "Wireframe: only the outline of each face, to see through the model (Z)",
            );
            switch(
                ui,
                &mut studio.shadow,
                Icon::Shadow,
                "Shadow",
                "Darken the ground under the model, darkest where something stands on it",
            );
            switch(
                ui,
                &mut studio.hits,
                Icon::Hits,
                "Hits",
                "Hit shapes: the box, ball or capsule round each part that a game tests what was hit with",
            );
        },
    );
    if studio.hidden_count() > 0 {
        let words = format!("{} hidden", studio.hidden_count());
        let place = Rect::from_min_size(
            pos2(row.left() - 96.0, row.top() + 3.0),
            egui::vec2(90.0, 24.0),
        );
        if ui
            .put(place, egui::Button::new(words))
            .on_hover_text("Parts that aren't drawn. They are still in the model. Click to show them all (Shift+H)")
            .clicked()
        {
            asked = Some(Action::ShowAll);
        }
    }
    asked
}

/// One tool on the toolbar: what it is, its icon and name, its key, and
/// what it does.
struct ToolRow<T> {
    tool: T,
    icon: Icon,
    name: &'static str,
    key: &'static str,
    tip: &'static str,
}

/// How tall a row of the toolbar is, and how wide the toolbar.
const TOOL_ROW: f32 = 28.0;
const TOOLBAR_WIDTH: f32 = 112.0;

/// Draws the toolbar's rows on one dark plate, down the left of the
/// picture. Gives the tool clicked.
fn toolbar<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    within: Rect,
    rows: &[ToolRow<T>],
    in_hand: T,
) -> Option<T> {
    let plate = Rect::from_min_size(
        within.left_top() + egui::vec2(8.0, TOOLS_TOP),
        egui::vec2(TOOLBAR_WIDTH, rows.len() as f32 * TOOL_ROW + 8.0),
    );
    ui.painter_at(within)
        .rect_filled(plate, 6.0, Color32::from_black_alpha(170));
    let mut chosen = None;
    for (at, row) in rows.iter().enumerate() {
        let place = Rect::from_min_size(
            plate.left_top() + egui::vec2(4.0, 4.0 + at as f32 * TOOL_ROW),
            egui::vec2(TOOLBAR_WIDTH - 8.0, TOOL_ROW - 2.0),
        );
        let response = ui.interact(
            place,
            ui.scope_id().with(("tool", row.name)),
            Sense::click(),
        );
        let selected = row.tool == in_hand;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, true, selected, row.name)
        });
        let painter = ui.painter_at(within);
        if selected {
            painter.rect_filled(place, 4.0, theme::NIGHT_INDIGO);
        } else if response.hovered() {
            painter.rect_filled(place, 4.0, Color32::from_white_alpha(24));
        }
        let ink = if selected {
            Color32::WHITE
        } else {
            Color32::from_gray(220)
        };
        let icon_place = Rect::from_min_size(
            place.left_top() + egui::vec2(5.0, 4.0),
            egui::Vec2::splat(place.height() - 8.0),
        );
        icons::paint(&painter, icon_place, row.icon, ink);
        painter.text(
            pos2(icon_place.right() + 8.0, place.center().y),
            egui::Align2::LEFT_CENTER,
            row.name,
            egui::FontId::proportional(13.0),
            ink,
        );
        painter.text(
            pos2(place.right() - 6.0, place.center().y),
            egui::Align2::RIGHT_CENTER,
            row.key,
            egui::FontId::proportional(12.0),
            if selected {
                Color32::from_white_alpha(200)
            } else {
                Color32::from_gray(150)
            },
        );
        if response
            .on_hover_text(format!("{} ({})", row.tip, row.key))
            .clicked()
        {
            chosen = Some(row.tool);
        }
    }
    chosen
}

/// The tools, down the left edge of the viewport: one is in hand.
fn tool_strip(ui: &mut egui::Ui, studio: &mut Studio, within: Rect) {
    if studio.look_only() {
        return;
    }
    if studio.mode() == Mode::Pose {
        use duck_mold_view::studio::PoseTool;
        let rows = [
            ToolRow {
                tool: PoseTool::Turn,
                icon: Icon::Rotate,
                name: "Turn",
                key: "R",
                tip: "Rings at the join: drag one to turn the part in the pose. It stops at its limit",
            },
            ToolRow {
                tool: PoseTool::Move,
                icon: Icon::Move,
                name: "Move",
                key: "G",
                tip: "Arrows: drag one to move the part in the pose (a crouch, a jump)",
            },
        ];
        if let Some(tool) = toolbar(ui, within, &rows, studio.pose_tool) {
            studio.pose_tool = tool;
        }
        return;
    }
    if studio.mode() != Mode::Part {
        let face_tools = [
            ToolRow {
                tool: FaceTool::Pull,
                icon: Icon::Pull,
                name: "Pull",
                key: "T",
                tip: "An arrow out of the faces selected: drag it to pull them out, or push them in",
            },
            ToolRow {
                tool: FaceTool::Move,
                icon: Icon::Move,
                name: "Move",
                key: "G",
                tip: "Three arrows: slide the faces along the part's own directions; the faces round them stretch",
            },
            ToolRow {
                tool: FaceTool::Turn,
                icon: Icon::Rotate,
                name: "Turn",
                key: "R",
                tip: "Three rings: turn the faces about their middle",
            },
            ToolRow {
                tool: FaceTool::Scale,
                icon: Icon::Scale,
                name: "Scale",
                key: "S",
                tip: "Three boxes, and one in the middle: make the faces bigger or smaller",
            },
            ToolRow {
                tool: FaceTool::Inset,
                icon: Icon::Inset,
                name: "Inset",
                key: "I",
                tip: "A knob on an edge: drag it in to make a smaller face inside, with a frame round it",
            },
            ToolRow {
                tool: FaceTool::Cut,
                icon: Icon::Cut,
                name: "Cut",
                key: "K",
                tip: "A ring round the part where you point: click to cut there, so there are more faces to shape",
            },
        ];
        // Edge mode has Move and Cut; Corner mode, Move.
        let rows: Vec<_> = face_tools
            .into_iter()
            .filter(|row| match studio.mode() {
                Mode::Edge => matches!(row.tool, FaceTool::Move | FaceTool::Cut),
                Mode::Corner => row.tool == FaceTool::Move,
                _ => true,
            })
            .collect();
        if let Some(tool) = toolbar(ui, within, &rows, studio.face_tool) {
            studio.face_tool = tool;
        }
        return;
    }
    let rows = [
        ToolRow {
            tool: Tool::Move,
            icon: Icon::Move,
            name: "Move",
            key: "G",
            tip: "Arrows: drag one to move the part",
        },
        ToolRow {
            tool: Tool::Rotate,
            icon: Icon::Rotate,
            name: "Rotate",
            key: "R",
            tip: "Rings: drag one to turn the part",
        },
        ToolRow {
            tool: Tool::Scale,
            icon: Icon::Scale,
            name: "Scale",
            key: "S",
            tip: "Boxes: drag one to make the part bigger or smaller",
        },
        ToolRow {
            tool: Tool::Size,
            icon: Icon::Size,
            name: "Size",
            key: "T",
            tip: "Knobs on a shape's faces: pull one to change its width, height, depth or radius",
        },
        ToolRow {
            tool: Tool::Paint,
            icon: Icon::Paint,
            name: "Paint",
            key: "P",
            tip: "Click a face to colour it with the colour picked in the palette. Shift+click colours the whole part",
        },
    ];
    if let Some(tool) = toolbar(ui, within, &rows, studio.tool) {
        studio.tool = tool;
    }
}

/// The short list that opens where a part is to be added: the five
/// shapes, the parts the recipe defines, and the parts of the library.
fn add_list(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    view: &mut View,
    thumbs: &mut Thumbs,
    within: Rect,
) {
    let Some(adding) = &view.adding else {
        return;
    };
    let title = match &adding.place {
        Place::Socket { part, socket } => format!("Add a part on {part}.{socket}"),
        Place::Ground => "Add a part on the ground".to_owned(),
    };
    let mut chosen: Option<Made> = None;
    let mut open = true;
    // Kept inside the viewport, beside the click.
    let at = pos2(
        (adding.at.x + 12.0).min(within.right() - 330.0),
        (adding.at.y + 12.0)
            .min(within.bottom() - 420.0)
            .max(within.top() + 8.0),
    );
    egui::Window::new(title)
        .id(egui::Id::unique("duck_mold_add_list"))
        .fixed_pos(at)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            chosen = choices(ui, studio, thumbs);
        });
    if let Some(made) = chosen {
        // What it did, or why not, is in the status line.
        let _ = match &adding.place {
            Place::Socket { part, socket } => studio.add_to_socket(part, socket, &made),
            Place::Ground => studio.add_free(&made),
        };
        view.adding = None;
    } else if !open {
        view.adding = None;
    }
}

/// How big the Add menu's pictures of the shapes are, in points.
const SHAPE_SIDE: f32 = 22.0;

/// The things a new part can be made of, as buttons: the shapes, the
/// parts this recipe defines, and the library. Gives the one clicked.
pub fn choices(ui: &mut egui::Ui, studio: &mut Studio, thumbs: &mut Thumbs) -> Option<Made> {
    let mut chosen = None;
    // The shapes, each with its picture, in three columns so the list
    // stays short.
    let ctx = ui.ctx().clone();
    egui::Grid::new("duck_mold_shape_choices")
        .num_columns(3)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (at, shape) in Shape::NAMES.into_iter().enumerate() {
                // "Box", "Cylinder"...
                let mut label = shape.to_owned();
                if let Some(first) = label.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                let button = match thumbs.shapes.get(&ctx, shape) {
                    Some(picture) => egui::Button::image_and_text(
                        egui::Image::from_texture(picture)
                            .fit_to_exact_size(egui::vec2(SHAPE_SIDE, SHAPE_SIDE)),
                        label,
                    ),
                    None => egui::Button::new(label),
                };
                if ui.add_sized([96.0, SHAPE_SIDE + 6.0], button).clicked() {
                    chosen = Some(Made::Shape(shape.to_owned()));
                }
                if at % 3 == 2 {
                    ui.end_row();
                }
            }
        });
    let defined = studio.definitions();
    if !defined.is_empty() {
        ui.separator();
        ui.label(RichText::new("Defined in this recipe").weak().small());
        for name in defined {
            // The icon of what it is made of, as the shapes have their
            // pictures: a defined arm is parts, a defined leg a box.
            let shape = studio
                .model()
                .and_then(|model| {
                    model
                        .leaves()
                        .into_iter()
                        .chain(model.parts.iter())
                        .find(|part| part.kind.uses.as_deref() == Some(name.as_str()))
                        .map(|part| part.kind.shape.clone())
                })
                .unwrap_or_default();
            if icons::labelled(ui, Icon::for_shape(&shape), &name, false).clicked() {
                chosen = Some(Made::Use(name));
            }
        }
    }
    let library = studio.library().to_vec();
    if !library.is_empty() {
        ui.separator();
        ui.label(RichText::new("From the library").weak().small());
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                for item in &library {
                    if crate::library::item_button(ui, studio, thumbs, item, 32.0).clicked() {
                        chosen = Some(Made::File(item.written.clone()));
                    }
                }
            });
    }
    chosen
}
