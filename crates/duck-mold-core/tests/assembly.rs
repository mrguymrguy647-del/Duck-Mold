//! Steps 3 and 4: a recipe's parts are moved, turned, scaled and
//! mirrored; parts declare sockets and are joined by socket name;
//! assemblies nest.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::{Allowed, Code, Issue, Report};
use duck_mold_core::load::{load, load_text};
use duck_mold_core::math::{Bounds, Vec3, vec3};
use duck_mold_core::model::{Model, Part};
use duck_mold_core::source::Syntax;

fn build(text: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, issues)
}

/// Builds a recipe that must have no errors.
fn built(text: &str) -> Model {
    let (model, issues) = build(text);
    let errors: Vec<&Issue> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.expect("a recipe without errors builds")
}

fn errors(text: &str) -> Vec<Issue> {
    let (model, issues) = build(text);
    assert!(model.is_none(), "a recipe with errors must not build");
    // A mistake inside a part that is used twice is found twice, and
    // reported once.
    Report::from_issues(issues).errors
}

fn only(issues: &[Issue], code: Code) -> &Issue {
    let found: Vec<&Issue> = issues.iter().filter(|issue| issue.code == code).collect();
    assert_eq!(found.len(), 1, "expected one {code} in {issues:#?}");
    found[0]
}

/// The part with this path: `left_arm/hand`.
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

fn bounds(model: &Model, path: &str) -> Bounds {
    part(model, path).bounds.expect("the part has faces")
}

fn socket(model: &Model, path: &str, name: &str) -> (Vec3, Vec3) {
    let socket = part(model, path)
        .sockets
        .iter()
        .find(|socket| socket.name == name)
        .unwrap_or_else(|| panic!("{path} has no socket {name}"));
    (socket.at, socket.facing)
}

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A recipe with a 1 x 1 x 1 box called `a` first, then `rest`. With
/// `origin = "a.center"` the box's middle is the model's zero point, so
/// positions are easy to read.
fn after_a(rest: &str) -> String {
    format!(
        "version = 0\norigin = \"a.center\"\n[[parts]]\nname = \"a\"\nshape = \"box\"\n{rest}\n"
    )
}

// ---- step 3: move, rotate, scale, mirror -----------------------------------

#[test]
fn a_model_stands_on_the_ground_by_default() {
    let model = built("version = 0\n[[parts]]\nshape = \"box\"\nsize = [2, 3, 4]\n");
    let bounds = model.bounds().unwrap();
    assert_eq!(bounds.min, vec3(-1.0, 0.0, -2.0));
    assert_eq!(bounds.max, vec3(1.0, 3.0, 2.0));
    assert_eq!(model.triangle_count(), 12);
    assert_eq!(model.vertex_count(), 24);
    // An unnamed part is called by its place in the list.
    assert_eq!(model.parts[0].name, "#0");
}

#[test]
fn move_puts_a_parts_centre_somewhere() {
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"box\"\nmove = [2, 0.5, -3]",
    ));
    assert_eq!(bounds(&model, "b").center().tidied(), vec3(2.0, 0.5, -3.0));
    assert_eq!(bounds(&model, "a").center().tidied(), Vec3::ZERO);
}

#[test]
fn rotate_turns_duck_forges_way() {
    // A plank 3 long from back to front, turned a quarter around the
    // upright axis, is 3 long from left to right.
    let model = built(&after_a(
        "[[parts]]\nname = \"plank\"\nshape = \"box\"\nsize = [1, 0.2, 3]\nrotate = [0, 90, 0]",
    ));
    assert_eq!(bounds(&model, "plank").size().tidied(), vec3(3.0, 0.2, 1.0));
    // Its own front (+z) now faces right (+x), as in Duck Forge.
    assert_eq!(
        socket(&model, "plank", "front"),
        (vec3(1.5, 0.0, 0.0), vec3(1.0, 0.0, 0.0))
    );

    // Around x, forward turns towards up.
    let model = built(&after_a(
        "[[parts]]\nname = \"plank\"\nshape = \"box\"\nsize = [1, 0.2, 3]\nrotate = [90, 0, 0]",
    ));
    assert_eq!(socket(&model, "plank", "front").1, vec3(0.0, 1.0, 0.0));
}

#[test]
fn scale_makes_a_part_bigger_evenly_or_not() {
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"box\"\nscale = 2\n[[parts]]\nname = \"c\"\nshape = \"cylinder\"\nscale = [1, 3, 0.5]",
    ));
    assert_eq!(bounds(&model, "b").size().tidied(), vec3(2.0, 2.0, 2.0));
    assert_eq!(bounds(&model, "c").size().tidied(), vec3(1.0, 3.0, 0.5));
}

#[test]
fn mirror_flips_a_part_and_its_sockets() {
    // A wedge slopes down to the front. Mirrored across z, to the back.
    let model = built(&after_a(
        "[[parts]]\nname = \"ramp\"\nshape = \"wedge\"\nmirror = \"z\"\nmove = [3, 0, 0]",
    ));
    let ramp = part(&model, "ramp");
    assert!(ramp.mirrored);
    let slope = &model.faces[ramp.faces.start];
    let normal = slope.normal().unwrap();
    assert!(normal.y > 0.0 && normal.z < 0.0, "{normal:?}");
    // Its socket called "back" is now at its front.
    assert_eq!(
        socket(&model, "ramp", "back"),
        (vec3(3.0, 0.0, 0.5), vec3(0.0, 0.0, 1.0))
    );
    // And its slope socket faces up and back, out of the flipped slope.
    let half = std::f64::consts::FRAC_1_SQRT_2;
    assert_eq!(
        socket(&model, "ramp", "slope"),
        (vec3(3.0, 0.0, 0.0), vec3(0.0, half, -half).tidied())
    );
    // Every face still faces outwards.
    let middle = ramp.bounds.unwrap().center() + vec3(0.0, -0.2, 0.2);
    for face in &model.faces[ramp.faces.clone()] {
        let outwards = face.corners[0] - middle;
        assert!(face.normal().unwrap().dot(outwards) > 0.0);
    }
}

#[test]
fn a_part_is_mirrored_then_scaled_then_turned_then_moved() {
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"box\"\nscale = [2, 1, 1]\nrotate = [0, 90, 0]\nmove = [5, 0, 0]",
    ));
    // Stretched along x first, then turned: so it ends up long in z.
    assert_eq!(bounds(&model, "b").size().tidied(), vec3(1.0, 1.0, 2.0));
    assert_eq!(bounds(&model, "b").center().tidied(), vec3(5.0, 0.0, 0.0));
}

// ---- step 4: sockets ---------------------------------------------------------

const TREE: &str = r#"
version = 0
name = "tree"

[[parts]]
name = "trunk"
shape = "cylinder"
radius = 0.15
height = 1.2

[[parts]]
name = "leaves"
shape = "cone"
radius = 0.7
height = 1.6
attach = "bottom"
to = "trunk.top"
"#;

#[test]
fn a_part_is_joined_by_socket_name_with_no_coordinates() {
    let model = built(TREE);
    let trunk = bounds(&model, "trunk");
    let leaves = bounds(&model, "leaves");
    // The cone's underside sits exactly on the trunk's top, centred.
    assert_eq!(trunk.min.y, 0.0);
    assert_eq!(leaves.min.y, trunk.max.y);
    assert_eq!(leaves.min.y, 1.2);
    assert_eq!((leaves.center().x, leaves.center().z), (0.0, 0.0));
    assert_eq!(model.bounds().unwrap().size().tidied(), vec3(1.4, 2.8, 1.4));
    assert_eq!(
        part(&model, "leaves").attached_to.as_deref(),
        Some("trunk.top")
    );
    assert_eq!(part(&model, "trunk").attached_to, None);
    // The order of the list doesn't matter: a part can come before the
    // one it hangs from.
    let swapped = TREE.replace(
        "[[parts]]\nname = \"trunk\"\nshape = \"cylinder\"\nradius = 0.15\nheight = 1.2\n",
        "",
    ) + "\n[[parts]]\nname = \"trunk\"\nshape = \"cylinder\"\nradius = 0.15\nheight = 1.2\n";
    let swapped = built(&swapped);
    assert_eq!(bounds(&swapped, "leaves"), leaves);
}

#[test]
fn every_part_has_seven_sockets_built_in() {
    let model = built(&after_a(""));
    let names: Vec<&str> = part(&model, "a")
        .sockets
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["top", "bottom", "left", "right", "front", "back", "center"]
    );
    assert_eq!(
        socket(&model, "a", "top"),
        (vec3(0.0, 0.5, 0.0), vec3(0.0, 1.0, 0.0))
    );
    assert_eq!(
        socket(&model, "a", "left"),
        (vec3(-0.5, 0.0, 0.0), vec3(-1.0, 0.0, 0.0))
    );
    // Front is where the model faces: +z.
    assert_eq!(
        socket(&model, "a", "front"),
        (vec3(0.0, 0.0, 0.5), vec3(0.0, 0.0, 1.0))
    );
    assert_eq!(
        socket(&model, "a", "center"),
        (Vec3::ZERO, vec3(0.0, 1.0, 0.0))
    );
}

#[test]
fn a_part_joined_side_to_side_is_not_turned() {
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"box\"\nsize = [0.4, 0.6, 0.2]\nattach = \"left\"\nto = \"a.right\"",
    ));
    let b = bounds(&model, "b");
    assert_eq!(b.min.x, 0.5);
    assert_eq!(b.size().tidied(), vec3(0.4, 0.6, 0.2));
    assert_eq!(b.center().tidied(), vec3(0.7, 0.0, 0.0));
    assert_eq!(socket(&model, "b", "top").1, vec3(0.0, 1.0, 0.0));
}

#[test]
fn a_part_turns_as_little_as_possible_to_face_its_socket() {
    // A post standing upright, its top joined to the right side of `a`:
    // it lies down, pointing right, and its front stays in front.
    let model = built(&after_a(
        "[[parts]]\nname = \"post\"\nshape = \"box\"\nsize = [0.2, 1, 0.2]\nattach = \"top\"\nto = \"a.right\"",
    ));
    let post = bounds(&model, "post");
    assert_eq!(post.size().tidied(), vec3(1.0, 0.2, 0.2));
    assert_eq!(post.min.x, 0.5);
    assert_eq!(
        socket(&model, "post", "top"),
        (vec3(0.5, 0.0, 0.0), vec3(-1.0, 0.0, 0.0))
    );
    assert_eq!(socket(&model, "post", "front").1, vec3(0.0, 0.0, 1.0));
}

#[test]
fn sockets_that_face_the_same_way_follow_the_fixed_rule() {
    // Sideways sockets: the part turns round the upright axis and stays
    // the right way up.
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"wedge\"\nattach = \"right\"\nto = \"a.right\"",
    ));
    assert_eq!(bounds(&model, "b").min.x, 0.5);
    assert_eq!(socket(&model, "b", "bottom").1, vec3(0.0, -1.0, 0.0));
    assert_eq!(socket(&model, "b", "back").1, vec3(0.0, 0.0, 1.0));

    // Upright sockets: the part rolls over sideways, its front still in
    // front.
    let model = built(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"wedge\"\nattach = \"bottom\"\nto = \"a.bottom\"",
    ));
    assert_eq!(bounds(&model, "b").max.y, -0.5);
    assert_eq!(
        socket(&model, "b", "bottom"),
        (vec3(0.0, -0.5, 0.0), vec3(0.0, 1.0, 0.0))
    );
    assert_eq!(socket(&model, "b", "back").1, vec3(0.0, 0.0, -1.0));
    assert_eq!(socket(&model, "b", "left").1, vec3(1.0, 0.0, 0.0));
}

#[test]
fn twist_turns_an_attached_part_around_its_join() {
    let plank = "[[parts]]\nname = \"plank\"\nshape = \"box\"\nsize = [1, 0.2, 0.4]\n";
    let straight = built(&after_a(&format!(
        "{plank}attach = \"bottom\"\nto = \"a.top\""
    )));
    assert_eq!(
        bounds(&straight, "plank").size().tidied(),
        vec3(1.0, 0.2, 0.4)
    );

    let twisted = built(&after_a(&format!(
        "{plank}attach = \"bottom\"\nto = \"a.top\"\ntwist = 90"
    )));
    assert_eq!(
        bounds(&twisted, "plank").size().tidied(),
        vec3(0.4, 0.2, 1.0)
    );
    assert_eq!(bounds(&twisted, "plank").min.y, 0.5);

    // On a socket that faces up it is the same turn as rotate = [0, 90, 0]
    // on a free part: forward turns towards right.
    let free = built(&after_a(&format!("{plank}rotate = [0, 90, 0]")));
    assert_eq!(
        socket(&twisted, "plank", "front").1,
        socket(&free, "plank", "front").1
    );
    assert_eq!(socket(&twisted, "plank", "front").1, vec3(1.0, 0.0, 0.0));

    // On a socket that faces right, a quarter twist seen from the right
    // is clockwise: up goes towards the front.
    let side = built(&after_a(&format!(
        "{plank}attach = \"left\"\nto = \"a.right\"\ntwist = 90"
    )));
    assert_eq!(socket(&side, "plank", "top").1, vec3(0.0, 0.0, 1.0));
    assert_eq!(bounds(&side, "plank").min.x, 0.5);
}

const TABLE: &str = r#"
version = 0

[define.leg]
shape = "box"
size = [0.08, 0.7, 0.08]

[[parts]]
name = "top"
shape = "box"
size = [1.2, 0.06, 0.7]

[parts.sockets]
leg_fl = { on = "bottom", shift = [-0.85, 0, 0.8] }
leg_fr = { on = "bottom", shift = [0.85, 0, 0.8] }
leg_bl = { on = "bottom", shift = [-0.85, 0, -0.8] }
leg_br = { on = "bottom", shift = [0.85, 0, -0.8] }

[[parts]]
name = "leg_fl"
use = "leg"
attach = "top"
to = "top.leg_fl"

[[parts]]
name = "leg_fr"
use = "leg"
attach = "top"
to = "top.leg_fr"

[[parts]]
name = "leg_bl"
use = "leg"
attach = "top"
to = "top.leg_bl"

[[parts]]
name = "leg_br"
use = "leg"
attach = "top"
to = "top.leg_br"
"#;

#[test]
fn declared_sockets_slide_across_a_side_and_defined_parts_are_reused() {
    let model = built(TABLE);
    assert_eq!(
        model.bounds().unwrap().size().tidied(),
        vec3(1.2, 0.76, 0.7)
    );
    assert_eq!(model.bounds().unwrap().min.y, 0.0);
    // Front left: 85% of the way to the left edge, 80% to the front.
    let leg = bounds(&model, "leg_fl");
    assert_eq!(leg.center().tidied(), vec3(-0.51, 0.35, 0.28));
    assert_eq!(leg.max.y, bounds(&model, "top").min.y);
    assert_eq!(
        bounds(&model, "leg_br").center().tidied(),
        vec3(0.51, 0.35, -0.28)
    );
    // Each leg knows what it was made from.
    let kind = &part(&model, "leg_bl").kind;
    assert_eq!(
        (kind.shape.as_str(), kind.uses.as_deref()),
        ("box", Some("leg"))
    );
    assert_eq!(kind.label(), "leg (box)");
    assert_eq!(model.triangle_count(), 5 * 12);
    // The table top's declared sockets are kept on the part.
    let (at, facing) = socket(&model, "top", "leg_fl");
    assert_eq!((at, facing), (vec3(-0.51, 0.7, 0.28), vec3(0.0, -1.0, 0.0)));
}

#[test]
fn a_socket_can_be_an_exact_point_with_a_direction() {
    let model = built(&after_a(
        "[parts.sockets]\nhook = { at = [0.5, 0.25, 0], facing = \"right\" }\nspike = { at = [0, 0.5, 0], facing = [0, 3, 4] }\n[[parts]]\nname = \"b\"\nshape = \"box\"\nsize = [0.2, 0.2, 0.2]\nattach = \"left\"\nto = \"a.hook\"",
    ));
    assert_eq!(bounds(&model, "b").center().tidied(), vec3(0.6, 0.25, 0.0));
    // A direction is made 1 long.
    assert_eq!(socket(&model, "a", "spike").1, vec3(0.0, 0.6, 0.8));
}

// ---- step 4: nesting ---------------------------------------------------------

const ROBOT: &str = r#"
version = 0
name = "robot"

[define.arm]
[define.arm.sockets]
shoulder = { on = "right", shift = [0, 0.7, 0] }
grip = { from = "hand.bottom" }

[[define.arm.parts]]
name = "upper"
shape = "box"
size = [0.16, 0.6, 0.16]

[[define.arm.parts]]
name = "hand"
shape = "sphere"
radius = 0.1
attach = "top"
to = "upper.bottom"

[[parts]]
name = "body"
shape = "box"
size = [0.7, 0.8, 0.4]

[parts.sockets]
left_arm = { on = "left", shift = [0, 0.6, 0] }
right_arm = { on = "right", shift = [0, 0.6, 0] }

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
"#;

#[test]
fn an_assembly_is_a_part_like_any_other() {
    let model = built(ROBOT);
    // The arms hang beside the body, touching it, mirror images of each
    // other.
    let body = bounds(&model, "body");
    let left = bounds(&model, "left_arm");
    let right = bounds(&model, "right_arm");
    // The shoulder socket is on the upper arm's own surface (since Phase
    // 2: before, it was on the box around arm and hand, and the upper
    // arm stopped short of the body by the hand's extra width).
    assert_eq!(bounds(&model, "left_arm/upper").max.x, body.min.x);
    assert_eq!(bounds(&model, "right_arm/upper").min.x, body.max.x);
    assert_eq!(left.min.x, -right.max.x);
    assert_eq!((left.min.y, left.max.y), (right.min.y, right.max.y));
    assert!(part(&model, "right_arm").mirrored);
    assert!(!part(&model, "left_arm").mirrored);

    // Parts inside parts have paths, and are kept as a tree.
    let left_arm = part(&model, "left_arm");
    let inner: Vec<&str> = left_arm
        .parts
        .iter()
        .map(|part| part.path.as_str())
        .collect();
    assert_eq!(inner, ["left_arm/upper", "left_arm/hand"]);
    assert_eq!(left_arm.kind.label(), "arm (assembly)");
    // The hand hangs under the upper arm.
    assert_eq!(
        bounds(&model, "left_arm/hand").max.y,
        bounds(&model, "left_arm/upper").min.y
    );

    // An assembly's faces are its parts' faces, in order.
    let upper = part(&model, "left_arm/upper");
    let hand = part(&model, "left_arm/hand");
    assert_eq!(left_arm.faces.start, upper.faces.start);
    assert_eq!(upper.faces.end, hand.faces.start);
    assert_eq!(left_arm.faces.end, hand.faces.end);

    // The leaves are the parts with faces of their own.
    let leaves: Vec<&str> = model
        .leaves()
        .iter()
        .map(|part| part.path.as_str())
        .collect();
    assert_eq!(
        leaves,
        [
            "body",
            "left_arm/upper",
            "left_arm/hand",
            "right_arm/upper",
            "right_arm/hand"
        ]
    );
    // Together they own every face exactly once.
    let mut next = 0;
    for leaf in model.leaves() {
        assert_eq!(leaf.faces.start, next);
        next = leaf.faces.end;
    }
    assert_eq!(next, model.faces.len());
}

#[test]
fn an_assembly_can_pass_on_a_socket_of_an_inner_part() {
    let model = built(ROBOT);
    // `grip` is the hand's underside, wherever the arm ended up.
    let (grip, facing) = socket(&model, "left_arm", "grip");
    let hand = bounds(&model, "left_arm/hand");
    assert_eq!(
        grip,
        vec3(hand.center().x, hand.min.y, hand.center().z).tidied()
    );
    assert_eq!(facing, vec3(0.0, -1.0, 0.0));
    // And something can be attached to it.
    let with_tool = format!(
        "{ROBOT}\n[[parts]]\nname = \"tool\"\nshape = \"box\"\nsize = [0.05, 0.3, 0.05]\nattach = \"top\"\nto = \"left_arm.grip\"\n"
    );
    let model = built(&with_tool);
    assert_eq!(
        bounds(&model, "tool").max.y,
        bounds(&model, "left_arm/hand").min.y
    );
}

#[test]
fn the_part_that_uses_a_definition_can_add_sockets() {
    let model = built(
        "version = 0\norigin = \"post.bottom\"\n[define.post]\nshape = \"box\"\nsize = [0.2, 2, 0.2]\n[[parts]]\nname = \"post\"\nuse = \"post\"\n[parts.sockets]\nlamp = { on = \"front\", shift = [0, 0.9, 0] }\n[[parts]]\nname = \"lamp\"\nshape = \"sphere\"\nradius = 0.1\nattach = \"back\"\nto = \"post.lamp\"\n",
    );
    assert_eq!(
        bounds(&model, "lamp").center().tidied(),
        vec3(0.0, 1.9, 0.2)
    );
}

#[test]
fn parts_can_be_nested_in_place_too() {
    let json = r#"{
  "version": 0,
  "parts": [
    { "name": "base", "shape": "box", "size": [2, 0.2, 2] },
    { "name": "tower", "attach": "bottom", "to": "base.top",
      "parts": [
        { "name": "low", "shape": "box" },
        { "name": "high", "shape": "cone", "attach": "bottom", "to": "low.top" }
      ] }
  ]
}"#;
    let mut issues = Vec::new();
    let loaded = load_text(
        json,
        Syntax::Json,
        "tower.json",
        Path::new("."),
        &mut issues,
    );
    let model = assemble(&loaded, &mut issues).unwrap();
    assert!(issues.iter().all(|issue| !issue.is_error()), "{issues:#?}");
    assert_eq!(bounds(&model, "tower").min.y, 0.2);
    assert_eq!(bounds(&model, "tower/high").max.y, 2.2);
    assert_eq!(part(&model, "tower").kind.label(), "assembly");
}

#[test]
fn the_zero_point_can_be_a_socket_of_one_part() {
    let flag = "[[parts]]\nname = \"pole\"\nshape = \"cylinder\"\nradius = 0.05\nheight = 3\n[[parts]]\nname = \"flag\"\nshape = \"box\"\nsize = [1, 0.6, 0.02]\nattach = \"left\"\nto = \"pole.right\"\n";
    // By default: the middle of the underside of the whole model.
    let model = built(&format!("version = 0\n{flag}"));
    let all = model.bounds().unwrap();
    assert_eq!(all.min.y, 0.0);
    assert_eq!(all.center().x, 0.0);
    assert!(bounds(&model, "pole").center().x < 0.0);

    // With the pole's foot as the zero point, the pole stands on it.
    let model = built(&format!("version = 0\norigin = \"pole.bottom\"\n{flag}"));
    assert_eq!(bounds(&model, "pole").center().x, 0.0);
    assert_eq!(model.bounds().unwrap().min.y, 0.0);

    // Or the middle of the model.
    let model = built(&format!("version = 0\norigin = \"center\"\n{flag}"));
    assert_eq!(model.bounds().unwrap().center().tidied(), Vec3::ZERO);
}

#[test]
fn a_whole_recipe_can_be_a_part() {
    let folder = folder("recipe_part_sockets");
    std::fs::write(
        folder.join("wheel.toml"),
        "version = 0\norigin = \"center\"\n[sockets]\naxle = { on = \"left\" }\n[[parts]]\nname = \"tyre\"\nshape = \"cylinder\"\nradius = 0.3\nheight = 0.2\nrotate = [0, 0, 90]\n",
    )
    .unwrap();
    std::fs::write(
        folder.join("cart.toml"),
        "version = 0\n[[parts]]\nname = \"bed\"\nshape = \"box\"\nsize = [1, 0.2, 2]\n[[parts]]\nname = \"wheel\"\nfile = \"wheel.toml\"\nattach = \"axle\"\nto = \"bed.right\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("cart.toml"), &mut issues).unwrap();
    let model = assemble(&loaded, &mut issues).unwrap();
    assert!(issues.iter().all(|issue| !issue.is_error()), "{issues:#?}");
    // The wheel lies on its side, its axle socket on the bed's right.
    let wheel = bounds(&model, "wheel");
    assert_eq!(wheel.size().tidied(), vec3(0.2, 0.6, 0.6));
    assert_eq!(wheel.min.x, bounds(&model, "bed").max.x);
    assert_eq!(part(&model, "wheel").parts[0].path, "wheel/tyre");
    assert!(
        part(&model, "wheel")
            .kind
            .file
            .as_deref()
            .unwrap()
            .ends_with("wheel.toml")
    );
    // The model stands on its lowest point: the wheel.
    assert_eq!(model.bounds().unwrap().min.y, 0.0);
}

// ---- step 4: mistakes --------------------------------------------------------

#[test]
fn a_part_or_socket_that_is_not_there_is_named_with_the_likely_fix() {
    let issues = errors(&after_a(
        "[[parts]]\nshape = \"box\"\nattach = \"bottom\"\nto = \"aa.top\"",
    ));
    let issue = only(&issues, Code::UnknownPart);
    assert_eq!((issue.path.as_str(), issue.line), ("parts[1].to", Some(9)));
    assert_eq!(issue.message, "there is no part \"aa\" in this list");
    assert_eq!(issue.suggestion.as_deref(), Some("a"));

    let issues = errors(&after_a(
        "[parts.sockets]\nneck = { on = \"top\" }\n[[parts]]\nshape = \"box\"\nattach = \"bottom\"\nto = \"a.nek\"",
    ));
    let issue = only(&issues, Code::UnknownSocket);
    assert_eq!(issue.path, "parts[1].to");
    assert_eq!(issue.message, "the part \"a\" has no socket \"nek\"");
    assert_eq!(issue.suggestion.as_deref(), Some("neck"));
    assert_eq!(
        issue.allowed,
        Some(Allowed::OneOf {
            options: [
                "neck", "top", "bottom", "left", "right", "front", "back", "center"
            ]
            .map(String::from)
            .to_vec()
        })
    );

    let issues = errors(&after_a(
        "[[parts]]\nshape = \"box\"\nattach = \"botom\"\nto = \"a.top\"",
    ));
    let issue = only(&issues, Code::UnknownSocket);
    assert_eq!(issue.path, "parts[1].attach");
    assert_eq!(issue.suggestion.as_deref(), Some("bottom"));
}

#[test]
fn parts_cannot_hang_from_themselves_or_each_other() {
    let issues = errors(&after_a(
        "[[parts]]\nname = \"b\"\nshape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"",
    ));
    assert!(
        only(&issues, Code::Cycle)
            .message
            .contains("can't be attached to itself")
    );

    let issues = errors(
        "version = 0\n[[parts]]\nname = \"a\"\nshape = \"box\"\nattach = \"bottom\"\nto = \"b.top\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nattach = \"bottom\"\nto = \"a.top\"\n",
    );
    let rings: Vec<&Issue> = issues
        .iter()
        .filter(|issue| issue.code == Code::Cycle)
        .collect();
    assert_eq!(rings.len(), 2);
    assert!(
        rings[0].message.contains("(a > b > a)"),
        "{}",
        rings[0].message
    );
    assert!(
        rings[1].message.contains("(b > a > b)"),
        "{}",
        rings[1].message
    );
}

#[test]
fn a_use_must_name_a_defined_part() {
    let issues = errors("version = 0\n[define.leg]\nshape = \"box\"\n[[parts]]\nuse = \"legg\"\n");
    let issue = only(&issues, Code::UnknownPart);
    assert_eq!((issue.path.as_str(), issue.line), ("parts[0].use", Some(5)));
    assert_eq!(issue.suggestion.as_deref(), Some("leg"));
}

#[test]
fn a_defined_part_cannot_use_itself() {
    let issues = errors(
        "version = 0\n[define.a]\nuse = \"b\"\n[define.b]\n[[define.b.parts]]\nuse = \"a\"\n[[parts]]\nuse = \"a\"\n",
    );
    let issue = only(&issues, Code::Cycle);
    assert_eq!(
        issue.message,
        "the part \"a\" ends up using itself (a > b > a)"
    );
    assert_eq!(issue.path, "define.b.parts[0].use");
}

#[test]
fn from_needs_an_inner_part() {
    let issues = errors(&after_a(
        "[parts.sockets]\ngrip = { from = \"hand.bottom\" }",
    ));
    let issue = only(&issues, Code::UnknownPart);
    assert!(
        issue.message.contains("a single box with no parts inside"),
        "{}",
        issue.message
    );

    let issues = errors(&ROBOT.replace("from = \"hand.bottom\"", "from = \"hnd.bottom\""));
    assert_eq!(
        only(&issues, Code::UnknownPart).suggestion.as_deref(),
        Some("hand")
    );
    let issues = errors(&ROBOT.replace("from = \"hand.bottom\"", "from = \"hand.botom\""));
    assert_eq!(
        only(&issues, Code::UnknownSocket).suggestion.as_deref(),
        Some("bottom")
    );
}

#[test]
fn a_socket_name_cannot_be_given_twice() {
    let issues = errors(&format!(
        "{}\n[parts.sockets]\nshoulder = {{ on = \"top\" }}\n",
        ROBOT.trim_end()
    ));
    let issue = only(&issues, Code::DuplicateName);
    assert_eq!(issue.path, "parts[2].sockets.shoulder");
}

#[test]
fn the_zero_point_must_be_a_real_socket() {
    let issues = errors("version = 0\norigin = \"botom\"\n[[parts]]\nshape = \"box\"\n");
    let issue = only(&issues, Code::UnknownSocket);
    assert_eq!((issue.path.as_str(), issue.line), ("origin", Some(2)));
    assert_eq!(issue.suggestion.as_deref(), Some("bottom"));

    let issues = errors(
        "version = 0\norigin = \"trunk.bottom\"\n[[parts]]\nname = \"trnk\"\nshape = \"box\"\n",
    );
    assert_eq!(
        only(&issues, Code::UnknownPart).suggestion.as_deref(),
        Some("trnk")
    );
}

#[test]
fn a_part_that_could_not_be_read_is_not_complained_about_twice() {
    // `a` has a shape that doesn't exist. The part attached to it, and
    // the socket named on it, are not reported as well.
    let issues = errors(
        "version = 0\norigin = \"a.top\"\n[[parts]]\nname = \"a\"\nshape = \"blob\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nattach = \"bottom\"\nto = \"a.top\"\n",
    );
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].code, Code::UnknownShape);
}

#[test]
fn a_defined_part_never_used_is_a_warning_and_is_still_checked() {
    let (model, issues) =
        build("version = 0\n[define.wheel]\nshape = \"cylinder\"\n[[parts]]\nshape = \"box\"\n");
    assert!(model.is_some());
    let issue = only(&issues, Code::Unused);
    assert!(!issue.is_error());
    assert_eq!((issue.path.as_str(), issue.line), ("define.wheel", Some(2)));
    assert_eq!(
        issue.message,
        "the part \"wheel\" is defined but never used"
    );

    // A mistake inside it is still found.
    let (model, issues) = build(
        "version = 0\n[define.cart]\n[[define.cart.parts]]\nuse = \"weel\"\n[[parts]]\nshape = \"box\"\n",
    );
    assert!(model.is_none());
    assert_eq!(
        only(&issues, Code::UnknownPart).path,
        "define.cart.parts[0].use"
    );
}

#[test]
fn a_model_with_far_too_many_triangles_is_refused() {
    // Ten spheres, ten of those, ten of those, ten of those: 39 million
    // triangles. It stops as soon as there are too many.
    let ten = |of: &str| -> String {
        (0..10)
            .map(|_| format!("{{ use = \"{of}\" }}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let text = format!(
        "version = 0\n[define.ball]\nshape = \"sphere\"\nsegments = 64\n[define.b1]\nparts = [{}]\n[define.b2]\nparts = [{}]\n[define.b3]\nparts = [{}]\n[[parts]]\nparts = [{}]\n",
        ten("ball"),
        ten("b1"),
        ten("b2"),
        ten("b3")
    );
    let issues = errors(&text);
    let issue = only(&issues, Code::OutOfRange);
    assert!(
        issue.message.contains("more than 1000000 triangles"),
        "{}",
        issue.message
    );
}

#[test]
fn the_same_recipe_builds_the_same_model() {
    assert_eq!(built(ROBOT), built(ROBOT));
    assert_eq!(built(TABLE), built(TABLE));
}

#[test]
fn a_part_scaled_down_to_nothing_is_an_error_not_an_empty_model() {
    // A thousandth of a thousandth of a millimetre.
    let issues = errors(
        "version = 0\n[[parts]]\nname = \"speck\"\nscale = 0.001\nparts = [{ name = \"grain\", shape = \"box\", size = [0.001, 0.001, 0.001], scale = 0.001 }]\n",
    );
    let issue = only(&issues, Code::OutOfRange);
    assert!(
        issue
            .message
            .contains("the part \"speck/grain\" ends up too small to build"),
        "{}",
        issue.message
    );
    assert_eq!(issue.line, Some(5));
}

#[test]
fn faces_squashed_flat_are_left_out_with_a_warning() {
    // A sheet a millionth of a millimetre thick: its four edges have no
    // area left, its two big faces do.
    let (model, issues) = build(
        "version = 0\n[[parts]]\nname = \"sheet\"\nscale = [1, 0.001, 1]\nparts = [{ name = \"leaf\", shape = \"box\", size = [1, 0.001, 1], scale = [1, 0.001, 1] }]\n",
    );
    let model = model.expect("it still builds");
    assert_eq!(model.triangle_count(), 4);
    let warning = only(&issues, Code::Skipped);
    assert_eq!(
        warning.message,
        "4 faces of the part \"sheet/leaf\" end up with no area (it is thinner than a thousandth of a millimetre) and were left out"
    );
    // What is counted is what is written.
    assert_eq!(model.faces.len(), 2);
    assert!(model.faces.iter().all(|face| face.normal().is_some()));
}

// ---- Phase 5, step 1: the tree of joins ---------------------------------------------

/// The example `name`, built.
fn example(name: &str) -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(format!("{name}.toml"));
    duck_mold_core::tools::build(&path).unwrap().model
}

#[test]
fn every_part_knows_what_it_turns_about_and_hangs_from() {
    let robot = example("robot");
    let socket = |part_path: &str, name: &str| {
        part(&robot, part_path)
            .sockets
            .iter()
            .find(|socket| socket.name == name)
            .unwrap()
            .at
    };
    // A part that isn't joined turns about its own middle, and hangs
    // from the model.
    let body = part(&robot, "body");
    assert_eq!(body.parent, None);
    assert!((body.pivot - body.bounds.unwrap().center()).length() < 1e-9);
    // A joined part turns about the socket it is joined to, and hangs
    // from that part.
    let head = part(&robot, "head");
    assert_eq!(head.parent.as_deref(), Some("body"));
    assert_eq!(head.pivot, socket("body", "neck"));
    // What is joined to a joined part hangs from it in turn.
    assert_eq!(part(&robot, "antenna").parent.as_deref(), Some("head"));
    assert_eq!(
        part(&robot, "antenna_tip").parent.as_deref(),
        Some("antenna")
    );
    // A part made of parts is joined like any other...
    let arm = part(&robot, "left_arm");
    assert_eq!(arm.parent.as_deref(), Some("body"));
    assert_eq!(arm.pivot, socket("body", "left_arm"));
    // ...the part inside it that isn't joined hangs from it, and the
    // others from each other, by path.
    let upper = part(&robot, "left_arm/upper");
    assert_eq!(upper.parent.as_deref(), Some("left_arm"));
    let elbow = part(&robot, "left_arm/elbow");
    assert_eq!(elbow.parent.as_deref(), Some("left_arm/upper"));
    assert_eq!(elbow.pivot, socket("left_arm/upper", "bottom"));
    assert_eq!(
        part(&robot, "left_arm/hand").parent.as_deref(),
        Some("left_arm/forearm")
    );

    // Every part is in the tree once: following what it hangs from ends
    // at the model, without going round.
    for name in ["robot", "house", "snowman", "dog", "gate", "desk", "temple"] {
        let model = example(name);
        let all = model.all_parts();
        for part in &all {
            let mut above = part.parent.clone();
            let mut steps = 0;
            while let Some(path) = above {
                let next = model.part(&path).unwrap_or_else(|| {
                    panic!("{name}: {} hangs from {path}, which isn't there", part.path)
                });
                above = next.parent.clone();
                steps += 1;
                assert!(steps <= all.len(), "{name}: {} goes round", part.path);
            }
        }
    }
}

#[test]
fn a_parts_turns_are_measured_in_the_directions_of_its_list() {
    let robot = example("robot");
    let unturned = duck_mold_core::math::Mat3::IDENTITY;
    // A part of the model's own list: the model's x, y and z.
    assert_eq!(part(&robot, "head").axes, unturned);
    assert_eq!(part(&robot, "right_arm").axes, unturned);
    // A part inside the left arm: the arm's, which stands as the model
    // does.
    let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-9;
    let left = part(&robot, "left_arm/elbow").axes;
    assert!(near(left * vec3(1.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0)));
    // Inside the right arm, a mirror image: its x is the other way, so
    // the same turn bends it as a mirror image too.
    let right = part(&robot, "right_arm/elbow").axes;
    assert!(near(right * vec3(1.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0)));
    assert!(near(right * vec3(0.0, 1.0, 0.0), vec3(0.0, 1.0, 0.0)));
    assert!(near(right * vec3(0.0, 0.0, 1.0), vec3(0.0, 0.0, 1.0)));
}

#[test]
fn a_socket_passed_on_is_carried_by_the_part_it_came_from() {
    let robot = example("robot");
    // The arm's `grip` is its hand's `bottom`: when the arm bends, it
    // goes with the hand.
    assert_eq!(
        robot.socket_carrier("left_arm.grip").as_deref(),
        Some("left_arm/hand")
    );
    assert_eq!(
        robot.socket_carrier("left_arm.shoulder").as_deref(),
        Some("left_arm/upper")
    );
    // A socket of its own is carried by the part itself; the model's own
    // by nothing.
    assert_eq!(robot.socket_carrier("head.top").as_deref(), Some("head"));
    assert_eq!(
        robot.socket_carrier("left_arm.top").as_deref(),
        Some("left_arm")
    );
    assert_eq!(robot.socket_carrier("top"), None);
    // Passed on twice: a model's own socket, from a part inside a part.
    let model = built(
        "version = 0\n[sockets]\ntip = { from = \"mast.peak\" }\n[[parts]]\nname = \"mast\"\n[parts.sockets]\npeak = { from = \"flag.top\" }\n[[parts.parts]]\nname = \"pole\"\nshape = \"box\"\nsize = [0.1, 2, 0.1]\n[[parts.parts]]\nname = \"flag\"\nshape = \"box\"\nsize = [0.5, 0.3, 0.02]\nattach = \"bottom\"\nto = \"pole.top\"\n",
    );
    assert_eq!(model.socket_carrier("tip").as_deref(), Some("mast/flag"));
    assert_eq!(
        model.socket_carrier("mast.peak").as_deref(),
        Some("mast/flag")
    );
}
