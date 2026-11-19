//! The Duck Forge that Duck Mold is pinned to loads every example model
//! with its own loader (`duckforge::glb`), the code games use: the same
//! triangles, palette and sockets that `duck-mold inspect` reports.

use std::path::Path;

use duckforge::glb::GlbModel;
use duckforge::prelude::Mesh;

const EXAMPLES: [&str; 24] = [
    "crate",
    "plank_crate",
    "signpost",
    "pip",
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
    "door",
    "cart",
    "gnome",
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

/// The examples that move, each with sockets on parts that move.
const MOVING: [(&str, &[&str]); 4] = [
    (
        "robot",
        &["head.top", "left_arm.grip", "right_leg/foot.bottom"],
    ),
    ("door", &["handle.front", "door.front"]),
    ("cart", &["wheel_front_left/spoke.bottom"]),
    (
        "gnome",
        &["hat.top", "left_arm/hand.bottom", "right_leg/foot.bottom"],
    ),
];

/// Every example that moves, built from its recipe and loaded by Duck
/// Forge from its golden file: Duck Forge's own player (`AnimatedModel`,
/// the code a game uses) puts it where Duck Mold does, all through every
/// one of its animations.
#[test]
fn duck_forge_plays_every_animation_as_duck_mold_poses_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, watched) in MOVING {
        plays_as_duck_mold_poses_it(&root, name, watched);
    }
    robots_poses_hold(&root);
}

/// One example, played by Duck Forge, against Duck Mold's own posing.
fn plays_as_duck_mold_poses_it(root: &Path, name: &str, watched: &[&str]) {
    use duck_mold_core::motion::{Motion, frame_times, posed};
    use duckforge::glb::AnimatedModel;

    let built = duck_mold_core::tools::build(&root.join(format!("examples/{name}.toml")))
        .unwrap_or_else(|_| panic!("{name} builds"));
    let model = built.model;
    assert!(!model.animations.is_empty(), "{name}");
    let file = root.join(format!("tests/golden/{name}.glb"));
    for animation in &model.animations {
        let mut robot = AnimatedModel::load(&file).unwrap();
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
                    "{name} {} at {at}: {found} against {wanted}",
                    animation.name
                );
            }
            // ...and where a hat would sit, and a sword be held.
            let sockets = here.named_sockets();
            for socket in watched {
                let wanted = sockets
                    .iter()
                    .find(|known| known.name == *socket)
                    .unwrap_or_else(|| panic!("{name}: {socket}"));
                let found = robot.socket(socket).unwrap().position();
                let apart = ((f64::from(found.x) - wanted.at.x).powi(2)
                    + (f64::from(found.y) - wanted.at.y).powi(2)
                    + (f64::from(found.z) - wanted.at.z).powi(2))
                .sqrt();
                assert!(
                    apart < 1e-3,
                    "{name}: {socket} in {} at {at}: {apart}",
                    animation.name
                );
            }
        }
        // It loops, or stops at its end, as the recipe says.
        robot.update(0.5);
        assert_eq!(
            robot.is_playing(),
            animation.looping,
            "{name} {}",
            animation.name
        );
        assert_eq!(
            robot.finished(),
            !animation.looping,
            "{name} {}",
            animation.name
        );
    }
}

/// The robot's poses are there to hold, by the recipe's names.
fn robots_poses_hold(root: &Path) {
    use duck_mold_core::motion::posed;
    use duckforge::glb::AnimatedModel;

    let model = duck_mold_core::tools::build(&root.join("examples/robot.toml"))
        .unwrap_or_else(|_| panic!("the robot builds"))
        .model;
    assert_eq!(model.animations.len(), 4);
    let mut robot = AnimatedModel::load(root.join("tests/golden/robot.glb")).unwrap();
    let names: Vec<&str> = model.poses.iter().map(|pose| pose.name.as_str()).collect();
    assert_eq!(robot.poses(), names);
    robot.set_blend(0.0);
    robot.set_pose("wave");
    let here = posed(&model, &model.pose("wave").unwrap().joints);
    let (_, high) = robot.mesh().bounds();
    assert!((f64::from(high.y) - here.bounds().unwrap().max.y).abs() < 1e-4);
}

#[test]
fn duck_forge_loads_a_model_with_its_images() {
    use duck_mold_core::picture::read_png_up_to;

    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let model = GlbModel::load(golden.join("plank_crate.glb")).unwrap();
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(golden.join("plank_crate.inspect.json")).unwrap(),
    )
    .unwrap();
    // The one picture its images are on: the size the report says, and
    // a PNG Duck Mold reads back.
    let picture = model.picture.as_ref().expect("a picture");
    assert_eq!(
        serde_json::json!([picture.width, picture.height]),
        report["images"]["sheet"]
    );
    let pixels = read_png_up_to(&picture.png, "the picture", 4096).unwrap();
    assert_eq!(
        (pixels.width, pixels.height),
        (picture.width, picture.height)
    );
    // The mesh a game draws comes with it: every triangle of the crate
    // has a place on the picture.
    let mesh = Mesh::load_glb(golden.join("plank_crate.glb")).unwrap();
    let texture = mesh.texture.as_ref().expect("a texture");
    assert_eq!(texture.width() as u32, picture.width);
    assert_eq!(mesh.triangles.len(), 12);
    assert!((0..12).all(|at| mesh.uvs.get(at).is_some_and(Option::is_some)));
    // A model without images has none.
    let plain = Mesh::load_glb(golden.join("crate.glb")).unwrap();
    assert!(plain.texture.is_none());

    // The signpost, whose pictures came inside a glTF file: its sign's
    // triangles have a place on the picture, its mound and stone don't.
    let model = GlbModel::load(golden.join("signpost.glb")).unwrap();
    let mesh = model.to_mesh();
    assert!(mesh.texture.is_some());
    let placed = model
        .triangles
        .iter()
        .filter(|triangle| triangle.on_picture.is_some())
        .count();
    assert_eq!((placed, mesh.triangles.len()), (28, 96));
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(golden.join("signpost.inspect.json")).unwrap(),
    )
    .unwrap();
    let picture = model.picture.as_ref().unwrap();
    assert_eq!(
        serde_json::json!([picture.width, picture.height]),
        report["images"]["sheet"]
    );
}
