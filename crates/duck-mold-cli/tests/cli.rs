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
warning: bad.toml:4: palette.bark: the colour \"bark\" is never used.
warning: bad.toml:6: parts[0]: faces of this part have no colour and are built in plain grey: give the part a \"color\" from the palette.
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
fn render_writes_a_png_next_to_the_recipe() {
    let folder = folder("cli_render");
    let output = run(&folder, &["render", "tree.toml"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let bytes = std::fs::read(folder.join("tree.png")).unwrap();
    assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(
        stdout(&output),
        format!(
            "wrote tree.png (513 x 536 pixels, {} bytes): tree: 1.40 x 2.80 x 1.40 m, 42 triangles, 2 parts, 2 colours, rests on the ground, nothing floating, nothing overlapping\n",
            bytes.len()
        )
    );
    assert_eq!(stderr(&output), "ok: no problems found\n");

    // The same recipe gives the same picture, run after run.
    let output = run(&folder, &["render", "tree.toml", "-o", "again.png"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(std::fs::read(folder.join("again.png")).unwrap(), bytes);

    // A model file can be drawn too, as it is.
    run(&folder, &["export", "tree.toml"]);
    let output = run(&folder, &["render", "tree.glb", "--size", "128"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("wrote tree.png (257 x 269 pixels"));
}

#[test]
fn render_json_says_where_each_view_is() {
    let folder = folder("cli_render_json");
    let output = run(
        &folder,
        &[
            "render",
            "tree.toml",
            "-o",
            "sheet.png",
            "--size",
            "200",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(report["output"], "sheet.png");
    assert_eq!(report["width"], 401);
    assert_eq!(report["triangles"], 42);
    assert_eq!(
        report["bytes"].as_u64().unwrap(),
        std::fs::metadata(folder.join("sheet.png")).unwrap().len()
    );
    let views: Vec<(String, u64, u64)> = report["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|view| {
            assert_eq!(view["size"], 200);
            (
                view["name"].as_str().unwrap().to_owned(),
                view["left"].as_u64().unwrap(),
                view["top"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        views,
        [
            ("front".to_owned(), 0, 0),
            ("right".to_owned(), 201, 0),
            ("back".to_owned(), 0, 201),
            ("three_quarter".to_owned(), 201, 201),
        ]
    );
    assert_eq!(
        report["views"][1]["from"],
        serde_json::json!([1.0, 0.0, 0.0])
    );
}

#[test]
fn render_writes_nothing_for_a_bad_recipe_or_a_bad_size() {
    let folder = folder("cli_render_bad");
    let output = run(&folder, &["render", "bad.toml"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!folder.join("bad.png").exists());

    let output = run(&folder, &["render", "tree.toml", "--size", "16"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!folder.join("tree.png").exists());
    assert_eq!(
        stderr(&output),
        "error: size: --size 16 is too small for the views' names to fit. Did you mean \"--size 256\"? Allowed: 128 to 2048.\n"
    );
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
            "render",
            "guide",
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

// ---- the guide for agents ----------------------------------------------------

/// The guide, as it is in the repository.
fn guide() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/AI_GUIDE.md"))
        .unwrap()
}

/// The text of the first fenced block that starts with `fence`, found
/// after `after` in the guide.
fn block_after(guide: &str, after: &str, fence: &str) -> String {
    let from = guide
        .find(after)
        .unwrap_or_else(|| panic!("the guide no longer says: {after}"));
    let start = from + guide[from..].find(fence).expect("a fenced block follows") + fence.len();
    let end = start + guide[start..].find("\n```").expect("the block ends");
    guide[start..end].trim_start_matches('\n').to_owned() + "\n"
}

#[test]
fn guide_prints_the_guide_that_is_in_the_repository() {
    let folder = folder("cli_guide");
    let output = run(&folder, &["guide"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output).trim_end(), guide().trim_end());
}

#[test]
fn the_guides_first_recipe_gives_exactly_the_report_it_shows() {
    // The guide teaches by example, so its example must be true.
    let guide = guide();
    let recipe = block_after(&guide, "## 3. A first recipe", "```toml");
    let shown = block_after(&guide, "`duck-mold inspect lamp.toml` prints:", "```");
    let folder = folder("cli_guide_lamp");
    std::fs::write(folder.join("lamp.toml"), recipe).unwrap();

    let output = run(&folder, &["inspect", "lamp.toml"]);
    assert_eq!(output.status.code(), Some(0));
    // The guide shows the report and the verdict together.
    assert_eq!(format!("{}{}", stdout(&output), stderr(&output)), shown);

    // And the JSON the guide quotes agrees with the real report.
    let report = json(&run(&folder, &["inspect", "lamp.toml", "--json"]));
    let quoted = block_after(&guide, "`inspect --json` prints (shortened):", "```json");
    let quoted: serde_json::Value =
        serde_json::from_str(&quoted).expect("the guide's JSON is valid");
    for field in [
        "ok",
        "summary",
        "source",
        "name",
        "size",
        "bounds",
        "triangles",
        "vertices",
        "ground",
        "floating",
        "overlapping",
        "warnings",
        "errors",
    ] {
        assert_eq!(report[field], quoted[field], "{field}");
    }
    assert_eq!(report["palette"][0], quoted["palette"][0]);
    assert_eq!(report["parts"][1], quoted["parts"][0]);
}

#[test]
fn every_field_the_guide_lists_is_a_real_field() {
    // A recipe with a made-up field is refused with the list of fields
    // that are allowed there. Every field in the guide's tables must be
    // in one of those lists.
    let folder = folder("cli_guide_fields");
    std::fs::write(
        folder.join("x.toml"),
        "version = 0\nzzz = 1\n[[parts]]\nshape = \"box\"\nzzz = 1\n",
    )
    .unwrap();
    let report = json(&run(&folder, &["inspect", "x.toml", "--json"]));
    let mut real: Vec<String> = Vec::new();
    for error in report["errors"].as_array().unwrap() {
        for option in error["allowed"]["options"].as_array().unwrap() {
            real.push(option.as_str().unwrap().to_owned());
        }
    }
    let guide = guide();
    let section = &guide[guide.find("## 5. The recipe on one page").unwrap()
        ..guide.find("The shapes. All are centred").unwrap()];
    let mut checked = 0;
    for line in section.lines().filter(|line| line.starts_with("| `")) {
        let cell = line.split('|').nth(1).unwrap();
        for field in cell.split('`').skip(1).step_by(2) {
            let field = field.split(' ').next().unwrap();
            assert!(
                real.iter().any(|name| name == field),
                "the guide lists `{field}`, which the program doesn't know"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 24, "the guide's tables list 24 fields");
}
