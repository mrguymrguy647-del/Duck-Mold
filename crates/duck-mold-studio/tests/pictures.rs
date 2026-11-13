//! Pictures of the Studio's whole screens, compared with the stored ones
//! in `tests/screens/`.
//!
//! Linux only. egui draws its text and panels through the graphics
//! driver, and drivers don't agree to the last pixel, so one system is
//! the reference: Linux, whose software driver (`mesa-vulkan-drivers`)
//! runs anywhere, CI included. A tiny difference is allowed.
//!
//! All the pictures are drawn by one test, one after the other, in a
//! test program of their own. Drawn at the same time by several tests,
//! the software driver crashed on CI.
//!
//! To store new pictures after a change that is meant to be seen:
//!
//! ```sh
//! UPDATE_SNAPSHOTS=1 cargo test -p duck-mold-studio --test pictures
//! ```

#![cfg(target_os = "linux")]

use std::path::Path;

use duck_mold_studio::StudioApp;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::{Harness, SnapshotOptions};

mod common;
use common::*;

/// Compares the screen with the stored picture `name`. Gives what is
/// wrong, if something is, so that every picture is tried before the
/// test fails.
fn check(screen: &mut Harness<'static, StudioApp>, name: &str) -> Option<String> {
    let stored = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/screens");
    screen
        .try_snapshot_options(
            name,
            &SnapshotOptions::new()
                .threshold(2.0)
                .max_failed_pixels(200)
                .output_path(stored),
        )
        .err()
        .map(|error| format!("{name}: {error}"))
}

/// The robot example open, as it is in the repository: nothing is
/// written to it, and its path isn't on the screen.
fn robot() -> Harness<'static, StudioApp> {
    let mut screen = harness(
        StudioApp::with_file(examples().join("robot.toml"), false),
        true,
    );
    screen.run_steps(2);
    screen
}

#[test]
fn the_screens_look_as_they_did() {
    let mut wrong: Vec<String> = Vec::new();

    // The start page.
    {
        let mut screen = harness(StudioApp::new(), true);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "start"));
    }

    // A recipe open, with nothing selected.
    {
        let mut screen = robot();
        screen.run_steps(1);
        wrong.extend(check(&mut screen, "robot"));
    }

    // A part selected: marked in the list and in the picture, with its
    // sockets and its properties.
    {
        let mut screen = robot();
        screen.get_by_label("left_arm").click();
        screen.run_steps(2);
        screen.remove_cursor();
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "selected"));
    }

    // A part from a definition: the panel says what else would change.
    {
        let mut screen = robot();
        screen.get_by_label("left_eye").click();
        screen.run_steps(2);
        screen.remove_cursor();
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "definition"));
    }

    // A click on a socket: the list of what to add there.
    {
        let mut screen = robot();
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(2);
        let on_right = socket_on_screen(&screen, "head", "right");
        click_at(&mut screen, on_right);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "add-list"));
    }

    // The same with a library: the desk example uses the examples'
    // `parts` folder.
    {
        let mut screen = harness(
            StudioApp::with_file(examples().join("desk.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen.state_mut().studio_mut().unwrap().select(Some("top"));
        screen.run_steps(2);
        let at = socket_on_screen(&screen, "top", "top");
        click_at(&mut screen, at);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "library"));
    }

    // The handles: a free part with the Move tool's arrows, then the
    // Rotate tool's rings.
    {
        let recipe = folder("pictures_handles").join("boxes.toml");
        std::fs::write(
            &recipe,
            "version = 0\nname = \"boxes\"\n\n[palette]\nstone = \"#C0C0C0\"\nred = \"#D0453A\"\n\n[[parts]]\nname = \"floor\"\nshape = \"box\"\nsize = [2, 0.2, 2]\ncolor = \"stone\"\n\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.5, 0.5, 0.5]\ncolor = \"red\"\nmove = [0.5, 0.35, 0]\n",
        )
        .unwrap();
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("crate"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "handles-move"));
        screen.key_press(egui::Key::R);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "handles-rotate"));
        screen.key_press(egui::Key::T);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "handles-size"));

        // The Add menu, open; then the part it adds, standing on the
        // ground beside the floor with the arrows on it.
        screen.get_by_label("Add").click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "add-menu"));
        screen
            .get_by_role_and_label(egui::accesskit::Role::Button, "Cone")
            .click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "add-free"));
    }

    // The newer shapes: an arch with its sizes in the panel and the Size
    // tool's knobs on it; a tower's tapered, bevelled shaft; and a ring
    // with a knob for how big it is and one for how thick.
    {
        let mut screen = harness(
            StudioApp::with_file(examples().join("gate.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("arch"));
        screen.key_press(egui::Key::T);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "shape-arch"));
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("left_tower/shaft"));
        screen.key_press(egui::Key::G);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "shape-bevel-taper"));
    }
    {
        let mut screen = harness(
            StudioApp::with_file(examples().join("doughnut.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("dough"));
        screen.key_press(egui::Key::T);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "shape-torus"));
    }

    // Several parts selected, with the arrows that move them together
    // and the panel for them all; the right-click menu; and a hidden
    // part, struck through in the list and counted in the bar.
    {
        let recipe = folder("pictures_several").join("yard.toml");
        std::fs::write(
            &recipe,
            "version = 0\nname = \"yard\"\n\n[palette]\nstone = \"#C0C0C0\"\nred = \"#D0453A\"\nblue = \"#3A6FD0\"\n\n[[parts]]\nname = \"floor\"\nshape = \"box\"\nsize = [4, 0.2, 4]\ncolor = \"stone\"\n\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.5, 0.5, 0.5]\ncolor = \"red\"\nmove = [0.8, 0.35, 0.6]\n\n[[parts]]\nname = \"post\"\nshape = \"cylinder\"\nradius = 0.1\nheight = 1\ncolor = \"blue\"\nmove = [-0.8, 0.6, 0.5]\n\n[[parts]]\nname = \"cap\"\nshape = \"sphere\"\nradius = 0.15\ncolor = \"red\"\nattach = \"bottom\"\nto = \"post.top\"\n",
        )
        .unwrap();
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("crate"));
        screen.state_mut().studio_mut().unwrap().toggle("post");
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "several"));
        let at = on_screen(&screen, "crate");
        right_click_at(&mut screen, at);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "menu"));
        screen.key_press(egui::Key::Escape);
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("post"));
        screen.key_press(egui::Key::H);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "hidden"));
    }

    // The look: wireframe, the view from the top, and the View menu.
    {
        let mut screen = harness(
            StudioApp::with_file(examples().join("temple.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen.key_press(egui::Key::Z);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "wireframe"));
        screen.key_press(egui::Key::Z);
        screen.key_press(egui::Key::Num3);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "view-top"));
        screen.key_press(egui::Key::Num4);
        screen.run_steps(2);
        screen.get_by_label("View").click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "menu-view"));
    }

    // A recipe saved with a mistake: the problems, and the last good
    // model still up.
    {
        let recipe = copy_of("house.toml", "pictures_mistake");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        let good = std::fs::read_to_string(&recipe).unwrap();
        std::fs::write(&recipe, good.replace("shape = \"box\"", "shape = \"blob\"")).unwrap();
        assert!(screen.state_mut().studio_mut().unwrap().check_files());
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "mistake"));
    }

    assert!(
        wrong.is_empty(),
        "{} of the screens changed:\n{}\nIf the change is meant, store the new pictures with: UPDATE_SNAPSHOTS=1 cargo test -p duck-mold-studio --test pictures",
        wrong.len(),
        wrong.join("\n")
    );
}
