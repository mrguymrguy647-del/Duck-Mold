//! Phase 5, step 5: animations and poses in the `.glb`, and the reader
//! written for Duck Forge playing them.
//!
//! The reader shares no code with Duck Mold. So the strongest check
//! there is: pose the model with Duck Mold, pose it with the reader from
//! what the file holds, and see that every corner of every part is in
//! the same place.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::glb::write_glb;
use duck_mold_core::load::load_text;
use duck_mold_core::math::Vec3;
use duck_mold_core::model::Model;
use duck_mold_core::motion::{Joint, Motion, frame_times, posed};
use duck_mold_core::source::Syntax;

/// The reader written for Duck Forge, exactly as it will be copied there.
#[path = "../../../docs/forge/duck_mold_glb.rs"]
#[allow(dead_code)]
mod forge;

use forge::{GlbModel, GlbPlace};

fn built(text: &str) -> Model {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    let errors: Vec<_> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.unwrap()
}

/// A puppet: a body, a head with a hat socket, and one arm (an upper
/// and a lower part) used twice, the right one a mirror image, and a
/// tail on a body that is turned, so that some parts' turns aren't
/// measured in the model's own directions.
const PUPPET: &str = r##"version = 0
name = "puppet"

[palette]
wood = "#B08050"
red = "#D0453A"

[define.arm]
color = "wood"

[define.arm.sockets]
shoulder = { from = "upper.joint" }
grip = { from = "lower.bottom" }

[[define.arm.parts]]
name = "upper"
shape = "box"
size = [0.1, 0.3, 0.1]
sockets = { joint = { on = "right", shift = [0, 0.8, 0] } }

[[define.arm.parts]]
name = "lower"
shape = "box"
size = [0.08, 0.3, 0.08]
color = "red"
attach = "top"
to = "upper.bottom"

[define.tail]
[[define.tail.parts]]
name = "root"
shape = "box"
size = [0.06, 0.06, 0.2]
color = "wood"

[[define.tail.parts]]
name = "tip"
shape = "box"
size = [0.04, 0.04, 0.2]
color = "red"
attach = "front"
to = "root.back"

[[parts]]
name = "body"
shape = "box"
size = [0.4, 0.6, 0.2]
color = "wood"

[parts.sockets]
neck = { on = "top" }
left_arm = { on = "left", shift = [0, 0.8, 0] }
right_arm = { on = "right", shift = [0, 0.8, 0] }

[[parts]]
name = "head"
shape = "box"
size = [0.25, 0.25, 0.25]
color = "wood"
attach = "bottom"
to = "body.neck"
sockets = { hat = { on = "top" } }

[[parts]]
name = "left_arm"
use = "arm"
attach = "shoulder"
to = "body.left_arm"

[[parts]]
name = "right_arm"
use = "arm"
mirror = "x"
attach = "shoulder"
to = "body.right_arm"

[[parts]]
name = "tail"
use = "tail"
move = [0, 0.2, -0.3]
rotate = [30, 40, 0]

[poses.wave]
left_arm = [0, 0, -150]
"left_arm/lower" = [0, 0, -30]
head = [0, -15, 0]

[poses.wave_out]
left_arm = [0, 0, -120]
"left_arm/lower" = [0, 0, -10]
head = [0, -15, 0]

[poses.crouch]
body = { move = [0, -0.1, 0] }
head = { turn = [20, 0, 0], move = [0, 0, 0.02] }
right_arm = [40, 0, 20]
"right_arm/lower" = [-60, 10, 0]

[poses.wag]
"tail/root" = [0, 35, 0]
"tail/tip" = [10, 35, 0]
tail = { move = [0, 0.05, 0] }

[poses.round]
head = [0, 360, 0]

[animations.hello]
keys = [
  { at = 0, pose = "rest" },
  { at = 0.3, pose = "wave" },
  { at = 0.6, pose = "wave_out" },
  { at = 0.9, pose = "wave" },
  { at = 1.25, pose = "rest" },
]

[animations.bob]
loop = true
keys = [
  { at = 0, pose = "rest" },
  { at = 0.4, pose = "crouch" },
  { at = 0.8, pose = "rest" },
]

[animations.wagging]
loop = true
smooth = false
keys = [
  { at = 0, pose = "rest" },
  { at = 0.2, pose = "wag" },
  { at = 0.4, pose = "rest" },
]

[animations.spin]
loop = true
smooth = false
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "round" },
]

[animations.still]
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "rest" },
]

[animations.held]
keys = [{ at = 0, pose = "wave" }]
"##;

fn read(model: &Model) -> GlbModel {
    forge::read_duck_mold_glb(&write_glb(model)).unwrap()
}

fn join(read: &GlbModel, name: &str) -> usize {
    read.joins
        .iter()
        .position(|join| join.name == name)
        .unwrap_or_else(|| panic!("no join {name}"))
}

/// One place for every join: those named, and the rest at rest.
fn places(read: &GlbModel, named: &[(usize, GlbPlace)]) -> Vec<GlbPlace> {
    let mut all = vec![GlbPlace::REST; read.joins.len()];
    for (join, place) in named {
        all[*join] = *place;
    }
    all
}

fn far(a: [f32; 3], b: Vec3) -> f64 {
    let d = [
        f64::from(a[0]) - b.x,
        f64::from(a[1]) - b.y,
        f64::from(a[2]) - b.z,
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// How far apart the reader's posed model and Duck Mold's are, at the
/// worst corner: every corner of every part of `mold` (posed by Duck
/// Mold) against the reader's corner that is in the same place at rest,
/// moved by the reader.
fn worst_gap(rest: &Model, mold: &Model, read: &GlbModel, moved: &[[f32; 3]]) -> f64 {
    let mut worst: f64 = 0.0;
    let mut checked = 0;
    for part in rest.leaves() {
        let join = &read.joins[join(read, &part.path)];
        let own = join.first_vertex..join.first_vertex + join.vertices;
        for (face, posed_face) in rest.faces[part.faces.clone()]
            .iter()
            .zip(&mold.faces[part.faces.clone()])
        {
            for (corner, posed_corner) in face.corners.iter().zip(&posed_face.corners) {
                // The reader's corner that is here at rest...
                let vertex = own
                    .clone()
                    .find(|&vertex| far(read.vertices[vertex], *corner) < 1e-5)
                    .unwrap_or_else(|| panic!("{} has no corner at {corner:?}", part.path));
                // ...is where Duck Mold put it, when posed.
                worst = worst.max(far(moved[vertex], *posed_corner));
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} corners were compared");
    worst
}

#[test]
fn the_file_names_every_animation_and_pose() {
    let model = built(PUPPET);
    let read = read(&model);
    let names: Vec<(&str, bool, f32)> = read
        .animations
        .iter()
        .map(|animation| (animation.name.as_str(), animation.looping, animation.length))
        .collect();
    assert_eq!(
        names,
        [
            ("hello", false, 1.25),
            ("bob", true, 0.8),
            ("wagging", true, 0.4),
            ("spin", true, 1.0),
            ("still", false, 1.0),
            ("held", false, 0.0),
        ]
    );
    let poses: Vec<&str> = read.poses.iter().map(|pose| pose.name.as_str()).collect();
    assert_eq!(poses, ["wave", "wave_out", "crouch", "wag", "round"]);
    // Thirty samples a second, and the very end.
    let hello = &read.animations[0];
    let times = frame_times(1.25);
    assert_eq!(hello.times.len(), times.len());
    for (read, written) in hello.times.iter().zip(&times) {
        assert_eq!(*read, *written as f32);
    }
    assert_eq!(*hello.times.last().unwrap(), 1.25);
    // Only what moves has a track, and only what it does: the wave
    // turns three parts and moves none.
    let moved: Vec<&str> = hello
        .tracks
        .iter()
        .map(|track| read.joins[track.join].name.as_str())
        .collect();
    assert_eq!(moved, ["head", "left_arm", "left_arm/lower"]);
    for track in &hello.tracks {
        assert_eq!(track.turns.len(), times.len());
        assert!(track.moves.is_empty());
    }
    // The crouch moves the body without turning it.
    let bob = &read.animations[1];
    let body = bob
        .tracks
        .iter()
        .find(|track| read.joins[track.join].name == "body")
        .unwrap();
    assert!(body.turns.is_empty());
    assert_eq!(body.moves.len(), bob.times.len());
    // One that moves nothing is known by its name and length, and one
    // key is one sample.
    assert!(read.animations[4].times.is_empty() && read.animations[4].tracks.is_empty());
    assert_eq!(read.animations[5].times, [0.0]);
}

#[test]
fn a_model_that_does_not_move_has_nothing_about_motion_in_its_file() {
    let still = PUPPET[..PUPPET.find("[poses.wave]").unwrap()].to_owned();
    let bytes = write_glb(&built(&still));
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("\"animations\""));
    assert!(!text.contains("\"poses\""));
    assert!(!text.contains("\"samplers\""));
    let read = forge::read_duck_mold_glb(&bytes).unwrap();
    assert!(read.animations.is_empty() && read.poses.is_empty());
    // The model itself is the same, moving or not.
    let moving = read_parts(&built(PUPPET));
    assert_eq!(read.vertices, moving.vertices);
    assert_eq!(read.triangles, moving.triangles);
    assert_eq!(read.joins, moving.joins);
    assert_eq!(read.sockets, moving.sockets);
}

fn read_parts(model: &Model) -> GlbModel {
    read(model)
}

#[test]
fn at_rest_the_reader_gives_the_model_exactly() {
    let model = built(PUPPET);
    let read = read(&model);
    let rest = vec![GlbPlace::REST; read.joins.len()];
    assert_eq!(read.posed(&rest), read.vertices);
    assert!(read.stands(&rest).iter().all(Option::is_none));
    // Too few places: the rest are at rest.
    assert_eq!(read.posed(&[]), read.vertices);
}

#[test]
fn the_reader_holds_every_pose_as_duck_mold_does() {
    let model = built(PUPPET);
    let read = read(&model);
    for pose in &model.poses {
        let mold = posed(&model, &pose.joints);
        let held = read
            .poses
            .iter()
            .find(|held| held.name == pose.name)
            .unwrap();
        let moved = read.posed(&places(&read, &held.joints));
        let gap = worst_gap(&model, &mold, &read, &moved);
        assert!(gap < 2e-6, "the pose {}: {gap} apart", pose.name);
    }
}

#[test]
fn the_reader_plays_every_animation_as_duck_mold_does() {
    let model = built(PUPPET);
    let read = read(&model);
    for animation in &model.animations {
        let moving = Motion::of(&model, animation);
        let played = read
            .animations
            .iter()
            .find(|played| played.name == animation.name)
            .unwrap();
        // At every sample, to the last digits a file keeps.
        for at in frame_times(animation.length()) {
            let mold = posed(&model, &moving.joints_at(at));
            let moved = read.posed(&places(&read, &played.at(at as f32)));
            let gap = worst_gap(&model, &mold, &read, &moved);
            assert!(gap < 3e-6, "{} at {at}: {gap} apart", animation.name);
        }
        // Between two samples the reader goes straight where Duck Mold
        // follows its curve. They differ most where a motion starts
        // from a stop: about a degree, which at the end of an arm is a
        // centimetre or so, for a thirtieth of a second.
        let length = animation.length();
        for step in 0..40 {
            let at = length * (f64::from(step) + 0.37) / 40.0;
            let mold = posed(&model, &moving.joints_at(at));
            let moved = read.posed(&places(&read, &played.at(at as f32)));
            let gap = worst_gap(&model, &mold, &read, &moved);
            assert!(gap < 0.02, "{} at {at}: {gap} apart", animation.name);
        }
        // Before the start and after the end it stands as at the ends.
        assert_eq!(played.at(-1.0), played.at(0.0));
        assert_eq!(played.at(99.0), played.at(played.length));
    }
}

#[test]
fn a_full_turn_goes_all_the_way_round_in_the_file() {
    // The head turns once round in a second. Half way it faces
    // backwards: the file's samples are close enough together that
    // going "the short way" between two of them is the right way.
    let model = built(PUPPET);
    let read = read(&model);
    let spin = read
        .animations
        .iter()
        .find(|animation| animation.name == "spin")
        .unwrap();
    let head = join(&read, "head");
    let hat = read
        .sockets
        .iter()
        .find(|socket| socket.name == "head.hat")
        .unwrap();
    assert_eq!(hat.join, Some(head));
    let front = [0.0, 0.0, 1.0];
    for (at, facing) in [
        (0.0, [0.0, 0.0, 1.0]),
        (0.25, [1.0, 0.0, 0.0]),
        (0.5, [0.0, 0.0, -1.0]),
        (0.75, [-1.0, 0.0, 0.0]),
        (1.0, [0.0, 0.0, 1.0]),
    ] {
        let stands = read.stands(&places(&read, &spin.at(at)));
        let now = stands[head].map_or(front, |stand| stand.direction(front));
        for axis in 0..3 {
            assert!(
                (now[axis] - facing[axis]).abs() < 1e-5,
                "at {at}: {now:?}, not {facing:?}"
            );
        }
    }
}

#[test]
fn sockets_go_with_the_parts_that_carry_them() {
    let model = built(PUPPET);
    let read = read(&model);
    for pose in &model.poses {
        let mold = posed(&model, &pose.joints);
        let held = read
            .poses
            .iter()
            .find(|held| held.name == pose.name)
            .unwrap();
        let stands = read.stands(&places(&read, &held.joints));
        let named = mold.named_sockets();
        assert!(named.len() > 20);
        for socket in &read.sockets {
            let (at, facing) = match socket.join.and_then(|join| stands[join]) {
                Some(stand) => (stand.point(socket.at), stand.direction(socket.facing)),
                None => (socket.at, socket.facing),
            };
            let there = named
                .iter()
                .find(|named| named.name == socket.name)
                .unwrap_or_else(|| panic!("no socket {}", socket.name));
            assert!(
                far(at, there.at) < 2e-6,
                "{} in {}: {at:?}, not {:?}",
                socket.name,
                pose.name,
                there.at
            );
            assert!(
                far(facing, there.facing) < 2e-6,
                "{} in {} faces {facing:?}, not {:?}",
                socket.name,
                pose.name,
                there.facing
            );
        }
    }
    // An arm's `grip` comes from its lower part, and goes with it.
    let grip = read
        .sockets
        .iter()
        .find(|socket| socket.name == "left_arm.grip")
        .unwrap();
    assert_eq!(grip.join, Some(join(&read, "left_arm/lower")));
}

#[test]
fn a_turn_in_degrees_is_the_turn_a_pose_writes() {
    // Game code turns a join by degrees. That must be the turn a pose
    // with the same numbers makes, for a part of the model's own list,
    // a part of a mirror image, and a part inside one that is turned.
    let model = built(PUPPET);
    let read = read(&model);
    for (part, degrees) in [
        ("head", [20.0, -35.0, 10.0]),
        ("left_arm", [0.0, 0.0, -150.0]),
        ("left_arm/lower", [-40.0, 15.0, 30.0]),
        ("right_arm", [40.0, 0.0, 20.0]),
        ("right_arm/lower", [-60.0, 10.0, 5.0]),
        ("tail", [0.0, 90.0, 0.0]),
        ("tail/root", [15.0, 35.0, -20.0]),
        ("tail/tip", [200.0, -170.0, 95.0]),
    ] {
        let mold = posed(&model, &[Joint::turned(part, degrees)]);
        let at = join(&read, part);
        let turn = read.turn_by(at, degrees.map(|angle| angle as f32));
        let moved = read.posed(&places(
            &read,
            &[(
                at,
                GlbPlace {
                    turn,
                    moved: [0.0; 3],
                },
            )],
        ));
        let gap = worst_gap(&model, &mold, &read, &moved);
        assert!(gap < 3e-6, "{part} turned {degrees:?}: {gap} apart");
    }
    // The parts of the model's own list turn in the model's own
    // directions; the others say theirs.
    let own = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    assert_eq!(read.joins[join(&read, "head")].turns_in, own);
    assert_eq!(read.joins[join(&read, "right_arm")].turns_in, own);
    assert_eq!(
        read.joins[join(&read, "right_arm/lower")].turns_in,
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
    assert_ne!(read.joins[join(&read, "tail/tip")].turns_in, own);
    // No turn is no turn.
    assert_eq!(read.turn_by(0, [0.0; 3]), GlbPlace::REST.turn);
    assert!(
        GlbPlace {
            turn: read.turn_by(0, [0.0; 3]),
            moved: [0.0; 3]
        }
        .is_rest()
    );
}

#[test]
fn part_way_between_two_places_is_between_them() {
    let model = built(PUPPET);
    let read = read(&model);
    let head = join(&read, "head");
    let a = GlbPlace {
        turn: read.turn_by(head, [0.0, 0.0, 0.0]),
        moved: [0.0, 0.0, 0.0],
    };
    let b = GlbPlace {
        turn: read.turn_by(head, [0.0, 90.0, 0.0]),
        moved: [0.0, 0.2, 0.0],
    };
    assert_eq!(a.towards(&b, 0.0), a);
    assert_eq!(a.towards(&b, 1.0), b);
    assert_eq!(a.towards(&b, -3.0), a);
    assert_eq!(a.towards(&b, 7.0), b);
    let half = a.towards(&b, 0.5);
    let straight = read.turn_by(head, [0.0, 45.0, 0.0]);
    for (now, wanted) in half.turn.iter().zip(straight) {
        assert!((now - wanted).abs() < 1e-6, "{:?} {straight:?}", half.turn);
    }
    assert_eq!(half.moved, [0.0, 0.1, 0.0]);
    // The same turn written the other way (every sign changed) is the
    // same turn: going towards it goes nowhere.
    let other_way = GlbPlace {
        turn: b.turn.map(|number| -number),
        moved: b.moved,
    };
    let stays = b.towards(&other_way, 0.5);
    for (now, wanted) in stays.turn.iter().zip(b.turn) {
        assert!((now.abs() - wanted.abs()).abs() < 1e-6);
    }
}

#[test]
fn a_damaged_animation_is_refused_with_a_sentence() {
    let bytes = write_glb(&built(PUPPET));
    let change = |from: &str, to: &str| {
        let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json = std::str::from_utf8(&bytes[20..20 + json_length]).unwrap();
        assert!(json.contains(from), "{from}");
        let mut json = json.replacen(from, to, 1).into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let bin = &bytes[20 + json_length..];
        let mut glb = Vec::new();
        glb.extend(b"glTF");
        glb.extend(2u32.to_le_bytes());
        glb.extend(((12 + 8 + json.len() + bin.len()) as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(b"JSON");
        glb.extend(json);
        glb.extend(bin);
        glb
    };
    for (from, to, said) in [
        (
            "\"path\":\"rotation\"",
            "\"path\":\"scale\"",
            "the animation \"hello\" changes something other than a turn or a place",
        ),
        (
            "\"interpolation\":\"LINEAR\"",
            "\"interpolation\":\"STEP\"",
            "the animation \"hello\" isn't made of straight steps, which is all Duck Mold writes",
        ),
        (
            "\"target\":{\"node\":2,",
            "\"target\":{\"node\":0,",
            "the animation \"hello\" moves an object that isn't a part of the model",
        ),
        (
            "\"joints\":[{\"node\":",
            "\"joints\":[{\"node\":9",
            "the pose \"wave\" moves an object that isn't a part of the model",
        ),
    ] {
        let error = forge::read_duck_mold_glb(&change(from, to)).unwrap_err();
        assert_eq!(error, said);
    }
}
