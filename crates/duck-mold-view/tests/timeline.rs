//! Phase 5, step 7: the timeline, driven with no window.
//!
//! The robot's animations: keys along the timeline, moving through
//! time, playing, loop and smooth, ghosts, keys dragged, added and
//! removed, and a turn between two keys making a new pose there.

use std::path::{Path, PathBuf};

use duck_mold_core::math::{Mat3, Vec3};
use duck_mold_view::studio::handles::Grip;
use duck_mold_view::studio::viewport::Picture;
use duck_mold_view::studio::{Mode, Studio};

const VIEW: (usize, usize) = (640, 480);

fn robot(test: &str) -> (PathBuf, Studio) {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let recipe = folder.join("robot.toml");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot.toml"),
        &recipe,
    )
    .unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.pose_mode().unwrap();
    (recipe, studio)
}

fn file(recipe: &Path) -> String {
    std::fs::read_to_string(recipe).unwrap()
}

fn added<'a>(old: &str, new: &'a str) -> Vec<&'a str> {
    let old: Vec<&str> = old.lines().collect();
    new.lines().filter(|line| !old.contains(line)).collect()
}

/// The highest point of a part as the model is shown now.
fn top_of(studio: &Studio, part: &str) -> f64 {
    studio
        .shown_model()
        .unwrap()
        .part(part)
        .unwrap()
        .bounds
        .unwrap()
        .max
        .y
}

#[test]
fn an_animation_is_chosen_and_its_keys_are_on_the_timeline() {
    let (_, mut studio) = robot("timeline_keys");
    assert_eq!(studio.animation_names(), ["idle", "walk", "wave"]);
    assert_eq!(studio.animation(), None);
    assert_eq!(studio.time(), None);
    assert!(studio.choose_animation(Some("wave")));
    assert!(!studio.choose_animation(Some("dance")));
    assert_eq!(studio.animation(), Some("wave"));
    assert_eq!(studio.time(), Some(0.0));
    assert_eq!(studio.animation_length(), 1.5);
    assert_eq!(studio.animation_settings(), Some((false, true)));
    let keys: Vec<(usize, f64, String)> = studio
        .keys()
        .into_iter()
        .map(|key| (key.number, key.at, key.pose))
        .collect();
    assert_eq!(
        keys,
        [
            (1, 0.0, "rest".to_owned()),
            (2, 0.35, "wave".to_owned()),
            (3, 0.6, "wave_out".to_owned()),
            (4, 0.85, "wave".to_owned()),
            (5, 1.1, "wave_out".to_owned()),
            (6, 1.5, "rest".to_owned()),
        ]
    );
    // At a key its pose is the one chosen: what a drag would change.
    assert_eq!(studio.pose(), "rest");
    studio.click_key(2);
    assert_eq!(studio.time(), Some(0.35));
    assert_eq!(studio.pose(), "wave");
    assert_eq!(
        studio.status(),
        "Key 2: the pose \"wave\". Key 4 uses it too, and changes with it"
    );
    // Moving through time shows the in-between: the arm half way up.
    let up = top_of(&studio, "left_arm");
    studio.set_time(0.175);
    assert!(studio.key_here().is_none());
    let half = top_of(&studio, "left_arm");
    studio.set_time(0.0);
    let down = top_of(&studio, "left_arm");
    assert!(down < half && half < up, "{down} {half} {up}");
    // Past the end it stays at the end.
    studio.set_time(9.0);
    assert_eq!(studio.time(), Some(1.5));
    // Choosing a pose in the list shows the pose itself.
    studio.choose_pose("step_left");
    assert_eq!(studio.time(), None);
    assert_eq!(studio.animation(), Some("wave"));
    // Letting go of the animation.
    studio.choose_animation(None);
    assert_eq!(studio.animation(), None);
}

#[test]
fn play_runs_the_animation_and_loops_or_stops_at_its_end() {
    let (_, mut studio) = robot("timeline_play");
    studio.choose_animation(Some("walk"));
    assert!(studio.play());
    assert!(studio.is_playing());
    // No rings while it plays.
    studio.select(Some("left_leg"));
    assert!(studio.grips().is_empty());
    studio.tick(0.3);
    assert!((studio.time().unwrap() - 0.3).abs() < 1e-9);
    // The walk loops: past its end, round again.
    studio.tick(0.7);
    assert!((studio.time().unwrap() - 0.2).abs() < 1e-9);
    assert!(studio.is_playing());
    studio.toggle_play();
    assert!(!studio.is_playing());
    let stopped = studio.time();
    studio.tick(1.0);
    assert_eq!(studio.time(), stopped);
    // The wave plays once, and stops at its end.
    studio.choose_animation(Some("wave"));
    studio.play();
    studio.tick(2.0);
    assert_eq!(studio.time(), Some(1.5));
    assert!(!studio.is_playing());
    // Played again from its end, it starts over.
    studio.play();
    assert_eq!(studio.time(), Some(0.0));
    // Taking hold of something stops it.
    studio.tick(0.2);
    studio.grab(VIEW, (5.0, 5.0));
    assert!(!studio.is_playing());
}

#[test]
fn play_works_in_part_mode_too_and_the_model_comes_back_to_rest() {
    let (_, mut studio) = robot("timeline_part_mode");
    studio.part_mode();
    assert_eq!(studio.mode(), Mode::Part);
    let rest = studio.model().unwrap().faces.clone();
    studio.choose_animation(Some("wave"));
    // Chosen but not playing: the model stands as built.
    assert_eq!(studio.shown_model().unwrap().faces, rest);
    studio.play();
    studio.tick(0.35);
    assert_ne!(studio.shown_model().unwrap().faces, rest);
    studio.stop();
    assert_eq!(studio.shown_model().unwrap().faces, rest);
}

#[test]
fn loop_smooth_and_the_keys_are_edits() {
    let (recipe, mut studio) = robot("timeline_edits");
    studio.choose_animation(Some("wave"));
    let before = file(&recipe);
    // Loop on: one line.
    studio.set_looping(true).unwrap();
    assert!(file(&recipe).contains("[animations.wave]\nloop = true\nkeys = ["));
    assert_eq!(file(&recipe).len(), before.len() + "loop = true\n".len());
    studio.undo().unwrap();
    studio.set_smooth(false).unwrap();
    assert_eq!(added(&before, &file(&recipe)), ["smooth = false"]);
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
    // A key dragged snaps to 0.05 s and can't pass its neighbours.
    assert_eq!(studio.key_time(3, 0.63, false), Some(0.65));
    assert_eq!(studio.key_time(3, 0.63, true), Some(0.63));
    assert_eq!(studio.key_time(3, 2.0, false), Some(0.84));
    assert_eq!(studio.key_time(3, 0.0, false), Some(0.36));
    assert_eq!(studio.key_time(1, 0.5, false), Some(0.0));
    studio.move_key(3, 0.66, false).unwrap();
    assert_eq!(
        added(&before, &file(&recipe)),
        ["  { at = 0.65, pose = \"wave_out\" },"]
    );
    assert_eq!(studio.time(), Some(0.65));
    assert_eq!(studio.pose(), "wave_out");
    studio.undo().unwrap();
    // Another pose for a key; then removed.
    studio.set_key_pose(3, "wave").unwrap();
    assert_eq!(
        added(&before, &file(&recipe)),
        ["  { at = 0.6, pose = \"wave\" },"]
    );
    studio.remove_key(3).unwrap();
    assert_eq!(studio.keys().len(), 5);
    studio.undo().unwrap();
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
    // + Key: the pose chosen, at the time shown.
    studio.choose_pose("step_left");
    studio.set_time(1.3);
    // (Choosing a pose let go of the time; set it again, then choose
    // the pose to add.)
    studio.choose_pose("step_left");
    studio.choose_animation(Some("wave"));
    studio.set_time(1.3);
    let pose_now = studio.pose().to_owned();
    studio.add_key_here().unwrap();
    assert_eq!(
        added(&before, &file(&recipe)),
        [format!("  {{ at = 1.3, pose = \"{pose_now}\" }},").as_str()]
    );
}

#[test]
fn new_makes_an_animation_with_a_first_key() {
    let (recipe, mut studio) = robot("timeline_new");
    studio.new_animation().unwrap();
    assert_eq!(studio.animation(), Some("animation"));
    assert!(
        file(&recipe)
            .ends_with("[animations.animation]\nkeys = [\n  { at = 0, pose = \"rest\" },\n]\n"),
        "{}",
        file(&recipe)
    );
    // One step to undo.
    studio.undo().unwrap();
    assert!(!file(&recipe).contains("[animations.animation]"));
    assert_eq!(studio.animation(), None);
}

/// Turns a ring of the selected part `degrees` round, and lets go.
fn turn_ring(studio: &mut Studio, which: usize, degrees: f64) -> Result<String, String> {
    let grip = Grip::Ring(which);
    let from = studio.grip_at(grip).unwrap();
    let orbit = studio.orbit;
    let seen = |point: Vec3| orbit.on_screen(point, VIEW.0, VIEW.1).unwrap();
    let at = seen(from);
    assert_eq!(studio.grab(VIEW, at), Some(grip));
    let model = studio.shown_model().unwrap();
    let part = model.part(studio.selected().unwrap()).unwrap();
    let centre = part.pivot + studio.offset();
    // Round the ring's own axis: the one square to the ring's points.
    let axes: Vec<Vec3> = (0..3)
        .filter_map(|column| part.axes.column(column).normalized())
        .collect();
    let axis = *axes
        .iter()
        .min_by(|a, b| {
            a.dot(from - centre)
                .abs()
                .total_cmp(&b.dot(from - centre).abs())
        })
        .unwrap();
    let to = centre + Mat3::around(axis, degrees) * (from - centre);
    let to = seen(to);
    studio.drag(VIEW, to, false);
    studio.release().unwrap()
}

#[test]
fn a_turn_at_a_key_changes_its_pose_and_between_keys_makes_a_new_one() {
    let (recipe, mut studio) = robot("timeline_turns");
    studio.choose_animation(Some("wave"));
    studio.select(Some("head"));
    // At key 2 (the pose wave), the head turned: the pose wave changes,
    // so key 4 changes with it.
    studio.click_key(2);
    let before = file(&recipe);
    turn_ring(&mut studio, 1, 20.0).unwrap();
    let after = file(&recipe);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "{new:?}");
    assert!(new[0].starts_with("head = [0, "), "{new:?}");
    studio.undo().unwrap();
    // Between keys 2 and 3: a new pose there, holding every part as the
    // animation has it then, and a key with it. One step to undo.
    studio.set_time(0.45);
    let said = turn_ring(&mut studio, 1, 20.0).unwrap();
    assert_eq!(said, "Made the pose \"wave_7\" here, with a key");
    assert_eq!(studio.pose(), "wave_7");
    let keys: Vec<(f64, String)> = studio
        .keys()
        .into_iter()
        .map(|key| (key.at, key.pose))
        .collect();
    assert_eq!(keys[2], (0.45, "wave_7".to_owned()));
    let text = file(&recipe);
    let pose = &text[text.find("[poses.wave_7]").unwrap()..];
    let pose = &pose[..pose.find("\n\n").unwrap_or(pose.len())];
    // Every part the wave moves, as it is at 0.45 s, the head as turned.
    let lines: Vec<&str> = pose.lines().skip(1).collect();
    assert_eq!(lines.len(), 3, "{pose}");
    assert!(lines[0].starts_with("head = [0, "), "{pose}");
    assert!(lines[1].starts_with("left_arm = ["), "{pose}");
    assert!(lines[2].starts_with("\"left_arm/elbow\" = ["), "{pose}");
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
    // At key 1, in rest: a new pose, and the key takes it.
    studio.click_key(1);
    assert_eq!(studio.pose(), "rest");
    let said = turn_ring(&mut studio, 1, 20.0).unwrap();
    assert!(said.starts_with("Made the pose \"wave_1\""), "{said}");
    assert_eq!(studio.keys()[0].pose, "wave_1");
    assert_eq!(studio.keys()[5].pose, "rest");
}

#[test]
fn the_ghosts_are_the_keys_before_and_after() {
    let (_, mut studio) = robot("timeline_ghosts");
    studio.choose_animation(Some("wave"));
    studio.set_time(0.5);
    let (before, after) = studio.ghost_keys();
    assert_eq!(before.unwrap().number, 2);
    assert_eq!(after.unwrap().number, 3);
    // At a key: its neighbours.
    studio.click_key(3);
    let (before, after) = studio.ghost_keys();
    assert_eq!((before.unwrap().number, after.unwrap().number), (2, 4));
    studio.click_key(1);
    let (before, after) = studio.ghost_keys();
    assert!(before.is_none());
    assert_eq!(after.unwrap().number, 2);
    // Drawn, or not.
    studio.set_time(0.5);
    let with = studio.picture(VIEW.0, VIEW.1);
    studio.set_ghosts(false);
    let without = studio.picture(VIEW.0, VIEW.1);
    assert_ne!(with.pixels, without.pixels);
    studio.set_ghosts(true);
    check_picture("studio-timeline-ghosts", &with);
}

fn check_picture(name: &str, picture: &Picture) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"));
    let found = picture.to_png();
    if std::env::var_os("DUCK_MOLD_ACCEPT").is_some() {
        std::fs::write(&path, &found).unwrap();
        return;
    }
    let stored = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "{} is missing. Create it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test timeline",
            path.display()
        )
    });
    if stored != found {
        let new = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.new.png"));
        std::fs::write(&new, &found).unwrap();
        panic!(
            "the picture {name} changed (the new one is in {}). If the change is meant, accept it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test timeline",
            new.display()
        );
    }
}
