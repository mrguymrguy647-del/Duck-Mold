//! The Studio's screens, run with no window.
//!
//! `egui_kittest` runs the app's own drawing code against a pretend
//! screen, finds things by their label and clicks them, so these tests
//! need no display. The Studio underneath is the real one, working on a
//! copy of an example recipe.
//!
//! The tests that click run on Windows and Linux. The pictures of whole
//! screens (the `picture` module at the end) are compared with stored
//! ones on Linux only: egui draws through the graphics driver, and
//! drivers don't agree to the last pixel. To store new pictures after a
//! change that is meant to be seen:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p duck-mold-studio --test screens
//! ```

use std::path::{Path, PathBuf};

use duck_mold_studio::{StudioApp, theme};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// A fresh, empty folder for one test.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A copy of an example recipe in a folder of its own, so a test can
/// change it and write files next to it.
fn copy_of(example: &str, test: &str) -> PathBuf {
    let folder = folder(test);
    let copy = folder.join(example);
    std::fs::copy(examples().join(example), &copy).unwrap();
    copy
}

/// The app on a pretend 1280 x 800 screen, run the way eframe runs it.
fn harness(app: StudioApp, render: bool) -> Harness<'static, StudioApp> {
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
const IN_VIEW: egui::Pos2 = egui::pos2(760.0, 360.0);

#[test]
fn the_start_page_offers_open_and_new() {
    let mut screen = harness(StudioApp::new(), false);
    screen.step();
    screen.get_by_label("Duck Mold Studio");
    screen.get_by_label("Open a model");
    screen.get_by_label("New model");
    assert_eq!(screen.state().title(), "Duck Mold Studio");

    // A new model is a recipe with one box, opened at once.
    let recipe = folder("screens_new").join("crate.toml");
    screen.state_mut().start_new(&recipe);
    screen.run_steps(2);
    assert_eq!(screen.state().title(), "crate.toml - Duck Mold Studio");
    screen.get_by_label("body");
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("name = \"crate\""), "{text}");

    // It is never written over.
    let mut screen = harness(StudioApp::new(), false);
    screen.state_mut().start_new(&recipe);
    screen.run_steps(2);
    screen.get_by_label_contains("is already there");
}

#[test]
fn a_file_that_isnt_there_is_said_on_the_start_page() {
    let mut screen = harness(StudioApp::with_file("nowhere.toml", false), false);
    screen.step();
    screen.get_by_label("There is no file \"nowhere.toml\".");
    screen.get_by_label("Open a model");
}

#[test]
fn an_open_recipe_shows_its_parts_and_no_problems() {
    let recipe = copy_of("robot.toml", "screens_open");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen.get_by_label("robot.toml");
    screen.get_by_label("Parts");
    for part in ["body", "head", "antenna", "left_arm", "right_leg"] {
        screen.get_by_label(part);
    }
    screen.get_by_label("Problems");
    screen.get_by_label("None");
    assert!(screen.query_by_label("looking only").is_none());
    for button in ["Open", "Frame", "Grid", "Export", "Render picture"] {
        screen.get_by_label(button);
    }
}

#[test]
fn export_and_render_picture_write_next_to_the_recipe() {
    let recipe = copy_of("robot.toml", "screens_export");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);

    screen.get_by_label("Export").click();
    screen.run_steps(2);
    let model = recipe.with_extension("glb");
    assert!(model.is_file(), "Export wrote no .glb");
    assert_eq!(&std::fs::read(&model).unwrap()[..4], b"glTF");
    screen.get_by_label_contains("robot.glb");

    screen.get_by_label("Render picture").click();
    screen.run_steps(2);
    let picture = recipe.with_extension("png");
    assert!(picture.is_file(), "Render picture wrote no .png");
    screen.get_by_label_contains("robot.png");

    // Neither touched the recipe.
    assert_eq!(
        std::fs::read(&recipe).unwrap(),
        std::fs::read(examples().join("robot.toml")).unwrap()
    );
}

#[test]
fn grid_and_frame_and_the_mouse_move_the_view() {
    let recipe = copy_of("robot.toml", "screens_view");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    let studio = |screen: &Harness<'static, StudioApp>| {
        let studio = screen.state().studio().unwrap();
        (studio.orbit, studio.grid)
    };
    let (start, grid) = studio(&screen);
    assert!(grid);

    screen.get_by_label("Grid").click();
    screen.run_steps(2);
    assert!(!studio(&screen).1, "Grid should turn the grid off");

    // Dragging with the left button turns the view.
    screen.hover_at(IN_VIEW);
    screen.step();
    screen.drag_at(IN_VIEW);
    screen.step();
    screen.hover_at(IN_VIEW + egui::vec2(80.0, 30.0));
    screen.step();
    screen.drop_at(IN_VIEW + egui::vec2(80.0, 30.0));
    screen.step();
    let (turned, _) = studio(&screen);
    assert_ne!(turned.round, start.round);
    assert_ne!(turned.above, start.above);
    assert_eq!(turned.target, start.target, "turning doesn't slide");
    assert_eq!(turned.distance, start.distance);

    // Dragging with the right button slides it.
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    screen.hover_at(IN_VIEW);
    screen.step();
    screen.event(press(IN_VIEW, true));
    screen.step();
    screen.hover_at(IN_VIEW + egui::vec2(60.0, 0.0));
    screen.step();
    screen.event(press(IN_VIEW + egui::vec2(60.0, 0.0), false));
    screen.step();
    let (slid, _) = studio(&screen);
    assert_ne!(slid.target, turned.target);
    assert_eq!(slid.round, turned.round, "sliding doesn't turn");

    // The wheel zooms, when the pointer is over the picture.
    screen.hover_at(IN_VIEW);
    screen.step();
    screen.event(egui::Event::MouseWheel {
        source: egui::MouseWheelSource::Wheel,
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 100.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    screen.run_steps(4);
    let (near, _) = studio(&screen);
    assert!(near.distance < slid.distance, "rolling away comes closer");

    // Frame brings the model back, without turning.
    screen.get_by_label("Frame").click();
    screen.run_steps(2);
    let (framed, _) = studio(&screen);
    assert_eq!(framed.target, start.target);
    assert_eq!(framed.distance, start.distance);
    assert_eq!(framed.round, near.round);
}

#[test]
fn a_mistake_saved_from_outside_shows_in_problems_and_the_model_stays() {
    let recipe = copy_of("robot.toml", "screens_problems");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen.get_by_label("None");

    // Another program saves the recipe with a mistake in it.
    let good = std::fs::read_to_string(&recipe).unwrap();
    std::fs::write(&recipe, good.replace("shape = \"box\"", "shape = \"blob\"")).unwrap();
    assert!(screen.state_mut().studio_mut().unwrap().check_files());
    screen.run_steps(2);
    assert!(screen.query_by_label("None").is_none());
    screen.get_by_label("The file has mistakes. The picture is of the last version that worked.");
    assert!(screen.query_all_by_label("mistake").count() >= 1);
    // The parts of the last good version are still listed.
    screen.get_by_label("antenna");

    // Saved right again: the problems go.
    std::fs::write(&recipe, good).unwrap();
    assert!(screen.state_mut().studio_mut().unwrap().check_files());
    screen.run_steps(2);
    screen.get_by_label("None");
}

#[test]
fn preview_only_looks() {
    let recipe = copy_of("house.toml", "screens_look");
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen.get_by_label("house.toml");
    screen.get_by_label("looking only");
    screen.get_by_label("Parts");
    assert!(screen.state().studio().unwrap().look_only());

    // A model file is only looked at too, however it was opened.
    let model = examples().join("../tests/golden/robot.glb");
    let mut screen = harness(StudioApp::with_file(&model, false), false);
    screen.run_steps(2);
    screen.get_by_label("robot.glb");
    screen.get_by_label("looking only");
}

/// The path of the selected part.
fn selected(screen: &Harness<'static, StudioApp>) -> Option<String> {
    let studio = screen.state().studio().unwrap();
    studio.selected().map(str::to_owned)
}

/// Where the middle of a part is on the screen.
fn on_screen(screen: &Harness<'static, StudioApp>, part: &str) -> egui::Pos2 {
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
fn click_at(screen: &mut Harness<'static, StudioApp>, at: egui::Pos2) {
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

#[test]
fn a_part_is_selected_in_the_list_or_in_the_picture() {
    let recipe = copy_of("robot.toml", "screens_select");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    assert_eq!(selected(&screen), None);

    // In the list.
    screen.get_by_label("head").click();
    screen.run_steps(2);
    assert_eq!(selected(&screen).as_deref(), Some("head"));
    screen.get_by_label("antenna_tip").click();
    screen.run_steps(2);
    assert_eq!(selected(&screen).as_deref(), Some("antenna_tip"));
    // Clicking the selected one lets go of it. (Its name is now in the
    // properties panel too, so the row is asked for as a button.)
    screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "antenna_tip")
        .click();
    screen.run_steps(2);
    assert_eq!(selected(&screen), None);

    // In the picture: the red panel on the robot's front.
    let panel = on_screen(&screen, "panel");
    click_at(&mut screen, panel);
    assert_eq!(selected(&screen).as_deref(), Some("panel"));
    // A leg is selected whole first, then the part of it that was clicked.
    let foot = on_screen(&screen, "left_leg/foot");
    click_at(&mut screen, foot);
    assert_eq!(selected(&screen).as_deref(), Some("left_leg"));
    click_at(&mut screen, foot);
    assert_eq!(selected(&screen).as_deref(), Some("left_leg/foot"));

    // Turning the view is a drag, not a click: the selection stays.
    screen.hover_at(IN_VIEW);
    screen.step();
    screen.drag_at(IN_VIEW);
    screen.step();
    screen.hover_at(IN_VIEW + egui::vec2(40.0, 0.0));
    screen.step();
    screen.drop_at(IN_VIEW + egui::vec2(40.0, 0.0));
    screen.run_steps(2);
    assert_eq!(selected(&screen).as_deref(), Some("left_leg/foot"));

    // Escape lets go; a click on the empty ground does too.
    screen.key_press(egui::Key::Escape);
    screen.run_steps(2);
    assert_eq!(selected(&screen), None);
    screen.get_by_label("head").click();
    screen.run_steps(2);
    let (rect, _) = screen.state().viewport().unwrap();
    click_at(&mut screen, rect.min + egui::vec2(6.0, 6.0));
    assert_eq!(selected(&screen), None);

    // Selecting works when only looking, too.
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen.get_by_label("head").click();
    screen.run_steps(2);
    assert_eq!(selected(&screen).as_deref(), Some("head"));
}

#[test]
fn clicking_a_problem_selects_the_part_it_is_about() {
    let recipe = folder("screens_problem_click").join("grey.toml");
    std::fs::write(
        &recipe,
        "version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nmove = [0, 3, 0]\n",
    )
    .unwrap();
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen.get_by_label_contains("touches nothing").click();
    screen.run_steps(2);
    assert_eq!(selected(&screen).as_deref(), Some("b"));
}

// ---- the properties panel ------------------------------------------------------

/// Opens a copy of the robot and selects a part in the list.
fn robot_with(test: &str, part: &str) -> (PathBuf, Harness<'static, StudioApp>) {
    let recipe = copy_of("robot.toml", test);
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen.state_mut().studio_mut().unwrap().select(Some(part));
    screen.run_steps(2);
    (recipe, screen)
}

/// The lines of the changed recipe that differ from the example's: what
/// is left of it after taking away the lines the two start and end with
/// alike.
fn new_lines(recipe: &Path) -> Vec<String> {
    let before = std::fs::read_to_string(examples().join("robot.toml")).unwrap();
    let after = std::fs::read_to_string(recipe).unwrap();
    let (old, new): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    new[start..new.len() - end]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

#[test]
fn the_panel_shows_the_selected_part_and_changes_the_recipe() {
    use egui::accesskit::Role;
    let (recipe, mut screen) = robot_with("screens_panel", "head");
    for label in [
        "Name",
        "Made of",
        "Size",
        "Colour",
        "Joined to",
        "By its",
        "Twist",
    ] {
        assert!(
            screen.query_all_by_label(label).count() >= 1,
            "the panel should show {label}"
        );
    }
    // Nothing has been written by looking.
    assert!(new_lines(&recipe).is_empty());

    // Colour: pick another from the palette.
    screen
        .get_by_role_and_label(Role::ComboBox, "Colour")
        .click();
    screen.run_steps(2);
    screen.get_by_label("red").click();
    screen.run_steps(3);
    assert_eq!(new_lines(&recipe), ["color = \"red\""]);
    screen.get_by_label_contains("Set \"head\": colour red");

    // Name: type a new one and press Enter. The part stays selected.
    let name = screen.get_by_role_and_label(Role::TextInput, "Name");
    name.focus();
    screen.run_steps(2);
    screen.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::TextInput, "Name")
        .type_text("skull");
    screen.run_steps(2);
    screen.key_press(egui::Key::Enter);
    screen.run_steps(3);
    assert_eq!(selected(&screen).as_deref(), Some("skull"));
    let lines = new_lines(&recipe);
    assert!(lines.contains(&"name = \"skull\"".to_owned()), "{lines:?}");
    // What was joined to the head is joined to the skull.
    assert!(
        lines.contains(&"to = \"skull.antenna\"".to_owned()),
        "{lines:?}"
    );
    assert!(screen.state().studio().unwrap().problems().is_empty());
}

#[test]
fn a_part_from_a_definition_says_so_before_any_edit() {
    // Made with `use`: the eye is defined once.
    let (recipe, screen) = robot_with("screens_panel_use", "left_eye");
    screen.get_by_label(
        "Made from the definition \"eye\". Changing its shape, size or colour also changes: right_eye.",
    );
    assert!(new_lines(&recipe).is_empty());

    // Inside a definition: the arm's hand is the other arm's hand too.
    let (recipe, mut screen) = robot_with("screens_panel_inside", "left_arm/hand");
    screen.get_by_label(
        "Inside the definition \"arm\". Any change here also changes: right_arm/hand.",
    );
    assert!(new_lines(&recipe).is_empty());
    // And the change is then made in the definition, once.
    screen
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Colour")
        .click();
    screen.run_steps(2);
    screen.get_by_label("glow").click();
    screen.run_steps(3);
    assert_eq!(new_lines(&recipe), ["color = \"glow\""]);
    screen.get_by_label_contains("(also changed: right_arm/hand)");

    // A part written once has no such notice.
    let (_, screen) = robot_with("screens_panel_once", "head");
    assert!(screen.query_by_label_contains("the definition").is_none());
}

#[test]
fn duplicate_mirror_copy_and_remove_are_buttons() {
    let (recipe, mut screen) = robot_with("screens_panel_buttons", "antenna_tip");
    screen.get_by_label("Duplicate").click();
    screen.run_steps(3);
    let copy = selected(&screen).unwrap();
    assert_ne!(copy, "antenna_tip");
    assert!(new_lines(&recipe).contains(&format!("name = \"{copy}\"")));

    // Nothing hangs from the copy: it goes at once.
    screen.get_by_label("Remove").click();
    screen.run_steps(3);
    assert_eq!(selected(&screen), None);
    assert!(new_lines(&recipe).is_empty(), "{:?}", new_lines(&recipe));

    // The head has eyes and an antenna hanging from it: the Studio asks.
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    screen.get_by_label("Remove").click();
    screen.run_steps(2);
    screen.get_by_label("4 other parts hang from this one. Remove them too?");
    assert!(screen.state().studio().unwrap().part("head").is_some());
    screen.get_by_label("Keep them").click();
    screen.run_steps(2);
    assert!(screen.state().studio().unwrap().part("head").is_some());
    screen.get_by_label("Remove").click();
    screen.run_steps(2);
    screen.get_by_label("Remove them all").click();
    screen.run_steps(3);
    let studio = screen.state().studio().unwrap();
    for gone in ["head", "left_eye", "right_eye", "antenna", "antenna_tip"] {
        assert!(studio.part(gone).is_none(), "{gone} should be gone");
    }
    assert!(studio.part("body").is_some());

    // A mirror copy of the left arm is a right arm.
    let (recipe, mut screen) = robot_with("screens_panel_mirror", "left_leg");
    screen.get_by_label("Mirror copy").click();
    screen.run_steps(3);
    let lines = new_lines(&recipe);
    // A new part, mirrored, on the socket of the other side.
    assert_eq!(
        lines,
        [
            "name = \"left_leg_2\"",
            "use = \"leg\"",
            "attach = \"hip\"",
            "to = \"body.right_leg\"",
            "mirror = \"x\"",
            "",
            "[[parts]]"
        ]
    );
}

#[test]
fn when_only_looking_the_panel_changes_nothing() {
    let recipe = copy_of("robot.toml", "screens_panel_look");
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("antenna_tip"));
    screen.run_steps(2);
    screen.get_by_label("Looking only: nothing can be changed here.");
    screen.get_by_label("Remove").click();
    screen.run_steps(3);
    assert!(
        screen
            .state()
            .studio()
            .unwrap()
            .part("antenna_tip")
            .is_some()
    );
    assert!(new_lines(&recipe).is_empty());
}

/// Pictures of whole screens, compared with the stored ones in
/// `tests/screens/`. Linux only: see the top of this file.
#[cfg(target_os = "linux")]
mod picture {
    use super::*;
    use egui_kittest::SnapshotOptions;

    fn check(screen: &mut Harness<'static, StudioApp>, name: &str) {
        let stored = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/screens");
        screen.snapshot_options(
            name,
            &SnapshotOptions::new()
                .threshold(2.0)
                .max_failed_pixels(200)
                .output_path(stored),
        );
    }

    #[test]
    fn the_start_page() {
        let mut screen = harness(StudioApp::new(), true);
        screen.run_steps(3);
        check(&mut screen, "start");
    }

    #[test]
    fn a_recipe_open() {
        // Opened where it is, not from a copy: nothing is written, and
        // the path isn't on the screen.
        let mut screen = harness(
            StudioApp::with_file(examples().join("robot.toml"), false),
            true,
        );
        screen.run_steps(3);
        check(&mut screen, "robot");
    }

    #[test]
    fn a_part_selected() {
        let mut screen = harness(
            StudioApp::with_file(examples().join("robot.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen.get_by_label("left_arm").click();
        screen.run_steps(2);
        screen.remove_cursor();
        screen.run_steps(2);
        check(&mut screen, "selected");
    }

    #[test]
    fn a_part_from_a_definition_selected() {
        let mut screen = harness(
            StudioApp::with_file(examples().join("robot.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen.get_by_label("left_eye").click();
        screen.run_steps(2);
        screen.remove_cursor();
        screen.run_steps(2);
        check(&mut screen, "definition");
    }

    #[test]
    fn a_recipe_with_a_mistake() {
        let recipe = copy_of("house.toml", "screens_picture_mistake");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        let good = std::fs::read_to_string(&recipe).unwrap();
        std::fs::write(&recipe, good.replace("shape = \"box\"", "shape = \"blob\"")).unwrap();
        assert!(screen.state_mut().studio_mut().unwrap().check_files());
        screen.run_steps(3);
        check(&mut screen, "mistake");
    }
}
