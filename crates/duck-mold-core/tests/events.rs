//! Phase 6, step 4: events on keys. What the recipe says, the warning
//! where a loop joins, `set-event` and `remove-event`, events kept when
//! keys move or animations are copied and saved, and what `inspect
//! --animation` says.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::{AnimationFrom, Applied, Edit, animation_recipe, apply};
use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::model::Model;
use duck_mold_core::source::Syntax;
use duck_mold_core::tools::{self, Failure};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn load(text: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded =
        duck_mold_core::load::load_text(text, Syntax::Toml, "test.toml", &examples(), &mut issues);
    let model = duck_mold_core::assemble::assemble(&loaded, &mut issues);
    (model, issues)
}

fn built(text: &str) -> Model {
    let (model, issues) = load(text);
    model.unwrap_or_else(|| panic!("it builds: {issues:#?}"))
}

fn errors(text: &str) -> Vec<Issue> {
    load(text).1.into_iter().filter(Issue::is_error).collect()
}

/// The warnings, but for the leg's having no colours.
fn warnings(text: &str) -> Vec<Issue> {
    load(text)
        .1
        .into_iter()
        .filter(|issue| !issue.is_error() && issue.code != Code::NoColor)
        .collect()
}

fn done(text: &str, edit: &Edit) -> Applied {
    apply(
        text,
        Syntax::Toml,
        &examples().join("test.toml"),
        "test.toml",
        edit,
    )
    .unwrap_or_else(|failure| panic!("{failure:?}"))
}

fn refused(text: &str, edit: &Edit) -> (Code, String) {
    match apply(
        text,
        Syntax::Toml,
        &examples().join("test.toml"),
        "test.toml",
        edit,
    ) {
        Err(Failure::Usage(issue)) => (issue.code, issue.message.clone()),
        Err(other) => panic!("refused another way: {other:?}"),
        Ok(applied) => panic!("not refused: {}", applied.summary),
    }
}

/// A leg that swings, with a walk of five keys.
const LEG: &str = r#"version = 0
name = "leg"

[[parts]]
name = "hip"
shape = "box"
size = [0.4, 0.2, 0.2]
sockets = { knee = { on = "bottom" } }

[[parts]]
name = "leg"
shape = "box"
size = [0.1, 0.6, 0.1]
attach = "top"
to = "hip.knee"

[poses.forward]
leg = [30, 0, 0]

[poses.back]
leg = [-30, 0, 0]

[animations.walk]
loop = true
keys = [
  { at = 0, pose = "forward" },
  { at = 0.2, pose = "rest" },
  { at = 0.4, pose = "back" },
  { at = 0.6, pose = "rest" },
  { at = 0.8, pose = "forward" },
]
"#;

fn with_events(first: &str, third: &str) -> String {
    LEG.replace(
        "{ at = 0, pose = \"forward\" }",
        &format!("{{ at = 0, pose = \"forward\"{first} }}"),
    )
    .replace(
        "{ at = 0.4, pose = \"back\" }",
        &format!("{{ at = 0.4, pose = \"back\"{third} }}"),
    )
}

fn walk_events(model: &Model) -> Vec<Vec<String>> {
    model
        .animation("walk")
        .unwrap()
        .keys
        .iter()
        .map(|key| key.events.clone())
        .collect()
}

fn set_event(key: usize, name: &str) -> Edit {
    Edit::SetEvent {
        animation: "walk".into(),
        key,
        name: name.into(),
    }
}

fn remove_event(key: usize, name: Option<&str>) -> Edit {
    Edit::RemoveEvent {
        animation: "walk".into(),
        key,
        name: name.map(str::to_owned),
    }
}

#[test]
fn a_key_carries_one_event_or_a_list_and_the_motion_is_the_same() {
    let text = with_events(
        ", event = \"footstep\"",
        ", event = [\"footstep\", \"dust\"]",
    );
    let model = built(&text);
    assert_eq!(
        walk_events(&model),
        [
            vec!["footstep".to_owned()],
            vec![],
            vec!["footstep".to_owned(), "dust".to_owned()],
            vec![],
            vec![],
        ]
    );
    // Events don't change the motion: every pose and time is the same.
    let plain = built(LEG);
    let (one, other) = (
        model.animation("walk").unwrap(),
        plain.animation("walk").unwrap(),
    );
    for (a, b) in one.keys.iter().zip(&other.keys) {
        assert_eq!((a.at, &a.pose), (b.at, &b.pose));
    }
    assert!(warnings(&text).is_empty(), "{:#?}", warnings(&text));
}

#[test]
fn a_wrong_event_is_an_error_that_says_how_to_write_it() {
    for (event, words) in [
        (
            " , event = 4",
            "the name of an event in quotes, like \"footstep\"",
        ),
        (", event = \"foot step\"", "can't be an event's name"),
        (
            ", event = [\"thud\", 2]",
            "text in quotes, like \"footstep\"",
        ),
        (
            ", event = [\"thud\", \"thud\"]",
            "has the event \"thud\" twice",
        ),
    ] {
        let found = errors(&with_events(event, ""));
        assert_eq!(found.len(), 1, "{event}: {found:#?}");
        assert!(found[0].message.contains(words), "{}", found[0].message);
        assert!(found[0].path.starts_with("animations.walk.keys[0].event"));
    }
    // A misspelt field is said too, with the word it means.
    let found = errors(&with_events(", events = \"thud\"", ""));
    assert_eq!(found[0].code, Code::UnknownField);
    assert!(
        found[0]
            .suggestion
            .as_deref()
            .unwrap_or("")
            .contains("event"),
        "{:?}",
        found[0]
    );
}

#[test]
fn the_same_event_where_a_loop_joins_is_a_warning() {
    // The first key and the last are the same moment of a loop.
    let text = LEG
        .replace(
            "{ at = 0, pose = \"forward\" }",
            "{ at = 0, pose = \"forward\", event = [\"footstep\", \"dust\"] }",
        )
        .replace(
            "{ at = 0.8, pose = \"forward\" }",
            "{ at = 0.8, pose = \"forward\", event = \"footstep\" }",
        );
    let found = warnings(&text);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, Code::EventTwice);
    assert_eq!(found[0].code.as_str(), "event_twice");
    assert!(
        found[0].message.contains(
            "the animation \"walk\" loops, and its first and last keys both have the event \"footstep\""
        ),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("remove-event walk 5 footstep"));
    assert!(found[0].path.starts_with("animations.walk.keys[4]"));
    // Played once, the two ends are two moments: no warning.
    let once = text.replace("loop = true\n", "");
    assert!(
        warnings(&once)
            .iter()
            .all(|issue| issue.code != Code::EventTwice)
    );
    // Different events at the two ends: no warning.
    let different = text.replace("event = \"footstep\" }", "event = \"thud\" }");
    assert!(
        warnings(&different)
            .iter()
            .all(|issue| issue.code != Code::EventTwice)
    );
}

#[test]
fn set_event_adds_to_a_key_and_remove_event_takes_one_or_all_away() {
    let one = done(LEG, &set_event(3, "footstep"));
    assert_eq!(
        one.summary,
        "key 3 of \"walk\", at 0.4 s (the pose \"back\"), has the event \"footstep\""
    );
    assert!(
        one.text
            .contains("  { at = 0.4, pose = \"back\", event = \"footstep\" },\n")
    );
    // Only that line changed.
    assert_eq!(one.lines, Some([28, 28]), "{:?}", one.lines);
    let two = done(&one.text, &set_event(3, "dust"));
    assert!(
        two.text
            .contains("  { at = 0.4, pose = \"back\", event = [\"footstep\", \"dust\"] },\n")
    );
    assert!(
        two.summary
            .ends_with("has the event \"dust\" now too: footstep, dust")
    );
    assert_eq!(
        walk_events(two.model.as_ref().unwrap())[2],
        ["footstep", "dust"]
    );
    // The same again is refused.
    let (code, message) = refused(&two.text, &set_event(3, "dust"));
    assert_eq!(code, Code::Conflict);
    assert!(message.contains("has the event \"dust\" already"));
    // A bad name, a key that isn't there, an animation that isn't.
    assert_eq!(
        refused(LEG, &set_event(3, "foot step")).0,
        Code::TypeMismatch
    );
    let (code, message) = refused(LEG, &set_event(9, "thud"));
    assert_eq!(code, Code::OutOfRange);
    assert!(message.contains("numbered from 1 to 5"), "{message}");
    let (code, _) = refused(
        LEG,
        &Edit::SetEvent {
            animation: "run".into(),
            key: 1,
            name: "thud".into(),
        },
    );
    assert_eq!(code, Code::UnknownAnimation);

    // Taking one away leaves the other; taking the last leaves the key
    // as it was before.
    let back = done(&two.text, &remove_event(3, Some("footstep")));
    assert_eq!(
        back.summary,
        "took the event \"footstep\" off key 3 of \"walk\""
    );
    assert!(
        back.text
            .contains("  { at = 0.4, pose = \"back\", event = \"dust\" },\n")
    );
    let (code, message) = refused(&back.text, &remove_event(3, Some("dusty")));
    assert_eq!(code, Code::UnknownEvent);
    assert_eq!(
        message,
        "key 3 of \"walk\" has no event \"dusty\": it has \"dust\""
    );
    let none = done(&two.text, &remove_event(3, None));
    assert_eq!(
        none.summary,
        "took the event \"footstep\" and the event \"dust\" off key 3 of \"walk\""
    );
    assert_eq!(none.text, LEG);
    let (code, message) = refused(LEG, &remove_event(2, None));
    assert_eq!(code, Code::MissingField);
    assert!(message.contains("key 2 of \"walk\" has no events"));
}

#[test]
fn a_key_moved_keeps_its_events() {
    let text = with_events(", event = \"step\"", ", event = [\"step\", \"dust\"]");
    // A little later, staying third.
    let moved = done(
        &text,
        &Edit::MoveKey {
            animation: "walk".into(),
            key: 3,
            at: 0.5,
        },
    );
    assert!(
        moved
            .text
            .contains("{ at = 0.5, pose = \"back\", event = [\"step\", \"dust\"] }")
    );
    // Past the next key: it is taken out and put back in its new place.
    let past = done(
        &text,
        &Edit::MoveKey {
            animation: "walk".into(),
            key: 3,
            at: 0.7,
        },
    );
    let events = walk_events(past.model.as_ref().unwrap());
    assert_eq!(events[3], ["step", "dust"]);
    assert!(events[2].is_empty());
    // Changing a key's pose keeps them too.
    let posed = done(
        &text,
        &Edit::SetKey {
            animation: "walk".into(),
            key: 1,
            pose: "back".into(),
        },
    );
    assert_eq!(walk_events(posed.model.as_ref().unwrap())[0], ["step"]);
}

#[test]
fn copying_and_saving_an_animation_keeps_its_events() {
    // The starters have theirs.
    for (starter, wanted) in [
        ("walk", vec![(1, "footstep"), (3, "footstep")]),
        ("jump", vec![(2, "takeoff"), (4, "land")]),
        ("open", vec![(2, "opened")]),
    ] {
        let text = duck_mold_core::starters::starter(starter).unwrap().text;
        let mut found = Vec::new();
        for (at, line) in text
            .lines()
            .filter(|line| line.trim_start().starts_with("{ at ="))
            .enumerate()
        {
            if let Some(start) = line.find("event = \"") {
                let name = &line[start + 9..];
                found.push((at + 1, name[..name.find('"').unwrap()].to_owned()));
            }
        }
        let wanted: Vec<(usize, String)> = wanted
            .into_iter()
            .map(|(key, name)| (key, name.to_owned()))
            .collect();
        assert_eq!(found, wanted, "{starter}");
    }
    // A starter copied into a recipe brings them.
    let robot = std::fs::read_to_string(examples().join("robot.toml")).unwrap();
    let copied = done(
        &robot,
        &Edit::AddAnimation {
            name: "stroll".into(),
            looping: false,
            from: Some(AnimationFrom::Starter("walk".into())),
            skip_missing: true,
        },
    );
    let stroll = copied.model.as_ref().unwrap().animation("stroll").unwrap();
    let events: Vec<&[String]> = stroll
        .keys
        .iter()
        .map(|key| key.events.as_slice())
        .collect();
    assert_eq!(events[0], ["footstep"]);
    assert_eq!(events[2], ["footstep"]);
    assert!(events[4].is_empty(), "nothing twice where the loop joins");
    // Saved to a file of its own, it keeps them.
    let saved = animation_recipe(&robot, Syntax::Toml, "robot.toml", "jump").unwrap();
    assert!(
        saved.contains("{ at = 0.3, pose = \"jump_crouch\", event = \"takeoff\" }"),
        "{saved}"
    );
    assert!(saved.contains("{ at = 1, pose = \"jump_crouch\", event = \"land\" }"));
}

#[test]
fn inspect_says_when_each_event_is_heard() {
    let path = examples().join("robot.toml");
    let walk = tools::inspect_animation_file(&path, "walk").unwrap();
    let names: Vec<(&str, f64, usize)> = walk
        .events
        .iter()
        .map(|event| (event.name.as_str(), event.at, event.key))
        .collect();
    assert_eq!(names, [("footstep", 0.0, 1), ("footstep", 0.4, 3)]);
    assert_eq!(walk.keys[0].events, ["footstep"]);
    assert!(walk.keys[1].events.is_empty());
    assert_eq!(
        walk.event_lines(),
        ["footstep at 0 s (key 1) and 0.4 s (key 3): twice a round"]
    );
    let text = walk.to_text();
    assert!(
        text.contains("  keys         1  0 s    step_left   event: footstep\n"),
        "{text}"
    );
    assert!(
        text.contains("  events       footstep at 0 s (key 1) and 0.4 s (key 3): twice a round\n"),
        "{text}"
    );
    // Played once: no "a round".
    let jump = tools::inspect_animation_file(&path, "jump").unwrap();
    assert_eq!(
        jump.event_lines(),
        [
            "takeoff at 0.3 s (key 2): once",
            "land at 1 s (key 4): once"
        ]
    );
    // An animation with none has no events line, and none in its JSON.
    let wave = tools::inspect_animation_file(&path, "wave").unwrap();
    assert!(wave.events.is_empty());
    assert!(!wave.to_text().contains("events"));
    let json = serde_json::to_value(&wave).unwrap();
    assert!(json.get("events").is_none());
    assert!(json["keys"][0].get("events").is_none());
}
