//! Step 7: `inspect` reports on a model in words and numbers: size,
//! triangles, parts, colours, the ground, floating and overlapping parts.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::inspect::{Floating, Inspection, Overlap, inspect};
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load_text;
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::{self, Failure};

/// Builds a recipe and inspects it.
fn report(text: &str) -> Inspection {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    let found = Report::from_issues(issues);
    assert_eq!(found.errors, Vec::new());
    inspect(&model.unwrap(), "test.toml", found.warnings)
}

fn only(issues: &[Issue], code: Code) -> &Issue {
    let found: Vec<&Issue> = issues.iter().filter(|issue| issue.code == code).collect();
    assert_eq!(found.len(), 1, "expected one {code} in {issues:#?}");
    found[0]
}

/// A recipe of red parts. The first, `a`, is a 1 x 1 x 1 box whose middle
/// is the model's zero point... but lifted so that it stands on the
/// ground: its top is at y = 1.
fn boxes(rest: &str) -> String {
    format!(
        "version = 0\norigin = \"a.bottom\"\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"a\"\nshape = \"box\"\ncolor = \"red\"\n{rest}\n"
    )
}

/// One more red part, placed by hand. `at` is where its middle goes,
/// measured from the middle of `a`.
fn part(name: &str, shape: &str, at: [f64; 3]) -> String {
    format!(
        "[[parts]]\nname = \"{name}\"\n{shape}\ncolor = \"red\"\nmove = [{}, {}, {}]\n",
        at[0], at[1], at[2]
    )
}

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

const ROBOT: &str = r##"
version = 0
name = "robot"

[palette]
steel = "#8A939B"
dark = "#3B4148"
red = "#D0453A"
glow = "#FFE08A"

[define.arm]
shape = "box"
size = [0.16, 0.6, 0.16]
color = "steel"
colors = { bottom = "red" }
[define.arm.sockets]
shoulder = { on = "right", shift = [0, 0.7, 0] }

[define.leg]
shape = "cylinder"
radius = 0.1
height = 0.5
color = "dark"

[[parts]]
name = "body"
shape = "box"
size = [0.7, 0.8, 0.4]
color = "steel"
[parts.sockets]
neck = { on = "top" }
left_arm = { on = "left", shift = [0, 0.6, 0] }
right_arm = { on = "right", shift = [0, 0.6, 0] }
left_leg = { on = "bottom", shift = [-0.5, 0, 0] }
right_leg = { on = "bottom", shift = [0.5, 0, 0] }

[[parts]]
name = "head"
shape = "box"
size = [0.45, 0.35, 0.4]
color = "steel"
colors = { front = "glow" }
attach = "bottom"
to = "body.neck"

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
name = "left_leg"
use = "leg"
attach = "top"
to = "body.left_leg"

[[parts]]
name = "right_leg"
use = "leg"
attach = "top"
to = "body.right_leg"
"##;

#[test]
fn the_report_measures_the_model() {
    let report = report(ROBOT);
    assert!(report.ok);
    assert_eq!(report.name, "robot");
    assert_eq!(report.source, "test.toml");
    assert_eq!(report.size, [1.02, 1.65, 0.4]);
    assert_eq!(report.bounds.min, [-0.51, 0.0, -0.2]);
    assert_eq!(report.bounds.max, [0.51, 1.65, 0.2]);
    assert_eq!(report.triangles, 168);
    assert_eq!(report.vertices, 288);
    assert!(report.ground.rests_on_ground);
    assert_eq!(report.ground.lowest_y, 0.0);
    let colours: Vec<(&str, &str, usize)> = report
        .palette
        .iter()
        .map(|color| (color.name.as_str(), color.hex.as_str(), color.triangles))
        .collect();
    assert_eq!(
        colours,
        [
            ("steel", "#8A939B", 42),
            ("dark", "#3B4148", 120),
            ("red", "#D0453A", 4),
            ("glow", "#FFE08A", 2)
        ]
    );
    assert_eq!(report.unpainted_triangles, 0);
    assert_eq!(report.floating, Vec::new());
    assert_eq!(report.overlapping, Vec::new());
    assert_eq!(report.warnings, Vec::new());
    assert_eq!(
        report.summary,
        "robot: 1.02 x 1.65 x 0.40 m, 168 triangles, 6 parts, 4 colours, rests on the ground, nothing floating, nothing overlapping"
    );
}

#[test]
fn the_report_lists_the_parts() {
    let report = report(ROBOT);
    let names: Vec<&str> = report.parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "body",
            "head",
            "left_arm",
            "right_arm",
            "left_leg",
            "right_leg"
        ]
    );

    let body = &report.parts[0];
    assert_eq!((body.kind.as_str(), body.triangles), ("box", 12));
    assert_eq!(body.size, [0.7, 0.8, 0.4]);
    assert_eq!(body.bounds.min, [-0.35, 0.5, -0.2]);
    assert_eq!(body.bounds.max, [0.35, 1.3, 0.2]);
    assert_eq!(body.colors, ["steel"]);
    assert_eq!(body.attached_to, None);
    assert!(!body.mirrored);

    let left_arm = &report.parts[2];
    assert_eq!(left_arm.kind, "box");
    assert_eq!(left_arm.uses.as_deref(), Some("arm"));
    assert_eq!(left_arm.size, [0.16, 0.6, 0.16]);
    assert_eq!(left_arm.bounds.min, [-0.51, 0.63, -0.08]);
    assert_eq!(left_arm.bounds.max, [-0.35, 1.23, 0.08]);
    assert_eq!(left_arm.colors, ["steel", "red"]);
    assert_eq!(left_arm.attached_to.as_deref(), Some("body.left_arm"));
    assert!(report.parts[3].mirrored);
    assert_eq!(report.parts[4].triangles, 60);
}

#[test]
fn the_report_reads_well_as_text() {
    let expected = "\
robot  (test.toml)
  size         1.02 wide x 1.65 tall x 0.40 deep, in metres (x, y, z)
  triangles    168  (288 corners)
  ground       rests on the ground
  colours      4:  steel 42, dark 120, red 4, glow 2  (triangles each)
  parts        6
    body        box              12 triangles
    head        box              12 triangles   on body.neck
    left_arm    arm (box)        12 triangles   on body.left_arm
    right_arm   arm (box)        12 triangles   on body.right_arm, mirrored
    left_leg    leg (cylinder)   60 triangles   on body.left_leg
    right_leg   leg (cylinder)   60 triangles   on body.right_leg
  floating     none
  overlapping  none
";
    assert_eq!(report(ROBOT).to_text(), expected);
}

#[test]
fn the_report_is_json_an_agent_can_read() {
    let json = serde_json::to_string_pretty(&report(ROBOT)).unwrap();
    // The one-line summary comes right at the top.
    assert!(
        json.starts_with("{\n  \"ok\": true,\n  \"summary\": \"robot: 1.02 x 1.65 x 0.40 m"),
        "{json}"
    );
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["size"], serde_json::json!([1.02, 1.65, 0.4]));
    assert_eq!(
        value["bounds"]["min"],
        serde_json::json!([-0.51, 0.0, -0.2])
    );
    assert_eq!(
        value["ground"],
        serde_json::json!({ "rests_on_ground": true, "lowest_y": 0.0 })
    );
    assert_eq!(
        value["palette"][1],
        serde_json::json!({ "name": "dark", "hex": "#3B4148", "triangles": 120 })
    );
    assert_eq!(
        value["parts"][0],
        serde_json::json!({
            "name": "body", "kind": "box", "triangles": 12,
            "size": [0.7, 0.8, 0.4],
            "bounds": { "min": [-0.35, 0.5, -0.2], "max": [0.35, 1.3, 0.2] },
            "colors": ["steel"], "attached_to": null, "parts": []
        })
    );
    assert_eq!(value["parts"][3]["uses"], "arm");
    assert_eq!(value["parts"][3]["mirrored"], true);
    assert_eq!(value["floating"], serde_json::json!([]));
    assert_eq!(value["overlapping"], serde_json::json!([]));
    assert_eq!(value["warnings"], serde_json::json!([]));
    assert_eq!(value["errors"], serde_json::json!([]));
    // Left out when there is nothing to say.
    assert!(value.get("unpainted_triangles").is_none());
    assert!(value.get("materials").is_none());
}

#[test]
fn nested_parts_are_reported_as_a_tree() {
    let report = report(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[define.arm]\ncolor = \"red\"\n[[define.arm.parts]]\nname = \"upper\"\nshape = \"box\"\nsize = [0.2, 0.6, 0.2]\n[[define.arm.parts]]\nname = \"hand\"\nshape = \"sphere\"\nradius = 0.15\nsegments = 8\nattach = \"top\"\nto = \"upper.bottom\"\n[[parts]]\nname = \"arm\"\nuse = \"arm\"\n",
    );
    let arm = &report.parts[0];
    assert_eq!(
        (arm.kind.as_str(), arm.uses.as_deref(), arm.triangles),
        ("assembly", Some("arm"), 60)
    );
    let inner: Vec<(&str, usize)> = arm
        .parts
        .iter()
        .map(|part| (part.name.as_str(), part.triangles))
        .collect();
    assert_eq!(inner, [("arm/upper", 12), ("arm/hand", 48)]);
    // The count of parts is of the parts that have a shape of their own.
    assert!(report.summary.contains(", 2 parts, "), "{}", report.summary);
    let text = report.to_text();
    assert!(text.contains("  parts        2\n    arm       arm (assembly)   60 triangles\n      upper   box              12 triangles\n      hand    sphere           48 triangles   on upper.bottom\n"), "{text}");
}

// ---- the ground --------------------------------------------------------------

#[test]
fn a_model_off_the_ground_is_reported() {
    let sunk = report(
        "version = 0\norigin = \"center\"\n[palette]\nred = \"#D0453A\"\n[[parts]]\nshape = \"box\"\ncolor = \"red\"\n",
    );
    assert!(!sunk.ground.rests_on_ground);
    assert_eq!(sunk.ground.lowest_y, -0.5);
    assert!(sunk.summary.contains("sinks 0.50 into the ground"));
    let warning = only(&sunk.warnings, Code::OffGround);
    assert_eq!(warning.message, "the model sinks 0.50 into the ground");
    assert_eq!(warning.file, "test.toml");
    assert!(
        sunk.to_text()
            .contains("  ground       sinks 0.50 into the ground\n")
    );

    let lifted = report(
        "version = 0\norigin = \"under\"\n[palette]\nred = \"#D0453A\"\n[sockets]\nunder = { at = [0, -2, 0] }\n[[parts]]\nshape = \"box\"\ncolor = \"red\"\n",
    );
    assert_eq!(lifted.ground.lowest_y, 1.5);
    assert_eq!(
        only(&lifted.warnings, Code::OffGround).message,
        "the model floats 1.50 above the ground"
    );

    // Within a millimetre counts as resting.
    let nearly = report(
        "version = 0\norigin = \"under\"\n[palette]\nred = \"#D0453A\"\n[sockets]\nunder = { at = [0, -0.5005, 0] }\n[[parts]]\nshape = \"box\"\ncolor = \"red\"\n",
    );
    assert!(nearly.ground.rests_on_ground);
    assert_eq!(nearly.warnings, Vec::new());
}

// ---- floating parts ----------------------------------------------------------

#[test]
fn a_part_that_touches_nothing_floats() {
    // `b` hovers a quarter of a metre above `a`.
    let found = report(&boxes(&part(
        "b",
        "shape = \"box\"\nsize = [0.5, 0.5, 0.5]",
        [0.0, 1.0, 0.0],
    )));
    assert_eq!(
        found.floating,
        [Floating {
            part: "b".to_owned(),
            nearest: "a".to_owned(),
            gap: 0.25
        }]
    );
    assert_eq!(found.overlapping, Vec::new());
    // The warning points at the part's line in the recipe.
    let warning = only(&found.warnings, Code::Floating);
    assert_eq!(
        warning.message,
        "the part \"b\" touches nothing: it floats 0.25 from \"a\""
    );
    assert_eq!(
        (warning.file.as_str(), warning.line, warning.path.as_str()),
        ("test.toml", Some(9), "parts[1]")
    );
    assert!(
        found
            .summary
            .ends_with("1 part floating, nothing overlapping"),
        "{}",
        found.summary
    );
    assert!(
        found
            .to_text()
            .contains("  floating     b floats 0.25 from a\n")
    );
    let json = serde_json::to_value(&found).unwrap();
    assert_eq!(
        json["floating"][0],
        serde_json::json!({ "part": "b", "nearest": "a", "gap": 0.25 })
    );
}

#[test]
fn the_gap_is_measured_between_the_surfaces_not_the_boxes() {
    // Corner to corner: 0.3 across and 0.4 up is 0.5 apart.
    let found = report(&boxes(&part("b", "shape = \"box\"", [1.3, 1.4, 0.0])));
    assert_eq!(found.floating[0].gap, 0.5);

    // A ball beside a box: nearer than their boxes' corners suggest.
    let found = report(&boxes(&part(
        "ball",
        "shape = \"sphere\"\nradius = 0.5",
        [1.25, 0.0, 0.0],
    )));
    assert_eq!(found.floating[0].gap, 0.25);
}

#[test]
fn parts_within_a_millimetre_touch() {
    let near = report(&boxes(&part("b", "shape = \"box\"", [1.0005, 0.0, 0.0])));
    assert_eq!(near.floating, Vec::new());
    let apart = report(&boxes(&part("b", "shape = \"box\"", [1.002, 0.0, 0.0])));
    assert_eq!(apart.floating.len(), 1);
    assert_eq!(apart.floating[0].gap, 0.002);
    // Parts that share only an edge or a corner touch too.
    let edge = report(&boxes(&part("b", "shape = \"box\"", [1.0, 1.0, 0.0])));
    assert_eq!(edge.floating, Vec::new());
    let corner = report(&boxes(&part("b", "shape = \"box\"", [1.0, 1.0, 1.0])));
    assert_eq!(corner.floating, Vec::new());
}

#[test]
fn everything_joined_to_the_first_part_is_the_body() {
    // a - b - c in a row: c touches a only through b. All one body.
    let chain = boxes(
        &(part("b", "shape = \"box\"", [1.0, 0.0, 0.0])
            + &part("c", "shape = \"box\"", [2.0, 0.0, 0.0])),
    );
    assert_eq!(report(&chain).floating, Vec::new());

    // d and e touch each other but nothing else: both float, and each
    // names the nearest part of the body.
    let pair = boxes(
        &(part("d", "shape = \"box\"", [0.0, 3.0, 0.0])
            + &part("e", "shape = \"box\"", [1.0, 3.0, 0.0])),
    );
    let found = report(&pair);
    assert_eq!(
        found.floating,
        [
            Floating {
                part: "d".to_owned(),
                nearest: "a".to_owned(),
                gap: 2.0
            },
            Floating {
                part: "e".to_owned(),
                nearest: "a".to_owned(),
                gap: 2.0
            },
        ]
    );
    assert!(found.summary.contains("2 parts floating"));
}

#[test]
fn parts_joined_by_sockets_never_float() {
    let found = report(&boxes(
        "[[parts]]\nname = \"b\"\nshape = \"cone\"\ncolor = \"red\"\nattach = \"bottom\"\nto = \"a.top\"\n[[parts]]\nname = \"c\"\nshape = \"sphere\"\nradius = 0.2\ncolor = \"red\"\nattach = \"left\"\nto = \"a.right\"\n",
    ));
    assert_eq!(found.floating, Vec::new());
    assert_eq!(found.overlapping, Vec::new());
    assert_eq!(found.warnings, Vec::new());
}

#[test]
fn a_floating_part_inside_an_assembly_is_named_by_its_path() {
    let found = report(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"body\"\nshape = \"box\"\ncolor = \"red\"\n[[parts]]\nname = \"arm\"\ncolor = \"red\"\nattach = \"left\"\nto = \"body.right\"\nparts = [{ name = \"upper\", shape = \"box\", size = [0.2, 0.2, 0.2] }, { name = \"hand\", shape = \"box\", size = [0.1, 0.1, 0.1], move = [0.5, 0, 0] }]\n",
    );
    assert_eq!(found.floating.len(), 1);
    assert_eq!(found.floating[0].part, "arm/hand");
    assert_eq!(found.floating[0].nearest, "arm/upper");
    assert_eq!(found.floating[0].gap, 0.35);
}

// ---- overlapping parts -------------------------------------------------------

#[test]
fn a_part_pushed_into_another_overlaps_it() {
    // A post standing 0.2 deep in the top of `a`.
    let found = report(&boxes(&part(
        "post",
        "shape = \"box\"\nsize = [0.2, 1, 0.2]",
        [0.0, 0.8, 0.0],
    )));
    assert_eq!(
        found.overlapping,
        [Overlap {
            a: "post".to_owned(),
            b: "a".to_owned(),
            depth: 0.2,
            hidden: false
        }]
    );
    assert_eq!(found.floating, Vec::new());
    // Overlap is often meant, so it is a fact in the report, not a warning.
    assert_eq!(found.warnings, Vec::new());
    assert!(
        found.summary.ends_with("nothing floating, 1 overlap"),
        "{}",
        found.summary
    );
    assert!(
        found
            .to_text()
            .contains("  overlapping  post is 0.20 inside a\n")
    );
    let json = serde_json::to_value(&found).unwrap();
    assert_eq!(
        json["overlapping"][0],
        serde_json::json!({ "a": "post", "b": "a", "depth": 0.2, "hidden": false })
    );
}

#[test]
fn parts_that_only_touch_do_not_overlap() {
    // Side by side, sharing a whole face.
    let found = report(&boxes(&part("b", "shape = \"box\"", [1.0, 0.0, 0.0])));
    assert_eq!(found.overlapping, Vec::new());
    assert_eq!(found.floating, Vec::new());
    // Less than a millimetre in doesn't count.
    let found = report(&boxes(&part("b", "shape = \"box\"", [0.9995, 0.0, 0.0])));
    assert_eq!(found.overlapping, Vec::new());
}

#[test]
fn a_part_entirely_inside_another_is_hidden() {
    let found = report(&boxes(&part(
        "gem",
        "shape = \"box\"\nsize = [0.2, 0.2, 0.2]",
        [0.0, 0.0, 0.0],
    )));
    assert_eq!(found.overlapping.len(), 1);
    let overlap = &found.overlapping[0];
    assert_eq!(
        (overlap.a.as_str(), overlap.b.as_str(), overlap.hidden),
        ("gem", "a", true)
    );
    assert_eq!(overlap.depth, 0.5);
    // It doesn't float: it is inside the body.
    assert_eq!(found.floating, Vec::new());
    let warning = only(&found.warnings, Code::Hidden);
    assert_eq!(
        warning.message,
        "the part \"gem\" is hidden inside \"a\": nobody will see it"
    );
    assert_eq!(warning.line, Some(9));
    assert!(
        found
            .to_text()
            .contains("  overlapping  gem is hidden inside a\n")
    );
}

#[test]
fn a_part_put_exactly_on_a_copy_of_itself_is_noticed() {
    let found = report(&boxes(&part("twin", "shape = \"box\"", [0.0, 0.0, 0.0])));
    assert_eq!(found.overlapping.len(), 1);
    assert!(found.overlapping[0].hidden);
    assert_eq!(found.overlapping[0].depth, 0.5);
}

#[test]
fn crossed_bars_overlap_though_no_corner_is_inside_the_other() {
    let found = report(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"across\"\nshape = \"box\"\nsize = [1, 0.1, 0.1]\ncolor = \"red\"\n[[parts]]\nname = \"along\"\nshape = \"box\"\nsize = [0.1, 0.1, 1]\ncolor = \"red\"\n",
    );
    assert_eq!(found.overlapping.len(), 1);
    assert_eq!(found.overlapping[0].depth, 0.05);
    assert!(!found.overlapping[0].hidden);
}

#[test]
fn round_parts_are_measured_too() {
    // A ball half sunk into the top of `a`.
    let found = report(&boxes(&part(
        "ball",
        "shape = \"sphere\"\nradius = 0.25",
        [0.0, 0.5, 0.0],
    )));
    assert_eq!(found.overlapping.len(), 1);
    let overlap = &found.overlapping[0];
    assert_eq!((overlap.a.as_str(), overlap.b.as_str()), ("ball", "a"));
    // Its lowest point is 0.25 below the top of `a`.
    assert_eq!(overlap.depth, 0.25);
}

// ---- colours -----------------------------------------------------------------

#[test]
fn faces_nobody_coloured_are_counted() {
    let found = report(
        "version = 0\n[palette]\nred = \"#D0453A\"\nblue = \"#3050C0\"\n[[parts]]\nshape = \"box\"\ncolors = { top = \"red\" }\n",
    );
    assert_eq!(found.unpainted_triangles, 10);
    // Every palette colour is listed, used or not.
    let counts: Vec<(&str, usize)> = found
        .palette
        .iter()
        .map(|c| (c.name.as_str(), c.triangles))
        .collect();
    assert_eq!(counts, [("red", 2), ("blue", 0)]);
    assert!(found.summary.contains(", 1 colour, "));
    assert!(
        found
            .to_text()
            .contains("  colours      1:  red 2, (no colour) 10  (triangles each)\n")
    );
    assert_eq!(
        serde_json::to_value(&found).unwrap()["unpainted_triangles"],
        10
    );
    // The warnings found while building are carried into the report.
    let codes: Vec<Code> = found.warnings.iter().map(|issue| issue.code).collect();
    assert_eq!(codes, [Code::NoColor, Code::Unused]);
}

// ---- the operations behind the commands --------------------------------------

#[test]
fn export_writes_a_glb_that_inspect_can_measure() {
    let folder = folder("inspect_export");
    std::fs::write(folder.join("robot.toml"), ROBOT).unwrap();
    let exported = tools::export_file(&folder.join("robot.toml"), None).unwrap();
    // Next to the recipe, with the same name.
    assert!(
        exported.output.ends_with("inspect_export/robot.glb"),
        "{}",
        exported.output
    );
    let bytes = std::fs::read(folder.join("robot.glb")).unwrap();
    assert_eq!(exported.bytes, bytes.len());
    assert_eq!(exported.inspection.triangles, 168);
    // As JSON: the report, then the file.
    let json = serde_json::to_value(&exported).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["triangles"], 168);
    assert_eq!(json["bytes"], bytes.len());
    assert!(json["output"].as_str().unwrap().ends_with("robot.glb"));

    // Inspecting the file that was written gives the same measurements.
    let from_recipe = tools::inspect_file(&folder.join("robot.toml")).unwrap();
    let from_glb = tools::inspect_file(&folder.join("robot.glb")).unwrap();
    assert_eq!(from_glb.name, "robot");
    assert_eq!(from_glb.size, from_recipe.size);
    assert_eq!(from_glb.bounds, from_recipe.bounds);
    assert_eq!(from_glb.triangles, 168);
    assert_eq!(from_glb.vertices, 288);
    assert_eq!(from_glb.palette, from_recipe.palette);
    assert_eq!(from_glb.ground, from_recipe.ground);
    assert_eq!(from_glb.floating, Vec::new());
    assert_eq!(from_glb.overlapping, Vec::new());
    // The parts are there, as a flat list.
    let parts: Vec<(&str, usize)> = from_glb
        .parts
        .iter()
        .map(|part| (part.name.as_str(), part.triangles))
        .collect();
    assert_eq!(parts[0], ("body", 12));
    assert_eq!(parts.len(), 6);

    // Somewhere else, if asked.
    let exported =
        tools::export_file(&folder.join("robot.toml"), Some(&folder.join("out.glb"))).unwrap();
    assert!(exported.output.ends_with("out.glb"));
    assert_eq!(std::fs::read(folder.join("out.glb")).unwrap(), bytes);
}

#[test]
fn a_recipe_with_mistakes_writes_nothing_and_exits_with_1() {
    let folder = folder("inspect_invalid");
    std::fs::write(
        folder.join("bad.toml"),
        "version = 0\n[[parts]]\nshape = \"blob\"\n",
    )
    .unwrap();
    let failure = tools::export_file(&folder.join("bad.toml"), None).unwrap_err();
    assert_eq!(failure.exit_code(), 1);
    let report = failure.report();
    assert!(!report.ok);
    assert_eq!(report.errors[0].code, Code::UnknownShape);
    assert!(!folder.join("bad.glb").exists());
    assert!(matches!(
        tools::inspect_file(&folder.join("bad.toml")),
        Err(Failure::Invalid(_))
    ));
}

#[test]
fn a_command_that_cannot_be_carried_out_exits_with_2() {
    let folder = folder("inspect_usage");
    std::fs::write(
        folder.join("good.toml"),
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nshape = \"box\"\ncolor = \"red\"\n",
    )
    .unwrap();
    std::fs::write(folder.join("notes.txt"), "hello").unwrap();

    let missing = tools::inspect_file(&folder.join("nowhere.toml")).unwrap_err();
    assert_eq!(missing.exit_code(), 2);
    assert!(
        missing.report().errors[0]
            .message
            .contains("there is no such file")
    );

    let wrong_kind = tools::inspect_file(&folder.join("notes.txt")).unwrap_err();
    assert_eq!(wrong_kind.exit_code(), 2);
    assert!(
        wrong_kind.report().errors[0]
            .message
            .contains("recipe (.toml or .json)")
    );

    let not_a_recipe = tools::export_file(&folder.join("notes.txt"), None).unwrap_err();
    assert_eq!(not_a_recipe.exit_code(), 2);
    assert!(
        not_a_recipe.report().errors[0]
            .message
            .contains("export needs a recipe")
    );

    let no_folder = tools::export_file(
        &folder.join("good.toml"),
        Some(&folder.join("no/such/folder/x.glb")),
    )
    .unwrap_err();
    assert_eq!(no_folder.exit_code(), 2);
    assert!(
        no_folder.report().errors[0]
            .message
            .contains("that folder doesn't exist")
    );

    let gltf = tools::inspect_file(&folder.join("duck.gltf")).unwrap_err();
    assert_eq!(gltf.exit_code(), 2);
    assert_eq!(gltf.report().errors[0].code, Code::Unsupported);
}
