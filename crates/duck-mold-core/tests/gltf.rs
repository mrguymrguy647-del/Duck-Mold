//! Phase 1, step 6: glTF and `.glb` files as static parts.
//!
//! The files here are built by a small writer in this test (not by Duck
//! Mold's exporter), so that each layout glTF allows can be tried: a
//! `.glb`, a `.gltf` with a `.bin` beside it or with its numbers written
//! inside as base64; triangles as a list, a strip or a fan; corner numbers
//! of every size or none; objects placed by numbers or by a matrix.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::glb::write_glb;
use duck_mold_core::gltf::read_gltf;
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load;
use duck_mold_core::mesh::MeshFile;
use duck_mold_core::model::Model;
use duck_mold_core::tools;
use serde_json::{Value, json};

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// A glTF file being written: its block of numbers, and the lists that
/// describe them.
#[derive(Default)]
struct Writer {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Writer {
    fn pad(&mut self) {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
    }

    /// Points, as an accessor; returns its number.
    fn points(&mut self, points: &[[f32; 3]]) -> usize {
        self.pad();
        let start = self.bin.len();
        for point in points {
            for number in point {
                self.bin.extend(number.to_le_bytes());
            }
        }
        self.views.push(
            json!({ "buffer": 0, "byteOffset": start, "byteLength": self.bin.len() - start }),
        );
        self.accessors.push(json!({
            "bufferView": self.views.len() - 1, "componentType": 5126,
            "count": points.len(), "type": "VEC3",
        }));
        self.accessors.len() - 1
    }

    /// Corner numbers of `size` bytes each (1, 2 or 4).
    fn indices(&mut self, indices: &[u32], size: usize) -> usize {
        self.pad();
        let start = self.bin.len();
        for &index in indices {
            self.bin.extend(&index.to_le_bytes()[..size]);
        }
        let component = match size {
            1 => 5121,
            2 => 5123,
            _ => 5125,
        };
        self.views.push(
            json!({ "buffer": 0, "byteOffset": start, "byteLength": self.bin.len() - start }),
        );
        self.accessors.push(json!({
            "bufferView": self.views.len() - 1, "componentType": component,
            "count": indices.len(), "type": "SCALAR",
        }));
        self.accessors.len() - 1
    }

    /// The description, with `rest` (nodes, meshes, materials...) added.
    fn json(&mut self, rest: Value) -> Value {
        self.pad();
        let mut gltf = json!({
            "asset": { "version": "2.0" },
            "accessors": self.accessors,
            "bufferViews": self.views,
            "buffers": [{ "byteLength": self.bin.len() }],
        });
        for (key, value) in rest.as_object().unwrap() {
            gltf[key] = value.clone();
        }
        gltf
    }

    /// As a `.glb`: one file.
    fn glb(&mut self, rest: Value) -> Vec<u8> {
        let mut json = serde_json::to_vec(&self.json(rest)).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + 8 + self.bin.len();
        let mut out = Vec::new();
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend((total as u32).to_le_bytes());
        out.extend((json.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(json);
        out.extend((self.bin.len() as u32).to_le_bytes());
        out.extend(b"BIN\0");
        out.extend(&self.bin);
        out
    }

    /// As a `.gltf` with its numbers written inside, as base64.
    fn gltf_inside(&mut self, rest: Value) -> Vec<u8> {
        let mut json = self.json(rest);
        json["buffers"][0]["uri"] = json!(format!(
            "data:application/octet-stream;base64,{}",
            base64(&self.bin)
        ));
        serde_json::to_vec_pretty(&json).unwrap()
    }

    /// As a `.gltf` naming `bin`, a file of its numbers beside it.
    fn gltf_beside(&mut self, rest: Value, uri: &str) -> (Vec<u8>, Vec<u8>) {
        let mut json = self.json(rest);
        json["buffers"][0]["uri"] = json!(uri);
        (serde_json::to_vec_pretty(&json).unwrap(), self.bin.clone())
    }
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

/// A unit box, from -0.5 to 0.5, its corners counter-clockwise seen from
/// outside as glTF wants: 8 points and 36 corner numbers.
fn unit_box() -> (Vec<[f32; 3]>, Vec<u32>) {
    let h = 0.5;
    let points = vec![
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
        [-h, -h, h],
        [h, -h, h],
        [h, h, h],
        [-h, h, h],
    ];
    let quads = [
        [4, 5, 6, 7],
        [1, 0, 3, 2],
        [0, 4, 7, 3],
        [5, 1, 2, 6],
        [3, 7, 6, 2],
        [0, 1, 5, 4],
    ];
    let indices = quads
        .iter()
        .flat_map(|[a, b, c, d]| [*a, *b, *c, *a, *c, *d])
        .collect();
    (points, indices)
}

/// A file with one box, coloured by one material.
fn box_file(write: impl FnOnce(&mut Writer, Value) -> Vec<u8>) -> Vec<u8> {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    let rest = json!({
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices, "material": 0 }] }],
        "materials": [{ "name": "paint", "pbrMetallicRoughness": { "baseColorFactor": [0.5, 0.2, 0.05, 1.0] } }],
    });
    write(&mut writer, rest)
}

fn read(bytes: &[u8], folder: &Path) -> (Option<MeshFile>, Vec<Issue>) {
    let mut issues = Vec::new();
    let mesh = read_gltf(bytes, "thing.gltf", folder, &mut issues);
    (mesh, issues)
}

fn mesh(bytes: &[u8]) -> MeshFile {
    let (mesh, issues) = read(bytes, Path::new("."));
    assert_eq!(issues, Vec::new());
    mesh.unwrap()
}

fn refused(bytes: &[u8]) -> Issue {
    let (mesh, issues) = read(bytes, Path::new("."));
    assert!(mesh.is_none(), "the file must be refused");
    assert_eq!(issues.len(), 1, "{issues:#?}");
    issues[0].clone()
}

/// A triangle's direction (not 1 long), from its corners in order.
fn normal(t: &[[f64; 3]; 3]) -> [f64; 3] {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let (u, v) = (sub(t[1], t[0]), sub(t[2], t[0]));
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Every triangle of a closed shape around `middle` faces outwards, in
/// Duck Forge's order (`(b - a) x (c - a)` pointing out).
fn faces_outwards(mesh: &MeshFile, middle: [f64; 3]) -> bool {
    mesh.triangles.iter().all(|triangle| {
        let n = normal(triangle);
        let centre: Vec<f64> = (0..3)
            .map(|axis| triangle.iter().map(|c| c[axis]).sum::<f64>() / 3.0 - middle[axis])
            .collect();
        n[0] * centre[0] + n[1] * centre[1] + n[2] * centre[2] > 0.0
    })
}

// ---- reading files -----------------------------------------------------------

#[test]
fn a_glb_and_both_kinds_of_gltf_give_the_same_mesh() {
    let from_glb = mesh(&box_file(|w, rest| w.glb(rest)));
    let inside = mesh(&box_file(|w, rest| w.gltf_inside(rest)));
    assert_eq!(from_glb.triangles, inside.triangles);
    assert_eq!(from_glb.triangles.len(), 12);
    assert!(faces_outwards(&from_glb, [0.0, 0.0, 0.0]));
    assert_eq!(from_glb.materials, ["paint"]);
    // The base colour comes back as the usual 0 to 255.
    assert_eq!(from_glb.colors, [Some([188, 124, 63])]);

    // The numbers in a file beside it, whose name has a space.
    let folder = folder("gltf_beside");
    let (json, bin) = box_file_beside("the box.bin");
    std::fs::write(folder.join("the box.bin"), bin).unwrap();
    let (beside, issues) = read(&json, &folder);
    assert_eq!(issues, Vec::new());
    assert_eq!(beside.unwrap().triangles, from_glb.triangles);
}

fn box_file_beside(name: &str) -> (Vec<u8>, Vec<u8>) {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    let rest = json!({
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices, "material": 0 }] }],
        "materials": [{ "name": "paint", "pbrMetallicRoughness": { "baseColorFactor": [0.5, 0.2, 0.05, 1.0] } }],
    });
    writer.gltf_beside(rest, &name.replace(' ', "%20"))
}

#[test]
fn right_is_minus_x_in_the_file_and_the_model_still_faces_forward() {
    // One triangle in front (+z) and to the model's left (+x in glTF),
    // facing forward.
    let mut writer = Writer::default();
    let position = writer.points(&[[1.0, 0.0, 1.0], [2.0, 0.0, 1.0], [1.0, 1.0, 1.0]]);
    let file = writer.glb(json!({
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": position } }] }],
    }));
    let mesh = mesh(&file);
    let triangle = mesh.triangles[0];
    // On the model's left, which is -x in Duck Mold; still in front.
    assert!(triangle.iter().all(|c| c[0] < 0.0 && c[2] == 1.0));
    // It still faces forward, in Duck Forge's order.
    assert!(normal(&triangle)[2] > 0.0);
    // No material: the faces are the default material, with no colour.
    assert_eq!(mesh.materials, ["default"]);
    assert_eq!(mesh.colors, [None]);
}

#[test]
fn corner_numbers_of_every_size_or_none_give_the_same_triangles() {
    let (points, indices) = unit_box();
    let expand: Vec<[f32; 3]> = indices.iter().map(|&i| points[i as usize]).collect();
    let mut results = Vec::new();
    for size in [1, 2, 4, 0] {
        let mut writer = Writer::default();
        let primitive = if size == 0 {
            json!({ "attributes": { "POSITION": writer.points(&expand) } })
        } else {
            let position = writer.points(&points);
            json!({ "attributes": { "POSITION": position }, "indices": writer.indices(&indices, size) })
        };
        let file = writer.glb(json!({
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{ "primitives": [primitive] }],
        }));
        results.push(mesh(&file).triangles);
    }
    assert!(results.iter().all(|r| *r == results[0]), "{results:?}");
}

#[test]
fn strips_and_fans_are_triangles_too() {
    // A square from two triangles, facing forward: as a list, a strip and
    // a fan.
    let points = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let read_as = |mode: u32, indices: &[u32]| {
        let mut writer = Writer::default();
        let position = writer.points(&points);
        let indices = writer.indices(indices, 4);
        let file = writer.glb(json!({
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0 }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices, "mode": mode }] }],
        }));
        mesh(&file).triangles
    };
    let list = read_as(4, &[0, 1, 2, 0, 2, 3]);
    let strip = read_as(5, &[1, 2, 0, 3]);
    let fan = read_as(6, &[0, 1, 2, 3]);
    for triangles in [&list, &strip, &fan] {
        assert_eq!(triangles.len(), 2);
        // Every one faces forward (+z).
        assert!(
            triangles.iter().all(|t| normal(t)[2] > 0.0),
            "{triangles:?}"
        );
    }
}

#[test]
fn objects_are_placed_by_numbers_or_by_a_matrix_and_inside_each_other() {
    let place = |node: Value| {
        let mut writer = Writer::default();
        let (points, indices) = unit_box();
        let position = writer.points(&points);
        let indices = writer.indices(&indices, 2);
        let file = writer.glb(json!({
            "scene": 0,
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "children": [1], "translation": [0.0, 1.0, 0.0] }, node],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices }] }],
        }));
        mesh(&file)
    };
    let half = std::f64::consts::FRAC_1_SQRT_2;
    // Moved 2 to glTF's +x, turned a quarter round y, made 3 tall.
    let by_numbers = place(json!({
        "mesh": 0, "translation": [2.0, 0.0, 0.0], "rotation": [0.0, half, 0.0, half], "scale": [1.0, 3.0, 1.0]
    }));
    let by_matrix = place(json!({
        "mesh": 0,
        "matrix": [0.0, 0.0, -1.0, 0.0,  0.0, 3.0, 0.0, 0.0,  1.0, 0.0, 0.0, 0.0,  2.0, 0.0, 0.0, 1.0]
    }));
    let bounds = |mesh: &MeshFile| {
        let corners: Vec<[f64; 3]> = mesh.triangles.iter().flatten().copied().collect();
        let low = |a: usize| corners.iter().map(|c| c[a]).fold(f64::INFINITY, f64::min);
        let high = |a: usize| {
            corners
                .iter()
                .map(|c| c[a])
                .fold(f64::NEG_INFINITY, f64::max)
        };
        [low(0), low(1), low(2), high(0), high(1), high(2)].map(|n| (n * 1e6).round() / 1e6)
    };
    // glTF's +x is the model's left, -x in Duck Mold; the parent lifted
    // it 1, and it is 3 tall around that.
    assert_eq!(bounds(&by_numbers), [-2.5, -0.5, -0.5, -1.5, 2.5, 0.5]);
    assert_eq!(bounds(&by_matrix), bounds(&by_numbers));
    assert!(faces_outwards(&by_numbers, [-2.0, 1.0, 0.0]));
}

#[test]
fn a_mirrored_object_still_faces_outwards() {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    let file = writer.glb(json!({
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "scale": [-1.0, 1.0, 1.0] }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices }] }],
    }));
    assert!(faces_outwards(&mesh(&file), [0.0, 0.0, 0.0]));
}

#[test]
fn only_the_main_scene_is_read() {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    let meshes =
        json!([{ "primitives": [{ "attributes": { "POSITION": position }, "indices": indices }] }]);
    let file = writer.glb(json!({
        "scene": 1,
        "scenes": [{ "nodes": [0] }, { "nodes": [1] }],
        "nodes": [{ "mesh": 0, "translation": [5.0, 0.0, 0.0] }, { "mesh": 0 }],
        "meshes": meshes,
    }));
    let mesh = mesh(&file);
    assert_eq!(mesh.triangles.len(), 12);
    assert!(mesh.triangles.iter().flatten().all(|c| c[0].abs() <= 0.5));
}

#[test]
fn what_cant_be_built_is_left_out_with_a_note() {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    let file = writer.glb(json!({
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "skin": 0 }],
        "meshes": [{ "primitives": [
            { "attributes": { "POSITION": position }, "indices": indices, "targets": [{ "POSITION": position }] },
            { "attributes": { "POSITION": position }, "mode": 1 }
        ] }],
        "skins": [{ "joints": [0] }],
        "animations": [{ "channels": [], "samplers": [] }],
        "materials": [{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }],
    }));
    let (mesh, issues) = read(&file, Path::new("."));
    assert_eq!(mesh.unwrap().triangles.len(), 12);
    let notes: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
    assert!(
        issues
            .iter()
            .all(|i| !i.is_error() && i.code == Code::Skipped)
    );
    for words in ["animations", "skins", "shape keys", "lines and points"] {
        assert!(
            notes.iter().any(|n| n.contains(words)),
            "{words} in {notes:#?}"
        );
    }
}

#[test]
fn files_duck_mold_cant_read_are_refused_with_a_sentence() {
    // Compressed: the file says it can't be read without the extension.
    let file = box_file(|w, mut rest| {
        rest["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]);
        w.glb(rest)
    });
    let issue = refused(&file);
    assert_eq!(issue.code, Code::Unsupported);
    assert!(
        issue.message.contains("KHR_draco_mesh_compression"),
        "{}",
        issue.message
    );

    // Corners packed into whole numbers.
    let file = box_file(|w, rest| {
        w.accessors[0]["componentType"] = json!(5123);
        w.glb(rest)
    });
    assert_eq!(refused(&file).code, Code::Unsupported);

    // An older glTF.
    let file = box_file(|w, rest| {
        let mut json = w.json(rest);
        json["asset"]["version"] = json!("1.0");
        serde_json::to_vec(&json).unwrap()
    });
    assert!(refused(&file).message.contains("Duck Mold reads glTF 2.0"));

    // Not JSON at all.
    assert_eq!(
        refused(b"solid cube\nfacet normal 0 0 1").code,
        Code::BadMesh
    );

    // A corner number past the end.
    let file = box_file(|w, rest| {
        let wrong = w.indices(&[0, 1, 99], 4);
        let mut rest = rest;
        rest["meshes"][0]["primitives"][0]["indices"] = json!(wrong);
        w.glb(rest)
    });
    assert!(refused(&file).message.contains("corner 99"));

    // A count far past the end of the numbers: refused, not a crash.
    let file = box_file(|w, rest| {
        w.accessors[0]["count"] = json!(u64::MAX / 4);
        w.glb(rest)
    });
    assert_eq!(refused(&file).code, Code::BadMesh);

    // A .glb that is cut short.
    let whole = box_file(|w, rest| w.glb(rest));
    assert!(
        refused(&whole[..whole.len() - 8])
            .message
            .contains("cut short")
    );

    // Objects inside each other in a loop.
    let file = box_file(|w, mut rest| {
        rest["nodes"] = json!([{ "mesh": 0, "children": [1] }, { "children": [0] }]);
        w.glb(rest)
    });
    assert!(refused(&file).message.contains("loop"));
}

#[test]
fn a_missing_bin_file_is_named() {
    let folder = folder("gltf_no_bin");
    let (json, _) = box_file_beside("gone.bin");
    let (mesh, issues) = read(&json, &folder);
    assert!(mesh.is_none());
    assert!(issues[0].message.contains("\"gone.bin\""), "{issues:#?}");
}

// ---- as parts of a recipe ----------------------------------------------------------

/// A warning's message with the test folder taken out of file names,
/// as it reads when the recipe is named from its own folder.
fn plain(message: &str, test: &str) -> String {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let shown = format!("{}/", folder.to_string_lossy().replace('\\', "/"));
    message.replace(&shown, "")
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

/// A file with two boxes side by side, materials `glass` and `frame`.
fn two_materials() -> Vec<u8> {
    let mut writer = Writer::default();
    let (points, indices) = unit_box();
    let position = writer.points(&points);
    let indices = writer.indices(&indices, 2);
    writer.glb(json!({
        "scenes": [{ "nodes": [0, 1] }],
        "nodes": [{ "mesh": 0 }, { "mesh": 1, "translation": [2.0, 0.0, 0.0] }],
        "meshes": [
            { "primitives": [{ "attributes": { "POSITION": position }, "indices": indices, "material": 0 }] },
            { "primitives": [{ "attributes": { "POSITION": position }, "indices": indices, "material": 1 }] }
        ],
        "materials": [
            { "name": "glass", "pbrMetallicRoughness": { "baseColorFactor": [0.2, 0.5, 0.8, 1.0] } },
            { "name": "frame", "pbrMetallicRoughness": { "baseColorFactor": [0.05, 0.05, 0.05, 1.0] } }
        ],
    }))
}

/// How many triangles each palette colour paints, by name.
fn painted(model: &Model) -> Vec<(String, usize)> {
    model
        .palette
        .iter()
        .enumerate()
        .map(|(at, color)| {
            let count = model
                .faces
                .iter()
                .filter(|face| face.color == Some(at))
                .map(|face| face.triangle_count())
                .sum();
            (color.name.clone(), count)
        })
        .collect()
}

#[test]
fn a_material_with_no_colour_in_the_recipe_keeps_its_own_with_a_warning() {
    let file = two_materials();
    let (model, report) = build(
        "gltf_own_colours",
        &[("window.glb", &file)],
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nname = \"window\"\nfile = \"window.glb\"\ncolors = { frame = \"wood\" }\n",
    );
    let model = model.expect("it builds");
    // The frame from the recipe; the glass its own, added to the palette
    // under its name.
    assert_eq!(painted(&model), [("wood".into(), 12), ("glass".into(), 12)]);
    assert_eq!(model.palette[1].rgb, [124, 188, 231]);
    assert_eq!(report.warnings.len(), 1, "{:#?}", report.warnings);
    let warning = &report.warnings[0];
    assert_eq!(warning.code, Code::NoColor);
    assert_eq!(warning.line, Some(4));
    assert_eq!(
        plain(&warning.message, "gltf_own_colours"),
        "the material \"glass\" of \"window.glb\" has no colour in the recipe, so it keeps the file's own, added to the palette under its name: glass #7CBCE7. To choose others, give the part a \"color\", or \"colors\" by material name"
    );

    // A colour for the whole part wins over the file's: no warning.
    let (model, report) = build(
        "gltf_whole_colour",
        &[("window.glb", &file)],
        "version = 0\n[palette]\nwood = \"#B07A3F\"\n[[parts]]\nfile = \"window.glb\"\ncolor = \"wood\"\n",
    );
    assert_eq!(painted(&model.unwrap()), [("wood".into(), 24)]);
    assert_eq!(report.warnings, Vec::new());
}

#[test]
fn a_palette_colour_with_the_materials_name_is_used_instead() {
    let file = two_materials();
    // The palette already has "glass", in another colour: the palette's is
    // used, and the warning says so. "frame" is the file's own.
    let (model, report) = build(
        "gltf_palette_name",
        &[("window.glb", &file)],
        "version = 0\n[palette]\nglass = \"#9CC8E0\"\n[[parts]]\nfile = \"window.glb\"\n",
    );
    let model = model.unwrap();
    assert_eq!(
        painted(&model),
        [("glass".into(), 12), ("frame".into(), 12)]
    );
    assert_eq!(model.palette[0].rgb, [0x9C, 0xC8, 0xE0]);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(
        plain(&report.warnings[0].message, "gltf_palette_name"),
        "the materials \"glass\" and \"frame\" of \"window.glb\" have no colour in the recipe; \"glass\" takes the palette's colour of that name, #9CC8E0, not the file's #7CBCE7; \"frame\" keeps the file's own, #3F3F3F, added to the palette under its name. To choose others, give the part a \"color\", or \"colors\" by material name"
    );

    // The same file used twice adds its colours once, and warns once.
    let (model, report) = build(
        "gltf_twice",
        &[("window.glb", &file)],
        "version = 0\n[[parts]]\nfile = \"window.glb\"\n[[parts]]\nfile = \"window.glb\"\nmove = [5, 0, 0]\n",
    );
    let model = model.unwrap();
    assert_eq!(
        painted(&model),
        [("glass".into(), 24), ("frame".into(), 24)]
    );
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn a_glb_duck_mold_wrote_keeps_its_palette() {
    // A crate, exported, then used as a part of another model.
    let (crate_model, _) = build(
        "gltf_crate_source",
        &[],
        "version = 0\nname = \"crate\"\n[palette]\nwood = \"#B07A3F\"\nplank = \"#8A5A2B\"\n[[parts]]\nshape = \"box\"\ncolor = \"wood\"\ncolors = { top = \"plank\", bottom = \"plank\" }\n",
    );
    let crate_glb = write_glb(&crate_model.unwrap());
    let (model, report) = build(
        "gltf_crate_part",
        &[("crate.glb", &crate_glb)],
        "version = 0\n[[parts]]\nname = \"crate\"\nfile = \"crate.glb\"\n",
    );
    let model = model.unwrap();
    assert_eq!(painted(&model), [("wood".into(), 8), ("plank".into(), 4)]);
    assert_eq!(report.warnings, Vec::new());
    // Its palette names are its face names: one can be changed.
    let (model, _) = build(
        "gltf_crate_recoloured",
        &[("crate.glb", &crate_glb)],
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nfile = \"crate.glb\"\ncolors = { plank = \"red\" }\n",
    );
    let painted = painted(&model.unwrap());
    assert!(painted.contains(&("red".into(), 4)), "{painted:?}");
    // A palette colour of the same name in another colour is a conflict,
    // as for a recipe used as a part.
    let (model, report) = build(
        "gltf_crate_conflict",
        &[("crate.glb", &crate_glb)],
        "version = 0\n[palette]\nwood = \"#000000\"\n[[parts]]\nfile = \"crate.glb\"\n",
    );
    assert!(model.is_none());
    assert_eq!(report.errors[0].code, Code::Conflict);
}

#[test]
fn a_model_exported_and_imported_again_comes_back_where_it_was() {
    let (original, _) = build(
        "gltf_round_trip_source",
        &[],
        "version = 0\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"body\"\nshape = \"box\"\nsize = [1, 2, 0.5]\ncolor = \"red\"\n[[parts]]\nname = \"nose\"\nshape = \"cone\"\nradius = 0.2\nsegments = 6\ncolor = \"red\"\nattach = \"bottom\"\nto = \"body.front\"\n",
    );
    let original = original.unwrap();
    let glb = write_glb(&original);
    let (again, report) = build(
        "gltf_round_trip",
        &[("it.glb", &glb)],
        "version = 0\norigin = \"it.center\"\n[[parts]]\nname = \"it\"\nfile = \"it.glb\"\n",
    );
    assert_eq!(report.errors, Vec::new());
    let again = again.unwrap();
    let corners = |model: &Model| -> Vec<[f64; 3]> {
        model
            .faces
            .iter()
            .flat_map(|face| face.triangles())
            .flatten()
            .map(|c| c.to_array().map(|n| (n * 1e4).round() / 1e4))
            .collect()
    };
    // The same triangles in the same order; the imported part is moved so
    // its middle is the zero point, and the original stands on the ground.
    let shift = original.bounds().unwrap().center().to_array();
    let moved: Vec<[f64; 3]> = corners(&original)
        .iter()
        .map(|c| [0, 1, 2].map(|a| ((c[a] - shift[a]) * 1e4).round() / 1e4))
        .collect();
    assert_eq!(corners(&again), moved);
}

#[test]
fn inspect_and_render_read_a_gltf_with_its_colours() {
    let folder = folder("gltf_inspect");
    std::fs::write(folder.join("window.glb"), two_materials()).unwrap();
    let report = tools::inspect_file(&folder.join("window.glb")).unwrap();
    assert_eq!(report.triangles, 24);
    let names: Vec<&str> = report.materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["glass", "frame"]);
    let palette: Vec<(&str, usize)> = report
        .palette
        .iter()
        .map(|c| (c.name.as_str(), c.triangles))
        .collect();
    assert_eq!(palette, [("glass", 12), ("frame", 12)]);

    // A file Duck Mold can't read is a mistake, exit code 1.
    std::fs::write(folder.join("broken.gltf"), "{ not json").unwrap();
    let failure = tools::inspect_file(&folder.join("broken.gltf")).unwrap_err();
    assert_eq!(failure.exit_code(), 1);
}

#[test]
fn the_example_lamp_is_read_as_its_maker_meant() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/parts");
    let bytes = std::fs::read(root.join("lamp.gltf")).unwrap();
    let (mesh, issues) = read(&bytes, &root);
    assert_eq!(issues, Vec::new());
    let mesh = mesh.unwrap();
    // A base and a pole (12 triangles each), a shade fanned from 10
    // corners round its tip, a bulb of 8 triangles with no corner numbers.
    assert_eq!(mesh.triangles.len(), 42);
    assert_eq!(mesh.materials, ["stand", "shade", "bulb"]);
    // The colours its maker wrote as #RRGGBB come back exactly.
    assert_eq!(
        mesh.colors,
        [
            Some([0x3A, 0x3D, 0x44]),
            Some([0x2E, 0x7D, 0x6B]),
            Some([0xFF, 0xE9, 0xA8])
        ]
    );
    // It stands on the ground, and its shade leans forward (+z).
    let lowest = mesh
        .triangles
        .iter()
        .flatten()
        .map(|c| c[1])
        .fold(f64::INFINITY, f64::min);
    assert!(lowest.abs() < 1e-6, "{lowest}");
    let shade: Vec<[f64; 3]> = mesh
        .triangles
        .iter()
        .zip(&mesh.material_of)
        .filter(|(_, m)| **m == 1)
        .flat_map(|(t, _)| *t)
        .collect();
    let middle_z = shade.iter().map(|c| c[2]).sum::<f64>() / shade.len() as f64;
    assert!(middle_z > 0.05, "{middle_z}");
}
