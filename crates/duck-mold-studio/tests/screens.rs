//! The Studio's screens, run with no window.
//!
//! `egui_kittest` runs the app's own drawing code against a pretend
//! screen, finds things by their label and clicks them, so these tests
//! need no display. The Studio underneath is the real one, working on a
//! copy of an example recipe.
//!
//! These run on Windows and Linux. Pictures of whole screens are in
//! `pictures.rs`.

use std::path::{Path, PathBuf};

use duck_mold_studio::StudioApp;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

mod common;
use common::*;

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

// ---- sockets -------------------------------------------------------------------

/// Presses on one point, moves to another in a few steps, and lets go.
fn drag(screen: &mut Harness<'static, StudioApp>, from: egui::Pos2, to: egui::Pos2) {
    screen.hover_at(from);
    screen.step();
    screen.drag_at(from);
    screen.step();
    for part in 1..=4 {
        screen.hover_at(from + (to - from) * (part as f32 / 4.0));
        screen.step();
    }
    screen.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    screen.run_steps(3);
}

#[test]
fn a_click_on_a_socket_offers_what_to_add_there() {
    let (recipe, mut screen) = robot_with("screens_socket", "head");
    let on_right = socket_on_screen(&screen, "head", "right");
    click_at(&mut screen, on_right);
    // The list: the five shapes, then the parts the recipe defines.
    screen.get_by_label("Add a part on head.right");
    for shape in ["Box", "Cylinder", "Sphere", "Cone", "Wedge"] {
        screen.get_by_label(shape);
    }
    screen.get_by_label("Defined in this recipe");
    assert!(new_lines(&recipe).is_empty(), "nothing is added yet");

    screen.get_by_label("Sphere").click();
    screen.run_steps(3);
    assert!(screen.query_by_label("Add a part on head.right").is_none());
    assert_eq!(selected(&screen).as_deref(), Some("sphere"));
    assert_eq!(
        new_lines(&recipe),
        [
            "",
            "[[parts]]",
            "name = \"sphere\"",
            "shape = \"sphere\"",
            "radius = 0.08",
            "color = \"steel\"",
            "attach = \"left\"",
            "to = \"head.right\"",
        ]
    );

    // Escape closes the list without adding anything.
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    let on_top = socket_on_screen(&screen, "head", "top");
    click_at(&mut screen, on_top);
    screen.get_by_label("Add a part on head.top");
    screen.key_press(egui::Key::Escape);
    screen.run_steps(2);
    assert!(screen.query_by_label("Add a part on head.top").is_none());
    assert_eq!(selected(&screen).as_deref(), Some("head"));
}

#[test]
fn a_click_on_the_selected_parts_surface_adds_a_part_at_that_point() {
    let (recipe, mut screen) = robot_with("screens_surface", "body");
    // A point of the body's front, beside the red panel.
    let (rect, [width, height]) = screen.state().viewport().unwrap();
    let at = {
        let studio = screen.state().studio().unwrap();
        let body = studio.part("body").unwrap().bounds.unwrap();
        let point = body.center()
            + duck_mold_core::math::vec3(body.half().x * 0.75, body.half().y * -0.6, body.half().z);
        let (x, y) = studio.orbit.on_screen(point, width, height).unwrap();
        rect.min
            + egui::vec2(
                x as f32 * rect.width() / width as f32,
                y as f32 * rect.height() / height as f32,
            )
    };
    click_at(&mut screen, at);
    screen.get_by_label("Add a part on the front of body");
    screen.get_by_label("Cone").click();
    screen.run_steps(3);
    assert_eq!(selected(&screen).as_deref(), Some("cone"));
    let studio = screen.state().studio().unwrap();
    assert_eq!(
        studio.part("cone").unwrap().attached_to.as_deref(),
        Some("body.spot")
    );
    let lines = new_lines(&recipe);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("spot = { on = \"front\", shift = [0.75, -0.6")),
        "{lines:?}"
    );
    assert!(studio.problems().is_empty(), "{:?}", studio.problems());
}

#[test]
fn dragging_the_selected_part_moves_it_and_dragging_elsewhere_turns_the_view() {
    let (recipe, mut screen) = robot_with("screens_drag", "antenna_tip");
    let before = screen.state().studio().unwrap().orbit;

    // From the tip to the body's right side socket.
    let from = on_screen(&screen, "antenna_tip");
    let to = socket_on_screen(&screen, "body", "right");
    drag(&mut screen, from, to);
    assert_eq!(new_lines(&recipe), ["to = \"body.right\""]);
    let studio = screen.state().studio().unwrap();
    assert_eq!(studio.orbit.round, before.round, "the view didn't turn");
    assert_eq!(studio.orbit.above, before.above);
    assert_eq!(studio.status(), "Joined \"antenna_tip\" to body.right");
    assert_eq!(selected(&screen).as_deref(), Some("antenna_tip"));

    // A drag that starts off the part turns the view and moves nothing.
    let (rect, _) = screen.state().viewport().unwrap();
    let empty = rect.min + egui::vec2(30.0, 30.0);
    drag(&mut screen, empty, empty + egui::vec2(60.0, 0.0));
    let studio = screen.state().studio().unwrap();
    assert_ne!(studio.orbit.round, before.round);
    assert_eq!(new_lines(&recipe), ["to = \"body.right\""]);

    // When only looking, a drag on the selected part turns the view too.
    let recipe = copy_of("robot.toml", "screens_drag_look");
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    let from = on_screen(&screen, "body");
    drag(&mut screen, from, from + egui::vec2(50.0, 0.0));
    assert!(new_lines(&recipe).is_empty());
    assert_ne!(screen.state().studio().unwrap().orbit.round, before.round);
}

// ---- undo and redo -------------------------------------------------------------

#[test]
fn undo_and_redo_are_buttons_and_keys() {
    use egui::accesskit::Role;
    let (recipe, mut screen) = robot_with("screens_undo", "head");
    let original = std::fs::read(&recipe).unwrap();

    // Two edits: a colour, and a part added on a socket.
    screen
        .get_by_role_and_label(Role::ComboBox, "Colour")
        .click();
    screen.run_steps(2);
    screen.get_by_label("red").click();
    screen.run_steps(3);
    let coloured = std::fs::read(&recipe).unwrap();
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    let on_right = socket_on_screen(&screen, "head", "right");
    click_at(&mut screen, on_right);
    screen.get_by_label("Box").click();
    screen.run_steps(3);
    let added = std::fs::read(&recipe).unwrap();
    assert_ne!(added, coloured);

    // The buttons.
    screen.get_by_label("Undo").click();
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), coloured);
    screen.get_by_label("Undid: Added \"box\" (box) on head.right");
    screen.get_by_label("Undo").click();
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), original);
    screen.get_by_label("Redo").click();
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), coloured);

    // The keys: Ctrl+Y redoes, Ctrl+Z undoes, Ctrl+Shift+Z redoes.
    screen.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Y);
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), added);
    screen.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), coloured);
    screen.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), added);

    // A change from outside is undone the same way.
    let text = String::from_utf8(added.clone()).unwrap();
    std::fs::write(
        &recipe,
        text.replace("name = \"robot\"", "name = \"robot2\""),
    )
    .unwrap();
    assert!(screen.state_mut().studio_mut().unwrap().check_files());
    screen.run_steps(2);
    screen.get_by_label("Undo").click();
    screen.run_steps(3);
    assert_eq!(std::fs::read(&recipe).unwrap(), added);
    screen.get_by_label("Undid: Changed outside the Studio");

    // When only looking there is nothing to undo, and no buttons for it.
    let mut looking = harness(StudioApp::with_file(&recipe, true), false);
    looking.run_steps(2);
    assert!(looking.query_by_label("Undo").is_none());
    assert!(looking.query_by_label("Redo").is_none());
}

// ---- the parts library ---------------------------------------------------------

#[test]
fn a_library_part_is_chosen_in_the_add_list() {
    // The robot, with a `parts` folder beside it.
    let recipe = copy_of("robot.toml", "screens_library");
    let parts = recipe.with_file_name("parts");
    std::fs::create_dir_all(&parts).unwrap();
    std::fs::copy(examples().join("parts/crate.glb"), parts.join("crate.glb")).unwrap();
    std::fs::write(
        parts.join("hat.toml"),
        "version = 0\nname = \"hat\"\n[palette]\nfelt = \"#333344\"\n[[parts]]\nname = \"cone\"\nshape = \"cone\"\nradius = 0.15\nheight = 0.25\ncolor = \"felt\"\n",
    )
    .unwrap();
    std::fs::write(
        parts.join("broken.toml"),
        "version = 0\n[[parts]]\nshape = \"blob\"\n",
    )
    .unwrap();
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);

    // The panel under the parts list shows what is in the folder.
    screen.get_by_label("Library");
    screen.get_by_label("parts");
    screen.get_by_label("crate");
    screen.get_by_label("hat");
    screen.get_by_label("broken");

    // Click a socket: the list offers the library's parts too.
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    let on_top = socket_on_screen(&screen, "head", "top");
    click_at(&mut screen, on_top);
    screen.get_by_label("From the library");
    // (Each is on the screen twice now: in the panel and in the list.
    // The list's is the later one.)
    let hat = screen.query_all_by_label("hat").last().unwrap();
    hat.click();
    screen.run_steps(3);
    assert_eq!(selected(&screen).as_deref(), Some("hat"));
    assert_eq!(
        new_lines(&recipe),
        [
            "",
            "[[parts]]",
            "name = \"hat\"",
            "file = \"parts/hat.toml\"",
            "attach = \"bottom\"",
            "to = \"head.top\"",
        ]
    );
    // It built; and inspect noticed that the hat covers the antenna.
    let studio = screen.state().studio().unwrap();
    assert!(!studio.has_mistakes(), "{:?}", studio.problems());
    assert!(studio.part("hat/cone").is_some());
    screen.get_by_label_contains("is hidden inside \"hat/cone\"");

    // When only looking, there is no library to add from.
    let mut looking = harness(StudioApp::with_file(&recipe, true), false);
    looking.run_steps(2);
    assert!(looking.query_by_label("Library").is_none());
}

// ---- the start page's list of recipes opened lately ----------------------------

#[test]
fn recipes_opened_lately_are_on_the_start_page() {
    use duck_mold_studio::recent::Recent;
    let folder = folder("screens_recent");
    let list = folder.join("settings/recent.txt");
    for name in ["robot.toml", "house.toml"] {
        std::fs::copy(examples().join(name), folder.join(name)).unwrap();
    }

    // Nothing opened yet: no list on the start page.
    let mut screen = harness(
        StudioApp::new().remembering(Recent::kept_in(list.clone())),
        false,
    );
    screen.step();
    assert!(screen.query_by_label("Opened lately").is_none());

    // Open two recipes, one after the other.
    screen.state_mut().open(folder.join("robot.toml"), false);
    screen.state_mut().open(folder.join("house.toml"), false);
    screen.run_steps(2);
    let written = std::fs::read_to_string(&list).unwrap();
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].ends_with("house.toml"),
        "the newest is first: {lines:?}"
    );
    assert!(lines[1].ends_with("robot.toml"));

    // The next time the Studio starts, they are on the start page, and a
    // click opens one.
    let mut screen = harness(
        StudioApp::new().remembering(Recent::kept_in(list.clone())),
        false,
    );
    screen.step();
    screen.get_by_label("Opened lately");
    screen.get_by_label("house.toml");
    screen.get_by_label("robot.toml").click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "robot.toml - Duck Mold Studio");
    let written = std::fs::read_to_string(&list).unwrap();
    assert!(written.lines().next().unwrap().ends_with("robot.toml"));

    // A recipe that has been deleted since is left out.
    std::fs::remove_file(folder.join("house.toml")).unwrap();
    let mut screen = harness(
        StudioApp::new().remembering(Recent::kept_in(list.clone())),
        false,
    );
    screen.step();
    screen.get_by_label("robot.toml");
    assert!(screen.query_by_label("house.toml").is_none());

    // A new model is remembered too; and only the latest eight are kept.
    for number in 0..10 {
        let recipe = folder.join(format!("model{number}.toml"));
        screen.state_mut().start_new(&recipe);
    }
    assert_eq!(std::fs::read_to_string(&list).unwrap().lines().count(), 8);
}
