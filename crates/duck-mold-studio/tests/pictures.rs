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
