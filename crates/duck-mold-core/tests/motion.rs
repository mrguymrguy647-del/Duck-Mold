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
