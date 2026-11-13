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
            "allowed": { "options": ["name", "shape", "use", "file", "parts", "size", "radius", "tube", "wall", "height", "segments", "steps", "bevel", "bevel_steps", "taper", "color", "changes", "colors", "attach", "to", "twist", "rotate", "move", "scale", "mirror", "limits", "sockets", "note"] },
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
fn inspect_sockets_lists_every_named_socket() {
    let folder = folder("cli_sockets");
    let output = run(&folder, &["inspect", "tree.toml", "--sockets"]);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    let lines: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("  sockets"))
        .collect();
    // The model's seven, and seven for each of its two parts.
    assert_eq!(lines.len(), 21, "{text}");
    assert_eq!(
        lines[0],
        "  sockets      top             at 0.00, 2.80, 0.00   facing up"
    );
    // A cone's sides are on its slant, facing out of it.
    assert!(
        lines
            .iter()
            .any(|line| line.contains("leaves.left") && line.contains("facing [-0.916, 0.401, 0]")),
        "{text}"
    );

    let report = json(&run(
        &folder,
        &["inspect", "tree.toml", "--sockets", "--json"],
    ));
    let sockets = report["sockets"].as_array().unwrap();
    assert_eq!(sockets.len(), 21);
    assert_eq!(sockets[7]["name"], "trunk.top");
    assert_eq!(sockets[7]["facing"], serde_json::json!([0.0, 1.0, 0.0]));

    // Without --sockets they aren't listed.
    let report = json(&run(&folder, &["inspect", "tree.toml", "--json"]));
    assert!(report.get("sockets").is_none());
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
fn edit_changes_the_recipe_and_prints_the_report() {
    let folder = folder("cli_edit");
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "star",
            "--shape",
            "sphere",
            "--radius",
            "0.1",
            "--to",
            "leaves.top",
            "--color",
            "leaf",
            "--twist",
            "-30",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let said = stdout(&output);
    assert!(
        said.starts_with(
            "edited tree.toml: added \"star\" (sphere) on leaves.top\ntree  (tree.toml)\n"
        ),
        "{said}"
    );
    assert!(said.contains("parts        3"), "{said}");
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        format!(
            "{TREE}\n[[parts]]\nname = \"star\"\nshape = \"sphere\"\nradius = 0.1\ncolor = \"leaf\"\nattach = \"bottom\"\nto = \"leaves.top\"\ntwist = -30\n"
        )
    );

    // As JSON: inspect's report, with what the edit did.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add-socket",
            "trunk",
            "knot",
            "--on",
            "front",
            "--shift",
            "-0.5,0.2,0",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(report["triangles"], 162);
    assert_eq!(report["edit"]["command"], "add-socket");
    assert_eq!(report["edit"]["written"], true);
    assert_eq!(
        report["edit"]["summary"],
        "added the socket \"knot\" on the front of \"trunk\""
    );

    // A dry run says what would happen and writes nothing.
    let before = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    let output = run(
        &folder,
        &["edit", "tree.toml", "rename", "star", "top", "--dry-run"],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout(&output).starts_with(
            "would edit tree.toml: renamed \"star\" to \"top\" (a dry run: nothing was written)\n"
        ),
        "{}",
        stdout(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        before
    );
}

#[test]
fn add_without_a_socket_stands_the_part_on_the_ground() {
    let folder = folder("cli_edit_free");
    // The trunk is 1.2 tall round the recipe's zero, so the ground is at
    // -0.6, and the middle of a rock 0.2 tall is 0.1 above it.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "rock",
            "--shape",
            "box",
            "--size",
            "0.4,0.2,0.4",
            "--color",
            "bark",
            "--stand",
            "1,-0.5",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("edited tree.toml: added \"rock\" (box), standing free\n"),
        "{}",
        stdout(&output)
    );
    let rock = "\n[[parts]]\nname = \"rock\"\nshape = \"box\"\nsize = [0.4, 0.2, 0.4]\ncolor = \"bark\"\nmove = [1, -0.5, -0.5]\n";
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        format!("{TREE}{rock}")
    );

    // An exact place, a turn and a size, as JSON.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "cloud",
            "--shape",
            "sphere",
            "--radius",
            "0.3",
            "--color",
            "leaf",
            "--move",
            "-1,3,0",
            "--rotate",
            "0,45,0",
            "--scale",
            "2,1,1",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["edit"]["command"], "add");
    assert_eq!(
        report["edit"]["summary"],
        "added \"cloud\" (sphere), standing free"
    );
    assert!(
        std::fs::read_to_string(folder.join("tree.toml"))
            .unwrap()
            .ends_with(
                "color = \"leaf\"\nrotate = [0, 45, 0]\nmove = [-1, 3, 0]\nscale = [2, 1, 1]\n"
            )
    );

    // A join's words with no join, and a place with a join: exit code 2.
    let before = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "x",
            "--shape",
            "box",
            "--attach",
            "top",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--attach and --twist belong to a join"),
        "{}",
        stderr(&output)
    );
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "x",
            "--shape",
            "box",
            "--to",
            "trunk.top",
            "--stand",
            "1,1",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("goes where its socket is"),
        "{}",
        stderr(&output)
    );
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "add",
            "x",
            "--shape",
            "box",
            "--stand",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--stand needs two numbers"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        before
    );
}

#[test]
fn the_newer_shapes_and_options_are_edited_from_the_command_line() {
    let folder = folder("cli_edit_shapes");
    // The trunk gets cut rims and a narrower top.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "set",
            "trunk",
            "--bevel",
            "0.03",
            "--bevel-steps",
            "2",
            "--taper",
            "0.6",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output)
            .starts_with("edited tree.toml: set \"trunk\": bevel 0.03, bevel_steps 2, taper 0.6\n"),
        "{}",
        stdout(&output)
    );
    let text = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    assert!(
        text.contains("segments = 8\nbevel = 0.03\nbevel_steps = 2\ntaper = 0.6\ncolor = \"bark\""),
        "{text}"
    );
    // Two numbers narrow it one way more than the other; 1 makes it
    // straight again, and 0 its rims sharp.
    let output = run(
        &folder,
        &["edit", "tree.toml", "set", "trunk", "--taper", "0.6,0.9"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        std::fs::read_to_string(folder.join("tree.toml"))
            .unwrap()
            .contains("taper = [0.6, 0.9]")
    );
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "set",
            "trunk",
            "--taper",
            "1",
            "--bevel",
            "0",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        TREE
    );

    // A ring round the trunk, steps beside it and an arch over it.
    for (name, words) in [
        (
            "swing",
            &["--shape", "torus", "--radius", "0.4", "--tube", "0.05"][..],
        ),
        (
            "ladder",
            &["--shape", "stairs", "--size", "0.5,0.6,0.6", "--steps", "3"][..],
        ),
        (
            "gate",
            &["--shape", "arch", "--size", "2,2.5,0.3", "--wall", "0.25"][..],
        ),
        (
            "pipe",
            &[
                "--shape", "tube", "--radius", "0.2", "--wall", "0.02", "--height", "0.5",
            ][..],
        ),
    ] {
        let mut arguments = vec!["edit", "tree.toml", "add", name, "--color", "bark"];
        arguments.extend_from_slice(words);
        let output = run(&folder, &arguments);
        assert_eq!(output.status.code(), Some(0), "{name}: {}", stderr(&output));
    }
    let text = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    for written in [
        "shape = \"torus\"\nradius = 0.4\ntube = 0.05\n",
        "shape = \"stairs\"\nsize = [0.5, 0.6, 0.6]\nsteps = 3\n",
        "shape = \"arch\"\nsize = [2, 2.5, 0.3]\nwall = 0.25\n",
        "shape = \"tube\"\nradius = 0.2\nwall = 0.02\nheight = 0.5\n",
    ] {
        assert!(text.contains(written), "{written} isn't in:\n{text}");
    }
    let output = run(&folder, &["inspect", "tree.toml", "--json"]);
    assert_eq!(json(&output)["parts"].as_array().unwrap().len(), 6);

    // Words that are wrong: exit code 2, and nothing written.
    let output = run(
        &folder,
        &["edit", "tree.toml", "set", "trunk", "--taper", "1,2,3"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--taper needs one number"),
        "{}",
        stderr(&output)
    );
    // An option the shape can't have: exit code 1, with why.
    let output = run(
        &folder,
        &["edit", "tree.toml", "set", "leaves", "--bevel", "0.1"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("only a box and a cylinder have edges to cut"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        text
    );
}

#[test]
fn a_colour_is_removed_and_a_faces_colour_taken_away_from_the_command_line() {
    let folder = folder("cli_edit_colours");
    // In use: it can't be done as asked (exit code 2, one line), which
    // names what is painted with it.
    let output = run(&folder, &["edit", "tree.toml", "remove-color", "bark"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains(
            "the colour \"bark\" is in use: 1 thing is painted with it (trunk). Say what to paint them instead, like --replace-with leaf"
        ),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        TREE
    );
    // Repainted first, it goes.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "remove-color",
            "bark",
            "--replace-with",
            "leaf",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["edit"]["command"], "remove-color");
    assert_eq!(
        report["edit"]["summary"],
        "removed the colour \"bark\", and painted 1 thing with \"leaf\" instead"
    );
    assert_eq!(report["palette"].as_array().unwrap().len(), 1);
    let text = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    assert!(!text.contains("bark"), "{text}");

    // A face gets a colour of its own, and loses it again.
    let before = text;
    for (face, wanted) in [("top=leaf", true), ("top=none", false)] {
        let output = run(
            &folder,
            &["edit", "tree.toml", "set", "trunk", "--face-color", face],
        );
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        let text = std::fs::read_to_string(folder.join("tree.toml")).unwrap();
        assert_eq!(
            text.contains("colors = { top = \"leaf\" }"),
            wanted,
            "{text}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        before
    );
}

#[test]
fn sockets_are_renamed_removed_and_tidied_from_the_command_line() {
    let folder = folder("cli_edit_sockets");
    let edit = |words: &[&str]| {
        let mut arguments = vec!["edit", "tree.toml"];
        arguments.extend_from_slice(words);
        run(&folder, &arguments)
    };
    let text = || std::fs::read_to_string(folder.join("tree.toml")).unwrap();
    // Two sockets on the trunk, and a bird on one of them.
    for name in ["spot", "spot_2"] {
        let output = edit(&["add-socket", "trunk", name, "--on", "front"]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    let output = edit(&[
        "add",
        "bird",
        "--shape",
        "sphere",
        "--radius",
        "0.05",
        "--color",
        "leaf",
        "--to",
        "trunk.spot",
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    // Renamed: the bird's join follows.
    let output = edit(&["rename-socket", "trunk.spot", "perch"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output)
            .starts_with("edited tree.toml: renamed the socket \"trunk.spot\" to \"perch\"\n"),
        "{}",
        stdout(&output)
    );
    assert!(text().contains("perch = { on = \"front\" }"), "{}", text());
    assert!(text().contains("to = \"trunk.perch\""), "{}", text());

    // In use: it can't be removed, and the line says what uses it.
    let output = edit(&["remove-socket", "trunk.perch"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: tree.toml: the socket \"trunk.perch\" is in use by: bird. Join that somewhere else first, or remove it.\n"
    );
    // Tidy takes the Studio's unused one, and leaves the rest.
    let output = edit(&["tidy-sockets", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["edit"]["command"], "tidy-sockets");
    assert_eq!(
        report["edit"]["summary"],
        "removed 1 socket nothing uses: trunk.spot_2"
    );
    assert!(!text().contains("spot_2"), "{}", text());
    assert!(text().contains("perch = "), "{}", text());
    // With the bird gone, the socket can go.
    assert_eq!(edit(&["remove", "bird"]).status.code(), Some(0));
    let output = edit(&["remove-socket", "trunk.perch"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(text(), TREE);
}

#[test]
fn an_edit_that_cant_be_made_says_why_and_writes_nothing() {
    let folder = folder("cli_edit_refused");
    // No such part: exit code 2, one line, with the likely fix.
    let output = run(&folder, &["edit", "tree.toml", "remove", "trunc"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: tree.toml: there is no part \"trunc\" in the recipe. Did you mean \"trunk\"?\n"
    );
    // It would break the recipe: exit code 1, the mistakes, as JSON too.
    let output = run(
        &folder,
        &[
            "edit",
            "tree.toml",
            "set",
            "trunk",
            "--color",
            "pink",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"][0]["code"], "unknown_color");
    // The words themselves are wrong: exit code 2.
    let output = run(
        &folder,
        &["edit", "tree.toml", "set", "trunk", "--size", "1,2"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--size needs three numbers"),
        "{}",
        stderr(&output)
    );
    let output = run(
        &folder,
        &["edit", "tree.toml", "add", "x", "--to", "trunk.top"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--shape box"),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("tree.toml")).unwrap(),
        TREE
    );
}

#[test]
fn every_edit_has_a_command_and_is_in_the_guide() {
    // The Studio builds `Edit` values; so does the command line. Every
    // kind of edit must be reachable from both, and written down for
    // agents.
    let folder = folder("cli_edit_all");
    let help = stdout(&run(&folder, &["edit", "--help"]));
    let guide = guide();
    for command in duck_mold_core::edit::Edit::COMMANDS {
        assert!(
            help.lines()
                .any(|line| line.trim_start().starts_with(&format!("{command} "))),
            "`duck-mold edit` has no command {command}:\n{help}"
        );
        let output = run(&folder, &["edit", "tree.toml", command, "--help"]);
        assert_eq!(output.status.code(), Some(0), "{command}");
        // (Followed by its words, or by nothing at all.)
        assert!(
            guide.contains(&format!("`duck-mold edit <recipe> {command} "))
                || guide.contains(&format!("`duck-mold edit <recipe> {command}`")),
            "the guide doesn't describe `duck-mold edit <recipe> {command}`"
        );
    }
}

#[test]
fn preview_needs_a_file_that_is_there() {
    // Only what fails before a window would open is tried here: a test
    // machine may have a screen, and a window would wait to be closed.
    let folder = folder("cli_preview");
    for command in ["preview", "studio"] {
        let output = run(&folder, &[command, "nowhere.toml"]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            "error: can't read \"nowhere.toml\": there is no such file.\n"
        );
    }
    let output = run(&folder, &["preview", "--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        stdout(&output).contains("drag to orbit"),
        "{}",
        stdout(&output)
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
            "edit",
            "new",
            "studio",
            "preview",
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

#[test]
fn studio_and_preview_start_the_studio_program_beside_duck_mold() {
    let folder = folder("cli_studio");
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    std::fs::copy(examples.join("robot.toml"), folder.join("robot.toml")).unwrap();
    let with_studio_at = |program: &Path, arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_duck-mold"))
            .current_dir(&folder)
            .env("DUCK_MOLD_STUDIO", program)
            .args(arguments)
            .output()
            .unwrap()
    };

    // The Studio is a program of its own. If it isn't there, say how to
    // get it.
    let output = with_studio_at(&folder.join("NoStudioHere"), &["studio", "robot.toml"]);
    assert_eq!(output.status.code(), Some(2));
    let said = stderr(&output);
    assert!(
        said.contains("the Studio program isn't here")
            && said.contains("cargo build --release -p duck-mold-studio"),
        "{said}"
    );

    // A program that is there is started, with the file (and --look for
    // preview). `duck-mold` itself stands in for the Studio here: it
    // doesn't know --look, so it ends at once, and that is reported.
    let stand_in = Path::new(env!("CARGO_BIN_EXE_duck-mold"));
    let output = with_studio_at(stand_in, &["preview", "robot.toml"]);
    assert_eq!(output.status.code(), Some(2));
    let said = stderr(&output);
    assert!(said.contains("--look"), "{said}");
    assert!(
        said.contains("the Studio couldn't open its window"),
        "{said}"
    );
    // With no --look, the stand-in is asked for "robot.toml", which it
    // doesn't know as a command either.
    let output = with_studio_at(stand_in, &["studio", "robot.toml"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("robot.toml"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn new_starts_a_recipe_and_never_writes_over_one() {
    let folder = folder("cli_new");
    let output = run(&folder, &["new", "crate.toml"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let said = stdout(&output);
    assert!(
        said.starts_with("wrote crate.toml\ncrate  (crate.toml)\n"),
        "{said}"
    );
    assert!(said.contains("rests on the ground"), "{said}");
    assert_eq!(stderr(&output), "", "a new recipe has no warnings");
    let text = std::fs::read_to_string(folder.join("crate.toml")).unwrap();
    assert!(text.contains("name = \"crate\""), "{text}");

    // It can be changed at once, like any recipe.
    let output = run(
        &folder,
        &[
            "edit",
            "crate.toml",
            "add",
            "lid",
            "--shape",
            "box",
            "--to",
            "body.top",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    // Never over a file that is there; only .toml.
    let output = run(&folder, &["new", "crate.toml"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: \"crate.toml\" is already there: new never writes over a file.\n"
    );
    assert!(
        std::fs::read_to_string(folder.join("crate.toml"))
            .unwrap()
            .contains("lid")
    );
    let output = run(&folder, &["new", "crate.json", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["ok"], false);
    assert!(!folder.join("crate.json").exists());

    let output = run(&folder, &["new", "second.toml", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["name"], "second");
    assert_eq!(report["triangles"], 12);
}

#[test]
fn new_can_start_from_a_copy_of_an_example() {
    let folder = folder("cli_new_example");
    // Every example is in the program, whatever folder it is run from,
    // and each copy is a clean model, the same as the one in the
    // repository.
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for example in duck_mold_core::tools::EXAMPLES {
        let name = format!("my_{}.toml", example.name);
        let output = run(
            &folder,
            &["new", &name, "--example", example.name, "--json"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {}",
            example.name,
            stderr(&output)
        );
        let report = json(&output);
        assert_eq!(report["ok"], true, "{}", example.name);
        assert_eq!(report["name"], example.name);
        // (But for how each system ends its lines.)
        let unix = |text: String| text.replace("\r\n", "\n");
        assert_eq!(
            unix(std::fs::read_to_string(folder.join(&name)).unwrap()),
            unix(std::fs::read_to_string(examples.join(format!("{}.toml", example.name))).unwrap()),
            "{}",
            example.name
        );
    }
    // The desk's lamp and crate came with it.
    assert!(folder.join("parts/lamp.gltf").is_file());
    assert!(folder.join("parts/crate.glb").is_file());

    // Never over a file that is there; and a name that isn't an example
    // lists the ones there are.
    let output = run(&folder, &["new", "my_dog.toml", "--example", "robot"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("is already there: new never writes over a file"),
        "{}",
        stderr(&output)
    );
    let output = run(&folder, &["new", "other.toml", "--example", "robbot"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "error: there is no example called \"robbot\". Did you mean \"robot\"?\n"
    );
    assert!(!folder.join("other.toml").exists());
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
    assert_eq!(checked, 27, "the guide's tables list 27 fields");
}

#[test]
fn a_box_is_shaped_from_the_command_line_and_its_faces_listed() {
    let folder = folder("cli_shaping");
    std::fs::write(
        folder.join("wall.toml"),
        "version = 0\n\n[palette]\nplaster = \"#E8E0D0\"\n\n[[parts]]\nname = \"wall\"\nshape = \"box\"\nsize = [3, 2.4, 0.2]\ncolor = \"plaster\"\n",
    )
    .unwrap();
    let edit = |words: &[&str]| {
        let mut arguments = vec!["edit", "wall.toml"];
        arguments.extend_from_slice(words);
        run(&folder, &arguments)
    };
    for words in [
        &["inset", "wall", "front", "--by", "0.6", "--name", "window"][..],
        &["inset", "wall", "window", "--by", "0.06", "--name", "glass"],
        &["pull", "wall", "glass", "--by", "-0.08"],
    ] {
        let output = edit(words);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    let text = std::fs::read_to_string(folder.join("wall.toml")).unwrap();
    assert!(text.ends_with(
        "color = \"plaster\"\nchanges = [\n  { inset = \"front\", by = 0.6, name = \"window\" },\n  { inset = \"window\", by = 0.06, name = \"glass\" },\n  { pull = \"glass\", by = -0.08 },\n]\n"
    ));

    // Several faces with commas; a face that isn't there is refused,
    // with the closest, and nothing is written.
    let output = edit(&["pull", "wall", "glas", "--by", "0.1"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains(
            "parts[0].changes[3]: there is no face \"glas\" on \"wall\". Did you mean \"glass\"?"
        ),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("wall.toml")).unwrap(),
        text
    );

    let output = run(&folder, &["inspect", "wall.toml", "--faces"]);
    let out = stdout(&output);
    assert!(
        out.contains("    wall   box, 3 changes   36 triangles\n"),
        "{out}"
    );
    assert!(
        out.contains("  faces        wall  (wall.toml): box, 3 changes, 18 faces\n"),
        "{out}"
    );
    assert!(out.contains(
        "    glass               faces front   1.68 x 1.08 m   next to: glass_side_bottom, glass_side_right, glass_side_top, glass_side_left\n"
    ));
    assert!(out.contains(
        "    window              stands for: glass_rim_bottom, glass_rim_right, glass_rim_top, glass_rim_left\n"
    ));
    let report = json(&run(
        &folder,
        &["inspect", "wall.toml", "--faces", "--json"],
    ));
    assert_eq!(report["parts"][0]["changes"], 3);
    assert_eq!(report["shaped"][0]["face_count"], 18);
    assert_eq!(report["shaped"][0]["faces"][4]["name"], "glass");
    // Without --faces, no list.
    let report = json(&run(&folder, &["inspect", "wall.toml", "--json"]));
    assert!(report.get("shaped").is_none());
}

// ---- Phase 5: models that move ---------------------------------------------------------

#[test]
fn limits_are_set_and_the_joins_listed_from_the_command_line() {
    let folder = folder("cli_limits");
    let edit = |words: &[&str]| {
        let mut arguments = vec!["edit", "tree.toml"];
        arguments.extend_from_slice(words);
        run(&folder, &arguments)
    };
    let text = || std::fs::read_to_string(folder.join("tree.toml")).unwrap();

    // The leaves sway a little on the trunk, and turn right round.
    let output = edit(&["set-limits", "leaves", "--x", "-10,10", "--y", "free"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output)
            .starts_with("edited tree.toml: set the limits of \"leaves\": x -10..10, y free\n"),
        "{}",
        stdout(&output)
    );
    assert!(
        text().contains("limits = { x = [-10, 10], y = \"free\" }"),
        "{}",
        text()
    );
    // The report says so beside the part.
    assert!(
        stdout(&output).contains("on trunk.top, turns x -10..10, y free"),
        "{}",
        stdout(&output)
    );

    // The tree of joins: what each part turns about, and how far.
    let output = run(&folder, &["inspect", "tree.toml", "--joins"]);
    let listing = stdout(&output);
    assert!(
        listing.contains("  joins        2: what each part turns about, and how far\n"),
        "{listing}"
    );
    assert!(
        listing.contains("    trunk      turns about its middle   free\n"),
        "{listing}"
    );
    assert!(
        listing.contains("      leaves   on trunk.top             x -10..10, y free\n"),
        "{listing}"
    );
    let report = json(&run(
        &folder,
        &["inspect", "tree.toml", "--joins", "--json"],
    ));
    assert_eq!(
        report["joins"][1],
        serde_json::json!({
            "part": "leaves",
            "parent": "trunk",
            "joined_to": "trunk.top",
            "pivot": [0.0, 1.2, 0.0],
            "limits": { "x": [-10.0, 10.0], "y": "free" }
        })
    );
    assert_eq!(report["joins"][0]["parent"], serde_json::Value::Null);
    assert_eq!(report["joins"][0]["limits"], "free");
    // In the report's own tree of parts, only a part with limits says so.
    assert_eq!(
        report["parts"][1]["limits"],
        serde_json::json!({ "x": [-10.0, 10.0], "y": "free" })
    );
    assert!(report["parts"][0].get("limits").is_none());
    // Without --joins the list isn't there.
    let report = json(&run(&folder, &["inspect", "tree.toml", "--json"]));
    assert!(report.get("joins").is_none());

    // One direction locked again, then fixed, then free.
    let output = edit(&["set-limits", "leaves", "--y", "none", "--dry-run"]);
    assert!(
        stdout(&output).contains("x -10..10\n"),
        "{}",
        stdout(&output)
    );
    assert!(text().contains("y = \"free\""), "a dry run writes nothing");
    let output = edit(&["set-limits", "leaves", "--fixed", "--json"]);
    assert_eq!(json(&output)["edit"]["command"], "set-limits");
    assert!(text().contains("limits = \"fixed\""), "{}", text());
    let output = edit(&["set-limits", "leaves", "--none"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(text(), TREE);

    // Mistakes say what to write.
    let output = edit(&["set-limits", "leaves", "--x", "ten"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains(
            "--x needs two angles in degrees, like -30,40, or free, or none, not \"ten\""
        ),
        "{}",
        stderr(&output)
    );
    let output = edit(&["set-limits", "leaves", "--x", "5,40"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("must have 0 between them"),
        "{}",
        stderr(&output)
    );
    assert_eq!(text(), TREE);
}
