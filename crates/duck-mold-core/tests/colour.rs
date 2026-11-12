//! Step 5: every face gets one colour from the palette, by part or by
//! face.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::{Allowed, Code, Issue, Report};
use duck_mold_core::load::{load, load_text};
use duck_mold_core::model::Model;
use duck_mold_core::source::Syntax;

fn build(text: &str) -> (Option<Model>, Report) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, Report::from_issues(issues))
}

/// Builds a recipe that must have no errors and no warnings.
fn built(text: &str) -> Model {
    let (model, report) = build(text);
    assert_eq!(report.errors, Vec::new());
    assert_eq!(report.warnings, Vec::new());
    model.unwrap()
}

fn errors(text: &str) -> Vec<Issue> {
    let (model, report) = build(text);
    assert!(model.is_none());
    report.errors
}

fn warnings(text: &str) -> Vec<Issue> {
    let (model, report) = build(text);
    assert!(model.is_some(), "{:#?}", report.errors);
    report.warnings
}

fn only(issues: &[Issue], code: Code) -> &Issue {
    let found: Vec<&Issue> = issues.iter().filter(|issue| issue.code == code).collect();
    assert_eq!(found.len(), 1, "expected one {code} in {issues:#?}");
    found[0]
}

/// How many triangles each palette colour paints, by name, in palette
/// order; unpainted triangles are counted under "(none)".
fn triangles_by_colour(model: &Model) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = model
        .palette
        .iter()
        .map(|color| (color.name.clone(), 0))
        .collect();
    counts.push(("(none)".to_owned(), 0));
    for face in &model.faces {
        let at = face.color.unwrap_or(model.palette.len());
        counts[at].1 += face.triangle_count();
    }
    counts.retain(|(_, count)| *count > 0);
    counts
}

fn counts(text: &str) -> Vec<(String, usize)> {
    triangles_by_colour(&built(text))
}

fn named(pairs: &[(&str, usize)]) -> Vec<(String, usize)> {
    pairs
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count))
        .collect()
}

/// The colour name of the face of part `path` that points along `facing`.
fn colour_facing(model: &Model, facing: [f64; 3]) -> &str {
    let face = model
        .faces
        .iter()
        .find(|face| {
            face.normal()
                .is_some_and(|normal| normal.to_array() == facing)
        })
        .expect("a face points that way");
    &model.palette[face.color.expect("the face is coloured")].name
}

const PALETTE: &str =
    "version = 0\n[palette]\nwood = \"#B07A3F\"\nplank = \"#8A5A2B\"\nred = \"#D0453A\"\n";

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn color_paints_the_whole_part() {
    let model = built(
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n",
    );
    assert_eq!(triangles_by_colour(&model), named(&[("wood", 12)]));
    // A face's colour is a place in the model's palette.
    assert_eq!(
        model.palette[model.faces[0].color.unwrap()].rgb,
        [0xB0, 0x7A, 0x3F]
    );
}

#[test]
fn colors_paints_single_faces_and_wins_over_color() {
    let model = built(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"wood\"\ncolors = {{ top = \"plank\", front = \"red\" }}\n"
    ));
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 8), ("plank", 2), ("red", 2)])
    );
    assert_eq!(colour_facing(&model, [0.0, 1.0, 0.0]), "plank");
    // Front is +z.
    assert_eq!(colour_facing(&model, [0.0, 0.0, 1.0]), "red");
    assert_eq!(colour_facing(&model, [0.0, 0.0, -1.0]), "wood");
    assert_eq!(colour_facing(&model, [0.0, -1.0, 0.0]), "wood");
}

#[test]
fn every_shape_has_named_faces() {
    // One part, with a palette of just the colours it names.
    let shape = |part: &str| {
        let palette: String = ["wood", "plank", "red"]
            .iter()
            .filter(|name| part.contains(&format!("\"{name}\"")))
            .map(|name| format!("{name} = \"#102030\"\n"))
            .collect();
        counts(&format!(
            "version = 0\n[palette]\n{palette}[[parts]]\n{part}\n"
        ))
    };
    // A box's four sides at once.
    assert_eq!(
        shape("shape = \"box\"\ncolor = \"plank\"\ncolors = { sides = \"wood\", top = \"red\" }"),
        named(&[("wood", 8), ("plank", 2), ("red", 2)])
    );
    // A cylinder of 8: 6 triangles for each end, 16 round the side.
    assert_eq!(
        shape(
            "shape = \"cylinder\"\nsegments = 8\ncolors = { top = \"red\", bottom = \"plank\", side = \"wood\" }"
        ),
        named(&[("wood", 16), ("plank", 6), ("red", 6)])
    );
    // A cone of 8: 8 round the side, 6 underneath.
    assert_eq!(
        shape("shape = \"cone\"\nsegments = 8\ncolors = { side = \"wood\", bottom = \"red\" }"),
        named(&[("wood", 8), ("red", 6)])
    );
    // A sphere is one face all round.
    assert_eq!(
        shape("shape = \"sphere\"\nsegments = 8\ncolors = { side = \"red\" }"),
        named(&[("red", 48)])
    );
    // A wedge: slope, bottom and back are two triangles each, the two
    // ends one each.
    assert_eq!(
        shape(
            "shape = \"wedge\"\ncolor = \"wood\"\ncolors = { slope = \"red\", left = \"plank\" }"
        ),
        named(&[("wood", 5), ("plank", 1), ("red", 2)])
    );
}

#[test]
fn a_face_name_the_shape_does_not_have_is_an_error() {
    let issues = errors(&format!(
        "{PALETTE}[[parts]]\nshape = \"cylinder\"\ncolor = \"wood\"\ncolors = {{ sides = \"red\", lid = \"plank\" }}\n"
    ));
    let faces: Vec<&Issue> = issues
        .iter()
        .filter(|issue| issue.code == Code::UnknownFace)
        .collect();
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].path, "parts[0].colors.sides");
    assert_eq!(faces[0].message, "this part has no face called \"sides\"");
    assert_eq!(faces[0].suggestion.as_deref(), Some("side"));
    assert_eq!(
        faces[0].allowed,
        Some(Allowed::OneOf {
            options: ["top", "bottom", "side"].map(String::from).to_vec()
        })
    );
    assert_eq!(faces[1].suggestion, None);
    assert_eq!(faces[1].line, Some(9));
}

#[test]
fn a_colour_must_be_a_palette_name() {
    let issues = errors(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"wod\"\n"
    ));
    let issue = only(&issues, Code::UnknownColor);
    assert_eq!(
        (issue.path.as_str(), issue.line),
        ("parts[0].color", Some(8))
    );
    assert_eq!(issue.message, "there is no colour \"wod\" in the palette");
    assert_eq!(issue.suggestion.as_deref(), Some("wood"));
    assert_eq!(
        issue.to_string(),
        "error: test.toml:8: parts[0].color: there is no colour \"wod\" in the palette. Did you mean \"wood\"?"
    );
    // Only that is reported: not also "this part has no colour".
    assert_eq!(issues.len(), 1);

    let issues = errors(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"wood\"\ncolors = {{ top = \"grass\" }}\n"
    ));
    let issue = only(&issues, Code::UnknownColor);
    assert_eq!(issue.path, "parts[0].colors.top");
    assert!(
        matches!(&issue.allowed, Some(Allowed::OneOf { options }) if options == &["wood", "plank", "red"])
    );
}

#[test]
fn a_colour_code_where_a_name_belongs_says_what_to_do() {
    let issues = errors(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"#112233\"\n"
    ));
    let issue = only(&issues, Code::UnknownColor);
    assert!(
        issue
            .message
            .contains("is not a palette name. Add it to [palette]"),
        "{}",
        issue.message
    );
    assert!(issue.message.contains("accent = \"#112233\""));

    // If the palette already has that very colour, its name is the fix.
    let issues = errors(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"#d0453a\"\n"
    ));
    assert_eq!(
        only(&issues, Code::UnknownColor).suggestion.as_deref(),
        Some("red")
    );

    let issues = errors("version = 0\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n");
    assert!(
        only(&issues, Code::UnknownColor)
            .message
            .contains("this recipe has no palette")
    );
}

#[test]
fn an_assemblys_color_is_for_everything_inside_without_one() {
    // The arm fixes its red hand and leaves its upper part to whoever
    // uses it.
    let arm = "[define.arm]\n[[define.arm.parts]]\nname = \"upper\"\nshape = \"box\"\n[[define.arm.parts]]\nname = \"hand\"\nshape = \"box\"\ncolor = \"red\"\nattach = \"top\"\nto = \"upper.bottom\"\n";
    let model = built(&format!(
        "{PALETTE}{arm}[[parts]]\nname = \"a\"\nuse = \"arm\"\ncolor = \"wood\"\n[[parts]]\nname = \"b\"\nuse = \"arm\"\ncolor = \"plank\"\nmove = [3, 0, 0]\n"
    ));
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 12), ("plank", 12), ("red", 24)])
    );

    // The same for parts nested in place, and through more than one level.
    let model = built(&format!(
        "{PALETTE}[[parts]]\ncolor = \"wood\"\nparts = [{{ shape = \"box\" }}, {{ move = [2, 0, 0], parts = [{{ shape = \"box\" }}, {{ shape = \"box\", color = \"red\", move = [0, 2, 0] }}] }}]\n"
    ).replace("plank = \"#8A5A2B\"\n", ""));
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 24), ("red", 12)])
    );
}

#[test]
fn the_nearest_setting_wins_and_a_face_name_beats_a_whole_colour() {
    let leg = "[define.leg]\nshape = \"box\"\ncolor = \"wood\"\ncolors = { top = \"plank\" }\n";
    // The definition's own colours stay, whatever the user of it says
    // about the whole part...
    let (model, report) = build(&format!(
        "{PALETTE}{leg}[[parts]]\nuse = \"leg\"\ncolor = \"red\"\n"
    ));
    assert_eq!(
        triangles_by_colour(&model.unwrap()),
        named(&[("wood", 10), ("plank", 2)])
    );
    // ...and that is pointed out, since it is probably not what was meant.
    let warning = only(&report.warnings, Code::Unused);
    assert_eq!(warning.path, "parts[0].color");
    assert!(warning.message.contains("changes nothing"));

    // A face name where the part is used beats the definition's whole
    // colour, but not the definition's own face name.
    let model = built(&format!(
        "{PALETTE}{leg}[[parts]]\nuse = \"leg\"\ncolors = {{ bottom = \"red\", top = \"red\" }}\n"
    ));
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 8), ("plank", 2), ("red", 2)])
    );
    assert_eq!(colour_facing(&model, [0.0, -1.0, 0.0]), "red");
    assert_eq!(colour_facing(&model, [0.0, 1.0, 0.0]), "plank");
}

#[test]
fn face_names_do_not_reach_through_an_assembly() {
    let arm = "[define.arm]\n[[define.arm.parts]]\nshape = \"box\"\ncolor = \"wood\"\n";
    let issues = errors(
        &format!("{PALETTE}{arm}[[parts]]\nuse = \"arm\"\ncolors = {{ top = \"red\" }}\n")
            .replace("plank = \"#8A5A2B\"\n", ""),
    );
    let issue = only(&issues, Code::UnknownFace);
    assert_eq!(issue.path, "parts[0].colors.top");
    assert!(
        issue.message.contains("made of other parts"),
        "{}",
        issue.message
    );

    let issues = errors(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\ncolors = { top = \"red\" }\nparts = [{ shape = \"box\", color = \"red\" }]\n",
    );
    assert_eq!(only(&issues, Code::UnknownFace).path, "parts[0].colors.top");
}

#[test]
fn a_part_with_no_colour_is_built_grey_with_a_warning() {
    let (model, report) = build("version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\n");
    let model = model.unwrap();
    assert!(model.faces.iter().all(|face| face.color.is_none()));
    let warning = only(&report.warnings, Code::NoColor);
    assert!(!warning.is_error());
    assert_eq!((warning.path.as_str(), warning.line), ("parts[0]", Some(2)));
    assert!(warning.message.contains("plain grey"));

    // Colouring only some faces leaves the rest grey: the same warning.
    let found = warnings(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nshape = \"box\"\ncolors = { top = \"red\" }\n",
    );
    assert_eq!(only(&found, Code::NoColor).path, "parts[0]");
    // Colouring every face by name is enough.
    built(
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nshape = \"cone\"\ncolors = { side = \"red\", bottom = \"red\" }\n",
    );
}

#[test]
fn the_warning_points_at_the_part_that_lacks_a_colour_not_at_its_definition() {
    // The leg has no colour of its own. One use gives it one, one doesn't.
    let found = warnings(
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[define.leg]\nshape = \"box\"\n[[parts]]\nuse = \"leg\"\ncolor = \"wood\"\n[[parts]]\nuse = \"leg\"\nmove = [2, 0, 0]\n",
    );
    let warning = only(&found, Code::NoColor);
    assert_eq!((warning.path.as_str(), warning.line), ("parts[1]", Some(9)));
}

#[test]
fn a_palette_colour_nothing_names_is_a_warning() {
    let found = warnings(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n"
    ));
    let unused: Vec<(&str, Option<usize>)> = found
        .iter()
        .map(|issue| (issue.path.as_str(), issue.line))
        .collect();
    assert_eq!(
        unused,
        [("palette.plank", Some(4)), ("palette.red", Some(5))]
    );
    assert_eq!(found[0].message, "the colour \"plank\" is never used");
    assert!(found.iter().all(|issue| issue.code == Code::Unused));

    // A colour named only by a part that is itself never used counts as
    // named: the warning is about that part instead.
    let found = warnings(
        "version = 0\n[palette]\nwood = \"#B07A3F\"\nred = \"#D0453A\"\n[define.spare]\nshape = \"box\"\ncolor = \"red\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\n",
    );
    assert_eq!(only(&found, Code::Unused).path, "define.spare");
}

#[test]
fn colours_from_a_shared_palette_file_are_never_unused() {
    let folder = folder("colour_shared_palette");
    std::fs::write(
        folder.join("palette.toml"),
        "steel = \"#8A939B\"\nred = \"#D0453A\"\ngold = \"#E0B040\"\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("robot.toml"),
        "version = 0\npalette = \"palette.toml\"\n[[parts]]\nshape = \"box\"\ncolor = \"steel\"\ncolors = { front = \"red\" }\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("robot.toml"), &mut issues).unwrap();
    let model = assemble(&loaded, &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    // The whole shared palette travels with the model, used or not.
    assert_eq!(model.palette.len(), 3);
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("steel", 10), ("red", 2)])
    );
}

#[test]
fn a_recipe_used_as_a_part_brings_its_colours_and_takes_a_default() {
    let folder = folder("colour_recipe_part");
    std::fs::write(
        folder.join("wheel.toml"),
        "version = 0\n[palette]\nrubber = \"#222222\"\n[[parts]]\nname = \"tyre\"\nshape = \"cylinder\"\nsegments = 8\ncolor = \"rubber\"\n[[parts]]\nname = \"hub\"\nshape = \"box\"\nsize = [0.2, 1.2, 0.2]\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("cart.toml"),
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nfile = \"wheel.toml\"\ncolor = \"wood\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("cart.toml"), &mut issues).unwrap();
    let model = assemble(&loaded, &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    // The tyre keeps its own colour; the hub, which had none, takes the
    // cart's.
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 12), ("rubber", 28)])
    );
    let names: Vec<&str> = model
        .palette
        .iter()
        .map(|color| color.name.as_str())
        .collect();
    assert_eq!(names, ["wood", "rubber"]);
}

#[test]
fn mirrored_and_turned_parts_keep_their_face_colours() {
    // A flipped part's faces are listed backwards; the colours must
    // follow the faces.
    let model = built(&format!(
        "{PALETTE}[[parts]]\nshape = \"box\"\nmirror = \"x\"\nrotate = [0, 90, 0]\ncolor = \"wood\"\ncolors = {{ left = \"red\", top = \"plank\" }}\n"
    ));
    assert_eq!(
        triangles_by_colour(&model),
        named(&[("wood", 8), ("plank", 2), ("red", 2)])
    );
    // The face called "left" is on the right after the flip (+x), and
    // faces the back (-z) after the quarter turn.
    assert_eq!(colour_facing(&model, [0.0, 0.0, -1.0]), "red");
    assert_eq!(colour_facing(&model, [0.0, 1.0, 0.0]), "plank");
}
