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
use crate::library::Thumbs;
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
}

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
    let size = picture_size(ui, rect);
    // A point of the screen in the picture's own pixels, whatever size
    // the picture is drawn at.
    let in_picture = |pointer: egui::Pos2| {
        let across = f64::from((pointer.x - rect.min.x) / rect.width());
        let along = f64::from((pointer.y - rect.min.y) / rect.height());
        (across * size.0 as f64, along * size.1 as f64)
    };

    let pointer = response
        .hover_pos()
        .or_else(|| response.interact_pointer_pos());
    studio.hover(size, pointer.map(in_picture));

    // A drag that starts on a handle, or on the selected part, moves the
    // part. Any other drag turns or slides the view.
    let (shift, free) = ui.input(|input| (input.modifiers.shift, input.modifiers.command));
    if response.drag_started_by(PointerButton::Primary)
        && !shift
        && let Some(pressed) = ui.input(|input| input.pointer.press_origin())
    {
        view.moving = studio.grab(size, in_picture(pressed)).is_some();
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
            studio.drag(size, in_picture(pointer), free);
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
        if studio.mode() != Mode::Part && studio.face_tool == FaceTool::Cut {
            // The Cut tool: cut where the ring is. What it did, or why
            // not, is in the status line.
            let _ = studio.cut_at(size, in_picture(pointer), free);
            view.adding = None;
        } else if studio.mode() != Mode::Part {
            // Face or Edge mode: the face (or edge) under the pointer is
            // selected; with Shift, added or taken out.
            studio.click_face(size, in_picture(pointer), shift);
            view.adding = None;
        } else if studio.tool == Tool::Paint && !studio.look_only() {
            // The Paint tool: the face under the pointer gets the colour
            // in hand; with Shift, the whole part does. What it did, or
            // why not, is in the status line.
            let _ = studio.paint_at(size, in_picture(pointer), shift);
            view.adding = None;
        } else if shift {
            // Shift+click: the part joins the selection, or leaves it.
            studio.click_adding(size, in_picture(pointer));
            view.adding = None;
        } else {
            view.adding = match studio.point_at(size, in_picture(pointer)) {
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
        if let Some(part) = studio.part_under(size, at) {
            if !studio.is_selected(&part) {
                studio.select(Some(&part));
            }
            view.menu_spot = studio.spot_at(size, at);
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
    let shows_markers = (studio.selected().is_some() && !studio.look_only()) || shaping;
    let now = Drawn {
        builds: studio.builds(),
        orbit: studio.orbit,
        grid: studio.grid,
        wire: studio.wire,
        shadow: studio.shadow,
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
    };
    // While a handle is held the picture changes with every move.
    if view.drawn.as_ref() != Some(&now) || view.texture.is_none() || view.moving {
        let picture = studio.picture(size.0, size.1);
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
        || (shaping && !studio.selected_faces().is_empty());
    let hint = if view.moving {
        "Let go to make the change. Escape lets go without changing anything."
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
    painter.text(
        rect.left_bottom() + egui::vec2(8.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        hint,
        egui::FontId::proportional(12.0),
        quiet,
    );
    if handles {
        painter.text(
            rect.right_bottom() + egui::vec2(-8.0, -6.0),
            egui::Align2::RIGHT_BOTTOM,
            studio.snap_text(),
            egui::FontId::proportional(12.0),
            quiet,
        );
    }
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
    ];
    for (at, (mode, name, tip)) in modes.into_iter().enumerate() {
        let widths = [44.0, 44.0, 44.0, 56.0];
        let left: f32 = widths[..at].iter().sum::<f32>() + at as f32 * 4.0;
        let place = Rect::from_min_size(
            within.left_top() + egui::vec2(8.0 + left, 8.0),
            egui::vec2(widths[at], 24.0),
        );
        ui.painter_at(within)
            .rect_filled(place, 4.0, Color32::from_black_alpha(150));
        // On one line, however narrow the button.
        let button = egui::Button::selectable(studio.mode() == mode, name)
            .wrap_mode(egui::TextWrapMode::Extend);
        if ui.put(place, button).on_hover_text(tip).clicked() && studio.mode() != mode {
            // Why not, if it can't, is in the status line.
            let _ = match mode {
                Mode::Part => {
                    studio.part_mode();
                    Ok(String::new())
                }
                Mode::Face => studio.face_mode(),
                Mode::Edge => studio.edge_mode(),
                Mode::Corner => studio.corner_mode(),
            };
        }
    }
}

/// The tools, down the left edge of the viewport: one is in hand.
fn tool_strip(ui: &mut egui::Ui, studio: &mut Studio, within: Rect) {
    if studio.look_only() {
        return;
    }
    if studio.mode() != Mode::Part {
        let face_tools = [
            (
                FaceTool::Pull,
                "Pull",
                "T",
                "An arrow out of the faces selected: drag it to pull them out, or push them in",
            ),
            (
                FaceTool::Move,
                "Move",
                "G",
                "Three arrows: slide the faces along the part's own directions; the faces round them stretch",
            ),
            (
                FaceTool::Turn,
                "Turn",
                "R",
                "Three rings: turn the faces about their middle",
            ),
            (
                FaceTool::Scale,
                "Scale",
                "S",
                "Three boxes, and one in the middle: make the faces bigger or smaller",
            ),
            (
                FaceTool::Inset,
                "Inset",
                "I",
                "A knob on an edge: drag it in to make a smaller face inside, with a frame round it",
            ),
            (
                FaceTool::Cut,
                "Cut",
                "K",
                "A ring round the part where you point: click to cut there, so there are more faces to shape",
            ),
        ];
        // Edge mode has Move and Cut; Corner mode, Move.
        let tools: Vec<_> = face_tools
            .into_iter()
            .filter(|(tool, ..)| match studio.mode() {
                Mode::Edge => matches!(tool, FaceTool::Move | FaceTool::Cut),
                Mode::Corner => *tool == FaceTool::Move,
                _ => true,
            })
            .collect();
        for (row, (tool, name, key, tip)) in tools.into_iter().enumerate() {
            let place = Rect::from_min_size(
                within.left_top() + egui::vec2(8.0, TOOLS_TOP + row as f32 * 30.0),
                egui::vec2(76.0, 24.0),
            );
            ui.painter_at(within)
                .rect_filled(place, 4.0, Color32::from_black_alpha(150));
            let button = egui::Button::selectable(studio.face_tool == tool, name);
            if ui
                .put(place, button)
                .on_hover_text(format!("{tip} ({key})"))
                .clicked()
            {
                studio.face_tool = tool;
            }
        }
        return;
    }
    let tools = [
        (Tool::Move, "Move", "G", "Arrows: drag one to move the part"),
        (
            Tool::Rotate,
            "Rotate",
            "R",
            "Rings: drag one to turn the part",
        ),
        (
            Tool::Scale,
            "Scale",
            "S",
            "Boxes: drag one to make the part bigger or smaller",
        ),
        (
            Tool::Size,
            "Size",
            "T",
            "Knobs on a shape's faces: pull one to change its width, height, depth or radius",
        ),
        (
            Tool::Paint,
            "Paint",
            "P",
            "Click a face to colour it with the colour picked in the palette. Shift+click colours the whole part",
        ),
    ];
    for (row, (tool, name, key, tip)) in tools.into_iter().enumerate() {
        let place = Rect::from_min_size(
            within.left_top() + egui::vec2(8.0, TOOLS_TOP + row as f32 * 30.0),
            egui::vec2(76.0, 24.0),
        );
        // A dark plate behind it, so it reads over the picture.
        ui.painter_at(within)
            .rect_filled(place, 4.0, Color32::from_black_alpha(150));
        let button = egui::Button::selectable(studio.tool == tool, name);
        if ui
            .put(place, button)
            .on_hover_text(format!("{tip} ({key})"))
            .clicked()
        {
            studio.tool = tool;
        }
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
            if ui.button(&name).clicked() {
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
