//! Phase 5, step 8: the animation library in the Studio, with no
//! window. The starters, the files in `animations` beside the recipe,
//! adding one (an `add-animation --from` edit) and saving one (what
//! `save-animation` does).

use std::path::{Path, PathBuf};

use duck_mold_core::edit::AnimationFrom;
use duck_mold_view::studio::Studio;

fn open(test: &str, example: &str) -> (PathBuf, Studio) {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let recipe = folder.join(example);
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(example),
        &recipe,
    )
    .unwrap();
    with_images_of(example, &recipe);
    let studio = Studio::open(&recipe, false);
    (recipe, studio)
}

fn file(recipe: &Path) -> String {
    std::fs::read_to_string(recipe).unwrap()
}

#[test]
fn the_library_lists_the_six_starters_first() {
    let (_, studio) = open("shelf_starters", "robot.toml");
    let shelf = studio.animation_library();
    let names: Vec<&str> = shelf.iter().map(|shelved| shelved.name.as_str()).collect();
    assert_eq!(names, ["idle", "walk", "wave", "jump", "open", "roll"]);
    let walk = &shelf[1];
    assert_eq!(walk.source, "starter");
    assert_eq!(walk.from, AnimationFrom::Starter("walk".into()));
    assert_eq!(
        walk.about,
        "For two legs. Legs and arms swing against each other. Loops, in place."
    );
    // The robot has every part the two-legged ones move, and none of
    // the door's or the wheels'.
    for shelved in &shelf[..4] {
        assert!(studio.missing_parts(shelved).is_empty(), "{}", shelved.name);
    }
    assert_eq!(studio.missing_parts(&shelf[4]), ["door"]);
    assert_eq!(studio.missing_parts(&shelf[5]).len(), 4);
}

#[test]
fn adding_a_starter_copies_it_in_under_a_free_name_and_undo_takes_it_out() {
    let (recipe, mut studio) = open("shelf_add", "robot.toml");
    let before = file(&recipe);
    // The robot has a jump already, made from this starter and then
    // fitted: the copy is jump_2. Its crouch was changed, so the
    // starter's comes in with a number; its leap wasn't, so it is used
    // as it is.
    let jump = studio.animation_library()[3].clone();
    let said = studio.add_from_library(&jump).unwrap();
    assert_eq!(
        said,
        "Added the animation \"jump_2\", a copy of \"jump\" from the starters, with 5 keys and the pose \"jump_crouch_2\" (names taken by other poses here, so they came in as: \"jump_crouch\" as \"jump_crouch_2\"); it uses the pose \"jump_up\" already here, which says the same"
    );
    assert!(
        file(&recipe).contains("  { at = 0.3, pose = \"jump_crouch_2\", event = \"takeoff\" },")
    );
    // It is the timeline's animation now.
    assert_eq!(studio.animation(), Some("jump_2"));
    assert_eq!(
        studio.animation_names(),
        ["idle", "walk", "wave", "jump", "jump_2"]
    );

    // The robot has a walk already: the starter's comes in as walk_2,
    // with its own poses.
    let walk = studio.animation_library()[1].clone();
    studio.add_from_library(&walk).unwrap();
    assert_eq!(studio.animation(), Some("walk_2"));
    assert!(file(&recipe).contains("[animations.walk_2]\nloop = true\n"));
    assert!(file(&recipe).contains("[poses.walk_left]\n"));

    studio.undo().unwrap();
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
}

#[test]
fn a_saved_animation_shows_in_the_list_and_another_model_takes_it() {
    let (robot, mut studio) = open("shelf_save", "robot.toml");
    let said = studio.save_to_animation_library("wave").unwrap();
    assert_eq!(
        said,
        "Saved the animation \"wave\" to the library: animations/wave.toml. Another model takes it from the Animations list"
    );
    assert_eq!(studio.status(), said);
    let saved = robot.parent().unwrap().join("animations/wave.toml");
    assert!(file(&saved).starts_with("# Saved by `duck-mold save-animation` from "));
    // Saved again: a number, never over the first.
    studio.save_to_animation_library("wave").unwrap();
    assert!(
        robot
            .parent()
            .unwrap()
            .join("animations/wave_2.toml")
            .exists()
    );

    let shelf = studio.animation_library();
    let mine: Vec<_> = shelf
        .iter()
        .filter(|shelved| shelved.source != "starter")
        .collect();
    assert_eq!(mine.len(), 2);
    assert_eq!(mine[0].name, "wave");
    assert_eq!(mine[0].source, "animations/wave.toml");
    assert_eq!(
        mine[0].about,
        "6 keys, 1.5 s. Moves left_arm, left_arm/elbow, head."
    );

    // A second robot in the same folder takes it, as "wave_2" since it
    // has a wave of its own; the poses say the same as its own, so none
    // is added.
    let other = robot.parent().unwrap().join("robot_two.toml");
    std::fs::copy(&robot, &other).unwrap();
    let mut two = Studio::open(&other, false);
    let wave = two
        .animation_library()
        .into_iter()
        .find(|shelved| shelved.source == "animations/wave.toml")
        .unwrap();
    let said = two.add_from_library(&wave).unwrap();
    assert_eq!(
        said,
        "Added the animation \"wave_2\", a copy of \"wave\" from \"animations/wave.toml\", with 6 keys; it uses the poses \"wave\" and \"wave_out\" already here, which say the same"
    );
    assert_eq!(two.animation(), Some("wave_2"));
}

#[test]
fn a_file_that_cant_be_read_is_listed_with_why() {
    let (recipe, mut studio) = open("shelf_bad_file", "robot.toml");
    let folder = recipe.parent().unwrap().join("animations");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("broken.toml"), "this isn't = = toml").unwrap();
    std::fs::write(folder.join("empty.toml"), "version = 0\nname = \"empty\"\n").unwrap();
    std::fs::write(folder.join("notes.txt"), "not a recipe").unwrap();
    // The Studio sees the new files when it checks.
    for _ in 0..3 {
        studio.tick(0.3);
    }
    let shelf = studio.animation_library();
    let mine: Vec<_> = shelf
        .iter()
        .filter(|shelved| shelved.source != "starter")
        .map(|shelved| (shelved.name.as_str(), shelved.problem.as_deref()))
        .collect();
    assert_eq!(
        mine,
        [
            (
                "broken.toml",
                Some("it can't be read: check it with duck-mold check")
            ),
            ("empty.toml", Some("it has no animations")),
        ]
    );
    assert!(studio.animation_library_reads() >= 2);
}

#[test]
fn a_door_takes_open_and_leaves_the_two_legged_ones() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("shelf_door");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let recipe = folder.join("door.toml");
    std::fs::write(
        &recipe,
        "version = 0\nname = \"door\"\n\n[[parts]]\nname = \"frame\"\nshape = \"box\"\nsize = [0.1, 2, 0.1]\nsockets = { hinge = { on = \"right\" } }\n\n[[parts]]\nname = \"door\"\nshape = \"box\"\nsize = [0.9, 2, 0.05]\nattach = \"left\"\nto = \"frame.hinge\"\n",
    )
    .unwrap();
    let mut studio = Studio::open(&recipe, false);
    let shelf = studio.animation_library();
    assert_eq!(studio.missing_parts(&shelf[1]).len(), 4);
    let open = shelf[4].clone();
    assert!(studio.missing_parts(&open).is_empty());
    studio.add_from_library(&open).unwrap();
    assert!(file(&recipe).ends_with(
        "[poses.door_open]\ndoor = [0, -100, 0]\n\n[animations.open]\nkeys = [\n  { at = 0, pose = \"rest\" },\n  { at = 1, pose = \"door_open\", event = \"opened\" },\n]\n"
    ));
    // Played, the door turns.
    studio.play();
    studio.tick(0.5);
    let door = studio
        .shown_model()
        .unwrap()
        .part("door")
        .unwrap()
        .bounds
        .unwrap();
    let rest = studio
        .model()
        .unwrap()
        .part("door")
        .unwrap()
        .bounds
        .unwrap();
    assert!((door.max.z - rest.max.z).abs() > 0.1, "{door:?} {rest:?}");
}

/// The images an example's recipe names (`textures/<example>_*.png`),
/// put beside a copy of it: it doesn't build without them.
#[allow(dead_code)]
fn with_images_of(example: &str, recipe: &std::path::Path) {
    let stem = example.rsplit_once('.').map_or(example, |(stem, _)| stem);
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/textures");
    let to = recipe.parent().unwrap().join("textures");
    for entry in std::fs::read_dir(from).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&format!("{stem}_")) && name.ends_with(".png") {
            std::fs::create_dir_all(&to).unwrap();
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}
