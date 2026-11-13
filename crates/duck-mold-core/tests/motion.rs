//! Phase 5: models that move. A join may say how far it turns
//! (`limits`); poses and animations come in the later steps.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::edit::{Applied, Edit, LimitsTo, apply, describe};
use duck_mold_core::inspect::join_report;
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load_text;
use duck_mold_core::model::{Model, Part};
use duck_mold_core::recipe::{Limits, Turns};
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::Failure;

/// A puppet: a body, a head on its neck, and one arm (an upper and a
/// lower part) used twice, the right one a mirror image.
const PUPPET: &str = r##"version = 0
name = "puppet"

[palette]
wood = "#B08050"

[define.arm]
color = "wood"

[define.arm.sockets]
shoulder = { from = "upper.joint" }

[[define.arm.parts]]
name = "upper"
shape = "box"
size = [0.1, 0.3, 0.1]
sockets = { joint = { on = "right", shift = [0, 0.8, 0] } }

[[define.arm.parts]]
name = "lower"
shape = "box"
size = [0.08, 0.3, 0.08]
attach = "top"
to = "upper.bottom"

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
"##;

fn build(text: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, issues)
}

fn built(text: &str) -> Model {
    let (model, issues) = build(text);
    let errors: Vec<&Issue> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.expect("a recipe without errors builds")
}

fn errors(text: &str) -> Vec<Issue> {
    let (model, issues) = build(text);
    assert!(model.is_none(), "a recipe with errors must not build");
    Report::from_issues(issues).errors
}

fn part<'a>(model: &'a Model, path: &str) -> &'a Part {
    model.part(path).unwrap_or_else(|| panic!("no part {path}"))
}

/// The puppet with a line added to the part whose `to` is given.
fn with_line(after: &str, line: &str) -> String {
    assert!(PUPPET.contains(after), "{after}");
    PUPPET.replacen(after, &format!("{after}\n{line}"), 1)
}

fn done(text: &str, edit: &Edit) -> Applied {
    match apply(
        text,
        Syntax::Toml,
        Path::new("test.toml"),
        "test.toml",
        edit,
    ) {
        Ok(applied) => applied,
        Err(failure) => panic!("{edit:?} was refused: {:#?}", failure.report()),
    }
}

fn refused(text: &str, edit: &Edit) -> Failure {
    match apply(
        text,
        Syntax::Toml,
        Path::new("test.toml"),
        "test.toml",
        edit,
    ) {
        Ok(applied) => panic!("{edit:?} should be refused, but gave:\n{}", applied.text),
        Err(failure) => failure,
    }
}

/// The lines only the new text has.
fn added<'a>(old: &str, new: &'a str) -> Vec<&'a str> {
    let old: Vec<&str> = old.lines().collect();
    new.lines().filter(|line| !old.contains(line)).collect()
}

fn limits(part: &str, limits: LimitsTo) -> Edit {
    Edit::SetLimits {
        part: part.into(),
        limits,
    }
}

// ---- step 2: limits -------------------------------------------------------------------

#[test]
fn a_part_without_limits_turns_freely() {
    let model = built(PUPPET);
    for part in model.all_parts() {
        assert_eq!(part.limits, Limits::Free, "{}", part.path);
        assert_eq!(part.limits.turns(), [Turns::Any; 3]);
        assert!(part.limits.may_move());
    }
}

#[test]
fn limits_name_the_directions_that_turn_and_how_far() {
    // A neck: nods and turns, doesn't tilt.
    let model = built(&with_line(
        "to = \"body.neck\"",
        "limits = { x = [-30, 40], y = [-80, 80] }",
    ));
    let neck = part(&model, "head").limits;
    assert_eq!(
        neck,
        Limits::Turns([
            Turns::Between(-30.0, 40.0),
            Turns::Between(-80.0, 80.0),
            Turns::No
        ])
    );
    assert_eq!(neck.describe(), "x -30..40, y -80..80");
    assert!(neck.may_move());
    // How far past a limit an angle is, and the nearest allowed one.
    let [x, _, z] = neck.turns();
    assert_eq!(
        (x.past(40.0), x.past(55.0), x.past(-31.5)),
        (0.0, 15.0, 1.5)
    );
    assert_eq!(
        (x.kept(55.0), x.kept(-90.0), x.kept(12.0)),
        (40.0, -30.0, 12.0)
    );
    assert_eq!((z.past(5.0), z.kept(5.0)), (5.0, 0.0));

    // A wheel turns round and round; a badge never moves; a drawer only
    // slides.
    let wheel = built(&with_line(
        "to = \"body.neck\"",
        "limits = { x = \"free\" }",
    ));
    let wheel = part(&wheel, "head").limits;
    assert_eq!(wheel, Limits::Turns([Turns::Any, Turns::No, Turns::No]));
    assert_eq!(wheel.describe(), "x free");
    assert_eq!(wheel.turns()[0].past(100_000.0), 0.0);
    let badge = built(&with_line("to = \"body.neck\"", "limits = \"fixed\""));
    let badge = part(&badge, "head").limits;
    assert_eq!(badge, Limits::Fixed);
    assert_eq!(badge.turns(), [Turns::No; 3]);
    assert!(!badge.may_move());
    assert_eq!(badge.describe(), "fixed");
    let drawer = built(&with_line("to = \"body.neck\"", "limits = {}"));
    let drawer = part(&drawer, "head").limits;
    assert_eq!(drawer, Limits::Turns([Turns::No; 3]));
    assert!(drawer.may_move());
    assert_eq!(drawer.describe(), "doesn't turn");
}

#[test]
fn limits_inside_a_definition_go_wherever_it_is_used() {
    let model = built(&with_line(
        "to = \"upper.bottom\"",
        "limits = { x = [0, 140] }",
    ));
    let elbow = Limits::Turns([Turns::Between(0.0, 140.0), Turns::No, Turns::No]);
    assert_eq!(part(&model, "left_arm/lower").limits, elbow);
    assert_eq!(part(&model, "right_arm/lower").limits, elbow);
    // The right arm is a mirror image, and so are the directions its
    // parts turn in: the same limit means the mirror image of the bend.
    let (left, right) = (
        part(&model, "left_arm/lower").axes,
        part(&model, "right_arm/lower").axes,
    );
    let x = duck_mold_core::math::vec3(1.0, 0.0, 0.0);
    assert!((left * x - x).length() < 1e-9);
    assert!((right * x + x).length() < 1e-9);
    // The arms themselves are parts of the model: each has its own.
    assert_eq!(part(&model, "left_arm").limits, Limits::Free);
}

#[test]
fn mistakes_in_limits_say_what_to_write() {
    let cases: [(&str, Code, &str, &str); 7] = [
        (
            "limits = \"fixd\"",
            Code::TypeMismatch,
            "parts[1].limits",
            "\"limits\" is \"fixed\" (it never moves), or the directions it may turn, like { x = [-30, 40] }, not \"fixd\"",
        ),
        (
            "limits = 30",
            Code::TypeMismatch,
            "parts[1].limits",
            "this needs \"fixed\" (it never moves), or the directions it may turn, like { x = [-30, 40] }, not the number 30",
        ),
        (
            "limits = { w = [-30, 40] }",
            Code::UnknownField,
            "parts[1].limits.w",
            "a part turns around \"x\" (a nod), \"y\" (a turn) or \"z\" (a tilt), not \"w\"",
        ),
        (
            "limits = { x = [10, 40] }",
            Code::OutOfRange,
            "parts[1].limits.x",
            "the part stands at 0 when the model is at rest, so its limits must have 0 between them: the first 0 or less and the second 0 or more, not [10, 40]",
        ),
        (
            "limits = { x = [-30, 0, 40] }",
            Code::TypeMismatch,
            "parts[1].limits.x",
            "a limit is two angles in degrees, like [-30, 40], or \"free\" for any amount, not a list of 3",
        ),
        (
            "limits = { y = \"loose\" }",
            Code::TypeMismatch,
            "parts[1].limits.y",
            "a limit is two angles in degrees, like [-30, 40], or \"free\" for any amount, not the text \"loose\"",
        ),
        (
            "limits = { z = [-99999, 0] }",
            Code::OutOfRange,
            "parts[1].limits.z",
            "",
        ),
    ];
    for (line, code, field, message) in cases {
        let found = errors(&with_line("to = \"body.neck\"", line));
        assert_eq!(found.len(), 1, "{line}: {found:#?}");
        assert_eq!(found[0].code, code, "{line}");
        assert_eq!(found[0].path, field, "{line}");
        if !message.is_empty() {
            assert_eq!(found[0].message, message, "{line}");
        }
        assert_eq!(found[0].line, Some(44), "{line}");
    }
    // The closest word is offered.
    let found = errors(&with_line("to = \"body.neck\"", "limits = \"fixd\""));
    assert_eq!(found[0].suggestion.as_deref(), Some("fixed"));

    // A definition has no join of its own: its limits go on the part
    // that uses it.
    let found = errors(&PUPPET.replacen(
        "[define.arm]\ncolor = \"wood\"",
        "[define.arm]\ncolor = \"wood\"\nlimits = { x = [-60, 180] }",
        1,
    ));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, Code::Conflict);
    assert_eq!(
        found[0].message,
        "a defined part has no place or size of its own: put \"limits\" on the part that uses it"
    );
}

#[test]
fn set_limits_writes_one_line() {
    let turns = |x, y, z| LimitsTo::Turns([x, y, z]);
    // With no limits yet, the directions given are the ones that turn.
    let neck = done(
        PUPPET,
        &limits(
            "head",
            turns(
                Some(Turns::Between(-30.0, 40.0)),
                Some(Turns::Between(-80.0, 80.0)),
                None,
            ),
        ),
    );
    assert_eq!(
        added(PUPPET, &neck.text),
        ["limits = { x = [-30, 40], y = [-80, 80] }"]
    );
    assert_eq!(neck.text.lines().count(), PUPPET.lines().count() + 1);
    assert_eq!(
        neck.summary,
        "set the limits of \"head\": x -30..40, y -80..80"
    );
    assert_eq!(
        part(neck.model.as_ref().unwrap(), "head").limits.describe(),
        "x -30..40, y -80..80"
    );
    // It goes with the part's place: after its join.
    assert!(
        neck.text.contains("to = \"body.neck\"\nlimits = {"),
        "{}",
        neck.text
    );

    // One direction more, one taken away: the others stay.
    let more = done(
        &neck.text,
        &limits("head", turns(None, None, Some(Turns::Any))),
    );
    assert_eq!(
        added(&neck.text, &more.text),
        ["limits = { x = [-30, 40], y = [-80, 80], z = \"free\" }"]
    );
    let less = done(
        &more.text,
        &limits("head", turns(None, Some(Turns::No), None)),
    );
    assert_eq!(
        added(&more.text, &less.text),
        ["limits = { x = [-30, 40], z = \"free\" }"]
    );
    assert_eq!(less.text.lines().count(), neck.text.lines().count());

    // Fixed, and free again: the line goes, and the file is as it was.
    let fixed = done(&less.text, &limits("head", LimitsTo::Fixed));
    assert_eq!(added(&less.text, &fixed.text), ["limits = \"fixed\""]);
    assert_eq!(
        fixed.summary,
        "fixed \"head\": it never turns or moves in a pose"
    );
    let free = done(&fixed.text, &limits("head", LimitsTo::None));
    assert_eq!(free.text, PUPPET);
    assert_eq!(
        free.summary,
        "took the limits of \"head\" away: it turns freely"
    );
    let again = done(PUPPET, &limits("head", LimitsTo::None));
    assert_eq!(again.text, PUPPET);
    assert_eq!(
        again.summary,
        "\"head\" has no limits to take away: it turns freely already"
    );

    // What the Studio's panel reads back.
    let facts = describe(&neck.text, Syntax::Toml, neck.model.as_ref(), "head").unwrap();
    assert_eq!(facts.limits.describe(), "x -30..40, y -80..80");
}

#[test]
fn set_limits_inside_a_definition_changes_every_part_that_uses_it() {
    let elbow = done(
        PUPPET,
        &limits(
            "left_arm/lower",
            LimitsTo::Turns([Some(Turns::Between(0.0, 140.0)), None, None]),
        ),
    );
    assert_eq!(added(PUPPET, &elbow.text), ["limits = { x = [0, 140] }"]);
    assert!(
        elbow
            .text
            .contains("to = \"upper.bottom\"\nlimits = { x = [0, 140] }"),
        "{}",
        elbow.text
    );
    let model = elbow.model.as_ref().unwrap();
    assert_eq!(part(model, "left_arm/lower").limits.describe(), "x 0..140");
    assert_eq!(part(model, "right_arm/lower").limits.describe(), "x 0..140");
    // And says so.
    let shared = elbow.shared.expect("it is in a definition");
    assert_eq!(shared.definition, "arm");
    assert_eq!(shared.also_changes, ["right_arm/lower"]);
}

#[test]
fn set_limits_refuses_what_makes_no_sense() {
    let failure = refused(
        PUPPET,
        &limits(
            "head",
            LimitsTo::Turns([Some(Turns::Between(10.0, 40.0)), None, None]),
        ),
    );
    let error = &failure.report().errors[0];
    assert_eq!(error.code, Code::OutOfRange);
    assert_eq!(
        error.message,
        "the part stands at 0 when the model is at rest, so the limits around x must have 0 between them: the first 0 or less and the second 0 or more, not 10,40"
    );
    let failure = refused(PUPPET, &limits("head", LimitsTo::Turns([None; 3])));
    assert_eq!(failure.report().errors[0].code, Code::MissingField);
    let failure = refused(PUPPET, &limits("hat", LimitsTo::Fixed));
    assert_eq!(failure.report().errors[0].code, Code::UnknownPart);
    let failure = refused(
        PUPPET,
        &limits(
            "head",
            LimitsTo::Turns([None, Some(Turns::Between(-40_000.0, 0.0)), None]),
        ),
    );
    assert!(
        failure.report().errors[0]
            .message
            .contains("write --y free"),
        "{}",
        failure.report().errors[0].message
    );
}

#[test]
fn set_limits_works_on_json_too() {
    let json = r#"{
  "version": 0,
  "parts": [
    { "name": "base", "shape": "box" },
    { "name": "lid", "shape": "box", "size": [1, 0.1, 1], "attach": "bottom", "to": "base.top" }
  ]
}
"#;
    let applied = apply(
        json,
        Syntax::Json,
        Path::new("test.json"),
        "test.json",
        &limits(
            "lid",
            LimitsTo::Turns([Some(Turns::Between(-110.0, 0.0)), None, None]),
        ),
    )
    .unwrap();
    assert!(
        applied
            .text
            .contains("\"to\": \"base.top\", \"limits\": { \"x\": [-110, 0] }"),
        "{}",
        applied.text
    );
    assert_eq!(
        part(applied.model.as_ref().unwrap(), "lid")
            .limits
            .describe(),
        "x -110..0"
    );
}

#[test]
fn a_mirror_copy_gets_its_limits_mirrored() {
    // The left arm lifts out to its side with a negative turn around z,
    // and swings further forward than back.
    let text = with_line(
        "to = \"body.left_arm\"",
        "limits = { x = [-60, 180], y = [-10, 45], z = [-170, 10] }",
    )
    .replace(
        "\n[[parts]]\nname = \"right_arm\"\nuse = \"arm\"\nmirror = \"x\"\nattach = \"shoulder\"\nto = \"body.right_arm\"\n",
        "",
    );
    let copy = done(
        &text,
        &Edit::Duplicate {
            part: "left_arm".into(),
            name: None,
            to: None,
            mirror: true,
        },
    );
    let model = copy.model.as_ref().unwrap();
    // Around x it is the same; around y and z, the other way.
    assert_eq!(
        part(model, "right_arm").limits.describe(),
        "x -60..180, y -45..10, z -10..170"
    );
    assert_eq!(
        part(model, "left_arm").limits.describe(),
        "x -60..180, y -10..45, z -170..10"
    );
    assert!(
        copy.text
            .contains("limits = { x = [-60, 180], y = [-45, 10], z = [-10, 170] }"),
        "{}",
        copy.text
    );
    // A plain copy keeps them as they are.
    let plain = done(
        &text,
        &Edit::Duplicate {
            part: "head".into(),
            name: None,
            to: Some("body.left_arm".into()),
            mirror: false,
        },
    );
    assert_eq!(
        part(plain.model.as_ref().unwrap(), "head_2").limits,
        Limits::Free
    );
}

#[test]
fn the_joins_are_listed_as_a_tree() {
    let model = built(&with_line(
        "to = \"body.neck\"",
        "limits = { x = [-30, 40], y = [-80, 80] }",
    ));
    let joins = join_report(&model);
    type Row<'a> = (usize, &'a str, Option<&'a str>, Option<&'a str>, &'a str);
    let rows: Vec<Row> = joins
        .iter()
        .map(|join| {
            (
                join.depth,
                join.part.as_str(),
                join.parent.as_deref(),
                join.joined_to.as_deref(),
                join.limits_words.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (0, "body", None, None, "free"),
            (
                1,
                "head",
                Some("body"),
                Some("body.neck"),
                "x -30..40, y -80..80"
            ),
            (1, "left_arm", Some("body"), Some("body.left_arm"), "free"),
            (2, "left_arm/upper", Some("left_arm"), None, "free"),
            (
                3,
                "left_arm/lower",
                Some("left_arm/upper"),
                Some("upper.bottom"),
                "free"
            ),
            (1, "right_arm", Some("body"), Some("body.right_arm"), "free"),
            (2, "right_arm/upper", Some("right_arm"), None, "free"),
            (
                3,
                "right_arm/lower",
                Some("right_arm/upper"),
                Some("upper.bottom"),
                "free"
            ),
        ]
    );
    assert_eq!(
        joins[1].limits,
        serde_json::json!({ "x": [-30.0, 40.0], "y": [-80.0, 80.0] })
    );
    assert_eq!(joins[0].limits, serde_json::json!("free"));
    // A joined part turns about the socket it is joined to.
    let neck = part(&model, "body")
        .sockets
        .iter()
        .find(|socket| socket.name == "neck")
        .unwrap();
    assert_eq!(joins[1].pivot, neck.at.to_array());
}

// ---- step 3: poses ---------------------------------------------------------------------

use duck_mold_core::math::{Vec3, vec3};
use duck_mold_core::motion::{Joint, limits_broken, placements, posed};

/// The puppet with poses written under it.
fn with_poses(poses: &str) -> String {
    format!("{PUPPET}\n{poses}")
}

const POSES: &str = r##"[poses.wave]
left_arm = [0, 0, -150]
"left_arm/lower" = [0, 0, -30]
head = [0, -15, 0]

[poses.crouch]
body = { move = [0, -0.1, 0] }
head = { turn = [20, 0, 0], move = [0, 0, 0.02] }
"##;

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-6
}

/// Every corner of a part of a model.
fn corners(model: &Model, path: &str) -> Vec<Vec3> {
    model.faces[part(model, path).faces.clone()]
        .iter()
        .flat_map(|face| face.corners.iter().copied())
        .collect()
}

fn socket(model: &Model, path: &str, name: &str) -> Vec3 {
    part(model, path)
        .sockets
        .iter()
        .find(|socket| socket.name == name)
        .unwrap_or_else(|| panic!("no socket {path}.{name}"))
        .at
}

#[test]
fn poses_are_read_from_the_recipe() {
    let model = built(&with_poses(POSES));
    let names: Vec<&str> = model.poses.iter().map(|pose| pose.name.as_str()).collect();
    assert_eq!(names, ["wave", "crouch"]);
    let wave = model.pose("wave").unwrap();
    let lines: Vec<(&str, [f64; 3], [f64; 3])> = wave
        .joints
        .iter()
        .map(|joint| (joint.part.as_str(), joint.turn, joint.move_by))
        .collect();
    assert_eq!(
        lines,
        [
            ("left_arm", [0.0, 0.0, -150.0], [0.0; 3]),
            ("left_arm/lower", [0.0, 0.0, -30.0], [0.0; 3]),
            ("head", [0.0, -15.0, 0.0], [0.0; 3]),
        ]
    );
    // The long way of writing a line: a turn, a move, or both.
    let crouch = model.pose("crouch").unwrap();
    assert_eq!(crouch.joints[0].turn, [0.0; 3]);
    assert_eq!(crouch.joints[0].move_by, [0.0, -0.1, 0.0]);
    assert_eq!(crouch.joints[1].turn, [20.0, 0.0, 0.0]);
    assert_eq!(crouch.joints[1].move_by, [0.0, 0.0, 0.02]);
    // `rest` is always there, with nothing in it; other names aren't.
    assert_eq!(model.pose("rest").unwrap().joints, Vec::new());
    assert!(model.pose("jump").is_none());
    // A recipe without poses has none.
    assert!(built(PUPPET).poses.is_empty());
}

#[test]
fn turning_a_part_carries_what_hangs_from_it() {
    let model = built(PUPPET);
    // The body turns a quarter round, about its own middle (it isn't
    // joined): everything goes with it.
    let turned = posed(&model, &[Joint::turned("body", [0.0, 90.0, 0.0])]);
    let middle = part(&model, "body").pivot;
    for path in ["body", "head", "left_arm/upper", "right_arm/lower"] {
        let (before, after) = (corners(&model, path), corners(&turned, path));
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(&after) {
            // A positive turn around y takes forward (+z) to the right
            // (+x): a point at (x, z) from the middle goes to (z, -x).
            let from = *before - middle;
            let wanted = middle + vec3(from.z, from.y, -from.x);
            assert!(near(*after, wanted), "{path}: {after:?} {wanted:?}");
        }
    }

    // The head nods: it turns about the neck, which stays where it is,
    // and nothing else moves.
    let neck = socket(&model, "body", "neck");
    assert_eq!(part(&model, "head").pivot, neck);
    let nod = posed(&model, &[Joint::turned("head", [30.0, 0.0, 0.0])]);
    assert_eq!(part(&nod, "head").pivot, neck);
    assert_eq!(corners(&nod, "body"), corners(&model, "body"));
    assert_eq!(
        corners(&nod, "left_arm/lower"),
        corners(&model, "left_arm/lower")
    );
    assert_ne!(corners(&nod, "head"), corners(&model, "head"));
    // A positive turn around x tips the head back: its top goes back.
    let top = |model: &Model| socket(model, "head", "top");
    assert!(top(&nod).z < top(&model).z - 0.05);
    assert!(top(&nod).y < top(&model).y);

    // The left arm swings forward at the shoulder: the whole arm, the
    // lower part with it, and the shoulder stays put.
    let swing = posed(&model, &[Joint::turned("left_arm", [90.0, 0.0, 0.0])]);
    let shoulder = part(&model, "left_arm").pivot;
    assert_eq!(part(&swing, "left_arm").pivot, shoulder);
    let hand = |model: &Model| socket(model, "left_arm/lower", "bottom");
    let reach = (hand(&model) - shoulder).length();
    // It hung straight down; now it points straight forward, as long as
    // it was.
    assert!(
        near(
            hand(&swing),
            shoulder + vec3(hand(&model).x - shoulder.x, 0.0, reach)
        ) || {
            let now = hand(&swing) - shoulder;
            (now.length() - reach).abs() < 1e-6 && now.z > reach * 0.9
        }
    );
    assert_eq!(
        corners(&swing, "right_arm/lower"),
        corners(&model, "right_arm/lower")
    );

    // The lower arm bends at the elbow: only it moves, about the join.
    let elbow = part(&model, "left_arm/lower").pivot;
    let bend = posed(&model, &[Joint::turned("left_arm/lower", [45.0, 0.0, 0.0])]);
    assert_eq!(part(&bend, "left_arm/lower").pivot, elbow);
    assert_eq!(
        corners(&bend, "left_arm/upper"),
        corners(&model, "left_arm/upper")
    );
    assert!(hand(&bend).z > hand(&model).z + 0.1);
}

#[test]
fn turns_add_up_down_the_tree() {
    // The shoulder a quarter turn forward and the elbow a quarter turn
    // more: the lower arm points straight up.
    let model = built(PUPPET);
    let both = posed(
        &model,
        &[
            Joint::turned("left_arm", [90.0, 0.0, 0.0]),
            Joint::turned("left_arm/lower", [90.0, 0.0, 0.0]),
        ],
    );
    let lower = part(&both, "left_arm/lower").bounds.unwrap();
    let upper = part(&both, "left_arm/upper").bounds.unwrap();
    assert!(lower.size().y > 0.29, "{lower:?}");
    assert!(lower.min.y >= upper.max.y - 0.06, "{lower:?} {upper:?}");
    // The elbow, the lower arm's pivot, went forward with the upper arm.
    let elbow = part(&both, "left_arm/lower").pivot;
    assert!(elbow.z > part(&model, "left_arm/lower").pivot.z + 0.2);
}

#[test]
fn a_mirrored_part_bends_as_a_mirror_image_with_the_same_numbers() {
    let model = built(PUPPET);
    // Each lower arm leans out sideways from its body with the same
    // turn around z, though one arm is the mirror image of the other.
    let out = posed(
        &model,
        &[
            Joint::turned("left_arm/lower", [20.0, 0.0, -40.0]),
            Joint::turned("right_arm/lower", [20.0, 0.0, -40.0]),
        ],
    );
    let (left, right) = (
        corners(&out, "left_arm/lower"),
        corners(&out, "right_arm/lower"),
    );
    // Every corner of the left has its mirror image on the right.
    for corner in &left {
        let image = vec3(-corner.x, corner.y, corner.z);
        assert!(
            right.iter().any(|other| near(*other, image)),
            "{corner:?} has no mirror image"
        );
    }
    // And they did lean out: the left one further left.
    assert!(
        part(&out, "left_arm/lower").bounds.unwrap().min.x
            < part(&model, "left_arm/lower").bounds.unwrap().min.x - 0.05
    );
}

#[test]
fn a_pose_can_move_a_part_and_sockets_go_along() {
    let model = built(&with_poses(POSES));
    let crouch = model.pose("crouch").unwrap();
    let low = posed(&model, &crouch.joints);
    // The body sinks 10 cm, and everything that hangs from it.
    for path in ["body", "left_arm/upper", "right_arm/lower"] {
        let (before, after) = (
            part(&model, path).bounds.unwrap(),
            part(&low, path).bounds.unwrap(),
        );
        assert!((after.min.y - (before.min.y - 0.1)).abs() < 1e-6, "{path}");
        assert!(near(after.size(), before.size()), "{path}");
    }
    // The head goes down with the body, forward a little, and nods.
    let head = part(&low, "head");
    assert!(near(
        head.pivot,
        part(&model, "head").pivot + vec3(0.0, -0.1, 0.02)
    ));
    // A socket is where its part is now, facing the way the part does.
    let neck = |model: &Model| socket(model, "body", "neck");
    assert!(near(neck(&low), neck(&model) + vec3(0.0, -0.1, 0.0)));
    let top = part(&low, "head")
        .sockets
        .iter()
        .find(|socket| socket.name == "top")
        .unwrap();
    assert!(
        top.facing.z < -0.3 && top.facing.y > 0.9,
        "{:?}",
        top.facing
    );
    // A socket the arm passes on from the part inside it goes with that
    // part.
    let wave = posed(&model, &model.pose("wave").unwrap().joints);
    let shoulder = |model: &Model| socket(model, "left_arm", "shoulder");
    assert!(
        near(shoulder(&wave), shoulder(&model)),
        "the shoulder stays"
    );
    // At rest, nothing changes at all.
    assert_eq!(posed(&model, &[]), model);
    assert_eq!(posed(&model, &[Joint::turned("head", [0.0; 3])]), model);
}

#[test]
fn posing_never_changes_a_parts_shape_and_joined_parts_stay_joined() {
    // Every example, in poses made of made-up numbers: each part keeps
    // its shape and size, and a part still meets the part it is joined
    // to at the join.
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seed = 0x5EED_u64;
    let mut random = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
    };
    for name in [
        "robot", "dog", "snowman", "house", "gate", "desk", "duck", "temple",
    ] {
        let model = duck_mold_core::tools::build(&folder.join(format!("{name}.toml")))
            .unwrap()
            .model;
        let all = model.all_parts();
        for _ in 0..4 {
            let mut joints: Vec<Joint> = Vec::new();
            for part in &all {
                // Most parts, not all.
                if random() < -0.3 {
                    continue;
                }
                joints.push(Joint {
                    part: part.path.clone(),
                    loc: Default::default(),
                    turn: [random() * 120.0, random() * 120.0, random() * 120.0],
                    move_by: [random() * 0.2, random() * 0.2, random() * 0.2],
                });
            }
            let placed = placements(&model, &joints);
            let moved = posed(&model, &joints);
            for (at, part) in all.iter().enumerate() {
                // Rigid: every distance between two corners is as it
                // was.
                if part.parts.is_empty() {
                    let (before, after) =
                        (corners(&model, &part.path), corners(&moved, &part.path));
                    for step in [1, 7] {
                        for (a, b) in (0..before.len()).zip((step..before.len()).chain(0..step)) {
                            let (was, is) = (
                                (before[a] - before[b]).length(),
                                (after[a] - after[b]).length(),
                            );
                            assert!(
                                (was - is).abs() < 1e-5,
                                "{name}: {} changed shape",
                                part.path
                            );
                        }
                    }
                }
                // Joined: a part's pivot goes where the part it hangs
                // from takes it, moved by the part's own move.
                let Some(parent) = part
                    .parent
                    .as_deref()
                    .and_then(|path| all.iter().position(|other| other.path == path))
                else {
                    continue;
                };
                let own_move = joints
                    .iter()
                    .find(|joint| joint.part == part.path)
                    .map_or(Vec3::ZERO, |joint| part.axes * Vec3::from(joint.move_by));
                let wanted = placed[parent].apply(part.pivot + own_move);
                let found = placed[at].apply(part.pivot);
                assert!(
                    (wanted - found).length() < 1e-6,
                    "{name}: {} came off {}",
                    part.path,
                    all[parent].path
                );
            }
            // Nothing is lost or made.
            assert_eq!(moved.triangle_count(), model.triangle_count());
        }
    }
}

#[test]
fn mistakes_in_poses_say_what_to_write() {
    let one = |poses: &str| {
        let found = errors(&with_poses(poses));
        assert_eq!(found.len(), 1, "{poses}: {found:#?}");
        found[0].clone()
    };
    // A part that isn't there, with the closest name.
    let issue = one("[poses.wave]\nleft_amr = [0, 0, -150]\n");
    assert_eq!(issue.code, Code::UnknownPart);
    assert_eq!(
        issue.message,
        "the pose \"wave\" turns \"left_amr\", but the model has no part with that name. A part inside another is written with its path, like \"left_arm/elbow\""
    );
    assert_eq!(issue.suggestion.as_deref(), Some("left_arm"));
    assert_eq!(issue.path, "poses.wave.left_amr");
    // A part inside another needs its path.
    let issue = one("[poses.wave]\nlower = [0, 0, 30]\n");
    assert_eq!(issue.code, Code::UnknownPart);
    assert!(issue.allowed.is_some());

    let issue = one("[poses.rest]\nhead = [0, 30, 0]\n");
    assert_eq!(issue.code, Code::Reserved);
    assert_eq!(
        issue.message,
        "\"rest\" is the model as the recipe builds it, and is always there: it can't be written as a pose. Give this pose another name"
    );
    let issue = one("[poses.\"my pose\"]\nhead = [0, 30, 0]\n");
    assert_eq!(
        issue.message,
        "\"my pose\" can't be a name: use letters, digits, \"_\" and \"-\" only"
    );
    let issue = one("[poses.wave]\nhead = 30\n");
    assert_eq!(
        issue.message,
        "a part in a pose is its turn, three numbers in degrees like [0, 30, 0], or { turn = ..., move = ... }, not the number 30"
    );
    let issue = one("[poses.wave]\nhead = [0, 30]\n");
    assert_eq!(
        issue.message,
        "this needs three numbers, like [0, 30, 0], not a list of 2"
    );
    let issue = one("[poses.wave]\nhead = {}\n");
    assert_eq!(issue.code, Code::MissingField);
    assert_eq!(
        issue.message,
        "a part in a pose needs \"turn\", \"move\" or both, like { turn = [0, 30, 0] }"
    );
    let issue = one("[poses.wave]\nhead = { turn = [0, 30, 0], spin = 2 }\n");
    assert_eq!(issue.code, Code::UnknownField);
    assert_eq!(issue.path, "poses.wave.head.spin");
    let issue = one("[poses.wave]\nhead = { turn = [0, 99999, 0] }\n");
    assert_eq!(issue.code, Code::OutOfRange);
    // Not a table of poses at all.
    let found = errors(&PUPPET.replacen("name = \"puppet\"", "name = \"puppet\"\nposes = 3", 1));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, Code::TypeMismatch);
    assert_eq!(
        found[0].message,
        "this needs a table of poses by name, like [poses.wave], not the number 3"
    );
}

#[test]
fn a_pose_past_a_limit_is_a_warning_and_the_model_still_builds() {
    let text = with_line(
        "to = \"body.neck\"",
        "limits = { x = [-30, 40], y = [-80, 80] }",
    ) + "\n[poses.look]\nhead = [50, -95, 10]\n\n[poses.fine]\nhead = [40, -80, 0]\n";
    let (model, issues) = build(&text);
    let model = model.expect("a broken limit doesn't stop the model");
    assert_eq!(model.poses.len(), 2);
    let warnings: Vec<&Issue> = issues
        .iter()
        .filter(|issue| issue.code == Code::LimitBroken)
        .collect();
    let messages: Vec<&str> = warnings
        .iter()
        .map(|issue| issue.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "the pose \"look\" turns \"head\" 50 around x: its limit is -30 to 40",
            "the pose \"look\" turns \"head\" -95 around y: its limit is -80 to 80",
            "the pose \"look\" turns \"head\" 10 around z: it doesn't turn around z",
        ]
    );
    assert!(warnings.iter().all(|issue| !issue.is_error()));
    assert_eq!(warnings[0].path, "poses.look.head");
    // The same, asked of the model.
    let broken = limits_broken(&model, &model.pose("look").unwrap().joints);
    assert_eq!(broken.len(), 3);
    assert!(limits_broken(&model, &model.pose("fine").unwrap().joints).is_empty());

    // A fixed part neither turns nor moves.
    let text = with_line("to = \"body.neck\"", "limits = \"fixed\"")
        + "\n[poses.look]\nhead = { turn = [0, 5, 0], move = [0, 0.1, 0] }\n";
    let (_, issues) = build(&text);
    let messages: Vec<&str> = issues
        .iter()
        .filter(|issue| issue.code == Code::LimitBroken)
        .map(|issue| issue.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "the pose \"look\" turns \"head\" 5 around y: it doesn't turn around y",
            "the pose \"look\" moves \"head\", which is fixed",
        ]
    );
}

// ---- step 3: the edits that change poses -----------------------------------------------

fn set_pose(pose: &str, part: &str, turn: [f64; 3]) -> Edit {
    Edit::SetPose {
        pose: pose.into(),
        part: part.into(),
        turn: Some(turn),
        move_by: None,
        rest: false,
    }
}

/// The lines only the old text has.
fn lost<'a>(old: &'a str, new: &str) -> Vec<&'a str> {
    let new: Vec<&str> = new.lines().collect();
    old.lines().filter(|line| !new.contains(line)).collect()
}

#[test]
fn set_pose_makes_the_pose_and_then_changes_one_line() {
    // The first line of a pose makes its table, at the end of the file.
    let first = done(PUPPET, &set_pose("wave", "left_arm", [0.0, 0.0, -150.0]));
    assert_eq!(
        first.text,
        format!("{PUPPET}\n[poses.wave]\nleft_arm = [0, 0, -150]\n")
    );
    assert_eq!(
        first.summary,
        "turned \"left_arm\" to [0, 0, -150] in the pose \"wave\", a new pose"
    );
    // A part inside another is written with its path, in quotes.
    let second = done(
        &first.text,
        &set_pose("wave", "left_arm/lower", [0.0, 0.0, -30.0]),
    );
    assert_eq!(
        second.text,
        format!("{}\"left_arm/lower\" = [0, 0, -30]\n", first.text)
    );
    assert_eq!(
        second.summary,
        "turned \"left_arm/lower\" to [0, 0, -30] in the pose \"wave\""
    );
    let model = second.model.as_ref().unwrap();
    assert_eq!(model.pose("wave").unwrap().joints.len(), 2);

    // Changed: only its own line.
    let third = done(
        &second.text,
        &set_pose("wave", "left_arm", [10.0, 0.0, -140.0]),
    );
    assert_eq!(
        added(&second.text, &third.text),
        ["left_arm = [10, 0, -140]"]
    );
    assert_eq!(lost(&second.text, &third.text), ["left_arm = [0, 0, -150]"]);

    // A move: the line becomes a table; a turn of nothing isn't written.
    let moved = done(
        &third.text,
        &Edit::SetPose {
            pose: "wave".into(),
            part: "left_arm".into(),
            turn: None,
            move_by: Some([0.0, 0.05, 0.0]),
            rest: false,
        },
    );
    assert_eq!(
        added(&third.text, &moved.text),
        ["left_arm = { turn = [10, 0, -140], move = [0, 0.05, 0] }"]
    );
    assert_eq!(
        moved.summary,
        "moved \"left_arm\" by [0, 0.05, 0] in the pose \"wave\""
    );
    let only_move = done(
        PUPPET,
        &Edit::SetPose {
            pose: "crouch".into(),
            part: "body".into(),
            turn: None,
            move_by: Some([0.0, -0.1, 0.0]),
            rest: false,
        },
    );
    assert!(
        only_move
            .text
            .ends_with("[poses.crouch]\nbody = { move = [0, -0.1, 0] }\n")
    );

    // Back at rest: the line goes. The pose stays, even with nothing in
    // it.
    let rest = |part: &str| Edit::SetPose {
        pose: "wave".into(),
        part: part.into(),
        turn: None,
        move_by: None,
        rest: true,
    };
    let one_left = done(&second.text, &rest("left_arm/lower"));
    assert_eq!(one_left.text, first.text);
    assert_eq!(
        one_left.summary,
        "\"left_arm/lower\" is at rest in the pose \"wave\" again"
    );
    let none_left = done(&one_left.text, &rest("left_arm"));
    assert_eq!(none_left.text, format!("{PUPPET}\n[poses.wave]\n"));
    assert_eq!(none_left.model.as_ref().unwrap().poses.len(), 1);
    // Turned to nothing is the same as put at rest.
    let zero = done(&first.text, &set_pose("wave", "left_arm", [0.0; 3]));
    assert_eq!(zero.text, none_left.text);

    // A second pose goes after the first.
    let two = done(&second.text, &set_pose("nod", "head", [20.0, 0.0, 0.0]));
    assert_eq!(
        two.text,
        format!("{}\n[poses.nod]\nhead = [20, 0, 0]\n", second.text)
    );
}

#[test]
fn set_pose_refuses_what_makes_no_sense() {
    let code = |edit: &Edit| refused(PUPPET, edit).report().errors[0].clone();
    // A part that isn't there, with the closest name.
    let error = code(&set_pose("wave", "left_amr", [0.0, 0.0, 10.0]));
    assert_eq!(error.code, Code::UnknownPart);
    // `rest` isn't a pose to change.
    let error = code(&set_pose("rest", "head", [0.0, 10.0, 0.0]));
    assert_eq!(error.code, Code::Reserved);
    assert!(
        error.message.contains("change the parts themselves"),
        "{}",
        error.message
    );
    let error = code(&set_pose("my pose", "head", [0.0, 10.0, 0.0]));
    assert_eq!(error.code, Code::TypeMismatch);
    // Nothing to set.
    let error = code(&Edit::SetPose {
        pose: "wave".into(),
        part: "head".into(),
        turn: None,
        move_by: None,
        rest: false,
    });
    assert_eq!(error.code, Code::MissingField);
    // Putting a part at rest in a pose that isn't there.
    let error = code(&Edit::SetPose {
        pose: "wave".into(),
        part: "head".into(),
        turn: None,
        move_by: None,
        rest: true,
    });
    assert_eq!(error.code, Code::UnknownPose);
    let error = code(&set_pose("wave", "head", [0.0, 40_000.0, 0.0]));
    assert_eq!(error.code, Code::OutOfRange);
}

#[test]
fn poses_are_added_copied_renamed_and_removed() {
    let text = with_poses(POSES);
    // A copy, to change.
    let copy = done(
        &text,
        &Edit::AddPose {
            name: "wave_out".into(),
            from: Some("wave".into()),
        },
    );
    assert_eq!(
        copy.text,
        format!(
            "{text}\n[poses.wave_out]\nleft_arm = [0, 0, -150]\n\"left_arm/lower\" = [0, 0, -30]\nhead = [0, -15, 0]\n"
        )
    );
    assert_eq!(
        copy.summary,
        "added the pose \"wave_out\", a copy of \"wave\""
    );
    let model = copy.model.as_ref().unwrap();
    assert_eq!(
        model.pose("wave_out").unwrap().joints,
        model
            .pose("wave")
            .unwrap()
            .joints
            .into_iter()
            .zip(&model.pose("wave_out").unwrap().joints)
            .map(|(old, new)| Joint {
                loc: new.loc.clone(),
                ..old
            })
            .collect::<Vec<_>>()
    );
    // An empty one.
    let empty = done(
        &text,
        &Edit::AddPose {
            name: "stand".into(),
            from: None,
        },
    );
    assert_eq!(empty.text, format!("{text}\n[poses.stand]\n"));
    // A name that is taken.
    let failure = refused(
        &text,
        &Edit::AddPose {
            name: "wave".into(),
            from: None,
        },
    );
    assert_eq!(failure.report().errors[0].code, Code::DuplicateName);
    let failure = refused(
        &text,
        &Edit::AddPose {
            name: "hello".into(),
            from: Some("wav".into()),
        },
    );
    let error = &failure.report().errors[0];
    assert_eq!(error.code, Code::UnknownPose);
    assert_eq!(error.suggestion.as_deref(), Some("wave"));

    // Renamed where it stands: one line changes.
    let renamed = done(
        &text,
        &Edit::RenamePose {
            pose: "wave".into(),
            new_name: "hello".into(),
        },
    );
    assert_eq!(added(&text, &renamed.text), ["[poses.hello]"]);
    assert_eq!(lost(&text, &renamed.text), ["[poses.wave]"]);
    assert_eq!(renamed.summary, "renamed the pose \"wave\" to \"hello\"");
    let failure = refused(
        &text,
        &Edit::RenamePose {
            pose: "wave".into(),
            new_name: "crouch".into(),
        },
    );
    assert_eq!(failure.report().errors[0].code, Code::DuplicateName);

    // Removed: the first with the blank line above it, the last with
    // nothing left behind.
    let removed = done(
        &text,
        &Edit::RemovePose {
            pose: "wave".into(),
        },
    );
    assert_eq!(
        removed.text,
        format!(
            "{PUPPET}\n[poses.crouch]\nbody = {{ move = [0, -0.1, 0] }}\nhead = {{ turn = [20, 0, 0], move = [0, 0, 0.02] }}\n"
        )
    );
    let both = done(
        &removed.text,
        &Edit::RemovePose {
            pose: "crouch".into(),
        },
    );
    assert_eq!(both.text, PUPPET);
    assert!(both.model.as_ref().unwrap().poses.is_empty());
}

#[test]
fn a_mirrored_pose_is_the_mirror_image() {
    let text = with_poses(
        "[poses.step_left]\nleft_arm = [-25, 5, 10]\nright_arm = [25, 0, 0]\n\"left_arm/lower\" = [20, 0, 5]\nhead = { turn = [10, 20, -5], move = [0.02, 0, 0.01] }\n",
    );
    let mirror = |pose: &str, as_name: Option<&str>| Edit::MirrorPose {
        pose: pose.into(),
        as_name: as_name.map(str::to_owned),
    };
    let made = done(&text, &mirror("step_left", Some("step_right")));
    // Left and right swap. A turn around x stays; around y and z it
    // goes the other way. Inside the arm, which is itself a mirror
    // image, the numbers are the same. A move to one side goes to the
    // other.
    assert_eq!(
        made.text,
        format!(
            "{text}\n[poses.step_right]\nright_arm = [-25, -5, -10]\nleft_arm = [25, 0, 0]\n\"right_arm/lower\" = [20, 0, 5]\nhead = {{ turn = [10, -20, 5], move = [-0.02, 0, 0.01] }}\n"
        )
    );
    assert_eq!(
        made.summary,
        "mirrored the pose \"step_left\" as \"step_right\", a new pose"
    );
    // The model in the one pose is the mirror image of the model in the
    // other: every corner has its image.
    let model = made.model.as_ref().unwrap();
    let (left, right) = (
        posed(model, &model.pose("step_left").unwrap().joints),
        posed(model, &model.pose("step_right").unwrap().joints),
    );
    let all = |model: &Model| -> Vec<Vec3> {
        model
            .faces
            .iter()
            .flat_map(|face| face.corners.iter().copied())
            .collect()
    };
    let images = all(&right);
    for corner in all(&left) {
        let image = vec3(-corner.x, corner.y, corner.z);
        assert!(
            images.iter().any(|other| (*other - image).length() < 1e-5),
            "{corner:?} has no mirror image"
        );
    }

    // Mirrored twice in place, a pose is itself again.
    let once = done(&text, &mirror("step_left", None));
    assert_ne!(once.text, text);
    assert_eq!(
        once.summary,
        "mirrored the pose \"step_left\", left to right"
    );
    let twice = done(&once.text, &mirror("step_left", None));
    let joints = |applied: &Applied| {
        let mut joints: Vec<(String, [f64; 3], [f64; 3])> = applied
            .model
            .as_ref()
            .unwrap()
            .pose("step_left")
            .unwrap()
            .joints
            .into_iter()
            .map(|joint| (joint.part, joint.turn, joint.move_by))
            .collect();
        joints.sort_by(|a, b| a.0.cmp(&b.0));
        joints
    };
    let original = done(&text, &set_pose("other", "head", [1.0, 0.0, 0.0]));
    assert_eq!(joints(&twice), joints(&original));

    // Made again, it takes the place of what was there, where it was.
    let changed = done(&made.text, &set_pose("step_left", "head", [0.0, 40.0, 0.0]));
    let again = done(&changed.text, &mirror("step_left", Some("step_right")));
    assert!(again.text.contains("[poses.step_right]\nright_arm = [-25, -5, -10]\nleft_arm = [25, 0, 0]\n\"right_arm/lower\" = [20, 0, 5]\nhead = { turn = [0, -40, 0], move = [-0.02, 0, 0.01] }\n"), "{}", again.text);
    assert_eq!(again.text.lines().count(), changed.text.lines().count());
    assert_eq!(
        again.summary,
        "mirrored the pose \"step_left\" as \"step_right\", in place of what \"step_right\" was"
    );

    // A full turn is still a full turn in the mirror: the numbers keep
    // their size.
    let wheel = done(
        &with_poses("[poses.spin]\nleft_arm = [720, 0, 0]\nhead = [0, 400, 0]\n"),
        &mirror("spin", None),
    );
    assert!(
        wheel
            .text
            .ends_with("[poses.spin]\nright_arm = [720, 0, 0]\nhead = [0, -400, 0]\n"),
        "{}",
        wheel.text
    );

    // A part with no other side can't be mirrored.
    let lonely = PUPPET.replace(
        "\n[[parts]]\nname = \"right_arm\"\nuse = \"arm\"\nmirror = \"x\"\nattach = \"shoulder\"\nto = \"body.right_arm\"\n",
        "",
    ) + "\n[poses.wave]\nleft_arm = [0, 0, -150]\n";
    let failure = refused(&lonely, &mirror("wave", None));
    let error = &failure.report().errors[0];
    assert_eq!(error.code, Code::UnknownPart);
    assert_eq!(
        error.message,
        "the pose turns \"left_arm\", and its mirror image would turn \"right_arm\", but the model has no part with that name"
    );
}

#[test]
fn renaming_and_removing_parts_keeps_the_poses_right() {
    let text = with_poses(
        "[poses.wave]\nleft_arm = [0, 0, -150]\n\"left_arm/lower\" = [0, 0, -30]\n\"right_arm/lower\" = [10, 0, 0]\n\"left_arm/upper\" = [5, 0, 0]\nhead = [0, -15, 0]\n",
    );
    // A part of the model: its line, and the lines of the parts inside
    // it.
    let renamed = done(
        &text,
        &Edit::Rename {
            part: "left_arm".into(),
            new_name: "port_arm".into(),
        },
    );
    assert!(
        renamed.text.ends_with(
            "[poses.wave]\nport_arm = [0, 0, -150]\n\"port_arm/lower\" = [0, 0, -30]\n\"right_arm/lower\" = [10, 0, 0]\n\"port_arm/upper\" = [5, 0, 0]\nhead = [0, -15, 0]\n"
        ),
        "{}",
        renamed.text
    );
    assert_eq!(
        renamed
            .model
            .as_ref()
            .unwrap()
            .pose("wave")
            .unwrap()
            .joints
            .len(),
        5
    );

    // A part inside a definition: in every part that uses it.
    let renamed = done(
        &text,
        &Edit::Rename {
            part: "left_arm/lower".into(),
            new_name: "forearm".into(),
        },
    );
    assert!(
        renamed.text.ends_with(
            "[poses.wave]\nleft_arm = [0, 0, -150]\n\"left_arm/forearm\" = [0, 0, -30]\n\"right_arm/forearm\" = [10, 0, 0]\n\"left_arm/upper\" = [5, 0, 0]\nhead = [0, -15, 0]\n"
        ),
        "{}",
        renamed.text
    );

    // A part removed: its lines go, and those of the parts inside it.
    let removed = done(
        &text,
        &Edit::Remove {
            part: "left_arm".into(),
            with_attached: false,
        },
    );
    assert!(
        removed
            .text
            .ends_with("[poses.wave]\n\"right_arm/lower\" = [10, 0, 0]\nhead = [0, -15, 0]\n"),
        "{}",
        removed.text
    );
    assert_eq!(
        removed.summary,
        "removed \"left_arm\", and 3 lines of poses"
    );
    // Inside a definition: in every part that uses it.
    let removed = done(
        &text,
        &Edit::Remove {
            part: "right_arm/lower".into(),
            with_attached: false,
        },
    );
    assert!(
        removed.text.ends_with(
            "[poses.wave]\nleft_arm = [0, 0, -150]\n\"left_arm/upper\" = [5, 0, 0]\nhead = [0, -15, 0]\n"
        ),
        "{}",
        removed.text
    );
    assert_eq!(
        removed.summary,
        "removed \"right_arm/lower\", and 2 lines of poses"
    );
    // One line.
    let removed = done(
        &text,
        &Edit::Remove {
            part: "head".into(),
            with_attached: false,
        },
    );
    assert_eq!(removed.summary, "removed \"head\", and its line in a pose");
    assert!(!removed.text.contains("head = [0, -15, 0]"));
    // A part no pose names: nothing is said about poses.
    let quiet = done(
        &with_poses("[poses.nod]\nhead = [20, 0, 0]\n"),
        &Edit::Remove {
            part: "left_arm".into(),
            with_attached: false,
        },
    );
    assert_eq!(quiet.summary, "removed \"left_arm\"");
    assert!(quiet.text.ends_with("[poses.nod]\nhead = [20, 0, 0]\n"));
}

#[test]
fn pose_edits_work_on_json_too() {
    let json = r#"{
  "version": 0,
  "parts": [
    { "name": "base", "shape": "box" },
    { "name": "lid", "shape": "box", "size": [1, 0.1, 1], "attach": "bottom", "to": "base.top" }
  ]
}
"#;
    let on = |text: &str, edit: &Edit| {
        apply(
            text,
            Syntax::Json,
            Path::new("test.json"),
            "test.json",
            edit,
        )
        .unwrap_or_else(|failure| panic!("{edit:?}: {:#?}", failure.report()))
    };
    let open = on(json, &set_pose("open", "lid", [-100.0, 0.0, 0.0]));
    assert!(
        open.text
            .contains("\"poses\": { \"open\": { \"lid\": [-100, 0, 0] } }"),
        "{}",
        open.text
    );
    assert_eq!(
        open.model
            .as_ref()
            .unwrap()
            .pose("open")
            .unwrap()
            .joints
            .len(),
        1
    );
    let ajar = on(
        &open.text,
        &Edit::AddPose {
            name: "ajar".into(),
            from: Some("open".into()),
        },
    );
    let ajar = on(&ajar.text, &set_pose("ajar", "lid", [-20.0, 0.0, 0.0]));
    let model = ajar.model.as_ref().unwrap();
    assert_eq!(
        model.pose("ajar").unwrap().joints[0].turn,
        [-20.0, 0.0, 0.0]
    );
    assert_eq!(
        model.pose("open").unwrap().joints[0].turn,
        [-100.0, 0.0, 0.0]
    );
    let renamed = on(
        &ajar.text,
        &Edit::RenamePose {
            pose: "ajar".into(),
            new_name: "peek".into(),
        },
    );
    assert!(renamed.model.as_ref().unwrap().pose("peek").is_some());
    let fewer = on(
        &renamed.text,
        &Edit::RemovePose {
            pose: "peek".into(),
        },
    );
    let none = on(
        &fewer.text,
        &Edit::RemovePose {
            pose: "open".into(),
        },
    );
    assert!(none.model.as_ref().unwrap().poses.is_empty());
    assert!(!none.text.contains("poses"), "{}", none.text);
}
