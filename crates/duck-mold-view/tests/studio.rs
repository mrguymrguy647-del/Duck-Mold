//! What Duck Mold Studio does, driven with no window: opening a file,
//! the viewport's pictures, noticing saves from outside, and (in the
//! steps that follow) selecting, editing, sockets, undo and the library.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use duck_mold_view::render::BACKGROUND;
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::viewport::Picture;

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
