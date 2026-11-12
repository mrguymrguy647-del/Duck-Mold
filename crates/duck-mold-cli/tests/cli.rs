//! The `duck-mold` program itself, run the way a person or an agent runs
//! it: what it prints, where it prints it, and its exit codes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs `duck-mold` with these arguments, in this folder.
fn run(folder: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_duck-mold"))
        .args(arguments)
        .current_dir(folder)
        .output()
        .expect("the program runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "standard output is one JSON document: {error}\n{}",
            stdout(output)
        )
    })
}

/// A folder for a test's files, emptied first, with a good recipe and a
/// bad one in it.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("tree.toml"), TREE).unwrap();
    std::fs::write(folder.join("bad.toml"), BAD).unwrap();
    folder
}

const TREE: &str = r##"version = 0
name = "tree"

[palette]
bark = "#6B4A2B"
leaf = "#3F8F4A"

[[parts]]
name = "trunk"
shape = "cylinder"
radius = 0.15
height = 1.2
segments = 8
color = "bark"

[[parts]]
name = "leaves"
shape = "cone"
radius = 0.7
height = 1.6
segments = 8
color = "leaf"
attach = "bottom"
to = "trunk.top"
"##;

const BAD: &str = r##"version = 0

[palette]
bark = "#6B4A2B"

[[parts]]
name = "trunk"
shape = "cylinder"
colour = "bark"

[[parts]]
name = "leaves"
shape = "cone"
color = "leaf"
attach = "bottom"
to = "trnk.top"
"##;

#[test]
fn inspect_prints_the_report_for_people() {
    let folder = folder("cli_inspect");
    let output = run(&folder, &["inspect", "tree.toml"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        "\
tree  (tree.toml)
  size         1.40 wide x 2.80 tall x 1.40 deep, in metres (x, y, z)
  triangles    42  (80 corners)
  ground       rests on the ground
  colours      2:  bark 28, leaf 14  (triangles each)
  parts        2
    trunk    cylinder   28 triangles
    leaves   cone       14 triangles   on trunk.top
  floating     none
  overlapping  none
"
    );
    // The verdict goes to standard error, so standard output is only
    // the report.
    assert_eq!(stderr(&output), "ok: no problems found\n");
    // Nothing is written.
    assert!(!folder.join("tree.glb").exists());
}

#[test]
fn inspect_json_prints_one_json_document() {
    let folder = folder("cli_inspect_json");
    let output = run(&folder, &["inspect", "tree.toml", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr(&output), "");
    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(
        report["summary"],
        "tree: 1.40 x 2.80 x 1.40 m, 42 triangles, 2 parts, 2 colours, rests on the ground, nothing floating, nothing overlapping"
    );
    assert_eq!(report["source"], "tree.toml");
    assert_eq!(report["size"], serde_json::json!([1.4, 2.8, 1.4]));
    assert_eq!(report["parts"][1]["attached_to"], "trunk.top");
}

#[test]
fn mistakes_are_listed_with_file_line_field_and_fix_and_exit_code_1() {
    let folder = folder("cli_mistakes");
    let output = run(&folder, &["inspect", "bad.toml"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "\
error: bad.toml:9: parts[0].colour: there is no field \"colour\" here. Did you mean \"color\"?
error: bad.toml:14: parts[1].color: there is no colour \"leaf\" in the palette. It can be: bark.
error: bad.toml:16: parts[1].to: there is no part \"trnk\" in this list. Did you mean \"trunk\"?
warning: bad.toml:6: parts[0]: faces of this part have no colour and are built in plain grey: give the part a \"color\" from the palette.
warning: bad.toml:4: palette.bark: the colour \"bark\" is never used.
3 errors, 2 warnings. Nothing was built.
"
    );
}

#[test]
fn mistakes_as_json_go_to_standard_output() {
    let folder = folder("cli_mistakes_json");
    let output = run(&folder, &["inspect", "bad.toml", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "");
    let report = json(&output);
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"].as_array().unwrap().len(), 3);
    assert_eq!(
        report["errors"][0],
        serde_json::json!({
            "severity": "error",
            "code": "unknown_field",
            "file": "bad.toml",
            "line": 9,
            "column": 1,
            "path": "parts[0].colour",
            "message": "there is no field \"colour\" here",
            "got": "colour",
            "allowed": { "options": ["name", "shape", "use", "file", "parts", "size", "radius", "height", "segments", "color", "colors", "attach", "to", "rotate", "move", "scale", "mirror", "sockets", "note"] },
            "suggestion": "color"
        })
    );
    assert_eq!(report["errors"][2]["code"], "unknown_part");
    assert_eq!(report["errors"][2]["suggestion"], "trunk");
    assert_eq!(report["warnings"].as_array().unwrap().len(), 2);
}

#[test]
fn export_writes_the_glb_next_to_the_recipe() {
    let folder = folder("cli_export");
    let output = run(&folder, &["export", "tree.toml"]);
    assert_eq!(output.status.code(), Some(0));
    let bytes = std::fs::read(folder.join("tree.glb")).unwrap();
    assert_eq!(&bytes[0..4], b"glTF");
    assert_eq!(
        stdout(&output),
        format!(
            "wrote tree.glb ({} bytes): tree: 1.40 x 2.80 x 1.40 m, 42 triangles, 2 parts, 2 colours, rests on the ground, nothing floating, nothing overlapping\n",
            bytes.len()
        )
    );
    assert_eq!(stderr(&output), "ok: no problems found\n");

    // The same recipe gives the same bytes, run after run.
    let again = folder.join("again.glb");
    let output = run(&folder, &["export", "tree.toml", "-o", "again.glb"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(std::fs::read(again).unwrap(), bytes);

    // And the file that was written can be inspected.
    let output = run(&folder, &["inspect", "tree.glb", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(json(&output)["triangles"], 42);
}

#[test]
fn export_json_adds_the_file_to_the_report() {
    let folder = folder("cli_export_json");
    let output = run(
        &folder,
        &["export", "tree.toml", "--output", "out.glb", "--json"],
    );
    assert_eq!(output.status.code(), Some(0));
    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(report["output"], "out.glb");
    assert_eq!(
        report["bytes"].as_u64().unwrap(),
        std::fs::metadata(folder.join("out.glb")).unwrap().len()
    );
    assert_eq!(report["triangles"], 42);
}

#[test]
fn export_writes_nothing_when_the_recipe_has_mistakes() {
    let folder = folder("cli_export_bad");
    let output = run(&folder, &["export", "bad.toml"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!folder.join("bad.glb").exists());
    assert!(stderr(&output).ends_with("3 errors, 2 warnings. Nothing was built.\n"));
}

#[test]
fn warnings_do_not_change_the_exit_code() {
    let folder = folder("cli_warnings");
    std::fs::write(
        folder.join("grey.toml"),
        "version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nmove = [0, 3, 0]\n",
    )
    .unwrap();
    let output = run(&folder, &["inspect", "grey.toml"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("  floating     b floats 2.00 from a\n"));
    assert_eq!(
        stderr(&output),
        "\
warning: grey.toml:2: parts[0]: faces of this part have no colour and are built in plain grey: give the part a \"color\" from the palette.
warning: grey.toml:5: parts[1]: faces of this part have no colour and are built in plain grey: give the part a \"color\" from the palette.
warning: grey.toml:5: parts[1]: the part \"b\" touches nothing: it floats 2.00 from \"a\".
ok, with 3 warnings
"
    );
    let output = run(&folder, &["export", "grey.toml"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(folder.join("grey.glb").exists());
}

#[test]
fn a_file_that_cannot_be_read_is_exit_code_2() {
    let folder = folder("cli_usage");
    let output = run(&folder, &["inspect", "nowhere.toml"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "error: can't read \"nowhere.toml\": there is no such file.\n"
    );

    // As JSON, still one document on standard output.
    let output = run(&folder, &["inspect", "nowhere.toml", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    let report = json(&output);
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"][0]["code"], "io");

    let output = run(
        &folder,
        &["export", "tree.toml", "-o", "no/such/folder/tree.glb"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("that folder doesn't exist"));
}

#[test]
fn a_wrong_command_line_is_exit_code_2() {
    let folder = folder("cli_wrong");
    for arguments in [
        &["inspect"][..],
        &["build", "tree.toml"],
        &["inspect", "tree.toml", "--jsno"],
        &[],
    ] {
        let output = run(&folder, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert_eq!(stdout(&output), "", "{arguments:?}");
    }
    // A slip in an option gets the likely fix.
    let output = run(&folder, &["inspect", "tree.toml", "--jsno"]);
    assert!(stderr(&output).contains("--json"), "{}", stderr(&output));
}

#[test]
fn help_and_version_explain_the_program() {
    let folder = folder("cli_help");
    let output = run(&folder, &["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!("duck-mold {}\n", env!("CARGO_PKG_VERSION"))
    );

    for arguments in [&["--help"][..], &["help"]] {
        let output = run(&folder, arguments);
        assert_eq!(output.status.code(), Some(0));
        let help = stdout(&output);
        for expected in [
            "inspect",
            "export",
            "Exit codes: 0 it worked, 1 the recipe has errors, 2",
        ] {
            assert!(
                help.contains(expected),
                "help should mention {expected}:\n{help}"
            );
        }
    }
    let output = run(&folder, &["export", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    let help = stdout(&output);
    assert!(
        help.contains("--output <FILE>") && help.contains("--json"),
        "{help}"
    );
    assert!(help.contains("Nothing is written if the recipe has mistakes"));
}
