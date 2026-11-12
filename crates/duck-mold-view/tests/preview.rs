//! The preview window, driven without a window by Duck Forge's
//! `Simulation`: it shows the model, orbits and zooms, builds it again
//! when a file is saved, and keeps the last good model when a save has
//! mistakes.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use duck_mold_view::preview::Preview;
use duck_mold_view::render::BACKGROUND;
use duckforge::Simulation;
use duckforge::prelude::{Key, MouseButton, vec2};

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes a file as an editor saves it: new text, and a time later than
/// before, so the change shows even where the clock is coarse.
fn save(path: &Path, text: &str) {
    let before = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::now());
    std::fs::write(path, text).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(before + Duration::from_secs(2)).unwrap();
}

const CRATE: &str = "version = 0\nname = \"crate\"\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n";

fn open(path: &Path) -> Simulation<Preview> {
    let preview = Preview::open(path);
    let mut sim = Simulation::new(preview.config(), preview);
    sim.step();
    sim
}

/// How many pixels are the model's, not the background's or the text's,
/// in the middle of the picture.
fn model_pixels(sim: &mut Simulation<Preview>) -> usize {
    let picture = sim.picture();
    let (width, height) = (picture.width() as i32, picture.height() as i32);
    let mut count = 0;
    for y in height / 6..height * 5 / 6 {
        for x in width / 6..width * 5 / 6 {
            if picture.get_pixel(x, y).map(|p| p.to_u32()) != Some(BACKGROUND.to_u32()) {
                count += 1;
            }
        }
    }
    count
}

/// The middle pixel's colour.
fn middle(sim: &mut Simulation<Preview>) -> u32 {
    let picture = sim.picture();
    let (x, y) = (picture.width() as i32 / 2, picture.height() as i32 / 2);
    picture.get_pixel(x, y).unwrap().to_u32()
}

#[test]
fn it_shows_the_model_with_its_name_and_size() {
    let folder = folder("preview_shows");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut sim = open(&recipe);
    let preview = sim.game();
    assert_eq!(
        preview.headline(),
        Some("crate: 1.00 x 1.00 x 1.00 m, 12 triangles")
    );
    assert_eq!(preview.lines(), []);
    assert_eq!(preview.builds(), 1);
    assert_eq!(preview.watched(), [recipe.as_path()]);
    assert!(model_pixels(&mut sim) > 1000, "the crate is drawn");
    assert!(preview_title(&recipe).ends_with("crate.toml - Duck Mold preview"));
}

fn preview_title(path: &Path) -> String {
    Preview::open(path).config().title
}

#[test]
fn dragging_orbits_and_the_wheel_zooms() {
    let folder = folder("preview_orbit");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut sim = open(&recipe);
    let (yaw, pitch, zoom) = sim.game().view();
    assert_eq!(zoom, 1.0);
    let before = sim.picture().pixels().to_vec();

    // Moving the mouse without the button does nothing.
    sim.move_mouse(vec2(100.0, 100.0));
    sim.move_mouse_by(vec2(40.0, 0.0));
    sim.step();
    assert_eq!(sim.game().view(), (yaw, pitch, zoom));

    // Dragging right turns the model to the right; dragging down tilts it.
    sim.press_mouse(MouseButton::Left);
    sim.move_mouse_by(vec2(50.0, 20.0));
    sim.step();
    sim.release_mouse(MouseButton::Left);
    let (turned, tilted, _) = sim.game().view();
    assert!((turned - (yaw - 0.5)).abs() < 1e-5, "{turned}");
    assert!((tilted - (pitch + 0.2)).abs() < 1e-5, "{tilted}");
    assert_ne!(sim.picture().pixels(), &before[..], "the view moved");

    // It can't tilt over the top.
    sim.press_mouse(MouseButton::Left);
    sim.move_mouse_by(vec2(0.0, 1000.0));
    sim.step();
    sim.release_mouse(MouseButton::Left);
    assert_eq!(sim.game().view().1, 1.5);

    // Rolling the wheel up comes closer; a long way stops at the closest.
    let far = model_pixels(&mut sim);
    sim.scroll(1.0);
    sim.step();
    assert!((sim.game().view().2 - 0.85).abs() < 1e-6);
    assert!(model_pixels(&mut sim) > far, "closer is bigger");
    for _ in 0..40 {
        sim.scroll(1.0);
        sim.step();
    }
    assert_eq!(sim.game().view().2, 0.25);
    for _ in 0..40 {
        sim.scroll(-1.0);
        sim.step();
    }
    assert_eq!(sim.game().view().2, 6.0);
}

#[test]
fn saving_the_recipe_builds_it_again_and_keeps_the_view() {
    let folder = folder("preview_saved");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut sim = open(&recipe);
    sim.press_mouse(MouseButton::Left);
    sim.move_mouse_by(vec2(30.0, 0.0));
    sim.step();
    sim.release_mouse(MouseButton::Left);
    let view = sim.game().view();
    let wood = middle(&mut sim);

    // Nothing saved: nothing built again, however long it runs.
    sim.run(120);
    assert_eq!(sim.game().builds(), 1);

    // Saved in blue: built again within half a second.
    save(&recipe, &CRATE.replace("#B07A3F", "#3F6FB0"));
    sim.run(31);
    assert_eq!(sim.game().builds(), 2);
    let blue = middle(&mut sim);
    assert_ne!(blue, wood);
    let [_, r, _, b] = blue.to_be_bytes();
    assert!(b > r, "the crate is blue now: {blue:06X}");
    assert_eq!(sim.game().view(), view, "the view stays where it was");

    // A different size shows in the headline.
    save(
        &recipe,
        &CRATE.replace("shape = \"box\"", "shape = \"box\"\nsize = [2, 1, 1]"),
    );
    sim.run(31);
    assert_eq!(
        sim.game().headline(),
        Some("crate: 2.00 x 1.00 x 1.00 m, 12 triangles")
    );
    assert_eq!(sim.number("builds"), 3.0);
}

#[test]
fn a_save_with_mistakes_keeps_the_last_good_model_and_shows_them() {
    let folder = folder("preview_mistake");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut sim = open(&recipe);
    let drawn = model_pixels(&mut sim);

    save(&recipe, &CRATE.replace("\"box\"", "\"boxx\""));
    sim.run(31);
    let preview = sim.game();
    assert_eq!(preview.builds(), 2);
    assert!(
        preview.lines().iter().any(|line| line.is_error
            && line
                .text
                .contains("there is no shape \"boxx\". Did you mean \"box\"?")),
        "{:#?}",
        preview.lines()
    );
    assert_eq!(
        sim.number("mistakes"),
        2.0,
        "the mistake and the closing line"
    );
    // The crate is still there.
    assert_eq!(
        sim.game().headline(),
        Some("crate: 1.00 x 1.00 x 1.00 m, 12 triangles")
    );
    assert!(model_pixels(&mut sim) >= drawn);

    // Fixed: the mistakes go.
    save(&recipe, CRATE);
    sim.run(31);
    assert_eq!(sim.game().lines(), []);
}

#[test]
fn a_recipe_that_has_never_built_shows_only_its_mistakes() {
    let folder = folder("preview_never");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, "version = 0\n[[parts]]\nshape = \"boxx\"\n").unwrap();
    let mut sim = open(&recipe);
    assert_eq!(sim.game().headline(), None);
    assert!(!sim.game().lines().is_empty());
    save(&recipe, CRATE);
    sim.run(31);
    assert!(sim.game().headline().is_some());
    assert!(model_pixels(&mut sim) > 1000);
}

#[test]
fn files_the_recipe_uses_are_watched_too() {
    let folder = folder("preview_palette");
    std::fs::write(folder.join("colours.toml"), "wood = \"#B07A3F\"\n").unwrap();
    let recipe = folder.join("crate.toml");
    std::fs::write(
        &recipe,
        "version = 0\npalette = \"colours.toml\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n",
    )
    .unwrap();
    let mut sim = open(&recipe);
    assert_eq!(
        sim.game().watched(),
        [recipe.as_path(), folder.join("colours.toml").as_path()]
    );
    let wood = middle(&mut sim);
    save(&folder.join("colours.toml"), "wood = \"#3F6FB0\"\n");
    sim.run(31);
    assert_eq!(sim.game().builds(), 2);
    assert_ne!(middle(&mut sim), wood);
}

#[test]
fn warnings_show_in_the_window_too() {
    let folder = folder("preview_warning");
    let recipe = folder.join("grey.toml");
    std::fs::write(&recipe, "version = 0\n[[parts]]\nshape = \"box\"\n").unwrap();
    let sim = open(&recipe);
    let lines = sim.game().lines();
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].is_error);
    assert!(lines[0].text.contains("plain grey"), "{lines:?}");
}

#[test]
fn escape_quits() {
    let folder = folder("preview_quit");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut sim = open(&recipe);
    assert!(!sim.quit_requested());
    sim.press(Key::Escape);
    sim.step();
    assert!(sim.quit_requested());
}
