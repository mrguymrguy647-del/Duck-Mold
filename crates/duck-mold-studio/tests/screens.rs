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
use egui_kittest::kittest::{NodeT, Queryable};

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
    for button in [
        "File",
        "Edit",
        "View",
        "Help",
        "Front",
        "Side",
        "Top",
        "3/4",
        "Frame",
        "Grid",
        "Wire",
        "Shadow",
        "Export",
        "Render picture",
    ] {
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
    // Nothing can be added there.
    assert!(screen.query_by_label("Add").is_none());

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
    // (The later one: the palette panel lists the colour too.)
    screen.query_all_by_label("red").last().unwrap().click();
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
    // (The later one: the palette panel lists the colour too.)
    screen.query_all_by_label("glow").last().unwrap().click();
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

#[test]
fn a_click_on_a_socket_offers_what_to_add_there() {
    let (recipe, mut screen) = robot_with("screens_socket", "head");
    let on_right = socket_on_screen(&screen, "head", "right");
    click_at(&mut screen, on_right);
    // The list: the shapes, then the parts the recipe defines.
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

// ---- handles -------------------------------------------------------------------

/// A floor with a crate standing free on it, open, with the crate
/// selected.
fn boxes(test: &str) -> (PathBuf, Harness<'static, StudioApp>) {
    let recipe = folder(test).join("boxes.toml");
    std::fs::write(
        &recipe,
        "version = 0\nname = \"boxes\"\n\n[palette]\nstone = \"#C0C0C0\"\nred = \"#D0453A\"\n\n[[parts]]\nname = \"floor\"\nshape = \"box\"\nsize = [2, 0.2, 2]\ncolor = \"stone\"\n\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.5, 0.5, 0.5]\ncolor = \"red\"\nmove = [0.5, 0.35, 0]\n",
    )
    .unwrap();
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(2);
    (recipe, screen)
}

fn move_line(recipe: &Path) -> String {
    let text = std::fs::read_to_string(recipe).unwrap();
    text.lines()
        .find(|line| line.starts_with("move = "))
        .unwrap()
        .to_owned()
}

#[test]
fn the_cube_can_be_dragged_by_its_arrows_and_by_itself() {
    use duck_mold_core::math::vec3;
    use duck_mold_view::studio::handles::{Grip, Tool};
    let (recipe, mut screen) = boxes("screens_handles_move");
    // The Studio opens with the Move tool in hand.
    assert_eq!(screen.state().studio().unwrap().tool, Tool::Move);
    screen.get_by_label("Move");
    assert_eq!(
        screen.state().studio().unwrap().snap_text(),
        "Snap 0.1 m, 15°  (Ctrl: off)"
    );

    // The red arrow, dragged 0.3 m along x.
    let from = screen
        .state()
        .studio()
        .unwrap()
        .grip_at(Grip::Arrow(0))
        .unwrap();
    let (start, end) = (
        point_on_screen(&screen, from),
        point_on_screen(&screen, from + vec3(0.3, 0.0, 0.0)),
    );
    drag(&mut screen, start, end);
    assert_eq!(move_line(&recipe), "move = [0.8, 0.35, 0]");
    let studio = screen.state().studio().unwrap();
    assert_eq!(studio.status(), "Set \"crate\": move [0.8, 0.35, 0]");
    assert_eq!(studio.selected(), Some("crate"));
    // It was one write, and is one step to undo.
    assert_eq!(
        studio.can_undo(),
        Some("Set \"crate\": move [0.8, 0.35, 0]")
    );
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(move_line(&recipe), "move = [0.5, 0.35, 0]");
    assert_eq!(screen.state().studio().unwrap().can_undo(), None);

    // The crate itself, dragged across the floor.
    let hold = {
        let studio = screen.state().studio().unwrap();
        let top = studio.part("crate").unwrap().bounds.unwrap();
        vec3(top.center().x + 0.15, top.max.y, top.center().z + 0.15) + studio.offset()
    };
    let (start, end) = (
        point_on_screen(&screen, hold),
        point_on_screen(&screen, hold + vec3(-0.6, 0.0, 0.4)),
    );
    drag(&mut screen, start, end);
    assert_eq!(move_line(&recipe), "move = [-0.1, 0.35, 0.4]");

    // A drag on empty space still turns the view, and moves nothing.
    let before = screen.state().studio().unwrap().orbit;
    let (rect, _) = screen.state().viewport().unwrap();
    let empty = rect.right_top() + egui::vec2(-60.0, 60.0);
    drag(&mut screen, empty, empty + egui::vec2(-50.0, 0.0));
    assert_ne!(screen.state().studio().unwrap().orbit.round, before.round);
    assert_eq!(move_line(&recipe), "move = [-0.1, 0.35, 0.4]");
}

#[test]
fn the_tools_are_buttons_and_keys() {
    use duck_mold_core::math::{Mat3, vec3};
    use duck_mold_view::studio::handles::{Grip, Tool};
    let (recipe, mut screen) = boxes("screens_handles_tools");
    let tool = |screen: &Harness<'static, StudioApp>| screen.state().studio().unwrap().tool;

    screen.get_by_label("Rotate").click();
    screen.run_steps(2);
    assert_eq!(tool(&screen), Tool::Rotate);
    screen.key_press(egui::Key::S);
    screen.run_steps(2);
    assert_eq!(tool(&screen), Tool::Scale);
    screen.key_press(egui::Key::G);
    screen.run_steps(2);
    assert_eq!(tool(&screen), Tool::Move);
    screen.key_press(egui::Key::R);
    screen.run_steps(2);
    assert_eq!(tool(&screen), Tool::Rotate);

    // The green ring, turned a little over 45 degrees: it snaps to 45.
    let (from, middle) = {
        let studio = screen.state().studio().unwrap();
        (
            studio.grip_at(Grip::Ring(1)).unwrap(),
            studio.part("crate").unwrap().place.shift + studio.offset(),
        )
    };
    let to = middle + Mat3::around_y(48.0) * (from - middle);
    let (start, end) = (point_on_screen(&screen, from), point_on_screen(&screen, to));
    drag(&mut screen, start, end);
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("rotate = [0, 45, 0]"), "{text}");

    // The scale tool's red box, pulled out to twice as far.
    screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "Scale")
        .click();
    screen.run_steps(2);
    let (from, middle) = {
        let studio = screen.state().studio().unwrap();
        (
            studio.grip_at(Grip::Box(1)).unwrap(),
            studio.part("crate").unwrap().bounds.unwrap().center() + studio.offset(),
        )
    };
    let to = middle + (from - middle) * 2.0;
    let (start, end) = (point_on_screen(&screen, from), point_on_screen(&screen, to));
    drag(&mut screen, start, end);
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("scale = [1, 2, 1]"), "{text}");

    // The Size tool (T): the knob on the crate's top, pulled up 0.4 m.
    // The crate is scaled to twice its height, so that is 0.2 of its
    // own size.
    screen.key_press(egui::Key::T);
    screen.run_steps(2);
    assert_eq!(tool(&screen), Tool::Size);
    screen.get_by_role_and_label(egui::accesskit::Role::Button, "Size");
    let from = screen
        .state()
        .studio()
        .unwrap()
        .grip_at(Grip::Knob(2))
        .unwrap();
    let (start, end) = (
        point_on_screen(&screen, from),
        point_on_screen(&screen, from + vec3(0.0, 0.4, 0.0)),
    );
    drag(&mut screen, start, end);
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("size = [0.5, 0.7, 0.5]"), "{text}");

    // When only looking there are no tools.
    let mut looking = harness(StudioApp::with_file(&recipe, true), false);
    looking.run_steps(2);
    assert!(looking.query_by_label("Rotate").is_none());
}

#[test]
fn the_panel_has_position_rotation_and_scale_and_unjoin() {
    use egui::accesskit::Role;
    let (recipe, mut screen) = boxes("screens_handles_panel");
    for label in ["Position", "Rotation", "Scale"] {
        assert!(screen.query_all_by_label(label).count() >= 1, "{label}");
    }
    // A joined part has no position of its own, and can be unjoined.
    let (robot, mut joined) = robot_with("screens_handles_unjoin", "head");
    assert!(joined.query_by_label("Position").is_none());
    assert!(joined.query_all_by_label("Scale").count() >= 1);
    joined.get_by_label("Unjoin").click();
    joined.run_steps(3);
    let lines = new_lines(&robot);
    assert!(
        lines.iter().any(|line| line.starts_with("move = [")),
        "{lines:?}"
    );
    assert!(joined.query_all_by_label("Position").count() >= 1);
    assert!(joined.query_by_label("Unjoin").is_none());
    let studio = joined.state().studio().unwrap();
    assert_eq!(studio.part("head").unwrap().attached_to, None);
    assert!(studio.problems().is_empty(), "{:?}", studio.problems());
    let _ = (Role::Button, &recipe, &mut screen);
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
    // (The later one: the palette panel lists the colour too.)
    screen.query_all_by_label("red").last().unwrap().click();
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

#[test]
fn the_add_menu_stands_a_new_part_on_the_ground_ready_to_move() {
    use duck_mold_core::math::vec3;
    use duck_mold_view::studio::handles::{Grip, Tool};
    let (recipe, mut screen) = boxes("screens_add_menu");
    let before = std::fs::read_to_string(&recipe).unwrap();
    screen.state_mut().studio_mut().unwrap().tool = Tool::Size;

    // The menu lists the shapes.
    screen.get_by_label("Add").click();
    screen.run_steps(3);
    screen.get_by_label("A new part, on the ground");
    for shape in ["Box", "Sphere", "Cone", "Wedge"] {
        screen.get_by_role_and_label(egui::accesskit::Role::Button, shape);
    }
    screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "Cylinder")
        .click();
    screen.run_steps(3);
    // The menu has closed.
    assert!(screen.query_by_label("A new part, on the ground").is_none());

    // A cylinder, joined to nothing, in the selected crate's colour.
    let now = std::fs::read_to_string(&recipe).unwrap();
    let new = now.strip_prefix(before.as_str()).unwrap();
    assert!(
        new.starts_with(
            "\n[[parts]]\nname = \"cylinder\"\nshape = \"cylinder\"\nradius = 0.175\nheight = 0.35\ncolor = \"red\"\nmove = ["
        ),
        "{new}"
    );
    {
        let studio = screen.state().studio().unwrap();
        assert_eq!(
            studio.status(),
            "Added \"cylinder\" (cylinder), standing free"
        );
        // It is selected, with the Move tool in hand, standing on the
        // ground clear of the other two.
        assert_eq!(studio.selected(), Some("cylinder"));
        assert_eq!(studio.tool, Tool::Move);
        let model = studio.model().unwrap();
        let it = studio.part("cylinder").unwrap().bounds.unwrap();
        assert!((it.min.y - model.bounds().unwrap().min.y).abs() < 1e-9);
        for other in ["floor", "crate"] {
            let gap = studio.part(other).unwrap().bounds.unwrap().gap_to(&it);
            assert!(gap > 0.05, "{other}: {gap}");
        }
    }
    // The panel shows where it stands.
    assert!(screen.get_all_by_label("Position").count() >= 1);

    // And it moves: its red arrow, dragged 0.3 m along x.
    let x_of = |text: &str| -> f64 {
        let line = text
            .lines()
            .rev()
            .find(|line| line.starts_with("move = "))
            .unwrap();
        line["move = [".len()..]
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let from = screen
        .state()
        .studio()
        .unwrap()
        .grip_at(Grip::Arrow(0))
        .unwrap();
    let (start, end) = (
        point_on_screen(&screen, from),
        point_on_screen(&screen, from + vec3(0.3, 0.0, 0.0)),
    );
    drag(&mut screen, start, end);
    let moved = std::fs::read_to_string(&recipe).unwrap();
    assert!((x_of(&moved) - x_of(&now) - 0.3).abs() < 1e-6, "{moved}");

    // Adding was one step: two undos give the file back.
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);
}

// ---- the newer shapes ------------------------------------------------------------

#[test]
fn the_panel_has_a_row_for_every_size_and_option_of_the_newer_shapes() {
    use egui::accesskit::Role;
    // Which rows each part should have, and which it shouldn't.
    for (example, part, rows, not) in [
        (
            "gate.toml",
            "arch",
            &["Size", "Wall", "Sides"][..],
            &["Radius", "Bevel", "Taper", "Steps"][..],
        ),
        (
            "gate.toml",
            "left_tower/shaft",
            &["Size", "Bevel", "Taper"][..],
            &["Wall", "Sides"][..],
        ),
        (
            "temple.toml",
            "steps",
            &["Size", "Steps"][..],
            &["Bevel", "Taper", "Radius"][..],
        ),
        (
            "doughnut.toml",
            "dough",
            &["Radius", "Tube", "Sides"][..],
            // (Not "Size": that is also the name of a tool's button.)
            &["Height", "Wall"][..],
        ),
        (
            "mug.toml",
            "cup",
            &["Radius", "Wall", "Height", "Sides", "Taper"][..],
            &["Bevel", "Tube"][..],
        ),
        (
            "dog.toml",
            "body",
            &["Radius", "Height", "Sides"][..],
            &["Bevel", "Taper"][..],
        ),
    ] {
        let recipe = copy_of(
            example,
            &format!("screens_newer_{}", part.replace('/', "_")),
        );
        let mut screen = harness(StudioApp::with_file(&recipe, false), false);
        screen.run_steps(2);
        assert!(screen.state_mut().studio_mut().unwrap().select(Some(part)));
        screen.run_steps(3);
        for row in rows {
            assert!(
                screen.query_all_by_label(row).count() >= 1,
                "{part} should have a row {row}"
            );
        }
        for row in not {
            assert_eq!(
                screen.query_all_by_label(row).count(),
                0,
                "{part} shouldn't have a row {row}"
            );
        }
    }

    // Every shape is in the "Made of" list, and choosing one changes the
    // recipe: the gate's door becomes a pyramid.
    let recipe = copy_of("gate.toml", "screens_newer_made_of");
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    assert!(
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("door"))
    );
    screen.run_steps(3);
    screen
        .get_by_role_and_label(Role::ComboBox, "Made of")
        .click();
    screen.run_steps(2);
    for shape in duck_mold_core::recipe::Shape::NAMES {
        assert!(
            screen.query_all_by_label(shape).count() >= 1,
            "{shape} isn't offered"
        );
    }
    screen.query_all_by_label("pyramid").last().unwrap().click();
    screen.run_steps(3);
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(
        text.contains("name = \"door\"\nshape = \"pyramid\"\nsize = [2, 1.7, 0.12]"),
        "{text}"
    );
    assert!(!screen.state().studio().unwrap().has_mistakes());
}

// ---- several parts, the right-click menu, the keys, hiding ----------------------

const YARD: &str = "version = 0\nname = \"yard\"\n\n[palette]\nstone = \"#C0C0C0\"\nred = \"#D0453A\"\nblue = \"#3A6FD0\"\n\n[[parts]]\nname = \"floor\"\nshape = \"box\"\nsize = [4, 0.2, 4]\ncolor = \"stone\"\n\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.5, 0.5, 0.5]\ncolor = \"red\"\nmove = [0.8, 0.35, 0.6]\n\n[[parts]]\nname = \"post\"\nshape = \"cylinder\"\nradius = 0.1\nheight = 1\ncolor = \"blue\"\nmove = [-0.8, 0.6, 0.5]\n\n[[parts]]\nname = \"cap\"\nshape = \"sphere\"\nradius = 0.15\ncolor = \"red\"\nattach = \"bottom\"\nto = \"post.top\"\n";

fn yard(test: &str) -> (PathBuf, Harness<'static, StudioApp>) {
    let recipe = folder(test).join("yard.toml");
    std::fs::write(&recipe, YARD).unwrap();
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    (recipe, screen)
}

fn text_of(recipe: &Path) -> String {
    std::fs::read_to_string(recipe).unwrap()
}

#[test]
fn shift_click_selects_several_and_the_panel_is_for_them_all() {
    use egui::accesskit::Role;
    let (recipe, mut screen) = yard("screens_several");
    // In the picture.
    let crate_at = on_screen(&screen, "crate");
    click_at(&mut screen, crate_at);
    assert_eq!(selection(&screen), ["crate"]);
    let post_at = on_screen(&screen, "post");
    shift_click_at(&mut screen, post_at);
    assert_eq!(selection(&screen), ["crate", "post"]);
    screen.get_by_label("2 parts");
    screen.get_by_label("crate, post");
    // And in the list: Shift+click takes the crate out, and puts it back.
    for now in [vec!["post"], vec!["post", "crate"]] {
        let row = screen
            .get_by_role_and_label(Role::Button, "crate")
            .rect()
            .center();
        shift_click_at(&mut screen, row);
        assert_eq!(selection(&screen), now);
    }

    // One colour for both.
    screen
        .get_by_role_and_label(Role::ComboBox, "Colour")
        .click();
    screen.run_steps(2);
    // (The later one: the palette panel lists the colour too.)
    screen.query_all_by_label("stone").last().unwrap().click();
    screen.run_steps(3);
    assert_eq!(text_of(&recipe).matches("color = \"stone\"").count(), 3);
    screen.get_by_label_contains("Coloured 2 parts stone");
    assert_eq!(selection(&screen), ["post", "crate"]);

    // The arrows move both: one write, one step to undo.
    use duck_mold_core::math::vec3;
    use duck_mold_view::studio::handles::Grip;
    let from = screen
        .state()
        .studio()
        .unwrap()
        .grip_at(Grip::Arrow(0))
        .unwrap();
    let (start, end) = (
        point_on_screen(&screen, from),
        point_on_screen(&screen, from + vec3(0.5, 0.0, 0.0)),
    );
    drag(&mut screen, start, end);
    let now = text_of(&recipe);
    assert!(now.contains("move = [1.3, 0.35, 0.6]"), "{now}");
    assert!(now.contains("move = [-0.3, 0.6, 0.5]"), "{now}");
    assert_eq!(
        screen.state().studio().unwrap().status(),
        "Moved 2 parts: x +0.5 m"
    );
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(text_of(&recipe), YARD);

    // A plain click on one of them selects just that one again.
    let crate_at = on_screen(&screen, "crate");
    click_at(&mut screen, crate_at);
    assert_eq!(selection(&screen), ["crate"]);
}

#[test]
fn the_keys_copy_remove_hide_frame_and_select() {
    use egui::{Key, Modifiers};
    let (recipe, mut screen) = yard("screens_keys");
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(2);

    // Ctrl+D: a copy, beside it, selected.
    screen.key_press_modifiers(Modifiers::COMMAND, Key::D);
    screen.run_steps(3);
    assert_eq!(selection(&screen), ["crate_2"]);
    assert!(text_of(&recipe).contains("move = [1.4, 0.35, 0.6]"));
    // Delete: gone again. Nothing hangs from it, so nothing is asked.
    screen.key_press(Key::Delete);
    screen.run_steps(3);
    assert!(selection(&screen).is_empty());
    assert_eq!(text_of(&recipe), YARD);

    // Delete on a part that another hangs from asks first, in the panel.
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("post"));
    screen.run_steps(2);
    screen.key_press(Key::Backspace);
    screen.run_steps(3);
    assert_eq!(text_of(&recipe), YARD);
    screen.get_by_label("Another part hangs from this one. Remove it too?");
    screen.get_by_label("Keep them").click();
    screen.run_steps(2);
    assert_eq!(text_of(&recipe), YARD);
    screen.key_press(Key::Delete);
    screen.run_steps(3);
    screen.get_by_label("Remove them all").click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(!now.contains("post") && !now.contains("cap"), "{now}");
    screen.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    screen.run_steps(3);
    assert_eq!(text_of(&recipe), YARD);

    // Ctrl+A selects every part; H hides them; the bar says how many,
    // and Shift+H shows them again.
    screen.key_press_modifiers(Modifiers::COMMAND, Key::A);
    screen.run_steps(2);
    assert_eq!(selection(&screen), ["floor", "crate", "post", "cap"]);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(2);
    screen.key_press(Key::H);
    screen.run_steps(3);
    assert!(screen.state().studio().unwrap().is_hidden("crate"));
    assert!(selection(&screen).is_empty());
    screen.get_by_label("1 hidden");
    // The recipe knows nothing of it.
    assert_eq!(text_of(&recipe), YARD);
    screen.key_press_modifiers(Modifiers::SHIFT, Key::H);
    screen.run_steps(3);
    assert!(!screen.state().studio().unwrap().is_hidden("crate"));
    assert!(screen.query_by_label("1 hidden").is_none());

    // The eye at the end of a line of the list does the same for one.
    let eyes: Vec<_> = screen.query_all_by_label("hide").collect();
    assert_eq!(eyes.len(), 4, "one for each part");
    eyes[1].click();
    screen.run_steps(3);
    assert!(screen.state().studio().unwrap().is_hidden("crate"));
    screen.get_by_label("show").click();
    screen.run_steps(3);
    assert!(!screen.state().studio().unwrap().is_hidden("crate"));

    // F brings the selected part into view: closer than the whole yard.
    let far = screen.state().studio().unwrap().orbit.distance;
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(2);
    screen.key_press(Key::F);
    screen.run_steps(3);
    assert!(screen.state().studio().unwrap().orbit.distance < far * 0.6);

    // Shift+A opens the list of what to add on the ground.
    screen.key_press_modifiers(Modifiers::SHIFT, Key::A);
    screen.run_steps(3);
    screen.get_by_label("Add a part on the ground");
    screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "Torus")
        .click();
    screen.run_steps(3);
    assert_eq!(selection(&screen), ["torus"]);
    assert!(text_of(&recipe).contains("shape = \"torus\""));
    assert!(screen.query_by_label("Add a part on the ground").is_none());

    // F2 puts the typing in the Name field; keys then belong to the
    // typing, so "h" is a letter, not Hide.
    screen.key_press(Key::F2);
    screen.run_steps(3);
    let name = screen.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name");
    assert!(name.is_focused(), "F2 should put the typing in Name");
    screen.key_press_modifiers(Modifiers::COMMAND, Key::A);
    screen.run_steps(2);
    screen
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .type_text("hoop");
    screen.run_steps(2);
    assert!(!screen.state().studio().unwrap().is_hidden("torus"));
    screen.key_press(Key::Enter);
    screen.run_steps(3);
    assert_eq!(selection(&screen), ["hoop"]);
}

#[test]
fn a_right_click_opens_the_menu_for_the_part_under_the_pointer() {
    let (recipe, mut screen) = yard("screens_menu");
    // Nothing is selected: the right-click selects the crate, and opens
    // its menu.
    let crate_at = on_screen(&screen, "crate");
    right_click_at(&mut screen, crate_at);
    assert_eq!(selection(&screen), ["crate"]);
    for line in [
        "Mirror copy",
        "Rename",
        "Join to",
        "Add a part here",
        "Frame",
    ] {
        // (A line with a key has the key in its label too.)
        assert!(
            screen.query_all_by_label_contains(line).count() >= 1,
            "the menu should have {line}"
        );
    }
    // Duplicate, from the menu (the later of the two on the screen: the
    // other is the panel's button).
    screen
        .query_all_by_label_contains("Duplicate")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    assert_eq!(selection(&screen), ["crate_2"]);
    // The menu has closed.
    assert_eq!(screen.query_all_by_label("Mirror copy").count(), 1);
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(text_of(&recipe), YARD);

    // "Add a part here" adds at the point that was clicked: on the
    // crate's surface, joined to a socket made there.
    let crate_at = on_screen(&screen, "crate");
    right_click_at(&mut screen, crate_at);
    screen.get_by_label("Add a part here").click();
    screen.run_steps(3);
    screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "Cone")
        .click();
    screen.run_steps(3);
    assert_eq!(selection(&screen), ["cone"]);
    let now = text_of(&recipe);
    assert!(now.contains("spot = { on = "), "{now}");
    assert!(now.contains("to = \"crate.spot\""), "{now}");
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(text_of(&recipe), YARD);

    // "Join to": the crate onto the top of the post.
    let crate_at = on_screen(&screen, "crate");
    right_click_at(&mut screen, crate_at);
    screen.get_by_label("Join to").click();
    screen.run_steps(3);
    // (The later "post": the earlier is its line in the parts list.)
    screen.query_all_by_label("post").last().unwrap().click();
    screen.run_steps(3);
    screen.query_all_by_label("left").last().unwrap().click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(now.contains("to = \"post.left\""), "{now}");
    screen.get_by_label("Undo").click();
    screen.run_steps(2);

    // In the list too: a right-click on a line selects that part and
    // opens the same menu. Hide, from it.
    let row = screen
        .get_by_role_and_label(egui::accesskit::Role::Button, "post")
        .rect()
        .center();
    right_click_at(&mut screen, row);
    assert_eq!(selection(&screen), ["post"]);
    screen
        .query_all_by_label_contains("Hide")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    assert!(screen.state().studio().unwrap().is_hidden("post"));
    assert_eq!(text_of(&recipe), YARD);

    // With several selected, a right-click on one of them keeps them
    // all, and the menu is for them all.
    screen.state_mut().studio_mut().unwrap().show_all();
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.state_mut().studio_mut().unwrap().toggle("post");
    screen.run_steps(2);
    let crate_at = on_screen(&screen, "crate");
    right_click_at(&mut screen, crate_at);
    assert_eq!(selection(&screen), ["crate", "post"]);
    screen
        .query_all_by_label_contains("Remove")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    // The cap hangs from the post and wasn't selected: asked first.
    screen.get_by_label("Another part hangs from these. Remove it too?");
    screen.get_by_label("Remove them all").click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(
        !now.contains("crate") && !now.contains("post") && !now.contains("cap"),
        "{now}"
    );
    assert_eq!(screen.state().studio().unwrap().status(), "Removed 2 parts");
}

#[test]
fn looking_only_nothing_is_copied_or_removed_but_parts_can_be_hidden() {
    use egui::{Key, Modifiers};
    let recipe = copy_of("robot.toml", "screens_look_keys");
    let before = text_of(&recipe);
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("head"));
    screen.run_steps(2);
    screen.key_press_modifiers(Modifiers::COMMAND, Key::D);
    screen.key_press(Key::Delete);
    screen.run_steps(3);
    assert_eq!(text_of(&recipe), before);
    assert!(screen.state().studio().unwrap().part("head").is_some());
    screen.key_press(Key::H);
    screen.run_steps(3);
    assert!(screen.state().studio().unwrap().is_hidden("head"));
    assert_eq!(text_of(&recipe), before);
}

// ---- views, wireframe, the shadow, and the menus ----------------------------------

#[test]
fn the_view_buttons_and_keys_turn_the_camera_and_change_the_look() {
    use egui::Key;
    let (recipe, mut screen) = yard("screens_views");
    let looking = |screen: &Harness<'static, StudioApp>| {
        let orbit = screen.state().studio().unwrap().orbit;
        (orbit.round, orbit.above)
    };
    assert_eq!(looking(&screen), (30.0, 20.0));
    // The buttons.
    for (button, wanted) in [
        ("Front", (0.0, 0.0)),
        ("Side", (90.0, 0.0)),
        ("Top", (0.0, 90.0)),
        ("3/4", (30.0, 20.0)),
    ] {
        screen.get_by_label(button).click();
        screen.run_steps(2);
        assert_eq!(looking(&screen), wanted, "{button}");
    }
    screen.get_by_label_contains("Looking from the front right");
    // The keys 1 to 4.
    for (key, wanted) in [
        (Key::Num1, (0.0, 0.0)),
        (Key::Num2, (90.0, 0.0)),
        (Key::Num3, (0.0, 90.0)),
        (Key::Num4, (30.0, 20.0)),
    ] {
        screen.key_press(key);
        screen.run_steps(2);
        assert_eq!(looking(&screen), wanted, "{key:?}");
    }

    // Wireframe: the button, and Z.
    assert!(!screen.state().studio().unwrap().wire);
    screen.get_by_label("Wire").click();
    screen.run_steps(2);
    assert!(screen.state().studio().unwrap().wire);
    screen.key_press(Key::Z);
    screen.run_steps(2);
    assert!(!screen.state().studio().unwrap().wire);
    // The shadow is on to begin with, and the grid.
    assert!(screen.state().studio().unwrap().shadow);
    assert!(screen.state().studio().unwrap().grid);
    screen.get_by_label("Shadow").click();
    screen.run_steps(2);
    assert!(!screen.state().studio().unwrap().shadow);
    // None of it is the recipe's business.
    assert_eq!(text_of(&recipe), YARD);
    assert_eq!(screen.state().studio().unwrap().can_undo(), None);
}

#[test]
fn the_menus_do_what_the_buttons_and_keys_do() {
    let (recipe, mut screen) = yard("screens_menus");
    // A line of a menu: open the menu, click the line.
    let choose = |screen: &mut Harness<'static, StudioApp>, menu: &str, line: &str| {
        screen.get_by_label(menu).click();
        screen.run_steps(3);
        screen
            .query_all_by_label_contains(line)
            .last()
            .unwrap_or_else(|| panic!("the {menu} menu has no line {line}"))
            .click();
        screen.run_steps(3);
    };

    // Edit: select all, duplicate, undo, redo.
    choose(&mut screen, "Edit", "Select all");
    assert_eq!(selection(&screen), ["floor", "crate", "post", "cap"]);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(2);
    choose(&mut screen, "Edit", "Duplicate");
    assert_eq!(selection(&screen), ["crate_2"]);
    choose(&mut screen, "Edit", "Undo");
    assert_eq!(text_of(&recipe), YARD);
    choose(&mut screen, "Edit", "Redo");
    assert!(text_of(&recipe).contains("crate_2"));
    // (Undo let go of the copy, so it is selected again by hand.)
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate_2"));
    screen.run_steps(2);
    choose(&mut screen, "Edit", "Remove");
    assert_eq!(text_of(&recipe), YARD);

    // View: the top, wireframe, and hiding.
    choose(&mut screen, "View", "Top");
    assert_eq!(screen.state().studio().unwrap().orbit.above, 90.0);
    choose(&mut screen, "View", "Wireframe: off");
    assert!(screen.state().studio().unwrap().wire);
    choose(&mut screen, "View", "Wireframe: on");
    assert!(!screen.state().studio().unwrap().wire);
    choose(&mut screen, "View", "Grid: on");
    assert!(!screen.state().studio().unwrap().grid);
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("post"));
    screen.run_steps(2);
    choose(&mut screen, "View", "Hide");
    assert!(screen.state().studio().unwrap().is_hidden("post"));
    choose(&mut screen, "View", "Show everything hidden");
    assert!(!screen.state().studio().unwrap().is_hidden("post"));

    // File: export writes the model next to the recipe.
    choose(&mut screen, "File", "Export");
    assert!(recipe.with_extension("glb").is_file());

    // Help: the keys, in a window; Escape closes it.
    choose(&mut screen, "Help", "The keys");
    screen.get_by_label("Ctrl+D");
    screen.get_by_label("Duplicate what is selected");
    screen.get_by_label("Keys don't fire while you are typing in a field.");
    screen.key_press(egui::Key::Escape);
    screen.run_steps(3);
    assert!(
        screen
            .query_by_label("Duplicate what is selected")
            .is_none()
    );
    choose(&mut screen, "Help", "About");
    screen.get_by_label_contains("Duck Mold Studio 0.");
    screen.key_press(egui::Key::Escape);
    screen.run_steps(3);

    // File: back to the start page. The model stays as it is on disk.
    let before = text_of(&recipe);
    choose(&mut screen, "File", "Back to the start page");
    assert!(screen.state().studio().is_none());
    screen.get_by_label("Open a model");
    assert_eq!(text_of(&recipe), before);
}

// ---- colour: the palette panel and the Paint tool -------------------------------

#[test]
fn the_palette_panel_picks_a_colour_and_the_paint_tool_paints_with_it() {
    use duck_mold_view::studio::handles::Tool;
    use egui::accesskit::Role;
    let (recipe, mut screen) = yard("screens_paint");
    screen.get_by_label("Palette");
    screen.get_by_label("3 colours");

    // A click on a colour's name takes the Paint tool, with that colour.
    screen.get_by_role_and_label(Role::Button, "blue").click();
    screen.run_steps(3);
    {
        let studio = screen.state().studio().unwrap();
        assert_eq!(studio.tool, Tool::Paint);
        assert_eq!(studio.paint_colour().as_deref(), Some("blue"));
    }
    // A click on the crate now paints the face under the pointer, and
    // selects nothing.
    let crate_at = on_screen(&screen, "crate");
    click_at(&mut screen, crate_at);
    let now = text_of(&recipe);
    assert!(now.contains("colors = { "), "{now}");
    assert!(now.contains(" = \"blue\" }"), "{now}");
    assert!(selection(&screen).is_empty());
    // With Shift, the whole part.
    shift_click_at(&mut screen, crate_at);
    let now = text_of(&recipe);
    assert!(!now.contains("colors = "), "{now}");
    assert!(
        now.contains("size = [0.5, 0.5, 0.5]\ncolor = \"blue\""),
        "{now}"
    );
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    screen.get_by_label("Undo").click();
    screen.run_steps(2);
    assert_eq!(text_of(&recipe), YARD);

    // The key P takes the tool too; G gives the arrows back, and a
    // click selects again.
    screen.key_press(egui::Key::G);
    screen.run_steps(2);
    assert_eq!(screen.state().studio().unwrap().tool, Tool::Move);
    screen.key_press(egui::Key::P);
    screen.run_steps(2);
    assert_eq!(screen.state().studio().unwrap().tool, Tool::Paint);
    screen.key_press(egui::Key::G);
    screen.run_steps(2);
    click_at(&mut screen, crate_at);
    assert_eq!(selection(&screen), ["crate"]);
    assert_eq!(text_of(&recipe), YARD);
    screen.key_press(egui::Key::Escape);
    screen.run_steps(2);

    // A new colour.
    screen
        .get_by_role_and_label(Role::Button, "New colour")
        .click();
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::TextInput, "Called")
        .focus();
    screen.run_steps(2);
    screen
        .get_by_role_and_label(Role::TextInput, "Called")
        .type_text("gold");
    screen.run_steps(2);
    screen.get_by_label("Add to palette").click();
    screen.run_steps(3);
    assert!(text_of(&recipe).contains("gold = \"#FFCC2E\""));
    screen.get_by_label("4 colours");

    // Removing: a colour nothing uses just goes; one in use asks what to
    // paint with instead. (The lines are in the palette's order.)
    let lines: Vec<_> = screen.query_all_by_label("remove").collect();
    assert_eq!(lines.len(), 4);
    lines[3].click();
    screen.run_steps(3);
    assert_eq!(text_of(&recipe), YARD);
    screen.query_all_by_label("remove").nth(1).unwrap().click();
    screen.run_steps(3);
    screen.get_by_label("\"red\" is in use. What it paints gets another colour:");
    assert_eq!(text_of(&recipe), YARD);
    screen.get_by_label("Repaint and remove").click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(!now.contains("red"), "{now}");
    // The first other colour was offered: stone.
    assert_eq!(now.matches("color = \"stone\"").count(), 3);
    screen.get_by_label("2 colours");
    assert!(!screen.state().studio().unwrap().has_mistakes());

    // Looking only: the colours are listed, and nothing can be clicked.
    let mut looking = harness(StudioApp::with_file(&recipe, true), false);
    looking.run_steps(2);
    looking.get_by_label("Palette");
    assert!(looking.query_by_label("New colour").is_none());
    assert_eq!(looking.query_all_by_label("remove").count(), 0);
}

// ---- sockets: the list in the panel, and tidying -----------------------------------

#[test]
fn the_panel_lists_a_parts_sockets_to_rename_and_remove() {
    use duck_mold_core::edit::Edit;
    let (recipe, mut screen) = yard("screens_sockets");
    // The crate gets two sockets, and a ball on one of them.
    for name in ["spot", "spot_2"] {
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .apply(&Edit::AddSocket {
                part: "crate".into(),
                name: name.into(),
                on: "top".into(),
                shift: [0.0; 3],
            })
            .unwrap();
    }
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .apply(&Edit::Join {
            part: "cap".into(),
            to: "crate.spot".into(),
            attach: None,
        })
        .unwrap();
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .select(Some("crate"));
    screen.run_steps(3);

    // The panel lists them, with how much each is used.
    screen.get_by_label("Sockets");
    screen.get_by_label("1 use");
    screen.get_by_label("not used");
    // The one in use can't be removed; the other can.
    assert_eq!(screen.query_all_by_label("remove socket").count(), 2);
    screen
        .query_all_by_label("remove socket")
        .next()
        .unwrap()
        .click();
    screen.run_steps(3);
    assert!(text_of(&recipe).contains("spot = { on = \"top\" }"));
    screen
        .query_all_by_label("remove socket")
        .nth(1)
        .unwrap()
        .click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(!now.contains("spot_2"), "{now}");
    assert!(now.contains("spot = { on = \"top\" }"), "{now}");
    screen.get_by_label_contains("Removed the socket \"crate.spot_2\"");

    // Renamed by typing: the ball's join follows.
    // (The field itself is the text input among what holds "spot".)
    let field = |screen: &Harness<'static, StudioApp>| {
        screen
            .query_all_by_value("spot")
            .find(|node| node.accesskit_node().role() == egui::accesskit::Role::TextInput)
            .expect("a field with the socket's name")
            .rect()
            .center()
    };
    let at = field(&screen);
    click_at(&mut screen, at);
    screen.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    screen.run_steps(2);
    screen
        .query_all_by_value("spot")
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::TextInput)
        .unwrap()
        .type_text("lid");
    screen.run_steps(2);
    screen.key_press(egui::Key::Enter);
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(now.contains("lid = { on = \"top\" }"), "{now}");
    assert!(now.contains("to = \"crate.lid\""), "{now}");
    assert!(!screen.state().studio().unwrap().has_mistakes());

    // Edit > Tidy sockets takes the Studio's unused ones.
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .apply(&Edit::AddSocket {
            part: "post".into(),
            name: "spot".into(),
            on: "front".into(),
            shift: [0.0; 3],
        })
        .unwrap();
    screen.run_steps(2);
    screen.get_by_label("Edit").click();
    screen.run_steps(3);
    screen
        .query_all_by_label_contains("Tidy sockets")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    let now = text_of(&recipe);
    assert!(!now.contains("spot"), "{now}");
    assert!(now.contains("lid = "), "{now}");
    screen.get_by_label_contains("Removed 1 socket nothing uses: post.spot");
}

// ---- the start page's examples ----------------------------------------------------

#[test]
fn an_example_on_the_start_page_opens_a_copy_of_your_own() {
    use duck_mold_core::tools::EXAMPLES;
    let models = folder("screens_gallery").join("Documents/Duck Mold");
    let mut screen = harness(StudioApp::new().keeping_models_in(&models), false);
    screen.step();
    // The page: the name and what it is, the version, the two ways to
    // begin, every example by name, and how it works.
    screen.get_by_label("Duck Mold Studio");
    screen.get_by_label("Build 3D models from parts, for Duck Forge games.");
    screen.get_by_label_contains("version 0.");
    screen.get_by_label("New model");
    screen.get_by_label("Open a model");
    screen.get_by_label("Examples: click one to open a copy of your own");
    for example in EXAMPLES {
        screen.get_by_label(example.name);
    }
    screen.get_by_label("How it works");
    screen.get_by_label_contains("Parts stand free, or join at dots");
    // Nothing is made until an example is asked for.
    assert!(!models.exists());

    // A click opens a copy, in the folder for the person's own models.
    screen.get_by_label("robot").click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "robot.toml - Duck Mold Studio");
    let copy = models.join("robot.toml");
    assert_eq!(
        std::fs::read_to_string(&copy)
            .unwrap()
            .replace("\r\n", "\n"),
        std::fs::read_to_string(examples().join("robot.toml"))
            .unwrap()
            .replace("\r\n", "\n")
    );
    // It is a model of one's own: it can be changed.
    {
        let studio = screen.state_mut().studio_mut().unwrap();
        assert!(!studio.look_only());
        assert!(!studio.has_mistakes());
        studio.select(Some("head"));
        studio.remove_selected(true).unwrap();
    }
    assert!(
        !std::fs::read_to_string(&copy)
            .unwrap()
            .contains("name = \"head\"")
    );

    // Back on the start page, the same example again is another copy;
    // the first, with its change, is left alone.
    screen.get_by_label("File").click();
    screen.run_steps(3);
    screen
        .query_all_by_label_contains("Back to the start page")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    screen.get_by_label("robot").click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "robot_2.toml - Duck Mold Studio");
    assert!(
        std::fs::read_to_string(models.join("robot_2.toml"))
            .unwrap()
            .contains("name = \"head\"")
    );
    assert!(
        !std::fs::read_to_string(&copy)
            .unwrap()
            .contains("name = \"head\"")
    );

    // From the File menu too. The desk brings the files it uses.
    screen.get_by_label("File").click();
    screen.run_steps(3);
    screen.get_by_label("Open an example").click();
    screen.run_steps(3);
    screen
        .query_all_by_label_contains("desk")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "desk.toml - Duck Mold Studio");
    assert!(models.join("parts/lamp.gltf").is_file());
    assert!(models.join("parts/crate.glb").is_file());
    assert!(!screen.state().studio().unwrap().has_mistakes());
    assert!(screen.state().studio().unwrap().part("lamp").is_some());
}

#[test]
fn models_opened_lately_have_pictures_and_are_in_the_file_menu() {
    use duck_mold_studio::recent::Recent;
    let folder = folder("screens_recent_tiles");
    let list = folder.join("settings/recent.txt");
    for name in ["robot.toml", "house.toml"] {
        std::fs::copy(examples().join(name), folder.join(name)).unwrap();
    }
    // One that no longer builds.
    std::fs::write(
        folder.join("broken.toml"),
        "version = 0\n[[parts]]\nshape = \"blob\"\n",
    )
    .unwrap();
    let mut screen = harness(
        StudioApp::new()
            .remembering(Recent::kept_in(list.clone()))
            .keeping_models_in(folder.join("models")),
        false,
    );
    for name in ["robot.toml", "broken.toml", "house.toml"] {
        screen.state_mut().open(folder.join(name), false);
    }
    screen.run_steps(2);

    // The File menu lists them, newest first, and opens one.
    screen.get_by_label("File").click();
    screen.run_steps(3);
    screen.get_by_label("Opened lately").click();
    screen.run_steps(3);
    screen
        .query_all_by_label_contains("robot.toml")
        .last()
        .unwrap()
        .click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "robot.toml - Duck Mold Studio");

    // On the start page each is a tile; the broken one too, and it
    // still opens, to show what is wrong with it.
    let mut screen = harness(
        StudioApp::new()
            .remembering(Recent::kept_in(list))
            .keeping_models_in(folder.join("models")),
        false,
    );
    screen.run_steps(2);
    screen.get_by_label("Opened lately");
    for name in ["robot.toml", "house.toml", "broken.toml"] {
        screen.get_by_label(name);
    }
    screen.get_by_label("broken.toml").click();
    screen.run_steps(3);
    assert_eq!(screen.state().title(), "broken.toml - Duck Mold Studio");
    assert!(screen.state().studio().unwrap().has_mistakes());
}
