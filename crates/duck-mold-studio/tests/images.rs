//! Phase 7b, step 3: images in the Studio's window, run with no window.
//!
//! The row of images under the palette, a part's images in its
//! properties, the "New image" window and the Paint tool with an image
//! in hand. The Studio underneath is the real one, working on a copy of
//! the plank crate.

use std::path::{Path, PathBuf};

use duck_mold_core::math::{Vec3, vec3};
use duck_mold_studio::StudioApp;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::images::Brush;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

mod common;
use common::*;

/// The plank crate, with its images, in a folder of its own, open.
fn plank_crate(test: &str, render: bool) -> (PathBuf, Harness<'static, StudioApp>) {
    let folder = folder(test);
    std::fs::create_dir_all(folder.join("textures")).unwrap();
    for file in [
        "plank_crate.toml",
        "textures/planks.png",
        "textures/lid.png",
    ] {
        std::fs::copy(examples().join(file), folder.join(file)).unwrap();
    }
    let recipe = folder.join("plank_crate.toml");
    let mut screen = harness(StudioApp::with_file(&recipe, false), render);
    screen.run_steps(2);
    (recipe, screen)
}

fn recipe_line(recipe: &Path, starts: &str) -> Option<String> {
    std::fs::read_to_string(recipe)
        .unwrap()
        .lines()
        .find(|line| line.starts_with(starts))
        .map(str::to_owned)
}

/// Where a point of the model is on the screen.
fn point_on_screen(screen: &Harness<'static, StudioApp>, point: Vec3) -> egui::Pos2 {
    let (rect, [width, height]) = screen.state().viewport().unwrap();
    let studio = screen.state().studio().unwrap();
    let (x, y) = studio
        .orbit
        .on_screen(point + studio.offset(), width, height)
        .unwrap();
    rect.min
        + egui::vec2(
            x as f32 * rect.width() / width as f32,
            y as f32 * rect.height() / height as f32,
        )
}

#[test]
fn the_palette_lists_the_images_and_one_is_picked_to_paint_with() {
    let (recipe, mut screen) = plank_crate("images_palette", false);
    // The model, with nothing selected, says how many images it has.
    assert_eq!(screen.query_all_by_label("Images").count(), 2);
    screen.get_by_label_contains("2 (128 x 64 pixels together)");
    screen.get_by_label("2 images");

    // A click on an image takes the Paint tool in hand, with it.
    screen
        .get_by_role_and_label(Role::Button, "lid.png")
        .click();
    screen.run_steps(2);
    {
        let studio = screen.state().studio().unwrap();
        assert_eq!(studio.tool, Tool::Paint);
        assert_eq!(
            studio.brush(),
            Some(Brush::Image("textures/lid.png".to_owned()))
        );
    }
    // Over the crate's front, the picture says what a click would do;
    // the click does it.
    let front = point_on_screen(&screen, vec3(0.0, 0.5, 0.5));
    screen.hover_at(front);
    screen.run_steps(2);
    assert_eq!(
        screen
            .state()
            .studio()
            .unwrap()
            .paint_hint(false)
            .as_deref(),
        Some("front of box: put lid.png on it")
    );
    click_at(&mut screen, front);
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("front = \"textures/lid.png\"")
    );

    // The first square is "no image": a click on the face takes it off.
    screen
        .get_by_role_and_label(Role::Button, "No image")
        .click();
    screen.run_steps(2);
    assert_eq!(
        screen.state().studio().unwrap().brush(),
        Some(Brush::NoImage)
    );
    click_at(&mut screen, front);
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("front = \"none\"")
    );
    // A click on it again puts it down.
    screen
        .get_by_role_and_label(Role::Button, "No image")
        .click();
    screen.run_steps(2);
    assert_eq!(screen.state().studio().unwrap().tool, Tool::Move);
    assert!(screen.state().studio().unwrap().problems().is_empty());
}

#[test]
fn a_parts_images_are_in_its_properties_and_can_be_taken_off() {
    let (recipe, mut screen) = plank_crate("images_properties", false);
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    for label in ["whole part", "top", "bottom"] {
        assert!(
            screen.query_all_by_label(label).count() >= 1,
            "the panel should show {label}"
        );
    }
    // The cross at the end of a line takes that image off.
    screen
        .get_by_role_and_label(Role::Button, "Take the image off top")
        .click();
    screen.run_steps(3);
    // The part has planks, so "none" is written to keep them off the top.
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("top = \"none\"")
    );
    screen
        .get_by_role_and_label(Role::Button, "Take the image off whole part")
        .click();
    screen.run_steps(3);
    assert_eq!(recipe_line(&recipe, "image = "), None);
    assert!(screen.state().studio().unwrap().problems().is_empty());
}

#[test]
fn a_new_image_is_made_from_its_window() {
    let (recipe, mut screen) = plank_crate("images_new", false);
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::Button, "New image...")
        .click();
    screen.run_steps(2);
    screen.get_by_label("New image for \"box\"");
    screen.get_by_label("textures/plank_crate_box.png");
    screen.get_by_label("1.00 x 1.00 m on the model");
    // Pixels the size of the model's other images: 32 to the metre.
    screen.get_by_label_contains("32 x 32: pixels the size of the model's other images");
    screen.get_by_label("Large: 64 x 64");

    // For the front only.
    screen.get_by_role_and_label(Role::ComboBox, "For").click();
    screen.run_steps(2);
    screen.get_by_label("front").click();
    screen.run_steps(3);
    screen.get_by_label("New image for front of \"box\"");
    screen.get_by_label("textures/plank_crate_box_front.png");

    screen.get_by_label("Large: 64 x 64").click();
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::Button, "Make it")
        .click();
    screen.run_steps(3);
    let made = recipe
        .parent()
        .unwrap()
        .join("textures/plank_crate_box_front.png");
    let picture =
        duck_mold_core::picture::read_png(&std::fs::read(&made).unwrap(), "made").unwrap();
    assert_eq!((picture.width, picture.height), (64, 64));
    assert!(picture.pixels.iter().all(|pixel| pixel[3] == 0));
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("front = \"textures/plank_crate_box_front.png\"")
    );
    // The window has closed, and the new image is in the palette's row.
    assert_eq!(screen.query_all_by_label("Make it").count(), 0);
    screen.get_by_role_and_label(Role::Button, "plank_crate_box_front.png");
    screen.get_by_label("3 images");
    // Twice as many pixels to the metre as the others: allowed, just.
    assert!(screen.state().studio().unwrap().problems().is_empty());
}

#[test]
fn looking_only_shows_the_images_and_offers_no_changes() {
    let folder = folder("images_look");
    std::fs::create_dir_all(folder.join("textures")).unwrap();
    for file in [
        "plank_crate.toml",
        "textures/planks.png",
        "textures/lid.png",
    ] {
        std::fs::copy(examples().join(file), folder.join(file)).unwrap();
    }
    let mut screen = harness(
        StudioApp::with_file(folder.join("plank_crate.toml"), true),
        false,
    );
    screen.run_steps(2);
    screen.get_by_role_and_label(Role::Button, "planks.png");
    assert_eq!(screen.query_all_by_label("No image").count(), 0);
    assert_eq!(screen.query_all_by_label("New image").count(), 0);
}

/// Pictures of the image screens, for a person to look at: run with
/// `DUCK_MOLD_LOOK=<folder> cargo test -p duck-mold-studio --test images -- --ignored`.
#[test]
#[ignore = "draws pictures for a person to look at"]
fn look() {
    let Some(out) = std::env::var_os("DUCK_MOLD_LOOK").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let save = |screen: &mut Harness<'static, StudioApp>, name: &str| {
        screen.run_steps(2);
        let image = screen.render().expect("drawn");
        image.save(out.join(format!("{name}.png"))).unwrap();
    };
    let (_recipe, mut screen) = plank_crate("images_look_pictures", true);
    save(&mut screen, "1-plank-crate");
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    save(&mut screen, "2-part-selected");
    screen
        .get_by_role_and_label(Role::Button, "New image...")
        .click();
    save(&mut screen, "3-new-image");
    screen.run_steps(4);
    screen.get_by_role_and_label(Role::Button, "Cancel").click();
    screen.run_steps(4);
    screen.state_mut().studio_mut().unwrap().select(None);
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::Button, "lid.png")
        .click();
    screen.run_steps(2);
    let front = point_on_screen(&screen, vec3(0.0, 0.5, 0.5));
    screen.hover_at(front);
    save(&mut screen, "4-painting-with-an-image");
}
