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

/// Rolls the wheel over the properties panel, down to its end.
fn scroll_panel(screen: &mut Harness<'static, StudioApp>) {
    let over = egui::pos2(1140.0, 300.0);
    screen.hover_at(over);
    screen.step();
    screen.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -2000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    screen.run_steps(4);
}

/// A wall with a framed window, shaped from one box.
const WALL: &str = r##"version = 0
name = "wall"

[palette]
plaster = "#E8E0D0"
wood = "#8B5A2B"
sky = "#9FD3F0"

[[parts]]
name = "wall"
shape = "box"
size = [3, 2.4, 0.2]
color = "plaster"
changes = [
  { inset = "front", by = 0.6, name = "window" },
  { inset = "window", by = 0.06, name = "glass" },
  { pull = "glass", by = -0.08 },
]
colors = { window = "wood", glass = "sky", glass_side = "wood" }
"##;

#[test]
fn the_screens_look_as_they_did() {
    let mut wrong: Vec<String> = Vec::new();

    // The start page: first with nothing opened yet, then with models
    // opened lately, one of which no longer builds. (The folder for
    // copies of the examples is named as on any machine.)
    {
        let mut screen = harness(
            StudioApp::new().keeping_models_in("Documents/Duck Mold"),
            true,
        );
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "start"));
    }
    {
        use duck_mold_studio::recent::Recent;
        let folder = folder("pictures_start");
        for name in ["castle", "my_robot", "mistake"] {
            let from = match name {
                "castle" => "gate.toml",
                _ => "robot.toml",
            };
            std::fs::copy(examples().join(from), folder.join(format!("{name}.toml"))).unwrap();
        }
        std::fs::write(
            folder.join("mistake.toml"),
            "version = 0\n[[parts]]\nshape = \"blob\"\n",
        )
        .unwrap();
        std::fs::write(folder.join("gone.toml"), "").unwrap();
        let list = folder.join("recent.txt");
        let mut recent = Recent::kept_in(list.clone());
        for name in ["gone", "mistake", "my_robot", "castle"] {
            recent.opened(&folder.join(format!("{name}.toml")));
        }
        // One opened lately and deleted since.
        std::fs::remove_file(folder.join("gone.toml")).unwrap();
        let mut screen = harness(
            StudioApp::new()
                .remembering(Recent::kept_in(list))
                .keeping_models_in("Documents/Duck Mold"),
            true,
        );
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "start-recent"));
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

    // The first run's tips: the first, the one that points at an arrow,
    // the one that points at a dot, and the one that points at Add.
    {
        use duck_mold_studio::recent::Recent;
        let folder = folder("pictures_tour");
        let mut screen = harness(
            StudioApp::new()
                .remembering(Recent::kept_in(folder.join("recent.txt")))
                .keeping_models_in("Documents/Duck Mold"),
            true,
        );
        screen
            .state_mut()
            .open(examples().join("robot.toml"), false);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "tour-1"));
        for name in ["tour-2", "tour-3", "tour-4", "tour-5"] {
            screen.get_by_label("Next").click();
            screen.run_steps(3);
            if name != "tour-2" {
                wrong.extend(check(&mut screen, name));
            }
        }
    }

    // Colour: the palette with a colour picked and the Paint tool in
    // hand, over a face that lights up; a face painted; and the question
    // asked when a colour in use is removed.
    {
        let mut screen = harness(
            StudioApp::with_file(copy_of("house.toml", "pictures_paint"), false),
            true,
        );
        screen.run_steps(2);
        screen
            .get_by_role_and_label(egui::accesskit::Role::Button, "glass")
            .click();
        screen.run_steps(3);
        let at = on_screen(&screen, "door");
        screen.hover_at(at);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "paint"));
        click_at(&mut screen, at);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "painted"));
        screen.query_all_by_label("remove").next().unwrap().click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "palette-remove"));
    }

    // Sockets: a part that declares some, listed in the panel with how
    // much each is used.
    {
        let mut screen = harness(
            StudioApp::with_file(examples().join("duck.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "sockets"));
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
        // Shown once it has stayed the same for two checks.
        screen.state_mut().studio_mut().unwrap().check_files();
        assert!(screen.state_mut().studio_mut().unwrap().check_files());
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "mistake"));
    }

    // A recipe deleted while it is open: the bar over the picture.
    {
        let recipe = copy_of("house.toml", "pictures_gone");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        std::fs::remove_file(&recipe).unwrap();
        assert!(screen.state_mut().studio_mut().unwrap().check_files());
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "missing"));
    }

    // The start page after a crash.
    {
        use duck_mold_studio::recent::Recent;
        let folder = folder("pictures_crashed");
        let settings = folder.join("settings");
        std::fs::create_dir_all(&settings).unwrap();
        let recipe = folder.join("house.toml");
        std::fs::copy(examples().join("house.toml"), &recipe).unwrap();
        std::fs::write(
            settings.join("crash-2026-11-18-14-02-33.txt"),
            format!("Open:     {}\n", recipe.display()),
        )
        .unwrap();
        let mut screen = harness(
            StudioApp::new()
                .remembering(Recent::kept_in(settings.join("recent.txt")))
                .keeping_models_in("Documents/Duck Mold"),
            true,
        );
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "start-crashed"));
    }

    // Shaping: a wall with a framed window, in Face mode with the glass
    // selected and the Changes list in the panel; then a pull of the
    // glass held too far in, its label red with why.
    {
        let recipe = folder("pictures_shaping").join("wall.toml");
        std::fs::write(&recipe, WALL).unwrap();
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.select(Some("wall"));
            studio.face_mode().unwrap();
            studio.look_from(duck_mold_view::studio::viewport::Viewpoint::ThreeQuarter);
        }
        screen.run_steps(2);
        let glass = face_on_screen(&screen, "glass");
        click_at(&mut screen, glass);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "shape-face"));

        use duck_mold_view::studio::handles::Grip;
        let from = screen
            .state()
            .studio()
            .unwrap()
            .grip_at(Grip::Pull)
            .unwrap();
        let start = point_on_screen(&screen, from);
        let end = point_on_screen(&screen, from + duck_mold_core::math::vec3(0.0, 0.0, -0.3));
        screen.hover_at(start);
        screen.step();
        screen.drag_at(start);
        screen.step();
        for part in 1..=4 {
            screen.hover_at(start + (end - start) * (part as f32 / 4.0));
            screen.step();
        }
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "shape-refused"));
        screen.event(egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        screen.run_steps(2);
        assert_eq!(std::fs::read_to_string(&recipe).unwrap(), WALL);

        // Edge mode: the top's front edge selected, its arrows on it.
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .edge_mode()
            .unwrap();
        screen.run_steps(2);
        let edge = screen
            .state()
            .studio()
            .unwrap()
            .edge_point("top+window_rim_top")
            .unwrap();
        let edge = picture_on_screen(&screen, edge);
        click_at(&mut screen, edge);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "shape-edge"));
    }

    // Corner mode on the rock: its corners as dots, one picked, with the
    // arrows that move it and its name in the panel.
    {
        let recipe = copy_of("rock.toml", "pictures_corner");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.select(Some("rock"));
            studio.corner_mode().unwrap();
            studio.look_from(duck_mold_view::studio::viewport::Viewpoint::ThreeQuarter);
        }
        screen.run_steps(2);
        let corner = screen
            .state()
            .studio()
            .unwrap()
            .corner_point("top_right_front+front_right_top+right_top_front")
            .unwrap();
        let corner = picture_on_screen(&screen, corner);
        click_at(&mut screen, corner);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "shape-corner"));
    }

    // The Cut tool on the chair, in Face mode: the ring where the cut
    // would go, across the face under the pointer, and its label.
    {
        let recipe = copy_of("chair.toml", "pictures_cut");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.select(Some("chair"));
            studio.face_mode().unwrap();
            studio.look_from(duck_mold_view::studio::viewport::Viewpoint::ThreeQuarter);
        }
        screen.run_steps(2);
        screen.key_press(egui::Key::K);
        screen.run_steps(2);
        // Along the top of the seat's front, between its middle and the
        // slice towards the right.
        let studio = screen.state().studio().unwrap();
        let (middle, right) = (
            studio.edge_point("top_front+front").unwrap(),
            studio.edge_point("top_right_front+front_right").unwrap(),
        );
        let between = ((middle.0 + right.0) / 2.0, (middle.1 + right.1) / 2.0);
        let point = picture_on_screen(&screen, between);
        screen.hover_at(point);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "shape-cut"));
    }

    // Using a shaped part again: the part's menu, the window that makes
    // a definition of it, and the library once it is saved there.
    {
        let recipe = copy_of("chair.toml", "pictures_reuse");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        // (Where the seat is, found in Face mode, then back to parts.)
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.select(Some("chair"));
            studio.face_mode().unwrap();
        }
        screen.run_steps(2);
        let seat = face_on_screen(&screen, "top");
        screen.state_mut().studio_mut().unwrap().part_mode();
        screen.run_steps(2);
        right_click_at(&mut screen, seat);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "reuse-menu"));
        screen
            .query_all_by_label_contains("Make a definition")
            .next()
            .unwrap()
            .click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "reuse-define"));
        screen.get_by_label("Cancel").click();
        screen.run_steps(2);
        right_click_at(&mut screen, seat);
        screen.run_steps(2);
        screen
            .query_all_by_label_contains("Save to the library")
            .next()
            .unwrap()
            .click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "reuse-library"));
    }

    // Pose mode: the robot waving, its head selected with its rings,
    // and the panel with the poses, the head's turn and its limits.
    {
        let recipe = copy_of("robot.toml", "pictures_pose");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.pose_mode().unwrap();
            studio.choose_pose("wave");
            studio.select(Some("head"));
        }
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "pose-mode"));
        // A fixed part: no rings, and the line under the picture says
        // why.
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("left_arm/forearm"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "pose-fixed"));

        // The timeline: the wave chosen, half way between two keys, with
        // the ghosts of both.
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.select(Some("left_arm"));
            studio.choose_animation(Some("wave"));
            studio.set_time(0.5);
        }
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "timeline"));
        // The walk: a footstep flag under its first and third keys; then
        // the third key's menu, with its event.
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.choose_animation(Some("walk"));
            studio.set_time(0.3);
        }
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "timeline-events"));
        let third = screen.state().timeline().key_on_screen(0.4).unwrap();
        right_click_at(&mut screen, third);
        screen.run_steps(2);
        wrong.extend(check(&mut screen, "timeline-event-menu"));
        screen.key_press(egui::Key::Escape);
        screen.run_steps(2);
        // Zoomed in three times on the walk's middle, scrolled there.
        screen.get_by_label("+").click();
        screen.run_steps(2);
        screen.get_by_label("+").click();
        screen.run_steps(2);
        screen.get_by_label("+").click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "timeline-zoomed"));
        screen.get_by_label("Fit").click();
        screen.run_steps(2);
    }

    // Play in Part mode: the robot waving, part way through.
    {
        let mut screen = robot();
        {
            let studio = screen.state_mut().studio_mut().unwrap();
            studio.choose_animation(Some("wave"));
            studio.play();
            // (While it plays, each frame moves it on by the same
            // step, so the picture is the same every time.)
            studio.tick(0.5);
        }
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "play"));
    }

    // The animation library: the starters, with the door's and the
    // wheels' greyed out for the robot.
    {
        let mut screen = robot();
        screen.get_by_label("Animations").click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "animations"));
    }

    // Hit shapes: the Hits switch on, the head selected with its Hit
    // shape section; nothing selected, with the model's line; and a
    // part with numbers of its own.
    {
        let mut screen = robot();
        screen.get_by_label("Hits").click();
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(3);
        scroll_panel(&mut screen);
        wrong.extend(check(&mut screen, "hits"));
        screen.state_mut().studio_mut().unwrap().select(None);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "hits-model"));
    }
    {
        let recipe = copy_of("robot.toml", "pictures_hit_own");
        let text = std::fs::read_to_string(&recipe).unwrap().replace(
            "collider = \"sphere\"   # a ball round the head, rather than a box",
            "collider = { shape = \"capsule\", radius = 0.17, length = 0.46, along = \"x\" }",
        );
        std::fs::write(&recipe, text).unwrap();
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        screen.state_mut().studio_mut().unwrap().hits = true;
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(3);
        scroll_panel(&mut screen);
        wrong.extend(check(&mut screen, "hits-own"));
    }

    // A far-away version: View > Far version > Far 2, nothing selected,
    // with the model's line and the count in the status bar.
    {
        let mut screen = robot();
        screen.state_mut().studio_mut().unwrap().far = 2;
        screen.state_mut().studio_mut().unwrap().select(None);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "far"));
    }

    // Every icon, at three sizes, on the dark look and on the light.
    {
        use duck_mold_studio::icons::{self, Icon};
        let mut sheet = Harness::builder()
            .with_size(egui::vec2(560.0, 280.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui(|ui| {
                for (side, theme) in [
                    (16.0, egui::Theme::Dark),
                    (24.0, egui::Theme::Dark),
                    (40.0, egui::Theme::Light),
                ] {
                    let back = if theme == egui::Theme::Dark {
                        egui::Color32::from_gray(30)
                    } else {
                        egui::Color32::from_gray(235)
                    };
                    let ink = if theme == egui::Theme::Dark {
                        egui::Color32::from_gray(225)
                    } else {
                        egui::Color32::from_gray(30)
                    };
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(
                            ui.available_width(),
                            (side + 4.0) * (Icon::ALL.len() as f32 * (side + 4.0) / 540.0).ceil(),
                        ),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 0.0, back);
                    let across = ((rect.width() - 8.0) / (side + 4.0)).floor() as usize;
                    for (at, icon) in Icon::ALL.into_iter().enumerate() {
                        let place = egui::Rect::from_min_size(
                            rect.min
                                + egui::vec2(
                                    4.0 + (at % across) as f32 * (side + 4.0),
                                    2.0 + (at / across) as f32 * (side + 4.0),
                                ),
                            egui::Vec2::splat(side),
                        );
                        icons::paint(ui.painter(), place, icon, ink);
                    }
                }
            });
        sheet.run();
        let stored = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/screens");
        wrong.extend(
            sheet
                .try_snapshot_options(
                    "icons",
                    &SnapshotOptions::new()
                        .threshold(2.0)
                        .max_failed_pixels(200)
                        .output_path(stored),
                )
                .err()
                .map(|error| format!("icons: {error}")),
        );
    }

    // The main screens at the audit's two sizes, 1280 x 720 and 2560 x
    // 1440, and the big one again with the interface at 150%.
    for (width, height, percent) in [
        (1280.0, 720.0, None),
        (2560.0, 1440.0, None),
        (2560.0, 1440.0, Some(150)),
    ] {
        use duck_mold_studio::recent::Recent;
        let tag = match percent {
            Some(percent) => format!("{width}x{height}-{percent}"),
            None => format!("{width}x{height}"),
        };
        let sized = |app: StudioApp| {
            // The window's size is in points: at 150%, a 2560-pixel
            // screen is 2560 / 1.5 points wide.
            let scale = percent.map_or(1.0, |percent| percent as f32 / 100.0);
            let mut screen = Harness::builder()
                .with_size(egui::vec2(width / scale, height / scale))
                .with_pixels_per_point(1.0)
                .with_render_cursor(false)
                .with_context_setup(|ctx| {
                    ctx.set_theme(egui::ThemePreference::Dark);
                    duck_mold_studio::theme::apply_brand(ctx);
                })
                .wgpu()
                .build_eframe(|_| app);
            screen.run_steps(3);
            screen
        };
        // Where the interface size is kept, for this run only.
        let settings = folder(&format!("pictures_sizes_{tag}"));
        let recent = settings.join("recent.txt");
        Recent::kept_in(recent.clone()).keep_interface_size(percent);
        let remembering = || Recent::kept_in(recent.clone());
        let mut screen = sized(
            StudioApp::new()
                .remembering(remembering())
                .keeping_models_in("Documents/Duck Mold"),
        );
        wrong.extend(check(&mut screen, &format!("size-{tag}-start")));
        let mut screen = sized(
            StudioApp::with_file(examples().join("robot.toml"), false).remembering(remembering()),
        );
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, &format!("size-{tag}-selected")));
        let mut screen = sized(
            StudioApp::with_file(examples().join("robot.toml"), false).remembering(remembering()),
        );
        screen.key_press(egui::Key::O);
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .choose_animation(Some("walk"));
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("left_leg"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, &format!("size-{tag}-pose")));
    }

    // Removing a part that others hang from: three clear choices.
    {
        let mut screen = robot();
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("head"));
        screen.run_steps(2);
        screen.key_press(egui::Key::Delete);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "remove-asks"));
    }

    // A recipe that has never built.
    {
        let recipe = folder("pictures_never_built").join("broken.toml");
        std::fs::write(
            &recipe,
            "version = 0\nname = \"x\"\n\n[[parts]]\nshape = \"blob\"\n",
        )
        .unwrap();
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "never-built"));
    }

    // A .glb opened only to look at, with a part selected: plain facts.
    {
        let folder = folder("pictures_glb");
        let recipe = folder.join("robot.toml");
        std::fs::copy(examples().join("robot.toml"), &recipe).unwrap();
        duck_mold_core::tools::export_file(&recipe, None).unwrap();
        let mut screen = harness(StudioApp::with_file(folder.join("robot.glb"), false), true);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "glb-opened"));
        let mut screen = harness(StudioApp::with_file(&recipe, true), true);
        screen.run_steps(2);
        screen
            .state_mut()
            .studio_mut()
            .unwrap()
            .select(Some("left_arm"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "look-only"));
    }

    // Help: the keys (they scroll) and About.
    {
        let mut screen = robot();
        screen.get_by_label("Help").click();
        screen.run_steps(3);
        screen
            .query_all_by_label_contains("The keys")
            .last()
            .unwrap()
            .click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "keys"));
        let mut screen = robot();
        screen.get_by_label("Help").click();
        screen.run_steps(3);
        screen
            .query_all_by_label_contains("About Duck Mold Studio")
            .last()
            .unwrap()
            .click();
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "about"));
    }

    // Export: the note over the picture.
    {
        let recipe = copy_of("robot.toml", "pictures_export_note");
        let mut screen = harness(StudioApp::with_file(&recipe, false), true);
        screen.run_steps(2);
        screen.get_by_label("Export").click();
        screen.run_steps(4);
        wrong.extend(check(&mut screen, "export-note"));
    }

    // Images (Phase 7b): the plank crate, with its images in the
    // picture, in the Model section and in the row under the palette.
    let plank_crate = |test: &str| {
        let folder = folder(test);
        std::fs::create_dir_all(folder.join("textures")).unwrap();
        for file in [
            "plank_crate.toml",
            "textures/planks.png",
            "textures/lid.png",
        ] {
            std::fs::copy(examples().join(file), folder.join(file)).unwrap();
        }
        let mut screen = harness(
            StudioApp::with_file(folder.join("plank_crate.toml"), false),
            true,
        );
        screen.run_steps(2);
        screen
    };
    {
        let mut screen = plank_crate("pictures_images");
        wrong.extend(check(&mut screen, "images"));
    }
    // A part with images selected: each image has its line in the
    // properties.
    {
        let mut screen = plank_crate("pictures_images_part");
        screen.state_mut().studio_mut().unwrap().select(Some("box"));
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "images-part"));
    }
    // The "New image" window.
    {
        use egui::accesskit::Role;
        let mut screen = plank_crate("pictures_images_new");
        screen.state_mut().studio_mut().unwrap().select(Some("box"));
        screen.run_steps(2);
        screen
            .get_by_role_and_label(Role::Button, "New image...")
            .click();
        screen.run_steps(5);
        wrong.extend(check(&mut screen, "images-new"));
    }
    // An image in hand, over a face: the hint says what a click would
    // do, and the image picked has a yellow edge.
    {
        use egui::accesskit::Role;
        let mut screen = plank_crate("pictures_images_paint");
        screen
            .get_by_role_and_label(Role::Button, "lid.png")
            .click();
        screen.run_steps(2);
        screen.hover_at(IN_VIEW);
        screen.run_steps(3);
        wrong.extend(check(&mut screen, "images-paint"));
    }

    assert!(
        wrong.is_empty(),
        "{} of the screens changed:\n{}\nIf the change is meant, store the new pictures with: UPDATE_SNAPSHOTS=1 cargo test -p duck-mold-studio --test pictures",
        wrong.len(),
        wrong.join("\n")
    );
}
