//! Phase 7b, step 2: a model's images in its `.glb`.
//!
//! All of a model's images go on one sheet, a PNG inside the file, and
//! every corner says where it is on it. The reader written for Duck
//! Forge (version 9) reads both back; these tests check it against
//! Duck Mold's own model, triangle by triangle.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::glb::{read_glb, write_glb};
use duck_mold_core::issue::Issue;
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::picture::{ImageSheet, read_png_up_to, write_png};
use duck_mold_core::source::Syntax;

/// The reader written for Duck Forge, exactly as it will be copied there.
#[path = "../../../docs/forge/duck_mold_glb.rs"]
#[allow(dead_code)]
mod forge;

/// A folder for one test, with `quad.png` (4 x 4, solid, a colour in
/// each corner) and `mouth.png` (4 x 2, see-through but for a black
/// bottom row).
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("glb_images")
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let quad: Vec<[u8; 4]> = (0..16)
        .map(|at| match (at % 4 < 2, at / 4 < 2) {
            (true, true) => [255, 0, 0, 255],
            (false, true) => [0, 255, 0, 255],
            (true, false) => [0, 0, 255, 255],
            (false, false) => [255, 255, 0, 255],
        })
        .collect();
    std::fs::write(folder.join("quad.png"), write_png(4, 4, &quad, false)).unwrap();
    let mouth: Vec<[u8; 4]> = (0..8)
        .map(|at| {
            if at >= 4 {
                [0, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect();
    std::fs::write(folder.join("mouth.png"), write_png(4, 2, &mouth, false)).unwrap();
    folder
}

fn built(name: &str, text: &str) -> Model {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", &folder(name), &mut issues);
    let model = assemble(&loaded, &mut issues);
    let errors: Vec<&Issue> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.unwrap()
}

/// A robot of sorts: a head with a mouth (see-through over steel), a
/// can with a label wrapped round it, a mirrored hand with a solid
/// image, and a plain foot. With far-away versions.
const FIGURE: &str = r##"
version = 0
name = "figure"
lods = "auto"

[palette]
steel = "#8A939B"
wood = "#B07A3F"

[[parts]]
name = "can"
shape = "cylinder"
radius = 0.3
height = 0.8
segments = 16
color = "wood"
images = { side = "quad.png" }

[[parts]]
name = "head"
shape = "box"
size = [0.4, 0.3, 0.4]
color = "steel"
attach = "bottom"
to = "can.top"
images = { front = "mouth.png" }

[[parts]]
name = "hand"
shape = "box"
size = [0.2, 0.2, 0.2]
color = "steel"
mirror = "x"
attach = "left"
to = "can.right"
image = "quad.png"

[[parts]]
name = "foot"
shape = "box"
size = [0.3, 0.1, 0.3]
color = "wood"
attach = "top"
to = "can.bottom"
"##;

/// The description inside a `.glb`.
fn json_of(glb: &[u8]) -> serde_json::Value {
    let length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
    serde_json::from_slice(&glb[20..20 + length]).unwrap()
}

/// The pixel of a sheet at a place on it.
fn pixel(pixels: &[[u8; 4]], width: u32, height: u32, at: [f64; 2]) -> [u8; 4] {
    let x = ((at[0] * f64::from(width)).floor() as u32).min(width - 1);
    let y = ((at[1] * f64::from(height)).floor() as u32).min(height - 1);
    pixels[(y * width + x) as usize]
}

#[test]
fn a_model_without_images_is_written_as_it_always_was() {
    let model = built(
        "plain",
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"box\"\nshape = \"box\"\ncolor = \"wood\"\n",
    );
    let glb = write_glb(&model);
    let json = json_of(&glb);
    assert_eq!(json["extras"]["duck_mold"]["version"], 1);
    for field in ["textures", "samplers", "images"] {
        assert!(json.get(field).is_none(), "{field}");
    }
    assert!(json["extras"]["duck_mold"].get("images").is_none());
    let text = String::from_utf8_lossy(&glb[20..]).into_owned();
    assert!(!text.contains("TEXCOORD_0") && !text.contains("baseColorTexture"));
    let read = forge::read_duck_mold_glb(&glb).unwrap();
    assert!(read.picture.is_none());
    assert!(
        read.triangles
            .iter()
            .all(|triangle| triangle.on_picture.is_none())
    );
}

#[test]
fn a_model_with_images_holds_one_sharp_picture_and_says_layout_2() {
    let model = built("figure", FIGURE);
    let glb = write_glb(&model);
    let json = json_of(&glb);
    let sheet = ImageSheet::of(&model).unwrap().unwrap();

    assert_eq!(json["extras"]["duck_mold"]["version"], 2);
    // One material, showing one picture, drawn with the nearest pixel
    // (9728) near and far, and never repeating (33071).
    assert_eq!(json["materials"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"],
        0
    );
    assert_eq!(
        json["samplers"],
        serde_json::json!([{ "magFilter": 9728, "minFilter": 9728, "wrapS": 33071, "wrapT": 33071 }])
    );
    assert_eq!(json["images"][0]["mimeType"], "image/png");
    assert!(
        json["images"][0].get("uri").is_none(),
        "the picture is inside the file"
    );
    // What is where on the sheet.
    let images = &json["extras"]["duck_mold"]["images"];
    assert_eq!(
        images["sheet"],
        serde_json::json!([sheet.width, sheet.height])
    );
    let files: Vec<(&str, Option<&str>)> = images["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| (file["file"].as_str().unwrap(), file["over"].as_str()))
        .collect();
    assert_eq!(files, [("quad.png", None), ("mouth.png", Some("steel"))]);
    // Every mesh says where its corners are on the sheet.
    for mesh in json["meshes"].as_array().unwrap() {
        assert!(mesh["primitives"][0]["attributes"]["TEXCOORD_0"].is_u64());
    }
    // The same model always gives the same bytes.
    assert_eq!(glb, write_glb(&built("figure_again", FIGURE)));
}

#[test]
fn the_reader_gives_back_the_picture_and_every_triangles_place_on_it() {
    let model = built("reader", FIGURE);
    let glb = write_glb(&model);
    let read = forge::read_duck_mold_glb(&glb).unwrap();
    let sheet = ImageSheet::of(&model).unwrap().unwrap();

    // The picture: the sheet, pixel for pixel.
    let picture = read.picture.as_ref().expect("a picture");
    assert_eq!((picture.width, picture.height), (sheet.width, sheet.height));
    let pixels = read_png_up_to(&picture.png, "sheet", 4096).unwrap();
    assert_eq!((pixels.width, pixels.height), (sheet.width, sheet.height));
    assert_eq!(pixels.pixels, sheet.pixels);

    // The triangles, in the model's order: each face a fan from its
    // first corner.
    let mut number = 0;
    let (mut with, mut without) = (0, 0);
    for leaf in model.leaves() {
        for face in &model.faces[leaf.faces.clone()] {
            let cell = sheet.cell(&model, face);
            for step in 1..face.corners.len() - 1 {
                let triangle = &read.triangles[number];
                number += 1;
                match (&face.image, cell) {
                    (Some(image), Some(cell)) => {
                        with += 1;
                        assert_eq!(triangle.color, [255, 255, 255]);
                        let on = triangle.on_picture.expect("a place on the picture");
                        for (corner, at) in [0, step, step + 1].into_iter().zip(on) {
                            let wanted = sheet.place(&model, cell, image.at[corner]);
                            assert!(
                                (f64::from(at[0]) - wanted[0]).abs() < 1e-6
                                    && (f64::from(at[1]) - wanted[1]).abs() < 1e-6,
                                "{}: {at:?} {wanted:?}",
                                leaf.path
                            );
                            // And the corner in the model is the same one.
                            let vertex = read.vertices[triangle.corners[[0, step, step + 1]
                                .iter()
                                .position(|c| *c == corner)
                                .unwrap()]];
                            let there = face.corners[corner];
                            assert!(
                                (f64::from(vertex[0]) - there.x).abs() < 1e-5
                                    && (f64::from(vertex[1]) - there.y).abs() < 1e-5
                                    && (f64::from(vertex[2]) - there.z).abs() < 1e-5
                            );
                        }
                    }
                    _ => {
                        without += 1;
                        assert_eq!(triangle.on_picture, None, "{}", leaf.path);
                        let rgb = face.color.map(|color| model.palette[color].rgb);
                        assert_eq!(Some(triangle.color), rgb);
                    }
                }
            }
        }
    }
    assert_eq!(number, read.triangles.len());
    assert!(with > 20 && without > 20, "{with} {without}");

    // What a game sees: the middle of the head's front, upper half, is
    // steel (the mouth image is see-through there); its lower half is
    // the mouth's black.
    let head = read.parts.iter().find(|part| part.name == "head").unwrap();
    let front: Vec<&forge::GlbTriangle> = read.triangles
        [head.first_triangle..head.first_triangle + head.triangles]
        .iter()
        .filter(|triangle| triangle.on_picture.is_some())
        .collect();
    assert_eq!(front.len(), 2);
    let on = front[0].on_picture.unwrap();
    let across = (on.iter().map(|at| at[0]).fold(f32::MAX, f32::min)
        + on.iter().map(|at| at[0]).fold(f32::MIN, f32::max))
        / 2.0;
    let top = on.iter().map(|at| at[1]).fold(f32::MAX, f32::min);
    let bottom = on.iter().map(|at| at[1]).fold(f32::MIN, f32::max);
    let at = |share: f32| {
        pixel(
            &pixels.pixels,
            pixels.width,
            pixels.height,
            [f64::from(across), f64::from(top + (bottom - top) * share)],
        )
    };
    assert_eq!(at(0.25), [0x8A, 0x93, 0x9B, 255]);
    assert_eq!(at(0.75), [0, 0, 0, 255]);
}

#[test]
fn far_versions_share_the_models_picture() {
    let model = built("far", FIGURE);
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    assert_eq!(read.far.len(), 2);
    assert!(read.picture.is_some());
    for far in &read.far {
        assert!(far.model.picture.is_none(), "it shows the model's own");
        let can = far
            .model
            .parts
            .iter()
            .find(|part| part.name == "can")
            .unwrap();
        let side = &far.model.triangles[can.first_triangle..can.first_triangle + can.triangles];
        assert!(side.iter().any(|triangle| triangle.on_picture.is_some()));
        // Every place is on the picture.
        for triangle in &far.model.triangles {
            for at in triangle.on_picture.into_iter().flatten() {
                assert!((0.0..=1.0).contains(&at[0]) && (0.0..=1.0).contains(&at[1]));
            }
        }
    }
}

#[test]
fn duck_mold_reads_its_own_file_back_with_its_images() {
    let model = built("back", FIGURE);
    let sheet = ImageSheet::of(&model).unwrap().unwrap();
    let back = read_glb(&write_glb(&model), "figure.glb").unwrap();
    assert_eq!(back.pictures.len(), 1);
    assert_eq!(back.pictures[0].pixels, sheet.pixels);
    assert_eq!(back.faces.len(), model.faces.len());
    for (face, was) in back.faces.iter().zip(&model.faces) {
        assert_eq!(face.image.is_some(), was.image.is_some());
        assert_eq!(face.corners.len(), was.corners.len());
    }
}

#[test]
fn a_damaged_file_with_images_is_a_sentence_and_never_a_crash() {
    let model = built("damaged", FIGURE);
    let glb = write_glb(&model);

    // Cut off anywhere.
    for length in (0..glb.len()).step_by(7) {
        assert!(
            forge::read_duck_mold_glb(&glb[..length]).is_err(),
            "{length}"
        );
        assert!(read_glb(&glb[..length], "cut.glb").is_err(), "{length}");
    }

    // The description changed, the numbers as they were.
    let with = |change: &dyn Fn(&mut serde_json::Value)| -> Vec<u8> {
        let length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let mut json = json_of(&glb);
        change(&mut json);
        let mut text = serde_json::to_vec(&json).unwrap();
        while !text.len().is_multiple_of(4) {
            text.push(b' ');
        }
        let rest = &glb[20 + length..];
        let total = 20 + text.len() + rest.len();
        let mut out = Vec::new();
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend((total as u32).to_le_bytes());
        out.extend((text.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(text);
        out.extend(rest);
        out
    };
    let said = |glb: Vec<u8>| forge::read_duck_mold_glb(&glb).unwrap_err();

    let no_material = with(&|json| {
        json["materials"][0]["pbrMetallicRoughness"]
            .as_object_mut()
            .unwrap()
            .remove("baseColorTexture");
    });
    assert_eq!(
        said(no_material.clone()),
        "this .glb says it has images, but holds no picture"
    );
    assert!(read_glb(&no_material, "x.glb").is_err());

    let outside = with(&|json| {
        let image = json["images"][0].as_object_mut().unwrap();
        image.remove("bufferView");
        image.insert("uri".to_owned(), "sheet.png".into());
    });
    assert_eq!(
        said(outside),
        "the picture of the model's images isn't inside the file, where Duck Mold puts it"
    );

    let no_places = with(&|json| {
        json["extras"]["duck_mold"]["images"]
            .as_object_mut()
            .unwrap()
            .remove("plain_at");
    });
    assert_eq!(
        said(no_places),
        "this .glb says it has images, but not where they are"
    );

    let past_the_end = with(&|json| {
        let view = json["images"][0]["bufferView"].as_u64().unwrap() as usize;
        json["bufferViews"][view]["byteLength"] = 99_999_999.into();
    });
    assert_eq!(
        said(past_the_end),
        "the picture of the model's images runs past the end of the file"
    );

    let not_a_png = with(&|json| {
        // Point the picture at the positions instead.
        json["images"][0]["bufferView"] = 0.into();
    });
    assert_eq!(
        said(not_a_png),
        "the picture of the model's images isn't a PNG"
    );

    let no_corners = with(&|json| {
        for mesh in json["meshes"].as_array_mut().unwrap() {
            mesh["primitives"][0]["attributes"]
                .as_object_mut()
                .unwrap()
                .remove("TEXCOORD_0");
        }
    });
    assert_eq!(said(no_corners), "a list of numbers is missing");

    // A corner moved off the picture: the first place of the list.
    let json = json_of(&glb);
    let accessor = json["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"]
        .as_u64()
        .unwrap() as usize;
    let view = json["accessors"][accessor]["bufferView"].as_u64().unwrap() as usize;
    let length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
    let start =
        20 + length + 8 + json["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
    let mut off = glb.clone();
    off[start..start + 4].copy_from_slice(&7.5f32.to_le_bytes());
    assert!(said(off).contains("has a corner that is off the picture"));
}

#[test]
fn a_newer_layout_than_the_reader_knows_is_refused_with_what_to_do() {
    let model = built("newer", FIGURE);
    let glb = write_glb(&model);
    let length = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
    let text = String::from_utf8(glb[20..20 + length].to_vec()).unwrap();
    // The same length, so nothing else in the file changes.
    let newer = text.replacen("\"version\":2", "\"version\":3", 1);
    assert_ne!(text, newer);
    let mut changed = glb.clone();
    changed[20..20 + length].copy_from_slice(newer.as_bytes());
    assert_eq!(
        forge::read_duck_mold_glb(&changed).unwrap_err(),
        "this .glb was written by a newer Duck Mold than this reader knows: update Duck Forge"
    );
}
