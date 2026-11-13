//! What Duck Mold Studio does, driven with no window: opening a file,
//! the viewport's pictures, noticing saves from outside, and (in the
//! steps that follow) selecting, editing, sockets, undo and the library.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use duck_mold_core::edit::{Changes, Edit, Made, Sizes};
use duck_mold_core::math::{Mat3, Vec3, vec3};
use duck_mold_view::render::BACKGROUND;
use duck_mold_view::studio::handles::{Grip, Tool};
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
    // The file is named from the recipe's folder, as the recipe names
    // files: not by the whole way to it.
    assert!(
        said.starts_with("Wrote table.glb (") && said.ends_with(" bytes)"),
        "{said}"
    );
    assert_eq!(studio.status(), said);
    assert_eq!(&std::fs::read(&glb).unwrap()[..4], b"glTF");

    let said = studio.render_picture().unwrap();
    assert_eq!(said, "Wrote table.png (513 x 536 pixels)");
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
    studio
        .orbit
        .on_screen(middle + studio.offset(), size, size)
        .unwrap()
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
    studio
        .orbit
        .on_screen(point + studio.offset(), VIEW.0, VIEW.1)
        .unwrap()
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
    // A click there no longer opens anything: it would be the start of
    // a drag. The place is asked for (the right-click menu does).
    assert_eq!(
        studio.point_at(VIEW, seen(&studio, point)),
        Clicked::Selected("head".into())
    );
    let (part, spot) = studio
        .spot_at(VIEW, seen(&studio, point))
        .expect("the pointer is on the head");
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
    let (_, spot) = studio
        .spot_at(VIEW, seen(&studio, other))
        .expect("the pointer is on a part");
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

// ---- step 7: undo and redo ---------------------------------------------------

fn colour(part: &str, color: &str) -> Edit {
    set(
        part,
        Changes {
            color: Some(color.into()),
            ..Changes::default()
        },
    )
}

#[test]
fn every_edit_can_be_undone_and_redone_to_the_byte() {
    let recipe = copy_of("robot", "studio_undo");
    let original = std::fs::read(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert_eq!(studio.can_undo(), None);
    assert_eq!(studio.undo().unwrap_err(), "Nothing to undo");

    // Three edits of different kinds.
    studio.apply(&colour("head", "red")).unwrap();
    let after_one = std::fs::read(&recipe).unwrap();
    studio
        .apply(&Edit::Rename {
            part: "head".into(),
            new_name: "skull".into(),
        })
        .unwrap();
    let after_two = std::fs::read(&recipe).unwrap();
    studio
        .apply(&Edit::Remove {
            part: "antenna_tip".into(),
            with_attached: false,
        })
        .unwrap();
    let after_three = std::fs::read(&recipe).unwrap();
    assert_eq!(studio.can_undo(), Some("Removed \"antenna_tip\""));
    assert_eq!(studio.can_redo(), None);

    // Back, one at a time: the file is each earlier version exactly.
    assert_eq!(studio.undo().unwrap(), "Undid: Removed \"antenna_tip\"");
    assert_eq!(std::fs::read(&recipe).unwrap(), after_two);
    assert!(
        studio.part("antenna_tip").is_some(),
        "the model is built again"
    );
    assert_eq!(
        studio.undo().unwrap(),
        "Undid: Renamed \"head\" to \"skull\""
    );
    assert_eq!(std::fs::read(&recipe).unwrap(), after_one);
    assert_eq!(studio.undo().unwrap(), "Undid: Set \"head\": colour red");
    assert_eq!(std::fs::read(&recipe).unwrap(), original);
    assert_eq!(studio.can_undo(), None);
    assert_eq!(studio.can_redo(), Some("Set \"head\": colour red"));

    // And forward again.
    assert_eq!(studio.redo().unwrap(), "Redid: Set \"head\": colour red");
    assert_eq!(std::fs::read(&recipe).unwrap(), after_one);
    studio.redo().unwrap();
    studio.redo().unwrap();
    assert_eq!(std::fs::read(&recipe).unwrap(), after_three);
    assert_eq!(studio.redo().unwrap_err(), "Nothing to redo");

    // A new edit after an undo: what was undone can't come back.
    studio.undo().unwrap();
    studio.apply(&colour("body", "red")).unwrap();
    assert_eq!(studio.can_redo(), None);
    assert_eq!(studio.can_undo(), Some("Set \"body\": colour red"));
}

#[test]
fn edits_made_together_are_one_step() {
    let recipe = copy_of("robot", "studio_undo_together");
    let original = std::fs::read(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);

    // A part added at a point of a surface is a socket and a part.
    assert!(studio.select(Some("head")));
    let bounds = studio.part("head").unwrap().bounds.unwrap();
    let point = bounds.center()
        + vec3(
            bounds.half().x * 0.5,
            bounds.half().y * -0.6,
            bounds.half().z,
        );
    let (_, spot) = studio
        .spot_at(VIEW, seen(&studio, point))
        .expect("the pointer is on a part");
    studio
        .add_to_surface("head", &spot, &Made::Shape("cone".into()))
        .unwrap();
    assert!(studio.part("cone").is_some());
    studio.undo().unwrap();
    assert_eq!(std::fs::read(&recipe).unwrap(), original);
    assert_eq!(studio.can_undo(), None);

    // A drag is one step too.
    assert!(studio.select(Some("antenna_tip")));
    let on_tip = seen(
        &studio,
        studio.part("antenna_tip").unwrap().bounds.unwrap().center(),
    );
    assert!(studio.drag_start(VIEW, on_tip));
    let head = studio.part("head").unwrap().bounds.unwrap();
    let target = head.center() + vec3(head.half().x, head.half().y * 0.6, head.half().z * 0.6);
    studio.hover(VIEW, Some(seen(&studio, target)));
    studio.drag_end().unwrap().unwrap();
    assert_ne!(std::fs::read(&recipe).unwrap(), original);
    studio.undo().unwrap();
    assert_eq!(std::fs::read(&recipe).unwrap(), original);
    // The part that was selected is still selected, back where it was.
    assert_eq!(studio.selected(), Some("antenna_tip"));
    assert_eq!(
        studio.part("antenna_tip").unwrap().attached_to.as_deref(),
        Some("antenna.top")
    );
}

#[test]
fn a_change_from_outside_is_a_step_to_undo_too() {
    let recipe = copy_of("robot", "studio_undo_outside");
    let original = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.apply(&colour("head", "red")).unwrap();
    let ours = std::fs::read_to_string(&recipe).unwrap();

    // An agent (or a text editor) saves the recipe.
    let theirs = ours.replace("name = \"robot\"", "name = \"robot2\"");
    save(&recipe, &theirs);
    assert!(studio.check_files());
    assert_eq!(studio.can_undo(), Some("Changed outside the Studio"));

    // Undo takes back the latest change to the file, whoever made it.
    assert_eq!(studio.undo().unwrap(), "Undid: Changed outside the Studio");
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), ours);
    assert_eq!(studio.undo().unwrap(), "Undid: Set \"head\": colour red");
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), original);
    // And both can be made again.
    studio.redo().unwrap();
    studio.redo().unwrap();
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), theirs);

    // Saved from outside and undone before the Studio has looked: the
    // outside change is seen first, and is what is taken back. Nothing
    // is written over unseen.
    let later = theirs.replace("name = \"robot2\"", "name = \"robot3\"");
    save(&recipe, &later);
    assert_eq!(studio.undo().unwrap(), "Undid: Changed outside the Studio");
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), theirs);
    assert_eq!(studio.can_redo(), Some("Changed outside the Studio"));
}

#[test]
fn undo_goes_back_through_a_version_with_mistakes() {
    let recipe = copy_of("robot", "studio_undo_mistake");
    let good = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    save(
        &recipe,
        &good.replace("shape = \"box\"", "shape = \"blob\""),
    );
    assert!(studio.check_files());
    assert!(studio.has_mistakes());
    studio.undo().unwrap();
    assert!(!studio.has_mistakes());
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), good);
    studio.redo().unwrap();
    assert!(studio.has_mistakes());
}

#[test]
fn when_only_looking_nothing_is_undone_and_old_steps_are_forgotten() {
    let recipe = copy_of("robot", "studio_undo_look");
    let mut looking = Studio::open(&recipe, true);
    assert_eq!(
        looking.undo().unwrap_err(),
        "This window is only for looking: nothing was changed"
    );

    // Only the latest 500 steps are kept. (A small recipe, so that 510
    // edits are quick.)
    let small = recipe.with_file_name("small.toml");
    std::fs::write(
        &small,
        "version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\nsize = [1, 1, 1]\n",
    )
    .unwrap();
    let mut studio = Studio::open(&small, false);
    for step in 0..510 {
        let tall = 1.0 + f64::from(step + 1) / 100.0;
        studio
            .apply(&set(
                "a",
                Changes {
                    sizes: Sizes {
                        size: Some([1.0, tall, 1.0]),
                        ..Sizes::default()
                    },
                    ..Changes::default()
                },
            ))
            .unwrap();
    }
    let mut undone = 0;
    while studio.undo().is_ok() {
        undone += 1;
    }
    assert_eq!(undone, duck_mold_view::studio::history::MOST_STEPS);
    // The first ten edits are beyond reach: the box is 1.1 tall.
    assert!(
        std::fs::read_to_string(&small)
            .unwrap()
            .contains("size = [1, 1.1, 1]")
    );
}

// ---- step 8: the parts library -----------------------------------------------

/// A copy of the robot with a `parts` folder beside it: the examples'
/// own parts, and a small recipe.
fn robot_with_library(test: &str) -> PathBuf {
    let recipe = copy_of("robot", test);
    let parts = recipe.with_file_name("parts");
    std::fs::create_dir_all(&parts).unwrap();
    for file in ["crate.glb", "lamp.gltf"] {
        std::fs::copy(examples().join("parts").join(file), parts.join(file)).unwrap();
    }
    std::fs::write(
        parts.join("hat.toml"),
        "version = 0\nname = \"hat\"\n[palette]\nfelt = \"#333344\"\n[[parts]]\nname = \"cone\"\nshape = \"cone\"\nradius = 0.15\nheight = 0.25\ncolor = \"felt\"\n",
    )
    .unwrap();
    recipe
}

#[test]
fn the_library_is_the_parts_folder_beside_the_recipe_with_a_picture_each() {
    // The examples' own `parts` folder is the library of every example.
    let studio = Studio::open(examples().join("desk.toml"), false);
    assert_eq!(studio.library_folder(), examples().join("parts"));
    let items = studio.library();
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, ["crate", "lamp"]);
    assert_eq!(items[0].written, "parts/crate.glb");
    assert_eq!(items[1].written, "parts/lamp.gltf");
    for item in items {
        assert_eq!(item.problem, None, "{}", item.name);
        let picture = item.picture.as_ref().unwrap();
        assert_eq!((picture.width, picture.height), (96, 96));
        assert!(drawn(picture) > 500, "{} is drawn", item.name);
    }
    // The same code as `render`, so the same picture on every computer.
    check_picture("library-lamp", items[1].picture.as_ref().unwrap());

    // No such folder: an empty library, and no mistake.
    let recipe = copy_of("robot", "studio_library_none");
    let studio = Studio::open(&recipe, false);
    assert!(studio.library().is_empty());
    assert!(!studio.has_mistakes());
}

#[test]
fn a_library_part_is_added_to_a_socket_like_a_shape() {
    let recipe = robot_with_library("studio_library_add");
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    let names: Vec<&str> = studio
        .library()
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, ["crate", "hat", "lamp"]);

    let hat = studio.library()[1].clone();
    assert_eq!(hat.written, "parts/hat.toml");
    let said = studio
        .add_to_socket("head", "top", &Made::File(hat.written.clone()))
        .unwrap();
    assert_eq!(said, "Added \"hat\" (parts/hat.toml) on head.top");
    let after = std::fs::read_to_string(&recipe).unwrap();
    let (gone, new) = changed(&before, &after);
    assert!(gone.is_empty());
    // The recipe names the file: nothing is copied, and it has no sizes
    // of its own.
    assert_eq!(
        new,
        [
            "",
            "[[parts]]",
            "name = \"hat\"",
            "file = \"parts/hat.toml\"",
            "attach = \"bottom\"",
            "to = \"head.top\"",
        ]
    );
    assert_eq!(studio.selected(), Some("hat"));
    assert!(!studio.has_mistakes(), "{:?}", studio.problems());
    assert!(studio.part("hat/cone").is_some());

    // A model file, at a point of a surface.
    assert!(studio.select(Some("body")));
    let body = studio.part("body").unwrap().bounds.unwrap();
    let point = body.center() + vec3(body.half().x * 0.75, body.half().y * -0.6, body.half().z);
    let (_, spot) = studio
        .spot_at(VIEW, seen(&studio, point))
        .expect("the pointer is on a part");
    studio
        .add_to_surface("body", &spot, &Made::File("parts/lamp.gltf".into()))
        .unwrap();
    assert_eq!(studio.selected(), Some("lamp"));
    assert_eq!(
        studio.part("lamp").unwrap().attached_to.as_deref(),
        Some("body.spot")
    );

    // The library's files are watched: when the hat changes, so does the
    // robot wearing it.
    let tall = std::fs::read_to_string(recipe.with_file_name("parts").join("hat.toml"))
        .unwrap()
        .replace("height = 0.25", "height = 0.6");
    let short = studio.model().unwrap().bounds().unwrap().size().y;
    save(&recipe.with_file_name("parts").join("hat.toml"), &tall);
    assert!(studio.check_files());
    assert!(studio.model().unwrap().bounds().unwrap().size().y > short + 0.3);
}

#[test]
fn the_library_follows_its_folder_and_says_what_cant_be_used() {
    let recipe = robot_with_library("studio_library_changes");
    let mut studio = Studio::open(&recipe, false);
    let parts = recipe.with_file_name("parts");
    assert!(!studio.check_library(), "nothing has changed yet");
    let reads = studio.library_reads();

    // A file that isn't a model is listed, with what is wrong with it,
    // and has no picture.
    std::fs::write(
        parts.join("broken.toml"),
        "version = 0\n[[parts]]\nshape = \"blob\"\n",
    )
    .unwrap();
    assert!(studio.check_library());
    assert_eq!(studio.library_reads(), reads + 1);
    let broken = &studio.library()[0];
    assert_eq!(broken.name, "broken");
    assert_eq!(broken.picture, None);
    assert!(
        broken.problem.as_deref().unwrap().contains("blob"),
        "{:?}",
        broken.problem
    );
    // Things that aren't parts are left out.
    std::fs::write(parts.join("notes.txt"), "not a part").unwrap();
    std::fs::remove_file(parts.join("broken.toml")).unwrap();
    assert!(studio.check_library());
    let names: Vec<&str> = studio
        .library()
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, ["crate", "hat", "lamp"]);

    // Another folder can be chosen. A file outside the recipe's folder
    // is written with the way to it.
    let elsewhere = recipe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("studio_library_elsewhere");
    let _ = std::fs::remove_dir_all(&elsewhere);
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::copy(parts.join("hat.toml"), elsewhere.join("cap.toml")).unwrap();
    studio.set_library_folder(Some(elsewhere.clone()));
    assert_eq!(studio.library_folder(), elsewhere);
    let items = studio.library();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].written, "../studio_library_elsewhere/cap.toml");
    studio
        .add_to_socket("head", "top", &Made::File(items[0].written.clone()))
        .unwrap();
    assert!(!studio.has_mistakes(), "{:?}", studio.problems());
    assert!(studio.part("cap/cone").is_some());

    // The recipe's own folder as the library: the recipe isn't a part of
    // itself.
    studio.set_library_folder(Some(recipe.parent().unwrap().to_path_buf()));
    assert!(studio.library().iter().all(|item| item.name != "robot"));
    studio.set_library_folder(None);
    assert_eq!(studio.library_folder(), parts);
}

#[test]
fn files_are_named_from_the_recipes_folder() {
    // The desk has two parts from files, and a warning about one. Opened
    // by its whole path, the Studio still names them as the recipe does.
    let studio = Studio::open(examples().join("desk.toml"), false);
    let rows = studio.outline();
    let lamp = rows.iter().find(|row| row.path == "lamp").unwrap();
    assert_eq!(lamp.kind, "parts/lamp.gltf (mesh)");
    let warning = &studio.problems()[0];
    assert!(
        warning
            .text
            .contains("of \"parts/lamp.gltf\" have no colour"),
        "{}",
        warning.text
    );
}

#[test]
fn a_json_recipe_is_edited_and_undone_the_same_way() {
    let folder = folder("studio_json");
    let recipe = folder.join("crate.json");
    std::fs::copy(examples().join("crate.json"), &recipe).unwrap();
    let before = std::fs::read_to_string(&recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert!(!studio.look_only());
    let first = studio.outline()[0].path.clone();
    assert!(studio.select(Some(&first)));
    let facts = studio.facts().expect("a JSON recipe's parts can be edited");
    assert_eq!(facts.shape.as_deref(), Some("box"));

    studio
        .add_to_socket(&first, "top", &Made::Shape("sphere".into()))
        .unwrap();
    assert_eq!(studio.selected(), Some("sphere"));
    let after = std::fs::read_to_string(&recipe).unwrap();
    assert!(after.contains("\"shape\": \"sphere\""), "{after}");
    // It is still JSON, and what was there is still there, as it was.
    let (gone, _) = changed(&before, &after);
    assert!(
        gone.len() <= 1,
        "only the line the new part follows: {gone:?}"
    );
    assert!(!studio.has_mistakes(), "{:?}", studio.problems());

    studio.undo().unwrap();
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);
}

// ---- Phase 3, step 1: handles ------------------------------------------------

/// A floor with a crate standing free on it.
const BOXES: &str = r##"# Two boxes.
version = 0
name = "boxes"

[palette]
stone = "#C0C0C0"
red   = "#D0453A"

[[parts]]
name = "floor"
shape = "box"
size = [2, 0.2, 2]
color = "stone"

# A crate, standing free.
[[parts]]
name = "crate"
shape = "box"
size = [0.5, 0.5, 0.5]
color = "red"
move = [0.5, 0.35, 0]   # on the floor
"##;

/// The boxes in a folder of their own, with the crate selected and the
/// pointer's picture set.
fn boxes(test: &str) -> (PathBuf, Studio) {
    let recipe = folder(test).join("boxes.toml");
    std::fs::write(&recipe, BOXES).unwrap();
    let mut studio = Studio::open(&recipe, false);
    assert!(studio.select(Some("crate")));
    studio.hover(VIEW, None);
    (recipe, studio)
}

/// Where a point of the picture's space is on the 640 x 480 screen.
fn at_screen(studio: &Studio, point: Vec3) -> (f64, f64) {
    studio.orbit.on_screen(point, VIEW.0, VIEW.1).unwrap()
}

/// Presses on a handle, moves the pointer so that the handle's own point
/// goes to `to`, and leaves it held there.
fn pull(studio: &mut Studio, grip: Grip, to: impl Fn(Vec3) -> Vec3, free: bool) {
    let from = studio.grip_at(grip).unwrap();
    assert_eq!(studio.grab(VIEW, at_screen(studio, from)), Some(grip));
    let to = to(from);
    studio.drag(VIEW, at_screen(studio, to), free);
}

fn file(recipe: &Path) -> String {
    std::fs::read_to_string(recipe).unwrap()
}

#[test]
fn an_arrow_moves_a_free_part_and_writes_once_when_let_go() {
    let (recipe, mut studio) = boxes("handles_move");
    assert_eq!(studio.tool, Tool::Move);
    assert_eq!(
        studio.grips(),
        [Grip::Arrow(0), Grip::Arrow(1), Grip::Arrow(2)]
    );

    // The red arrow, pulled 0.3 m along x.
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + vec3(0.3, 0.0, 0.0),
        false,
    );
    assert_eq!(studio.grabbed(), Some(Grip::Arrow(0)));
    assert_eq!(studio.drag_label(), Some("x 0.8 m"));
    // While it is held, the picture shows the crate moved, and the file
    // is as it was.
    assert_eq!(file(&recipe), BOXES);
    let shown = studio.shown_model().unwrap();
    let crate_shown = shown
        .parts
        .iter()
        .find(|part| part.name == "crate")
        .unwrap();
    assert!(((crate_shown.local.shift - shown.zero).x - 0.8).abs() < 1e-9);
    assert!(
        ((studio.part("crate").unwrap().local.shift - studio.model().unwrap().zero).x - 0.5).abs()
            < 1e-9
    );

    // Let go: one line of the file changes, comment and all else kept.
    let said = studio.release().unwrap().unwrap();
    assert_eq!(said, "Set \"crate\": move [0.8, 0.35, 0]");
    let now = file(&recipe);
    assert_eq!(
        changed(BOXES, &now),
        (
            vec!["move = [0.5, 0.35, 0]   # on the floor"],
            vec!["move = [0.8, 0.35, 0]   # on the floor"]
        )
    );
    assert_eq!(studio.grabbed(), None);
    assert_eq!(studio.selected(), Some("crate"));
    // One step to undo, back to the byte.
    studio.undo().unwrap();
    assert_eq!(file(&recipe), BOXES);
    assert_eq!(studio.can_undo(), None);

    // It snaps to the grid: 0.33 m further is 0.3 m further. With
    // snapping off, it is where the pointer is.
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + vec3(0.33, 0.0, 0.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("x 0.8 m"));
    studio.cancel_drag();
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + vec3(0.33, 0.0, 0.0),
        true,
    );
    assert_eq!(studio.drag_label(), Some("x 0.83 m"));
    // Escape lets go and writes nothing.
    studio.cancel_drag();
    assert_eq!(studio.grabbed(), None);
    assert_eq!(file(&recipe), BOXES);
    assert_eq!(studio.shown_model(), studio.model());

    // Up, and front to back.
    pull(
        &mut studio,
        Grip::Arrow(1),
        |from| from + vec3(0.0, 0.5, 0.0),
        false,
    );
    studio.release().unwrap().unwrap();
    pull(
        &mut studio,
        Grip::Arrow(2),
        |from| from + vec3(0.0, 0.0, -0.4),
        false,
    );
    studio.release().unwrap().unwrap();
    let now = file(&recipe);
    let line = now
        .lines()
        .find(|line| line.starts_with("move = "))
        .unwrap();
    assert_eq!(line, "move = [0.5, 0.85, -0.4]   # on the floor");
}

#[test]
fn the_grid_stays_put_when_a_part_moves() {
    let (_, mut studio) = boxes("handles_grid");
    // Where the floor is on the screen, and the recipe's own zero.
    let floor_was = seen(
        &studio,
        studio.part("floor").unwrap().bounds.unwrap().center(),
    );
    // Move the crate far out to one side: the model's middle moves.
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + vec3(3.0, 0.0, 0.0),
        false,
    );
    studio.release().unwrap().unwrap();
    let zero = studio.model().unwrap().zero;
    assert!(
        zero.x < -0.5,
        "the model's zero point went after the crate: {zero:?}"
    );
    // The floor hasn't moved in the picture, and the crate is 3 m along.
    let floor_is = seen(
        &studio,
        studio.part("floor").unwrap().bounds.unwrap().center(),
    );
    assert!((floor_is.0 - floor_was.0).abs() < 1e-6 && (floor_is.1 - floor_was.1).abs() < 1e-6);
    let shown_at = studio.part("crate").unwrap().bounds.unwrap().center() + studio.offset();
    assert!((shown_at.x - 3.5).abs() < 1e-9, "{shown_at:?}");
}

#[test]
fn lifting_the_lowest_part_says_what_happens_instead() {
    // The floor is the lowest part. Lifted a little, it is still the
    // lowest: it stays on the ground, and the crate comes down.
    let (recipe, mut studio) = boxes("handles_lowest");
    assert!(studio.select(Some("floor")));
    let crate_was = studio.part("crate").unwrap().bounds.unwrap().min.y;
    pull(
        &mut studio,
        Grip::Arrow(1),
        |from| from + vec3(0.0, 0.1, 0.0),
        false,
    );
    let said = studio.release().unwrap().unwrap();
    assert_eq!(
        said,
        "Set \"floor\": move [0, 0.1, 0]. It is the lowest part, so it stays on the ground and the others come down instead"
    );
    assert!(file(&recipe).contains("move = [0, 0.1, 0]"));
    assert_eq!(studio.part("floor").unwrap().bounds.unwrap().min.y, 0.0);
    let crate_is = studio.part("crate").unwrap().bounds.unwrap().min.y;
    assert!((crate_was - crate_is - 0.1).abs() < 1e-9);

    // A model with one part: lifting it shows nothing at all.
    let lone = recipe.with_file_name("lone.toml");
    duck_mold_core::tools::new_file(&lone).unwrap();
    let mut studio = Studio::open(&lone, false);
    studio.hover(VIEW, None);
    assert!(studio.select(Some("body")));
    let before = studio.picture(VIEW.0, VIEW.1);
    pull(
        &mut studio,
        Grip::Arrow(1),
        |from| from + vec3(0.0, 1.0, 0.0),
        false,
    );
    let said = studio.release().unwrap().unwrap();
    assert!(
        said.ends_with("It is the only part, and a model always stands on the ground, so lifting it shows nothing"),
        "{said}"
    );
    assert!(file(&lone).contains("move = [0, 1, 0]"));
    assert_eq!(studio.picture(VIEW.0, VIEW.1), before);
    // Sideways it does move, over the grid.
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + vec3(1.0, 0.0, 0.0),
        false,
    );
    studio.release().unwrap().unwrap();
    assert_ne!(studio.picture(VIEW.0, VIEW.1), before);
}

#[test]
fn dragging_a_free_part_slides_it_over_the_ground_or_onto_a_socket() {
    let (recipe, mut studio) = boxes("handles_body");
    // Take hold of the crate itself, by a point of its top.
    let top = studio.part("crate").unwrap().bounds.unwrap();
    // (Not its middle: the green arrow stands there.)
    let hold = vec3(top.center().x + 0.15, top.max.y, top.center().z + 0.15) + studio.offset();
    assert_eq!(
        studio.grab(VIEW, at_screen(&studio, hold)),
        Some(Grip::Body)
    );
    // To a point of the same height, 0.4 m along x and 0.3 m along z.
    studio.drag(VIEW, at_screen(&studio, hold + vec3(0.4, 0.0, 0.3)), false);
    assert_eq!(studio.drag_label(), Some("x 0.9 m, z 0.3 m"));
    assert_eq!(file(&recipe), BOXES);
    studio.release().unwrap().unwrap();
    assert!(
        file(&recipe).contains("move = [0.9, 0.35, 0.3]"),
        "{}",
        file(&recipe)
    );

    // Dropped on a dot of the floor, it is joined there.
    let hold = hold + vec3(0.4, 0.0, 0.3);
    assert_eq!(
        studio.grab(VIEW, at_screen(&studio, hold)),
        Some(Grip::Body)
    );
    assert!(studio.markers().iter().any(|marker| marker.part == "floor"));
    let dot = socket_at(&studio, "floor", "front") + studio.offset();
    studio.drag(VIEW, at_screen(&studio, dot), false);
    assert_eq!(studio.drag_label(), Some("join floor.front"));
    let said = studio.release().unwrap().unwrap();
    assert_eq!(said, "Joined \"crate\" to floor.front");
    let text = file(&recipe);
    assert!(text.contains("to = \"floor.front\""), "{text}");
    assert!(
        !text.contains("move = [0.9"),
        "a joined part has no move: {text}"
    );
    assert_eq!(
        studio.part("crate").unwrap().attached_to.as_deref(),
        Some("floor.front")
    );
}

#[test]
fn a_ring_turns_a_free_part_in_steps_of_fifteen_degrees() {
    let (recipe, mut studio) = boxes("handles_rotate");
    studio.tool = Tool::Rotate;
    assert_eq!(
        studio.grips(),
        [Grip::Ring(0), Grip::Ring(1), Grip::Ring(2)]
    );
    // The middle of the rings is the crate's own middle.
    let middle = studio.part("crate").unwrap().place.shift + studio.offset();
    let turned =
        |degrees: f64| move |from: Vec3| middle + Mat3::around_y(degrees) * (from - middle);

    // The green ring, turned 37 degrees: it snaps to 30.
    pull(&mut studio, Grip::Ring(1), turned(37.0), false);
    assert_eq!(studio.drag_label(), Some("30°"));
    assert_eq!(file(&recipe), BOXES);
    let said = studio.release().unwrap().unwrap();
    assert_eq!(said, "Set \"crate\": rotate [0, 30, 0]");
    let now = file(&recipe);
    let (gone, new) = changed(BOXES, &now);
    assert!(gone.is_empty());
    assert_eq!(new, ["rotate = [0, 30, 0]"]);
    // The crate is turned that way in the model.
    let turn = studio.part("crate").unwrap().place.turn;
    let wanted = Mat3::around_y(30.0);
    for row in 0..3 {
        for column in 0..3 {
            assert!((turn.rows[row][column] - wanted.rows[row][column]).abs() < 1e-9);
        }
    }

    // Again, with snapping off: 37 more.
    pull(&mut studio, Grip::Ring(1), turned(37.0), true);
    assert_eq!(studio.drag_label(), Some("37°"));
    studio.release().unwrap().unwrap();
    assert!(
        file(&recipe).contains("rotate = [0, 67, 0]"),
        "{}",
        file(&recipe)
    );

    // A turn around another direction is added to the first, as one
    // `rotate` that gives the same turn.
    let before = studio.part("crate").unwrap().place.turn;
    let around_x = move |from: Vec3| middle + Mat3::around_x(90.0) * (from - middle);
    pull(&mut studio, Grip::Ring(0), around_x, false);
    assert_eq!(studio.drag_label(), Some("90°"));
    studio.release().unwrap().unwrap();
    let turn = studio.part("crate").unwrap().place.turn;
    let wanted = Mat3::around_x(90.0) * before;
    for row in 0..3 {
        for column in 0..3 {
            assert!(
                (turn.rows[row][column] - wanted.rows[row][column]).abs() < 1e-3,
                "{turn:?} for {wanted:?}"
            );
        }
    }
}

#[test]
fn boxes_scale_a_part_one_way_or_all_round() {
    let (recipe, mut studio) = boxes("handles_scale");
    studio.tool = Tool::Scale;
    assert_eq!(
        studio.grips(),
        [Grip::Box(0), Grip::Box(1), Grip::Box(2), Grip::Middle]
    );
    let middle = studio.part("crate").unwrap().bounds.unwrap().center() + studio.offset();

    // The red box, pulled out to one and a half times as far.
    pull(
        &mut studio,
        Grip::Box(0),
        |from| middle + (from - middle) * 1.5,
        false,
    );
    assert_eq!(studio.drag_label(), Some("x x 1.5"));
    let said = studio.release().unwrap().unwrap();
    assert_eq!(said, "Set \"crate\": scale [1.5, 1, 1]");
    let now = file(&recipe);
    let (gone, new) = changed(BOXES, &now);
    assert!(gone.is_empty());
    assert_eq!(new, ["scale = [1.5, 1, 1]"]);
    let wide = studio.part("crate").unwrap().bounds.unwrap().size();
    assert!((wide.x - 0.75).abs() < 1e-9 && (wide.y - 0.5).abs() < 1e-9);

    // The middle box: 60 pixels to the right is half as big again, all
    // round.
    studio.undo().unwrap();
    let from = studio.grip_point(Grip::Middle).unwrap();
    assert_eq!(studio.grab(VIEW, from), Some(Grip::Middle));
    studio.drag(VIEW, (from.0 + 60.0, from.1), false);
    studio.release().unwrap().unwrap();
    let now = file(&recipe);
    assert_eq!(changed(BOXES, &now).1, ["scale = 1.5"]);

    // Never down to nothing.
    pull(
        &mut studio,
        Grip::Box(1),
        |from| middle + (from - middle) * 0.001,
        false,
    );
    studio.release().unwrap().unwrap();
    assert!(
        file(&recipe).contains("scale = [1.5, 0.1, 1.5]"),
        "{}",
        file(&recipe)
    );
}

#[test]
fn a_joined_part_slides_along_its_surface_and_twists_around_its_join() {
    let recipe = copy_of("robot", "handles_joined");
    let before = file(&recipe);
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);

    // The red panel sits on the body's `chest`. It has two arrows, lying
    // on the body's front.
    assert!(studio.select(Some("panel")));
    assert_eq!(studio.grips(), [Grip::Arrow(0), Grip::Arrow(1)]);
    let chest_was = socket_at(&studio, "body", "chest");
    let along = studio.grip_at(Grip::Arrow(0)).unwrap() - (chest_was + studio.offset());
    let along = along.normalized().unwrap();
    assert!(
        along.z.abs() < 1e-9,
        "the arrow lies on the body's front: {along:?}"
    );
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + along * 0.1,
        false,
    );
    assert_eq!(studio.drag_label(), Some("slid 0.1 m"));
    assert_eq!(file(&recipe), before);
    let said = studio.release().unwrap().unwrap();
    assert!(said.starts_with("Moved the socket body.chest"), "{said}");
    // The socket moved 0.1 m that way, and the panel with it; only the
    // socket's line changed.
    let chest_is = socket_at(&studio, "body", "chest");
    assert!(
        (chest_is - (chest_was + along * 0.1)).length() < 0.002,
        "{chest_is:?}"
    );
    let now = file(&recipe);
    let (gone, new) = changed(&before, &now);
    assert_eq!((gone.len(), new.len()), (1, 1));
    assert!(
        new[0].starts_with("chest     = { on = \"front\", shift = ["),
        "{new:?}"
    );

    // One ring, around the join: it twists.
    studio.tool = Tool::Rotate;
    assert_eq!(studio.grips(), [Grip::Ring(0)]);
    let join = chest_is + studio.offset();
    let facing = vec3(0.0, 0.0, 1.0);
    pull(
        &mut studio,
        Grip::Ring(0),
        |from| join + Mat3::around(facing, 45.0) * (from - join),
        false,
    );
    assert_eq!(studio.drag_label(), Some("twist 45°"));
    studio.release().unwrap().unwrap();
    assert!(
        file(&recipe).contains("to = \"body.chest\"\ntwist = 45\n"),
        "{}",
        file(&recipe)
    );

    // The antenna's tip sits on a built-in socket, which can't be moved:
    // sliding it makes a socket for it.
    studio.tool = Tool::Move;
    assert!(studio.select(Some("antenna_tip")));
    let across = studio.grip_at(Grip::Arrow(0)).unwrap()
        - (socket_at(&studio, "antenna", "top") + studio.offset());
    let across = across.normalized().unwrap();
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + across * 0.01,
        true,
    );
    let said = studio.release().unwrap().unwrap();
    assert!(
        said.starts_with("Added the socket \"spot\" on the top of \"antenna\"")
            && said.ends_with("then joined \"antenna_tip\" to antenna.spot"),
        "{said}"
    );
    assert!(!studio.has_mistakes());
}

#[test]
fn a_part_inside_a_turned_part_moves_in_that_parts_own_directions() {
    let recipe = folder("handles_nested").join("nested.toml");
    std::fs::write(
        &recipe,
        "version = 0\n[[parts]]\nname = \"table\"\nrotate = [0, 90, 0]\nscale = 2\n\n[[parts.parts]]\nname = \"top\"\nshape = \"box\"\nsize = [1, 0.1, 1]\n\n[[parts.parts]]\nname = \"cup\"\nshape = \"cylinder\"\nradius = 0.1\nheight = 0.2\nmove = [0, 0.15, 0]\n",
    )
    .unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    assert!(studio.select(Some("table/cup")));
    // The red arrow points along the table's own x, which the table's
    // quarter turn has made the model's -z. Pulled 0.4 m in the model is
    // 0.2 of the table's own units, since the table is twice as big.
    let from = studio.grip_at(Grip::Arrow(0)).unwrap();
    let middle = studio.part("table/cup").unwrap().bounds.unwrap().center() + studio.offset();
    let along = (from - middle).normalized().unwrap();
    assert!((along - vec3(0.0, 0.0, -1.0)).length() < 1e-9, "{along:?}");
    pull(
        &mut studio,
        Grip::Arrow(0),
        |from| from + along * 0.4,
        false,
    );
    studio.release().unwrap().unwrap();
    assert!(
        file(&recipe).contains("move = [0.2, 0.15, 0]"),
        "{}",
        file(&recipe)
    );
}

#[test]
fn handles_are_drawn_and_only_on_what_can_be_changed() {
    let (recipe, mut studio) = boxes("handles_pictures");
    check_picture("studio-handles-move", &studio.picture(VIEW.0, VIEW.1));
    studio.tool = Tool::Rotate;
    check_picture("studio-handles-rotate", &studio.picture(VIEW.0, VIEW.1));
    studio.tool = Tool::Scale;
    // The pointer on a handle lights it up.
    let on_box = studio.grip_point(Grip::Box(1)).unwrap();
    studio.hover(VIEW, Some(on_box));
    check_picture("studio-handles-scale", &studio.picture(VIEW.0, VIEW.1));
    // A part being dragged is drawn where it would go.
    studio.tool = Tool::Move;
    pull(
        &mut studio,
        Grip::Arrow(1),
        |from| from + vec3(0.0, 0.6, 0.0),
        false,
    );
    check_picture("studio-handles-dragging", &studio.picture(VIEW.0, VIEW.1));
    studio.cancel_drag();

    // Nothing selected: no handles, and a press takes hold of nothing.
    studio.select(None);
    assert!(studio.grips().is_empty());
    assert_eq!(studio.grab(VIEW, (320.0, 240.0)), None);
    // When only looking: none either.
    let mut looking = Studio::open(&recipe, true);
    looking.hover(VIEW, None);
    assert!(looking.select(Some("crate")));
    assert!(looking.grips().is_empty());
    assert_eq!(looking.grab(VIEW, (320.0, 240.0)), None);
    assert_eq!(studio.snap_text(), "Snap 0.1 m, 15°  (Ctrl: off)");
}

// ---- Phase 3, step 2: the Size tool ------------------------------------------

#[test]
fn pulling_a_face_of_a_box_changes_its_size_and_keeps_the_face_opposite() {
    let (recipe, mut studio) = boxes("size_box");
    studio.tool = Tool::Size;
    // Six knobs: right and left, top and bottom, front and back.
    assert_eq!(studio.grips(), (0..6).map(Grip::Knob).collect::<Vec<_>>());
    let was = studio.part("crate").unwrap().bounds.unwrap();

    // The right face, pulled out 0.2 m.
    pull(
        &mut studio,
        Grip::Knob(0),
        |from| from + vec3(0.2, 0.0, 0.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("width 0.7 m"));
    assert_eq!(file(&recipe), BOXES, "nothing is written while it is held");
    let said = studio.release().unwrap().unwrap();
    assert_eq!(
        said,
        "Set \"crate\": size [0.7, 0.5, 0.5], move [0.6, 0.35, 0]"
    );
    let now = file(&recipe);
    assert_eq!(
        changed(BOXES, &now),
        (
            vec![
                "size = [0.5, 0.5, 0.5]",
                "color = \"red\"",
                "move = [0.5, 0.35, 0]   # on the floor"
            ],
            vec![
                "size = [0.7, 0.5, 0.5]",
                "color = \"red\"",
                "move = [0.6, 0.35, 0]   # on the floor"
            ]
        )
    );
    // The left face is where it was; the right one is 0.2 m further out.
    let is = studio.part("crate").unwrap().bounds.unwrap();
    let offset_change = 0.0;
    let shown = |bounds: duck_mold_core::math::Bounds, studio: &Studio| {
        (bounds.min + studio.offset(), bounds.max + studio.offset())
    };
    let _ = offset_change;
    let (was_min, was_max) = (was.min, was.max);
    let (is_min, is_max) = shown(is, &studio);
    // (Measured in the picture's space, where the grid stays put.)
    let (was_min, was_max) = {
        let (_, before) = boxes("size_box_before");
        (was_min + before.offset(), was_max + before.offset())
    };
    assert!(
        (is_min.x - was_min.x).abs() < 1e-9,
        "{is_min:?} {was_min:?}"
    );
    assert!((is_max.x - was_max.x - 0.2).abs() < 1e-9);
    // One step to undo.
    studio.undo().unwrap();
    assert_eq!(file(&recipe), BOXES);

    // The top, pushed down 0.2 m: lower, and still standing on the floor.
    pull(
        &mut studio,
        Grip::Knob(2),
        |from| from + vec3(0.0, -0.2, 0.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("height 0.3 m"));
    studio.release().unwrap().unwrap();
    assert!(file(&recipe).contains("size = [0.5, 0.3, 0.5]"));
    assert!(file(&recipe).contains("move = [0.5, 0.25, 0]"));
    // A face can't be pushed through the one opposite: a centimetre is
    // the least.
    pull(
        &mut studio,
        Grip::Knob(4),
        |from| from + vec3(0.0, 0.0, -3.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("depth 0.01 m"));
    studio.cancel_drag();
    // It snaps in steps; with snapping off it is where the pointer is.
    pull(
        &mut studio,
        Grip::Knob(1),
        |from| from + vec3(-0.07, 0.0, 0.0),
        true,
    );
    assert_eq!(studio.drag_label(), Some("width 0.57 m"));
    studio.cancel_drag();
}

#[test]
fn round_shapes_have_knobs_for_radius_and_height() {
    let recipe = folder("size_round").join("round.toml");
    std::fs::write(
        &recipe,
        "version = 0\n[[parts]]\nname = \"post\"\nshape = \"cylinder\"\nradius = 0.2\nheight = 1\n\n[[parts]]\nname = \"ball\"\nshape = \"sphere\"\nradius = 0.3\nattach = \"bottom\"\nto = \"post.top\"\n",
    )
    .unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.tool = Tool::Size;

    // A cylinder: top, bottom, and one on its side.
    assert!(studio.select(Some("post")));
    assert_eq!(
        studio.grips(),
        [Grip::Knob(0), Grip::Knob(1), Grip::Knob(2)]
    );
    pull(
        &mut studio,
        Grip::Knob(0),
        |from| from + vec3(0.0, 0.5, 0.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("height 1.5 m"));
    studio.release().unwrap().unwrap();
    pull(
        &mut studio,
        Grip::Knob(2),
        |from| from + vec3(0.1, 0.0, 0.0),
        false,
    );
    assert_eq!(studio.drag_label(), Some("radius 0.3 m"));
    studio.release().unwrap().unwrap();
    let text = file(&recipe);
    assert!(text.contains("radius = 0.3\nheight = 1.5\n"), "{text}");

    // A ball: one knob. It is joined, so only its size changes: its join
    // keeps it on the post.
    assert!(studio.select(Some("ball")));
    assert_eq!(studio.grips(), [Grip::Knob(0)]);
    pull(
        &mut studio,
        Grip::Knob(0),
        |from| from + vec3(0.2, 0.0, 0.0),
        false,
    );
    let said = studio.release().unwrap().unwrap();
    assert_eq!(said, "Set \"ball\": radius 0.5");
    let ball = studio.part("ball").unwrap().bounds.unwrap();
    let post = studio.part("post").unwrap().bounds.unwrap();
    assert!((ball.min.y - post.max.y).abs() < 1e-6, "still on the post");
    // (The post is free, so pulling its top did move its middle.)
    assert!(file(&recipe).contains("move = [0, 0.25, 0]"));
}

#[test]
fn the_size_tool_is_for_shapes_and_changes_a_definition_for_all_its_parts() {
    let recipe = copy_of("robot", "size_definition");
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.tool = Tool::Size;
    // A part made of parts has no size of its own.
    assert!(studio.select(Some("left_arm")));
    assert!(studio.grips().is_empty());
    // An eye is made from a definition: pulling its face changes the
    // definition, and so the other eye.
    assert!(studio.select(Some("left_eye")));
    assert_eq!(studio.grips().len(), 6);
    let other_was = studio.part("right_eye").unwrap().bounds.unwrap().size();
    pull(
        &mut studio,
        Grip::Knob(2),
        |from| from + vec3(0.0, 0.02, 0.0),
        true,
    );
    let said = studio.release().unwrap().unwrap();
    assert_eq!(
        said,
        "Set \"left_eye\": size [0.08, 0.1, 0.03] (also changed: right_eye)"
    );
    let other_is = studio.part("right_eye").unwrap().bounds.unwrap().size();
    assert!((other_is.y - other_was.y - 0.02).abs() < 1e-9);

    // The knobs are drawn on the faces.
    let (_, mut studio) = boxes("size_picture");
    studio.tool = Tool::Size;
    let on_top = studio.grip_point(Grip::Knob(2)).unwrap();
    studio.hover(VIEW, Some(on_top));
    check_picture("studio-handles-size", &studio.picture(VIEW.0, VIEW.1));
}
