//! Phase 7b, step 6: where the model's `.glb` goes, and the Studio
//! shown inside another program's window.
//!
//! Duck Forge's Studio shows Duck Mold's Studio in a tab. It is this
//! same code with one switch, "hosted": no start page and nothing about
//! other files, Export reads "Send to game", and the model is exported
//! after every change so that a running game shows it. The Studio on
//! its own gets the same lines in its Model section.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::{Changes, Edit};
use duck_mold_studio::StudioApp;
use duck_mold_studio::recent::Recent;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

mod common;
use common::*;

/// A game's folder with the robot's recipe in `art`.
fn game(test: &str) -> (PathBuf, PathBuf) {
    let game = folder(test);
    std::fs::create_dir_all(game.join("art")).unwrap();
    let recipe = game.join("art/robot.toml");
    std::fs::copy(examples().join("robot.toml"), &recipe).unwrap();
    with_robots_images(&recipe);
    (game, recipe)
}

fn recipe_line(recipe: &Path, starts: &str) -> Option<String> {
    std::fs::read_to_string(recipe)
        .unwrap()
        .lines()
        .find(|line| line.starts_with(starts))
        .map(str::to_owned)
}

/// A change to the model: the head's colour.
fn paint_the_head(screen: &mut Harness<'static, StudioApp>, colour: &str) {
    screen
        .state_mut()
        .studio_mut()
        .unwrap()
        .apply(&Edit::Set {
            part: "head".to_owned(),
            changes: Changes {
                color: Some(colour.to_owned()),
                ..Changes::default()
            },
        })
        .unwrap();
    screen.run_steps(2);
}

#[test]
fn hosted_has_no_start_page_and_nothing_about_other_files() {
    let (game, recipe) = game("hosted_menus");
    let mut screen = harness(StudioApp::hosted(&recipe, false).for_game(&game), false);
    screen.run_steps(2);
    assert!(screen.state().is_hosted());
    // Export reads "Send to game" (to a screen reader it is "Export"
    // either way).
    screen.get_by_role_and_label(Role::Button, "Export");
    assert_eq!(screen.state().exporting().button(), "Send to game");

    // The File menu has only what is done with this model.
    screen.get_by_label("File").click();
    screen.run_steps(2);
    assert!(screen.query_by_label_contains("New model").is_none());
    assert!(screen.query_by_label_contains("Open...").is_none());
    assert!(screen.query_by_label_contains("Opened lately").is_none());
    assert!(screen.query_by_label_contains("start page").is_none());
    // (The button at the top right, and the menu's line.)
    assert_eq!(
        screen.query_all_by_label_contains("Render picture").count(),
        2
    );
    screen.key_press(egui::Key::Escape);
    screen.run_steps(2);
    // The size of everything is the other program's to set.
    screen.get_by_label("View").click();
    screen.run_steps(2);
    assert!(screen.query_by_label_contains("Interface size").is_none());
    screen.key_press(egui::Key::Escape);
    screen.run_steps(2);

    // Ctrl+N and Ctrl+O do nothing here: the robot stays.
    screen.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
    screen.run_steps(2);
    assert!(screen.state().studio().is_some());

    // A file that isn't there is a sentence, not a start page.
    let mut screen = harness(StudioApp::hosted(game.join("art/gone.toml"), false), false);
    screen.run_steps(2);
    screen.get_by_label_contains("There is no file");
    assert!(screen.query_by_label_contains("New model").is_none());

    // The Studio on its own is as it was.
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    assert!(!screen.state().is_hosted());
    assert_eq!(screen.state().exporting().button(), "Export .glb");
    assert!(!screen.state().exporting().always);
}

#[test]
fn one_click_sends_the_model_into_the_games_assets() {
    let (game, recipe) = game("hosted_into_game");
    let mut screen = harness(StudioApp::hosted(&recipe, false).for_game(&game), false);
    screen.run_steps(2);
    // With nothing selected, the Model section says where it goes: a
    // model made for a game goes to the game's assets, line or no line.
    screen.get_by_label("Goes to");
    screen.get_by_label("the game's assets (robot.glb)");
    assert!(
        !game.join("art/robot.glb").exists() && !game.join("assets/robot.glb").exists(),
        "opening a model exports nothing"
    );
    assert_eq!(
        screen.state().studio().unwrap().export_path(),
        game.join("assets/robot.glb")
    );

    // One click writes the line, so that Duck Mold on its own exports
    // there too: the same edit as `set-export`.
    screen
        .get_by_role_and_label(Role::Button, "Write it in the recipe")
        .click();
    screen.run_steps(3);
    assert_eq!(
        recipe_line(&recipe, "export").as_deref(),
        Some("export = \"../assets/robot.glb\"")
    );
    screen.get_by_label("../assets/robot.glb");
    assert!(
        screen
            .query_by_label_contains("Write it in the recipe")
            .is_none(),
        "it is there already"
    );
    // That was a change, and the model is exported after every change
    // here: it is in the game's assets already, and the program around
    // the Studio is told which file.
    let sent = game.join("assets/robot.glb");
    assert!(sent.is_file());
    assert_eq!(
        screen.state_mut().take_exported(),
        std::slice::from_ref(&sent)
    );
    assert!(screen.state_mut().take_exported().is_empty(), "told once");

    // Another change, another export: the file is new.
    let before = std::fs::read(&sent).unwrap();
    paint_the_head(&mut screen, "red");
    assert_ne!(std::fs::read(&sent).unwrap(), before);
    assert_eq!(
        screen.state_mut().take_exported(),
        std::slice::from_ref(&sent)
    );
    // Undo is a change too.
    screen.state_mut().studio_mut().unwrap().undo().unwrap();
    screen.run_steps(2);
    assert_eq!(std::fs::read(&sent).unwrap(), before);
    assert_eq!(screen.state_mut().take_exported().len(), 1);

    // A recipe with a mistake isn't sent: the game keeps the last
    // model that worked.
    let text = std::fs::read_to_string(&recipe).unwrap();
    std::fs::write(
        &recipe,
        text.replace("color = \"steel\"", "color = \"stel\""),
    )
    .unwrap();
    screen.state_mut().studio_mut().unwrap().check_files();
    screen.run_steps(2);
    assert!(screen.state().studio().unwrap().has_mistakes());
    assert_eq!(std::fs::read(&sent).unwrap(), before);
    assert!(screen.state_mut().take_exported().is_empty());

    // "Send to game" by hand writes there as well.
    std::fs::write(&recipe, text).unwrap();
    screen.state_mut().studio_mut().unwrap().check_files();
    screen.run_steps(2);
    screen.state_mut().take_exported();
    screen.get_by_role_and_label(Role::Button, "Export").click();
    screen.run_steps(3);
    assert_eq!(screen.state_mut().take_exported(), [sent]);
    let said = screen.state().studio().unwrap().status().to_owned();
    assert!(
        said.starts_with("Wrote ../assets/robot.glb (") && said.ends_with(" KB)"),
        "{said}"
    );
}

#[test]
fn without_a_line_a_model_for_a_game_still_goes_to_its_assets() {
    let (game, recipe) = game("hosted_no_line");
    let mut screen = harness(StudioApp::hosted(&recipe, false).for_game(&game), false);
    screen.run_steps(2);
    paint_the_head(&mut screen, "red");
    let sent = game.join("assets/robot.glb");
    assert!(sent.is_file());
    assert!(!game.join("art/robot.glb").exists());
    assert_eq!(
        screen.state_mut().take_exported(),
        std::slice::from_ref(&sent)
    );
    assert_eq!(
        recipe_line(&recipe, "export"),
        None,
        "the recipe isn't touched"
    );
    // "Send to game" by hand, the same.
    std::fs::remove_file(&sent).unwrap();
    screen.get_by_role_and_label(Role::Button, "Export").click();
    screen.run_steps(3);
    assert!(sent.is_file());
}

#[test]
fn export_after_every_change_can_be_turned_off_and_is_remembered() {
    let (game, recipe) = game("hosted_always");
    let settings = game.join("settings/recent.txt");
    let open = |hosted: bool| {
        let app = if hosted {
            StudioApp::hosted(&recipe, false).for_game(&game)
        } else {
            StudioApp::with_file(&recipe, false)
        };
        let mut screen = harness(app.remembering(Recent::kept_in(settings.clone())), false);
        screen.run_steps(2);
        screen
    };
    // On inside another program's window; off for the Studio on its own.
    let mut screen = open(true);
    assert!(screen.state().exporting().always);
    screen.get_by_label("Export after every change").click();
    screen.run_steps(2);
    assert!(!screen.state().exporting().always);
    paint_the_head(&mut screen, "red");
    assert!(
        !game.join("art/robot.glb").exists() && !game.join("assets/robot.glb").exists(),
        "nothing is exported now"
    );
    assert!(screen.state_mut().take_exported().is_empty());
    drop(screen);

    // Remembered, each on its own.
    let screen = open(true);
    assert!(!screen.state().exporting().always);
    drop(screen);
    let mut screen = open(false);
    assert!(!screen.state().exporting().always);
    screen.get_by_label("Export after every change").click();
    screen.run_steps(2);
    paint_the_head(&mut screen, "steel");
    assert!(game.join("art/robot.glb").is_file(), "beside the recipe");
    drop(screen);
    assert!(open(false).state().exporting().always);
    assert!(!open(true).state().exporting().always);
}

#[test]
fn the_studio_on_its_own_shows_and_clears_the_export_line() {
    let (game, recipe) = game("hosted_standalone");
    let text = std::fs::read_to_string(&recipe).unwrap();
    std::fs::write(
        &recipe,
        text.replacen(
            "version = 0\n",
            "version = 0\nexport = \"../assets/robot.glb\"\n",
            1,
        ),
    )
    .unwrap();
    let mut screen = harness(StudioApp::with_file(&recipe, false), false);
    screen.run_steps(2);
    screen.get_by_label("../assets/robot.glb");
    // No game is known, so there is nothing to offer but a choice.
    assert!(screen.query_by_label_contains("Into the game").is_none());
    screen.get_by_role_and_label(Role::Button, "Choose...");

    // Export writes where the line says.
    screen.get_by_role_and_label(Role::Button, "Export").click();
    screen.run_steps(3);
    assert!(game.join("assets/robot.glb").is_file());
    assert!(!game.join("art/robot.glb").exists());

    // "Beside the recipe" takes the line away.
    screen
        .get_by_role_and_label(Role::Button, "Beside the recipe")
        .click();
    screen.run_steps(3);
    assert_eq!(recipe_line(&recipe, "export"), None);
    screen.get_by_label("beside the recipe (robot.glb)");

    // Looking only, the lines say where it goes and offer no change.
    let mut screen = harness(StudioApp::with_file(&recipe, true), false);
    screen.run_steps(2);
    screen.get_by_label("Goes to");
    assert!(screen.query_by_label("Export after every change").is_none());
    assert!(screen.query_by_label("Choose...").is_none());
}

/// The robot's two images (its face and its label), put beside a copy
/// of its recipe: the recipe names them, so it doesn't build without.
#[allow(dead_code)]
fn with_robots_images(recipe: &std::path::Path) {
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/textures");
    let to = recipe.parent().unwrap().join("textures");
    std::fs::create_dir_all(&to).unwrap();
    for image in ["robot_face.png", "robot_label.png"] {
        std::fs::copy(from.join(image), to.join(image)).unwrap();
    }
}
