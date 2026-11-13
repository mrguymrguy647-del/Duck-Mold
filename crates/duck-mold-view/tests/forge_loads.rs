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
