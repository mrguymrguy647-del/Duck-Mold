//! The Duck Forge that Duck Mold is pinned to loads every example model
//! with its own loader (`duckforge::glb`), the code games use: the same
//! triangles, palette and sockets that `duck-mold inspect` reports.

use std::path::Path;

use duckforge::glb::GlbModel;
use duckforge::prelude::Mesh;

const EXAMPLES: [&str; 18] = [
    "crate",
    "table",
    "tree",
    "house",
    "robot",
    "snowman",
    "desk",
    "duck",
    "dog",
    "doughnut",
    "mug",
    "temple",
    "barn",
    "slide",
    "gate",
    "chair",
    "window_wall",
    "rock",
];

#[test]
fn duck_forge_loads_every_golden_model() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    for name in EXAMPLES {
        let model = GlbModel::load(golden.join(format!("{name}.glb")))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let report: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(golden.join(format!("{name}.inspect.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(model.name, name);
        assert_eq!(
            model.triangles.len() as u64,
            report["triangles"].as_u64().unwrap(),
            "{name}"
        );
        let palette: Vec<&str> = report["palette"]
            .as_array()
            .unwrap()
            .iter()
            .map(|color| color["name"].as_str().unwrap())
            .collect();
        let loaded: Vec<&str> = model.palette.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(loaded, palette, "{name}");
        // Every model has the seven sockets of its own, and Duck Forge
        // finds them by name.
        assert!(model.socket("top").is_some(), "{name}");
        assert!(model.sockets.len() >= 7, "{name}");
        let mesh: Mesh = Mesh::load_glb(golden.join(format!("{name}.glb"))).unwrap();
        assert_eq!(mesh.triangles.len(), model.triangles.len(), "{name}");
    }
    // The robot's head has a top, where Duck Forge's turntable puts a hat.
    let robot = GlbModel::load(golden.join("robot.glb")).unwrap();
    assert_eq!(robot.sockets.len(), 187);
    assert!(robot.socket("head.top").is_some());
}

/// The robot, built from its recipe and loaded by Duck Forge from its
/// golden file: Duck Forge's own player (`AnimatedModel`, the code a
/// game uses) puts it where Duck Mold does, all through every one of its
/// animations.
#[test]
fn duck_forge_plays_the_robots_animations_as_duck_mold_poses_them() {
    use duck_mold_core::motion::{Motion, frame_times, posed};
    use duckforge::glb::AnimatedModel;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let built = duck_mold_core::tools::build(&root.join("examples/robot.toml"))
        .unwrap_or_else(|_| panic!("the robot builds"));
    let model = built.model;
    assert_eq!(model.animations.len(), 3);
    for animation in &model.animations {
        let mut robot = AnimatedModel::load(root.join("tests/golden/robot.glb")).unwrap();
        assert!(robot.animations().contains(&animation.name.as_str()));
        robot.set_blend(0.0);
        robot.play(&animation.name);
        let moving = Motion::of(&model, animation);
        let mut before = 0.0;
        for at in frame_times(animation.length()) {
            robot.update((at - before) as f32);
            before = at;
            let here = posed(&model, &moving.joints_at(at));
            // The box around it...
            let bounds = here.bounds().unwrap();
            let (low, high) = robot.mesh().bounds();
            for (found, wanted) in [
                (low.x, bounds.min.x),
                (low.y, bounds.min.y),
                (low.z, bounds.min.z),
                (high.x, bounds.max.x),
                (high.y, bounds.max.y),
                (high.z, bounds.max.z),
            ] {
                assert!(
                    (f64::from(found) - wanted).abs() < 1e-3,
                    "{} at {at}: {found} against {wanted}",
                    animation.name
                );
            }
            // ...and where a hat would sit, and a sword be held.
            let sockets = here.named_sockets();
            for name in ["head.top", "left_arm.grip", "right_leg/foot.bottom"] {
                let wanted = sockets.iter().find(|socket| socket.name == name).unwrap();
                let found = robot.socket(name).unwrap().position();
                let apart = ((f64::from(found.x) - wanted.at.x).powi(2)
                    + (f64::from(found.y) - wanted.at.y).powi(2)
                    + (f64::from(found.z) - wanted.at.z).powi(2))
                .sqrt();
                assert!(
                    apart < 1e-3,
                    "{name} in {} at {at}: {apart}",
                    animation.name
                );
            }
        }
        // It loops, or stops at its end, as the recipe says.
        robot.update(0.5);
        assert_eq!(robot.is_playing(), animation.looping, "{}", animation.name);
        assert_eq!(robot.finished(), !animation.looping, "{}", animation.name);
    }
    // And its poses are there to hold, by the recipe's names.
    let mut robot = AnimatedModel::load(root.join("tests/golden/robot.glb")).unwrap();
    let names: Vec<&str> = model.poses.iter().map(|pose| pose.name.as_str()).collect();
    assert_eq!(robot.poses(), names);
    robot.set_blend(0.0);
    robot.set_pose("wave");
    let here = posed(&model, &model.pose("wave").unwrap().joints);
    let (_, high) = robot.mesh().bounds();
    assert!((f64::from(high.y) - here.bounds().unwrap().max.y).abs() < 1e-4);
}
