//! Step 2: recipes are read from TOML and JSON, and every mistake names
//! the file, the field and the line, and suggests a fix.

use std::path::{Path, PathBuf};

use duck_mold_core::issue::{Allowed, Code, Issue, Report};
use duck_mold_core::load::{Loaded, load, load_text};
use duck_mold_core::recipe::{Placement, Shape, SocketKind, Source};
use duck_mold_core::source::Syntax;

/// Reads a TOML recipe that names no other files.
fn read(text: &str) -> (Loaded, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    (loaded, issues)
}

fn issues_of(text: &str) -> Vec<Issue> {
    read(text).1
}

/// The one issue with this code. Fails the test, showing them all, if
/// there isn't exactly one.
fn only(issues: &[Issue], code: Code) -> &Issue {
    let found: Vec<&Issue> = issues.iter().filter(|issue| issue.code == code).collect();
    assert_eq!(found.len(), 1, "expected one {code} in {issues:#?}");
    found[0]
}

/// A recipe that is fine apart from what is added to its first part.
fn with_part(extra: &str) -> String {
    format!("version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"a\"\n{extra}\n")
}

/// A folder for a test's files, emptied first.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

const CRATE: &str = r##"
version = 0
name = "crate"
description = "A wooden crate."

[palette]
wood = "#B07A3F"
plank = "#8a5a2b"

[[parts]]
shape = "box"
size = [1, 1.5, 1]
color = "wood"
colors = { top = "plank" }
"##;

#[test]
fn a_good_recipe_has_no_issues() {
    let (loaded, issues) = read(CRATE);
    assert_eq!(issues, Vec::new());
    let recipe = &loaded.recipe;
    assert_eq!(recipe.name, "crate");
    assert_eq!(recipe.description.as_deref(), Some("A wooden crate."));
    assert_eq!(loaded.palette.len(), 2);
    assert_eq!(loaded.palette[0].name, "wood");
    assert_eq!(loaded.palette[0].rgb, [0xB0, 0x7A, 0x3F]);
    // Small letters in a colour are fine.
    assert_eq!(loaded.palette[1].rgb, [0x8A, 0x5A, 0x2B]);
    let part = &recipe.parts[0];
    assert_eq!(
        part.source,
        Source::Shape(Shape::Box {
            size: [1.0, 1.5, 1.0]
        })
    );
    assert_eq!(part.color.as_ref().unwrap().text, "wood");
    assert_eq!(part.colors[0].face, "top");
    assert_eq!(part.colors[0].color, "plank");
}

#[test]
fn the_smallest_recipe_takes_every_default() {
    let (loaded, issues) = read("version = 0\n[[parts]]\nshape = \"cylinder\"\n");
    assert_eq!(issues, Vec::new());
    // Named after its file.
    assert_eq!(loaded.recipe.name, "test");
    assert_eq!(
        loaded.recipe.parts[0].source,
        Source::Shape(Shape::Cylinder {
            radius: 0.5,
            height: 1.0,
            segments: 16
        })
    );
    assert_eq!(loaded.recipe.parts[0].scale, [1.0; 3]);
}

#[test]
fn json_reads_the_same_fields() {
    let json = r##"{
  "version": 0,
  "name": "crate",
  "note": ["anything", 1],
  "palette": { "wood": "#B07A3F" },
  "parts": [
    { "name": "lid", "shape": "box", "size": [1, 0.1, 1], "color": "wood", "move": [0, 0.5, 0], "note": "the lid" }
  ]
}"##;
    let mut issues = Vec::new();
    let loaded = load_text(
        json,
        Syntax::Json,
        "crate.json",
        Path::new("."),
        &mut issues,
    );
    assert_eq!(issues, Vec::new());
    let part = &loaded.recipe.parts[0];
    assert_eq!(part.name.as_deref(), Some("lid"));
    assert_eq!(part.loc.file, "crate.json");
    assert_eq!(part.loc.pos.line, 7);
    assert_eq!(
        part.placement,
        Placement::Free {
            move_by: [0.0, 0.5, 0.0],
            rotate: [0.0; 3]
        }
    );
}

#[test]
fn a_syntax_mistake_names_the_file_and_line() {
    let issues = issues_of("version = 0\n[[parts]]\nshape = \"box\nsize = 1\n");
    let issue = only(&issues, Code::Syntax);
    assert_eq!(issue.file, "test.toml");
    assert_eq!(issue.line, Some(3));
    // Nothing else is reported about a file that couldn't be read.
    assert_eq!(issues.len(), 1);

    let mut issues = Vec::new();
    load_text(
        "{\n  \"version\": 0,\n}",
        Syntax::Json,
        "bad.json",
        Path::new("."),
        &mut issues,
    );
    let issue = only(&issues, Code::Syntax);
    assert_eq!((issue.file.as_str(), issue.line), ("bad.json", Some(3)));
    assert!(issue.message.contains("remove it"), "{}", issue.message);
}

#[test]
fn the_version_is_checked() {
    let missing = issues_of("[[parts]]\nshape = \"box\"\n");
    assert_eq!(only(&missing, Code::MissingField).path, "version");
    assert!(
        only(&missing, Code::MissingField)
            .message
            .contains("version = 0")
    );

    let newer = issues_of("version = 3\n[[parts]]\nshape = \"box\"\n");
    let issue = only(&newer, Code::UnsupportedVersion);
    assert_eq!((issue.path.as_str(), issue.line), ("version", Some(1)));

    let text = issues_of("version = \"0\"\n[[parts]]\nshape = \"box\"\n");
    assert_eq!(only(&text, Code::TypeMismatch).path, "version");
}

#[test]
fn a_misspelt_field_is_an_error_with_the_likely_fix() {
    let issues = issues_of(&with_part("shape = \"box\"\natach = \"top\""));
    let issue = only(&issues, Code::UnknownField);
    assert_eq!(issue.file, "test.toml");
    assert_eq!(issue.line, Some(7));
    assert_eq!(issue.path, "parts[0].atach");
    assert_eq!(issue.suggestion.as_deref(), Some("attach"));
    assert!(
        matches!(&issue.allowed, Some(Allowed::OneOf { options }) if options.contains(&"attach".to_owned()))
    );
    assert_eq!(
        issue.to_string(),
        "error: test.toml:7: parts[0].atach: there is no field \"atach\" here. Did you mean \"attach\"?"
    );

    // British spelling is the commonest slip of all.
    let issues = issues_of(&with_part("shape = \"box\"\ncolour = \"wood\""));
    assert_eq!(
        only(&issues, Code::UnknownField).suggestion.as_deref(),
        Some("color")
    );

    // At the top of the file too.
    let issues = issues_of("version = 0\n[[part]]\nshape = \"box\"\n");
    assert_eq!(
        only(&issues, Code::UnknownField).suggestion.as_deref(),
        Some("parts")
    );
}

#[test]
fn reserved_names_say_not_yet() {
    for (field, purpose) in [
        ("bones", "rigs"),
        ("colliders", "collision shapes"),
        ("lods", "simpler versions"),
    ] {
        let issues = issues_of(&with_part(&format!("shape = \"box\"\n{field} = []")));
        let issue = only(&issues, Code::Reserved);
        assert_eq!(issue.path, format!("parts[0].{field}"));
        assert!(issue.message.contains(purpose), "{}", issue.message);
        assert!(issue.message.contains("isn't available yet"));
    }
    // And at the top of the file.
    let issues = issues_of("version = 0\nbones = 1\n[[parts]]\nshape = \"box\"\n");
    assert_eq!(only(&issues, Code::Reserved).line, Some(2));
}

#[test]
fn a_part_needs_exactly_one_source_of_shape() {
    let none = issues_of(&with_part("color = \"wood\""));
    let issue = only(&none, Code::MissingField);
    assert_eq!((issue.path.as_str(), issue.line), ("parts[0]", Some(4)));
    assert!(issue.message.contains("shape = \"box\""));

    let two = issues_of(&with_part("shape = \"box\"\nuse = \"leg\""));
    let issue = only(&two, Code::Conflict);
    assert_eq!(issue.path, "parts[0].use");
    assert!(
        issue.message.contains("remove \"use\""),
        "{}",
        issue.message
    );

    let missing_parts = issues_of("version = 0\n");
    assert_eq!(only(&missing_parts, Code::MissingField).path, "parts");
    let empty = issues_of("version = 0\nparts = []\n");
    assert_eq!(only(&empty, Code::MissingField).path, "parts");
}

#[test]
fn an_unknown_shape_lists_the_real_ones() {
    let issues = issues_of(&with_part("shape = \"cilinder\""));
    let issue = only(&issues, Code::UnknownShape);
    assert_eq!(issue.suggestion.as_deref(), Some("cylinder"));
    assert_eq!(
        issue.allowed,
        Some(Allowed::OneOf {
            options: ["box", "cylinder", "sphere", "cone", "wedge"]
                .map(String::from)
                .to_vec()
        })
    );
    // Nothing more is said about the sizes of a shape that doesn't exist.
    let issues = issues_of(&with_part("shape = \"blob\"\nradius = 2"));
    assert_eq!(issues.len(), 1);
}

#[test]
fn size_fields_belong_to_their_shape() {
    let issues = issues_of(&with_part("shape = \"box\"\nradius = 0.3"));
    let issue = only(&issues, Code::UnknownField);
    assert_eq!(issue.path, "parts[0].radius");
    assert_eq!(
        issue.message,
        "a box has no \"radius\": its size is given by \"size\""
    );

    let issues = issues_of(&with_part("shape = \"sphere\"\nheight = 2"));
    assert!(
        only(&issues, Code::UnknownField)
            .message
            .contains("\"radius\" and \"segments\"")
    );

    let issues = issues_of(&format!(
        "{}\n[define.leg]\nshape = \"box\"\n",
        with_part("use = \"leg\"\nsize = [1, 2, 1]")
    ));
    let issue = only(&issues, Code::Conflict);
    assert!(issue.message.contains("use \"scale\""), "{}", issue.message);
}

#[test]
fn numbers_are_checked_for_kind_and_range() {
    let issues = issues_of(&with_part("shape = \"cylinder\"\nsegments = 200"));
    let issue = only(&issues, Code::OutOfRange);
    assert_eq!(issue.path, "parts[0].segments");
    assert_eq!(
        issue.allowed,
        Some(Allowed::Range {
            min: 3.0,
            max: 64.0
        })
    );
    assert_eq!(
        issue.to_string(),
        "error: test.toml:7: parts[0].segments: 200 is too big. Allowed: 3 to 64."
    );

    // A sphere needs at least 4.
    let issues = issues_of(&with_part("shape = \"sphere\"\nsegments = 3"));
    assert_eq!(
        only(&issues, Code::OutOfRange).allowed,
        Some(Allowed::Range {
            min: 4.0,
            max: 64.0
        })
    );

    let issues = issues_of(&with_part("shape = \"cone\"\nsegments = 8.5"));
    assert!(
        only(&issues, Code::TypeMismatch)
            .message
            .contains("whole number")
    );

    let issues = issues_of(&with_part("shape = \"box\"\nsize = \"big\""));
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!(
        issue.message,
        "this needs three numbers, like [1, 2, 0.5], not the text \"big\""
    );

    let issues = issues_of(&with_part("shape = \"box\"\nsize = [1, 2]"));
    assert!(
        only(&issues, Code::TypeMismatch)
            .message
            .contains("not a list of 2")
    );

    let issues = issues_of(&with_part("shape = \"box\"\nsize = [1, 0, 1]"));
    let issue = only(&issues, Code::OutOfRange);
    assert_eq!(issue.path, "parts[0].size[1]");
    assert!(issue.message.contains("too small"));

    let issues = issues_of(&with_part("shape = \"cylinder\"\nradius = \"wide\""));
    assert_eq!(only(&issues, Code::TypeMismatch).path, "parts[0].radius");
}

#[test]
fn attach_and_to_go_together() {
    let issues = issues_of(&with_part("shape = \"box\"\nattach = \"bottom\""));
    let issue = only(&issues, Code::MissingField);
    assert_eq!(issue.path, "parts[0].to");
    assert!(
        issue.message.contains("like to = \"body.top\""),
        "{}",
        issue.message
    );

    let issues = issues_of(&with_part("shape = \"box\"\nto = \"trunk.top\""));
    let issue = only(&issues, Code::MissingField);
    assert_eq!(issue.path, "parts[0].attach");
    assert!(
        issue.message.contains("like attach = \"bottom\""),
        "{}",
        issue.message
    );

    let issues = issues_of(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"trunk\"",
    ));
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!(issue.path, "parts[0].to");
    assert!(issue.message.contains("like \"body.neck\""));
}

#[test]
fn an_attached_part_is_not_moved_by_hand() {
    let issues = issues_of(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\nmove = [0, 1, 0]",
    ));
    let issue = only(&issues, Code::Conflict);
    assert_eq!(issue.path, "parts[0].move");
    assert!(issue.message.contains("remove \"move\""));
}

#[test]
fn a_joined_part_twists_and_a_free_part_rotates() {
    let (loaded, issues) = read(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\ntwist = 90",
    ));
    assert_eq!(issues, Vec::new());
    assert!(matches!(
        &loaded.recipe.parts[0].placement,
        Placement::Attached { twist, to_part, to_socket, .. }
            if *twist == 90.0 && to_part == "b" && to_socket == "top"
    ));

    let (loaded, issues) = read(&with_part("shape = \"box\"\nrotate = [0, 90, 0]"));
    assert_eq!(issues, Vec::new());
    assert_eq!(
        loaded.recipe.parts[0].placement,
        Placement::Free {
            move_by: [0.0; 3],
            rotate: [0.0, 90.0, 0.0]
        }
    );

    // `twist` is one number.
    let issues = issues_of(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\ntwist = [0, 90, 0]",
    ));
    assert!(
        only(&issues, Code::TypeMismatch)
            .message
            .contains("like 90")
    );

    let issues = issues_of(&with_part("shape = \"box\"\nrotate = 45"));
    let issue = only(&issues, Code::TypeMismatch);
    assert!(
        issue.message.contains("rotate = [0, 45, 0]"),
        "{}",
        issue.message
    );
}

#[test]
fn rotate_on_a_joined_part_is_an_error_that_shows_twist() {
    // Phase 0's one-number form.
    let issues = issues_of(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\nrotate = 90",
    ));
    let issue = only(&issues, Code::Conflict);
    assert_eq!(issue.path, "parts[0].rotate");
    assert_eq!(
        issue.message,
        "a joined part turns around its join with \"twist\": write twist = 90"
    );
    assert_eq!(issue.suggestion.as_deref(), Some("twist = 90"));
    assert_eq!(issue.line, Some(9));

    // Three numbers on a joined part never worked; now it points to twist
    // too.
    let issues = issues_of(&with_part(
        "shape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\nrotate = [0, 45, 0]",
    ));
    let issue = only(&issues, Code::Conflict);
    assert!(
        issue.message.contains("write twist = 90"),
        "{}",
        issue.message
    );
}

#[test]
fn twist_on_a_part_that_isnt_joined_is_an_error_that_shows_rotate() {
    let issues = issues_of(&with_part("shape = \"box\"\ntwist = 30"));
    let issue = only(&issues, Code::Conflict);
    assert_eq!(issue.path, "parts[0].twist");
    assert_eq!(
        issue.message,
        "this part isn't joined to anything, so there is nothing to twist it around: to turn it around the upright axis, write rotate = [0, 30, 0]"
    );
    assert_eq!(issue.suggestion.as_deref(), Some("rotate = [0, 30, 0]"));

    // A defined part has no place of its own, so no twist either.
    let issues = issues_of(
        "version = 0\n[define.arm]\nshape = \"box\"\ntwist = 30\n[[parts]]\nuse = \"arm\"\n",
    );
    assert_eq!(only(&issues, Code::Conflict).path, "define.arm.twist");
}

#[test]
fn scale_and_mirror_are_checked() {
    let (loaded, issues) = read(&with_part("shape = \"box\"\nscale = 2\nmirror = \"x\""));
    assert_eq!(issues, Vec::new());
    assert_eq!(loaded.recipe.parts[0].scale, [2.0; 3]);
    assert!(loaded.recipe.parts[0].mirror.is_some());

    let (loaded, issues) = read(&with_part("shape = \"box\"\nscale = [1, 2, 0.5]"));
    assert_eq!(issues, Vec::new());
    assert_eq!(loaded.recipe.parts[0].scale, [1.0, 2.0, 0.5]);

    let issues = issues_of(&with_part("shape = \"box\"\nscale = [-1, 1, 1]"));
    let issue = only(&issues, Code::OutOfRange);
    assert!(
        issue.message.contains("use mirror = \"x\""),
        "{}",
        issue.message
    );

    let issues = issues_of(&with_part("shape = \"box\"\nmirror = \"X\""));
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!(issue.suggestion.as_deref(), Some("x"));
}

#[test]
fn names_are_checked() {
    let issues = issues_of("version = 0\n[[parts]]\nname = \"left arm\"\nshape = \"box\"\n");
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!(
        (issue.path.as_str(), issue.line),
        ("parts[0].name", Some(3))
    );
    assert!(issue.message.contains("letters, digits"));

    let issues = issues_of(
        "version = 0\n[[parts]]\nname = \"leg\"\nshape = \"box\"\n[[parts]]\nname = \"leg\"\nshape = \"box\"\n",
    );
    let issue = only(&issues, Code::DuplicateName);
    assert_eq!(issue.line, Some(5));
    assert!(issue.message.contains("parts[0] is the other"));
}

#[test]
fn a_defined_part_has_no_place_of_its_own() {
    let text = "version = 0\n[define.leg]\nshape = \"box\"\nmove = [0, 1, 0]\nscale = 2\nname = \"leg\"\n[[parts]]\nuse = \"leg\"\n";
    let issues = issues_of(text);
    let conflicts: Vec<&str> = issues
        .iter()
        .filter(|issue| issue.code == Code::Conflict)
        .map(|issue| issue.path.as_str())
        .collect();
    assert_eq!(
        conflicts,
        ["define.leg.name", "define.leg.move", "define.leg.scale"]
    );
    assert!(issues.iter().all(|issue| issue.code == Code::Conflict));
}

#[test]
fn sockets_are_read_three_ways() {
    let (loaded, issues) = read(&with_part(
        "shape = \"box\"\n[parts.sockets]\nneck = { on = \"top\" }\nleg = { on = \"bottom\", shift = [-0.5, 0, 0.25] }\nhook = { at = [0.2, 0.9, 0], facing = \"front\" }\nspike = { at = [0, 0, 0], facing = [1, 1, 0] }\ngrip = { from = \"hand.bottom\" }",
    ));
    assert_eq!(issues, Vec::new());
    let sockets = &loaded.recipe.parts[0].sockets;
    let names: Vec<&str> = sockets.iter().map(|socket| socket.name.as_str()).collect();
    assert_eq!(names, ["neck", "leg", "hook", "spike", "grip"]);
    assert!(
        matches!(&sockets[1].kind, SocketKind::On { shift, facing: None, .. } if *shift == [-0.5, 0.0, 0.25])
    );
    assert!(
        matches!(&sockets[2].kind, SocketKind::At { at, facing } if *at == [0.2, 0.9, 0.0] && *facing == [0.0, 0.0, 1.0])
    );
    assert!(
        matches!(&sockets[4].kind, SocketKind::From { part, socket } if part == "hand" && socket == "bottom")
    );
}

#[test]
fn socket_mistakes_are_explained() {
    let socket = |text: &str| {
        issues_of(&with_part(&format!(
            "shape = \"box\"\n[parts.sockets]\n{text}"
        )))
    };

    let issues = socket("top = { on = \"top\" }");
    let issue = only(&issues, Code::DuplicateName);
    assert_eq!(issue.path, "parts[0].sockets.top");
    assert!(
        issue
            .message
            .contains("already has a socket called \"top\"")
    );

    let issues = socket("neck = { on = \"centre\" }");
    assert_eq!(
        only(&issues, Code::TypeMismatch).suggestion.as_deref(),
        Some("center")
    );

    let issues = socket("neck = { on = \"top\", shift = [0, 0.5, 0] }");
    let issue = only(&issues, Code::Conflict);
    assert_eq!(issue.path, "parts[0].sockets.neck.shift");
    assert!(
        issue.message.contains("its y number must stay 0"),
        "{}",
        issue.message
    );

    let issues = socket("neck = { on = \"top\", shift = [2, 0, 0] }");
    assert_eq!(
        only(&issues, Code::OutOfRange).allowed,
        Some(Allowed::Range {
            min: -1.0,
            max: 1.0
        })
    );

    let issues = socket("neck = { on = \"top\", at = [0, 1, 0] }");
    assert!(
        only(&issues, Code::Conflict)
            .message
            .contains("remove \"at\"")
    );

    let issues = socket("neck = { at = [0, 1, 0], shift = [0.5, 0, 0] }");
    assert_eq!(
        only(&issues, Code::Conflict).path,
        "parts[0].sockets.neck.shift"
    );

    let issues = socket("neck = { from = \"hand.grip\", facing = \"up\" }");
    assert!(
        only(&issues, Code::Conflict)
            .message
            .contains("remove \"facing\"")
    );

    let issues = socket("neck = { }");
    assert!(
        only(&issues, Code::MissingField)
            .message
            .contains("{ on = \"top\" }")
    );

    let issues = socket("neck = { on = \"top\", facing = \"north\" }");
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!(issue.path, "parts[0].sockets.neck.facing");
    assert!(matches!(&issue.allowed, Some(Allowed::OneOf { options }) if options.len() == 6));

    let issues = socket("neck = { at = [0, 1, 0], facing = [0, 0, 0] }");
    assert!(
        only(&issues, Code::OutOfRange)
            .message
            .contains("point somewhere")
    );

    let issues = socket("neck = { on = \"top\", sift = [0, 0, 0] }");
    assert_eq!(
        only(&issues, Code::UnknownField).suggestion.as_deref(),
        Some("shift")
    );

    // The centre can be shifted in every direction.
    let issues = socket("core = { on = \"center\", shift = [0.5, 0.5, 0.5] }");
    assert_eq!(issues, Vec::new());
}

#[test]
fn palette_mistakes_are_explained() {
    let palette = |entry: &str| {
        issues_of(&format!(
            "version = 0\n[palette]\n{entry}\n[[parts]]\nshape = \"box\"\n"
        ))
    };

    let issues = palette("wood = \"brown\"");
    let issue = only(&issues, Code::TypeMismatch);
    assert_eq!((issue.path.as_str(), issue.line), ("palette.wood", Some(3)));
    assert!(issue.message.contains("\"#RRGGBB\""));

    let issues = palette("glass = \"#88CCFF80\"");
    assert!(
        only(&issues, Code::Unsupported)
            .message
            .contains("see-through")
    );

    let issues = palette("\"dark wood\" = \"#553311\"");
    assert!(
        only(&issues, Code::TypeMismatch)
            .message
            .contains("can't be a colour's name")
    );

    let (loaded, issues) =
        read("version = 0\n[palette]\nred = \"#F00\"\n[[parts]]\nshape = \"box\"\n");
    assert_eq!(issues, Vec::new());
    assert_eq!(loaded.palette[0].rgb, [255, 0, 0]);

    let many: String = (0..257).map(|n| format!("c{n} = \"#000000\"\n")).collect();
    let issues = palette(&many);
    assert!(
        only(&issues, Code::OutOfRange)
            .message
            .contains("at most 256")
    );
}

#[test]
fn origin_names_a_socket() {
    let (loaded, issues) = read(
        "version = 0\norigin = \"trunk.bottom\"\n[[parts]]\nname = \"trunk\"\nshape = \"box\"\n",
    );
    assert_eq!(issues, Vec::new());
    assert!(loaded.recipe.origin.is_some());

    let issues = issues_of("version = 0\norigin = \"a.b.c\"\n[[parts]]\nshape = \"box\"\n");
    assert_eq!(only(&issues, Code::TypeMismatch).path, "origin");
}

#[test]
fn every_mistake_is_reported_at_once() {
    let text = r##"version = 0

[palette]
wood = "brown"

[[parts]]
name = "body"
shape = "boxx"

[[parts]]
name = "lid"
shape = "box"
size = [1, 1]
atach = "bottom"

[[parts]]
shape = "cone"
segments = 2
"##;
    let report = Report::from_issues(issues_of(text));
    let lines: Vec<(usize, Code)> = report
        .errors
        .iter()
        .map(|issue| (issue.line.unwrap(), issue.code))
        .collect();
    assert_eq!(
        lines,
        [
            (4, Code::TypeMismatch),
            (8, Code::UnknownShape),
            (13, Code::TypeMismatch),
            (14, Code::UnknownField),
            (18, Code::OutOfRange),
        ]
    );
    assert!(report.errors.iter().all(|issue| issue.file == "test.toml"));
    assert_eq!(report.closing_line(), "5 errors. Nothing was built.");
}

#[test]
fn nested_parts_have_nested_paths() {
    let text = "version = 0\n[define.arm]\n[[define.arm.parts]]\nshape = \"box\"\n[[define.arm.parts]]\nshape = \"ball\"\n[[parts]]\nuse = \"arm\"\n";
    let issues = issues_of(text);
    let issue = only(&issues, Code::UnknownShape);
    assert_eq!(
        (issue.path.as_str(), issue.line),
        ("define.arm.parts[1].shape", Some(6))
    );
}

// ---- recipes that name other files -------------------------------------------

#[test]
fn a_palette_can_live_in_a_shared_file() {
    let folder = folder("shared_palette");
    std::fs::write(
        folder.join("palette.toml"),
        "steel = \"#8A939B\"\nred = \"#D0453A\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("robot.toml"),
        "version = 0\npalette = \"palette.toml\"\n[[parts]]\nshape = \"box\"\ncolor = \"steel\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("robot.toml"), &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    let names: Vec<&str> = loaded
        .palette
        .iter()
        .map(|color| color.name.as_str())
        .collect();
    assert_eq!(names, ["steel", "red"]);
    // Colours from a shared file never count as "never used".
    assert!(loaded.palette.iter().all(|color| !color.own));
    assert!(
        loaded.palette[0]
            .loc
            .file
            .ends_with("shared_palette/palette.toml")
    );
}

#[test]
fn a_missing_file_is_reported_where_the_recipe_names_it() {
    let folder = folder("missing_file");
    std::fs::write(
        folder.join("robot.toml"),
        "version = 0\npalette = \"nowhere.toml\"\n[[parts]]\nfile = \"wheel.toml\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    load(&folder.join("robot.toml"), &mut issues).unwrap();
    let missing: Vec<(&str, Option<usize>)> = issues
        .iter()
        .filter(|issue| issue.code == Code::FileNotFound)
        .map(|issue| (issue.path.as_str(), issue.line))
        .collect();
    assert_eq!(missing, [("palette", Some(2)), ("parts[0].file", Some(4))]);
    assert!(issues[0].message.contains("there is no such file"));
    assert!(issues[0].message.contains("recipe's own folder"));
}

#[test]
fn a_recipe_can_use_another_recipe_as_a_part() {
    let folder = folder("recipe_as_part");
    std::fs::create_dir_all(folder.join("parts")).unwrap();
    std::fs::write(
        folder.join("parts/wheel.toml"),
        "version = 0\n[palette]\nrubber = \"#222222\"\n[[parts]]\nshape = \"cylinder\"\ncolor = \"rubber\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("cart.toml"),
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"wheel\"\nfile = \"parts/wheel.toml\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("cart.toml"), &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    assert!(
        matches!(&loaded.recipe.parts[0].source, Source::Recipe(inner) if inner.name == "wheel")
    );
    // The wheel's colour joins the model's palette, after the cart's own.
    let names: Vec<(&str, bool)> = loaded
        .palette
        .iter()
        .map(|c| (c.name.as_str(), c.own))
        .collect();
    assert_eq!(names, [("wood", true), ("rubber", false)]);
}

#[test]
fn mistakes_in_a_part_file_name_that_file() {
    let folder = folder("mistake_in_part_file");
    std::fs::write(
        folder.join("wheel.toml"),
        "version = 0\n[[parts]]\nshape = \"weel\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("cart.toml"),
        "version = 0\n[[parts]]\nfile = \"wheel.toml\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    load(&folder.join("cart.toml"), &mut issues).unwrap();
    let issue = only(&issues, Code::UnknownShape);
    assert!(
        issue.file.ends_with("mistake_in_part_file/wheel.toml"),
        "{}",
        issue.file
    );
    assert_eq!(issue.line, Some(3));
}

#[test]
fn one_colour_name_cannot_mean_two_colours() {
    let folder = folder("palette_clash");
    std::fs::write(
        folder.join("wheel.toml"),
        "version = 0\n[palette]\nsteel = \"#777777\"\n[[parts]]\nshape = \"cylinder\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("cart.toml"),
        "version = 0\n[palette]\nsteel = \"#8A939B\"\n[[parts]]\nfile = \"wheel.toml\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    load(&folder.join("cart.toml"), &mut issues).unwrap();
    let issue = only(&issues, Code::Conflict);
    assert!(issue.file.ends_with("wheel.toml"));
    assert_eq!(issue.path, "palette.steel");
    assert!(
        issue.message.contains("#777777 here but #8A939B in"),
        "{}",
        issue.message
    );
}

#[test]
fn a_recipe_that_uses_itself_is_caught() {
    let folder = folder("file_cycle");
    std::fs::write(
        folder.join("a.toml"),
        "version = 0\n[[parts]]\nfile = \"b.toml\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("b.toml"),
        "version = 0\n[[parts]]\nfile = \"a.toml\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    load(&folder.join("a.toml"), &mut issues).unwrap();
    let issue = only(&issues, Code::Cycle);
    assert!(issue.file.ends_with("b.toml"));
    assert!(issue.message.contains("a.toml > "), "{}", issue.message);
    assert!(issue.message.ends_with("a.toml)"), "{}", issue.message);
}

#[test]
fn files_that_are_not_recipes_or_meshes_are_refused() {
    // glTF is read since Phase 1: a .glb that isn't there is a missing
    // file, not a kind Duck Mold refuses.
    let issues = issues_of("version = 0\n[[parts]]\nfile = \"duck.glb\"\n");
    assert!(
        only(&issues, Code::FileNotFound)
            .message
            .contains("can't read \"duck.glb\"")
    );
    let issues = issues_of("version = 0\n[[parts]]\nfile = \"duck.png\"\n");
    let issue = only(&issues, Code::Unsupported);
    assert!(issue.message.contains(".toml or .json"));
    assert!(issue.message.contains(".obj, .gltf or .glb"));
}

#[test]
fn the_file_on_the_command_line_must_be_a_readable_recipe() {
    let mut issues = Vec::new();
    let error = load(Path::new("no/such/recipe.toml"), &mut issues).unwrap_err();
    assert_eq!(error.code, Code::Io);
    assert!(error.message.contains("there is no such file"));
    let error = load(Path::new("robot.txt"), &mut issues).unwrap_err();
    assert!(error.message.contains("ends in .toml or .json"));
}
