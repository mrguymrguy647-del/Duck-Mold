//! Phase 5, step 6: Pose mode in the Studio, driven with no window.
//!
//! The robot has limits on its joins, six poses and three animations.
//! A click selects a part; rings appear at its join, one for each
//! direction its limits let it turn; a drag turns it and stops at the
//! limit; letting go is one `set-pose`.

use std::path::{Path, PathBuf};

use duck_mold_core::math::{Mat3, Vec3};
use duck_mold_core::model::Model;
use duck_mold_view::studio::handles::Grip;
use duck_mold_view::studio::viewport::Picture;
use duck_mold_view::studio::{Mode, PoseTool, Studio};

const VIEW: (usize, usize) = (640, 480);

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// A copy of the robot in a folder of its own, to change, open in the
/// Studio in Pose mode.
fn robot(test: &str) -> (PathBuf, Studio) {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let recipe = folder.join("robot.toml");
    std::fs::copy(examples().join("robot.toml"), &recipe).unwrap();
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.pose_mode().unwrap();
    (recipe, studio)
}

fn file(recipe: &Path) -> String {
    std::fs::read_to_string(recipe).unwrap()
}

/// The lines only the new text has.
fn added<'a>(old: &str, new: &'a str) -> Vec<&'a str> {
    let old: Vec<&str> = old.lines().collect();
    new.lines().filter(|line| !old.contains(line)).collect()
}

fn at_screen(studio: &Studio, point: Vec3) -> (f64, f64) {
    studio.orbit.on_screen(point, VIEW.0, VIEW.1).unwrap()
}

/// Where a part's join is in the picture, as the model is shown now.
fn join_seen(studio: &Studio, path: &str) -> Vec3 {
    let model = studio.shown_model().unwrap();
    model.part(path).unwrap().pivot + studio.offset()
}

/// A socket of the model as it is shown now.
fn socket_now(model: &Model, part: &str, name: &str) -> Vec3 {
    model
        .part(part)
        .unwrap()
        .sockets
        .iter()
        .find(|socket| socket.name == name)
        .unwrap()
        .at
}

/// Presses on a ring and turns the pointer `degrees` round it (as
/// [`Mat3::around`] counts round `axis`), leaving it held there.
fn turn_ring(studio: &mut Studio, which: usize, axis: Vec3, degrees: f64, free: bool) {
    let grip = Grip::Ring(which);
    let from = studio.grip_at(grip).unwrap();
    assert_eq!(studio.grab(VIEW, at_screen(studio, from)), Some(grip));
    let centre = join_seen(studio, studio.selected().unwrap());
    let to = centre + Mat3::around(axis, degrees) * (from - centre);
    studio.drag(VIEW, at_screen(studio, to), free);
}

/// The model's direction a ring turns around: the ring's own points lie
/// square to it.
fn ring_axis(studio: &Studio, which: usize) -> Vec3 {
    let centre = join_seen(studio, studio.selected().unwrap());
    let point = studio.grip_at(Grip::Ring(which)).unwrap() - centre;
    // Two points of a ring a quarter apart: the axis is square to both.
    // The ring's nearest point is one; the axis is found from the turn
    // that maps it onto the ring again, so take it from the part itself:
    // the candidates are the part's directions as its parent holds them.
    let model = studio.shown_model().unwrap();
    let part = model.part(studio.selected().unwrap()).unwrap();
    let candidates = [
        part.axes.column(0),
        part.axes.column(1),
        part.axes.column(2),
    ];
    candidates
        .into_iter()
        .filter_map(|axis| axis.normalized())
        .min_by(|a, b| a.dot(point).abs().total_cmp(&b.dot(point).abs()))
        .unwrap()
}

#[test]
fn pose_mode_shows_the_poses_and_stands_the_model_in_one() {
    let (_, mut studio) = robot("pose_mode_poses");
    assert_eq!(studio.mode(), Mode::Pose);
    assert_eq!(
        studio.pose_names(),
        [
            "rest",
            "step_left",
            "step_right",
            "wave",
            "wave_out",
            "breathe_in",
            "breathe_out"
        ]
    );
    assert_eq!(studio.pose(), "rest");
    // At rest the model is as it is built.
    let rest = studio.model().unwrap().clone();
    assert_eq!(studio.shown_model().unwrap().faces, rest.faces);
    assert!(studio.posed_parts().is_empty());
    // In the wave, the left arm is up.
    assert!(studio.choose_pose("wave"));
    assert_eq!(studio.pose(), "wave");
    assert_eq!(studio.posed_parts(), ["left_arm", "left_arm/elbow", "head"]);
    let up = studio.shown_model().unwrap().part("left_arm").unwrap();
    let down = rest.part("left_arm").unwrap();
    assert!(up.bounds.unwrap().max.y > down.bounds.unwrap().max.y + 0.3);
    // The model itself, and what the window says is selected, don't
    // change.
    assert_eq!(studio.model().unwrap().faces, rest.faces);
    assert!(!studio.choose_pose("dance"));
    assert_eq!(studio.pose(), "wave");
    // A click picks what is shown: the arm where it is in the pose.
    let hand = studio
        .shown_model()
        .unwrap()
        .part("left_arm/hand")
        .unwrap()
        .bounds
        .unwrap()
        .center()
        + studio.offset();
    let picked = studio
        .click(VIEW, at_screen(&studio, hand))
        .map(str::to_owned);
    assert_eq!(picked.as_deref(), Some("left_arm"));
    // Back to parts: the model stands as built again.
    studio.toggle_pose_mode().unwrap();
    assert_eq!(studio.mode(), Mode::Part);
    assert_eq!(studio.shown_model().unwrap().faces, rest.faces);
}

#[test]
fn a_part_has_a_ring_for_each_direction_it_may_turn() {
    let (_, mut studio) = robot("pose_mode_rings");
    // The head nods and looks round: x and y.
    studio.select(Some("head"));
    assert_eq!(studio.grips(), [Grip::Ring(0), Grip::Ring(1)]);
    assert_eq!(studio.why_no_pose_handles(), None);
    // The arm swings and lifts: x and z.
    studio.select(Some("left_arm"));
    assert_eq!(studio.grips(), [Grip::Ring(0), Grip::Ring(2)]);
    // A foot bends one way.
    studio.select(Some("left_leg/foot"));
    assert_eq!(studio.grips(), [Grip::Ring(0)]);
    // A fixed part has none, and says so.
    studio.select(Some("left_arm/forearm"));
    assert!(studio.grips().is_empty());
    assert_eq!(
        studio.why_no_pose_handles().as_deref(),
        Some("\"left_arm/forearm\" is fixed: its limits say it neither turns nor moves")
    );
    // A part with no limits turns every way.
    studio.select(Some("body"));
    assert_eq!(
        studio.grips(),
        [Grip::Ring(0), Grip::Ring(1), Grip::Ring(2)]
    );
    // Move gives arrows, except to a fixed part.
    studio.pose_tool = PoseTool::Move;
    assert_eq!(
        studio.grips(),
        [Grip::Arrow(0), Grip::Arrow(1), Grip::Arrow(2)]
    );
    studio.select(Some("panel"));
    assert!(studio.grips().is_empty());
    assert_eq!(
        studio.why_no_pose_handles().as_deref(),
        Some("\"panel\" is fixed: its limits say it neither turns nor moves")
    );
    // Nothing selected, no rings.
    studio.select(None);
    assert!(studio.grips().is_empty());
}

#[test]
fn a_ring_turns_the_part_the_way_the_pointer_goes_and_writes_one_set_pose() {
    let (recipe, mut studio) = robot("pose_mode_turn");
    studio.choose_pose("wave");
    studio.select(Some("head"));
    let before = file(&recipe);
    // Where the head's left eye is now, in the wave (head turned -15
    // around y).
    let eye = socket_now(studio.shown_model().unwrap(), "head", "left_eye");
    let centre = join_seen(&studio, "head") - studio.offset();
    // Turn the y ring 30 degrees round, the way `around` counts.
    let axis = ring_axis(&studio, 1);
    turn_ring(&mut studio, 1, axis, 30.0, false);
    let label = studio.drag_label().unwrap().to_owned();
    // While held, the picture shows the turn and nothing is written.
    assert_eq!(file(&recipe), before);
    let held = socket_now(studio.shown_model().unwrap(), "head", "left_eye");
    let wanted = centre + Mat3::around(axis, 30.0) * (eye - centre);
    assert!(
        (held - wanted).length() < 1e-6,
        "{held:?} against {wanted:?} ({label})"
    );
    // Let go: one line of the pose changes.
    let said = studio.release().unwrap().unwrap();
    let after = file(&recipe);
    let new = added(&before, &after);
    assert_eq!(new.len(), 1, "{new:?}");
    assert!(
        new[0] == "head = [0, 15, 0]" || new[0] == "head = [0, -45, 0]",
        "{new:?}"
    );
    let value = if new[0] == "head = [0, 15, 0]" {
        15
    } else {
        -45
    };
    assert_eq!(label, format!("y {value}°"));
    assert!(said.starts_with("Turned \"head\""), "{said}");
    // The model shown is the pose as written now: the eye is where it
    // was dragged to.
    assert_eq!(studio.pose(), "wave");
    let now = socket_now(studio.shown_model().unwrap(), "head", "left_eye");
    assert!((now - wanted).length() < 1e-6);
    // One step to undo.
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
}

#[test]
fn a_turn_snaps_to_five_degrees_and_stops_at_the_limit() {
    let (recipe, mut studio) = robot("pose_mode_limit");
    studio.choose_pose("wave");
    studio.select(Some("head"));
    let before = file(&recipe);
    // Nodding is limited to 30 each way: a turn of 80 stops there.
    let axis = ring_axis(&studio, 0);
    turn_ring(&mut studio, 0, axis, 80.0, false);
    let label = studio.drag_label().unwrap().to_owned();
    assert!(label.ends_with("(its limit)"), "{label}");
    studio.release().unwrap().unwrap();
    let line = added(&before, &file(&recipe))[0].to_owned();
    assert!(
        line == "head = [30, -15, 0]" || line == "head = [-30, -15, 0]",
        "{line}"
    );
    assert!(studio.problems().is_empty(), "{:?}", studio.problems());
    // A small turn snaps to 5 degrees; with Ctrl it doesn't.
    studio.undo().unwrap();
    turn_ring(&mut studio, 0, axis, 7.0, false);
    let snapped = studio.drag_label().unwrap().to_owned();
    assert!(snapped == "x 5°" || snapped == "x -5°", "{snapped}");
    studio.cancel_drag();
    turn_ring(&mut studio, 0, axis, 7.0, true);
    let free = studio.drag_label().unwrap().to_owned();
    assert!(free == "x 7°" || free == "x -7°", "{free}");
    // Escape lets go without writing.
    assert!(studio.escape());
    assert_eq!(file(&recipe), before);
}

#[test]
fn a_drag_in_rest_starts_a_new_pose() {
    let (recipe, mut studio) = robot("pose_mode_new_from_rest");
    studio.select(Some("left_arm"));
    let before = file(&recipe);
    let axis = ring_axis(&studio, 0);
    turn_ring(&mut studio, 0, axis, 45.0, false);
    let said = studio.release().unwrap().unwrap();
    assert!(said.starts_with("Made the pose \"pose\""), "{said}");
    assert_eq!(studio.pose(), "pose");
    let after = file(&recipe);
    let new = added(&before, &after);
    assert_eq!(new[0], "[poses.pose]");
    assert!(new[1].starts_with("left_arm = ["), "{new:?}");
    // The model itself is as it was: rest is rest.
    let mut fresh = Studio::open(&recipe, false);
    fresh.pose_mode().unwrap();
    assert_eq!(
        fresh.shown_model().unwrap().faces,
        Studio::open(examples().join("robot.toml"), false)
            .model()
            .unwrap()
            .faces
    );
}

#[test]
fn the_list_of_poses_makes_copies_mirrors_renames_and_removes() {
    let (recipe, mut studio) = robot("pose_mode_list");
    let before = file(&recipe);
    // New: an empty pose, chosen at once.
    studio.new_pose().unwrap();
    assert_eq!(studio.pose(), "pose");
    assert!(file(&recipe).ends_with("[poses.pose]\n") || file(&recipe).contains("[poses.pose]\n"));
    studio.undo().unwrap();
    assert_eq!(file(&recipe), before);
    // Copy.
    studio.choose_pose("wave");
    studio.copy_pose().unwrap();
    assert_eq!(studio.pose(), "wave_copy");
    assert_eq!(studio.posed_parts(), ["left_arm", "left_arm/elbow", "head"]);
    // Rename.
    studio.rename_pose("wave_big").unwrap();
    assert_eq!(studio.pose(), "wave_big");
    assert!(file(&recipe).contains("[poses.wave_big]"));
    // Mirror: a pose named for one side makes the other.
    studio.choose_pose("step_left");
    let said = studio.mirror_pose().unwrap();
    assert!(said.contains("step_right"), "{said}");
    assert_eq!(studio.pose(), "step_right");
    // Any other is mirrored in place: its left arm becomes its right.
    studio.choose_pose("wave_big");
    studio.mirror_pose().unwrap();
    assert_eq!(studio.pose(), "wave_big");
    assert_eq!(
        studio.posed_parts(),
        ["right_arm", "right_arm/elbow", "head"]
    );
    // Remove: a pose that keys use is refused, and says which.
    studio.choose_pose("wave");
    assert_eq!(studio.keys_using("wave"), ["wave"]);
    let refused = studio.remove_pose(false).unwrap_err();
    assert!(
        refused.contains("is used by the animation \"wave\""),
        "{refused}"
    );
    // With its keys it goes, and rest is chosen.
    studio.remove_pose(true).unwrap();
    assert_eq!(studio.pose(), "rest");
    assert!(!file(&recipe).contains("[poses.wave]"));
    // Mirroring rest makes no sense.
    assert!(studio.mirror_pose().is_err());
}

#[test]
fn move_slides_a_part_and_the_numbers_can_be_typed() {
    let (recipe, mut studio) = robot("pose_mode_move");
    studio.choose_pose("step_left");
    studio.select(Some("body"));
    studio.pose_tool = PoseTool::Move;
    let before = file(&recipe);
    // The up arrow, pulled 10 cm down.
    let grip = Grip::Arrow(1);
    let from = studio.grip_at(grip).unwrap();
    assert_eq!(studio.grab(VIEW, at_screen(&studio, from)), Some(grip));
    studio.drag(VIEW, at_screen(&studio, from - Vec3::UP * 0.1), false);
    assert_eq!(studio.drag_label(), Some("y -0.14 m"));
    studio.release().unwrap().unwrap();
    assert_eq!(
        added(&before, &file(&recipe)),
        ["body = { move = [0, -0.14, 0] }"]
    );
    // Typed in the panel: a turn and a move together.
    studio
        .set_pose_numbers("head", [10.0, 20.0, 0.0], [0.0, 0.0, 0.0])
        .unwrap();
    assert!(file(&recipe).contains("head = [10, 20, 0]\n"));
    assert_eq!(studio.pose_line("head"), ([10.0, 20.0, 0.0], [0.0; 3]));
    // Back to rest takes the line away.
    studio.pose_back_to_rest("head").unwrap();
    assert!(!file(&recipe).contains("head = [10, 20, 0]"));
    assert_eq!(studio.pose_line("head"), ([0.0; 3], [0.0; 3]));
}

#[test]
fn a_pose_that_leaves_a_part_floating_shows_in_problems() {
    let (_, mut studio) = robot("pose_mode_problems");
    studio.choose_pose("wave");
    assert!(studio.problems().is_empty(), "{:?}", studio.problems());
    // A foot pulled off its leg. (A head lifted off its neck doesn't
    // float: its eyes and antenna still touch it.)
    studio
        .set_pose_numbers("left_leg/foot", [0.0; 3], [0.0, 0.3, 0.4])
        .unwrap();
    let problems = studio.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0]
            .text
            .starts_with("in the pose \"wave\", \"left_leg/foot\" floats"),
        "{problems:?}"
    );
    assert_eq!(problems[0].part.as_deref(), Some("left_leg/foot"));
    // In another pose it isn't said.
    studio.choose_pose("rest");
    assert!(studio.problems().is_empty());
    // A foot pushed into the ground leaves the rest of the robot above
    // it, away from what stands on the ground: one line says so.
    studio.choose_pose("wave");
    studio
        .set_pose_numbers("left_leg/foot", [0.0; 3], [0.0, -0.3, 0.0])
        .unwrap();
    let problems = studio.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0]
            .text
            .contains("parts touch nothing that reaches the ground: body, panel, head and"),
        "{problems:?}"
    );
}

#[test]
fn the_limits_are_set_from_the_panel() {
    let (recipe, mut studio) = robot("pose_mode_limits");
    assert_eq!(
        studio.limits_of("head"),
        Some(duck_mold_core::recipe::Limits::Turns([
            duck_mold_core::recipe::Turns::Between(-30.0, 30.0),
            duck_mold_core::recipe::Turns::Between(-80.0, 80.0),
            duck_mold_core::recipe::Turns::No,
        ]))
    );
    studio
        .set_limits_of(
            "head",
            duck_mold_core::edit::LimitsTo::Turns([
                Some(duck_mold_core::recipe::Turns::Between(-45.0, 45.0)),
                None,
                None,
            ]),
        )
        .unwrap();
    assert!(file(&recipe).contains("limits = { x = [-45, 45], y = [-80, 80] }"));
}

/// Compares a picture with the stored one, byte for byte, or stores it
/// when `DUCK_MOLD_ACCEPT` is set.
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
            "{} is missing. Create it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test pose_mode",
            path.display()
        )
    });
    if stored != found {
        let new = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.new.png"));
        std::fs::write(&new, &found).unwrap();
        panic!(
            "the picture {name} changed (the new one is in {}). If the change is meant, accept it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-view --test pose_mode",
            new.display()
        );
    }
}

#[test]
fn pose_mode_is_drawn_with_the_rings_at_the_join() {
    let (_, mut studio) = robot("pose_mode_picture");
    studio.choose_pose("wave");
    studio.select(Some("head"));
    studio.frame();
    check_picture("studio-pose-rings", &studio.picture(VIEW.0, VIEW.1));
}
