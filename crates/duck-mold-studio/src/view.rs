//! The viewport: the picture of the model, and the mouse on it.
//!
//! - Drag turns the view; right-drag (or middle-drag, or Shift and drag)
//!   slides it; the wheel zooms.
//! - A click selects the part under the pointer.
//! - With a part selected, its sockets show as markers. A click on one,
//!   or on the part's surface, opens a short list of what to add there.
//! - Dragging the selected part moves it: dropped on a socket of another
//!   part it joins there, dropped on a part's surface it slides there.
//!
//! The picture is drawn by Duck Forge's CPU renderer
//! ([`Studio::picture`]) and shown as one image.

use duck_mold_core::edit::{Made, Spot};
use duck_mold_core::recipe::Shape;
use duck_mold_view::studio::viewport::Orbit;
use duck_mold_view::studio::{Clicked, Studio};
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
    selected: Option<String>,
    size: [usize; 2],
    /// Where the pointer is, when that changes what is drawn (a part is
    /// selected, so markers show).
    pointer: Option<(i32, i32)>,
    dragging: bool,
}

/// Where a part is about to be added.
#[derive(Clone, PartialEq)]
enum Place {
    Socket { part: String, socket: String },
    Surface { part: String, spot: Spot },
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

/// Draws the viewport and handles the mouse on it.
pub fn show(ui: &mut egui::Ui, studio: &mut Studio, view: &mut View) {
    let rect = ui.available_rect_before_wrap();
    view.rect = Some(rect);
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    if rect.width() < 2.0 || rect.height() < 2.0 {
        return;
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

    // A drag that starts on the selected part moves the part. Any other
    // drag turns or slides the view.
    let shift = ui.input(|input| input.modifiers.shift);
    if response.drag_started_by(PointerButton::Primary)
        && !shift
        && let Some(pressed) = ui.input(|input| input.pointer.press_origin())
    {
        view.moving = studio.drag_start(size, in_picture(pressed));
        if view.moving {
            view.adding = None;
        }
    }
    if view.moving {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            studio.drag_cancel();
            view.moving = false;
        } else if response.drag_stopped() {
            // What it did, or why not, is in the status line.
            let _ = studio.drag_end();
            view.moving = false;
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
        view.adding = match studio.point_at(size, in_picture(pointer)) {
            Clicked::Socket { part, socket } => Some(Adding {
                place: Place::Socket { part, socket },
                at: pointer,
            }),
            Clicked::Surface { part, spot } => Some(Adding {
                place: Place::Surface { part, spot },
                at: pointer,
            }),
            Clicked::Selected(_) | Clicked::Nothing => None,
        };
    }
    if response.hovered() {
        let rolled = ui.input(|input| input.smooth_scroll_delta.y);
        if rolled != 0.0 {
            studio.orbit.zoom(f64::from(rolled / WHEEL_STEP));
        }
    }

    let shows_markers = studio.selected().is_some() && !studio.look_only();
    let now = Drawn {
        builds: studio.builds(),
        orbit: studio.orbit,
        grid: studio.grid,
        selected: studio.selected().map(str::to_owned),
        size: [size.0, size.1],
        pointer: pointer
            .filter(|_| shows_markers)
            .map(in_picture)
            .map(|(x, y)| (x.round() as i32, y.round() as i32)),
        dragging: studio.dragging().is_some(),
    };
    if view.drawn.as_ref() != Some(&now) || view.texture.is_none() {
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

    // What the mouse does here, in a line at the bottom.
    let hint = if view.moving {
        "Drop it on a socket to join it there, or on a part to put it at that point. Escape lets go."
    } else if shows_markers && studio.facts().is_some() {
        "Click a socket, or the part itself, to add a part there. Drag the part to move it."
    } else {
        "Drag to turn, right-drag to slide, roll the wheel to zoom. Click a part to select it."
    };
    ui.painter_at(rect).text(
        rect.left_bottom() + egui::vec2(8.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        hint,
        egui::FontId::proportional(12.0),
        Color32::from_white_alpha(150),
    );

    add_list(ui, studio, view, rect);
}

/// The short list that opens where a part is to be added: the five
/// shapes, and the parts the recipe defines.
fn add_list(ui: &mut egui::Ui, studio: &mut Studio, view: &mut View, within: Rect) {
    let Some(adding) = &view.adding else {
        return;
    };
    let title = match &adding.place {
        Place::Socket { part, socket } => format!("Add a part on {part}.{socket}"),
        Place::Surface { part, spot } => format!("Add a part on the {} of {part}", spot.on),
    };
    let mut chosen: Option<Made> = None;
    let mut open = true;
    // Kept inside the viewport, beside the click.
    let at = pos2(
        (adding.at.x + 12.0).min(within.right() - 190.0),
        (adding.at.y + 12.0).min(within.bottom() - 260.0),
    );
    egui::Window::new(title)
        .id(egui::Id::unique("duck_mold_add_list"))
        .fixed_pos(at)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            for shape in Shape::NAMES {
                // "Box", "Cylinder"...
                let mut label = shape.to_owned();
                if let Some(first) = label.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                if ui.button(label).clicked() {
                    chosen = Some(Made::Shape(shape.to_owned()));
                }
            }
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
        });
    if let Some(made) = chosen {
        // What it did, or why not, is in the status line.
        let _ = match &adding.place {
            Place::Socket { part, socket } => studio.add_to_socket(part, socket, &made),
            Place::Surface { part, spot } => studio.add_to_surface(part, spot, &made),
        };
        view.adding = None;
    } else if !open {
        view.adding = None;
    }
}
