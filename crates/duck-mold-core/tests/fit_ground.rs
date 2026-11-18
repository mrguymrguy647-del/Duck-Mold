//! Phase 6, step 6: `fit-ground` stands an animation's poses on the
//! ground by moving the body up or down in each, and leaves leaps alone.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::{AnimationFrom, Applied, Edit, apply};
use duck_mold_core::issue::Code;
use duck_mold_core::model::Model;
use duck_mold_core::motion::{Motion, frame_times, posed};
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::{self, Failure};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn run(text: &str, edit: &Edit) -> Result<Applied, Failure> {
    apply(
        text,
        Syntax::Toml,
        &examples().join("test.toml"),
        "test.toml",
        edit,
    )
}

fn done(text: &str, edit: &Edit) -> Applied {
    run(text, edit).unwrap_or_else(|failure| panic!("{failure:?}"))
}

fn refused(text: &str, edit: &Edit) -> (Code, String) {
    match run(text, edit) {
        Err(Failure::Usage(issue)) => (issue.code, issue.message.clone()),
        Err(other) => panic!("refused another way: {other:?}"),
        Ok(applied) => panic!("not refused: {}", applied.summary),
    }
}

fn fit(animation: &str) -> Edit {
    Edit::FitGround {
        animation: animation.into(),
        part: None,
        all: false,
        keys_only: false,
    }
}

fn fit_keys_only(animation: &str) -> Edit {
    Edit::FitGround {
        animation: animation.into(),
        part: None,
        all: false,
        keys_only: true,
    }
}

/// The lowest point of the model in a pose.
fn lowest_in(model: &Model, pose: &str) -> f64 {
    let pose = model.pose(pose).unwrap();
    posed(model, &pose.joints).bounds().unwrap().min.y
}

/// The lowest the model goes through an animation, frame by frame.
fn lowest_playing(model: &Model, animation: &str) -> f64 {
    let played = model.animation(animation).unwrap();
    let moving = Motion::of(model, played);
    frame_times(played.length())
        .into_iter()
        .map(|at| posed(model, &moving.joints_at(at)).bounds().unwrap().min.y)
        .fold(f64::INFINITY, f64::min)
}

/// The gnome as it was before Phase 6 fitted it: its step poses with
/// the body moves typed by hand, and no jump.
fn gnome_by_hand() -> String {
    std::fs::read_to_string(examples().join("gnome.toml"))
        .unwrap()
        .replace(
            "body = { move = [0, -0.0187, 0] }\nleft_leg = [25, 0, 0]",
            "body = { move = [0, -0.017, 0] }\nleft_leg = [25, 0, 0]",
        )
        .replace(
            "body = { move = [0, -0.0187, 0] }\nright_leg = [25, 0, 0]",
            "body = { move = [0, -0.017, 0] }\nright_leg = [25, 0, 0]",
        )
}

#[test]
fn every_key_pose_stands_exactly_on_the_ground_and_only_the_body_moves() {
    let before = gnome_by_hand();
    let fitted = done(&before, &fit("walk"));
    assert_eq!(
        fitted.summary,
        "stood the poses of \"walk\" on the ground, moving \"body\" in \"step_left\" (down 0.0017), \"step_right\" (down 0.0017); between keys it now sinks at most 0.002 (at 0.1 s)"
    );
    // Only the two body lines changed.
    let changed: Vec<(&str, &str)> = before
        .lines()
        .zip(fitted.text.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        changed,
        [(
            "body = { move = [0, -0.017, 0] }",
            "body = { move = [0, -0.0187, 0] }"
        ); 2]
    );
    let model = fitted.model.unwrap();
    for pose in ["step_left", "step_right"] {
        let low = lowest_in(&model, pose);
        assert!(low.abs() < 0.00005, "{pose}: {low}");
    }
    // As the example is now: fitted, so fitting again changes nothing.
    let now = std::fs::read_to_string(examples().join("gnome.toml")).unwrap();
    assert_eq!(fitted.text, now.replace("\r\n", "\n"));
    let again = done(&now, &fit("walk"));
    assert_eq!(again.lines, None, "{}", again.summary);
}

#[test]
fn a_leap_is_left_alone_unless_all() {
    let gnome = std::fs::read_to_string(examples().join("gnome.toml")).unwrap();
    // The crouch is fitted already; the leap, 40 cm up, is a leap.
    let left = done(&gnome, &fit("jump"));
    assert_eq!(left.lines, None);
    let model = left.model.unwrap();
    assert!(lowest_in(&model, "jump_crouch").abs() < 0.00005);
    assert!(lowest_in(&model, "jump_up") > 0.3);
    // With --all, the leap comes down to the ground too.
    let all = done(
        &gnome,
        &Edit::FitGround {
            animation: "jump".into(),
            part: None,
            all: true,
            keys_only: true,
        },
    );
    assert!(
        all.summary.contains("\"jump_up\" (down 0.3926)"),
        "{}",
        all.summary
    );
    assert!(!all.summary.contains("alone"), "{}", all.summary);
    assert!(lowest_in(all.model.as_ref().unwrap(), "jump_up").abs() < 0.00005);

    // A crouch sunk into the ground by hand comes back up; the summary
    // says what was left alone, and how far it sinks between keys.
    let sunk = gnome.replacen(
        "[poses.jump_crouch]\nbody = { move = [0, -0.0187, 0] }",
        "[poses.jump_crouch]\nbody = { move = [0, -0.06, 0] }",
        1,
    );
    let fitted = done(&sunk, &fit("jump"));
    assert!(
        fitted.summary.starts_with(
            "stood the poses of \"jump\" on the ground, moving \"body\" in \"jump_crouch\" (up 0.0413); left \"jump_up\" alone: it lifts the body (a leap; --all fits it too)"
        ),
        "{}",
        fitted.summary
    );
    assert!(fitted.text.contains("body = { move = [0, -0.0187, 0] }"));
}

/// A stick figure whose two animations share a pose that sinks.
const FIGURE: &str = r#"version = 0
name = "figure"

[[parts]]
name = "body"
shape = "box"
size = [0.4, 0.4, 0.2]
move = [0, 1, 0]
sockets = { hip = { on = "bottom" } }

[[parts]]
name = "leg"
shape = "box"
size = [0.1, 0.8, 0.1]
attach = "top"
to = "body.hip"

[poses.kneel]
body = { move = [0, -0.1, 0] }

[animations.down]
keys = [
  { at = 0, pose = "rest" },
  { at = 0.5, pose = "kneel" },
]

[animations.bow]
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "kneel" },
  { at = 2, pose = "rest" },
]
"#;

#[test]
fn a_pose_other_animations_use_changes_for_them_and_is_said() {
    let fitted = done(FIGURE, &fit("down"));
    assert_eq!(
        fitted.summary,
        "stood the poses of \"down\" on the ground, moving \"body\" in \"kneel\" (up 0.1); \"bow\" uses the same poses, so it is changed too; it doesn't sink into the ground between keys either"
    );
    let model = fitted.model.unwrap();
    assert!(lowest_in(&model, "kneel").abs() < 0.00005);
    assert!(lowest_playing(&model, "bow") > -0.00005);
    // The kneel's body move is gone: back at rest, its line goes.
    assert!(!fitted.text.contains("move = [0, -0.1, 0]"));
}

#[test]
fn which_part_moves_and_what_is_refused() {
    // A part that hangs from another can't be the one moved.
    let (code, message) = refused(
        FIGURE,
        &Edit::FitGround {
            animation: "down".into(),
            part: Some("leg".into()),
            all: false,
            keys_only: true,
        },
    );
    assert_eq!(code, Code::Conflict);
    assert_eq!(
        message,
        "\"leg\" hangs from \"body\": give the part the others hang from, like --part body"
    );
    let (code, _) = refused(
        FIGURE,
        &Edit::FitGround {
            animation: "down".into(),
            part: Some("bodu".into()),
            all: false,
            keys_only: true,
        },
    );
    assert_eq!(code, Code::UnknownPart);
    // Two figures side by side: which?
    let two = FIGURE.replace(
        "[poses.kneel]",
        "[[parts]]\nname = \"post\"\nshape = \"box\"\nsize = [0.1, 1, 0.1]\nmove = [2, 0.5, 0]\nsockets = { peak = { on = \"top\" } }\n\n[[parts]]\nname = \"cap\"\nshape = \"box\"\nsize = [0.2, 0.1, 0.2]\nattach = \"bottom\"\nto = \"post.peak\"\n\n[poses.kneel]",
    );
    let (code, message) = refused(&two, &fit("down"));
    assert_eq!(code, Code::Conflict);
    assert!(
        message.starts_with("more than one part has others hanging from it (body, post)"),
        "{message}"
    );
    let named = done(
        &two,
        &Edit::FitGround {
            animation: "down".into(),
            part: Some("body".into()),
            all: false,
            keys_only: true,
        },
    );
    assert!(named.summary.contains("moving \"body\""));
    // An animation that only uses rest has nothing to fit; one that
    // isn't there is said so.
    let rested = FIGURE.replace("pose = \"kneel\"", "pose = \"rest\"");
    let (code, message) = refused(&rested, &fit("down"));
    assert_eq!(code, Code::Conflict);
    assert!(message.contains("only uses rest"), "{message}");
    assert_eq!(refused(FIGURE, &fit("run")).0, Code::UnknownAnimation);
}

/// Every example animation, fitted: each key pose stands exactly on the
/// ground (or is a leap, left alone), and between the keys it sinks at
/// most 2 cm. (The examples as they are kept within half a centimetre:
/// the golden tests check that. Fitting the robot's hand-tuned jump
/// exactly makes it dip a centimetre between its keys: the curve's limit
/// that the summary says.)
#[test]
fn every_example_animation_fitted_stands_each_key_on_the_ground() {
    let mut fitted_any = 0;
    for example in tools::EXAMPLES {
        let text = example.recipe;
        let Ok(applied) = run(text, &Edit::SetColliders { colliders: None })
            .or_else(|_| run(text, &fit("__none__")))
        else {
            // Not built this way: the example has no "colliders" to
            // take away and no animation; it has nothing to fit.
            continue;
        };
        let Some(model) = applied.model else { continue };
        for animation in &model.animations {
            let name = animation.name.clone();
            let fitted = match run(text, &fit(&name)) {
                Ok(fitted) => fitted,
                // Wheels on a cart, a door: nothing hangs from one part,
                // or every pose is rest. Said, not done.
                Err(Failure::Usage(_)) => continue,
                Err(other) => panic!("{}: {name}: {other:?}", example.name),
            };
            let model = fitted.model.unwrap();
            let played = model.animation(&name).unwrap();
            for key in &played.keys {
                if key.pose == "rest" {
                    continue;
                }
                let low = lowest_in(&model, &key.pose);
                let leap = low > 0.05;
                assert!(
                    leap || low.abs() < 0.00005,
                    "{} {name}: {} at {low}",
                    example.name,
                    key.pose
                );
            }
            // Between the keys too: keys are added where it sank more
            // than 3 mm.
            let low = lowest_playing(&model, &name);
            assert!(low > -0.003, "{} {name}: sinks {low}", example.name);
            // The summary says the worst, when it sinks at all.
            if low < -0.0001 {
                assert!(
                    fitted.summary.contains("between keys it now sinks at most")
                        || fitted.lines.is_none(),
                    "{}",
                    fitted.summary
                );
            }
            fitted_any += 1;
        }
    }
    assert!(fitted_any >= 8, "{fitted_any}");
}

#[test]
fn a_starter_copied_onto_the_gnome_and_fitted_stands_on_the_ground() {
    // The starter's idle, onto the gnome, fitted: it was made for the
    // robot's longer legs.
    let gnome = std::fs::read_to_string(examples().join("gnome.toml")).unwrap();
    let added = done(
        &gnome,
        &Edit::AddAnimation {
            name: "idle".into(),
            looping: false,
            from: Some(AnimationFrom::Starter("idle".into())),
            skip_missing: true,
        },
    );
    let fitted = done(&added.text, &fit("idle"));
    let model = fitted.model.unwrap();
    assert!(lowest_playing(&model, "idle") > -0.005);
}

// ---- between the keys (Phase 7a) ---------------------------------------------------

/// The robot's jump as it was before Phase 7a: its crouch stood a little
/// high, by hand, so the feet didn't dig in between the keys.
const HAND_TUNED_CROUCH: &str = "body = { move = [0, -0.025, 0] }";

fn robot_hand_tuned() -> String {
    let robot = std::fs::read_to_string(examples().join("robot.toml")).unwrap();
    // The robot as kept is fitted already: put its jump back as it was.
    let start = robot.find("[poses.jump_crouch]").unwrap();
    let line_end = robot[start..].find('\n').unwrap() + start + 1;
    let next = robot[line_end..].find('\n').unwrap() + line_end;
    let mut text = format!(
        "{}{HAND_TUNED_CROUCH}{}",
        &robot[..line_end],
        &robot[next..]
    );
    for added in ["jump_2", "jump_3"] {
        let key = format!(
            "  {{ at = {}, pose = \"{added}\" }},\n",
            if added == "jump_2" { "0.13" } else { "1.17" }
        );
        text = text.replace(&key, "");
        // And the pose's table, up to the next one.
        let start = text.find(&format!("[poses.{added}]\n")).unwrap();
        let end = text[start + 1..].find("\n[").unwrap() + start + 2;
        text.replace_range(start..end, "");
    }
    text
}

#[test]
fn fitting_only_the_keys_leaves_a_dip_that_inspect_warns_about() {
    use duck_mold_core::inspect::{inspect, inspect_animation};
    let fitted = done(&robot_hand_tuned(), &fit_keys_only("jump"));
    assert!(
        fitted
            .summary
            .contains("between keys it now sinks at most 0.0102"),
        "{}",
        fitted.summary
    );
    let model = fitted.model.unwrap();
    assert!(lowest_playing(&model, "jump") < -0.009);
    let rest = inspect(&model, "robot.toml", Vec::new());
    let report = inspect_animation(
        &rest,
        &model,
        model.animation("jump").unwrap(),
        "robot.toml",
    );
    let dip = report
        .warnings
        .iter()
        .find(|warning| warning.code == Code::DipsBetweenKeys)
        .expect("a dip between keys is warned about");
    assert!(
        dip.message
            .contains("sinks 0.01 into the ground at 0.133 s")
            && dip.message.contains("fit-ground jump"),
        "{}",
        dip.message
    );
}

#[test]
fn fitting_adds_keys_where_it_dips_between_keys() {
    use duck_mold_core::inspect::{inspect, inspect_animation};
    let fitted = done(&robot_hand_tuned(), &fit("jump"));
    assert!(
        fitted.summary.contains(
            "added 2 keys between keys, at 0.13 s and 1.17 s, where it sank 10 mm and 10 mm"
        ),
        "{}",
        fitted.summary
    );
    let model = fitted.model.unwrap();
    // The new poses are named after the animation, and stand on the
    // ground; the jump sinks at most 3 mm now.
    for (name, at) in [("jump_2", 0.13), ("jump_3", 1.17)] {
        assert!(lowest_in(&model, name).abs() < 0.0002, "{name}");
        let played = model.animation("jump").unwrap();
        assert!(
            played
                .keys
                .iter()
                .any(|key| key.pose == name && (key.at - at).abs() < 1e-9)
        );
    }
    assert!(lowest_playing(&model, "jump") > -0.003);
    let rest = inspect(&model, "robot.toml", Vec::new());
    let report = inspect_animation(
        &rest,
        &model,
        model.animation("jump").unwrap(),
        "robot.toml",
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.code == Code::DipsBetweenKeys)
    );
    // Fitted again, nothing more is added: it's done.
    let again = done(&fitted.text, &fit("jump"));
    assert!(!again.summary.contains("added"), "{}", again.summary);
}

#[test]
fn the_robots_jump_as_kept_is_fitted_between_keys() {
    let model = tools::build(&examples().join("robot.toml")).unwrap().model;
    assert!(lowest_playing(&model, "jump") > -0.003);
}
