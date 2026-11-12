//! What Duck Mold Studio does, driven with no window: opening a file,
//! the viewport's pictures, noticing saves from outside, and (in the
//! steps that follow) selecting, editing, sockets, undo and the library.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use duck_mold_core::edit::{Changes, Edit, Made, Sizes};
use duck_mold_core::math::{Vec3, vec3};
use duck_mold_view::render::BACKGROUND;
use duck_mold_view::studio::viewport::Picture;
use duck_mold_view::studio::{Clicked, Studio};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// A folder for a test's files, emptied first.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A copy of an example recipe in a folder of its own, to change.
fn copy_of(example: &str, test: &str) -> PathBuf {
    let folder = folder(test);
    let recipe = folder.join(format!("{example}.toml"));
    std::fs::copy(examples().join(format!("{example}.toml")), &recipe).unwrap();
    recipe
}

/// Writes a file as an editor saves it: new text, and a time later than
/// before, so the change shows even where the clock is coarse.
fn save(path: &Path, text: &str) {
    let before = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::now());
    std::fs::write(path, text).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(before + Duration::from_secs(2)).unwrap();
}

/// Compares a picture with the stored one, byte for byte, or stores it
/// when `DUCK_MOLD_ACCEPT` is set. The viewport is drawn by Duck Forge's
/// CPU renderer with Duck Mold's own sine and cosine, so the same view is
/// the same picture on every computer.
fn check_picture(name: &str, picture: &Picture) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"));
    let found = picture.to_png();
    if std::env::var_os("DUCK_MOLD_ACCEPT").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &found).unwrap();
        return;
    }
    let stored = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "{} is missing. Create it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test studio",
            path.display()
        )
    });
    if stored != found {
        let new = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.new.png"));
        std::fs::write(&new, &found).unwrap();
        panic!(
            "the picture {name} changed (the new one is in {}). If the change is meant, accept it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test studio",
            new.display()
        );
    }
}

/// How many pixels aren't the background.
fn drawn(picture: &Picture) -> usize {
    picture
        .pixels
        .iter()
        .filter(|&&pixel| pixel != BACKGROUND.to_u32())
        .count()
}

const CRATE: &str = "version = 0\nname = \"crate\"\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"box\"\nshape = \"box\"\ncolor = \"wood\"\n";

// ---- step 3: the window's contents ---------------------------------------------

#[test]
fn it_opens_a_recipe_and_lists_its_parts() {
    let studio = Studio::open(examples().join("robot.toml"), false);
    assert_eq!(
        studio.headline().as_deref(),
        Some("robot: 0.90 x 1.83 x 0.39 m, 384 triangles")
    );
    assert_eq!(studio.title(), "robot.toml");
    assert!(!studio.look_only());
    assert_eq!(studio.problems(), []);
    assert_eq!(studio.builds(), 1);
    assert_eq!(studio.status(), "");
    // The outliner: the tree `inspect` prints.
    let rows = studio.outline();
    let shown: Vec<String> = rows
        .iter()
        .map(|row| format!("{}{} [{}]", "  ".repeat(row.depth), row.name, row.kind))
        .collect();
    assert_eq!(
        shown[..6],
        [
            "body [box]",
            "panel [box]",
            "head [box]",
            "left_eye [eye (box)]",
            "right_eye [eye (box)]",
            "antenna [cylinder]",
        ]
    );
    let arm = rows.iter().position(|row| row.path == "left_arm").unwrap();
    assert_eq!(rows[arm].kind, "arm (assembly)");
    assert_eq!(rows[arm + 1].path, "left_arm/upper");
    assert_eq!(rows[arm + 1].depth, 1);
    assert_eq!(rows.iter().map(|row| row.triangles).max(), Some(84));
}

#[test]
fn the_viewport_shows_the_model_on_a_grid() {
    let mut studio = Studio::open(examples().join("robot.toml"), false);
    let picture = studio.picture(480, 360);
    assert_eq!((picture.width, picture.height), (480, 360));
    check_picture("studio-robot", &picture);
    // The same view is the same picture.
    assert_eq!(studio.picture(480, 360), picture);

    // Without the grid, less is drawn; the model is still there.
    studio.grid = false;
    let bare = studio.picture(480, 360);
    assert!(drawn(&bare) > 10_000, "{}", drawn(&bare));
    assert!(drawn(&bare) < drawn(&picture));
    check_picture("studio-robot-no-grid", &bare);
}

#[test]
fn the_camera_orbits_zooms_pans_and_frames() {
    let mut studio = Studio::open(examples().join("house.toml"), false);
    let start = studio.picture(320, 240);
    let home = studio.orbit;

    studio.orbit.turn(120.0, 30.0);
    let turned = studio.picture(320, 240);
    assert_ne!(turned, start);
    check_picture("studio-house-turned", &turned);

    studio.orbit.zoom(3.0);
    let close = studio.picture(320, 240);
    assert!(drawn(&close) > drawn(&turned), "closer is bigger");

    studio.orbit.pan(400.0, 0.0, 240);
    let aside = studio.picture(320, 240);
    assert!(drawn(&aside) < drawn(&close), "slid mostly out of view");

    // Frame brings it back into view, without turning.
    studio.frame();
    assert_eq!(studio.orbit.target, home.target);
    assert_eq!(studio.orbit.distance, home.distance);
    assert_eq!(studio.orbit.round, 330.0);
    assert_eq!(studio.picture(320, 240), turned);
}

#[test]
fn a_save_from_outside_is_picked_up_and_the_view_stays() {
    let folder = folder("studio_outside");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.orbit.turn(40.0, 0.0);
    let view = studio.orbit;
    let before = studio.picture(200, 150);
    assert_eq!(studio.watched(), [recipe.as_path()]);

    // Nothing saved: nothing is built again, however long it runs.
    for _ in 0..200 {
        assert!(!studio.tick(1.0 / 60.0));
    }
    assert_eq!(studio.builds(), 1);

    // An agent (or a text editor) saves it in blue.
    save(&recipe, &CRATE.replace("#B07A3F", "#3F6FB0"));
    assert!(!studio.tick(0.1), "not checked yet");
    assert!(studio.tick(0.5));
    assert_eq!(studio.builds(), 2);
    assert_eq!(studio.orbit, view, "the view stays where it was");
    assert_ne!(studio.picture(200, 150), before);
    assert!(studio.text().contains("#3F6FB0"));
}

#[test]
fn a_save_with_mistakes_shows_them_and_keeps_the_last_good_model() {
    let folder = folder("studio_mistake");
    let recipe = folder.join("crate.toml");
    std::fs::write(&recipe, CRATE).unwrap();
    let mut studio = Studio::open(&recipe, false);
    let good = studio.picture(200, 150);

    save(&recipe, &CRATE.replace("\"box\"\ncolor", "\"boxx\"\ncolor"));
    assert!(studio.tick(0.5));
    assert!(studio.has_mistakes());
    let problems = studio.problems();
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(problems[0].is_error);
    assert_eq!(
        problems[0].text,
        "line 7: there is no shape \"boxx\" (did you mean box?)"
    );
    // The crate is still shown.
    assert_eq!(
        studio.headline().as_deref(),
        Some("crate: 1.00 x 1.00 x 1.00 m, 12 triangles")
    );
    assert_eq!(studio.picture(200, 150), good);

    save(&recipe, CRATE);
    assert!(studio.tick(0.5));
    assert!(!studio.has_mistakes());
    assert_eq!(studio.problems(), []);
}

#[test]
fn warnings_are_problems_too_and_name_their_part() {
    let folder = folder("studio_warning");
    let recipe = folder.join("grey.toml");
    std::fs::write(
        &recipe,
        "version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nmove = [0, 3, 0]\n",
    )
    .unwrap();
    let studio = Studio::open(&recipe, false);
    assert!(!studio.has_mistakes());
    let problems = studio.problems();
    let floating = problems
        .iter()
        .find(|problem| problem.text.contains("touches nothing"))
        .expect("b floats");
    assert!(!floating.is_error);
    assert_eq!(floating.part.as_deref(), Some("b"));
    assert!(floating.text.starts_with("line 5: "), "{}", floating.text);
}

#[test]
fn files_the_recipe_uses_are_watched_too() {
    let folder = folder("studio_palette");
    std::fs::write(folder.join("colours.toml"), "wood = \"#B07A3F\"\n").unwrap();
    let recipe = folder.join("crate.toml");
    std::fs::write(
        &recipe,
        "version = 0\npalette = \"colours.toml\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n",
    )
    .unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert_eq!(
        studio.watched(),
        [recipe.as_path(), folder.join("colours.toml").as_path()]
    );
    let before = studio.picture(120, 90);
    save(&folder.join("colours.toml"), "wood = \"#3F6FB0\"\n");
    assert!(studio.tick(0.5));
    assert_ne!(studio.picture(120, 90), before);
}

#[test]
fn export_and_render_write_next_to_the_recipe_and_say_so() {
    let recipe = copy_of("table", "studio_export");
    let mut studio = Studio::open(&recipe, false);
    let said = studio.export().unwrap();
    let glb = recipe.with_extension("glb");
    assert!(
        said.starts_with("Wrote ") && said.contains("table.glb"),
        "{said}"
    );
    assert_eq!(studio.status(), said);
    assert_eq!(&std::fs::read(&glb).unwrap()[..4], b"glTF");

    let said = studio.render_picture().unwrap();
    assert!(said.contains("table.png (513 x 536 pixels)"), "{said}");
    assert_eq!(
        &std::fs::read(recipe.with_extension("png")).unwrap()[..4],
        b"\x89PNG"
    );

    // With a mistake in the file, neither is written, and the status
    // says why.
    let _ = std::fs::remove_file(&glb);
    save(&recipe, "version = 0\n[[parts]]\nshape = \"boxx\"\n");
    studio.tick(0.5);
    let why = studio.export().unwrap_err();
    assert!(why.contains("there is no shape \"boxx\""), "{why}");
    assert!(studio.status().starts_with("Not exported: "));
    assert!(!glb.exists());
}

#[test]
fn a_model_file_is_opened_to_look_at_only() {
    let glb = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/robot.glb");
    let studio = Studio::open(&glb, false);
    assert!(studio.look_only(), "a .glb can't be edited");
    assert_eq!(studio.outline().len(), 23);
    assert!(drawn(&studio.picture(200, 150)) > 2_000);

    let recipe = Studio::open(examples().join("crate.toml"), true);
    assert!(recipe.look_only(), "duck-mold preview only looks");

    let missing = Studio::open(examples().join("nowhere.toml"), false);
    assert!(missing.has_mistakes());
    assert_eq!(missing.headline(), None);
    // Nothing to show but the grid: it still draws.
    assert!(drawn(&missing.picture(64, 48)) > 0);
}

// ---- step 4: selecting -------------------------------------------------------

/// Where the middle of a part is in a picture `size` x `size`.
fn on_screen(studio: &Studio, part: &str, size: usize) -> (f64, f64) {
    let middle = studio.part(part).unwrap().bounds.unwrap().center();
    studio.orbit.on_screen(middle, size, size).unwrap()
}

#[test]
fn a_part_is_selected_by_its_path() {
    let mut studio = Studio::open(examples().join("robot.toml"), false);
    assert_eq!(studio.selected(), None);
    assert!(studio.select(Some("head")));
    assert_eq!(studio.selected(), Some("head"));
    assert!(studio.select(Some("left_arm/hand")));
    assert_eq!(studio.selected(), Some("left_arm/hand"));
    // A part that isn't there leaves the selection alone.
    assert!(!studio.select(Some("tail")));
    assert_eq!(studio.selected(), Some("left_arm/hand"));
    assert!(studio.select(None));
    assert_eq!(studio.selected(), None);
}

#[test]
fn a_click_selects_the_part_under_the_pointer() {
    const SIZE: usize = 400;
    let mut studio = Studio::open(examples().join("robot.toml"), false);
    let picture = (SIZE, SIZE);

    // The red panel is on the body's front, in plain sight.
    let panel = on_screen(&studio, "panel", SIZE);
    assert_eq!(studio.click(picture, panel), Some("panel"));
    // Clicking the same single part again keeps it.
    assert_eq!(studio.click(picture, panel), Some("panel"));

    // A part made of parts is selected whole first, then the part inside.
    let foot = on_screen(&studio, "left_leg/foot", SIZE);
    assert_eq!(studio.click(picture, foot), Some("left_leg"));
    assert_eq!(studio.click(picture, foot), Some("left_leg/foot"));
    assert_eq!(studio.click(picture, foot), Some("left_leg/foot"));
    // From a part inside one leg to the other leg: whole first again.
    let other = on_screen(&studio, "right_leg/foot", SIZE);
    assert_eq!(studio.click(picture, other), Some("right_leg"));

    // A click on nothing selects nothing.
    assert_eq!(studio.click(picture, (3.0, 3.0)), None);
    assert_eq!(studio.selected(), None);
}

#[test]
fn the_selected_part_is_brightened_and_boxed() {
    let mut studio = Studio::open(examples().join("robot.toml"), false);
    let plain = studio.picture(480, 360);
    assert!(studio.select(Some("head")));
    let selected = studio.picture(480, 360);
    assert_ne!(plain, selected);
    check_picture("studio-robot-head-selected", &selected);
    // Only the head and the box round it changed: the feet are the same.
    let changed = plain
        .pixels
        .iter()
        .zip(&selected.pixels)
        .enumerate()
        .filter(|(_, (before, after))| before != after)
        .map(|(at, _)| at / 480)
        .collect::<Vec<_>>();
    let (top, bottom) = (changed[0], changed[changed.len() - 1]);
    assert!(top > 20 && bottom < 200, "rows {top} to {bottom} changed");

    // Frame, with a part selected, brings that part into view.
    let whole = studio.orbit;
    studio.frame();
    assert!(studio.orbit.distance < whole.distance);
    assert_eq!(studio.orbit.round, whole.round);
}

#[test]
fn the_selection_lasts_while_the_part_does() {
    let recipe = copy_of("robot", "studio_selection_lasts");
    let mut studio = Studio::open(&recipe, false);
    assert!(studio.select(Some("antenna_tip")));

    // Saved from outside with something else changed: still selected.
    let text = std::fs::read_to_string(&recipe).unwrap();
    save(
        &recipe,
        &text.replace("name = \"robot\"", "name = \"robot2\""),
    );
    assert!(studio.check_files());
    assert_eq!(studio.selected(), Some("antenna_tip"));

    // Saved with the part renamed: it is gone, so nothing is selected.
    save(
        &recipe,
        &text.replace("name = \"antenna_tip\"", "name = \"bulb\""),
    );
    assert!(studio.check_files());
    assert_eq!(studio.selected(), None);
    assert!(studio.part("bulb").is_some());
}

// ---- step 5: editing ---------------------------------------------------------

/// The lines only the old text has, and the lines only the new one has.
fn changed<'a>(old: &'a str, new: &'a str) -> (Vec<&'a str>, Vec<&'a str>) {
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (
        old[start..old.len() - end].to_vec(),
        new[start..new.len() - end].to_vec(),
    )
}

fn set(part: &str, changes: Changes) -> Edit {
    Edit::Set {
        part: part.into(),
        changes,
    }
}

#[test]
fn an_edit_is_written_to_the_recipe_at_once_and_nothing_else_changes() {
    let recipe = copy_of("robot", "studio_edit");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    let builds = studio.builds();

    let said = studio
        .apply(&set(
            "head",
            Changes {
                color: Some("red".into()),
                ..Changes::default()
            },
        ))
        .unwrap();
    assert_eq!(said, "Set \"head\": colour red");
    assert_eq!(studio.status(), said);
    let after = std::fs::read_to_string(&recipe).unwrap();
    let (gone, new) = changed(&before, &after);
    assert_eq!(new, ["color = \"red\""]);
    assert_eq!(gone.len(), 1, "{gone:?}");
    // What the Studio shows is the file, built again.
    assert_eq!(studio.text(), after);
    assert_eq!(studio.builds(), builds + 1);
    assert!(!studio.has_mistakes());

    // The panel's facts are read from the file too.
    assert!(studio.select(Some("head")));
    let facts = studio.facts().unwrap();
    assert_eq!(facts.color.as_deref(), Some("red"));
    assert_eq!(facts.shape.as_deref(), Some("box"));
    assert!(studio.palette().iter().any(|(name, _)| name == "red"));
}

#[test]
fn an_edit_that_cant_be_made_says_why_and_writes_nothing() {
    let recipe = copy_of("robot", "studio_edit_refused");
    let before = std::fs::read(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    let why = studio
        .apply(&set(
            "head",
            Changes {
                color: Some("pink".into()),
                ..Changes::default()
            },
        ))
        .unwrap_err();
    assert!(why.starts_with("Not changed: "), "{why}");
    assert!(why.contains("pink"), "{why}");
    assert_eq!(studio.status(), why);
    assert_eq!(std::fs::read(&recipe).unwrap(), before);
    assert!(!studio.has_mistakes());

    // Looking only: nothing is ever written.
    let mut looking = Studio::open(&recipe, true);
    let why = looking
        .apply(&Edit::Remove {
            part: "antenna_tip".into(),
            with_attached: false,
        })
        .unwrap_err();
    assert_eq!(why, "This window is only for looking: nothing was changed");
    assert_eq!(std::fs::read(&recipe).unwrap(), before);
}

#[test]
fn the_selection_follows_the_part_through_an_edit() {
    let recipe = copy_of("robot", "studio_edit_selection");
    let mut studio = Studio::open(&recipe, false);

    // Renamed: still selected, under its new name.
    assert!(studio.select(Some("antenna_tip")));
    studio
        .apply(&Edit::Rename {
            part: "antenna_tip".into(),
            new_name: "bulb".into(),
        })
        .unwrap();
    assert_eq!(studio.selected(), Some("bulb"));

    // A part inside a renamed part keeps its place in it.
    assert!(studio.select(Some("left_arm/hand")));
    studio
        .apply(&Edit::Rename {
            part: "left_arm".into(),
            new_name: "port_arm".into(),
        })
        .unwrap();
    assert_eq!(studio.selected(), Some("port_arm/hand"));

    // Added: the new part is selected.
    studio
        .apply(&Edit::Add {
            name: "hat".into(),
            made: Made::Shape("cone".into()),
            to: "head.top".into(),
            attach: None,
            sizes: Sizes {
                radius: Some(0.1),
                height: Some(0.1),
                ..Sizes::default()
            },
            color: Some("red".into()),
            twist: None,
        })
        .unwrap();
    assert_eq!(studio.selected(), Some("hat"));

    // Copied: the copy is selected.
    studio
        .apply(&Edit::Duplicate {
            part: "hat".into(),
            name: None,
            to: None,
            mirror: false,
        })
        .unwrap();
    let copy = studio.selected().unwrap().to_owned();
    assert_ne!(copy, "hat");
    assert!(copy.starts_with("hat"), "{copy}");

    // Removed: nothing is selected.
    studio
        .apply(&Edit::Remove {
            part: copy,
            with_attached: false,
        })
        .unwrap();
    assert_eq!(studio.selected(), None);
}

#[test]
fn a_part_from_a_definition_says_which_parts_change_before_any_edit() {
    let recipe = copy_of("robot", "studio_edit_definition");
    let before = std::fs::read(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);

    // Made with `use`: what it is made of is the definition's.
    assert!(studio.select(Some("left_eye")));
    let uses = studio.facts().unwrap().uses.unwrap();
    assert_eq!(uses.definition, "eye");
    assert_eq!(uses.also_changes, ["right_eye"]);

    // Inside a definition: any edit changes the other arm's hand too.
    assert!(studio.select(Some("left_arm/hand")));
    let inside = studio.facts().unwrap().inside.unwrap();
    assert_eq!(inside.definition, "arm");
    assert_eq!(inside.also_changes, ["right_arm/hand"]);

    // A part written once names nothing.
    assert!(studio.select(Some("head")));
    let facts = studio.facts().unwrap();
    assert_eq!((facts.inside, facts.uses), (None, None));
    // Asking changed nothing.
    assert_eq!(std::fs::read(&recipe).unwrap(), before);

    // And the edit, when made, says so again.
    let said = studio
        .apply(&set(
            "left_arm/hand",
            Changes {
                color: Some("glow".into()),
                ..Changes::default()
            },
        ))
        .unwrap();
    assert_eq!(
        said,
        "Set \"left_arm/hand\": colour glow (also changed: right_arm/hand)"
    );
}

#[test]
fn an_edit_is_made_on_what_the_file_holds_now() {
    // Both at once: another program saves the recipe, and before the
    // Studio has looked, the person changes something in the Studio.
    let recipe = copy_of("robot", "studio_edit_both");
    let mut studio = Studio::open(&recipe, false);
    let text = std::fs::read_to_string(&recipe).unwrap();
    save(
        &recipe,
        &text.replace("name = \"robot\"", "name = \"robot2\""),
    );

    studio
        .apply(&set(
            "head",
            Changes {
                color: Some("red".into()),
                ..Changes::default()
            },
        ))
        .unwrap();
    let after = std::fs::read_to_string(&recipe).unwrap();
    // Both changes are in the file.
    assert!(after.contains("name = \"robot2\""));
    assert_eq!(studio.model().unwrap().name, "robot2");
    studio.select(Some("head"));
    assert_eq!(studio.facts().unwrap().color.as_deref(), Some("red"));
}

// ---- step 6: sockets ---------------------------------------------------------

const VIEW: (usize, usize) = (640, 480);

/// Where a point of the model is in the 640 x 480 picture.
fn seen(studio: &Studio, point: Vec3) -> (f64, f64) {
    studio.orbit.on_screen(point, VIEW.0, VIEW.1).unwrap()
}

/// Where a socket of a part is in the picture.
fn socket_seen(studio: &Studio, part: &str, socket: &str) -> (f64, f64) {
    let at = studio
        .part(part)
        .unwrap()
        .sockets
        .iter()
        .find(|known| known.name == socket)
        .unwrap()
        .at;
    seen(studio, at)
}

/// Where a socket of a part is in the model.
fn socket_at(studio: &Studio, part: &str, socket: &str) -> Vec3 {
    let part = studio.part(part).unwrap();
    part.sockets
        .iter()
        .find(|known| known.name == socket)
        .unwrap()
        .at
}

#[test]
fn the_selected_parts_sockets_show_as_markers() {
    let mut studio = Studio::open(examples().join("robot.toml"), false);
    assert!(studio.markers().is_empty(), "nothing selected, no markers");
    let plain = studio.picture(VIEW.0, VIEW.1);

    assert!(studio.select(Some("head")));
    let markers = studio.markers();
    let names: Vec<&str> = markers
        .iter()
        .map(|marker| marker.socket.as_str())
        .collect();
    // Its six sides and the three it declares; not the one in its middle.
    assert_eq!(
        names,
        [
            "top",
            "bottom",
            "left",
            "right",
            "front",
            "back",
            "left_eye",
            "right_eye",
            "antenna"
        ]
    );
    assert!(markers.iter().all(|marker| marker.part == "head"));

    // The pointer on one makes it bigger.
    let on_top = socket_seen(&studio, "head", "right");
    studio.hover(VIEW, Some(on_top));
    let hot = studio.picture(VIEW.0, VIEW.1);
    check_picture("studio-robot-head-sockets", &hot);
    studio.hover(VIEW, None);
    let cold = studio.picture(VIEW.0, VIEW.1);
    assert_ne!(hot, cold);
    assert_ne!(cold, plain);

    // When only looking there are none.
    let mut looking = Studio::open(examples().join("robot.toml"), true);
    assert!(looking.select(Some("head")));
    assert!(looking.markers().is_empty());
}

#[test]
fn a_click_on_a_socket_is_a_place_to_add_a_part() {
    let recipe = copy_of("robot", "studio_socket_add");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert!(studio.select(Some("head")));

    let on_right = socket_seen(&studio, "head", "right");
    let clicked = studio.point_at(VIEW, on_right);
    assert_eq!(
        clicked,
        Clicked::Socket {
            part: "head".into(),
            socket: "right".into()
        }
    );
    // Finding the place changed nothing, and the head is still selected.
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);
    assert_eq!(studio.selected(), Some("head"));

    let said = studio
        .add_to_socket("head", "right", &Made::Shape("cylinder".into()))
        .unwrap();
    assert_eq!(said, "Added \"cylinder\" (cylinder) on head.right");
    let after = std::fs::read_to_string(&recipe).unwrap();
    let (gone, new) = changed(&before, &after);
    assert!(gone.is_empty(), "{gone:?}");
    // Half the head's smallest side across, the head's colour, joined by
    // the side that faces the socket.
    assert_eq!(
        new,
        [
            "",
            "[[parts]]",
            "name = \"cylinder\"",
            "shape = \"cylinder\"",
            "radius = 0.08",
            "height = 0.16",
            "color = \"steel\"",
            "attach = \"left\"",
            "to = \"head.right\"",
        ]
    );
    assert_eq!(studio.selected(), Some("cylinder"));
    assert!(!studio.has_mistakes());

    // A second one gets the next free name; a defined part can be added
    // the same way.
    studio
        .add_to_socket("head", "left", &Made::Shape("cylinder".into()))
        .unwrap();
    assert_eq!(studio.selected(), Some("cylinder_2"));
    assert_eq!(studio.definitions(), ["arm", "leg", "eye"]);
    studio
        .add_to_socket("head", "back", &Made::Use("eye".into()))
        .unwrap();
    assert_eq!(studio.selected(), Some("eye"));
    assert_eq!(
        studio.part("eye").unwrap().attached_to.as_deref(),
        Some("head.back")
    );
}

#[test]
fn a_click_on_the_selected_parts_surface_makes_a_socket_there() {
    let recipe = copy_of("robot", "studio_surface_add");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert!(studio.select(Some("head")));

    // A point on the head's front, up and to one side of the middle.
    let bounds = studio.part("head").unwrap().bounds.unwrap();
    let (middle, half) = (bounds.center(), bounds.half());
    let point = middle + vec3(half.x * 0.5, half.y * -0.6, half.z);
    let head_was_seen = seen(&studio, middle);
    let clicked = studio.point_at(VIEW, seen(&studio, point));
    let Clicked::Surface { part, spot } = clicked else {
        panic!("expected the head's surface, got {clicked:?}");
    };
    assert_eq!(part, "head");
    assert_eq!(spot.on, "front");
    assert!((spot.shift[0] - 0.5).abs() < 0.02, "{spot:?}");
    assert!((spot.shift[1] + 0.6).abs() < 0.02, "{spot:?}");
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);

    let said = studio
        .add_to_surface("head", &spot, &Made::Shape("cone".into()))
        .unwrap();
    assert!(
        said.starts_with("Added the socket \"spot\" on the front of \"head\"")
            && said.ends_with("then added \"cone\" (cone) on head.spot"),
        "{said}"
    );
    // The cone is selected, and sits where the click was.
    assert_eq!(studio.selected(), Some("cone"));
    let cone = studio.part("cone").unwrap();
    assert_eq!(cone.attached_to.as_deref(), Some("head.spot"));
    // (Measured from the head: with a cone sticking out in front, the
    // model's middle is somewhere else.)
    let head_now = studio.part("head").unwrap().bounds.unwrap().center();
    let landed = socket_at(&studio, "head", "spot") - head_now;
    assert!(
        (landed - (point - middle)).length() < 0.01,
        "{landed:?} for {:?}",
        point - middle
    );
    // The camera went with the model, so the picture didn't jump: the
    // head is where it was on the screen.
    let head_seen = seen(&studio, head_now);
    assert!((head_seen.0 - head_was_seen.0).abs() < 0.5, "{head_seen:?}");
    assert!((head_seen.1 - head_was_seen.1).abs() < 0.5, "{head_seen:?}");
    let after = std::fs::read_to_string(&recipe).unwrap();
    assert!(
        after.contains("spot = { on = \"front\", shift = ["),
        "{after}"
    );
    assert!(!studio.has_mistakes());

    // A second socket on the same part gets the next name.
    assert!(studio.select(Some("head")));
    let middle = studio.part("head").unwrap().bounds.unwrap().center();
    let other = middle + vec3(half.x * -0.5, half.y * -0.6, half.z);
    let Clicked::Surface { spot, .. } = studio.point_at(VIEW, seen(&studio, other)) else {
        panic!("expected the head's surface");
    };
    studio
        .add_to_surface("head", &spot, &Made::Shape("box".into()))
        .unwrap();
    assert_eq!(
        studio.part("box").unwrap().attached_to.as_deref(),
        Some("head.spot_2")
    );
}

#[test]
fn a_part_is_dragged_to_another_socket() {
    let recipe = copy_of("robot", "studio_drag_join");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);

    // Nothing selected, or the pointer off the selected part: no drag,
    // so the view turns as usual.
    let on_tip = on_screen(&studio, "antenna_tip", VIEW.0);
    let on_tip = (on_tip.0, on_tip.1 * VIEW.1 as f64 / VIEW.0 as f64);
    let on_tip = {
        let middle = studio.part("antenna_tip").unwrap().bounds.unwrap().center();
        let _ = on_tip;
        seen(&studio, middle)
    };
    assert!(!studio.drag_start(VIEW, on_tip));
    assert!(studio.select(Some("antenna_tip")));
    assert!(!studio.drag_start(VIEW, (5.0, 5.0)));
    assert_eq!(studio.dragging(), None);

    // On the selected part: the drag begins, and the sockets shown are
    // those of the parts it could join.
    assert!(studio.drag_start(VIEW, on_tip));
    assert_eq!(studio.dragging(), Some("antenna_tip"));
    let markers = studio.markers();
    assert!(markers.iter().any(|marker| marker.part == "body"));
    assert!(markers.iter().any(|marker| marker.part == "left_arm"));
    assert!(
        markers.iter().all(|marker| marker.part != "antenna_tip"),
        "a part can't be joined to itself"
    );

    // Dropped on the body's right side socket.
    studio.hover(VIEW, Some(socket_seen(&studio, "body", "right")));
    let said = studio.drag_end().unwrap().unwrap();
    assert_eq!(said, "Joined \"antenna_tip\" to body.right");
    assert_eq!(studio.dragging(), None);
    let after = std::fs::read_to_string(&recipe).unwrap();
    assert_eq!(
        changed(&before, &after),
        (vec!["to = \"antenna.top\""], vec!["to = \"body.right\""])
    );
    assert_eq!(studio.selected(), Some("antenna_tip"));

    // A part isn't offered the sockets of what hangs from it: the head
    // can't be joined to its own antenna.
    assert!(studio.select(Some("head")));
    // (A point of its front that no eye covers.)
    let head = studio.part("head").unwrap().bounds.unwrap();
    let on_head = seen(
        &studio,
        head.center() + vec3(head.half().x * -0.1, head.half().y * -0.6, head.half().z),
    );
    assert!(studio.drag_start(VIEW, on_head));
    let markers = studio.markers();
    for hanging in ["antenna", "left_eye", "right_eye", "head"] {
        assert!(
            markers.iter().all(|marker| marker.part != hanging),
            "{hanging}"
        );
    }
    // Dropped on nothing: nothing changes.
    studio.hover(VIEW, Some((4.0, 4.0)));
    let why = studio.drag_end().unwrap().unwrap_err();
    assert_eq!(why, "Not moved: drop it on a socket, or on another part");
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), after);
    assert_eq!(studio.drag_end(), None, "nothing is being dragged now");

    // When only looking, nothing can be dragged.
    let mut looking = Studio::open(&recipe, true);
    assert!(looking.select(Some("head")));
    assert!(!looking.drag_start(VIEW, on_head));
}

#[test]
fn a_part_dropped_on_a_surface_slides_there() {
    let recipe = copy_of("robot", "studio_drag_slide");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);

    // The red panel sits on the body's `chest` socket, which is its
    // alone. Dragged along the body's front, that socket moves.
    assert!(studio.select(Some("panel")));
    let on_panel = seen(
        &studio,
        studio.part("panel").unwrap().bounds.unwrap().center(),
    );
    assert!(studio.drag_start(VIEW, on_panel));
    let body = studio.part("body").unwrap().bounds.unwrap();
    let target = body.center() + vec3(body.half().x * 0.5, body.half().y * -0.5, body.half().z);
    studio.hover(VIEW, Some(seen(&studio, target)));
    let said = studio.drag_end().unwrap().unwrap();
    assert!(said.starts_with("Moved the socket body.chest"), "{said}");
    let after = std::fs::read_to_string(&recipe).unwrap();
    let (gone, new) = changed(&before, &after);
    assert_eq!(
        gone,
        ["chest     = { on = \"front\", shift = [0, 0.3, 0] }"]
    );
    assert_eq!(new.len(), 1);
    assert!(
        new[0].starts_with("chest     = { on = \"front\", shift = [0.5, -0.5"),
        "{new:?}"
    );
    // The panel's back is at that point now.
    let landed = socket_at(&studio, "body", "chest");
    assert!(
        (landed - target).length() < 0.01,
        "{landed:?} for {target:?}"
    );
    assert!(!studio.has_mistakes());

    // The antenna's tip sits on the antenna's built-in `top`, which
    // can't be moved. Dropped on the head, a socket is made there for it.
    assert!(studio.select(Some("antenna_tip")));
    let on_tip = seen(
        &studio,
        studio.part("antenna_tip").unwrap().bounds.unwrap().center(),
    );
    assert!(studio.drag_start(VIEW, on_tip));
    let head = studio.part("head").unwrap().bounds.unwrap();
    let target = head.center() + vec3(head.half().x, head.half().y * 0.6, head.half().z * 0.6);
    studio.hover(VIEW, Some(seen(&studio, target)));
    let said = studio.drag_end().unwrap().unwrap();
    assert!(
        said.starts_with("Added the socket \"spot\" on the right of \"head\"")
            && said.ends_with("then joined \"antenna_tip\" to head.spot"),
        "{said}"
    );
    let tip = studio.part("antenna_tip").unwrap();
    assert_eq!(tip.attached_to.as_deref(), Some("head.spot"));
    let landed = socket_at(&studio, "head", "spot");
    assert!(
        (landed - target).length() < 0.01,
        "{landed:?} for {target:?}"
    );

    // Dragged again along the head, its own socket moves: no second one.
    let on_tip = seen(
        &studio,
        studio.part("antenna_tip").unwrap().bounds.unwrap().center(),
    );
    assert!(studio.drag_start(VIEW, on_tip));
    let target = head.center() + vec3(head.half().x, head.half().y * -0.6, head.half().z * -0.5);
    studio.hover(VIEW, Some(seen(&studio, target)));
    let said = studio.drag_end().unwrap().unwrap();
    assert!(said.starts_with("Moved the socket head.spot"), "{said}");
    let head_sockets = &studio.part("head").unwrap().sockets;
    assert!(head_sockets.iter().all(|socket| socket.name != "spot_2"));
    let landed = socket_at(&studio, "head", "spot");
    assert!(
        (landed - target).length() < 0.01,
        "{landed:?} for {target:?}"
    );
    assert!(!studio.has_mistakes());
}
