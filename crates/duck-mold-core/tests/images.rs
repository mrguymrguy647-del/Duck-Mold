//! Phase 7b, step 1: images on parts.
//!
//! A part's `image`, or one face's (`images`), is a PNG file beside the
//! recipe. It lies over the face's colour, fitted to the face as you see
//! it straight on (round faces are wrapped). These tests build recipes
//! with small images written for the purpose and check where each face
//! is on its image, what the mistakes say, what `inspect` reports and
//! what goes on the sheet a `.glb` will hold.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::inspect::{images_report, inspect};
use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::load::load_text;
use duck_mold_core::math::{Vec3, vec3};
use duck_mold_core::model::{Face, Model};
use duck_mold_core::picture::{ImageSheet, write_png};
use duck_mold_core::source::Syntax;

/// A folder of its own for one test, with the images every test uses:
///
/// - `quad.png`, 4 x 4 and solid: red top-left, green top-right, blue
///   bottom-left, yellow bottom-right;
/// - `mouth.png`, 4 x 2: see-through but for a black bottom row;
/// - `wide.png`, 8 x 2 and solid white.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("images")
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("textures")).unwrap();
    let quad: Vec<[u8; 4]> = (0..16)
        .map(|at| match (at % 4 < 2, at / 4 < 2) {
            (true, true) => [255, 0, 0, 255],
            (false, true) => [0, 255, 0, 255],
            (true, false) => [0, 0, 255, 255],
            (false, false) => [255, 255, 0, 255],
        })
        .collect();
    std::fs::write(
        folder.join("textures/quad.png"),
        write_png(4, 4, &quad, false),
    )
    .unwrap();
    let mouth: Vec<[u8; 4]> = (0..8)
        .map(|at| {
            if at >= 4 {
                [0, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect();
    std::fs::write(
        folder.join("textures/mouth.png"),
        write_png(4, 2, &mouth, false),
    )
    .unwrap();
    std::fs::write(
        folder.join("textures/wide.png"),
        write_png(8, 2, &[[255, 255, 255, 255]; 16], false),
    )
    .unwrap();
    folder
}

const TOP: &str = r##"
version = 0
name = "test"
[palette]
steel = "#8A939B"
wood = "#B07A3F"
"##;

fn build_in(folder: &Path, parts: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let text = format!("{TOP}{parts}");
    let loaded = load_text(&text, Syntax::Toml, "test.toml", folder, &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, issues)
}

fn built(name: &str, parts: &str) -> Model {
    let (model, issues) = build_in(&folder(name), parts);
    let errors: Vec<_> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.unwrap()
}

fn errors(name: &str, parts: &str) -> Vec<Issue> {
    let (model, issues) = build_in(&folder(name), parts);
    assert!(model.is_none());
    issues.into_iter().filter(Issue::is_error).collect()
}

/// The faces of a part that go by a name.
fn faces<'a>(model: &'a Model, part: &str, name: &str) -> Vec<&'a Face> {
    let part = model.part(part).unwrap();
    part.faces_named(name)
        .into_iter()
        .map(|face| &model.faces[face])
        .collect()
}

/// The faces of the ring round an inset face called `middle`.
fn ring<'a>(model: &'a Model, part: &str, middle: &str) -> Vec<&'a Face> {
    ["top", "bottom", "left", "right"]
        .into_iter()
        .flat_map(|side| faces(model, part, &format!("{middle}_rim_{side}")))
        .collect()
}

/// The image file on a face, if it has one.
fn file_on<'a>(model: &'a Model, face: &Face) -> Option<&'a str> {
    face.image
        .as_ref()
        .map(|image| model.pictures[image.picture].file.as_str())
}

/// Where on its image the corner of a face nearest to `point` is.
fn at(face: &Face, point: Vec3) -> [f64; 2] {
    let nearest = face
        .corners
        .iter()
        .enumerate()
        .min_by(|a, b| (*a.1 - point).length().total_cmp(&(*b.1 - point).length()))
        .unwrap()
        .0;
    face.image.as_ref().unwrap().at[nearest]
}

/// How much of its image some faces cover: from and to, across and
/// down.
fn covered(faces: &[&Face]) -> [f64; 4] {
    let all = || {
        faces
            .iter()
            .flat_map(|face| face.image.as_ref().unwrap().at.iter().copied())
    };
    [
        all().map(|at| at[0]).fold(f64::INFINITY, f64::min),
        all().map(|at| at[0]).fold(f64::NEG_INFINITY, f64::max),
        all().map(|at| at[1]).fold(f64::INFINITY, f64::min),
        all().map(|at| at[1]).fold(f64::NEG_INFINITY, f64::max),
    ]
}

// ---- in the recipe ---------------------------------------------------------

#[test]
fn a_recipe_without_images_has_none() {
    let model = built(
        "none",
        r#"
[[parts]]
name = "box"
shape = "box"
color = "wood"
"#,
    );
    assert!(model.pictures.is_empty());
    assert!(model.faces.iter().all(|face| face.image.is_none()));
    assert!(ImageSheet::of(&model).unwrap().is_none());
    assert!(images_report(&model).is_none());
}

#[test]
fn an_image_on_one_face_is_on_that_face_only_and_keeps_its_colour() {
    let model = built(
        "one_face",
        r#"
[[parts]]
name = "head"
shape = "box"
size = [0.4, 0.2, 0.4]
color = "steel"
images = { front = "textures/mouth.png" }
"#,
    );
    assert_eq!(model.pictures.len(), 1);
    assert_eq!(model.pictures[0].file, "textures/mouth.png");
    let front = faces(&model, "head", "front");
    assert_eq!(file_on(&model, front[0]), Some("textures/mouth.png"));
    // The colour is still the head's: it shows where the image is
    // see-through.
    assert_eq!(front[0].color, Some(0));
    for name in ["top", "bottom", "left", "right", "back"] {
        assert_eq!(
            file_on(&model, faces(&model, "head", name)[0]),
            None,
            "{name}"
        );
    }
    // Upright, seen from the front: the model's right (+x) on the
    // viewer's left. The head stands on the ground: its top is at 0.2.
    assert_eq!(at(front[0], vec3(0.2, 0.2, 0.2)), [0.0, 0.0]);
    assert_eq!(at(front[0], vec3(-0.2, 0.0, 0.2)), [1.0, 1.0]);
}

#[test]
fn a_parts_image_is_on_every_face_and_a_faces_own_wins() {
    let model = built(
        "whole",
        r#"
[[parts]]
name = "crate"
shape = "box"
color = "wood"
image = "textures/quad.png"
images = { top = "textures/wide.png", bottom = "none" }
"#,
    );
    for name in ["left", "right", "front", "back"] {
        let face = faces(&model, "crate", name)[0];
        assert_eq!(file_on(&model, face), Some("textures/quad.png"), "{name}");
        assert_eq!(covered(&[face]), [0.0, 1.0, 0.0, 1.0], "{name}");
    }
    assert_eq!(
        file_on(&model, faces(&model, "crate", "top")[0]),
        Some("textures/wide.png")
    );
    assert_eq!(file_on(&model, faces(&model, "crate", "bottom")[0]), None);
}

#[test]
fn an_assemblys_image_is_for_the_parts_inside_that_have_none() {
    let model = built(
        "assembly",
        r#"
[[parts]]
name = "arm"
color = "steel"
image = "textures/quad.png"
parts = [
  { name = "upper", shape = "box", size = [0.2, 0.4, 0.2] },
  { name = "hand", shape = "box", size = [0.2, 0.2, 0.2], attach = "top", to = "upper.bottom", image = "textures/wide.png" },
  { name = "nail", shape = "box", size = [0.05, 0.05, 0.05], attach = "top", to = "hand.bottom", image = "none" },
]
"#,
    );
    let on = |part: &str| file_on(&model, faces(&model, part, "front")[0]);
    assert_eq!(on("arm/upper"), Some("textures/quad.png"));
    assert_eq!(on("arm/hand"), Some("textures/wide.png"));
    assert_eq!(on("arm/nail"), None);
}

#[test]
fn a_definitions_image_beats_the_one_of_the_part_that_uses_it() {
    // As with colour: the nearest setting wins, and the definition is
    // nearer the shape than the part that uses it.
    let model = built(
        "define",
        r#"
[define.eye]
shape = "box"
size = [0.1, 0.1, 0.1]
color = "steel"
images = { front = "textures/mouth.png" }

[[parts]]
name = "eye"
use = "eye"
image = "textures/quad.png"
images = { front = "textures/wide.png" }
"#,
    );
    let on = |name: &str| file_on(&model, faces(&model, "eye", name)[0]);
    assert_eq!(on("front"), Some("textures/mouth.png"));
    assert_eq!(on("top"), Some("textures/quad.png"));
}

#[test]
fn a_mirrored_part_shows_the_mirror_image() {
    let model = built(
        "mirror",
        r#"
[[parts]]
name = "plain"
shape = "box"
color = "steel"
images = { front = "textures/quad.png" }

[[parts]]
name = "flipped"
shape = "box"
color = "steel"
move = [3, 0, 0]
mirror = "x"
images = { front = "textures/quad.png" }
"#,
    );
    let plain = faces(&model, "plain", "front")[0];
    let flipped = faces(&model, "flipped", "front")[0];
    // The top corner of a face on the model's right (+x), or its left.
    let top_corner = |face: &Face, right: bool| {
        let sign = if right { 1.0 } else { -1.0 };
        let far = face
            .corners
            .iter()
            .map(|corner| corner.x * sign)
            .fold(f64::NEG_INFINITY, f64::max);
        at(face, vec3(far * sign, 1.0, 0.5))
    };
    // The image's left edge is on the model's right of the plain part,
    // and on the model's left of the flipped one.
    assert_eq!(top_corner(plain, true), [0.0, 0.0]);
    assert_eq!(top_corner(plain, false), [1.0, 0.0]);
    assert_eq!(top_corner(flipped, false), [0.0, 0.0]);
    assert_eq!(top_corner(flipped, true), [1.0, 0.0]);
    // Both still face outwards.
    assert!(plain.normal().unwrap().z > 0.99 && flipped.normal().unwrap().z > 0.99);
}

#[test]
fn round_shapes_are_wrapped() {
    let model = built(
        "round",
        r#"
[[parts]]
name = "can"
shape = "cylinder"
radius = 0.5
height = 1
segments = 8
color = "steel"
image = "textures/quad.png"
"#,
    );
    let side = faces(&model, "can", "side");
    assert_eq!(side.len(), 8);
    // Round once: from one end of the image to the other, a face at the
    // back reaching a little past an end rather than jumping across.
    let [from, to, top, bottom] = covered(&side);
    assert!((to - from - 1.0).abs() < 1e-9, "{from} {to}");
    assert_eq!([top, bottom], [0.0, 1.0]);
    // The round top shows the middle of the image.
    let top = faces(&model, "can", "top");
    let [from, to, up, down] = covered(&top);
    assert!(from >= 0.0 && to <= 1.0 && up >= 0.0 && down <= 1.0);
    assert!(to - from > 0.9 && down - up > 0.9);
}

// ---- shaped boxes ----------------------------------------------------------

#[test]
fn a_pulled_face_keeps_its_image_and_the_new_walls_are_plain() {
    let model = built(
        "pull",
        r#"
[[parts]]
name = "post"
shape = "box"
color = "wood"
images = { top = "textures/quad.png" }
changes = [ { pull = "top", by = 0.5, name = "neck" } ]
"#,
    );
    let top = faces(&model, "post", "top");
    assert_eq!(file_on(&model, top[0]), Some("textures/quad.png"));
    assert_eq!(covered(&top), [0.0, 1.0, 0.0, 1.0]);
    // It went up with the face.
    assert!(
        top[0]
            .corners
            .iter()
            .all(|corner| (corner.y - 1.5).abs() < 1e-9)
    );
    for wall in faces(&model, "post", "neck") {
        assert_eq!(file_on(&model, wall), None);
    }
}

#[test]
fn the_walls_of_a_pull_show_the_parts_own_image() {
    let model = built(
        "pull_whole",
        r#"
[[parts]]
name = "post"
shape = "box"
color = "wood"
image = "textures/quad.png"
changes = [ { inset = "top", by = 0.25, name = "middle" }, { pull = "middle", by = 0.5, name = "neck" } ]
"#,
    );
    let walls = faces(&model, "post", "neck_front");
    assert_eq!(file_on(&model, walls[0]), Some("textures/quad.png"));
    // The part's image is fitted to the whole front of the part, the
    // wall included: the box's own front is the lower two thirds.
    let front = faces(&model, "post", "front");
    let [.., top, bottom] = covered(&front);
    assert!(
        (top - 1.0 / 3.0).abs() < 1e-9 && bottom == 1.0,
        "{top} {bottom}"
    );
    let [from, to, top, bottom] = covered(&walls);
    assert_eq!([from, to], [0.25, 0.75]);
    assert!(top == 0.0 && (bottom - 1.0 / 3.0).abs() < 1e-9);
}

#[test]
fn an_inset_leaves_the_image_in_place_round_a_hole() {
    let model = built(
        "inset",
        r#"
[[parts]]
name = "wall"
shape = "box"
size = [2, 2, 0.2]
color = "wood"
images = { front = "textures/quad.png" }
changes = [ { inset = "front", by = 0.5, name = "window" } ]
"#,
    );
    // `front` now stands for the ring: still the whole image, in place.
    let ring = ring(&model, "wall", "window");
    assert_eq!(ring.len(), 4);
    assert!(
        ring.iter()
            .all(|face| file_on(&model, face) == Some("textures/quad.png"))
    );
    assert_eq!(covered(&ring), [0.0, 1.0, 0.0, 1.0]);
    // The ring's inner corners are a quarter in: where they were.
    let inner = at(ring[0], vec3(0.5, 1.5, 0.1));
    assert_eq!(inner, [0.25, 0.25]);
    // The new face in the middle is a face of its own.
    let window = faces(&model, "wall", "window");
    assert_eq!(file_on(&model, window[0]), None);
}

#[test]
fn a_new_face_takes_a_whole_image_of_its_own() {
    let model = built(
        "inset_own",
        r#"
[[parts]]
name = "wall"
shape = "box"
size = [2, 2, 0.2]
color = "wood"
images = { front = "textures/quad.png", window = "textures/wide.png" }
changes = [ { inset = "front", by = 0.5, name = "window" } ]
"#,
    );
    let window = faces(&model, "wall", "window");
    assert_eq!(file_on(&model, window[0]), Some("textures/wide.png"));
    assert_eq!(covered(&window), [0.0, 1.0, 0.0, 1.0]);
    // And the ring's image hasn't moved.
    assert_eq!(
        covered(&ring(&model, "wall", "window")),
        [0.0, 1.0, 0.0, 1.0]
    );
}

#[test]
fn a_cut_leaves_a_faces_image_on_the_piece_that_kept_the_name() {
    let model = built(
        "cut",
        r#"
[[parts]]
name = "head"
shape = "box"
color = "steel"
images = { front = "textures/quad.png" }
changes = [ { cut = "left", at = 0.5 } ]
"#,
    );
    let front = faces(&model, "head", "front");
    assert_eq!(covered(&front), [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(
        file_on(&model, faces(&model, "head", "front_left")[0]),
        None
    );
}

#[test]
fn the_parts_own_image_carries_on_across_a_cut() {
    let model = built(
        "cut_whole",
        r#"
[[parts]]
name = "crate"
shape = "box"
color = "wood"
image = "textures/quad.png"
changes = [ { cut = "left", at = 0.5 } ]
"#,
    );
    // Seen from the front, the model's left is on the right of the
    // image: the slice is its last quarter.
    assert_eq!(
        covered(&faces(&model, "crate", "front")),
        [0.0, 0.75, 0.0, 1.0]
    );
    assert_eq!(
        covered(&faces(&model, "crate", "front_left")),
        [0.75, 1.0, 0.0, 1.0]
    );
}

#[test]
fn a_face_that_is_tilted_keeps_its_image_and_stretches_it() {
    let model = built(
        "turn",
        r#"
[[parts]]
name = "lid"
shape = "box"
color = "wood"
images = { top = "textures/quad.png" }
changes = [ { move = "top+front", by = [0, 0.3, 0] } ]
"#,
    );
    let top = faces(&model, "lid", "top");
    assert_eq!(covered(&top), [0.0, 1.0, 0.0, 1.0]);
}

// ---- far-away versions -----------------------------------------------------

#[test]
fn far_versions_keep_their_images() {
    let folder = folder("far");
    let text = format!(
        "{}{}",
        TOP.replace("version = 0", "version = 0\nlods = \"auto\""),
        r#"

[[parts]]
name = "can"
shape = "cylinder"
radius = 0.5
height = 1
segments = 24
color = "steel"
images = { side = "textures/quad.png" }

[[parts]]
name = "steps"
shape = "stairs"
size = [1, 1, 1]
color = "wood"
move = [2, 0.5, 0]
colors = { top = "steel" }
images = { front = "textures/wide.png" }
"#
    );
    let mut issues = Vec::new();
    let loaded = load_text(&text, Syntax::Toml, "test.toml", &folder, &mut issues);
    let model = assemble(&loaded, &mut issues).expect("the model builds");
    assert_eq!(model.far.len(), 2, "both far versions are built");
    for far in &model.far {
        let side = faces(&far.model, "can", "side");
        assert!(side.len() < 24);
        assert!(
            side.iter()
                .all(|face| file_on(&far.model, face) == Some("textures/quad.png"))
        );
        let [from, to, ..] = covered(&side);
        assert!((to - from - 1.0).abs() < 1e-9);
        assert_eq!(far.model.pictures, model.pictures);
    }
    // In Far 2 the stairs are one slope, which has no `front`: its
    // image is simply not there.
    let slope = far_part_faces(&model.far[1].model, "steps");
    assert!(slope.iter().all(|face| face.image.is_none()));
    let risers = far_part_faces(&model.far[0].model, "steps");
    assert!(risers.iter().any(|face| face.image.is_some()));
}

fn far_part_faces<'a>(model: &'a Model, part: &str) -> Vec<&'a Face> {
    model.faces[model.part(part).unwrap().faces.clone()]
        .iter()
        .collect()
}

// ---- mistakes --------------------------------------------------------------

#[test]
fn every_mistake_is_a_sentence_and_nothing_is_built() {
    let said = |parts: &str| -> (Code, String) {
        let found = errors("mistakes", parts);
        assert_eq!(found.len(), 1, "{found:#?}");
        (found[0].code, found[0].to_string())
    };
    let part = |line: &str| {
        format!("[[parts]]\nname = \"a\"\nshape = \"box\"\ncolor = \"steel\"\n{line}\n")
    };

    let (code, words) = said(&part(r#"image = "textures/missing.png""#));
    assert_eq!(code, Code::FileNotFound);
    assert!(
        words.contains("there is no file \"textures/missing.png\" beside test.toml"),
        "{words}"
    );
    assert!(words.contains("parts[0].image"), "{words}");

    std::fs::write(
        folder("mistakes").join("textures/photo.png"),
        [0xFF, 0xD8, 0xFF],
    )
    .unwrap();
    let (model, issues) = build_in(
        &Path::new(env!("CARGO_TARGET_TMPDIR")).join("images/mistakes"),
        &part(r#"images = { front = "textures/photo.png" }"#),
    );
    assert!(model.is_none());
    let words = issues[0].to_string();
    assert_eq!(issues[0].code, Code::BadImage);
    assert!(
        words.contains(
            "\"textures/photo.png\" isn't a PNG picture (it is a JPEG). Save it as a PNG"
        ),
        "{words}"
    );

    let (code, words) = said(&part(r#"images = { frnt = "textures/quad.png" }"#));
    assert_eq!(code, Code::UnknownFace);
    assert!(
        words.contains("no face called \"frnt\"") && words.contains("front"),
        "{words}"
    );

    let (code, words) = said(&part(
        "bevel = 0.05\nimages = { bevel = \"textures/quad.png\" }",
    ));
    assert_eq!(code, Code::Unsupported);
    assert!(
        words.contains("can't take an image: they keep their colour"),
        "{words}"
    );

    let (_, words) = said(&part(r#"image = "C:/pictures/face.png""#));
    assert!(words.contains("starts from the top of the disk"), "{words}");
    let (_, words) = said(&part(r#"image = "/home/me/face.png""#));
    assert!(words.contains("starts from the top of the disk"), "{words}");

    let (_, words) = said(&part(r#"image = "textures/face.jpg""#));
    assert!(words.contains("isn't a PNG file"), "{words}");

    let (code, words) = said(
        "[[parts]]\nname = \"a\"\ncolor = \"steel\"\nimages = { front = \"textures/quad.png\" }\nparts = [ { shape = \"box\" } ]\n",
    );
    assert_eq!(code, Code::UnknownFace);
    assert!(
        words.contains("put \"images\" on the parts inside it"),
        "{words}"
    );
}

#[test]
fn a_bevelled_boxs_cut_edges_stay_plain_under_the_parts_image() {
    let model = built(
        "bevel",
        r#"
[[parts]]
name = "crate"
shape = "box"
color = "wood"
bevel = 0.05
image = "textures/quad.png"
"#,
    );
    assert!(
        faces(&model, "crate", "bevel")
            .iter()
            .all(|face| face.image.is_none())
    );
    assert!(faces(&model, "crate", "front")[0].image.is_some());
}

#[test]
fn an_image_too_big_to_use_says_its_size() {
    let folder = folder("too_big");
    std::fs::write(
        folder.join("textures/big.png"),
        write_png(1025, 2, &vec![[9, 9, 9, 255]; 2050], true),
    )
    .unwrap();
    let (model, issues) = build_in(
        &folder,
        "[[parts]]\nshape = \"box\"\ncolor = \"steel\"\nimage = \"textures/big.png\"\n",
    );
    assert!(model.is_none());
    assert!(
        issues[0].to_string().contains(
            "is 1025 x 2 pixels, and an image can be 1024 x 1024 at most. Make it smaller"
        ),
        "{}",
        issues[0]
    );
}

// ---- the sheet -------------------------------------------------------------

#[test]
fn the_sheet_holds_each_image_once_for_each_colour_under_it() {
    let model = built(
        "sheet",
        r#"
[[parts]]
name = "a"
shape = "box"
color = "steel"
images = { front = "textures/mouth.png", back = "textures/quad.png" }

[[parts]]
name = "b"
shape = "box"
color = "wood"
move = [2, 0, 0]
images = { front = "textures/mouth.png", back = "textures/quad.png" }
"#,
    );
    let sheet = ImageSheet::of(&model).unwrap().unwrap();
    // The see-through mouth over steel and over wood, and the solid
    // quad once: no colour shows through it.
    assert_eq!(sheet.cells.len(), 3);
    assert!(sheet.width.is_power_of_two() && sheet.height.is_power_of_two());
    assert!(sheet.pixels.iter().all(|pixel| pixel[3] == 255));
    assert_eq!(
        ImageSheet::size(&model).unwrap(),
        Some((sheet.width, sheet.height))
    );

    let pixel_at = |part: &str, name: &str, place: [f64; 2]| {
        let number = model.part(part).unwrap().faces_named(name)[0];
        let cell = sheet.cell_of[number].unwrap();
        let [across, down] = sheet.place(&model, cell, place);
        let x = (across * f64::from(sheet.width)).floor() as u32;
        let y = (down * f64::from(sheet.height)).floor() as u32;
        sheet.pixels[(y * sheet.width + x) as usize]
    };
    // The top of the mouth image is see-through: the colour shows.
    assert_eq!(pixel_at("a", "front", [0.5, 0.25]), [0x8A, 0x93, 0x9B, 255]);
    assert_eq!(pixel_at("b", "front", [0.5, 0.25]), [0xB0, 0x7A, 0x3F, 255]);
    // Its bottom row is black on both.
    assert_eq!(pixel_at("a", "front", [0.5, 0.75]), [0, 0, 0, 255]);
    assert_eq!(pixel_at("b", "front", [0.5, 0.75]), [0, 0, 0, 255]);
    // The quad's corners.
    assert_eq!(pixel_at("a", "back", [0.1, 0.1]), [255, 0, 0, 255]);
    assert_eq!(pixel_at("b", "back", [0.9, 0.9]), [255, 255, 0, 255]);
    // Just off the image's edge is its own edge again, not a neighbour.
    assert_eq!(pixel_at("a", "back", [-0.1, 0.1]), [255, 0, 0, 255]);
    assert_eq!(pixel_at("a", "back", [1.1, 1.1]), [255, 255, 0, 255]);

    // The same model always gives the same sheet, and the same bytes.
    let again = ImageSheet::of(&model).unwrap().unwrap();
    assert_eq!(sheet, again);
    assert_eq!(sheet.png(), again.png());
}

#[test]
fn a_face_across_the_two_ends_of_a_wrapped_image_shows_it_carrying_on() {
    let model = built(
        "seam",
        r#"
[[parts]]
name = "can"
shape = "cylinder"
radius = 0.5
height = 1
segments = 8
color = "steel"
images = { side = "textures/quad.png" }
"#,
    );
    let sheet = ImageSheet::of(&model).unwrap().unwrap();
    // The face at the back lies across the two ends of the image. Half
    // of it is the image's last pixels and half its first, in one piece
    // on the sheet.
    let part = model.part("can").unwrap();
    let back = part
        .faces_named("side")
        .into_iter()
        .find(|&face| {
            let on = &model.faces[face].image.as_ref().unwrap().at;
            on.iter().any(|at| at[0] < 0.0) || on.iter().any(|at| at[0] > 1.0)
        })
        .expect("a face across the ends");
    let cell = sheet.cell_of[back].unwrap();
    let on = &model.faces[back].image.as_ref().unwrap().at;
    let low = on.iter().map(|at| at[0]).fold(f64::INFINITY, f64::min);
    let high = on.iter().map(|at| at[0]).fold(f64::NEG_INFINITY, f64::max);
    let pixel = |across: f64| {
        let [x, y] = sheet.place(&model, cell, [across, 0.1]);
        let x = (x * f64::from(sheet.width)).floor() as u32;
        let y = (y * f64::from(sheet.height)).floor() as u32;
        sheet.pixels[(y * sheet.width + x) as usize]
    };
    // Top row of the quad: red on its left half, green on its right.
    let (first, last) = (pixel(low + 0.01), pixel(high - 0.01));
    assert_ne!(first, last);
    assert!([first, last].contains(&[255, 0, 0, 255]));
    assert!([first, last].contains(&[0, 255, 0, 255]));
}

// ---- inspect ---------------------------------------------------------------

const ROBOT: &str = r#"
[[parts]]
name = "head"
shape = "box"
size = [0.4, 0.2, 0.4]
color = "steel"
images = { front = "textures/mouth.png" }

[[parts]]
name = "crate"
shape = "box"
size = [0.8, 0.8, 0.8]
color = "wood"
move = [2, 0, 0]
image = "textures/quad.png"
"#;

#[test]
fn inspect_lists_the_images_where_they_are_and_how_big_their_pixels_are() {
    let model = built("inspect", ROBOT);
    let report = inspect(&model, "test.toml", Vec::new());
    let text = report.to_text();
    assert!(
        text.contains("  images       2 files on 7 faces; 16 x 16 pixels together in the .glb\n"),
        "{text}"
    );
    assert!(
        text.contains("    textures/mouth.png   4 x 2   head: front      10 pixels a metre\n"),
        "{text}"
    );
    assert!(
        text.contains("    textures/quad.png    4 x 4   crate: 6 faces   5 pixels a metre\n"),
        "{text}"
    );
    let json = serde_json::to_value(&report).unwrap();
    let images = &json["images"];
    assert_eq!(images["sheet"], serde_json::json!([16, 16]));
    assert_eq!(images["files"][0]["file"], "textures/mouth.png");
    assert_eq!(images["files"][0]["solid"], false);
    assert_eq!(
        images["files"][0]["on"][0],
        serde_json::json!({ "part": "head", "face": "front", "metres": [0.4, 0.2], "pixels_per_metre": 10.0 })
    );
    assert_eq!(images["files"][1]["on"].as_array().unwrap().len(), 6);

    // A model without images says nothing about them.
    let plain = built(
        "inspect_plain",
        "[[parts]]\nname = \"a\"\nshape = \"box\"\ncolor = \"steel\"\n",
    );
    let report = inspect(&plain, "test.toml", Vec::new());
    assert!(!report.to_text().contains("images"));
    assert!(
        serde_json::to_value(&report)
            .unwrap()
            .get("images")
            .is_none()
    );
}

#[test]
fn pixels_of_very_different_sizes_are_a_warning_that_names_the_two() {
    let model = built("mixed", ROBOT);
    let report = inspect(&model, "test.toml", Vec::new());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| warning.code != Code::MixedPixels),
        "twice is allowed: {:#?}",
        report.warnings
    );

    // The same head, on a crate four times its size with the same image.
    let model = built(
        "mixed_more",
        &ROBOT.replace("size = [0.8, 0.8, 0.8]", "size = [1.6, 1.6, 1.6]"),
    );
    let report = inspect(&model, "test.toml", Vec::new());
    let mixed: Vec<_> = report
        .warnings
        .iter()
        .filter(|warning| warning.code == Code::MixedPixels)
        .collect();
    assert_eq!(mixed.len(), 1);
    assert_eq!(
        mixed[0].to_string(),
        "warning: test.toml: mixed pixel sizes: \"textures/mouth.png\" on head has 10 pixels a metre and \"textures/quad.png\" on crate has 3. Pixels look best all one size: make one image smaller or the other bigger."
    );
}

#[test]
fn a_recipe_used_as_a_part_brings_its_images_from_its_own_folder() {
    let folder = folder("nested");
    std::fs::create_dir_all(folder.join("parts/textures")).unwrap();
    std::fs::copy(
        folder.join("textures/wide.png"),
        folder.join("parts/textures/label.png"),
    )
    .unwrap();
    std::fs::write(
        folder.join("parts/sign.toml"),
        "version = 0\n[palette]\nsteel = \"#8A939B\"\n[[parts]]\nname = \"board\"\nshape = \"box\"\ncolor = \"steel\"\nimages = { front = \"textures/label.png\" }\n",
    )
    .unwrap();
    let (model, issues) = build_in(
        &folder,
        "[[parts]]\nname = \"sign\"\nfile = \"parts/sign.toml\"\n\n[[parts]]\nname = \"box\"\nshape = \"box\"\ncolor = \"wood\"\nmove = [2, 0, 0]\nimage = \"textures/quad.png\"\n",
    );
    assert!(issues.iter().all(|issue| !issue.is_error()), "{issues:#?}");
    let model = model.unwrap();
    let files: Vec<&str> = model
        .pictures
        .iter()
        .map(|picture| picture.file.as_str())
        .collect();
    assert_eq!(files, ["parts/textures/label.png", "textures/quad.png"]);
    assert_eq!(
        file_on(&model, faces(&model, "sign/board", "front")[0]),
        Some("parts/textures/label.png")
    );
}
