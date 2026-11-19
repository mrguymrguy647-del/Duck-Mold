//! Phase 7b, step 5: a model from another program keeps its pictures.
//!
//! A glTF material's colour picture comes in with the model, laid on by
//! the file's own layout (its "UVs"): a PNG or a JPEG, inside the file
//! or beside it. The recipe has the last word, as it has for colour.
//! The files here are written by this test, small enough to check
//! corner by corner; the example signpost is read too.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::glb::{read_glb, write_glb};
use duck_mold_core::gltf::read_gltf;
use duck_mold_core::inspect::images_report;
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load;
use duck_mold_core::mesh::{MeshFile, read_obj};
use duck_mold_core::model::Model;
use duck_mold_core::picture::{ImageSheet, read_file_picture, write_png};
use duck_mold_core::tools;
use serde_json::{Value, json};

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A test's folder as messages show it.
fn folder_shown(name: &str) -> String {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(name)
        .to_string_lossy()
        .replace('\\', "/")
}

fn base64(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut bits = [0u8; 3];
        bits[..chunk.len()].copy_from_slice(chunk);
        let n = u32::from_be_bytes([0, bits[0], bits[1], bits[2]]);
        for at in 0..4 {
            if at <= chunk.len() {
                out.push(DIGITS[(n >> (18 - 6 * at) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// A 2 x 2 PNG with a different colour in each corner: red and green on
/// top, blue and white below.
fn corners_png() -> Vec<u8> {
    write_png(2, 2, &[RED, GREEN, BLUE, WHITE], true)
}

/// A `.gltf` with one wall, 2 wide and 1 high, facing the front, of the
/// material "paint". `places` are its four corners' places on the
/// picture, in the order bottom left, bottom right, top right, top left
/// as you look at the wall from the front; `None` writes no layout.
/// `material` and `more` are added to the material and the file.
fn wall(places: Option<[[f32; 2]; 4]>, material: Value, more: Value) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    // Seen from the front, the model's left (glTF's +x) is on the right.
    let points: [[f32; 3]; 4] = [
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    for number in points.iter().flatten() {
        bin.extend(number.to_le_bytes());
    }
    let places_at = bin.len();
    for number in places.iter().flatten().flatten() {
        bin.extend(number.to_le_bytes());
    }
    let corners_at = bin.len();
    for corner in [0u16, 1, 2, 0, 2, 3] {
        bin.extend(corner.to_le_bytes());
    }
    let mut attributes = json!({ "POSITION": 0 });
    if places.is_some() {
        attributes["TEXCOORD_0"] = json!(2);
    }
    let mut paint = json!({ "name": "paint" });
    for (key, value) in material.as_object().unwrap() {
        paint[key] = value.clone();
    }
    let mut gltf = json!({
        "asset": { "version": "2.0" },
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": attributes, "indices": 1, "material": 0 }] }],
        "materials": [paint],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5123, "count": 6, "type": "SCALAR" },
            { "bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC2" },
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": places_at },
            { "buffer": 0, "byteOffset": places_at, "byteLength": corners_at - places_at },
            { "buffer": 0, "byteOffset": corners_at, "byteLength": 12 },
        ],
        "buffers": [{
            "byteLength": bin.len(),
            "uri": format!("data:application/octet-stream;base64,{}", base64(&bin)),
        }],
    });
    for (key, value) in more.as_object().unwrap() {
        gltf[key] = value.clone();
    }
    serde_json::to_vec_pretty(&gltf).unwrap()
}

/// The whole picture on the wall, upright.
const WHOLE: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

/// A picture written inside the file, for [`wall`]'s `more`.
fn picture_inside(bytes: &[u8], kind: &str) -> Value {
    json!({
        "textures": [{ "source": 0 }],
        "images": [{ "name": "art", "uri": format!("data:image/{kind};base64,{}", base64(bytes)) }],
    })
}

/// A material that shows picture 0.
fn pictured() -> Value {
    json!({ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } })
}

fn read(bytes: &[u8], folder: &Path) -> (Option<MeshFile>, Vec<Issue>) {
    let mut issues = Vec::new();
    let mesh = read_gltf(bytes, "wall.gltf", folder, &mut issues);
    (mesh, issues)
}

/// A file's name as a report gives it, without the folders the test
/// works in.
fn short(name: &str) -> &str {
    name.rsplit('/').next().unwrap()
}

fn notes(issues: &[Issue]) -> Vec<&str> {
    assert!(issues.iter().all(|issue| !issue.is_error()), "{issues:#?}");
    issues.iter().map(|issue| issue.message.as_str()).collect()
}

/// Builds `recipe` in a folder that also holds the `files`.
fn build(name: &str, files: &[(&str, &[u8])], recipe: &str) -> (Option<Model>, Report) {
    let folder = folder(name);
    for (file, bytes) in files {
        std::fs::write(folder.join(file), bytes).unwrap();
    }
    std::fs::write(folder.join("scene.toml"), recipe).unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("scene.toml"), &mut issues).unwrap();
    let model = assemble(&loaded, &mut issues);
    (model, Report::from_issues(issues))
}

/// The colour the model shows at a point of the wall: `across` from its
/// left edge as you look at it from the front and `down` from its top,
/// each 0 to 1. Found the way a game would: through the sheet of images.
fn colour_at(model: &Model, across: f64, down: f64) -> [u8; 3] {
    let sheet = ImageSheet::of(model)
        .unwrap()
        .expect("the model has images");
    let bounds = model.bounds().unwrap();
    // Seen from the front, the model's right (+x in Duck Mold) is on
    // the left.
    let x = bounds.max.x - across * (bounds.max.x - bounds.min.x);
    let y = bounds.max.y - down * (bounds.max.y - bounds.min.y);
    for (number, face) in model.faces.iter().enumerate() {
        let Some(image) = &face.image else { continue };
        let [a, b, c] = [face.corners[0], face.corners[1], face.corners[2]];
        // How much of each corner the point is.
        let whole = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        let of_b = ((x - a.x) * (c.y - a.y) - (c.x - a.x) * (y - a.y)) / whole;
        let of_c = ((b.x - a.x) * (y - a.y) - (x - a.x) * (b.y - a.y)) / whole;
        let of_a = 1.0 - of_b - of_c;
        if of_a < -1e-9 || of_b < -1e-9 || of_c < -1e-9 {
            continue;
        }
        let at = [0, 1].map(|axis| {
            of_a * image.at[0][axis] + of_b * image.at[1][axis] + of_c * image.at[2][axis]
        });
        let cell = sheet.cell_of[number].unwrap();
        let [u, v] = sheet.place(model, cell, at);
        let column = ((u * f64::from(sheet.width)) as u32).min(sheet.width - 1);
        let row = ((v * f64::from(sheet.height)) as u32).min(sheet.height - 1);
        let pixel = sheet.pixels[(row * sheet.width + column) as usize];
        return [pixel[0], pixel[1], pixel[2]];
    }
    panic!("no face with an image at {across}, {down}");
}

const PLAIN: &str = "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\n";

// ---- reading -----------------------------------------------------------------

#[test]
fn a_picture_is_laid_on_the_way_the_file_says() {
    let file = wall(
        Some(WHOLE),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(issues, Vec::new());
    let mesh = mesh.unwrap();
    assert_eq!(mesh.pictures.len(), 1);
    assert_eq!(mesh.pictures[0].file, "wall.gltf: art");
    assert_eq!((mesh.pictures[0].width, mesh.pictures[0].height), (2, 2));
    assert_eq!(mesh.picture_of, [Some(0), Some(0)]);

    let (model, report) = build("import_images_wall", &[("wall.gltf", &file)], PLAIN);
    assert_eq!(report.errors, Vec::new());
    // Nothing to warn about: a face under a solid picture needs no
    // colour, and the palette isn't given one for it.
    assert_eq!(report.warnings, Vec::new());
    let model = model.unwrap();
    assert!(model.palette.is_empty(), "{:?}", model.palette);
    assert!(model.faces.iter().all(|face| face.color.is_none()));
    // Each corner of the picture is in its corner of the wall.
    assert_eq!(colour_at(&model, 0.25, 0.25), [255, 0, 0]);
    assert_eq!(colour_at(&model, 0.75, 0.25), [0, 255, 0]);
    assert_eq!(colour_at(&model, 0.25, 0.75), [0, 0, 255]);
    assert_eq!(colour_at(&model, 0.75, 0.75), [255, 255, 255]);

    // The report names it, with how big its pixels are on the model:
    // 2 pixels over 2 metres and 2 over 1.
    let images = images_report(&model).unwrap();
    assert_eq!(images.files.len(), 1);
    assert_eq!(short(&images.files[0].file), "wall.gltf: art");
    assert_eq!(images.files[0].on[0].part, "wall");
    assert_eq!(images.files[0].on[0].face, "paint");
    assert_eq!(images.files[0].on[0].metres, [2.0, 1.0]);

    // And it is in the .glb, and comes back from it.
    let again = read_glb(&write_glb(&model), "wall.glb").unwrap();
    assert!(again.faces.iter().all(|face| face.image.is_some()));
}

#[test]
fn a_layout_turned_or_flipped_turns_or_flips_the_picture() {
    // The picture's left edge along the wall's bottom: a quarter turn.
    let turned = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    let file = wall(
        Some(turned),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    let (model, _) = build("import_images_turned", &[("wall.gltf", &file)], PLAIN);
    let model = model.unwrap();
    assert_eq!(colour_at(&model, 0.25, 0.75), [255, 0, 0]);
    assert_eq!(colour_at(&model, 0.25, 0.25), [0, 255, 0]);
    assert_eq!(colour_at(&model, 0.75, 0.75), [0, 0, 255]);

    // A mirrored part shows it mirrored, as any image.
    let file = wall(
        Some(WHOLE),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    let (model, report) = build(
        "import_images_mirrored",
        &[("wall.gltf", &file)],
        "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\nmirror = \"x\"\n",
    );
    assert_eq!(report.errors, Vec::new());
    let model = model.unwrap();
    assert!(model.parts[0].mirrored);
    assert_eq!(colour_at(&model, 0.75, 0.25), [255, 0, 0]);
    assert_eq!(colour_at(&model, 0.25, 0.25), [0, 255, 0]);
    assert_eq!(colour_at(&model, 0.75, 0.75), [0, 0, 255]);
}

#[test]
fn pictures_come_inside_the_file_beside_it_and_as_jpeg() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
    let bytes = std::fs::read(root.join("sign.gltf")).unwrap();
    let mut issues = Vec::new();
    let mesh = read_gltf(&bytes, "parts/sign.gltf", &root, &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    assert_eq!(mesh.materials, ["post", "board", "cap"]);
    let pictures: Vec<(&str, u32, u32)> = mesh
        .pictures
        .iter()
        .map(|picture| (picture.file.as_str(), picture.width, picture.height))
        .collect();
    assert_eq!(
        pictures,
        [
            // A PNG beside the file, a PNG inside its block of numbers,
            // and a JPEG written inside it as text.
            ("parts/sign_post.png", 8, 16),
            ("parts/sign.gltf: board", 64, 32),
            ("parts/sign.gltf: cap", 16, 8),
        ]
    );
    // The file beside it is one of the files a window watches.
    assert_eq!(mesh.extra_files, [root.join("sign_post.png")]);
    // Every triangle shows its material's picture.
    for (triangle, &material) in mesh.material_of.iter().enumerate() {
        assert_eq!(mesh.picture(triangle), Some(material));
        assert!(mesh.lie(triangle).is_some());
    }
    // The post's picture repeats seven and a half times up it.
    let furthest = mesh
        .lies
        .iter()
        .zip(&mesh.material_of)
        .filter(|(_, material)| **material == 0)
        .flat_map(|(lie, _)| lie.unwrap())
        .map(|at| at[1])
        .fold(0.0, f64::max);
    assert_eq!(furthest, 7.5);

    // A JPEG is read to the same pixels on every computer: this number
    // is worked out from all of them.
    let jpeg = std::fs::read(root.join("sign_cap.jpg")).unwrap();
    let (cap, was) = read_file_picture(&jpeg, "sign_cap.jpg").unwrap();
    assert_eq!(was, None);
    assert!(cap.is_solid());
    assert_eq!(cap.stamp, mesh.pictures[2].stamp);
    assert_eq!(
        cap.stamp, JPEG_STAMP,
        "the cap's pixels: {:#018x}",
        cap.stamp
    );
}

/// See the test above.
const JPEG_STAMP: u64 = 0x6d5a_1911_ccdb_5759;

#[test]
fn a_material_colour_that_isnt_white_tints_its_picture() {
    // Half the light of each colour but blue, which has none.
    let material = json!({ "pbrMetallicRoughness": {
        "baseColorTexture": { "index": 0 },
        "baseColorFactor": [0.5, 0.5, 0.0, 1.0],
    } });
    let file = wall(Some(WHOLE), material, picture_inside(&corners_png(), "png"));
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(issues, Vec::new());
    let mesh = mesh.unwrap();
    // Half the light is 188 of 255 as colours are written.
    assert_eq!(
        mesh.pictures[0].pixels,
        [
            [188, 0, 0, 255],
            [0, 188, 0, 255],
            [0, 0, 0, 255],
            [188, 188, 0, 255]
        ]
    );
    // White leaves every pixel as it was.
    let file = wall(
        Some(WHOLE),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    let mesh = read(&file, Path::new(".")).0.unwrap();
    assert_eq!(mesh.pictures[0].pixels, [RED, GREEN, BLUE, WHITE]);
}

#[test]
fn see_through_pixels_show_the_materials_colour_only_if_the_material_says_so() {
    let gaps = write_png(2, 2, &[RED, [0, 0, 0, 0], BLUE, [9, 9, 9, 0]], false);
    // An ordinary material is solid whatever its picture says.
    let file = wall(Some(WHOLE), pictured(), picture_inside(&gaps, "png"));
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(issues, Vec::new());
    assert!(mesh.unwrap().pictures[0].is_solid());

    // One that says its picture has gaps keeps them, and the material's
    // colour shows there, with a note that nothing is truly see-through.
    let material = json!({ "alphaMode": "BLEND", "pbrMetallicRoughness": {
        "baseColorTexture": { "index": 0 },
    } });
    let file = wall(Some(WHOLE), material, picture_inside(&gaps, "png"));
    let (mesh, issues) = read(&file, Path::new("."));
    assert!(!mesh.unwrap().pictures[0].is_solid());
    assert_eq!(
        notes(&issues),
        [
            "where a picture is see-through, its material's colour shows: Duck Mold has no see-through faces"
        ]
    );
    let (model, report) = build("import_images_gaps", &[("wall.gltf", &file)], PLAIN);
    let model = model.unwrap();
    // The material's white is under the gaps, so it is kept, and said.
    assert_eq!(model.palette.len(), 1);
    assert_eq!(model.palette[0].name, "paint");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.code == Code::NoColor)
    );
    assert_eq!(colour_at(&model, 0.25, 0.25), [255, 0, 0]);
    assert_eq!(colour_at(&model, 0.75, 0.25), [255, 255, 255]);
}

#[test]
fn a_picture_repeats_across_a_face_up_to_eight_times() {
    let picture = picture_inside(&corners_png(), "png");
    // Three times across, twice down.
    let thrice = [[0.0, 2.0], [3.0, 2.0], [3.0, 0.0], [0.0, 0.0]];
    let file = wall(Some(thrice), pictured(), picture.clone());
    let (model, report) = build("import_images_repeat", &[("wall.gltf", &file)], PLAIN);
    assert_eq!(report.warnings, Vec::new());
    let model = model.unwrap();
    for copy in 0..3 {
        let left = (f64::from(copy) + 0.25) / 3.0;
        assert_eq!(colour_at(&model, left, 0.125), [255, 0, 0], "copy {copy}");
        assert_eq!(colour_at(&model, left + 0.5 / 3.0, 0.125), [0, 255, 0]);
        assert_eq!(colour_at(&model, left, 0.875), [0, 0, 255]);
    }

    // Far along a repeating picture is the same as at its start: the
    // layout is moved back by whole pictures, below zero too.
    let far = [[5.0, -3.0], [6.0, -3.0], [6.0, -4.0], [5.0, -4.0]];
    let file = wall(Some(far), pictured(), picture.clone());
    let mesh = read(&file, Path::new(".")).0.unwrap();
    for lie in &mesh.lies {
        for at in lie.unwrap().iter().flatten() {
            assert!((0.0..=1.0).contains(at), "{at}");
        }
    }

    // More than eight times is too many for the sheet: the picture is
    // shown once, with a note.
    let many = [[0.0, 1.0], [20.0, 1.0], [20.0, 0.0], [0.0, 0.0]];
    let file = wall(Some(many), pictured(), picture.clone());
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(
        notes(&issues),
        [
            "2 triangles of this file repeat a picture more than 8 times across: there the picture is shown once"
        ]
    );
    let furthest = mesh
        .unwrap()
        .lies
        .iter()
        .flat_map(|lie| lie.unwrap())
        .map(|at| at[0])
        .fold(0.0, f64::max);
    assert_eq!(furthest, 1.0);

    // A picture that says it stops at its edge doesn't repeat.
    let mut held = picture.clone();
    held["textures"][0]["sampler"] = json!(0);
    held["samplers"] = json!([{ "wrapS": 33071, "wrapT": 33071 }]);
    let past = [[-0.5, 1.5], [1.5, 1.5], [1.5, -0.5], [-0.5, -0.5]];
    let file = wall(Some(past), pictured(), held);
    let mesh = read(&file, Path::new(".")).0.unwrap();
    for at in mesh.lies.iter().flat_map(|lie| lie.unwrap()).flatten() {
        assert!((0.0..=1.0).contains(&at), "{at}");
    }
}

#[test]
fn a_picture_too_big_is_made_smaller_with_a_note() {
    // 2048 x 4: left half red, right half blue.
    let pixels: Vec<[u8; 4]> = (0..2048 * 4)
        .map(|at| if at % 2048 < 1024 { RED } else { BLUE })
        .collect();
    let big = write_png(2048, 4, &pixels, true);
    let file = wall(Some(WHOLE), pictured(), picture_inside(&big, "png"));
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(
        notes(&issues),
        [
            "its picture \"wall.gltf: art\" is 2048 x 4 pixels and was made smaller, to 1024 x 2: an image can be 1024 x 1024 at most"
        ]
    );
    let picture = &mesh.unwrap().pictures[0];
    assert_eq!((picture.width, picture.height), (1024, 2));
    assert_eq!(picture.pixel(0, 0), RED);
    assert_eq!(picture.pixel(511, 1), RED);
    assert_eq!(picture.pixel(512, 0), BLUE);

    // An odd size is rounded up, and the last pixels are the average of
    // what there is.
    let pixels: Vec<[u8; 4]> = (0..1025).map(|_| [10, 20, 30, 255]).collect();
    let (picture, was) = read_file_picture(&write_png(1025, 1, &pixels, true), "odd.png").unwrap();
    assert_eq!(was, Some([1025, 1]));
    assert_eq!((picture.width, picture.height), (513, 1));
    assert_eq!(picture.pixel(512, 0), [10, 20, 30, 255]);
}

#[test]
fn a_picture_that_cant_be_read_is_left_out_and_nothing_crashes() {
    let material = pictured();
    let tried = |more: Value, files: &[(&str, &[u8])], name: &str| -> Vec<String> {
        let folder = folder(name);
        for (file, bytes) in files {
            std::fs::write(folder.join(file), bytes).unwrap();
        }
        let file = wall(Some(WHOLE), material.clone(), more);
        let (mesh, issues) = read(&file, &folder);
        let mesh = mesh.expect("the model still comes in");
        assert!(mesh.pictures.is_empty() && mesh.picture_of.is_empty());
        // Its layout is still there, for an image from the recipe.
        assert_eq!(mesh.lies.len(), 2);
        notes(&issues).into_iter().map(str::to_owned).collect()
    };
    // A file beside it that isn't there.
    let beside = json!({ "textures": [{ "source": 0 }], "images": [{ "uri": "art/wood.png" }] });
    assert_eq!(
        tried(beside.clone(), &[], "import_images_missing"),
        [
            "its picture \"art/wood.png\" was left out (there is no such file: it must be where the .gltf says, starting from the .gltf's folder): its faces keep their material's colour"
        ]
    );
    // One that isn't a picture.
    std::fs::create_dir_all(folder("import_images_text").join("art")).unwrap();
    let said = {
        let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("import_images_text");
        std::fs::write(folder.join("art/wood.png"), b"hello").unwrap();
        let file = wall(Some(WHOLE), material.clone(), beside);
        let (_, issues) = read(&file, &folder);
        notes(&issues).join("\n")
    };
    assert_eq!(
        said,
        "its picture \"art/wood.png\" was left out (isn't a PNG or a JPEG picture): its faces keep their material's colour"
    );
    // Broken text inside the file.
    let broken = json!({ "textures": [{ "source": 0 }], "images": [{ "uri": "data:image/png;base64,!!!" }] });
    assert_eq!(
        tried(broken, &[], "import_images_broken"),
        [
            "its picture \"wall.gltf: picture 1\" was left out (its text inside the file is broken): its faces keep their material's colour"
        ]
    );
    // A kind of picture Duck Mold doesn't read.
    let webp = json!({ "textures": [{ "extensions": { "EXT_texture_webp": { "source": 0 } } }], "images": [{ "uri": "a.webp" }] });
    assert!(tried(webp, &[], "import_images_webp")[0].contains("WebP or KTX2"));
    // A picture that isn't in the file at all.
    assert!(tried(json!({}), &[], "import_images_none")[0].contains("isn't in the file"));

    // A JPEG and a PNG cut off anywhere: a sentence or a picture, never
    // a crash.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
    for name in ["sign_cap.jpg", "sign_board.png"] {
        let whole = std::fs::read(root.join(name)).unwrap();
        for length in 0..whole.len() {
            let _ = read_file_picture(&whole[..length], name);
        }
        // And with a byte changed.
        for at in (0..whole.len()).step_by(7) {
            let mut changed = whole.clone();
            changed[at] ^= 0x55;
            let _ = read_file_picture(&changed, name);
        }
    }
}

#[test]
fn other_maps_are_left_out_with_a_note() {
    let material = json!({
        "normalTexture": { "index": 0 },
        "emissiveTexture": { "index": 0 },
        "pbrMetallicRoughness": {
            "baseColorTexture": { "index": 0, "extensions": { "KHR_texture_transform": { "scale": [2, 2] } } },
            "metallicRoughnessTexture": { "index": 0 },
        },
    });
    let mut more = picture_inside(&corners_png(), "png");
    more["textures"][0]["sampler"] = json!(0);
    more["samplers"] = json!([{ "wrapS": 33648 }]);
    let file = wall(Some(WHOLE), material, more);
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(
        mesh.unwrap().pictures.len(),
        1,
        "the colour picture is still read"
    );
    let notes = notes(&issues);
    for words in [
        "bump, shine and glow maps",
        "KHR_texture_transform",
        "mirrored",
    ] {
        assert!(
            notes.iter().any(|note| note.contains(words)),
            "{words} in {notes:#?}"
        );
    }
}

// ---- the recipe has the last word ---------------------------------------------

#[test]
fn a_colour_from_the_recipe_takes_the_files_picture_off() {
    let file = wall(
        Some(WHOLE),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    for said in ["colors = { paint = \"stone\" }", "color = \"stone\""] {
        let (model, report) = build(
            "import_images_painted",
            &[("wall.gltf", &file)],
            &format!(
                "version = 0\n[palette]\nstone = \"#8C8F94\"\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\n{said}\n"
            ),
        );
        assert_eq!(report.errors, Vec::new());
        assert_eq!(report.warnings, Vec::new());
        let model = model.unwrap();
        assert!(model.pictures.is_empty(), "{said}");
        assert!(
            model
                .faces
                .iter()
                .all(|face| face.image.is_none() && face.color == Some(0))
        );
    }
}

#[test]
fn an_image_from_the_recipe_goes_on_by_the_files_layout() {
    let file = wall(
        Some(WHOLE),
        pictured(),
        picture_inside(&corners_png(), "png"),
    );
    // Another picture: the same corners, the other way up.
    let mine = write_png(2, 2, &[BLUE, WHITE, RED, GREEN], true);
    for said in ["images = { paint = \"mine.png\" }", "image = \"mine.png\""] {
        let (model, report) = build(
            "import_images_own",
            &[("wall.gltf", &file), ("mine.png", &mine)],
            &format!("version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\n{said}\n"),
        );
        assert_eq!(report.errors, Vec::new(), "{said}");
        let model = model.unwrap();
        // Only the recipe's picture is in the model now.
        assert_eq!(model.pictures.len(), 1);
        assert_eq!(short(&model.pictures[0].file), "mine.png");
        assert_eq!(colour_at(&model, 0.25, 0.25), [0, 0, 255]);
        assert_eq!(colour_at(&model, 0.75, 0.75), [0, 255, 0]);
    }

    // "none" leaves the material's own colour.
    let (model, report) = build(
        "import_images_none_said",
        &[("wall.gltf", &file)],
        "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\nimages = { paint = \"none\" }\n",
    );
    assert_eq!(report.errors, Vec::new());
    let model = model.unwrap();
    assert!(model.pictures.is_empty());
    assert_eq!(model.palette[0].name, "paint");
    assert_eq!(model.palette[0].rgb, [255, 255, 255]);

    // A material that isn't there is a mistake that lists the ones that are.
    let (model, report) = build(
        "import_images_misspelt",
        &[("wall.gltf", &file), ("mine.png", &mine)],
        "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\nimages = { pain = \"mine.png\" }\n",
    );
    assert!(model.is_none());
    assert_eq!(report.errors[0].code, Code::UnknownFace);
    assert_eq!(report.errors[0].suggestion.as_deref(), Some("paint"));
}

#[test]
fn a_file_without_a_layout_cant_take_an_image_and_says_why() {
    let file = wall(None, json!({}), json!({}));
    let mine = corners_png();
    let (model, report) = build(
        "import_images_no_layout",
        &[("wall.gltf", &file), ("mine.png", &mine)],
        "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\nimage = \"mine.png\"\n",
    );
    assert!(model.is_none());
    assert_eq!(report.errors.len(), 1, "{:#?}", report.errors);
    assert_eq!(report.errors[0].code, Code::Unsupported);
    assert_eq!(
        report.errors[0]
            .message
            .replace(&format!("{}/", folder_shown("import_images_no_layout")), ""),
        "\"wall.gltf\" doesn't say where a picture goes on \"paint\" (it has no layout for pictures there, no \"UVs\"), so an image can't be put on. Unwrap the model in the program it was made in and save it again"
    );
    // A layout without a picture is enough.
    let file = wall(Some(WHOLE), json!({}), json!({}));
    let (model, report) = build(
        "import_images_layout_only",
        &[("wall.gltf", &file), ("mine.png", &mine)],
        "version = 0\n[[parts]]\nname = \"wall\"\nfile = \"wall.gltf\"\nimage = \"mine.png\"\n",
    );
    assert_eq!(report.errors, Vec::new());
    assert_eq!(colour_at(&model.unwrap(), 0.25, 0.25), [255, 0, 0]);
}

#[test]
fn an_obj_files_layout_is_read_so_its_materials_can_take_images() {
    // The same wall as an OBJ: its places count up from the picture's
    // bottom, and two corners name theirs from the end of the list.
    let obj = "v -1 0 0\nv 1 0 0\nv 1 1 0\nv -1 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nusemtl paint\nf 1/1 2/2 3/-2 4/-1\n";
    let mut issues = Vec::new();
    let mesh = read_obj(obj, "wall.obj", &mut issues).unwrap();
    assert_eq!(issues, Vec::new());
    assert_eq!(mesh.lies.len(), 2);
    assert!(mesh.pictures.is_empty());
    let (model, report) = build(
        "import_images_obj",
        &[("wall.obj", obj.as_bytes()), ("mine.png", &corners_png())],
        "version = 0\n[palette]\nstone = \"#8C8F94\"\n[[parts]]\nname = \"wall\"\nfile = \"wall.obj\"\ncolor = \"stone\"\nimages = { paint = \"mine.png\" }\n",
    );
    assert_eq!(report.errors, Vec::new());
    let model = model.unwrap();
    assert_eq!(colour_at(&model, 0.25, 0.25), [255, 0, 0]);
    assert_eq!(colour_at(&model, 0.75, 0.25), [0, 255, 0]);
    assert_eq!(colour_at(&model, 0.25, 0.75), [0, 0, 255]);

    // An OBJ without places has no layout, and says so when asked.
    let bare = "v -1 0 0\nv 1 0 0\nv 1 1 0\nf 1 2 3\n";
    let mesh = read_obj(bare, "bare.obj", &mut issues).unwrap();
    assert!(mesh.lies.is_empty());
}

#[test]
fn a_glb_duck_mold_wrote_keeps_its_images_as_a_part() {
    // A box with an image on its front, exported...
    let (source, report) = build(
        "import_images_duck_mold_source",
        &[("face.png", &corners_png())],
        "version = 0\nname = \"block\"\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"block\"\nshape = \"box\"\ncolor = \"wood\"\nimages = { front = \"face.png\" }\n",
    );
    assert_eq!(report.errors, Vec::new());
    let glb = write_glb(&source.unwrap());
    // ...and used as a part of another model.
    let (model, report) = build(
        "import_images_duck_mold_part",
        &[("block.glb", &glb)],
        "version = 0\n[[parts]]\nname = \"block\"\nfile = \"block.glb\"\n",
    );
    assert_eq!(report.errors, Vec::new());
    assert_eq!(report.warnings, Vec::new());
    let model = model.unwrap();
    let pictured = model
        .faces
        .iter()
        .filter(|face| face.image.is_some())
        .count();
    assert_eq!(pictured, 2, "the front's two triangles");
    assert_eq!(model.palette[0].name, "wood");
    // The front still shows the four corners where they were.
    let sheet = ImageSheet::of(&model).unwrap().unwrap();
    let mut seen: Vec<[u8; 4]> = sheet.pixels.clone();
    seen.sort();
    seen.dedup();
    for colour in [RED, GREEN, BLUE, WHITE] {
        assert!(seen.contains(&colour), "{colour:?}");
    }
    // The recipe that uses it can still take its images off.
    let (model, _) = build(
        "import_images_duck_mold_bare",
        &[("block.glb", &glb)],
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"block\"\nfile = \"block.glb\"\ncolor = \"red\"\nimage = \"none\"\n",
    );
    assert!(model.unwrap().pictures.is_empty());
}

#[test]
fn inspect_and_render_show_a_files_own_pictures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
    let report = tools::inspect_file(&root.join("sign.gltf")).unwrap();
    let images = report.images.expect("the sign has images");
    assert_eq!(images.files.len(), 3);
    assert_eq!(report.unpainted_triangles, 0);
    // The pixels are one size all over the sign: nothing to warn about.
    let per_metre: Vec<f64> = images
        .files
        .iter()
        .flat_map(|file| file.on.iter().map(|on| on.pixels_per_metre))
        .collect();
    let (least, most) = per_metre
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(least, most), &one| {
            (least.min(one), most.max(one))
        });
    assert!(most / least < 1.5, "{per_metre:?}");
}
