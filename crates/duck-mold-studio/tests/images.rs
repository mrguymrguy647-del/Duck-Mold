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
        .get_by_role_and_label(Role::Button, "Just make it")
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
    assert_eq!(screen.query_all_by_label("Just make it").count(), 0);
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

// ---- painting an image in another program (step 4) -------------------------

#[test]
fn paint_in_duck_quill_hands_the_image_its_guide_and_the_colour_over() {
    use duck_mold_view::quill::With;
    let (recipe, mut screen) = plank_crate("images_paint_in_quill", false);
    let folder = recipe.parent().unwrap().to_path_buf();
    // A Duck Quill that is "there", and nothing really started.
    screen.state_mut().painting().with = Some(With::DuckQuill(folder.join("DuckQuill")));
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::Button, "Paint in Duck Quill: top")
        .click();
    screen.run_steps(3);
    let job = screen.state_mut().painting().last.clone().expect("a job");
    assert_eq!(job.image, folder.join("textures/lid.png"));
    assert_eq!(job.with, With::DuckQuill(folder.join("DuckQuill")));
    // The crate's wood is the paper.
    assert_eq!(job.paper, [0xB0, 0x7A, 0x3F]);
    let guide = job.guide.clone().expect("a guide for Duck Quill");
    assert!(guide.is_file());
    assert!(
        screen
            .state()
            .studio()
            .unwrap()
            .status()
            .starts_with("Would start: "),
        "{}",
        screen.state().studio().unwrap().status()
    );
    // Nothing in the recipe changed.
    assert_eq!(
        std::fs::read_to_string(&recipe).unwrap(),
        std::fs::read_to_string(examples().join("plank_crate.toml")).unwrap()
    );

    // Edit > Paint in Duck Quill does the same for the selected part's
    // image (the first it has: the planks on the whole part).
    screen.state_mut().painting().last = None;
    screen.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::P);
    screen.run_steps(3);
    let job = screen.state_mut().painting().last.clone().expect("a job");
    assert_eq!(job.image, folder.join("textures/planks.png"));
    // The Edit menu has a line for it, with its key.
    let before = screen
        .query_all_by_label_contains("Paint in Duck Quill")
        .count();
    screen.get_by_label("Edit").click();
    screen.run_steps(2);
    assert_eq!(
        screen
            .query_all_by_label_contains("Paint in Duck Quill")
            .count(),
        before + 1
    );
}

#[test]
fn without_duck_quill_the_studio_asks_once_and_any_picture_program_does() {
    use duck_mold_view::quill::With;
    let (recipe, mut screen) = plank_crate("images_no_quill", false);
    let folder = recipe.parent().unwrap().to_path_buf();
    // Somewhere to remember the choice; nothing is really started. (No
    // Duck Quill is found from here: none is on the machines that run
    // the tests.)
    let settings = folder.join("settings");
    screen.state_mut().painting().settings = Some(settings.clone());
    if screen.state_mut().painting().program().is_some() {
        // A machine with Duck Quill on it: the question never comes up.
        return;
    }
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::Button, "Paint in Duck Quill: top")
        .click();
    screen.run_steps(3);
    // The window says so, and offers three ways on.
    screen.get_by_label("Duck Quill isn't on this computer");
    screen.get_by_role_and_label(Role::Button, "Find DuckQuill.exe...");
    screen.get_by_label("Get Duck Quill");
    assert!(screen.state_mut().painting().last.is_none());
    screen
        .get_by_role_and_label(Role::Button, "Open in my picture program instead")
        .click();
    screen.run_steps(3);
    let job = screen.state_mut().painting().last.clone().expect("a job");
    assert_eq!(job.with, With::System);
    assert_eq!(job.image, folder.join("textures/lid.png"));
    // Another program draws no guide over the picture.
    assert_eq!(job.guide, None);
    // The window has gone, the choice is remembered, and the button
    // says what it now does.
    assert_eq!(
        screen
            .query_all_by_label("Duck Quill isn't on this computer")
            .count(),
        0
    );
    assert_eq!(
        duck_mold_view::quill::remembered(&settings),
        Some(With::System)
    );
    screen.get_by_role_and_label(Role::Button, "Open in my picture program: top");

    // Help > Duck Quill... says what images are painted in.
    screen.get_by_label("Help").click();
    screen.run_steps(2);
    screen
        .query_all_by_label_contains("Duck Quill...")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    screen.get_by_label("the picture program this computer opens PNG files with");
}

#[test]
fn a_new_image_can_be_made_and_painted_in_one_click() {
    use duck_mold_view::quill::With;
    let (recipe, mut screen) = plank_crate("images_new_and_paint", false);
    let folder = recipe.parent().unwrap().to_path_buf();
    screen.state_mut().painting().with = Some(With::DuckQuill(folder.join("DuckQuill")));
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    // Shift+P on a part that has images paints the first; so the "New
    // image" window is opened by its button here.
    screen
        .get_by_role_and_label(Role::Button, "New image...")
        .click();
    screen.run_steps(3);
    screen.get_by_role_and_label(Role::ComboBox, "For").click();
    screen.run_steps(2);
    screen.get_by_label("front").click();
    screen.run_steps(3);
    screen
        .get_by_role_and_label(Role::Button, "Make it and paint in Duck Quill")
        .click();
    screen.run_steps(4);
    let made = folder.join("textures/plank_crate_box_front.png");
    assert!(made.is_file());
    let job = screen.state_mut().painting().last.clone().expect("a job");
    assert_eq!(job.image, made);
    assert!(job.guide.is_some());
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
    // Without Duck Quill: the window that asks.
    screen.state_mut().studio_mut().unwrap().select(Some("box"));
    screen.run_steps(2);
    if screen.state_mut().painting().program().is_none() {
        screen
            .get_by_role_and_label(Role::Button, "Paint in Duck Quill: top")
            .click();
        screen.run_steps(4);
        save(&mut screen, "5-no-duck-quill");
    }
}

#[test]
fn a_part_from_a_file_lists_the_pictures_the_file_brought() {
    let folder = folder("images_from_file");
    std::fs::create_dir_all(folder.join("parts")).unwrap();
    for file in ["signpost.toml", "parts/sign.gltf", "parts/sign_post.png"] {
        std::fs::copy(examples().join(file), folder.join(file)).unwrap();
    }
    let recipe = folder.join("signpost.toml");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    // The model's images are counted, and the one that is a PNG on the
    // disk is in the row to paint with.
    screen.get_by_label_contains("3 (128 x 128 pixels together)");
    screen.get_by_role_and_label(Role::Button, "sign_post.png");

    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("sign"));
    screen.run_steps(3);
    assert_eq!(screen.query_all_by_label("from the file").count(), 3);
    assert!(screen.query_by_label("None yet.").is_none());
    // A file's picture can't be painted on: no painting button for it.
    assert!(
        screen
            .query_by_label_contains("Paint in Duck Quill: board")
            .is_none()
            && screen
                .query_by_label_contains("Open in my picture program: board")
                .is_none()
    );

    // The cross takes the board's picture off: "none" is written.
    screen
        .get_by_role_and_label(Role::Button, "Take the image off board")
        .click();
    screen.run_steps(3);
    assert_eq!(
        recipe_line(&recipe, "images").as_deref(),
        Some("images = { board = \"none\" }")
    );
    assert_eq!(screen.query_all_by_label("from the file").count(), 2);
    screen.get_by_label("none");

    // The cross on that line shows the file's picture again.
    screen
        .get_by_role_and_label(Role::Button, "Take the image off board")
        .click();
    screen.run_steps(3);
    assert_eq!(recipe_line(&recipe, "images"), None);
    assert_eq!(screen.query_all_by_label("from the file").count(), 3);
}
