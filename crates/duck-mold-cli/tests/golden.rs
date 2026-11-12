//! Golden files: the five example recipes must always build to exactly
//! the same bytes.
//!
//! `tests/golden/` at the top of the repository holds, for each example,
//! the `.glb` that `duck-mold export` writes and the report that
//! `duck-mold inspect --json` prints. These tests run the real program
//! and compare every byte. That is what "the same recipe always gives
//! the same output" means in practice: on this computer, on another one,
//! and after any change to the code.
//!
//! When an example or the output is changed on purpose, accept the new
//! files with
//!
//! ```text
//! DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test golden
//! ```
//!
//! and say in the commit why they changed.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The reader written for Duck Forge, exactly as it will be copied there.
#[path = "../../../docs/forge/duck_mold_glb.rs"]
mod forge;

const EXAMPLES: [&str; 5] = ["crate", "table", "tree", "house", "robot"];

/// The top of the repository.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A folder for the files a test writes.
fn scratch(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Runs `duck-mold` from the top of the repository, so that the paths in
/// its output are the ones a person would type.
fn duck_mold(arguments: &[&str]) -> Vec<u8> {
    let output = Command::new(env!("CARGO_BIN_EXE_duck-mold"))
        .args(arguments)
        .current_dir(root())
        .output()
        .expect("the program runs");
    assert!(
        output.status.success(),
        "duck-mold {arguments:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// Builds an example and returns the bytes of its `.glb`.
fn export(recipe: &str, into: &Path) -> Vec<u8> {
    let output = into.join("out.glb");
    duck_mold(&["export", recipe, "-o", output.to_str().unwrap()]);
    std::fs::read(output).unwrap()
}

/// Compares what the program gave with the stored golden file, or stores
/// it when `DUCK_MOLD_ACCEPT` is set.
fn check(golden: &str, found: &[u8]) {
    let path = root().join("tests/golden").join(golden);
    if std::env::var_os("DUCK_MOLD_ACCEPT").is_some() {
        std::fs::write(&path, found).unwrap();
        return;
    }
    let expected = std::fs::read(&path).unwrap_or_else(|_| {
        panic!("tests/golden/{golden} is missing. Create it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test golden")
    });
    if expected != found {
        // Show where they first differ; for text, show both.
        let at = expected
            .iter()
            .zip(found)
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(found.len()));
        let detail = match (std::str::from_utf8(&expected), std::str::from_utf8(found)) {
            (Ok(expected), Ok(found)) => format!("\n--- expected\n{expected}\n--- found\n{found}"),
            _ => String::new(),
        };
        panic!(
            "tests/golden/{golden} no longer matches what duck-mold gives: {} bytes stored, {} found, first difference at byte {at}.\nIf the change is meant, accept it with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test golden{detail}",
            expected.len(),
            found.len()
        );
    }
}

#[test]
fn every_example_builds_to_its_golden_glb() {
    let scratch = scratch("golden_glb");
    for name in EXAMPLES {
        let bytes = export(&format!("examples/{name}.toml"), &scratch);
        check(&format!("{name}.glb"), &bytes);
    }
}

#[test]
fn every_example_inspects_to_its_golden_report() {
    for name in EXAMPLES {
        let report = duck_mold(&["inspect", &format!("examples/{name}.toml"), "--json"]);
        check(&format!("{name}.inspect.json"), &report);
    }
}

#[test]
fn building_twice_gives_the_same_bytes() {
    let scratch = scratch("golden_twice");
    for name in EXAMPLES {
        let recipe = format!("examples/{name}.toml");
        assert_eq!(
            export(&recipe, &scratch),
            export(&recipe, &scratch),
            "{name}"
        );
    }
}

#[test]
fn the_toml_and_json_crates_are_the_same_model() {
    let scratch = scratch("golden_twins");
    let toml = export("examples/crate.toml", &scratch);
    let json = export("examples/crate.json", &scratch);
    assert_eq!(toml, json);
}

#[test]
fn every_example_is_a_clean_model() {
    for name in EXAMPLES {
        let report: serde_json::Value = serde_json::from_slice(&duck_mold(&[
            "inspect",
            &format!("examples/{name}.toml"),
            "--json",
        ]))
        .unwrap();
        assert_eq!(report["ok"], true, "{name}");
        assert_eq!(report["name"], name);
        assert_eq!(report["warnings"], serde_json::json!([]), "{name}");
        assert_eq!(report["ground"]["rests_on_ground"], true, "{name}");
        assert_eq!(report["floating"], serde_json::json!([]), "{name}");
        // Only the tree overlaps, and it means to: its tiers are pushed
        // into each other.
        let overlaps = report["overlapping"].as_array().unwrap().len();
        assert_eq!(overlaps, if name == "tree" { 2 } else { 0 }, "{name}");
        assert!(report.get("unpainted_triangles").is_none(), "{name}");
    }
}

#[test]
fn the_duck_forge_reader_reads_every_golden_glb() {
    // While new golden files are being accepted they may not be written
    // yet; they are read on the next ordinary run.
    if std::env::var_os("DUCK_MOLD_ACCEPT").is_some() {
        return;
    }
    for name in EXAMPLES {
        let bytes = std::fs::read(root().join("tests/golden").join(format!("{name}.glb")))
            .expect("the golden files exist");
        let model =
            forge::read_duck_mold_glb(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                root()
                    .join("tests/golden")
                    .join(format!("{name}.inspect.json")),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(model.name, name);
        assert_eq!(
            model.triangles.len() as u64,
            report["triangles"].as_u64().unwrap(),
            "{name}"
        );
        // The palette, by name and exact colour.
        let palette: Vec<(String, String)> = model
            .palette
            .iter()
            .map(|color| {
                let [r, g, b] = color.rgb;
                (color.name.clone(), format!("#{r:02X}{g:02X}{b:02X}"))
            })
            .collect();
        let expected: Vec<(String, String)> = report["palette"]
            .as_array()
            .unwrap()
            .iter()
            .map(|color| {
                (
                    color["name"].as_str().unwrap().to_owned(),
                    color["hex"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(palette, expected, "{name}");
        // Every triangle has one of the palette's colours, and each
        // colour paints as many triangles as the report says.
        for (color, entry) in model
            .palette
            .iter()
            .zip(report["palette"].as_array().unwrap())
        {
            let painted = model
                .triangles
                .iter()
                .filter(|triangle| triangle.color == color.rgb)
                .count();
            assert_eq!(
                painted as u64,
                entry["triangles"].as_u64().unwrap(),
                "{name}: {}",
                color.name
            );
        }
        // The model stands on the ground and is the size the report says.
        let lowest = model
            .vertices
            .iter()
            .map(|v| v[1])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(lowest, 0.0, "{name}");
        for axis in 0..3 {
            let low = model
                .vertices
                .iter()
                .map(|v| v[axis])
                .fold(f32::INFINITY, f32::min);
            let high = model
                .vertices
                .iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!(
                low,
                report["bounds"]["min"][axis].as_f64().unwrap() as f32,
                "{name}"
            );
            assert_eq!(
                high,
                report["bounds"]["max"][axis].as_f64().unwrap() as f32,
                "{name}"
            );
        }
        // What docs/forge-handoff.md suggests as Duck Forge's first test.
        if name == "crate" {
            assert_eq!((model.vertices.len(), model.triangles.len()), (8, 12));
        }
        // The parts own every triangle, in order.
        let mut next = 0;
        for part in &model.parts {
            assert_eq!(part.first_triangle, next, "{name}: {}", part.name);
            next += part.triangles;
        }
        assert_eq!(next, model.triangles.len(), "{name}");
    }
}

#[test]
fn the_readmes_examples_are_true() {
    let readme = std::fs::read_to_string(root().join("README.md")).unwrap();
    // The text between a line that is exactly `fence` and the next
    // closing fence, starting the search at `after`.
    let block = |after: &str, fence: &str| -> String {
        let from = readme
            .find(after)
            .unwrap_or_else(|| panic!("the README no longer says: {after}"));
        let start = from + readme[from..].find(fence).unwrap() + fence.len();
        let end = start + readme[start..].find("\n```").unwrap();
        readme[start..end].trim_start_matches('\n').to_owned() + "\n"
    };

    // Its first model builds with nothing to complain about.
    let scratch = scratch("golden_readme");
    let recipe = scratch.join("tree.toml");
    std::fs::write(&recipe, block("## A first model", "```toml")).unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&duck_mold(&["inspect", recipe.to_str().unwrap(), "--json"]))
            .unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["warnings"], serde_json::json!([]));
    assert_eq!(report["floating"], serde_json::json!([]));

    // Every line it quotes from the robot's report is in the real one.
    let output = Command::new(env!("CARGO_BIN_EXE_duck-mold"))
        .args(["inspect", "examples/robot.toml"])
        .current_dir(root())
        .output()
        .unwrap();
    let real = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let quoted = block("## What `inspect` tells you", "```");
    let mut lines = 0;
    for line in quoted.lines().filter(|line| line.trim() != "...") {
        assert!(
            real.lines().any(|real| real == line),
            "the README quotes a line the program doesn't print: {line}"
        );
        lines += 1;
    }
    assert!(lines >= 10);
}
