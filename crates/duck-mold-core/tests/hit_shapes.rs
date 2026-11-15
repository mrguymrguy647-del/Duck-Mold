//! Phase 6, step 1: hit shapes (colliders) in the recipe and the core.
//! What the recipe says, how each shape is fitted, how it moves with a
//! pose, what `inspect` reports, and the two edit commands.

use std::path::Path;

use duck_mold_core::edit::{ColliderTo, Edit, apply};
use duck_mold_core::hit::corners_of;
use duck_mold_core::inspect::{collider_report, inspect};
use duck_mold_core::issue::Code;
use duck_mold_core::model::{HitShape, Model};
use duck_mold_core::motion::{Motion, frame_times, posed};
use duck_mold_core::recipe::{Colliders, HitKind};
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::{self, Failure};

fn examples() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Builds recipe text as if it were a file in the examples folder.
fn build(text: &str) -> (Option<Model>, Vec<duck_mold_core::issue::Issue>) {
    let applied = apply(
        text,
        Syntax::Toml,
        &examples().join("test.toml"),
        "test.toml",
        // An edit that changes nothing a model has: only to build it.
        &Edit::SetColliders { colliders: None },
    );
    match applied {
        Ok(applied) => (applied.model, applied.issues),
        Err(Failure::Usage(_)) => {
            // There was no `colliders` line to take away: build it as it is.
            let mut issues = Vec::new();
            let loaded = duck_mold_core::load::load_text(
                text,
                Syntax::Toml,
                "test.toml",
                &examples(),
                &mut issues,
            );
            (
                duck_mold_core::assemble::assemble(&loaded, &mut issues),
                issues,
            )
        }
        Err(other) => panic!("{other:?}"),
    }
}

fn built(text: &str) -> Model {
    let mut issues = Vec::new();
    let loaded =
        duck_mold_core::load::load_text(text, Syntax::Toml, "test.toml", &examples(), &mut issues);
    duck_mold_core::assemble::assemble(&loaded, &mut issues)
        .unwrap_or_else(|| panic!("it builds: {issues:#?}"))
}

fn errors(text: &str) -> Vec<duck_mold_core::issue::Issue> {
    let mut issues = Vec::new();
    let loaded =
        duck_mold_core::load::load_text(text, Syntax::Toml, "test.toml", &examples(), &mut issues);
    duck_mold_core::assemble::assemble(&loaded, &mut issues);
    issues
        .into_iter()
        .filter(|issue| issue.is_error())
        .collect()
}

/// Every example, with `colliders = "parts"` put at its top.
fn every_example_with_parts() -> Vec<(String, String)> {
    tools::EXAMPLES
        .iter()
        .map(|example| {
            let text = if example.recipe.contains("\ncolliders = ") {
                example.recipe.to_owned()
            } else {
                example.recipe.replacen(
                    "\nversion = 0\n",
                    "\nversion = 0\ncolliders = \"parts\"\n",
                    1,
                )
            };
            (example.name.to_owned(), text)
        })
        .collect()
}

/// How far a capsule may leave a corner of a round part outside its
/// round end: a capsule as long as the part leaves a cylinder's rim
/// corners out by up to 0.42 of its radius.
fn slack(shape: &HitShape) -> f64 {
    match shape {
        HitShape::Capsule { radius, .. } => radius * (2f64.sqrt() - 1.0) + 1e-6,
        _ => 1e-6,
    }
}

const TOP: &str = r##"version = 0
name = "toy"
colliders = "parts"

[palette]
red = "#CC3333"

[[parts]]
name = "body"
shape = "box"
size = [0.4, 0.6, 0.3]
color = "red"
sockets = { neck = { on = "top" }, arm = { on = "left", shift = [0, 0.7, 0] } }

[[parts]]
name = "head"
shape = "sphere"
radius = 0.15
color = "red"
attach = "bottom"
to = "body.neck"

[[parts]]
name = "arm"
shape = "cylinder"
radius = 0.05
height = 0.4
color = "red"
attach = "right"
to = "body.arm"
"##;

#[test]
fn colliders_parts_fits_the_shape_that_suits_each_part() {
    let model = built(TOP);
    assert_eq!(model.hit_setting, Some(Colliders::Parts));
    let kinds: Vec<(&str, HitKind)> = model
        .colliders
        .iter()
        .map(|collider| (collider.part.as_str(), collider.shape.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("body", HitKind::Box),
            ("head", HitKind::Sphere),
            ("arm", HitKind::Capsule)
        ]
    );
    // The body's box is the body.
    let HitShape::Box { center, half, .. } = model.colliders[0].shape else {
        panic!("a box");
    };
    let body = model.part("body").unwrap().bounds.unwrap();
    assert!((center - body.center()).length() < 1e-6);
    assert!((half - body.half()).length() < 1e-6);
    // The head's ball holds every corner, and is no bigger than that.
    let HitShape::Sphere { radius, .. } = model.colliders[1].shape else {
        panic!("a sphere");
    };
    assert!(radius <= 0.15 + 1e-6 && radius > 0.14, "{radius}");
    // The arm's capsule lies along it and is as long as it.
    let HitShape::Capsule { a, b, radius } = model.colliders[2].shape else {
        panic!("a capsule");
    };
    assert!(((b - a).length() + 2.0 * radius - 0.4).abs() < 1e-6);
    assert!((radius - 0.05).abs() < 0.005, "{radius}");
    assert!(model.without_collider.is_empty());
}

#[test]
fn every_fitted_shape_holds_its_part_in_every_example_and_every_pose() {
    for (name, text) in every_example_with_parts() {
        let model = built(&text);
        assert!(!model.colliders.is_empty(), "{name}");
        // Every part with faces of its own has a shape, but the ones that
        // say "none".
        let solid = model
            .all_parts()
            .iter()
            .filter(|part| part.parts.is_empty() && !part.faces.is_empty())
            .count();
        assert_eq!(
            model.colliders.len() + model.without_collider.len(),
            solid,
            "{name}"
        );
        let check = |model: &Model, when: &str| {
            for collider in &model.colliders {
                let part = model.part(&collider.part).unwrap();
                for corner in corners_of(model, part) {
                    let out = collider.shape.distance_outside(corner);
                    assert!(
                        out <= slack(&collider.shape),
                        "{name} {when}: a corner of {} is {out} outside its {}",
                        collider.part,
                        collider.shape.describe()
                    );
                }
            }
        };
        check(&model, "at rest");
        for pose in &model.poses {
            check(&posed(&model, &pose.joints), &pose.name);
        }
        for animation in &model.animations {
            let moving = Motion::of(&model, animation);
            for at in frame_times(animation.length()).into_iter().step_by(5) {
                check(&posed(&model, &moving.joints_at(at)), &animation.name);
            }
        }
    }
}

#[test]
fn one_shape_round_the_whole_model() {
    for kind in ["capsule", "box", "sphere"] {
        let text = TOP.replace("colliders = \"parts\"", &format!("colliders = \"{kind}\""));
        let model = built(&text);
        assert_eq!(model.colliders.len(), 1, "{kind}");
        let collider = &model.colliders[0];
        assert_eq!(collider.part, "", "{kind}");
        assert_eq!(collider.shape.kind().name(), kind);
        for face in &model.faces {
            for corner in &face.corners {
                assert!(
                    collider.shape.distance_outside(*corner) <= slack(&collider.shape),
                    "{kind}"
                );
            }
        }
        // A capsule stands up.
        if let HitShape::Capsule { a, b, .. } = collider.shape {
            assert!((a.x - b.x).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9);
            assert!(b.y > a.y);
        }
        // It stays where it is in a pose.
        let turned = posed(
            &model,
            &[duck_mold_core::motion::Joint::turned(
                "arm",
                [60.0, 0.0, 0.0],
            )],
        );
        assert_eq!(turned.colliders, model.colliders);
    }
}

#[test]
fn a_part_says_its_own() {
    let text = TOP
        .replace(
            "radius = 0.15\ncolor = \"red\"",
            "radius = 0.15\ncolor = \"red\"\ncollider = \"box\"",
        )
        .replace(
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = \"none\"",
        );
    let model = built(&text);
    let kinds: Vec<(&str, HitKind)> = model
        .colliders
        .iter()
        .map(|collider| (collider.part.as_str(), collider.shape.kind()))
        .collect();
    assert_eq!(kinds, [("body", HitKind::Box), ("head", HitKind::Box)]);
    assert_eq!(model.without_collider, [("arm".to_owned(), true)]);

    // Its own numbers, in its own directions, from its own middle.
    let text = TOP.replace(
        "radius = 0.15\ncolor = \"red\"",
        "radius = 0.15\ncolor = \"red\"\ncollider = { shape = \"sphere\", radius = 0.2, center = [0, 0.05, 0] }",
    );
    let model = built(&text);
    let head = model.part("head").unwrap().bounds.unwrap().center();
    let HitShape::Sphere { center, radius } = model.colliders[1].shape else {
        panic!("a sphere");
    };
    assert!((radius - 0.2).abs() < 1e-9);
    assert!((center - head).length() - 0.05 < 1e-6);
    assert!(model.colliders[1].own);

    // With no line at the top, only parts that say their own have one.
    let text = TOP.replace("colliders = \"parts\"\n", "").replace(
        "radius = 0.15\ncolor = \"red\"",
        "radius = 0.15\ncolor = \"red\"\ncollider = \"sphere\"",
    );
    let model = built(&text);
    assert_eq!(model.colliders.len(), 1);
    assert_eq!(model.colliders[0].part, "head");
    assert!(model.without_collider.is_empty());
}

#[test]
fn a_part_made_of_parts_can_have_one_round_all_of_it() {
    let text = std::fs::read_to_string(examples().join("robot.toml")).unwrap();
    // The left arm: one capsule round all four of its parts.
    let text = text.replacen(
        "name = \"left_arm\"\nuse = \"arm\"",
        "name = \"left_arm\"\nuse = \"arm\"\ncollider = \"capsule\"",
        1,
    );
    let model = built(&text);
    let arm: Vec<&str> = model
        .colliders
        .iter()
        .map(|collider| collider.part.as_str())
        .filter(|part| part.starts_with("left_arm"))
        .collect();
    assert_eq!(arm, ["left_arm"]);
    let shape = &model
        .colliders
        .iter()
        .find(|collider| collider.part == "left_arm")
        .unwrap()
        .shape;
    for corner in corners_of(&model, model.part("left_arm").unwrap()) {
        assert!(shape.distance_outside(corner) <= slack(shape));
    }
    // The right arm keeps a shape for each of its parts.
    assert!(
        model
            .colliders
            .iter()
            .any(|collider| collider.part == "right_arm/hand")
    );
}

#[test]
fn mistakes_in_hit_shapes_say_what_to_write() {
    let cases: [(&str, &str, Code, &str); 7] = [
        (
            "colliders = \"parts\"",
            "colliders = \"all\"",
            Code::TypeMismatch,
            "\"colliders\" is \"parts\"",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = \"ball\"",
            Code::TypeMismatch,
            "a hit shape is",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = { radius = 0.1 }",
            Code::MissingField,
            "says which shape it is",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = { shape = \"capsule\", radius = 0.1 }",
            Code::MissingField,
            "needs \"length\"",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = { shape = \"capsule\", radius = 0.2, length = 0.3 }",
            Code::OutOfRange,
            "at least twice its radius",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = { shape = \"box\", radius = 0.2 }",
            Code::Conflict,
            "has no \"radius\"",
        ),
        (
            "height = 0.4\ncolor = \"red\"",
            "height = 0.4\ncolor = \"red\"\ncollider = { shape = \"capsule\", radius = 0.05, length = 0.4, along = \"w\" }",
            Code::TypeMismatch,
            "lies along",
        ),
    ];
    for (from, to, code, words) in cases {
        let text = TOP.replace(from, to);
        assert_ne!(text, TOP, "{to}");
        let found = errors(&text);
        assert!(
            found
                .iter()
                .any(|issue| issue.code == code && issue.message.contains(words)),
            "{to}: {found:#?}"
        );
    }
    // `colliders` isn't reserved any more; `bones` still is.
    let found = errors(&TOP.replace("colliders = \"parts\"", "bones = 1"));
    assert!(found.iter().any(|issue| issue.code == Code::Reserved));
}

#[test]
fn a_hit_shape_of_its_own_that_misses_its_part_is_a_warning() {
    let small = TOP.replace(
        "radius = 0.15\ncolor = \"red\"",
        "radius = 0.15\ncolor = \"red\"\ncollider = { shape = \"sphere\", radius = 0.05 }",
    );
    let (built, issues) = build(&small);
    assert!(built.is_some());
    let warnings: Vec<_> = issues
        .iter()
        .filter(|issue| issue.code == Code::HitShape)
        .collect();
    assert_eq!(warnings.len(), 1, "{issues:#?}");
    assert!(warnings[0].message.contains("most of \"head\" is outside"));
    assert_eq!(warnings[0].line, Some(14)); // the part's own line

    let big = TOP.replace(
        "radius = 0.15\ncolor = \"red\"",
        "radius = 0.15\ncolor = \"red\"\ncollider = { shape = \"sphere\", radius = 0.5 }",
    );
    let (_, issues) = build(&big);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == Code::HitShape && issue.message.contains("8 times")),
        "{issues:#?}"
    );
    // A fitted shape never warns.
    let (_, issues) = build(TOP);
    assert!(issues.iter().all(|issue| issue.code != Code::HitShape));
}

#[test]
fn inspect_says_how_many_and_which_have_none() {
    let text = TOP.replace(
        "height = 0.4\ncolor = \"red\"",
        "height = 0.4\ncolor = \"red\"\ncollider = \"none\"",
    );
    let model = built(&text);
    let report = inspect(&model, "toy.toml", Vec::new());
    let words = report.to_text();
    assert!(
        words.contains(
            "  hit shapes   each part: 2 (box 1, sphere 1)\n               none: arm (its own)\n"
        ),
        "{words}"
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["hit_shapes"]["setting"], "parts");
    assert_eq!(json["hit_shapes"]["count"], 2);
    assert_eq!(json["parts_without_colliders"], serde_json::json!(["arm"]));
    // A model with none and no line says nothing about them.
    let plain = model_of_example("crate.toml");
    let json = serde_json::to_value(inspect(&plain, "crate.toml", Vec::new())).unwrap();
    assert!(json.get("hit_shapes").is_none());

    // --colliders: each one's numbers.
    let listed = collider_report(&model);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].part, "body");
    assert_eq!(listed[0].shape, "box");
    assert_eq!(listed[0].size, Some([0.4, 0.6, 0.3]));
    assert_eq!(listed[1].shape, "sphere");
    assert!(listed[1].radius.is_some());
    let mut with = report.clone();
    with.colliders = listed;
    let words = with.to_text();
    assert!(
        words.contains("  colliders    2: each one's shape and where its middle is\n"),
        "{words}"
    );
    assert!(words.contains("    body   box 0.4 x 0.6 x 0.3"), "{words}");
}

fn model_of_example(file: &str) -> Model {
    tools::build(&examples().join(file))
        .unwrap_or_else(|_| panic!("{file} builds"))
        .model
}

#[test]
fn the_examples_have_their_hit_shapes() {
    let robot = model_of_example("robot.toml");
    assert_eq!(robot.hit_setting, Some(Colliders::Parts));
    let head = robot
        .colliders
        .iter()
        .find(|collider| collider.part == "head")
        .unwrap();
    assert_eq!(head.shape.kind(), HitKind::Sphere);
    let none: Vec<&str> = robot
        .without_collider
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(none, ["panel", "left_eye", "right_eye", "antenna_tip"]);
    let door = model_of_example("door.toml");
    assert_eq!(door.colliders.len(), 1);
    assert_eq!(door.colliders[0].part, "door");
    // The cart's flat wheels get boxes, not capsules lying across them.
    let cart = model_of_example("cart.toml");
    assert!(
        cart.colliders
            .iter()
            .filter(|collider| collider.part.contains("tyre"))
            .all(|collider| collider.shape.kind() == HitKind::Box)
    );
    // The other examples have none.
    for name in ["crate.toml", "house.toml", "duck.toml", "rock.toml"] {
        let model = model_of_example(name);
        assert!(
            model.colliders.is_empty() && model.hit_setting.is_none(),
            "{name}"
        );
    }
}

// ---- the edit commands -------------------------------------------------------------------

fn edited(text: &str, edit: &Edit) -> String {
    match apply(
        text,
        Syntax::Toml,
        &examples().join("toy.toml"),
        "toy.toml",
        edit,
    ) {
        Ok(applied) => applied.text,
        Err(failure) => panic!("{edit:?}: {:#?}", failure.report()),
    }
}

fn refused(text: &str, edit: &Edit) -> String {
    match apply(
        text,
        Syntax::Toml,
        &examples().join("toy.toml"),
        "toy.toml",
        edit,
    ) {
        Ok(applied) => panic!("{edit:?} gave {}", applied.text),
        Err(failure) => failure.report().errors[0].message.clone(),
    }
}

#[test]
fn set_colliders_writes_and_takes_away_the_line() {
    let none = TOP.replace("colliders = \"parts\"\n", "");
    let text = edited(
        &none,
        &Edit::SetColliders {
            colliders: Some(Colliders::Whole(HitKind::Capsule)),
        },
    );
    assert_eq!(
        text,
        none.replace(
            "name = \"toy\"\n",
            "name = \"toy\"\ncolliders = \"capsule\"\n"
        )
    );
    let back = edited(&text, &Edit::SetColliders { colliders: None });
    assert_eq!(back, none);
    let message = refused(&none, &Edit::SetColliders { colliders: None });
    assert!(message.contains("no \"colliders\" to take away"));
    let parts = edited(
        &text,
        &Edit::SetColliders {
            colliders: Some(Colliders::Parts),
        },
    );
    assert!(parts.contains("colliders = \"parts\"\n") && !parts.contains("capsule"));
}

#[test]
fn set_collider_gives_a_part_its_own_and_takes_it_away() {
    let sphere = |radius: Option<f64>| Edit::SetCollider {
        part: "head".into(),
        to: ColliderTo::Shape {
            kind: HitKind::Sphere,
            size: None,
            radius,
            length: None,
            center: None,
            along: None,
        },
    };
    let text = edited(TOP, &sphere(None));
    assert!(
        text.contains("to = \"body.neck\"\ncollider = \"sphere\"\n"),
        "{text}"
    );
    // Numbers: the radius given, the middle fitted.
    let text = edited(TOP, &sphere(Some(0.25)));
    assert!(
        text.contains("collider = { shape = \"sphere\", radius = 0.25 }"),
        "{text}"
    );
    // A capsule's length alone: its radius fitted.
    let text = edited(
        TOP,
        &Edit::SetCollider {
            part: "arm".into(),
            to: ColliderTo::Shape {
                kind: HitKind::Capsule,
                size: None,
                radius: None,
                length: Some(0.5),
                center: None,
                along: None,
            },
        },
    );
    assert!(
        text.contains("collider = { shape = \"capsule\", radius = 0.051, length = 0.5 }"),
        "{text}"
    );
    let none = edited(
        TOP,
        &Edit::SetCollider {
            part: "arm".into(),
            to: ColliderTo::None,
        },
    );
    assert!(none.contains("collider = \"none\""));
    let back = edited(
        &none,
        &Edit::SetCollider {
            part: "arm".into(),
            to: ColliderTo::Fitted,
        },
    );
    assert_eq!(back, TOP);
    // Mistakes.
    let message = refused(
        TOP,
        &Edit::SetCollider {
            part: "arm".into(),
            to: ColliderTo::Fitted,
        },
    );
    assert!(message.contains("no hit shape of its own to take away"));
    let message = refused(
        TOP,
        &Edit::SetCollider {
            part: "arm".into(),
            to: ColliderTo::Shape {
                kind: HitKind::Box,
                size: None,
                radius: Some(0.1),
                length: None,
                center: None,
                along: None,
            },
        },
    );
    assert!(message.contains("a box has no --radius"), "{message}");
    let message = refused(
        TOP,
        &Edit::SetCollider {
            part: "leg".into(),
            to: ColliderTo::None,
        },
    );
    assert!(message.contains("\"leg\""), "{message}");
}

#[test]
fn a_new_model_starts_with_hit_shapes() {
    assert!(tools::STARTER.contains("\ncolliders = \"parts\""));
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("hit_new");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let recipe = folder.join("thing.toml");
    tools::new_file(&recipe).unwrap();
    let built = tools::build(&recipe)
        .unwrap_or_else(|_| panic!("builds"))
        .model;
    assert_eq!(built.hit_setting, Some(Colliders::Parts));
    assert_eq!(built.colliders.len(), 1);
}
