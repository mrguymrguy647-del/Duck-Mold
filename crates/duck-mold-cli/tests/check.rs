//! `duck-mold check`: one list of problems, worst first, each with the
//! command that fixes it; and the commands it gives really fix them.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(folder: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_duck-mold"))
        .args(arguments)
        .current_dir(folder)
        .output()
        .expect("the program runs")
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "standard output is one JSON document: {error}\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn folder(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("check-{name}"));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    for (name, text) in files {
        std::fs::write(folder.join(name), text).unwrap();
    }
    folder
}

/// A model that builds, with three problems: a part in the air, an
/// event heard twice where a loop joins, and a colour nobody uses.
const PROBLEMS: &str = r##"version = 0
name = "problems"

[palette]
red = "#D0453A"
rust = "#8B3A1A"

[[parts]]
name = "body"
shape = "box"
size = [1, 1, 1]
color = "red"

[[parts]]
name = "ball"
shape = "sphere"
radius = 0.2
color = "red"
move = [3, 2, 0]

[animations.bob]
loop = true
keys = [
  { at = 0, pose = "rest", event = "step" },
  { at = 1, pose = "rest", event = "step" },
]
"##;

/// A recipe with a mistake, and a colour nobody uses.
const BROKEN: &str = r##"version = 0
name = "broken"

[palette]
red = "#D0453A"
rust = "#8B3A1A"

[[parts]]
name = "body"
shape = "box"
size = [1, 1, 1]
colour = "red"
"##;

fn problems(report: &serde_json::Value) -> Vec<(String, String)> {
    report["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["rank"].as_str().unwrap().to_owned(),
                p["code"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn check_lists_every_problem_worst_first_with_its_fix() {
    let folder = folder("order", &[("problems.toml", PROBLEMS)]);
    let output = run(&folder, &["check", "problems.toml", "--json"]);
    assert_eq!(output.status.code(), Some(0), "warnings still exit 0");
    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(
        problems(&report),
        [
            ("warning".to_owned(), "floating".to_owned()),
            ("warning".to_owned(), "event_twice".to_owned()),
            ("small".to_owned(), "unused".to_owned()),
        ]
    );
    assert_eq!(
        (
            report["errors"].as_u64(),
            report["warnings"].as_u64(),
            report["small"].as_u64()
        ),
        (Some(0), Some(2), Some(1))
    );
    let list = report["problems"].as_array().unwrap();
    // No command surely fixes a part in the air: it says what to do.
    assert!(list[0]["fix"].is_null());
    assert!(list[0]["advice"].as_str().unwrap().contains("join"));
    assert_eq!(
        list[1]["fix"],
        "duck-mold edit problems.toml remove-event bob 2 step"
    );
    assert_eq!(
        list[2]["fix"],
        "duck-mold edit problems.toml remove-color rust"
    );
    assert!(list[2]["advice"].is_null(), "a fix needs no advice");

    // It looked at the animation, drew the sheet, and kept inspect's
    // report.
    assert_eq!(report["animations"][0]["name"], "bob");
    assert!(report["animations"][0]["frames"].as_u64().unwrap() > 2);
    assert_eq!(report["picture"]["output"], "problems.png");
    assert!(folder.join("problems.png").is_file());
    assert_eq!(report["inspection"]["name"], "problems");
    assert!(report["next"].as_str().unwrap().contains("warnings"));
}

#[test]
fn the_fixes_check_gives_fix_their_problems() {
    let folder = folder("fixes", &[("problems.toml", PROBLEMS)]);
    let report = json(&run(
        &folder,
        &["check", "problems.toml", "--json", "--no-render"],
    ));
    let fixes: Vec<String> = report["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["fix"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(fixes.len(), 2);
    for fix in &fixes {
        let words: Vec<&str> = fix.split_whitespace().collect();
        assert_eq!(words[0], "duck-mold");
        let output = run(&folder, &words[1..]);
        assert!(
            output.status.success(),
            "{fix}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let after = json(&run(
        &folder,
        &["check", "problems.toml", "--json", "--no-render"],
    ));
    assert_eq!(
        problems(&after),
        [("warning".to_owned(), "floating".to_owned())]
    );
    assert!(
        !folder.join("problems.png").exists(),
        "--no-render writes nothing"
    );
}

#[test]
fn check_puts_mistakes_first_and_exits_1() {
    let folder = folder("broken", &[("broken.toml", BROKEN)]);
    let output = run(&folder, &["check", "broken.toml", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["ok"], false);
    let list = problems(&report);
    assert_eq!(list[0], ("error".to_owned(), "unknown_field".to_owned()));
    assert_eq!(report["problems"][0]["suggestion"], "color");
    assert!(list.iter().skip(1).all(|(rank, _)| rank != "error"));
    assert!(report["picture"].is_null(), "nothing is drawn");
    assert!(report["inspection"].is_null());
    assert!(!folder.join("broken.png").exists());
    assert!(
        report["next"]
            .as_str()
            .unwrap()
            .starts_with("Fix the error")
    );
}

#[test]
fn check_with_no_problems_says_to_look_then_export() {
    let tree = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/tree.toml"),
    )
    .unwrap();
    let folder = folder("clean", &[("tree.toml", &tree)]);
    let output = run(&folder, &["check", "tree.toml"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("0 errors, 0 warnings, 0 small"), "{text}");
    assert!(text.contains("picture: tree.png"), "{text}");
    assert!(text.contains("duck-mold export tree.toml"), "{text}");
}

#[test]
fn check_of_a_missing_file_is_a_usage_failure() {
    let folder = folder("missing", &[]);
    let output = run(&folder, &["check", "nothing.toml", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(json(&output)["ok"], false);
}
