//! Phase 5, step 8: the animation library. `save-animation` writes an
//! animation and its poses to a file of their own; `add-animation
//! --from` (or `--starter`) copies one into a recipe.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::{AnimationFrom, Edit, animation_recipe, apply};
use duck_mold_core::issue::Code;
use duck_mold_core::source::{Syntax, parse};
use duck_mold_core::starters::{STARTERS, TWO_LEGS, starter};
use duck_mold_core::tools::{Failure, save_animation};

/// The robot's recipe, without the two lines that put images on it:
/// these tests write the text into folders that don't have the images.
fn robot_text() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot.toml"))
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("images = { front = \"textures/robot_"))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

fn copy(name: &str, from: AnimationFrom, skip_missing: bool) -> Edit {
    Edit::AddAnimation {
        name: name.into(),
        looping: false,
        from: Some(from),
        skip_missing,
    }
}

fn refused(text: &str, path: &Path, edit: &Edit) -> (Code, String) {
    match apply(text, Syntax::Toml, path, "test.toml", edit) {
        Err(Failure::Usage(issue)) => (issue.code, issue.message.clone()),
        Err(other) => panic!("refused another way: {other:?}"),
        Ok(applied) => panic!("not refused: {}", applied.summary),
    }
}

/// A frame with a door on a hinge at its side.
const DOOR: &str = r#"version = 0
name = "door"

[[parts]]
name = "frame"
shape = "box"
size = [0.1, 2, 0.1]
sockets = { hinge = { on = "right" } }

[[parts]]
name = "door"
shape = "box"
size = [0.9, 2, 0.05]
attach = "left"
to = "frame.hinge"
"#;

/// A box on four wheels.
const CART: &str = r#"version = 0
name = "cart"

[[parts]]
name = "bed"
shape = "box"
size = [0.6, 0.2, 1]

[parts.sockets]
front_left = { on = "left", shift = [0, 0, 0.7] }
front_right = { on = "right", shift = [0, 0, 0.7] }
back_left = { on = "left", shift = [0, 0, -0.7] }
back_right = { on = "right", shift = [0, 0, -0.7] }

[[parts]]
name = "wheel_front_left"
shape = "box"
size = [0.05, 0.3, 0.3]
attach = "right"
to = "bed.front_left"

[[parts]]
name = "wheel_front_right"
shape = "box"
size = [0.05, 0.3, 0.3]
attach = "left"
to = "bed.front_right"

[[parts]]
name = "wheel_back_left"
shape = "box"
size = [0.05, 0.3, 0.3]
attach = "right"
to = "bed.back_left"

[[parts]]
name = "wheel_back_right"
shape = "box"
size = [0.05, 0.3, 0.3]
attach = "left"
to = "bed.back_right"
"#;

#[test]
fn save_animation_writes_the_animation_and_only_the_poses_it_uses() {
    let saved = animation_recipe(&robot_text(), Syntax::Toml, "robot.toml", "wave").unwrap();
    assert!(saved.starts_with("# Saved by `duck-mold save-animation` from robot.toml.\n"));
    assert!(saved.contains("# It moves: left_arm, left_arm/elbow, head.\n"));
    assert!(saved.contains("version = 0\nname = \"wave\"\n"));
    assert!(saved.contains("[poses.wave]\nleft_arm = [170, 0, 0]\n\"left_arm/elbow\" = [0, 0, 10]\nhead = [0, -15, 0]\n"));
    assert!(saved.contains("[poses.wave_out]"));
    // Only the poses the keys use: not the walk's.
    assert!(!saved.contains("step_left"));
    assert!(saved.ends_with(
        "[animations.wave]\nkeys = [\n  { at = 0, pose = \"rest\" },\n  { at = 0.35, pose = \"wave\" },\n  { at = 0.6, pose = \"wave_out\" },\n  { at = 0.85, pose = \"wave\" },\n  { at = 1.1, pose = \"wave_out\" },\n  { at = 1.5, pose = \"rest\" },\n]\n"
    ));
    // It reads back as TOML.
    assert!(parse(&saved, Syntax::Toml).is_ok());

    let walk = animation_recipe(&robot_text(), Syntax::Toml, "robot.toml", "walk").unwrap();
    assert!(walk.contains("[animations.walk]\nloop = true\nkeys = ["));

    let Err(Failure::Usage(issue)) =
        animation_recipe(&robot_text(), Syntax::Toml, "robot.toml", "dance")
    else {
        panic!("an animation that isn't there");
    };
    assert_eq!(issue.code, Code::UnknownAnimation);
    assert!(
        issue.message.contains("there is no animation \"dance\""),
        "{}",
        issue.message
    );
}

#[test]
fn save_animation_writes_a_new_file_and_never_over_one() {
    let folder = folder("save_animation");
    let recipe = folder.join("robot.toml");
    std::fs::write(&recipe, robot_text()).unwrap();
    let file = folder.join("animations").join("walk.toml");
    let text = save_animation(&recipe, "walk", &file).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    // The recipe doesn't change.
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), robot_text());
    let Err(Failure::Usage(issue)) = save_animation(&recipe, "walk", &file) else {
        panic!("written over");
    };
    assert!(
        issue.message.contains("never writes over a file"),
        "{}",
        issue.message
    );
    let Err(Failure::Usage(issue)) = save_animation(&recipe, "walk", &folder.join("walk.txt"))
    else {
        panic!("not a recipe's name");
    };
    assert!(issue.message.contains(".toml"), "{}", issue.message);
}

#[test]
fn a_saved_animation_comes_in_with_its_poses_and_reuses_those_that_say_the_same() {
    let folder = folder("add_animation_from");
    let recipe = folder.join("robot.toml");
    std::fs::write(&recipe, robot_text()).unwrap();
    save_animation(&recipe, "walk", &folder.join("animations/walk.toml")).unwrap();

    // Into the robot itself, by another name: its poses are already
    // there and say the same, so none is added.
    let applied = apply(
        &robot_text(),
        Syntax::Toml,
        &recipe,
        "robot.toml",
        &copy(
            "stroll",
            AnimationFrom::File {
                file: "animations/walk.toml".into(),
                take: None,
            },
            false,
        ),
    )
    .unwrap();
    assert_eq!(
        applied.summary,
        "added the animation \"stroll\", a copy of \"walk\" from \"animations/walk.toml\", with 5 keys; it uses the poses \"step_left\" and \"step_right\" already here, which say the same"
    );
    let added = &applied.text[robot_text().len()..];
    assert_eq!(
        added,
        "\n[animations.stroll]\nloop = true\nkeys = [\n  { at = 0, pose = \"step_left\", event = \"footstep\" },\n  { at = 0.2, pose = \"rest\" },\n  { at = 0.4, pose = \"step_right\", event = \"footstep\" },\n  { at = 0.6, pose = \"rest\" },\n  { at = 0.8, pose = \"step_left\" },\n]\n"
    );
    let model = applied.model.unwrap();
    let stroll = model.animation("stroll").unwrap();
    let walk = model.animation("walk").unwrap();
    let keys = |animation: &duck_mold_core::motion::Animation| -> Vec<(f64, String)> {
        animation
            .keys
            .iter()
            .map(|key| (key.at, key.pose.clone()))
            .collect()
    };
    assert_eq!(keys(stroll), keys(walk));
    assert!(stroll.looping);

    // A robot whose step_left says something else: the copy's comes in
    // as step_left_2, and its keys use that.
    let changed = robot_text().replace(
        "[poses.step_left]\nbody = { move = [0, -0.04, 0] }\nleft_leg = [25, 0, 0]",
        "[poses.step_left]\nbody = { move = [0, -0.04, 0] }\nleft_leg = [30, 0, 0]",
    );
    assert_ne!(changed, robot_text());
    let applied = apply(
        &changed,
        Syntax::Toml,
        &recipe,
        "robot.toml",
        &copy(
            "stroll",
            AnimationFrom::File {
                file: "animations/walk.toml".into(),
                take: None,
            },
            false,
        ),
    )
    .unwrap();
    assert!(
        applied.summary.contains("and the pose \"step_left_2\" (names taken by other poses here, so they came in as: \"step_left\" as \"step_left_2\"); it uses the pose \"step_right\" already here"),
        "{}",
        applied.summary
    );
    assert!(
        applied.text.contains(
            "[poses.step_left_2]\nbody = { move = [0, -0.04, 0] }\nleft_leg = [25, 0, 0]\n"
        )
    );
    assert!(
        applied
            .text
            .contains("  { at = 0, pose = \"step_left_2\", event = \"footstep\" },\n")
    );
    // The pose goes after the other poses, the animation after the
    // other animations.
    let pose_at = applied.text.find("[poses.step_left_2]").unwrap();
    assert!(pose_at > applied.text.find("[poses.breathe_out]").unwrap());
    assert!(pose_at < applied.text.find("[animations.idle]").unwrap());
}

#[test]
fn a_file_is_found_from_the_recipes_folder_and_a_file_with_several_needs_the_name() {
    let folder = folder("add_animation_names");
    let recipe = folder.join("puppet.toml");
    // The whole robot recipe is a file to copy from too: it has three
    // animations, so the name must be one of them.
    std::fs::write(folder.join("robot.toml"), robot_text()).unwrap();
    let text = robot_text();
    let (code, message) = refused(
        &text,
        &recipe,
        &copy(
            "dance",
            AnimationFrom::File {
                file: "robot.toml".into(),
                take: None,
            },
            false,
        ),
    );
    assert_eq!(code, Code::UnknownAnimation);
    assert!(
        message.contains("\"robot.toml\" has more than one animation and none called \"dance\": name the one to copy"),
        "{message}"
    );
    let (code, message) = refused(
        &text,
        &recipe,
        &copy(
            "walk",
            AnimationFrom::File {
                file: "nowhere.toml".into(),
                take: None,
            },
            false,
        ),
    );
    assert_eq!(code, Code::Io);
    assert!(
        message.contains("there is no file \"nowhere.toml\""),
        "{message}"
    );
    // A name already taken here.
    let (code, message) = refused(
        &text,
        &recipe,
        &copy(
            "walk",
            AnimationFrom::File {
                file: "robot.toml".into(),
                take: None,
            },
            false,
        ),
    );
    assert_eq!(code, Code::DuplicateName);
    assert!(
        message.contains("there is already an animation called \"walk\""),
        "{message}"
    );
}

#[test]
fn parts_the_model_lacks_are_listed_or_left_out() {
    let path = Path::new("door.toml");
    let (code, message) = refused(
        DOOR,
        path,
        &copy("walk", AnimationFrom::Starter("walk".into()), false),
    );
    assert_eq!(code, Code::UnknownPart);
    assert_eq!(
        message,
        "the animation moves the parts \"left_leg\", \"right_leg\", \"left_arm\" and \"right_arm\", which this model doesn't have. Give the parts those names (a two-legged character's are body, head, left_arm, right_arm, left_leg, right_leg), or add --skip-missing to take the rest"
    );
    // With nothing left, --skip-missing can't help.
    let (code, message) = refused(
        DOOR,
        path,
        &copy("walk", AnimationFrom::Starter("walk".into()), true),
    );
    assert_eq!(code, Code::UnknownPart);
    assert!(
        message.starts_with("none of the parts the animation moves are in this model"),
        "{message}"
    );

    // A one-armed figure takes the walk without the other arm.
    let applied = apply(
        ONE_ARM,
        Syntax::Toml,
        Path::new("one_arm.toml"),
        "one_arm.toml",
        &copy("amble", AnimationFrom::Starter("walk".into()), true),
    )
    .unwrap();
    assert_eq!(
        applied.summary,
        "added the animation \"amble\", a copy of \"walk\" from the starters, with 5 keys and the poses \"walk_left\" and \"walk_right\"; it left out what it does to \"right_arm\", which this model doesn't have"
    );
    let walk_left = applied.text.split("[poses.walk_left]\n").nth(1).unwrap();
    let walk_left = &walk_left[..walk_left.find("\n\n").unwrap()];
    assert_eq!(
        walk_left,
        "left_leg = [20, 0, 0]\nright_leg = [-20, 0, 0]\nleft_arm = [-20, 0, 0]"
    );
    let model = applied.model.unwrap();
    assert!(model.animation("amble").unwrap().looping);
}

/// A figure with a body, a head, two legs and one arm.
const ONE_ARM: &str = r#"version = 0
name = "one_arm"

[[parts]]
name = "body"
shape = "box"
size = [0.4, 0.6, 0.2]

[parts.sockets]
neck = { on = "top" }
arm = { on = "left", shift = [0, 0.8, 0] }
left_hip = { on = "bottom", shift = [-0.5, 0, 0] }
right_hip = { on = "bottom", shift = [0.5, 0, 0] }

[[parts]]
name = "head"
shape = "box"
size = [0.25, 0.25, 0.25]
attach = "bottom"
to = "body.neck"

[[parts]]
name = "left_arm"
shape = "box"
size = [0.1, 0.5, 0.1]
attach = "right"
to = "body.arm"

[[parts]]
name = "left_leg"
shape = "box"
size = [0.12, 0.5, 0.12]
attach = "top"
to = "body.left_hip"

[[parts]]
name = "right_leg"
shape = "box"
size = [0.12, 0.5, 0.12]
attach = "top"
to = "body.right_hip"
"#;

#[test]
fn every_starter_names_the_parts_it_says_and_comes_in_whole() {
    let robot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot.toml");
    for starter in STARTERS {
        // The file says what the list says.
        let header = format!("# Starter animation `{}`", starter.name);
        assert!(starter.text.starts_with(&header), "{}", starter.name);
        assert!(
            starter
                .text
                .contains(&format!("name = \"{}\"", starter.name))
        );
        assert!(
            starter
                .text
                .contains(&format!("[animations.{}]", starter.name))
        );
        let saved_parts: Vec<&str> = starter
            .text
            .lines()
            .filter(|line| !line.starts_with(['#', '[', ' ']) && line.contains(" = "))
            .map(|line| line.split(" = ").next().unwrap())
            .filter(|field| !["version", "name", "loop", "smooth", "keys"].contains(field))
            .collect();
        for part in &saved_parts {
            assert!(starter.parts.contains(part), "{}: {part}", starter.name);
        }
        for part in starter.parts {
            assert!(saved_parts.contains(part), "{}: {part}", starter.name);
        }

        let (text, path) = match starter.made_for {
            "two legs" => {
                for part in starter.parts {
                    assert!(TWO_LEGS.contains(part), "{}: {part}", starter.name);
                }
                (robot_text(), robot.as_path())
            }
            "a door" => (DOOR.to_owned(), Path::new("door.toml")),
            "four wheels" => (CART.to_owned(), Path::new("cart.toml")),
            other => panic!("{other}"),
        };
        let name = format!("my_{}", starter.name);
        let applied = apply(
            &text,
            Syntax::Toml,
            path,
            "test.toml",
            &copy(&name, AnimationFrom::Starter(starter.name.into()), false),
        )
        .unwrap_or_else(|failure| panic!("{}: {failure:?}", starter.name));
        let model = applied.model.unwrap();
        let animation = model.animation(&name).unwrap();
        assert!(animation.keys.len() >= 2, "{}", starter.name);
        assert_eq!(animation.keys.first().unwrap().at, 0.0);
    }
    // Loops and smoothness come along.
    let applied = apply(
        CART,
        Syntax::Toml,
        Path::new("cart.toml"),
        "cart.toml",
        &copy("roll", AnimationFrom::Starter("roll".into()), false),
    )
    .unwrap();
    assert!(applied.text.ends_with(
        "[poses.roll_round]\nwheel_front_left = [-360, 0, 0]\nwheel_front_right = [-360, 0, 0]\nwheel_back_left = [-360, 0, 0]\nwheel_back_right = [-360, 0, 0]\n\n[animations.roll]\nloop = true\nsmooth = false\nkeys = [\n  { at = 0, pose = \"rest\" },\n  { at = 1, pose = \"roll_round\" },\n]\n"
    ), "{}", applied.text);
    assert_eq!(
        applied.summary,
        "added the animation \"roll\" from the starters, with 2 keys and the pose \"roll_round\""
    );
    let model = applied.model.unwrap();
    let roll = model.animation("roll").unwrap();
    assert!(roll.looping);
    assert!(!roll.smooth);

    assert!(starter("walk").is_some());
    let (code, message) = refused(
        DOOR,
        Path::new("door.toml"),
        &copy("x", AnimationFrom::Starter("dance".into()), false),
    );
    assert_eq!(code, Code::UnknownAnimation);
    assert_eq!(message, "there is no starter animation \"dance\"");
}

#[test]
fn a_starter_goes_into_a_json_recipe_too() {
    let json = r#"{
  "version": 0,
  "name": "door",
  "parts": [
    { "name": "frame", "shape": "box", "size": [0.1, 2, 0.1], "sockets": { "hinge": { "on": "right" } } },
    { "name": "door", "shape": "box", "size": [0.9, 2, 0.05], "attach": "left", "to": "frame.hinge" }
  ]
}
"#;
    let applied = apply(
        json,
        Syntax::Json,
        Path::new("door.json"),
        "door.json",
        &copy("open", AnimationFrom::Starter("open".into()), false),
    )
    .unwrap();
    assert!(
        serde_json::from_str::<serde_json::Value>(&applied.text).is_ok(),
        "{}",
        applied.text
    );
    let model = applied.model.unwrap();
    assert_eq!(model.animation("open").unwrap().keys.len(), 2);
    assert!(model.pose("door_open").is_some());
}
