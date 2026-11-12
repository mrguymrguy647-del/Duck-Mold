//! What the Studio's screen tests share: the app on a pretend screen,
//! copies of the examples to change, and the mouse.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use duck_mold_studio::{StudioApp, theme};
use eframe::egui;
use egui_kittest::Harness;

pub fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// A fresh, empty folder for one test.
pub fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A copy of an example recipe in a folder of its own, so a test can
/// change it and write files next to it.
pub fn copy_of(example: &str, test: &str) -> PathBuf {
    let folder = folder(test);
    let copy = folder.join(example);
    std::fs::copy(examples().join(example), &copy).unwrap();
    copy
}

/// The app on a pretend 1280 x 800 screen, run the way eframe runs it.
pub fn harness(app: StudioApp, render: bool) -> Harness<'static, StudioApp> {
    let builder = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_pixels_per_point(1.0)
        .with_render_cursor(false)
        .with_context_setup(|ctx| {
            ctx.set_theme(egui::ThemePreference::Dark);
            theme::apply_brand(ctx);
        });
    let builder = if render { builder.wgpu() } else { builder };
    builder.build_eframe(|_| app)
}

/// The middle of the viewport on that screen.
pub const IN_VIEW: egui::Pos2 = egui::pos2(760.0, 360.0);

/// The path of the selected part.
pub fn selected(screen: &Harness<'static, StudioApp>) -> Option<String> {
    let studio = screen.state().studio().unwrap();
    studio.selected().map(str::to_owned)
}

/// Where the middle of a part is on the screen.
pub fn on_screen(screen: &Harness<'static, StudioApp>, part: &str) -> egui::Pos2 {
    let (rect, [width, height]) = screen.state().viewport().unwrap();
    let studio = screen.state().studio().unwrap();
    let middle = studio.part(part).unwrap().bounds.unwrap().center();
    let (x, y) = studio.orbit.on_screen(middle, width, height).unwrap();
    rect.min
        + egui::vec2(
            x as f32 * rect.width() / width as f32,
            y as f32 * rect.height() / height as f32,
        )
}

/// Presses and lets go of the left button without moving: a click.
pub fn click_at(screen: &mut Harness<'static, StudioApp>, at: egui::Pos2) {
    screen.hover_at(at);
    screen.step();
    screen.drag_at(at);
    screen.step();
    screen.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    screen.run_steps(2);
}

/// Where a socket of a part is on the screen.
pub fn socket_on_screen(
    screen: &Harness<'static, StudioApp>,
    part: &str,
    socket: &str,
) -> egui::Pos2 {
    let (rect, [width, height]) = screen.state().viewport().unwrap();
    let studio = screen.state().studio().unwrap();
    let at = studio
        .part(part)
        .unwrap()
        .sockets
        .iter()
        .find(|known| known.name == socket)
        .unwrap()
        .at;
    let (x, y) = studio.orbit.on_screen(at, width, height).unwrap();
    rect.min
        + egui::vec2(
            x as f32 * rect.width() / width as f32,
            y as f32 * rect.height() / height as f32,
        )
}
