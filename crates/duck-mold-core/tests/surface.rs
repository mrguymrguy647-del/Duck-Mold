//! Phase 1, step 3: sockets that know the shape.
//!
//! The rule: a built-in socket never floats. A socket already on the
//! shape's surface doesn't move or turn. One that was in the air beside
//! the shape moves straight in until it touches it, and faces straight
//! out of the surface there. Sockets declared with `on` and `shift`
//! follow the same rule.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::inspect::inspect;
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load_text;
use duck_mold_core::math::{Vec3, vec3};
use duck_mold_core::model::{Model, Part};
use duck_mold_core::source::Syntax;

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

fn part<'a>(model: &'a Model, name: &str) -> &'a Part {
    model
        .parts
        .iter()
        .find(|part| part.name == name)
        .unwrap_or_else(|| panic!("no part {name}"))
}

fn socket(model: &Model, part_name: &str, name: &str) -> (Vec3, Vec3) {
    let socket = part(model, part_name)
        .sockets
        .iter()
        .find(|socket| socket.name == name)
        .unwrap_or_else(|| panic!("{part_name} has no socket {name}"));
    (socket.at, socket.facing)
}

/// One part, `shape` plus whatever else, with its middle at the model's
/// zero point so positions are easy to read.
fn one(part: &str) -> Model {
    built(&format!(
        "version = 0\norigin = \"p.center\"\n[[parts]]\nname = \"p\"\n{part}\n"
    ))
}

/// `v` made 1 long and tidied like the model's sockets.
fn unit(v: Vec3) -> Vec3 {
    v.normalized().unwrap().tidied()
}

const SIDES: [&str; 7] = ["top", "bottom", "left", "right", "front", "back", "center"];

#[test]
fn a_box_keeps_every_socket_where_it_was() {
    let model = one("shape = \"box\"\nsize = [2, 1, 4]");
    let expected = [
        (vec3(0.0, 0.5, 0.0), vec3(0.0, 1.0, 0.0)),
        (vec3(0.0, -0.5, 0.0), vec3(0.0, -1.0, 0.0)),
        (vec3(-1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0)),
        (vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0)),
        (vec3(0.0, 0.0, 2.0), vec3(0.0, 0.0, 1.0)),
        (vec3(0.0, 0.0, -2.0), vec3(0.0, 0.0, -1.0)),
        (vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0)),
    ];
    for (name, expected) in SIDES.into_iter().zip(expected) {
        assert_eq!(socket(&model, "p", name), expected, "{name}");
    }
}

#[test]
fn a_cones_sides_sit_halfway_up_its_slant_facing_out_of_it() {
    // Radius 0.5, height 1: halfway up, the slant is 0.25 from the middle,
    // and it faces out and up, 1 along for every 0.5 up (about 27°).
    let model = one("shape = \"cone\"");
    assert_eq!(
        socket(&model, "p", "left"),
        (vec3(-0.25, 0.0, 0.0), unit(vec3(-1.0, 0.5, 0.0)))
    );
    assert_eq!(
        socket(&model, "p", "right"),
        (vec3(0.25, 0.0, 0.0), unit(vec3(1.0, 0.5, 0.0)))
    );
    assert_eq!(
        socket(&model, "p", "front"),
        (vec3(0.0, 0.0, 0.25), unit(vec3(0.0, 0.5, 1.0)))
    );
    assert_eq!(
        socket(&model, "p", "back"),
        (vec3(0.0, 0.0, -0.25), unit(vec3(0.0, 0.5, -1.0)))
    );
    // Its point and its base were on it already.
    assert_eq!(
        socket(&model, "p", "top"),
        (vec3(0.0, 0.5, 0.0), vec3(0.0, 1.0, 0.0))
    );
    assert_eq!(
        socket(&model, "p", "bottom"),
        (vec3(0.0, -0.5, 0.0), vec3(0.0, -1.0, 0.0))
    );
}

#[test]
fn a_cone_whose_side_is_an_edge_gets_a_socket_on_the_edge() {
    // Three flat sides, the first facing right: on the left there's an
    // edge where two sides meet, and the socket faces halfway between
    // them, which is straight out to the left and up.
    let model = one("shape = \"cone\"\nsegments = 3");
    let (at, facing) = socket(&model, "p", "left");
    let box_left = part(&model, "p").bounds.unwrap().min.x;
    assert!(at.x > box_left + 0.1, "moved in from the box: {at:?}");
    assert_eq!((at.y, at.z), (0.0, 0.0));
    assert!(
        facing.x < 0.0 && facing.y > 0.0 && facing.z == 0.0,
        "{facing:?}"
    );
}

#[test]
fn a_wedge_has_a_slope_socket_and_its_top_and_front_moved_onto_the_slope() {
    // The slope runs from the top of the back to the bottom of the front,
    // through the middle of the box.
    let model = one("shape = \"wedge\"\nsize = [2, 1, 4]");
    let on_slope = (vec3(0.0, 0.0, 0.0), unit(vec3(0.0, 4.0, 1.0)));
    assert_eq!(socket(&model, "p", "slope"), on_slope);
    assert_eq!(socket(&model, "p", "top"), on_slope);
    assert_eq!(socket(&model, "p", "front"), on_slope);
    // Its real sides didn't move.
    assert_eq!(
        socket(&model, "p", "back"),
        (vec3(0.0, 0.0, -2.0), vec3(0.0, 0.0, -1.0))
    );
    assert_eq!(
        socket(&model, "p", "bottom"),
        (vec3(0.0, -0.5, 0.0), vec3(0.0, -1.0, 0.0))
    );
    assert_eq!(
        socket(&model, "p", "left"),
        (vec3(-1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0))
    );
    assert_eq!(
        socket(&model, "p", "right"),
        (vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0))
    );
    // Only a wedge has one.
    assert!(
        !part(&one("shape = \"box\""), "p")
            .sockets
            .iter()
            .any(|socket| socket.name == "slope")
    );
}

#[test]
fn shifted_sockets_on_round_sides_sit_on_the_surface_and_look_out_of_it() {
    // Eyes on a ball, up and to the side of its front: on the ball, not
    // in the air in front of it, and looking up and out.
    let model =
        one("shape = \"sphere\"\n[parts.sockets]\neye = { on = \"front\", shift = [0.4, 0.4, 0] }");
    let (at, facing) = socket(&model, "p", "eye");
    assert!((at.x, at.y) == (0.2, 0.2), "slid as before: {at:?}");
    assert!(at.z > 0.35 && at.z < 0.45, "on the ball: {at:?}");
    assert!(
        facing.x > 0.2 && facing.y > 0.2 && facing.z > 0.5,
        "{facing:?}"
    );

    // A cylinder's flat top is the surface already: nothing changes.
    let model =
        one("shape = \"cylinder\"\n[parts.sockets]\nrim = { on = \"top\", shift = [0.6, 0, 0.3] }");
    assert_eq!(
        socket(&model, "p", "rim"),
        (vec3(0.3, 0.5, 0.15), vec3(0.0, 1.0, 0.0))
    );

    // Its round side is not: a socket slid round it moves in.
    let model = one(
        "shape = \"cylinder\"\n[parts.sockets]\nhandle = { on = \"right\", shift = [0, 0, 0.6] }",
    );
    let (at, facing) = socket(&model, "p", "handle");
    assert!(at.x < 0.5 && at.x > 0.35 && at.z == 0.3, "{at:?}");
    assert!(facing.z > 0.3 && facing.y == 0.0, "{facing:?}");
}

#[test]
fn a_facing_given_in_the_recipe_is_kept_on_the_surface() {
    // A chimney standing upright on a roof: on the slope, facing up.
    let model = one(
        "shape = \"wedge\"\n[parts.sockets]\nchimney = { on = \"top\", shift = [0.5, 0, 0.5], facing = \"up\" }",
    );
    assert_eq!(
        socket(&model, "p", "chimney"),
        (vec3(0.25, -0.25, 0.25), vec3(0.0, 1.0, 0.0))
    );
}

#[test]
fn a_shift_beside_the_shape_is_an_error_that_says_so() {
    let found = errors(
        "version = 0\n[[parts]]\nname = \"p\"\nshape = \"cone\"\n[parts.sockets]\nhook = { on = \"left\", shift = [0, 0.9, 0.9] }\n",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, Code::OutOfRange);
    assert_eq!(found[0].path, "parts[0].sockets.hook");
    assert_eq!(
        found[0].message,
        "shift = [0, 0.9, 0.9] puts the socket \"hook\" beside the cone, where there is nothing for it to sit on: bring the numbers closer to 0"
    );
}

#[test]
fn a_wedge_cant_declare_a_socket_called_slope() {
    let found = errors(
        "version = 0\n[[parts]]\nname = \"p\"\nshape = \"wedge\"\n[parts.sockets]\nslope = { on = \"top\" }\n",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].code, Code::DuplicateName);
    // Any other part can.
    let model = one("shape = \"box\"\n[parts.sockets]\nslope = { on = \"top\" }");
    assert_eq!(socket(&model, "p", "slope").0, vec3(0.0, 0.5, 0.0));
}

#[test]
fn a_misspelt_slope_is_suggested() {
    let found = errors(
        "version = 0\n[[parts]]\nname = \"roof\"\nshape = \"wedge\"\n[[parts]]\nname = \"vane\"\nshape = \"box\"\nattach = \"bottom\"\nto = \"roof.slop\"\n",
    );
    assert_eq!(found[0].code, Code::UnknownSocket);
    assert_eq!(found[0].suggestion.as_deref(), Some("slope"));
}

#[test]
fn parts_joined_to_a_cone_or_a_slope_touch_it() {
    // A tree with baubles on its sides and a wedge roof with a vane: in
    // Phase 0 all of these floated beside the slopes.
    let model = built(
        r#"
version = 0
[[parts]]
name = "tree"
shape = "cone"
radius = 1
height = 2

[[parts]]
name = "bauble_left"
shape = "sphere"
radius = 0.1
attach = "center"
to = "tree.left"

[[parts]]
name = "bauble_front"
shape = "sphere"
radius = 0.1
attach = "center"
to = "tree.front"
"#,
    );
    let report = inspect(&model, "test.toml", Vec::new());
    assert!(report.floating.is_empty(), "{:#?}", report.floating);

    let model = built(
        r#"
version = 0
[[parts]]
name = "roof"
shape = "wedge"

[[parts]]
name = "vane"
shape = "box"
size = [0.1, 0.4, 0.1]
attach = "bottom"
to = "roof.slope"
"#,
    );
    let report = inspect(&model, "test.toml", Vec::new());
    assert!(report.floating.is_empty(), "{:#?}", report.floating);
    // The vane stands out of the roof at right angles: tilted forward.
    let vane_top = socket(&model, "vane", "top").1;
    assert_eq!(vane_top, unit(vec3(0.0, 1.0, 1.0)));
}

#[test]
fn a_scaled_cone_still_has_its_sockets_on_its_slant() {
    // Twice as wide: the slant leans at 45°.
    let model = one("shape = \"cone\"\nscale = [2, 1, 1]");
    assert_eq!(
        socket(&model, "p", "left"),
        (vec3(-0.5, 0.0, 0.0), unit(vec3(-1.0, 1.0, 0.0)))
    );
}

#[test]
fn a_mesh_or_an_assembly_keeps_its_sockets_on_its_box() {
    let model = built(
        "version = 0\norigin = \"group.center\"\n[[parts]]\nname = \"group\"\n[[parts.parts]]\nname = \"c\"\nshape = \"cone\"\n",
    );
    assert_eq!(
        socket(&model, "group", "left"),
        (vec3(-0.5, 0.0, 0.0), vec3(-1.0, 0.0, 0.0))
    );
}
