//! Phase 7's polish audit: every screen, mode, panel, menu, dialog and
//! error of the Studio, with every example, drawn at 1280 x 720 and at
//! 2560 x 1440, and saved as pictures to look at. Not a check: it is
//! run by hand.
//!
//!     DUCK_MOLD_AUDIT=<folder> cargo test -p duck-mold-studio --test audit -- --ignored --nocapture
//!
//! The pictures of Phase 7's audit are in `docs/audit/` (made with this,
//! then cut down to 128 colours). Speed is measured apart, by
//! `crates/duck-mold-view/tests/speed.rs`.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use duck_mold_studio::StudioApp;
use duck_mold_studio::recent::Recent;
use eframe::egui::{self, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

type Screen = Harness<'static, StudioApp>;

/// The app on a pretend screen of `size`, at 1 pixel to a point.
fn screen_of(app: StudioApp, size: (f32, f32)) -> Screen {
    let mut screen = Harness::builder()
        .with_size(egui::vec2(size.0, size.1))
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
}

fn save(screen: &mut Screen, path: &Path) {
    let image = screen.render().expect("drawn");
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(image.as_raw())
        .unwrap();
}

fn studio_of(screen: &mut Screen) -> &mut duck_mold_view::studio::Studio {
    screen.state_mut().studio_mut().unwrap()
}

fn select(screen: &mut Screen, part: &str) {
    studio_of(screen).select(Some(part));
    screen.run_steps(3);
}

fn menu(screen: &mut Screen, name: &str) {
    screen.get_by_label(name).click();
    screen.run_steps(3);
}

fn opened(example: &str) -> impl Fn() -> StudioApp + use<> {
    let path = examples().join(example);
    move || StudioApp::with_file(path.clone(), false)
}

fn copied(example: &str, test: &str) -> impl Fn() -> StudioApp + use<> {
    let example = example.to_owned();
    let test = test.to_owned();
    move || StudioApp::with_file(copy_of(&example, &test), false)
}

/// A screen: its name, how the app starts, and what is done to it.
struct Shot {
    name: String,
    start: Box<dyn Fn() -> StudioApp>,
    act: Box<dyn Fn(&mut Screen)>,
}

fn shot(
    name: impl Into<String>,
    start: impl Fn() -> StudioApp + 'static,
    act: impl Fn(&mut Screen) + 'static,
) -> Shot {
    Shot {
        name: name.into(),
        start: Box::new(start),
        act: Box::new(act),
    }
}

fn shots() -> Vec<Shot> {
    let mut all = Vec::new();
    // ---- the start page
    all.push(shot(
        "01-start-first-time",
        || StudioApp::new().keeping_models_in("Documents/Duck Mold"),
        |_| {},
    ));
    all.push(shot(
        "02-start-recent-with-missing-and-broken",
        || {
            let folder = folder("audit_recent");
            std::fs::copy(examples().join("robot.toml"), folder.join("my_robot.toml")).unwrap();
            std::fs::copy(examples().join("gate.toml"), folder.join("castle.toml")).unwrap();
            std::fs::write(
                folder.join("mistake.toml"),
                "version = 0\n[[parts]]\nshape = \"blob\"\n",
            )
            .unwrap();
            let mut recent = Recent::kept_in(folder.join("recent.txt"));
            for name in ["gone", "mistake", "my_robot", "castle"] {
                recent.opened(&folder.join(format!("{name}.toml")));
            }
            StudioApp::new()
                .remembering(Recent::kept_in(folder.join("recent.txt")))
                .keeping_models_in("Documents/Duck Mold")
        },
        |_| {},
    ));
    all.push(shot(
        "03-start-file-not-there",
        || StudioApp::with_file("no/such/model.toml", false),
        |_| {},
    ));
    // ---- every example, as it opens
    let mut names: Vec<String> = std::fs::read_dir(examples())
        .unwrap()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            name.ends_with(".toml").then_some(name)
        })
        .collect();
    names.sort();
    for name in names {
        all.push(shot(
            format!("10-example-{}", name.trim_end_matches(".toml")),
            opened(&name),
            |_| {},
        ));
    }
    // ---- Part mode
    all.push(shot("20-part-selected", opened("robot.toml"), |s| {
        select(s, "left_arm")
    }));
    all.push(shot("21-part-from-definition", opened("robot.toml"), |s| {
        select(s, "left_eye")
    }));
    all.push(shot(
        "22-part-inside-definition",
        opened("robot.toml"),
        |s| select(s, "left_arm/hand"),
    ));
    all.push(shot("23-socket-add-list", opened("robot.toml"), |s| {
        select(s, "head");
        let at = socket_on_screen(s, "head", "right");
        click_at(s, at);
        s.run_steps(2);
    }));
    all.push(shot(
        "24-socket-add-list-with-library",
        opened("desk.toml"),
        |s| {
            select(s, "top");
            let at = socket_on_screen(s, "top", "top");
            click_at(s, at);
            s.run_steps(2);
        },
    ));
    for (key, name) in [
        (Key::G, "move"),
        (Key::R, "rotate"),
        (Key::S, "scale"),
        (Key::T, "size"),
    ] {
        all.push(shot(
            format!("25-tool-{name}"),
            copied("crate.toml", "audit_tools"),
            move |s| {
                let first = s.state().studio().unwrap().model().unwrap().parts[0]
                    .path
                    .clone();
                select(s, &first);
                s.key_press(key);
                s.run_steps(3);
            },
        ));
    }
    all.push(shot("26-add-menu", opened("robot.toml"), |s| {
        menu(s, "Add");
    }));
    all.push(shot("27-several-selected", opened("snowman.toml"), |s| {
        let parts: Vec<String> = s
            .state()
            .studio()
            .unwrap()
            .model()
            .unwrap()
            .parts
            .iter()
            .take(3)
            .map(|part| part.path.clone())
            .collect();
        select(s, &parts[0]);
        for part in &parts[1..] {
            studio_of(s).toggle(part);
        }
        s.run_steps(3);
    }));
    all.push(shot("28-right-click-menu", opened("robot.toml"), |s| {
        let at = on_screen(s, "body");
        right_click_at(s, at);
        s.run_steps(2);
    }));
    all.push(shot("29-hidden-part", opened("robot.toml"), |s| {
        select(s, "head");
        s.key_press(Key::H);
        s.run_steps(3);
    }));
    all.push(shot("30-rename", opened("robot.toml"), |s| {
        select(s, "head");
        s.key_press(Key::F2);
        s.run_steps(3);
    }));
    all.push(shot(
        "31-remove-asks",
        copied("robot.toml", "audit_remove"),
        |s| {
            select(s, "head");
            s.key_press(Key::Delete);
            s.run_steps(3);
        },
    ));
    all.push(shot(
        "32-look-only",
        || StudioApp::with_file(examples().join("robot.toml"), true),
        |s| select(s, "head"),
    ));
    all.push(shot(
        "33-glb-opened",
        || {
            StudioApp::with_file(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/robot.glb"),
                false,
            )
        },
        |_| {},
    ));
    // ---- menus and windows
    for name in ["File", "Edit", "View", "Help"] {
        all.push(shot(
            format!("40-menu-{}", name.to_lowercase()),
            opened("robot.toml"),
            move |s| menu(s, name),
        ));
    }
    all.push(shot("41-open-example-submenu", opened("robot.toml"), |s| {
        menu(s, "File");
        s.get_by_label("Open an example").hover();
        s.run_steps(3);
    }));
    all.push(shot("42-keys-window", opened("robot.toml"), |s| {
        menu(s, "Help");
        s.get_by_label_contains("The keys").click();
        s.run_steps(3);
    }));
    all.push(shot("43-about-window", opened("robot.toml"), |s| {
        menu(s, "Help");
        s.get_by_label_contains("About Duck Mold Studio").click();
        s.run_steps(3);
    }));
    for step in 1..=5 {
        all.push(shot(
            format!("44-tips-{step}"),
            || {
                let folder = folder("audit_tour");
                StudioApp::new()
                    .remembering(Recent::kept_in(folder.join("recent.txt")))
                    .keeping_models_in("Documents/Duck Mold")
            },
            move |s| {
                s.state_mut().open(examples().join("robot.toml"), false);
                s.run_steps(3);
                for _ in 1..step {
                    if let Some(next) = s.query_by_label("Next") {
                        next.click();
                        s.run_steps(3);
                    }
                }
            },
        ));
    }
    all.push(shot(
        "45-export-said",
        copied("robot.toml", "audit_export"),
        |s| {
            menu(s, "File");
            s.query_all_by_label_contains("Export")
                .last()
                .unwrap()
                .click();
            s.run_steps(3);
        },
    ));
    all.push(shot(
        "46-render-said",
        copied("robot.toml", "audit_render"),
        |s| {
            menu(s, "File");
            s.query_all_by_label_contains("Render picture")
                .last()
                .unwrap()
                .click();
            s.run_steps(3);
        },
    ));
    // ---- views
    all.push(shot("50-wireframe", opened("temple.toml"), |s| {
        s.key_press(Key::Z);
        s.run_steps(3);
    }));
    all.push(shot("51-view-top", opened("house.toml"), |s| {
        s.key_press(Key::Num3);
        s.run_steps(3);
    }));
    all.push(shot("52-hits-on", opened("robot.toml"), |s| {
        s.get_by_label("Hits").click();
        s.run_steps(3);
    }));
    // ---- Paint and the palette
    all.push(shot("55-paint-tool", opened("house.toml"), |s| {
        s.key_press(Key::P);
        s.run_steps(3);
    }));
    all.push(shot("56-new-colour-form", opened("house.toml"), |s| {
        s.get_by_label("New colour").click();
        s.run_steps(3);
    }));
    // ---- Face, Edge, Corner modes
    all.push(shot("60-face-mode", opened("window_wall.toml"), |s| {
        let first = s.state().studio().unwrap().model().unwrap().parts[0]
            .path
            .clone();
        select(s, &first);
        s.key_press(Key::Tab);
        s.run_steps(3);
    }));
    all.push(shot("61-face-selected", opened("chair.toml"), |s| {
        select(s, "chair");
        s.key_press(Key::Tab);
        s.run_steps(3);
        let at = face_on_screen(s, "top_front");
        click_at(s, at);
        s.run_steps(3);
    }));
    all.push(shot("62-edge-mode", opened("rock.toml"), |s| {
        let first = s.state().studio().unwrap().model().unwrap().parts[0]
            .path
            .clone();
        select(s, &first);
        s.key_press(Key::E);
        s.run_steps(3);
    }));
    all.push(shot("63-corner-mode", opened("rock.toml"), |s| {
        let first = s.state().studio().unwrap().model().unwrap().parts[0]
            .path
            .clone();
        select(s, &first);
        s.key_press(Key::C);
        s.run_steps(3);
    }));
    all.push(shot(
        "64-face-mode-on-a-sphere",
        opened("snowman.toml"),
        |s| {
            let first = s.state().studio().unwrap().model().unwrap().parts[0]
                .path
                .clone();
            select(s, &first);
            s.key_press(Key::Tab);
            s.run_steps(3);
        },
    ));
    // ---- Pose mode and the timeline
    for example in ["robot", "door", "cart", "gnome", "dog"] {
        let file = format!("{example}.toml");
        all.push(shot(
            format!("70-pose-mode-{example}"),
            opened(&file),
            |s| {
                s.key_press(Key::O);
                s.run_steps(3);
            },
        ));
    }
    all.push(shot("71-pose-ring", opened("robot.toml"), |s| {
        s.key_press(Key::O);
        s.run_steps(2);
        studio_of(s).choose_pose("wave");
        select(s, "left_arm");
    }));
    all.push(shot("72-timeline-walk", opened("robot.toml"), |s| {
        s.key_press(Key::O);
        s.run_steps(2);
        studio_of(s).choose_animation(Some("walk"));
        studio_of(s).set_time(0.3);
        s.run_steps(3);
    }));
    all.push(shot("73-key-menu", opened("robot.toml"), |s| {
        s.key_press(Key::O);
        s.run_steps(2);
        studio_of(s).choose_animation(Some("walk"));
        s.run_steps(3);
        let at = s.state().timeline().key_on_screen(0.4).unwrap();
        right_click_at(s, at);
        s.run_steps(2);
    }));
    all.push(shot("74-playing", opened("robot.toml"), |s| {
        studio_of(s).choose_animation(Some("wave"));
        studio_of(s).play();
        studio_of(s).tick(0.5);
        s.run_steps(3);
    }));
    all.push(shot("75-animations-library", opened("gnome.toml"), |s| {
        s.get_by_label("Animations").click();
        s.run_steps(3);
    }));
    all.push(shot("76-no-animation-yet", opened("table.toml"), |s| {
        s.key_press(Key::O);
        s.run_steps(3);
    }));
    // ---- mistakes
    all.push(shot(
        "80-recipe-with-mistake",
        || {
            let folder = folder("audit_mistake");
            let path = folder.join("house.toml");
            std::fs::copy(examples().join("house.toml"), &path).unwrap();
            StudioApp::with_file(path, false)
        },
        |s| {
            let path = folder_path("audit_mistake").join("house.toml");
            let good = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                &path,
                good.replacen("shape = \"box\"", "shape = \"blob\"", 1),
            )
            .unwrap();
            s.state_mut().studio_mut().unwrap().check_files();
            s.run_steps(3);
        },
    ));
    all.push(shot(
        "81-broken-from-start",
        || {
            let folder = folder("audit_broken");
            let path = folder.join("broken.toml");
            std::fs::write(&path, "version = 0\nname = \"x\"\n[[parts]]\nshape = 4\n").unwrap();
            StudioApp::with_file(path, false)
        },
        |_| {},
    ));
    all.push(shot(
        "82-not-a-toml",
        || {
            let folder = folder("audit_text");
            let path = folder.join("notes.toml");
            std::fs::write(&path, "this isn't a recipe [[[").unwrap();
            StudioApp::with_file(path, false)
        },
        |_| {},
    ));
    all
}

fn folder_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
#[ignore = "run by hand for the polish audit"]
fn every_screen_at_two_sizes() {
    let out = PathBuf::from(
        std::env::var("DUCK_MOLD_AUDIT")
            .unwrap_or_else(|_| folder_path("audit_pictures").to_string_lossy().into_owned()),
    );
    let only = std::env::var("DUCK_MOLD_AUDIT_ONLY").ok();
    for (size, label) in [
        ((1280.0, 720.0), "1280x720"),
        ((2560.0, 1440.0), "2560x1440"),
    ] {
        let dir = out.join(label);
        std::fs::create_dir_all(&dir).unwrap();
        for shot in shots() {
            if only
                .as_deref()
                .is_some_and(|only| !shot.name.contains(only))
            {
                continue;
            }
            let mut screen = screen_of((shot.start)(), size);
            (shot.act)(&mut screen);
            screen.run_steps(2);
            save(&mut screen, &dir.join(format!("{}.png", shot.name)));
            eprintln!("{label} {}", shot.name);
        }
    }
}
