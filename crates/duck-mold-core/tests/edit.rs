//! Phase 2, step 1: edits. Every edit changes the recipe's text only
//! where it has to, in TOML and in JSON, and the model built from the new
//! text is what the edit meant.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::{
    Applied, Changes, Edit, Free, Made, Sizes, apply, definitions, describe, edit_file, shared,
    spot, swap_in, unused_name,
};
use duck_mold_core::issue::Code;
use duck_mold_core::math::vec3;
use duck_mold_core::model::{Model, Part};
use duck_mold_core::source::{Node, Syntax, Value, parse};
use duck_mold_core::tools::Failure;

/// A small recipe with everything an edit must leave alone: comments
/// above parts and after values, blank lines, odd spacing, a definition
/// used twice, a part inside a part.
const LAMP: &str = r##"# A lamp.   (this comment stays)
version = 0
name = "lamp"

[palette]
steel = "#8A939B"   # the stand
glow  = "#FFE08A"

# One bulb, used twice.
[define.bulb]
color = "glow"

[[define.bulb.parts]]
name = "glass"
shape = "sphere"
radius = 0.1

[[define.bulb.parts]]
name = "cap"
shape = "cylinder"
radius = 0.04
height = 0.05
attach = "top"
to = "glass.bottom"

# The stand.
[[parts]]
name   = "base"
shape  = "cylinder"
radius = 0.3      # wide, so it stands
height = 0.05
color  = "steel"

[parts.sockets]
pole = { on = "top" }

[[parts]]
name = "pole"
shape = "box"
size = [0.05, 1.0, 0.05]
color = "steel"
attach = "bottom"
to = "base.pole"
twist = 15

[[parts]]
name = "left_bulb"
use = "bulb"
attach = "right"
to = "pole.left"

[[parts]]
name = "right_bulb"
use = "bulb"
attach = "left"
to = "pole.right"
"##;

/// A recipe's tree as JSON text, laid out the usual way.
fn to_json(text: &str) -> String {
    fn value(node: &Node) -> serde_json::Value {
        match &node.value {
            Value::Null | Value::Other(_) => serde_json::Value::Null,
            Value::Bool(b) => (*b).into(),
            Value::Int(i) => (*i).into(),
            Value::Float(f) => (*f).into(),
            Value::Str(s) => s.as_str().into(),
            Value::Array(items) => items.iter().map(value).collect(),
            Value::Table(entries) => serde_json::Value::Object(
                entries
                    .iter()
                    .map(|entry| (entry.key.clone(), value(&entry.node)))
                    .collect(),
            ),
        }
    }
    let root = parse(text, Syntax::Toml).unwrap();
    // serde_json sorts an object's fields by name unless asked not to,
    // which is fine here: a recipe means the same in any order of fields,
    // and lists keep their order.
    let mut out = serde_json::to_string_pretty(&value(&root)).unwrap();
    out.push('\n');
    out
}

fn path_for(syntax: Syntax) -> &'static Path {
    match syntax {
        Syntax::Toml => Path::new("test.toml"),
        Syntax::Json => Path::new("test.json"),
    }
}

fn done(text: &str, syntax: Syntax, edit: &Edit) -> Applied {
    let path = path_for(syntax);
    match apply(text, syntax, path, &path.to_string_lossy(), edit) {
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

/// The lines only the old text has, and the lines only the new text has,
/// by taking away what the two start and end with alike.
fn changed<'a>(old: &'a str, new: &'a str) -> (Vec<&'a str>, Vec<&'a str>) {
    let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let end = old[start..]
        .iter()
        .rev()
        .zip(new[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (
        old[start..old.len() - end].to_vec(),
        new[start..new.len() - end].to_vec(),
    )
}

fn part<'a>(model: &'a Model, path: &str) -> &'a Part {
    fn find<'a>(parts: &'a [Part], path: &str) -> Option<&'a Part> {
        parts.iter().find_map(|part| {
            if part.path == path {
                Some(part)
            } else {
                find(&part.parts, path)
            }
        })
    }
    find(&model.parts, path).unwrap_or_else(|| panic!("no part {path}"))
}

fn add_hat() -> Edit {
    Edit::Add {
        name: "hat".into(),
        made: Made::Shape("cone".into()),
        to: Some("pole.top".into()),
        attach: None,
        sizes: Sizes {
            radius: Some(0.2),
            height: Some(0.3),
            ..Sizes::default()
        },
        color: Some("glow".into()),
        twist: None,
        free: Free::default(),
    }
}

fn set_with(part: &str, changes: Changes) -> Edit {
    Edit::Set {
        part: part.into(),
        changes,
    }
}

fn set(part: &str) -> Edit {
    set_with(part, Changes::default())
}

// ---- each edit, and only what it has to change --------------------------------

#[test]
fn add_puts_a_new_block_after_the_last_part_and_touches_nothing_else() {
    let applied = done(LAMP, Syntax::Toml, &add_hat());
    assert_eq!(
        applied.text,
        format!(
            "{LAMP}\n[[parts]]\nname = \"hat\"\nshape = \"cone\"\nradius = 0.2\nheight = 0.3\ncolor = \"glow\"\nattach = \"bottom\"\nto = \"pole.top\"\n"
        )
    );
    assert_eq!(applied.summary, "added \"hat\" (cone) on pole.top");
    assert_eq!(applied.lines, Some([57, 65]));
    let model = applied.model.unwrap();
    let hat = part(&model, "hat");
    assert_eq!(hat.attached_to.as_deref(), Some("pole.top"));
    // It sits on the pole: its lowest point is the pole's highest.
    let pole = part(&model, "pole").bounds.unwrap();
    assert_eq!(hat.bounds.unwrap().min.y, pole.max.y);
}

#[test]
fn a_value_changes_where_it_stands_with_its_spacing_and_comment() {
    let edit = set_with(
        "base",
        Changes {
            sizes: Sizes {
                radius: Some(0.4),
                ..Sizes::default()
            },
            ..Changes::default()
        },
    );
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec!["radius = 0.3      # wide, so it stands"],
            vec!["radius = 0.4      # wide, so it stands"]
        )
    );
    assert_eq!(applied.lines, Some([30, 30]));
    assert_eq!(
        part(&applied.model.unwrap(), "base")
            .bounds
            .unwrap()
            .size()
            .x,
        0.8
    );
}

#[test]
fn a_new_field_goes_in_its_usual_place() {
    // The base has no twist and isn't joined; the pole has no segments.
    let edit = set_with(
        "pole",
        Changes {
            color: Some("glow".into()),
            twist: Some(30.0),
            face_colors: vec![("top".into(), "steel".into())],
            ..Changes::default()
        },
    );
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec![
                "color = \"steel\"",
                "attach = \"bottom\"",
                "to = \"base.pole\"",
                "twist = 15"
            ],
            vec![
                "color = \"glow\"",
                "colors = { top = \"steel\" }",
                "attach = \"bottom\"",
                "to = \"base.pole\"",
                "twist = 30"
            ]
        )
    );
}

#[test]
fn changing_a_shape_drops_the_sizes_and_faces_it_doesnt_have() {
    let with_faces = LAMP.replace(
        "size = [0.05, 1.0, 0.05]\n",
        "size = [0.05, 1.0, 0.05]\ncolors = { top = \"glow\", front = \"glow\" }\n",
    );
    let edit = set_with(
        "pole",
        Changes {
            shape: Some("cylinder".into()),
            sizes: Sizes {
                radius: Some(0.03),
                height: Some(1.0),
                ..Sizes::default()
            },
            ..Changes::default()
        },
    );
    let applied = done(&with_faces, Syntax::Toml, &edit);
    assert_eq!(
        changed(&with_faces, &applied.text),
        (
            vec![
                "shape = \"box\"",
                "size = [0.05, 1.0, 0.05]",
                "colors = { top = \"glow\", front = \"glow\" }"
            ],
            vec![
                "shape = \"cylinder\"",
                "radius = 0.03",
                "height = 1",
                "colors = { top = \"glow\" }"
            ]
        )
    );
    assert_eq!(part(&applied.model.unwrap(), "pole").kind.shape, "cylinder");
}

#[test]
fn rename_changes_the_name_and_everything_that_refers_to_it() {
    let recipe = format!("{LAMP}\n[sockets]\nhook = {{ from = \"pole.top\" }}\n").replace(
        "name = \"lamp\"\n",
        "name = \"lamp\"\norigin = \"pole.bottom\"\n",
    );
    let edit = Edit::Rename {
        part: "pole".into(),
        new_name: "stem".into(),
    };
    let applied = done(&recipe, Syntax::Toml, &edit);
    let (old, new) = changed(&recipe, &applied.text);
    let only = |lines: &[&str], word: &str| -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.contains(word))
            .map(|line| (*line).to_owned())
            .collect()
    };
    assert_eq!(
        only(&new, "stem"),
        [
            "origin = \"stem.bottom\"",
            "name = \"stem\"",
            "to = \"stem.left\"",
            "to = \"stem.right\"",
            "hook = { from = \"stem.top\" }"
        ]
    );
    assert_eq!(old.len(), new.len());
    assert!(!applied.text.contains("\"pole."));
    // `base.pole` is a socket called pole, not the part: it stays.
    assert!(applied.text.contains("to = \"base.pole\""));
    // And back again gives the first file exactly.
    let back = Edit::Rename {
        part: "stem".into(),
        new_name: "pole".into(),
    };
    assert_eq!(done(&applied.text, Syntax::Toml, &back).text, recipe);
}

#[test]
fn remove_takes_the_block_its_comment_and_its_sub_tables() {
    // The base can't go while the pole hangs from it.
    let failure = refused(
        LAMP,
        &Edit::Remove {
            part: "base".into(),
            with_attached: false,
        },
    );
    assert_eq!(failure.exit_code(), 2);
    let message = &failure.report().errors[0].message;
    assert_eq!(
        message,
        "\"pole\", \"left_bulb\", \"right_bulb\" hang from \"base\": move them first, or remove them too with --with-attached"
    );

    let applied = done(
        LAMP,
        Syntax::Toml,
        &Edit::Remove {
            part: "left_bulb".into(),
            with_attached: false,
        },
    );
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec![
                "name = \"left_bulb\"",
                "use = \"bulb\"",
                "attach = \"right\"",
                "to = \"pole.left\"",
                "",
                "[[parts]]"
            ],
            vec![]
        )
    );

    let applied = done(
        LAMP,
        Syntax::Toml,
        &Edit::Remove {
            part: "pole".into(),
            with_attached: true,
        },
    );
    assert!(
        applied
            .text
            .ends_with("[parts.sockets]\npole = { on = \"top\" }\n"),
        "{}",
        applied.text
    );
    assert_eq!(
        applied.summary,
        "removed \"pole\" and the 2 parts that hung from it"
    );
    // Removing the base too would leave no parts at all, which is a
    // mistake the recipe didn't have: refused.
    let failure = refused(
        LAMP,
        &Edit::Remove {
            part: "base".into(),
            with_attached: true,
        },
    );
    assert_eq!(failure.exit_code(), 1);
}

#[test]
fn add_then_remove_gives_the_file_back_exactly() {
    for (text, syntax) in [
        (LAMP.to_owned(), Syntax::Toml),
        (to_json(LAMP), Syntax::Json),
    ] {
        let added = done(&text, syntax, &add_hat());
        assert_ne!(added.text, text);
        let removed = done(
            &added.text,
            syntax,
            &Edit::Remove {
                part: "hat".into(),
                with_attached: false,
            },
        );
        assert_eq!(removed.text, text, "{syntax:?}");
    }
}

#[test]
fn join_moves_a_part_to_another_socket() {
    let edit = Edit::Join {
        part: "left_bulb".into(),
        to: "pole.top".into(),
        attach: Some("bottom".into()),
    };
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec!["attach = \"right\"", "to = \"pole.left\""],
            vec!["attach = \"bottom\"", "to = \"pole.top\""]
        )
    );
    // A part placed by hand loses its `move` when it is joined, and joins
    // by the side opposite the socket.
    let by_hand = LAMP.replace(
        "attach = \"bottom\"\nto = \"base.pole\"\ntwist = 15\n",
        "move = [0, 2, 0]\nrotate = [0, 45, 0]\n",
    );
    let edit = Edit::Join {
        part: "pole".into(),
        to: "base.top".into(),
        attach: None,
    };
    let applied = done(&by_hand, Syntax::Toml, &edit);
    assert_eq!(
        changed(&by_hand, &applied.text),
        (
            vec!["move = [0, 2, 0]", "rotate = [0, 45, 0]"],
            vec!["attach = \"bottom\"", "to = \"base.top\""]
        )
    );
    // A part of another list can't be joined to.
    let failure = refused(
        LAMP,
        &Edit::Join {
            part: "pole".into(),
            to: "left_bulb/glass.top".into(),
            attach: None,
        },
    );
    assert_eq!(failure.report().errors[0].code, Code::Conflict);
}

#[test]
fn duplicate_copies_the_block_with_its_comments() {
    let edit = Edit::Duplicate {
        part: "base".into(),
        name: None,
        to: None,
        mirror: false,
    };
    let applied = done(LAMP, Syntax::Toml, &edit);
    let (old, new) = changed(LAMP, &applied.text);
    assert_eq!(old, Vec::<&str>::new());
    assert_eq!(
        new,
        [
            "# The stand.",
            "[[parts]]",
            "name   = \"base_2\"",
            "shape  = \"cylinder\"",
            "radius = 0.3      # wide, so it stands",
            "height = 0.05",
            "color  = \"steel\"",
            "",
            "[parts.sockets]",
            "pole = { on = \"top\" }",
            ""
        ]
    );
    assert_eq!(applied.summary, "copied \"base\" as \"base_2\"");
}

#[test]
fn a_copy_of_a_joined_part_does_not_sit_on_the_part_it_copies() {
    let example = |name: &str| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples")
            .join(format!("{name}.toml"));
        std::fs::read_to_string(path).unwrap()
    };
    let copy = |part: &str| Edit::Duplicate {
        part: part.into(),
        name: None,
        to: None,
        mirror: false,
    };
    let table = example("table");
    // 1. The next socket nothing is joined to, facing the same way.
    let without = done(
        &table,
        Syntax::Toml,
        &Edit::Remove {
            part: "leg_back_right".into(),
            with_attached: false,
        },
    )
    .text;
    let applied = done(&without, Syntax::Toml, &copy("leg_front_left"));
    assert_eq!(
        applied.summary,
        "copied \"leg_front_left\" as \"leg_front_left_2\", on top.back_right, the next free socket"
    );
    assert!(applied.text.contains("to = \"top.back_right\""));
    // 2. With none free, a new socket beside it, on the same side, 10 cm
    // past it.
    let applied = done(&table, Syntax::Toml, &copy("leg_front_left"));
    assert_eq!(
        applied.summary,
        "copied \"leg_front_left\" as \"leg_front_left_2\", on top.spot, a new socket beside top.front_left"
    );
    let model = applied.model.unwrap();
    let (first, second) = (
        part(&model, "leg_front_left").bounds.unwrap(),
        part(&model, "leg_front_left_2").bounds.unwrap(),
    );
    assert!(
        (second.min.x - first.max.x - 0.1).abs() < 1e-6,
        "{first:?} {second:?}"
    );
    assert!((second.min.y - first.min.y).abs() < 1e-9);
    assert!((second.min.z - first.min.z).abs() < 1e-9);
    let looked = duck_mold_core::inspect::inspect(&model, "table", Vec::new());
    assert!(looked.overlapping.is_empty(), "{:?}", looked.overlapping);
    // 3. With no room beside it either, free, in front of the model, on
    // the ground.
    let robot = example("robot");
    let applied = done(&robot, Syntax::Toml, &copy("left_arm"));
    assert_eq!(
        applied.summary,
        "copied \"left_arm\" as \"left_arm_2\", standing free in front of the model: no socket beside it was free"
    );
    let model = applied.model.unwrap();
    let new = part(&model, "left_arm_2");
    assert_eq!(new.attached_to, None);
    let (arm, all) = (new.bounds.unwrap(), model.bounds().unwrap());
    assert!((arm.min.y - all.min.y).abs() < 1e-9, "{arm:?}");
    let body = part(&model, "body").bounds.unwrap();
    assert!(arm.min.z > body.max.z, "{arm:?} {body:?}");
    // It still looks the same way as the arm it copies.
    let old = part(&model, "left_arm").bounds.unwrap();
    assert!((arm.size() - old.size()).length() < 1e-9);
}

#[test]
fn a_mirrored_copy_swaps_left_and_right_and_turns_its_twist() {
    // (The pole stands straight here, so that its left and right are the
    // model's.)
    let twisted = LAMP
        .replace("to = \"base.pole\"\ntwist = 15\n", "to = \"base.pole\"\n")
        .replace("to = \"pole.left\"\n", "to = \"pole.left\"\ntwist = 20\n");
    let without_right = twisted.replace(
        "\n[[parts]]\nname = \"right_bulb\"\nuse = \"bulb\"\nattach = \"left\"\nto = \"pole.right\"\n",
        "",
    );
    let edit = Edit::Duplicate {
        part: "left_bulb".into(),
        name: None,
        to: None,
        mirror: true,
    };
    let applied = done(&without_right, Syntax::Toml, &edit);
    assert_eq!(
        changed(&without_right, &applied.text).1,
        [
            "",
            "[[parts]]",
            "name = \"right_bulb\"",
            "use = \"bulb\"",
            "attach = \"right\"",
            "to = \"pole.right\"",
            "twist = -20",
            "mirror = \"x\""
        ]
    );
    assert_eq!(
        applied.summary,
        "copied \"left_bulb\" as \"right_bulb\", mirrored, on pole.right"
    );
    // The copy is the mirror image: as far to the right as the first is
    // to the left.
    let model = applied.model.unwrap();
    let (left, right) = (
        part(&model, "left_bulb").bounds.unwrap(),
        part(&model, "right_bulb").bounds.unwrap(),
    );
    assert!(
        (left.min.x + right.max.x).abs() < 1e-9,
        "{left:?} {right:?}"
    );
    assert!((left.max.x + right.min.x).abs() < 1e-9);
    assert!(part(&model, "right_bulb").mirrored);
    // A mirrored copy of a mirrored part is the right way round again.
    let back = Edit::Duplicate {
        part: "right_bulb".into(),
        name: Some("third".into()),
        to: Some("pole.top".into()),
        mirror: true,
    };
    let again = done(&applied.text, Syntax::Toml, &back);
    assert!(!part(&again.model.unwrap(), "third").mirrored);
}

#[test]
fn palette_colours_are_added_and_changed() {
    let applied = done(
        LAMP,
        Syntax::Toml,
        &Edit::AddColor {
            name: "red".into(),
            hex: "#D0453A".into(),
        },
    );
    assert_eq!(
        changed(LAMP, &applied.text),
        (vec![], vec!["red = \"#D0453A\""])
    );
    assert!(
        applied
            .text
            .contains("glow  = \"#FFE08A\"\nred = \"#D0453A\"\n\n# One bulb")
    );

    let applied = done(
        LAMP,
        Syntax::Toml,
        &Edit::SetColor {
            name: "steel".into(),
            hex: "#000000".into(),
        },
    );
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec!["steel = \"#8A939B\"   # the stand"],
            vec!["steel = \"#000000\"   # the stand"]
        )
    );
    assert_eq!(applied.model.unwrap().palette[0].rgb, [0, 0, 0]);

    // A recipe with no palette gets one, above its parts.
    let bare = "version = 0\n\n[[parts]]\nname = \"a\"\nshape = \"box\"\n";
    let applied = done(
        bare,
        Syntax::Toml,
        &Edit::AddColor {
            name: "red".into(),
            hex: "#D0453A".into(),
        },
    );
    assert_eq!(
        applied.text,
        "version = 0\n\n[palette]\nred = \"#D0453A\"\n\n[[parts]]\nname = \"a\"\nshape = \"box\"\n"
    );

    for (edit, code) in [
        (
            Edit::AddColor {
                name: "glow".into(),
                hex: "#000000".into(),
            },
            Code::DuplicateName,
        ),
        (
            Edit::SetColor {
                name: "glw".into(),
                hex: "#000000".into(),
            },
            Code::UnknownColor,
        ),
        (
            Edit::SetColor {
                name: "glow".into(),
                hex: "yellow".into(),
            },
            Code::TypeMismatch,
        ),
    ] {
        assert_eq!(
            refused(LAMP, &edit).report().errors[0].code,
            code,
            "{edit:?}"
        );
    }
}

#[test]
fn a_colour_is_removed_once_nothing_is_painted_with_it() {
    let remove = |name: &str, replace_with: Option<&str>| Edit::RemoveColor {
        name: name.into(),
        replace_with: replace_with.map(str::to_owned),
    };
    // In use: refused, naming everything painted with it and what to do.
    let report = refused(LAMP, &remove("steel", None)).report();
    assert_eq!(report.errors[0].code, Code::Conflict);
    assert_eq!(
        report.errors[0].message,
        "the colour \"steel\" is in use: 2 things are painted with it (base, pole). Say what to paint them instead, like --replace-with glow"
    );
    // A face's colour counts too, and so does a definition's.
    let report = refused(LAMP, &remove("glow", None)).report();
    assert!(
        report.errors[0]
            .message
            .contains("1 thing is painted with it (define.bulb)"),
        "{}",
        report.errors[0].message
    );
    let with_face = done(
        LAMP,
        Syntax::Toml,
        &set_with(
            "base",
            Changes {
                face_colors: vec![("top".into(), "glow".into())],
                ..Changes::default()
            },
        ),
    );
    let report = refused(&with_face.text, &remove("glow", None)).report();
    assert!(
        report.errors[0]
            .message
            .contains("2 things are painted with it (base (top), define.bulb)"),
        "{}",
        report.errors[0].message
    );

    // Repainted first, it goes: the palette line, and every use.
    let applied = done(
        &with_face.text,
        Syntax::Toml,
        &remove("glow", Some("steel")),
    );
    assert_eq!(
        applied.summary,
        "removed the colour \"glow\", and painted 2 things with \"steel\" instead"
    );
    assert!(!applied.text.contains("glow"), "{}", applied.text);
    assert!(applied.text.contains("[define.bulb]\ncolor = \"steel\""));
    assert!(applied.text.contains("colors = { top = \"steel\" }"));
    assert!(
        applied
            .text
            .contains("steel = \"#8A939B\"   # the stand\n\n# One bulb")
    );
    assert_eq!(applied.model.unwrap().palette.len(), 1);

    // A colour nothing uses just goes.
    let extra = done(
        LAMP,
        Syntax::Toml,
        &Edit::AddColor {
            name: "gold".into(),
            hex: "#FFD700".into(),
        },
    );
    let applied = done(&extra.text, Syntax::Toml, &remove("gold", None));
    assert_eq!(applied.text, LAMP);
    assert_eq!(applied.summary, "removed the colour \"gold\"");

    // What can't be: a colour that isn't there, and replacing a colour
    // with itself or with one that isn't there.
    let report = refused(LAMP, &remove("pink", None)).report();
    assert_eq!(report.errors[0].code, Code::UnknownColor);
    for other in ["steel", "pink"] {
        let report = refused(LAMP, &remove("steel", Some(other))).report();
        assert_eq!(report.errors[0].code, Code::UnknownColor, "{other}");
        assert!(
            report.errors[0].message.contains("must be another colour"),
            "{}",
            report.errors[0].message
        );
    }
}

#[test]
fn a_faces_own_colour_is_taken_away_again() {
    let paint = |face: &str, color: &str| {
        set_with(
            "base",
            Changes {
                face_colors: vec![(face.into(), color.into())],
                ..Changes::default()
            },
        )
    };
    let top = done(LAMP, Syntax::Toml, &paint("top", "glow"));
    let both = done(&top.text, Syntax::Toml, &paint("side", "glow"));
    // What the recipe says about the part names them.
    let facts = describe(&both.text, Syntax::Toml, both.model.as_ref(), "base").unwrap();
    assert_eq!(
        facts.face_colors,
        [
            ("top".to_owned(), "glow".to_owned()),
            ("side".to_owned(), "glow".to_owned())
        ]
    );
    // One goes: the other stays.
    let one = done(&both.text, Syntax::Toml, &paint("side", "none"));
    assert_eq!(one.text, top.text);
    assert_eq!(one.summary, "set \"base\": side no colour of its own");
    // The last goes: so does the table, and the file is as it was.
    let none = done(&one.text, Syntax::Toml, &paint("top", "none"));
    assert_eq!(none.text, LAMP);
    // Taking away what isn't there changes nothing.
    let again = done(LAMP, Syntax::Toml, &paint("top", "none"));
    assert_eq!(again.text, LAMP);
}

#[test]
fn a_models_parts_know_the_names_of_their_faces() {
    let model = done(LAMP, Syntax::Toml, &add_colour_x()).model.unwrap();
    let base = part(&model, "base");
    assert_eq!(base.face_names, ["top", "bottom", "side"]);
    assert_eq!(base.face_groups.len(), base.faces.len());
    // The first face of a cylinder is its top.
    assert_eq!(base.face_name(base.faces.start), Some("top"));
    assert_eq!(base.face_name(base.faces.start + 5), Some("side"));
    assert_eq!(base.faces_named("top"), [base.faces.start]);
    assert_eq!(base.faces_named("side").len(), 16);
    assert!(base.faces_named("lid").is_empty());
    // A face of another part isn't this part's.
    let pole = part(&model, "pole");
    assert_eq!(base.face_name(pole.faces.start), None);
    assert_eq!(pole.face_name(pole.faces.start), Some("top"));
    // A part made of parts has none of its own.
    assert!(part(&model, "left_bulb").face_names.is_empty());
    assert_eq!(part(&model, "left_bulb/glass").face_names, ["side"]);
}

#[test]
fn a_socket_is_declared_and_slid_and_what_is_joined_moves_with_it() {
    // On a part that has sockets already: one more line.
    let add = Edit::AddSocket {
        part: "base".into(),
        name: "spot".into(),
        on: "top".into(),
        shift: [0.5, 0.0, 0.0],
    };
    let applied = done(LAMP, Syntax::Toml, &add);
    assert_eq!(
        changed(LAMP, &applied.text),
        (vec![], vec!["spot = { on = \"top\", shift = [0.5, 0, 0] }"])
    );
    // On one that has none: a table of its own, under the part.
    let add_on_pole = Edit::AddSocket {
        part: "pole".into(),
        name: "badge".into(),
        on: "front".into(),
        shift: [0.0; 3],
    };
    let on_pole = done(LAMP, Syntax::Toml, &add_on_pole);
    assert_eq!(
        changed(LAMP, &on_pole.text),
        (
            vec![],
            vec!["[parts.sockets]", "badge = { on = \"front\" }", ""]
        )
    );
    assert!(on_pole.text.contains("twist = 15\n\n[parts.sockets]\nbadge = { on = \"front\" }\n\n[[parts]]\nname = \"left_bulb\""));

    // Join the pole to the new socket, then slide the socket: the pole
    // moves with it.
    let joined = done(
        &applied.text,
        Syntax::Toml,
        &Edit::Join {
            part: "pole".into(),
            to: "base.spot".into(),
            attach: None,
        },
    );
    let at = |applied: &Applied| {
        let model = applied.model.as_ref().unwrap();
        let centre = |name: &str| part(model, name).bounds.unwrap().center().x;
        centre("pole") - centre("base")
    };
    assert!((at(&joined) - 0.15).abs() < 1e-5, "{}", at(&joined));
    let slid = done(
        &joined.text,
        Syntax::Toml,
        &Edit::SetSocket {
            socket: "base.spot".into(),
            on: None,
            shift: Some([-0.5, 0.0, 0.25]),
            facing: None,
        },
    );
    assert_eq!(
        changed(&joined.text, &slid.text),
        (
            vec!["spot = { on = \"top\", shift = [0.5, 0, 0] }"],
            vec!["spot = { on = \"top\", shift = [-0.5, 0, 0.25] }"]
        )
    );
    assert!((at(&slid) + 0.15).abs() < 1e-5);

    // Turned: the pole on it leans to face the front, and "out" puts the
    // socket back to facing straight out of its side.
    let facing = |way: &str| Edit::SetSocket {
        socket: "base.spot".into(),
        on: None,
        shift: None,
        facing: Some(way.into()),
    };
    let turned = done(&slid.text, Syntax::Toml, &facing("front"));
    assert_eq!(
        changed(&slid.text, &turned.text).1,
        ["spot = { on = \"top\", shift = [-0.5, 0, 0.25], facing = \"front\" }"]
    );
    assert_eq!(turned.summary, "turned the socket base.spot to face front");
    let pole = part(turned.model.as_ref().unwrap(), "pole").bounds.unwrap();
    assert!(pole.size().z > pole.size().y, "{pole:?}");
    let back = done(&turned.text, Syntax::Toml, &facing("out"));
    assert_eq!(back.text, slid.text);
    assert_eq!(
        back.summary,
        "turned the socket base.spot to face straight out"
    );
    let failure = refused(&slid.text, &facing("sideways"));
    let error = &failure.report().errors[0];
    assert_eq!(error.code, Code::TypeMismatch);
    assert!(error.message.contains("\"sideways\""), "{}", error.message);

    // A built-in socket can't be moved, and the message says what to do.
    let failure = refused(
        LAMP,
        &Edit::SetSocket {
            socket: "base.top".into(),
            on: None,
            shift: Some([0.0; 3]),
            facing: None,
        },
    );
    assert!(failure.report().errors[0].message.contains("add-socket"));
}

// ---- parts inside definitions and files ---------------------------------------

#[test]
fn a_socket_is_renamed_with_everything_that_names_it() {
    let rename = |socket: &str, new_name: &str| Edit::RenameSocket {
        socket: socket.into(),
        new_name: new_name.into(),
    };
    // The base declares `pole`, and the pole is joined to it.
    let applied = done(LAMP, Syntax::Toml, &rename("base.pole", "mast"));
    assert_eq!(
        changed(LAMP, &applied.text),
        (
            vec![
                "pole = { on = \"top\" }",
                "",
                "[[parts]]",
                "name = \"pole\"",
                "shape = \"box\"",
                "size = [0.05, 1.0, 0.05]",
                "color = \"steel\"",
                "attach = \"bottom\"",
                "to = \"base.pole\""
            ],
            vec![
                "mast = { on = \"top\" }",
                "",
                "[[parts]]",
                "name = \"pole\"",
                "shape = \"box\"",
                "size = [0.05, 1.0, 0.05]",
                "color = \"steel\"",
                "attach = \"bottom\"",
                "to = \"base.mast\""
            ]
        )
    );
    assert_eq!(
        applied.summary,
        "renamed the socket \"base.pole\" to \"mast\""
    );
    // The model is the same.
    let before = done(LAMP, Syntax::Toml, &add_colour_x()).model.unwrap();
    assert!(same_shape(&before, applied.model.as_ref().unwrap()));
    // What the recipe says about the base names its socket, and that
    // one thing uses it.
    let facts = describe(&applied.text, Syntax::Toml, applied.model.as_ref(), "base").unwrap();
    assert_eq!(facts.sockets, [("mast".to_owned(), 1)]);

    // A built-in socket can't be renamed.
    let report = refused(LAMP, &rename("base.top", "lid")).report();
    assert_eq!(report.errors[0].code, Code::UnknownSocket);
    assert!(
        report.errors[0].message.contains("is built in"),
        "{}",
        report.errors[0].message
    );
    // A name that is taken, or a built-in one, is refused.
    let two = done(
        LAMP,
        Syntax::Toml,
        &Edit::AddSocket {
            part: "base".into(),
            name: "knob".into(),
            on: "front".into(),
            shift: [0.0; 3],
        },
    );
    for taken in ["knob", "top", "slope"] {
        let report = refused(&two.text, &rename("base.pole", taken)).report();
        assert_eq!(report.errors[0].code, Code::DuplicateName, "{taken}");
    }
    let report = refused(LAMP, &rename("base.pol", "mast")).report();
    assert_eq!(report.errors[0].suggestion.as_deref(), Some("pole"));
}

#[test]
fn a_socket_is_removed_once_nothing_uses_it_and_unused_ones_are_tidied_away() {
    let remove = |socket: &str| Edit::RemoveSocket {
        socket: socket.into(),
    };
    let socket = |part: &str, name: &str| Edit::AddSocket {
        part: part.into(),
        name: name.into(),
        on: "front".into(),
        shift: [0.5, 0.0, 0.0],
    };
    // In use: refused, naming what sits on it.
    let report = refused(LAMP, &remove("base.pole")).report();
    assert_eq!(report.errors[0].code, Code::Conflict);
    assert_eq!(
        report.errors[0].message,
        "the socket \"base.pole\" is in use by: pole. Join that somewhere else first, or remove it"
    );
    // Not in use: it goes, and the file is as it was before it came.
    let with = done(LAMP, Syntax::Toml, &socket("base", "knob"));
    let without = done(&with.text, Syntax::Toml, &remove("base.knob"));
    assert_eq!(without.text, LAMP);
    assert_eq!(without.summary, "removed the socket \"base.knob\"");
    // The last socket of a part takes the table with it.
    let only = done(LAMP, Syntax::Toml, &socket("pole", "hook"));
    assert!(
        only.text.contains("[parts.sockets]\nhook = "),
        "{}",
        only.text
    );
    let none = done(&only.text, Syntax::Toml, &remove("pole.hook"));
    assert!(!none.text.contains("hook"), "{}", none.text);
    assert!(none.model.is_some());

    // Tidy: only the Studio's own names, and only those nothing uses.
    let mut text = LAMP.to_owned();
    for (part, name) in [
        ("base", "spot"),
        ("base", "spot_2"),
        ("pole", "spot"),
        ("base", "knob"),
        ("base", "spotty"),
    ] {
        text = done(&text, Syntax::Toml, &socket(part, name)).text;
    }
    // Something sits on base.spot_2.
    text = done(
        &text,
        Syntax::Toml,
        &Edit::Add {
            name: "bead".into(),
            made: Made::Shape("sphere".into()),
            to: Some("base.spot_2".into()),
            attach: None,
            sizes: Sizes {
                radius: Some(0.02),
                ..Sizes::default()
            },
            color: Some("glow".into()),
            twist: None,
            free: Free::default(),
        },
    )
    .text;
    let tidied = done(&text, Syntax::Toml, &Edit::TidySockets);
    assert_eq!(
        tidied.summary,
        "removed 2 sockets nothing uses: base.spot, pole.spot"
    );
    for kept in [
        "spot_2 = ",
        "knob = ",
        "spotty = ",
        "pole = { on = \"top\" }",
    ] {
        assert!(tidied.text.contains(kept), "{kept} went:\n{}", tidied.text);
    }
    assert_eq!(tidied.text.matches("spot = ").count(), 0, "{}", tidied.text);
    // Again there is nothing to do, and nothing changes.
    let again = done(&tidied.text, Syntax::Toml, &Edit::TidySockets);
    assert_eq!(again.text, tidied.text);
    assert_eq!(
        again.summary,
        "found no sockets to tidy away: every \"spot\" socket is in use"
    );
    // The same in JSON.
    let json = to_json(&text);
    let tidied = done(&json, Syntax::Json, &Edit::TidySockets);
    assert_eq!(
        tidied.summary,
        "removed 2 sockets nothing uses: base.spot, pole.spot"
    );
    assert!(tidied.model.is_some());
}

#[test]
fn a_part_inside_a_definition_is_edited_there_and_the_others_are_named() {
    let edit = set_with(
        "left_bulb/glass",
        Changes {
            sizes: Sizes {
                radius: Some(0.15),
                ..Sizes::default()
            },
            ..Changes::default()
        },
    );
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert_eq!(
        changed(LAMP, &applied.text),
        (vec!["radius = 0.1"], vec!["radius = 0.15"])
    );
    let also = applied.shared.unwrap();
    assert_eq!(also.definition, "bulb");
    assert_eq!(also.also_changes, ["right_bulb/glass"]);
    // Both bulbs grew, from 0.2 across to 0.3. (About: a bulb joins by
    // the side of its glass, below the middle, so it hangs a little
    // tilted, and a tilted ball of flat faces isn't exactly as tall.)
    let model = applied.model.unwrap();
    for bulb in ["left_bulb/glass", "right_bulb/glass"] {
        let tall = part(&model, bulb).bounds.unwrap().size().y;
        assert!((tall - 0.3).abs() < 0.02, "{tall}");
    }

    // The Studio asks before any edit: the same answer.
    let model = done(LAMP, Syntax::Toml, &add_hat()).model.unwrap();
    let asked = shared(LAMP, Syntax::Toml, Some(&model), "right_bulb/cap").unwrap();
    assert_eq!(asked.definition, "bulb");
    assert_eq!(asked.also_changes, ["left_bulb/cap"]);
    assert_eq!(shared(LAMP, Syntax::Toml, Some(&model), "pole"), None);

    // Adding inside a definition adds to every part that uses it.
    let edit = Edit::Add {
        name: "ring".into(),
        made: Made::Shape("cylinder".into()),
        to: Some("left_bulb/cap.bottom".into()),
        attach: None,
        sizes: Sizes::default(),
        color: None,
        twist: None,
        free: Free::default(),
    };
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert!(applied.text.contains("to = \"glass.bottom\"\n\n[[define.bulb.parts]]\nname = \"ring\"\nshape = \"cylinder\"\nattach = \"top\"\nto = \"cap.bottom\"\n\n# The stand."));
    let model = applied.model.unwrap();
    part(&model, "left_bulb/ring");
    part(&model, "right_bulb/ring");
}

#[test]
fn what_a_part_that_uses_a_definition_is_made_of_is_changed_in_the_definition() {
    // The bulb's colour is written once, in its definition.
    let edit = set_with(
        "left_bulb",
        Changes {
            color: Some("steel".into()),
            ..Changes::default()
        },
    );
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert_eq!(
        changed(LAMP, &applied.text),
        (vec!["color = \"glow\""], vec!["color = \"steel\""])
    );
    assert!(applied.text.contains("[define.bulb]\ncolor = \"steel\"\n"));
    let also = applied.shared.unwrap();
    assert_eq!(also.definition, "bulb");
    assert_eq!(also.also_changes, ["right_bulb"]);

    // Its twist is its own: only that part's lines change, and no other
    // part is named.
    let edit = set_with(
        "left_bulb",
        Changes {
            twist: Some(30.0),
            ..Changes::default()
        },
    );
    let applied = done(LAMP, Syntax::Toml, &edit);
    let (gone, new) = changed(LAMP, &applied.text);
    assert_eq!((gone, new), (vec![], vec!["twist = 30"]));
    assert!(applied.text.contains("to = \"pole.left\"\ntwist = 30\n"));
    assert_eq!(applied.shared, None);

    // A definition that is one shape: its size is the definition's too.
    let eyes = "version = 0\n[define.eye]\nshape = \"box\"\nsize = [0.1, 0.1, 0.1]\n\n[[parts]]\nname = \"head\"\nshape = \"box\"\n\n[[parts]]\nname = \"left_eye\"\nuse = \"eye\"\nattach = \"back\"\nto = \"head.front\"\n\n[[parts]]\nname = \"right_eye\"\nuse = \"eye\"\nattach = \"bottom\"\nto = \"head.top\"\n";
    let edit = set_with(
        "right_eye",
        Changes {
            sizes: Sizes {
                size: Some([0.2, 0.2, 0.1]),
                ..Sizes::default()
            },
            twist: Some(45.0),
            ..Changes::default()
        },
    );
    let applied = done(eyes, Syntax::Toml, &edit);
    assert_eq!(
        changed(eyes, &applied.text),
        (
            vec![
                "size = [0.1, 0.1, 0.1]",
                "",
                "[[parts]]",
                "name = \"head\"",
                "shape = \"box\"",
                "",
                "[[parts]]",
                "name = \"left_eye\"",
                "use = \"eye\"",
                "attach = \"back\"",
                "to = \"head.front\"",
                "",
                "[[parts]]",
                "name = \"right_eye\"",
                "use = \"eye\"",
                "attach = \"bottom\"",
                "to = \"head.top\"",
            ],
            vec![
                "size = [0.2, 0.2, 0.1]",
                "",
                "[[parts]]",
                "name = \"head\"",
                "shape = \"box\"",
                "",
                "[[parts]]",
                "name = \"left_eye\"",
                "use = \"eye\"",
                "attach = \"back\"",
                "to = \"head.front\"",
                "",
                "[[parts]]",
                "name = \"right_eye\"",
                "use = \"eye\"",
                "attach = \"bottom\"",
                "to = \"head.top\"",
                "twist = 45",
            ]
        )
    );
    assert_eq!(applied.shared.unwrap().also_changes, ["left_eye"]);
    let model = applied.model.unwrap();
    for eye in ["left_eye", "right_eye"] {
        let size = part(&model, eye).bounds.unwrap().size();
        assert!((size.x.max(size.z) - 0.2).abs() < 0.08, "{eye}: {size:?}");
    }
}

#[test]
fn describe_reads_what_the_recipe_says_about_a_part() {
    let model = done(LAMP, Syntax::Toml, &add_hat()).model.unwrap();
    let facts = |part: &str| describe(LAMP, Syntax::Toml, Some(&model), part).unwrap();

    // A shape, with its sizes; one the recipe leaves out has its default.
    let base = facts("base");
    assert_eq!(base.name.as_deref(), Some("base"));
    assert_eq!(base.made, Some(Made::Shape("cylinder".into())));
    assert_eq!(base.shape.as_deref(), Some("cylinder"));
    assert_eq!(
        base.sizes,
        // What a cylinder has, with the default for what the recipe
        // leaves out: no bevel, and straight sides.
        Sizes {
            radius: Some(0.3),
            height: Some(0.05),
            segments: Some(16),
            bevel: Some(0.0),
            bevel_steps: Some(1),
            taper: Some([1.0; 2]),
            ..Sizes::default()
        }
    );
    assert_eq!(base.color.as_deref(), Some("steel"));
    assert_eq!((base.to, base.attach, base.twist), (None, None, 0.0));
    assert_eq!((base.inside, base.uses), (None, None));

    // A joined, twisted box.
    let pole = facts("pole");
    assert_eq!(pole.sizes.size, Some([0.05, 1.0, 0.05]));
    assert_eq!(pole.sizes.radius, None);
    assert_eq!(pole.to.as_deref(), Some("base.pole"));
    assert_eq!(pole.attach.as_deref(), Some("bottom"));
    assert_eq!(pole.twist, 15.0);

    // A part made from a definition: its colour is the definition's, and
    // changing what it is made of changes the other bulb.
    let bulb = facts("left_bulb");
    assert_eq!(bulb.made, Some(Made::Use("bulb".into())));
    assert_eq!(bulb.shape, None);
    assert_eq!(bulb.sizes, Sizes::default());
    assert_eq!(bulb.color.as_deref(), Some("glow"));
    assert_eq!(bulb.to.as_deref(), Some("pole.left"));
    assert_eq!(bulb.inside, None);
    let uses = bulb.uses.unwrap();
    assert_eq!(uses.definition, "bulb");
    assert_eq!(uses.also_changes, ["right_bulb"]);

    // A part inside a definition: every edit changes the other one.
    let glass = facts("right_bulb/glass");
    assert_eq!(glass.shape.as_deref(), Some("sphere"));
    assert_eq!(glass.sizes.radius, Some(0.1));
    assert_eq!(glass.sizes.segments, Some(12));
    assert_eq!(glass.uses, None);
    let inside = glass.inside.unwrap();
    assert_eq!(inside.definition, "bulb");
    assert_eq!(inside.also_changes, ["left_bulb/glass"]);

    // The same from JSON.
    let json = to_json(LAMP);
    let pole = describe(&json, Syntax::Json, Some(&model), "pole").unwrap();
    assert_eq!(pole.sizes.size, Some([0.05, 1.0, 0.05]));
    assert_eq!(pole.twist, 15.0);

    // No such part, or a part inside another file: nothing to show.
    assert_eq!(describe(LAMP, Syntax::Toml, Some(&model), "shade"), None);
    let recipe = "version = 0\n[[parts]]\nname = \"wheel\"\nfile = \"wheel.toml\"\n";
    assert_eq!(describe(recipe, Syntax::Toml, None, "wheel/tyre"), None);
    let wheel = describe(recipe, Syntax::Toml, None, "wheel").unwrap();
    assert_eq!(wheel.made, Some(Made::File("wheel.toml".into())));
}

#[test]
fn a_point_on_a_surface_becomes_a_socket_right_there() {
    // A point of the model, on a part's surface, is turned into the side
    // and slide `add-socket` takes; the socket declared with them is at
    // that point again. On a plain box, a twisted one, a round one, and
    // a part that is turned, scaled and mirrored.
    let recipe = r##"version = 0
[[parts]]
name = "block"
shape = "box"
size = [2, 1, 1]

[[parts]]
name = "post"
shape = "box"
size = [0.2, 1.5, 0.4]
attach = "bottom"
to = "block.top"
twist = 30

[[parts]]
name = "ball"
shape = "sphere"
radius = 0.4
segments = 16
attach = "left"
to = "block.right"

[[parts]]
name = "fin"
shape = "wedge"
size = [0.4, 0.6, 1]
rotate = [20, 40, 10]
move = [0, 0.5, 3]
scale = [1, 2, 0.5]
mirror = "x"
"##;
    let path = Path::new("test.toml");
    let built = |text: &str| {
        apply(
            text,
            Syntax::Toml,
            path,
            "test.toml",
            &Edit::AddColor {
                name: "c".into(),
                hex: "#000000".into(),
            },
        )
        .unwrap()
        .model
        .unwrap()
    };
    let model = built(recipe);
    let mut checked = 0;
    for name in ["block", "post", "ball", "fin"] {
        let target = part(&model, name);
        for face in &model.faces[target.faces.clone()] {
            let Some(normal) = face.normal() else {
                continue;
            };
            // A point inside the face: nearer its first corner than its
            // middle, so it is nowhere special.
            let middle = face
                .corners
                .iter()
                .fold(vec3(0.0, 0.0, 0.0), |sum, &corner| sum + corner)
                * (1.0 / face.corners.len() as f64);
            let point = middle + (face.corners[0] - middle) * 0.3;
            let found = spot(&model, target, point, normal).unwrap();
            let edit = Edit::AddSocket {
                part: name.into(),
                name: "here".into(),
                on: found.on.clone(),
                shift: found.shift,
            };
            let after = done(recipe, Syntax::Toml, &edit).model.unwrap();
            let socket = part(&after, name)
                .sockets
                .iter()
                .find(|socket| socket.name == "here")
                .unwrap()
                .clone();
            let off = (socket.at - point).length();
            // The ball's sides curve away under the box side they are
            // reached from, so a point there can be hidden behind a
            // nearer face; everywhere else it is the same point.
            if name != "ball" {
                assert!(off < 0.005, "{name} {found:?}: {off} m off");
                assert!(
                    socket.facing.dot(normal) > 0.99,
                    "{name} {found:?}: faces {:?}, not {normal:?}",
                    socket.facing
                );
            } else {
                assert!(off < 0.12, "{name} {found:?}: {off} m off");
            }
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked} faces were tried");

    // The slide is written with at most three decimals, and is nothing
    // along the side's own direction.
    // The block is 2 wide, 1 tall and 1 deep: half a metre right of the
    // middle of its front is half way to the edge, and a quarter up is
    // half way to the top.
    let block = part(&model, "block");
    let middle = block.bounds.unwrap().center();
    let found = spot(
        &model,
        block,
        middle + vec3(0.5, 0.25, 0.5),
        vec3(0.0, 0.0, 1.0),
    )
    .unwrap();
    assert_eq!(found.on, "front");
    assert_eq!(found.shift, [0.5, 0.5, 0.0]);
}

#[test]
fn the_recipes_definitions_and_free_names_are_listed() {
    assert_eq!(definitions(LAMP, Syntax::Toml), ["bulb"]);
    assert_eq!(definitions(&to_json(LAMP), Syntax::Json), ["bulb"]);
    assert!(definitions("version = 0\n", Syntax::Toml).is_empty());
    let taken = ["box".to_owned(), "box_2".to_owned()];
    assert_eq!(unused_name("cone", &taken), "cone");
    assert_eq!(unused_name("box", &taken), "box_3");
}

// ---- Phase 3, step 1: free parts ---------------------------------------------

fn robot() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot.toml"))
        .unwrap()
}

/// Every corner of two models, compared.
fn same_shape(a: &Model, b: &Model) -> bool {
    a.faces.len() == b.faces.len()
        && a.faces.iter().zip(&b.faces).all(|(a, b)| {
            a.corners.len() == b.corners.len()
                && a.corners
                    .iter()
                    .zip(&b.corners)
                    .all(|(a, b)| (*a - *b).length() < 1e-3)
        })
}

#[test]
fn a_free_part_is_moved_turned_and_scaled() {
    // The pole stands free on the base: give it a place of its own first.
    let free = LAMP.replace("attach = \"bottom\"\nto = \"base.pole\"\ntwist = 15\n", "");
    let edit = set_with(
        "pole",
        Changes {
            move_to: Some([0.5, 1.0, -0.25]),
            rotate: Some([0.0, 45.0, 0.0]),
            scale: Some([2.0, 2.0, 2.0]),
            ..Changes::default()
        },
    );
    let applied = done(&free, Syntax::Toml, &edit);
    let (gone, new) = changed(&free, &applied.text);
    assert!(gone.is_empty(), "{gone:?}");
    // In the order a recipe usually has them; one number for a scale
    // that is the same each way.
    assert_eq!(
        new,
        ["rotate = [0, 45, 0]", "move = [0.5, 1, -0.25]", "scale = 2"]
    );
    assert_eq!(
        applied.summary,
        "set \"pole\": move [0.5, 1, -0.25], rotate [0, 45, 0], scale 2"
    );
    // Changing them again changes the numbers where they stand.
    let again = done(
        &applied.text,
        Syntax::Toml,
        &set_with(
            "pole",
            Changes {
                move_to: Some([0.5, 1.0, 0.75]),
                scale: Some([1.0, 2.0, 1.0]),
                ..Changes::default()
            },
        ),
    );
    assert_eq!(
        changed(&applied.text, &again.text),
        (
            vec!["move = [0.5, 1, -0.25]", "scale = 2"],
            vec!["move = [0.5, 1, 0.75]", "scale = [1, 2, 1]"]
        )
    );
    // The part is where the recipe says, measured from the recipe's own
    // zero, wherever the model's zero point is.
    let model = again.model.unwrap();
    let pole = part(&model, "pole");
    let at = pole.local.shift - model.zero;
    assert!((at - vec3(0.5, 1.0, 0.75)).length() < 1e-9, "{at:?}");

    // The same in JSON.
    let json = to_json(&free);
    let applied = done(&json, Syntax::Json, &edit);
    assert!(
        applied.text.contains("\"move\": [0.5, 1, -0.25]"),
        "{}",
        applied.text
    );
}

/// A box to add with no socket: 0.2 wide, 0.4 tall.
fn add_free(name: &str, free: Free) -> Edit {
    Edit::Add {
        name: name.into(),
        made: Made::Shape("box".into()),
        to: None,
        attach: None,
        sizes: Sizes {
            size: Some([0.2, 0.4, 0.2]),
            ..Sizes::default()
        },
        color: Some("steel".into()),
        twist: None,
        free,
    }
}

#[test]
fn a_part_added_without_a_socket_stands_free_on_the_ground() {
    let before = done(LAMP, Syntax::Toml, &add_colour_x()).model.unwrap();
    // The lamp's base is 0.05 thick round the recipe's zero, so the
    // ground is at -0.025, and the middle of a box 0.4 tall is 0.2 above.
    // With no place given, it walks from the middle of the lamp towards
    // its front until it stands clear, 10 cm from the base: the base is
    // 0.3 round and the box 0.1 from its middle to its sides.
    let applied = done(LAMP, Syntax::Toml, &add_free("crate", Free::default()));
    assert!(
        applied.text.ends_with(
            "to = \"pole.right\"\n\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.2, 0.4, 0.2]\ncolor = \"steel\"\nmove = [0, 0.175, 0.5]\n"
        ),
        "{}",
        applied.text
    );
    assert!(applied.text.starts_with(LAMP));
    assert_eq!(applied.summary, "added \"crate\" (box), standing free");
    let model = applied.model.unwrap();
    let new = part(&model, "crate");
    assert_eq!(new.attached_to, None);
    // It stands on the same ground as the lamp, and the lamp hasn't moved.
    let (low, ground) = (new.bounds.unwrap().min.y, model.bounds().unwrap().min.y);
    assert!((low - ground).abs() < 1e-9, "{low} {ground}");
    // (In the recipe's own measurements: the picture is centred again
    // round the whole model.)
    let own = |model: &Model, name: &str| {
        part(model, name)
            .bounds
            .map(|bounds| (bounds.min - model.zero, bounds.max - model.zero))
    };
    for name in ["base", "pole", "left_bulb", "right_bulb"] {
        let (now, was) = (own(&model, name).unwrap(), own(&before, name).unwrap());
        assert!(
            (now.0 - was.0).length() < 1e-9 && (now.1 - was.1).length() < 1e-9,
            "{name}: {now:?} {was:?}"
        );
    }
    // And it stands clear of the lamp.
    let (crate_box, base) = (new.bounds.unwrap(), part(&model, "base").bounds.unwrap());
    assert!(crate_box.min.z >= base.max.z + 0.1 - 1e-9, "{crate_box:?}");

    // Somewhere else on the ground: its middle is over the place.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &add_free(
            "crate",
            Free {
                stand: Some([1.0, -0.5]),
                ..Free::default()
            },
        ),
    );
    assert!(
        applied.text.contains("move = [1, 0.175, -0.5]\n"),
        "{}",
        applied.text
    );

    // Turned on its side and twice as big, it still stands on the ground.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &add_free(
            "crate",
            Free {
                rotate: Some([0.0, 0.0, 90.0]),
                scale: Some([2.0; 3]),
                ..Free::default()
            },
        ),
    );
    assert!(
        applied
            .text
            .ends_with("rotate = [0, 0, 90]\nmove = [0, 0.175, 0.6]\nscale = 2\n"),
        "{}",
        applied.text
    );
    let model = applied.model.unwrap();
    let low = part(&model, "crate").bounds.unwrap().min.y;
    assert!((low - model.bounds().unwrap().min.y).abs() < 1e-9, "{low}");

    // An exact place is written as it is given.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &add_free(
            "crate",
            Free {
                move_to: Some([0.5, 2.0, 0.0]),
                ..Free::default()
            },
        ),
    );
    assert!(
        applied.text.ends_with("move = [0.5, 2, 0]\n"),
        "{}",
        applied.text
    );

    // The same in JSON.
    let applied = done(
        &to_json(LAMP),
        Syntax::Json,
        &add_free("crate", Free::default()),
    );
    assert!(
        applied.text.contains("\"move\": [0, 0.175, 0.5]"),
        "{}",
        applied.text
    );
    assert!(applied.model.is_some());
}

#[test]
fn the_first_part_of_an_empty_recipe_makes_its_own_ground() {
    let empty = "version = 0\nname = \"empty\"\n\n[palette]\nsteel = \"#8A939B\"\n";
    let applied = done(empty, Syntax::Toml, &add_free("crate", Free::default()));
    assert!(
        applied.text.ends_with(
            "\n[[parts]]\nname = \"crate\"\nshape = \"box\"\nsize = [0.2, 0.4, 0.2]\ncolor = \"steel\"\nmove = [0, 0, 0]\n"
        ),
        "{}",
        applied.text
    );
    assert_eq!(applied.model.unwrap().parts.len(), 1);
    // The same in JSON, where the list is started on one line.
    let applied = done(
        &to_json(empty),
        Syntax::Json,
        &add_free("crate", Free::default()),
    );
    assert!(
        applied.text.contains("\"parts\": [{ \"name\": \"crate\", "),
        "{}",
        applied.text
    );
    assert_eq!(applied.model.unwrap().parts.len(), 1);
}

#[test]
fn a_free_part_and_a_join_dont_mix() {
    // A join's words on a part that stands free.
    let mut edit = add_free("crate", Free::default());
    if let Edit::Add { attach, .. } = &mut edit {
        *attach = Some("top".into());
    }
    let report = refused(LAMP, &edit).report();
    assert_eq!(report.errors[0].code, Code::Conflict);
    assert!(
        report.errors[0].message.contains("--to"),
        "{}",
        report.errors[0].message
    );
    // A place on a part that is joined.
    let mut edit = add_free(
        "crate",
        Free {
            stand: Some([1.0, 0.0]),
            ..Free::default()
        },
    );
    if let Edit::Add { to, .. } = &mut edit {
        *to = Some("pole.top".into());
    }
    let report = refused(LAMP, &edit).report();
    assert_eq!(report.errors[0].code, Code::Conflict);
    // A name the model's list already has.
    let report = refused(LAMP, &add_free("pole", Free::default())).report();
    assert_eq!(report.errors[0].code, Code::DuplicateName);
    // A joined part can be given a scale, though.
    let mut edit = add_free(
        "crate",
        Free {
            scale: Some([1.0, 2.0, 1.0]),
            ..Free::default()
        },
    );
    if let Edit::Add { to, .. } = &mut edit {
        *to = Some("pole.top".into());
    }
    let applied = done(LAMP, Syntax::Toml, &edit);
    assert!(
        applied
            .text
            .ends_with("attach = \"bottom\"\nto = \"pole.top\"\nscale = [1, 2, 1]\n"),
        "{}",
        applied.text
    );
}

#[test]
fn cut_edges_and_a_narrowed_top_are_set_and_taken_away() {
    let sizes = |sizes: Sizes| {
        set_with(
            "pole",
            Changes {
                sizes,
                ..Changes::default()
            },
        )
    };
    // The pole is a box: its edges are cut, in two flat cuts, and its
    // top made half as wide.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &sizes(Sizes {
            bevel: Some(0.01),
            bevel_steps: Some(2),
            taper: Some([0.5, 0.5]),
            ..Sizes::default()
        }),
    );
    let (gone, new) = changed(LAMP, &applied.text);
    assert!(gone.is_empty(), "{gone:?}");
    // After the size, in the order a recipe has them.
    assert_eq!(new, ["bevel = 0.01", "bevel_steps = 2", "taper = 0.5"]);
    assert_eq!(
        applied.summary,
        "set \"pole\": bevel 0.01, bevel_steps 2, taper 0.5"
    );
    let cut = applied.model.as_ref().unwrap();
    assert!(part(cut, "pole").faces.len() > 6);
    // The recipe says so when asked.
    let facts = describe(&applied.text, Syntax::Toml, applied.model.as_ref(), "pole").unwrap();
    assert_eq!(
        (
            facts.sizes.bevel,
            facts.sizes.bevel_steps,
            facts.sizes.taper
        ),
        (Some(0.01), Some(2), Some([0.5, 0.5]))
    );

    // A top that is narrowed one way only is two numbers.
    let two = done(
        &applied.text,
        Syntax::Toml,
        &sizes(Sizes {
            taper: Some([0.5, 1.0]),
            ..Sizes::default()
        }),
    );
    assert!(two.text.contains("taper = [0.5, 1]\n"), "{}", two.text);
    let facts = describe(&two.text, Syntax::Toml, two.model.as_ref(), "pole").unwrap();
    assert_eq!(facts.sizes.taper, Some([0.5, 1.0]));

    // No bevel takes its steps away with it; straight sides take the
    // taper away. The file is as it was.
    let plain = done(
        &two.text,
        Syntax::Toml,
        &sizes(Sizes {
            bevel: Some(0.0),
            taper: Some([1.0, 1.0]),
            ..Sizes::default()
        }),
    );
    assert_eq!(plain.text, LAMP);
    assert_eq!(plain.summary, "set \"pole\": no bevel, no taper");

    // One cut is what a bevel has when nothing is said: it isn't written.
    let one = done(
        &applied.text,
        Syntax::Toml,
        &sizes(Sizes {
            bevel_steps: Some(1),
            ..Sizes::default()
        }),
    );
    assert!(!one.text.contains("bevel_steps"), "{}", one.text);
    assert!(one.text.contains("bevel = 0.01\n"));

    // Changing the shape to one that has no edges to cut drops both.
    let ball = done(
        &applied.text,
        Syntax::Toml,
        &set_with(
            "pole",
            Changes {
                shape: Some("sphere".into()),
                ..Changes::default()
            },
        ),
    );
    for gone in ["bevel", "taper", "size"] {
        assert!(
            !ball.text.contains(&format!("\n{gone}")),
            "{gone} is still there:\n{}",
            ball.text
        );
    }
    // A cut too deep for the shape is refused, with why.
    let report = refused(
        LAMP,
        &sizes(Sizes {
            bevel: Some(0.03),
            ..Sizes::default()
        }),
    )
    .report();
    assert_eq!(report.errors[0].code, Code::OutOfRange);
    assert!(
        report.errors[0].message.contains("must be less than 0.025"),
        "{}",
        report.errors[0].message
    );
}

#[test]
fn the_newer_shapes_are_added_set_and_described() {
    let add = |name: &str, shape: &str, sizes: Sizes| Edit::Add {
        name: name.into(),
        made: Made::Shape(shape.into()),
        to: Some("pole.top".into()),
        attach: None,
        sizes,
        color: Some("glow".into()),
        twist: None,
        free: Free::default(),
    };
    // A ring.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &add(
            "halo",
            "torus",
            Sizes {
                radius: Some(0.2),
                tube: Some(0.03),
                ..Sizes::default()
            },
        ),
    );
    assert!(
        applied.text.ends_with(
            "name = \"halo\"\nshape = \"torus\"\nradius = 0.2\ntube = 0.03\ncolor = \"glow\"\nattach = \"bottom\"\nto = \"pole.top\"\n"
        ),
        "{}",
        applied.text
    );
    let facts = describe(&applied.text, Syntax::Toml, applied.model.as_ref(), "halo").unwrap();
    assert_eq!(
        facts.sizes,
        Sizes {
            radius: Some(0.2),
            tube: Some(0.03),
            segments: Some(16),
            ..Sizes::default()
        }
    );
    // Thicker than its hole is wide: refused.
    let report = refused(
        &applied.text,
        &set_with(
            "halo",
            Changes {
                sizes: Sizes {
                    tube: Some(0.25),
                    ..Sizes::default()
                },
                ..Changes::default()
            },
        ),
    )
    .report();
    assert!(
        report.errors[0].message.contains("close its own hole"),
        "{}",
        report.errors[0].message
    );

    // Stairs, an arch and a tube, with what each leaves out filled in.
    for (shape, given, described) in [
        (
            "stairs",
            Sizes {
                size: Some([0.2, 0.2, 0.4]),
                steps: Some(5),
                ..Sizes::default()
            },
            Sizes {
                size: Some([0.2, 0.2, 0.4]),
                steps: Some(5),
                ..Sizes::default()
            },
        ),
        (
            "arch",
            Sizes {
                size: Some([1.0, 1.0, 0.2]),
                ..Sizes::default()
            },
            Sizes {
                size: Some([1.0, 1.0, 0.2]),
                wall: Some(0.2),
                segments: Some(8),
                ..Sizes::default()
            },
        ),
        (
            "tube",
            Sizes {
                wall: Some(0.05),
                ..Sizes::default()
            },
            Sizes {
                radius: Some(0.5),
                wall: Some(0.05),
                height: Some(1.0),
                segments: Some(16),
                taper: Some([1.0, 1.0]),
                ..Sizes::default()
            },
        ),
        (
            "capsule",
            Sizes::default(),
            Sizes {
                radius: Some(0.25),
                height: Some(1.0),
                segments: Some(16),
                ..Sizes::default()
            },
        ),
        (
            "pyramid",
            Sizes::default(),
            Sizes {
                size: Some([1.0; 3]),
                ..Sizes::default()
            },
        ),
    ] {
        let applied = done(LAMP, Syntax::Toml, &add("new", shape, given));
        let facts = describe(&applied.text, Syntax::Toml, applied.model.as_ref(), "new").unwrap();
        assert_eq!(facts.shape.as_deref(), Some(shape));
        assert_eq!(facts.sizes, described, "{shape}");
        assert!(applied.model.is_some(), "{shape}");
    }

    // A size the shape doesn't have is refused, naming the ones it has.
    let report = refused(
        LAMP,
        &add(
            "new",
            "prism",
            Sizes {
                wall: Some(0.1),
                ..Sizes::default()
            },
        ),
    )
    .report();
    assert!(
        report.errors[0]
            .message
            .contains("a prism has no \"wall\": its size is given by \"size\""),
        "{}",
        report.errors[0].message
    );
}

#[test]
fn a_joined_part_isnt_moved_or_turned_but_says_how() {
    let failure = refused(
        LAMP,
        &set_with(
            "pole",
            Changes {
                move_to: Some([1.0, 0.0, 0.0]),
                ..Changes::default()
            },
        ),
    );
    assert_eq!(failure.exit_code(), 2);
    assert_eq!(
        failure.report().errors[0].message,
        "\"pole\" is joined to base.pole, so it goes where that socket is. Slide the socket with set-socket, or make the part free first with unjoin"
    );
    let failure = refused(
        LAMP,
        &set_with(
            "pole",
            Changes {
                rotate: Some([0.0, 90.0, 0.0]),
                ..Changes::default()
            },
        ),
    );
    assert!(failure.report().errors[0].message.contains("with --twist"));
    // Scale is the part's own, joined or not.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &set_with(
            "pole",
            Changes {
                scale: Some([1.0, 0.5, 1.0]),
                ..Changes::default()
            },
        ),
    );
    assert!(
        applied.text.contains("twist = 15\nscale = [1, 0.5, 1]\n"),
        "{}",
        applied.text
    );
}

#[test]
fn unjoin_leaves_the_part_exactly_where_it_stood() {
    let recipe = robot();
    let before = done(&recipe, Syntax::Toml, &add_colour_x()).model.unwrap();
    // A plain part, a twisted one, a part made of parts, a mirrored one,
    // and a part inside a definition.
    for name in [
        "head",
        "panel",
        "antenna_tip",
        "left_arm",
        "right_arm",
        "left_leg/foot",
    ] {
        let applied = done(&recipe, Syntax::Toml, &Edit::Unjoin { part: name.into() });
        let after = applied.model.unwrap();
        assert!(
            same_shape(&before, &after),
            "{name} moved when it was unjoined"
        );
        let free = part(&after, name);
        assert_eq!(free.attached_to, None, "{name}");
        // Only that part's lines changed: its join became a place. (A
        // line that is in both lists only moved past the new ones.)
        let (gone, new) = changed(&recipe, &applied.text);
        for line in gone.iter().filter(|line| !new.contains(line)) {
            assert!(
                line.starts_with("attach = ") || line.starts_with("to = "),
                "{name}: lost {line}"
            );
        }
        let added: Vec<&&str> = new.iter().filter(|line| !gone.contains(line)).collect();
        assert!(
            added.iter().any(|line| line.starts_with("move = [")),
            "{name}: {added:?}"
        );
        for line in added {
            assert!(
                line.starts_with("move = [") || line.starts_with("rotate = ["),
                "{name}: gained {line}"
            );
        }
    }
    // What hung from the head still hangs from it.
    let applied = done(
        &recipe,
        Syntax::Toml,
        &Edit::Unjoin {
            part: "head".into(),
        },
    );
    assert!(applied.text.contains("to = \"head.antenna\""));
    assert_eq!(
        applied.summary,
        "unjoined \"head\" from body.neck: it is free, where it stood"
    );

    // A twisted part keeps its turn, as `rotate`.
    let applied = done(
        LAMP,
        Syntax::Toml,
        &Edit::Unjoin {
            part: "pole".into(),
        },
    );
    let (gone, new) = changed(LAMP, &applied.text);
    assert_eq!(
        gone,
        ["attach = \"bottom\"", "to = \"base.pole\"", "twist = 15"]
    );
    assert_eq!(new[0], "rotate = [0, 15, 0]");
    assert!(new[1].starts_with("move = [0, 0.525, 0]"), "{new:?}");

    // A part that is free already says so.
    let failure = refused(
        &recipe,
        &Edit::Unjoin {
            part: "body".into(),
        },
    );
    assert_eq!(
        failure.report().errors[0].message,
        "\"body\" isn't joined to anything: it is free already"
    );
}

fn add_colour_x() -> Edit {
    Edit::AddColor {
        name: "x".into(),
        hex: "#000000".into(),
    }
}

#[test]
fn a_part_inside_another_file_is_refused_with_where_to_go() {
    let recipe = "version = 0\n[[parts]]\nname = \"wheel\"\nfile = \"wheel.toml\"\n";
    let failure = refused(recipe, &set("wheel/tyre"));
    assert_eq!(
        failure.report().errors[0].message,
        "\"tyre\" is inside \"wheel.toml\", another file: open that file to edit it"
    );
}

// ---- what is refused ----------------------------------------------------------

#[test]
fn a_part_that_isnt_there_is_suggested() {
    let failure = refused(LAMP, &set("pol"));
    assert_eq!(failure.exit_code(), 2);
    let issue = &failure.report().errors[0];
    assert_eq!(issue.code, Code::UnknownPart);
    assert_eq!(issue.file, "test.toml");
    assert_eq!(issue.suggestion.as_deref(), Some("pole"));
    let failure = refused(LAMP, &set("left_bulb/glas"));
    assert_eq!(
        failure.report().errors[0].message,
        "there is no part \"glas\" in \"left_bulb\""
    );
}

#[test]
fn an_edit_that_would_break_the_recipe_is_refused_with_the_mistakes() {
    for edit in [
        // A colour that isn't in the palette.
        set_with(
            "pole",
            Changes {
                color: Some("purple".into()),
                ..Changes::default()
            },
        ),
        // A socket the part doesn't have.
        Edit::Join {
            part: "pole".into(),
            to: "base.lid".into(),
            attach: None,
        },
        // A size the shape doesn't take.
        set_with(
            "pole",
            Changes {
                sizes: Sizes {
                    radius: Some(1.0),
                    ..Sizes::default()
                },
                ..Changes::default()
            },
        ),
        // A part hung from itself, through another.
        Edit::Join {
            part: "base".into(),
            to: "pole.top".into(),
            attach: None,
        },
    ] {
        let failure = refused(LAMP, &edit);
        assert_eq!(failure.exit_code(), 1, "{edit:?}");
        assert!(!failure.report().errors.is_empty());
    }
    // But a recipe that already has a mistake can still be edited, and
    // the edit that mends it is the one most wanted.
    let broken = LAMP.replace("color  = \"steel\"", "color  = \"stel\"");
    let failure = refused(
        &broken,
        &set_with(
            "pole",
            Changes {
                color: Some("purple".into()),
                ..Changes::default()
            },
        ),
    );
    assert_eq!(failure.exit_code(), 1);
    let mended = done(
        &broken,
        Syntax::Toml,
        &set_with(
            "base",
            Changes {
                color: Some("steel".into()),
                ..Changes::default()
            },
        ),
    );
    assert!(mended.model.is_some());
    let still = done(&broken, Syntax::Toml, &add_hat());
    assert!(
        still.model.is_none(),
        "one mistake before, one after: allowed"
    );
}

#[test]
fn a_file_that_cant_be_read_isnt_edited() {
    let failure = refused("version = 0\n[[parts]\n", &add_hat());
    assert_eq!(failure.exit_code(), 1);
    assert_eq!(failure.report().errors[0].code, Code::Syntax);
}

// ---- JSON, line endings, the invisible mark -----------------------------------

#[test]
fn json_recipes_are_edited_the_same_way() {
    let json = to_json(LAMP);
    let applied = done(&json, Syntax::Json, &add_hat());
    let (old, new) = changed(&json, &applied.text);
    // The part before gets a comma; the new part follows, laid out like
    // its neighbours.
    assert_eq!(old, Vec::<&str>::new());
    assert_eq!(
        new,
        [
            "    },",
            "    {",
            "      \"name\": \"hat\",",
            "      \"shape\": \"cone\",",
            "      \"radius\": 0.2,",
            "      \"height\": 0.3,",
            "      \"color\": \"glow\",",
            "      \"attach\": \"bottom\",",
            "      \"to\": \"pole.top\""
        ]
    );
    assert_eq!(
        part(&applied.model.unwrap(), "hat").attached_to.as_deref(),
        Some("pole.top")
    );

    // The same edits give the same model in both languages.
    let edits = [
        add_hat(),
        set_with(
            "hat",
            Changes {
                twist: Some(40.0),
                color: Some("steel".into()),
                ..Changes::default()
            },
        ),
        Edit::Rename {
            part: "pole".into(),
            new_name: "stem".into(),
        },
        Edit::AddColor {
            name: "red".into(),
            hex: "#D0453A".into(),
        },
        Edit::AddSocket {
            part: "stem".into(),
            name: "badge".into(),
            on: "front".into(),
            shift: [0.0, 0.5, 0.0],
        },
        Edit::Duplicate {
            part: "left_bulb".into(),
            name: Some("third".into()),
            to: Some("stem.badge".into()),
            mirror: true,
        },
        Edit::SetSocket {
            socket: "stem.badge".into(),
            on: Some("back".into()),
            shift: Some([0.0, -0.5, 0.0]),
            facing: None,
        },
        Edit::Remove {
            part: "right_bulb".into(),
            with_attached: false,
        },
    ];
    let (mut toml, mut json) = (LAMP.to_owned(), json);
    for edit in &edits {
        let (a, b) = (
            done(&toml, Syntax::Toml, edit),
            done(&json, Syntax::Json, edit),
        );
        assert_eq!(a.summary, b.summary);
        let (a_model, b_model) = (a.model.unwrap(), b.model.unwrap());
        // (The JSON twin lists its palette in another order, so colours
        // are compared by name.)
        let painted = |model: &Model| -> Vec<(Vec<[f64; 3]>, Option<String>)> {
            model
                .faces
                .iter()
                .map(|face| {
                    (
                        face.corners
                            .iter()
                            .map(|corner| corner.to_array())
                            .collect(),
                        face.color.map(|at| model.palette[at].name.clone()),
                    )
                })
                .collect()
        };
        assert_eq!(painted(&a_model), painted(&b_model), "{edit:?}");
        assert_eq!(a_model.named_sockets(), b_model.named_sockets(), "{edit:?}");
        (toml, json) = (a.text, b.text);
    }
}

#[test]
fn windows_line_endings_and_the_invisible_mark_survive() {
    let windows = format!("\u{feff}{}", LAMP.replace('\n', "\r\n"));
    let mut text = windows.clone();
    for edit in [
        add_hat(),
        set_with(
            "hat",
            Changes {
                twist: Some(5.0),
                ..Changes::default()
            },
        ),
        Edit::AddSocket {
            part: "pole".into(),
            name: "badge".into(),
            on: "front".into(),
            shift: [0.0; 3],
        },
        Edit::Duplicate {
            part: "pole".into(),
            name: None,
            to: None,
            mirror: false,
        },
    ] {
        text = done(&text, Syntax::Toml, &edit).text;
        assert!(text.starts_with('\u{feff}'));
        assert!(
            !text.replace("\r\n", "").contains(['\r', '\n']),
            "every line still ends the Windows way after {edit:?}"
        );
    }
    // And every line of the first file is still there, in order.
    let mut new_lines = text.lines();
    for line in windows.lines() {
        assert!(
            new_lines.any(|new| new == line || new.replace("pole_2", "pole") == line),
            "lost: {line}"
        );
    }
}

// ---- every example, both languages --------------------------------------------

fn examples() -> Vec<(String, String)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    [
        "crate",
        "table",
        "tree",
        "house",
        "robot",
        "snowman",
        "desk",
        "duck",
        "dog",
        "doughnut",
        "mug",
        "temple",
        "barn",
        "slide",
        "gate",
        "chair",
        "window_wall",
        "rock",
    ]
    .into_iter()
    .map(|name| {
        let path = folder.join(format!("{name}.toml"));
        (name.to_owned(), std::fs::read_to_string(path).unwrap())
    })
    .collect()
}

/// The first part of the recipe that has a name.
fn first_named(text: &str, syntax: Syntax) -> String {
    let root = parse(text, syntax).unwrap();
    let Value::Table(entries) = &root.value else {
        panic!("a recipe is a table");
    };
    let parts = &entries
        .iter()
        .find(|entry| entry.key == "parts")
        .unwrap()
        .node;
    let Value::Array(parts) = &parts.value else {
        panic!("parts is a list");
    };
    parts
        .iter()
        .find_map(|part| match &part.value {
            Value::Table(fields) => fields
                .iter()
                .find(|f| f.key == "name")
                .and_then(|f| match &f.node.value {
                    Value::Str(name) => Some(name.clone()),
                    _ => None,
                }),
            _ => None,
        })
        .expect("an example has a named part")
}

#[test]
fn edits_leave_the_rest_of_every_example_untouched() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for (name, toml) in examples() {
        for (text, syntax) in [(toml.clone(), Syntax::Toml), (to_json(&toml), Syntax::Json)] {
            let file = folder.join(match syntax {
                Syntax::Toml => format!("{name}.toml"),
                Syntax::Json => format!("{name}.json"),
            });
            let run = |text: &str, edit: &Edit| -> Applied {
                apply(text, syntax, &file, &name, edit)
                    .unwrap_or_else(|failure| panic!("{name}: {edit:?}: {:#?}", failure.report()))
            };
            let target = first_named(&text, syntax);
            let lines_before: Vec<&str> = text.lines().collect();

            // Add a colour and a part painted with it: every line the
            // file had is still there, in order (in JSON, one line gains
            // a comma).
            let coloured = run(
                &text,
                &Edit::AddColor {
                    name: "edit_test".into(),
                    hex: "#123456".into(),
                },
            );
            let added = run(
                &coloured.text,
                &Edit::Add {
                    name: "edit_test_part".into(),
                    made: Made::Shape("box".into()),
                    to: Some(format!("{target}.top")),
                    attach: None,
                    sizes: Sizes {
                        size: Some([0.1, 0.1, 0.1]),
                        ..Sizes::default()
                    },
                    color: Some("edit_test".into()),
                    twist: Some(10.0),
                    free: Free::default(),
                },
            );
            let mut after = added.text.lines();
            for line in &lines_before {
                assert!(
                    after.any(|new| new == *line || new.strip_suffix(',') == Some(line)),
                    "{name} ({syntax:?}) lost the line: {line}"
                );
            }
            let model = added.model.expect("it builds");
            assert_eq!(
                part(&model, "edit_test_part").attached_to,
                Some(format!("{target}.top"))
            );

            // Rename a part and rename it back: the same bytes.
            let renamed = run(
                &text,
                &Edit::Rename {
                    part: target.clone(),
                    new_name: "edit_test_name".into(),
                },
            );
            assert_ne!(renamed.text, text);
            let back = run(
                &renamed.text,
                &Edit::Rename {
                    part: "edit_test_name".into(),
                    new_name: target.clone(),
                },
            );
            assert_eq!(back.text, text, "{name} ({syntax:?})");

            // Copy a part and remove the copy: the same bytes.
            let copied = run(
                &text,
                &Edit::Duplicate {
                    part: target.clone(),
                    name: Some("edit_test_copy".into()),
                    to: None,
                    mirror: false,
                },
            );
            let removed = run(
                &copied.text,
                &Edit::Remove {
                    part: "edit_test_copy".into(),
                    with_attached: false,
                },
            );
            assert_eq!(removed.text, text, "{name} ({syntax:?})");
        }
    }
}

// ---- files --------------------------------------------------------------------

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn edit_file_writes_the_recipe_and_reports_like_inspect() {
    let folder = folder("edit_file");
    let recipe = folder.join("lamp.toml");
    std::fs::write(&recipe, LAMP).unwrap();

    let dry = edit_file(&recipe, &add_hat(), true).unwrap();
    assert!(!dry.edit.written);
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), LAMP);
    assert_eq!(dry.before, LAMP);
    assert!(dry.after.contains("name = \"hat\""));

    let edited = edit_file(&recipe, &add_hat(), false).unwrap();
    assert!(edited.edit.written);
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), edited.after);
    let report = edited.to_json();
    assert_eq!(report["ok"], true);
    assert_eq!(report["edit"]["command"], "add");
    assert_eq!(
        report["edit"]["summary"],
        "added \"hat\" (cone) on pole.top"
    );
    assert_eq!(report["edit"]["lines"], serde_json::json!([57, 65]));
    assert_eq!(
        report["triangles"],
        edited.inspection.as_ref().unwrap().triangles
    );
    // Nothing else is left in the folder.
    let files: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, ["lamp.toml"]);

    // The same edit again is refused (the name is taken), and nothing
    // changes.
    let failure = edit_file(&recipe, &add_hat(), false).unwrap_err();
    assert_eq!(failure.report().errors[0].code, Code::DuplicateName);
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), edited.after);

    // An edit that changes nothing writes nothing.
    let same = edit_file(
        &recipe,
        &set_with(
            "pole",
            Changes {
                twist: Some(15.0),
                ..Changes::default()
            },
        ),
        false,
    )
    .unwrap();
    assert!(!same.edit.written);

    let missing = edit_file(&folder.join("gone.toml"), &add_hat(), false).unwrap_err();
    assert_eq!(missing.exit_code(), 2);
    let wrong = edit_file(&folder.join("lamp.glb"), &add_hat(), false).unwrap_err();
    assert!(wrong.report().errors[0].message.contains("isn't a recipe"));
}

#[test]
fn a_file_saved_by_someone_else_meanwhile_is_not_overwritten() {
    let folder = folder("edit_swap");
    let recipe = folder.join("lamp.toml");
    std::fs::write(&recipe, "the agent's newer text").unwrap();
    // The edit was worked out on LAMP, but the file now holds something
    // else: nothing is written, and the caller starts again.
    assert!(!swap_in(&recipe, Some(LAMP), "the edit's text").unwrap());
    assert_eq!(
        std::fs::read_to_string(&recipe).unwrap(),
        "the agent's newer text"
    );
    assert!(swap_in(&recipe, Some("the agent's newer text"), "the edit's text").unwrap());
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), "the edit's text");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
}

#[test]
fn every_kind_of_edit_has_a_name() {
    let all = [
        add_hat(),
        Edit::Remove {
            part: String::new(),
            with_attached: false,
        },
        Edit::Rename {
            part: String::new(),
            new_name: String::new(),
        },
        Edit::Join {
            part: String::new(),
            to: String::new(),
            attach: None,
        },
        set(""),
        Edit::Duplicate {
            part: String::new(),
            name: None,
            to: None,
            mirror: false,
        },
        Edit::AddColor {
            name: String::new(),
            hex: String::new(),
        },
        Edit::SetColor {
            name: String::new(),
            hex: String::new(),
        },
        Edit::RemoveColor {
            name: String::new(),
            replace_with: None,
        },
        Edit::AddSocket {
            part: String::new(),
            name: String::new(),
            on: String::new(),
            shift: [0.0; 3],
        },
        Edit::SetSocket {
            socket: String::new(),
            on: None,
            shift: None,
            facing: None,
        },
        Edit::RenameSocket {
            socket: String::new(),
            new_name: String::new(),
        },
        Edit::RemoveSocket {
            socket: String::new(),
        },
        Edit::TidySockets,
        Edit::Unjoin {
            part: String::new(),
        },
        Edit::Pull {
            part: String::new(),
            faces: Vec::new(),
            by: 0.0,
            name: None,
        },
        Edit::Inset {
            part: String::new(),
            faces: Vec::new(),
            by: 0.0,
            name: None,
        },
        Edit::SetChange {
            part: String::new(),
            number: 1,
            by: None,
            by_xyz: None,
            at: None,
        },
        Edit::RemoveChange {
            part: String::new(),
            number: None,
        },
        Edit::MoveFace {
            part: String::new(),
            faces: Vec::new(),
            by: [0.0; 3],
        },
        Edit::TurnFace {
            part: String::new(),
            faces: Vec::new(),
            by: [0.0; 3],
        },
        Edit::ScaleFace {
            part: String::new(),
            faces: Vec::new(),
            by: [1.0; 3],
        },
        Edit::MoveEdge {
            part: String::new(),
            edges: Vec::new(),
            by: [0.0; 3],
        },
        Edit::Cut {
            part: String::new(),
            toward: String::new(),
            at: 0.0,
            name: None,
        },
        Edit::MoveCorner {
            part: String::new(),
            corners: Vec::new(),
            by: [0.0; 3],
        },
        Edit::Define {
            part: String::new(),
            name: String::new(),
        },
        Edit::SetLimits {
            part: String::new(),
            limits: duck_mold_core::edit::LimitsTo::Fixed,
        },
    ];
    let names: Vec<&str> = all.iter().map(Edit::command).collect();
    assert_eq!(names, Edit::COMMANDS);
}
