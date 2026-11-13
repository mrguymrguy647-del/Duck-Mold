//! Step 6: a model is written as a `.glb` that follows glTF's rules and
//! that Duck Forge can read.
//!
//! The file is checked three ways: against glTF's rules, byte by byte
//! (the first tests); by the separate reader written for Duck Forge,
//! which shares no code with the writer; and by Duck Mold's own reader.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::glb::{SRGB_TO_LINEAR, UNPAINTED, read_glb, write_glb};
use duck_mold_core::issue::Code;
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::source::Syntax;

/// The reader written for Duck Forge, exactly as it will be copied there.
#[path = "../../../docs/forge/duck_mold_glb.rs"]
mod forge;

fn model_from(text: &str, syntax: Syntax) -> Model {
    let mut issues = Vec::new();
    let loaded = load_text(text, syntax, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    let errors: Vec<_> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.unwrap()
}

fn built(text: &str) -> Model {
    model_from(text, Syntax::Toml)
}

const ROBOT: &str = r##"
version = 0
name = "robot"

[palette]
steel = "#8A939B"
dark = "#3B4148"
red = "#D0453A"
glow = "#FFE08A"

[define.arm]
shape = "box"
size = [0.16, 0.6, 0.16]
color = "steel"
colors = { bottom = "red" }
[define.arm.sockets]
shoulder = { on = "right", shift = [0, 0.7, 0] }

[define.leg]
shape = "cylinder"
radius = 0.1
height = 0.5
color = "dark"

[[parts]]
name = "body"
shape = "box"
size = [0.7, 0.8, 0.4]
color = "steel"
[parts.sockets]
neck = { on = "top" }
left_arm = { on = "left", shift = [0, 0.6, 0] }
right_arm = { on = "right", shift = [0, 0.6, 0] }
left_leg = { on = "bottom", shift = [-0.5, 0, 0] }
right_leg = { on = "bottom", shift = [0.5, 0, 0] }

[[parts]]
name = "head"
shape = "box"
size = [0.45, 0.35, 0.4]
color = "steel"
colors = { front = "glow" }
attach = "bottom"
to = "body.neck"

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

[[parts]]
name = "left_leg"
use = "leg"
attach = "top"
to = "body.left_leg"

[[parts]]
name = "right_leg"
use = "leg"
attach = "top"
to = "body.right_leg"
"##;

/// A `.glb` taken apart: its description and its block of numbers.
struct Glb {
    json: serde_json::Value,
    bin: Vec<u8>,
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Takes a `.glb` apart, checking the container's rules on the way.
fn open(bytes: &[u8]) -> Glb {
    assert_eq!(&bytes[0..4], b"glTF", "the magic word");
    assert_eq!(u32_at(bytes, 4), 2, "glTF version 2");
    assert_eq!(u32_at(bytes, 8) as usize, bytes.len(), "the total length");
    assert_eq!(bytes.len() % 4, 0, "the file is a multiple of four bytes");
    let json_length = u32_at(bytes, 12) as usize;
    assert_eq!(&bytes[16..20], b"JSON");
    assert_eq!(json_length % 4, 0, "the JSON block is padded to four bytes");
    let json_bytes = &bytes[20..20 + json_length];
    // Padding is spaces, and only at the end.
    assert!(json_bytes.iter().all(|&byte| byte >= 0x20));
    let bin_start = 20 + json_length;
    let bin_length = u32_at(bytes, bin_start) as usize;
    assert_eq!(&bytes[bin_start + 4..bin_start + 8], b"BIN\0");
    assert_eq!(bin_length % 4, 0);
    assert_eq!(
        bin_start + 8 + bin_length,
        bytes.len(),
        "nothing after the numbers"
    );
    Glb {
        json: serde_json::from_slice(json_bytes).expect("the description is JSON"),
        bin: bytes[bin_start + 8..].to_vec(),
    }
}

/// Every part's numbers put together again, as the file had them in one
/// list before Phase 5: the corners in the model (each part's own, added
/// to where its object is), in the file's directions.
struct Whole {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    /// Counted from the first corner of all.
    indices: Vec<u32>,
}

impl Glb {
    /// The numbers of accessor `at`, as decimals.
    fn floats(&self, at: usize) -> Vec<f32> {
        let (start, count) = self.span(at, 5126, "VEC3", 12);
        (0..count * 3)
            .map(|n| {
                f32::from_le_bytes(
                    self.bin[start + n * 4..start + n * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect()
    }

    /// The corners of the triangles of one lump, counted from its own
    /// first corner.
    fn indices_of(&self, primitive: &serde_json::Value) -> Vec<u32> {
        let at = primitive["indices"].as_u64().unwrap() as usize;
        let (start, count) = self.span(at, 5125, "SCALAR", 4);
        (0..count)
            .map(|n| u32_at(&self.bin, start + n * 4))
            .collect()
    }

    /// Where an accessor's numbers start and how many there are, with
    /// glTF's rules about it checked.
    fn span(&self, at: usize, component: u64, kind: &str, size: usize) -> (usize, usize) {
        let accessor = &self.json["accessors"][at];
        assert_eq!(accessor["componentType"], component);
        assert_eq!(accessor["type"], kind);
        let count = accessor["count"].as_u64().unwrap() as usize;
        assert!(count > 0, "glTF has no empty lists");
        let view = &self.json["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
        assert_eq!(view["buffer"], 0);
        let within = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        let start = view["byteOffset"].as_u64().unwrap() as usize + within;
        let length = view["byteLength"].as_u64().unwrap() as usize;
        assert_eq!(start % 4, 0, "numbers start on a four-byte boundary");
        assert_eq!(within % size, 0, "and on a whole entry");
        assert!(
            within + count * size <= length,
            "the view holds the accessor's numbers"
        );
        assert!(start + count * size <= self.bin.len());
        (start, count)
    }

    /// The object each object hangs from.
    fn parents(&self) -> Vec<Option<usize>> {
        let nodes = self.json["nodes"].as_array().unwrap();
        let mut parents = vec![None; nodes.len()];
        for (at, node) in nodes.iter().enumerate() {
            for child in node["children"].as_array().into_iter().flatten() {
                let child = child.as_u64().unwrap() as usize;
                assert!(parents[child].is_none(), "an object hangs from one other");
                parents[child] = Some(at);
            }
        }
        parents
    }

    /// Where an object is in the model, in the file's directions: its
    /// own shift and those of everything it hangs from.
    fn place(&self, node: usize) -> [f64; 3] {
        let parents = self.parents();
        let mut place = [0.0; 3];
        let mut at = Some(node);
        while let Some(node) = at {
            if let Some(shift) = self.json["nodes"][node]["translation"].as_array() {
                for axis in 0..3 {
                    place[axis] += shift[axis].as_f64().unwrap();
                }
            }
            at = parents[node];
        }
        place
    }

    /// The lump of triangles of the part `name`: its object and its
    /// primitive.
    fn part(&self, name: &str) -> (usize, &serde_json::Value) {
        let entry = self.json["extras"]["duck_mold"]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == name)
            .unwrap_or_else(|| panic!("no part {name}"));
        let node = entry["node"].as_u64().unwrap() as usize;
        let mesh = self.json["nodes"][node]["mesh"].as_u64().unwrap() as usize;
        (node, &self.json["meshes"][mesh]["primitives"][0])
    }

    fn attribute(primitive: &serde_json::Value, name: &str) -> usize {
        primitive["attributes"][name].as_u64().unwrap() as usize
    }

    fn whole(&self) -> Whole {
        let mut whole = Whole {
            positions: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        };
        let parts = self.json["extras"]["duck_mold"]["parts"]
            .as_array()
            .unwrap();
        for entry in parts {
            let (node, primitive) = self.part(entry["name"].as_str().unwrap());
            let from = self.place(node);
            let first = whole.positions.len() as u32;
            assert_eq!(
                entry["first_triangle"].as_u64().unwrap() as usize,
                whole.indices.len() / 3,
                "the parts are in the order of their triangles"
            );
            for own in triples(&self.floats(Glb::attribute(primitive, "POSITION"))) {
                whole.positions.push([
                    (from[0] + f64::from(own[0])) as f32,
                    (from[1] + f64::from(own[1])) as f32,
                    (from[2] + f64::from(own[2])) as f32,
                ]);
            }
            whole
                .normals
                .extend(triples(&self.floats(Glb::attribute(primitive, "NORMAL"))));
            whole
                .colors
                .extend(triples(&self.floats(Glb::attribute(primitive, "COLOR_0"))));
            let indices = self.indices_of(primitive);
            assert_eq!(
                indices.len() / 3,
                entry["triangles"].as_u64().unwrap() as usize
            );
            whole
                .indices
                .extend(indices.into_iter().map(|index| first + index));
        }
        whole
    }
}

/// A `.glb` with its JSON block changed by `change`, put back together
/// with the right lengths, so that only what `change` did is different.
fn with_json(glb: &[u8], change: impl Fn(&str) -> String) -> Vec<u8> {
    let json_end = 20 + u32_at(glb, 12) as usize;
    let json = std::str::from_utf8(&glb[20..json_end]).unwrap();
    let mut json = change(json.trim_end()).into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let numbers = &glb[json_end..]; // its length, "BIN\0", the numbers
    let total = 12 + 8 + json.len() + numbers.len();
    let mut out = Vec::new();
    out.extend(b"glTF");
    out.extend(2u32.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(json);
    out.extend(numbers);
    out
}

/// The file with Duck Mold's mark under "extras" renamed, byte for byte
/// (so its lengths still add up): what a .glb from another program looks
/// like to the readers.
fn renamed(glb: &[u8]) -> Vec<u8> {
    let (from, to) = (b"duck_mold", b"duck_mole");
    let mut out = glb.to_vec();
    let at = out
        .windows(from.len())
        .position(|window| window == from)
        .expect("the file has Duck Mold's mark");
    out[at..at + to.len()].copy_from_slice(to);
    out
}

fn triples(numbers: &[f32]) -> Vec<[f32; 3]> {
    numbers.chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
}

#[test]
fn the_file_follows_gltfs_rules() {
    let model = built(ROBOT);
    let glb = open(&write_glb(&model));
    let json = &glb.json;

    assert_eq!(json["asset"]["version"], "2.0");
    assert_eq!(json["asset"]["generator"], "Duck Mold");
    // One scene, with one object in it, the model; under it an object
    // for every part, each hanging from the part it is joined to. This
    // robot's six parts are single shapes: each has its own triangles.
    assert_eq!(json["scene"], 0);
    assert_eq!(json["scenes"].as_array().unwrap().len(), 1);
    assert_eq!(json["scenes"][0]["nodes"], serde_json::json!([0]));
    let nodes = json["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 7);
    assert_eq!(nodes[0]["name"], "robot");
    assert!(nodes[0].get("mesh").is_none());
    assert!(nodes[0].get("translation").is_none());
    // The body isn't joined: it hangs from the model. Everything else
    // is joined to the body.
    assert_eq!(nodes[0]["children"], serde_json::json!([1]));
    assert_eq!(nodes[1]["name"], "body");
    assert_eq!(nodes[1]["children"], serde_json::json!([2, 3, 4, 5, 6]));
    let names: Vec<&str> = nodes
        .iter()
        .map(|node| node["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names[2..],
        ["head", "left_arm", "right_arm", "left_leg", "right_leg"]
    );
    // Nothing is turned or stretched at rest: an object is only
    // somewhere.
    for node in nodes {
        for absent in ["rotation", "scale", "matrix"] {
            assert!(node.get(absent).is_none(), "{absent}");
        }
    }
    assert_eq!(json["meshes"].as_array().unwrap().len(), 6);
    for (at, mesh) in json["meshes"].as_array().unwrap().iter().enumerate() {
        assert_eq!(nodes[at + 1]["mesh"], at, "each part's object has its mesh");
        assert_eq!(mesh["name"], nodes[at + 1]["name"]);
        let primitives = mesh["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 1);
        assert_eq!(primitives[0]["mode"], 4, "triangles");
        assert_eq!(primitives[0]["material"], 0);
        assert!(primitives[0]["attributes"].get("TEXCOORD_0").is_none());
    }
    // A plain, matt material (glTF's default would be shiny metal).
    assert_eq!(json["materials"].as_array().unwrap().len(), 1);
    let surface = &json["materials"][0]["pbrMetallicRoughness"];
    assert_eq!(surface["metallicFactor"], 0.0);
    assert_eq!(surface["roughnessFactor"], 1.0);
    assert_eq!(
        surface["baseColorFactor"],
        serde_json::json!([1.0, 1.0, 1.0, 1.0])
    );
    // Nothing this phase doesn't do.
    for absent in [
        "textures",
        "images",
        "samplers",
        "animations",
        "skins",
        "cameras",
    ] {
        assert!(json.get(absent).is_none(), "{absent}");
    }

    // One buffer, the block of numbers, and four views that fill it.
    assert_eq!(json["buffers"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
        glb.bin.len()
    );
    assert!(json["buffers"][0].get("uri").is_none());
    let views = json["bufferViews"].as_array().unwrap();
    assert_eq!(views.len(), 4);
    let mut next = 0;
    for view in views {
        assert_eq!(view["byteOffset"].as_u64().unwrap(), next);
        next += view["byteLength"].as_u64().unwrap();
    }
    assert_eq!(next as usize, glb.bin.len());
    assert_eq!(views[0]["target"], 34962);
    assert_eq!(views[3]["target"], 34963);
    // The parts share the three lists of corners, so glTF wants to be
    // told how far apart the entries are; not for the triangles.
    for view in &views[..3] {
        assert_eq!(view["byteStride"], 12);
    }
    assert!(views[3].get("byteStride").is_none());

    let Whole {
        positions,
        normals,
        colors,
        indices,
    } = glb.whole();
    assert_eq!(positions.len(), model.vertex_count());
    assert_eq!(normals.len(), positions.len());
    assert_eq!(colors.len(), positions.len());
    assert_eq!(indices.len(), model.triangle_count() * 3);
    assert_eq!((positions.len(), indices.len() / 3), (288, 168));

    // glTF insists on the smallest and biggest position of every list,
    // exactly.
    for mesh in json["meshes"].as_array().unwrap() {
        let at = Glb::attribute(&mesh["primitives"][0], "POSITION");
        let own = triples(&glb.floats(at));
        let accessor = &json["accessors"][at];
        for axis in 0..3 {
            let smallest = own.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
            let biggest = own
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!(accessor["min"][axis].as_f64().unwrap() as f32, smallest);
            assert_eq!(accessor["max"][axis].as_f64().unwrap() as f32, biggest);
        }
    }
    // Directions are 1 long; colours are between 0 and 1; every triangle
    // names corners that exist.
    for normal in &normals {
        let length = normal.iter().map(|n| n * n).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-5, "{normal:?}");
    }
    assert!(colors.iter().flatten().all(|c| (0.0..=1.0).contains(c)));
    assert!(
        indices
            .iter()
            .all(|&index| (index as usize) < positions.len())
    );
}

#[test]
fn triangles_wind_the_way_gltf_wants() {
    // In glTF a triangle's front is the side its corners go round
    // counter-clockwise, which is the side `(b - a) x (c - a)` points
    // to. It must agree with the stored direction of the face.
    let glb = open(&write_glb(&built(ROBOT)));
    let Whole {
        positions,
        normals,
        indices,
        ..
    } = glb.whole();
    for triangle in indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|n| positions[triangle[n] as usize]);
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cross = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let normal = normals[triangle[0] as usize];
        let agree = cross[0] * normal[0] + cross[1] * normal[1] + cross[2] * normal[2];
        assert!(agree > 0.0, "a triangle faces the wrong way");
        // All three corners of a triangle share one direction and one
        // colour: flat shading.
        assert_eq!(normals[triangle[1] as usize], normal);
        assert_eq!(normals[triangle[2] as usize], normal);
    }
}

#[test]
fn right_is_minus_x_in_the_file_and_the_model_still_faces_forward() {
    // A part moved to the right (+x) is at -x in the file; forward (+z)
    // and up (+y) are unchanged.
    let model = built(
        "version = 0\norigin = \"a.center\"\n[palette]\nred = \"#D0453A\"\n[[parts]]\nname = \"a\"\nshape = \"box\"\ncolor = \"red\"\n[[parts]]\nname = \"b\"\nshape = \"box\"\nsize = [0.2, 0.2, 0.2]\ncolor = \"red\"\nmove = [3, 1, 2]\n",
    );
    let glb = open(&write_glb(&model));
    let positions = glb.whole().positions;
    // The second box's corners come after the first's 24.
    let span = |axis: usize| {
        let values = positions[24..].iter().map(|p| p[axis]);
        (
            values.clone().fold(f32::INFINITY, f32::min),
            values.fold(f32::NEG_INFINITY, f32::max),
        )
    };
    assert_eq!(span(0), (-3.1, -2.9));
    assert_eq!(span(1), (0.9, 1.1));
    assert_eq!(span(2), (1.9, 2.1));
    // Its object is at its middle, which it turns about (it isn't
    // joined), and its corners are measured from there.
    let (node, primitive) = glb.part("b");
    assert_eq!(
        glb.json["nodes"][node]["translation"],
        serde_json::json!([-3.0, 1.0, 2.0])
    );
    let accessor = &glb.json["accessors"][Glb::attribute(primitive, "POSITION")];
    assert_eq!(accessor["min"][0].as_f64().unwrap() as f32, -0.1);
    assert_eq!(accessor["max"][0].as_f64().unwrap() as f32, 0.1);
}

#[test]
fn the_palette_and_the_parts_are_kept_in_the_file() {
    let model = built(ROBOT);
    let glb = open(&write_glb(&model));
    let extras = &glb.json["extras"]["duck_mold"];
    assert_eq!(extras["version"], 1);
    assert_eq!(extras["name"], "robot");
    assert_eq!(
        extras["palette"],
        serde_json::json!([
            { "name": "steel", "hex": "#8A939B" },
            { "name": "dark", "hex": "#3B4148" },
            { "name": "red", "hex": "#D0453A" },
            { "name": "glow", "hex": "#FFE08A" }
        ])
    );
    assert_eq!(
        extras["parts"],
        serde_json::json!([
            { "name": "body", "first_triangle": 0, "triangles": 12, "node": 1 },
            { "name": "head", "first_triangle": 12, "triangles": 12, "node": 2 },
            { "name": "left_arm", "first_triangle": 24, "triangles": 12, "node": 3 },
            { "name": "right_arm", "first_triangle": 36, "triangles": 12, "node": 4 },
            { "name": "left_leg", "first_triangle": 48, "triangles": 60, "node": 5 },
            { "name": "right_leg", "first_triangle": 108, "triangles": 60, "node": 6 }
        ])
    );
    // This robot's arms and legs are single shapes: no assemblies.
    assert_eq!(extras["assemblies"], serde_json::json!([]));
}

/// A tower: a base, and a column made of two parts, the top one itself
/// made of two parts. Parts inside parts, two levels deep.
const TOWER: &str = r##"
version = 0
name = "tower"

[palette]
stone = "#8C8C88"
gold = "#E0B040"

[define.top]
color = "gold"
[[define.top.parts]]
name = "cap"
shape = "box"
size = [0.6, 0.2, 0.6]
[[define.top.parts]]
name = "spire"
shape = "cone"
segments = 4
attach = "bottom"
to = "cap.top"

[[parts]]
name = "base"
shape = "box"
size = [2, 0.2, 2]
color = "stone"

[[parts]]
name = "column"
color = "stone"
attach = "bottom"
to = "base.top"
[[parts.parts]]
name = "shaft"
shape = "cylinder"
segments = 8
radius = 0.2
height = 2
[[parts.parts]]
name = "top"
use = "top"
attach = "bottom"
to = "shaft.top"
"##;

#[test]
fn assemblies_are_kept_with_the_triangles_of_every_part_inside_them() {
    let model = built(TOWER);
    let glb = open(&write_glb(&model));
    let extras = &glb.json["extras"]["duck_mold"];
    // The parts with shapes of their own, one after the other...
    assert_eq!(
        extras["parts"],
        serde_json::json!([
            { "name": "base", "first_triangle": 0, "triangles": 12, "node": 1 },
            { "name": "column/shaft", "first_triangle": 12, "triangles": 28, "node": 3 },
            { "name": "column/top/cap", "first_triangle": 40, "triangles": 12, "node": 5 },
            { "name": "column/top/spire", "first_triangle": 52, "triangles": 6, "node": 6 }
        ])
    );
    // ...and the parts made of parts, outer first, each owning all the
    // triangles of the parts inside it.
    assert_eq!(
        extras["assemblies"],
        serde_json::json!([
            { "name": "column", "first_triangle": 12, "triangles": 46, "node": 2 },
            { "name": "column/top", "first_triangle": 40, "triangles": 18, "node": 4 }
        ])
    );
    // The tree follows the joins: the column stands on the base; its
    // shaft is inside it; its top is joined to the shaft; the cap is
    // inside the top, and the spire stands on the cap.
    let children = |node: usize| glb.json["nodes"][node]["children"].clone();
    assert_eq!(children(0), serde_json::json!([1]));
    assert_eq!(children(1), serde_json::json!([2]));
    assert_eq!(children(2), serde_json::json!([3]));
    assert_eq!(children(3), serde_json::json!([4]));
    assert_eq!(children(4), serde_json::json!([5]));
    assert_eq!(children(5), serde_json::json!([6]));
    // A part made of parts has no triangles of its own.
    assert!(glb.json["nodes"][2].get("mesh").is_none());

    // The Duck Forge reader gives them too.
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    let assemblies: Vec<(&str, usize, usize)> = read
        .assemblies
        .iter()
        .map(|a| (a.name.as_str(), a.first_triangle, a.triangles))
        .collect();
    assert_eq!(assemblies, [("column", 12, 46), ("column/top", 40, 18)]);
    // Recolouring an assembly's range recolours exactly its parts: all of
    // them gold or stone before, the column's 46 triangles are the ones in
    // its range.
    let column = &read.assemblies[0];
    let inside: usize = read
        .parts
        .iter()
        .filter(|part| part.name.starts_with("column/"))
        .map(|part| part.triangles)
        .sum();
    assert_eq!(inside, column.triangles);
}

#[test]
fn duck_mold_reads_the_tree_of_parts_back() {
    let model = built(TOWER);
    let read = read_glb(&write_glb(&model), "tower.glb").unwrap();
    // The same tree: names, paths, kinds and faces.
    fn shape(parts: &[duck_mold_core::model::Part]) -> Vec<String> {
        parts
            .iter()
            .map(|part| {
                let kind = if part.parts.is_empty() {
                    "part"
                } else {
                    "assembly"
                };
                format!(
                    "{} {} {}..{} [{}]",
                    part.path,
                    kind,
                    part.faces.start,
                    part.faces.end,
                    shape(&part.parts).join(", ")
                )
            })
            .collect()
    }
    assert_eq!(shape(&read.parts), shape(&model.parts));
    assert_eq!(
        shape(&read.parts),
        [
            "base part 0..6 []",
            "column assembly 6..27 [column/shaft part 6..16 [], column/top assembly 16..27 [column/top/cap part 16..22 [], column/top/spire part 22..27 []]]"
        ]
    );
    assert_eq!(read.parts[1].kind.shape, "assembly");
    assert_eq!(read.parts[1].parts[1].name, "top");
    assert_eq!(read.parts[1].bounds, model.parts[1].bounds);
    // And writing it again gives the same file, assemblies included.
    assert_eq!(write_glb(&read), write_glb(&model));
}

/// A `.glb` as Duck Mold wrote it before Phase 5: one object, one list
/// of triangles. Kept from Phase 4's stored files.
fn from_before(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/before-phase5")
        .join(format!("{name}.glb"));
    std::fs::read(path).unwrap()
}

/// The example `name`, built now.
fn example(name: &str) -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(format!("{name}.toml"));
    duck_mold_core::tools::build(&path).unwrap().model
}

#[test]
fn files_from_before_the_tree_still_read_and_give_the_same_model() {
    for name in ["crate", "house", "robot"] {
        let old = from_before(name);
        assert_eq!(open(&old).json["extras"]["duck_mold"]["version"], 0);
        assert_eq!(open(&old).json["nodes"].as_array().unwrap().len(), 1);
        let new = write_glb(&example(name));

        // Duck Forge's reader: the same triangles in the same order with
        // the same colours, the same parts, assemblies and sockets. A
        // corner is now its part's pivot plus its place in the part,
        // added up on reading, so it can differ in its last digit.
        let (before, now) = (
            forge::read_duck_mold_glb(&old).unwrap(),
            forge::read_duck_mold_glb(&new).unwrap(),
        );
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-6);
        assert_eq!(before.triangles.len(), now.triangles.len(), "{name}");
        for (old, new) in before.triangles.iter().zip(&now.triangles) {
            assert_eq!(old.color, new.color, "{name}");
            for corner in 0..3 {
                assert!(
                    near(
                        before.vertices[old.corners[corner]],
                        now.vertices[new.corners[corner]]
                    ),
                    "{name}"
                );
            }
        }
        assert_eq!(before.name, now.name);
        assert_eq!(before.palette, now.palette);
        assert_eq!(before.parts, now.parts, "{name}");
        assert_eq!(before.assemblies, now.assemblies, "{name}");
        assert_eq!(before.sockets.len(), now.sockets.len());
        for (old, new) in before.sockets.iter().zip(&now.sockets) {
            assert_eq!(old.name, new.name);
            assert!(
                near(old.at, new.at) && near(old.facing, new.facing),
                "{}",
                old.name
            );
            // Only a file laid out as a tree says what carries a socket.
            assert_eq!(old.join, None);
        }
        // An old file can be drawn, not posed: it has no joins.
        assert_eq!(before.joins, Vec::new());
        assert!(!now.joins.is_empty());

        // Duck Mold's own reader: the same faces and the same tree of
        // parts.
        let (before, now) = (
            read_glb(&old, "old.glb").unwrap(),
            read_glb(&new, "new.glb").unwrap(),
        );
        assert_eq!(before.faces, now.faces, "{name}");
        assert_eq!(before.palette, now.palette);
        let paths = |model: &Model| -> Vec<(String, std::ops::Range<usize>)> {
            model
                .all_parts()
                .iter()
                .map(|part| (part.path.clone(), part.faces.clone()))
                .collect()
        };
        assert_eq!(paths(&before), paths(&now), "{name}");
        assert_eq!(before.named_sockets(), now.named_sockets(), "{name}");
    }
}

#[test]
fn a_file_from_before_assemblies_still_reads() {
    // Phase 0's files had no "assemblies": both readers take that as none,
    // and Duck Mold lists the parts flat, as it did then.
    let old = with_json(&from_before("house"), |json| {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        value["extras"]["duck_mold"]
            .as_object_mut()
            .unwrap()
            .remove("assemblies");
        serde_json::to_string(&value).unwrap()
    });
    let read = forge::read_duck_mold_glb(&old).unwrap();
    assert_eq!(read.assemblies, Vec::new());
    assert_eq!(read.parts.len(), 8);
    let model = read_glb(&old, "old.glb").unwrap();
    let names: Vec<&str> = model.parts.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(
        names,
        [
            "walls",
            "roof/front_half",
            "roof/back_half",
            "door",
            "window_left",
            "window_right",
            "chimney",
            "skylight"
        ]
    );
}

#[test]
fn a_file_from_a_newer_duck_mold_is_refused_with_a_sentence() {
    let newer = with_json(&write_glb(&built(TOWER)), |json| {
        assert!(json.contains("\"version\":1,\"name\":\"tower\""));
        json.replacen("\"version\":1,\"name\"", "\"version\":2,\"name\"", 1)
    });
    let error = forge::read_duck_mold_glb(&newer).unwrap_err();
    assert!(error.contains("newer Duck Mold"), "{error}");
    let error = read_glb(&newer, "newer.glb").unwrap_err();
    assert!(
        error.message.contains("newer Duck Mold"),
        "{}",
        error.message
    );
}

#[test]
fn a_damaged_file_that_claims_too_much_is_refused_not_a_crash() {
    // Sizes inside the file are checked before they are added or
    // multiplied: a damaged one is a refusal, not an overflow (which a
    // debug build turns into a crash).
    let glb = write_glb(&built(TOWER));
    // (The base is a box: 24 corners, the first list of all.)
    let count = "\"count\":24".to_owned();
    for huge in ["99999", "4611686018427387904", "18446744073709551615"] {
        let damaged = with_json(&glb, |json| {
            assert!(json.contains(&count), "the first count is the corners'");
            json.replacen(&count, &format!("\"count\":{huge}"), 1)
        });
        let error = forge::read_duck_mold_glb(&damaged).unwrap_err();
        assert!(error.contains("runs past the end of the file"), "{error}");
        let error = read_glb(&damaged, "huge.glb").unwrap_err();
        assert_eq!(error.code, Code::BadMesh);
    }
    for (list, entry) in [
        (
            "parts",
            "\"name\":\"base\",\"first_triangle\":0,\"triangles\":12",
        ),
        (
            "assemblies",
            "\"name\":\"column\",\"first_triangle\":12,\"triangles\":46",
        ),
    ] {
        let damaged = with_json(&glb, |json| {
            assert!(json.contains(entry), "{list}");
            json.replacen(entry, &entry.replace(":12", ":18446744073709551615"), 1)
        });
        let error = forge::read_duck_mold_glb(&damaged).unwrap_err();
        assert!(
            error.contains("names triangles that aren't there"),
            "{list}: {error}"
        );
        let error = read_glb(&damaged, "parts.glb").unwrap_err();
        assert!(
            error.message.contains("doesn't match its triangles"),
            "{list}"
        );
    }
}

// ---- the reader for Duck Forge -----------------------------------------------

#[test]
fn the_duck_forge_reader_finds_the_same_model() {
    let model = built(ROBOT);
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    assert_eq!(read.name, "robot");

    // The same triangles, in the same order, with corners in Duck
    // Forge's order and in Duck Forge's directions.
    let expected: Vec<([[f32; 3]; 3], [u8; 3])> = model
        .faces
        .iter()
        .flat_map(|face| {
            let rgb = face
                .color
                .map_or(UNPAINTED, |color| model.palette[color].rgb);
            face.triangles()
                .map(move |corners| (corners.map(|c| [c.x as f32, c.y as f32, c.z as f32]), rgb))
        })
        .collect();
    let found: Vec<([[f32; 3]; 3], [u8; 3])> = read
        .triangles
        .iter()
        .map(|triangle| {
            (
                triangle.corners.map(|corner| read.vertices[corner]),
                triangle.color,
            )
        })
        .collect();
    assert_eq!(found.len(), 168);
    // (A corner is its part's pivot plus its place in the part, added up
    // by the reader: the same to a ten-thousandth of a millimetre.)
    for (found, expected) in found.iter().zip(&expected) {
        assert_eq!(found.1, expected.1);
        for (found, expected) in found.0.iter().flatten().zip(expected.0.iter().flatten()) {
            assert!((found - expected).abs() < 1e-7, "{found} {expected}");
        }
    }

    // A part's corners in the same place are shared: far fewer points
    // than the file's 288 corners.
    assert!(read.vertices.len() < 150, "{}", read.vertices.len());

    let palette: Vec<(&str, [u8; 3])> = read
        .palette
        .iter()
        .map(|c| (c.name.as_str(), c.rgb))
        .collect();
    assert_eq!(
        palette,
        [
            ("steel", [0x8A, 0x93, 0x9B]),
            ("dark", [0x3B, 0x41, 0x48]),
            ("red", [0xD0, 0x45, 0x3A]),
            ("glow", [0xFF, 0xE0, 0x8A])
        ]
    );
    let parts: Vec<(&str, usize, usize)> = read
        .parts
        .iter()
        .map(|part| (part.name.as_str(), part.first_triangle, part.triangles))
        .collect();
    assert_eq!(parts[0], ("body", 0, 12));
    assert_eq!(parts[5], ("right_leg", 108, 60));
    // The parts own every triangle, one after the other.
    let mut next = 0;
    for (_, first, count) in parts {
        assert_eq!(first, next);
        next += count;
    }
    assert_eq!(next, read.triangles.len());
}

#[test]
fn what_duck_forge_reads_passes_duck_forges_own_mesh_test() {
    // Duck Forge's test for its built-in meshes: every triangle's front
    // (clockwise corners; `(b - a) x (c - a)` in numbers) faces away
    // from the middle of a solid shape.
    for shape in ["box", "cylinder", "sphere", "cone", "wedge"] {
        let model = built(&format!(
            "version = 0\norigin = \"center\"\n[palette]\nred = \"#D0453A\"\n[[parts]]\nshape = \"{shape}\"\ncolor = \"red\"\n"
        ));
        let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
        // The wedge's middle is on its slope, so measure from a point
        // safely inside it.
        let inside = if shape == "wedge" {
            [0.0, -0.25, -0.25]
        } else {
            [0.0; 3]
        };
        for triangle in &read.triangles {
            let [a, b, c] = triangle.corners.map(|corner| read.vertices[corner]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let normal = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            let middle = [0, 1, 2].map(|axis| (a[axis] + b[axis] + c[axis]) / 3.0 - inside[axis]);
            let outwards = normal[0] * middle[0] + normal[1] * middle[1] + normal[2] * middle[2];
            assert!(outwards > 0.0, "{shape}: a triangle faces inwards");
        }
        // A box has its 8 corners and 12 triangles again, like Duck
        // Forge's own cube.
        if shape == "box" {
            assert_eq!((read.vertices.len(), read.triangles.len()), (8, 12));
        }
    }
}

#[test]
fn every_colour_survives_the_trip_exactly() {
    // 256 greys, one per small box: every value a channel can have.
    let mut recipe = String::from("version = 0\n[palette]\n");
    for value in 0..256 {
        recipe += &format!("g{value} = \"#{value:02X}{value:02X}{value:02X}\"\n");
    }
    for value in 0..256 {
        recipe += &format!(
            "[[parts]]\nshape = \"box\"\nsize = [0.1, 0.1, 0.1]\ncolor = \"g{value}\"\nmove = [{}, 0, 0]\n",
            value as f64 * 0.2
        );
    }
    let read = forge::read_duck_mold_glb(&write_glb(&built(&recipe))).unwrap();
    assert_eq!(read.triangles.len(), 256 * 12);
    for (value, part) in read.parts.iter().enumerate() {
        let value = value as u8;
        for triangle in &read.triangles[part.first_triangle..part.first_triangle + part.triangles] {
            assert_eq!(triangle.color, [value; 3]);
        }
    }
    // The curve itself, for the record: mid grey is about a fifth as
    // bright as white in linear light.
    assert!((SRGB_TO_LINEAR[128] - 0.2158605).abs() < 1e-6);
}

#[test]
fn a_face_nobody_coloured_is_grey() {
    let model = built("version = 0\n[[parts]]\nshape = \"box\"\n");
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    assert!(
        read.triangles
            .iter()
            .all(|triangle| triangle.color == [180, 180, 180])
    );
    assert_eq!(read.palette, Vec::new());
}

#[test]
fn the_duck_forge_reader_refuses_what_it_cannot_read() {
    let glb = write_glb(&built(ROBOT));
    assert!(
        forge::read_duck_mold_glb(b"not a model")
            .unwrap_err()
            .contains("doesn't start with \"glTF\"")
    );
    assert!(
        forge::read_duck_mold_glb(&glb[..glb.len() - 8])
            .unwrap_err()
            .contains("cut short")
    );
    // A file without Duck Mold's extras.
    let error = forge::read_duck_mold_glb(&renamed(&glb)).unwrap_err();
    assert!(error.contains("wasn't written by Duck Mold"), "{error}");
}

// ---- the same recipe, the same bytes -----------------------------------------

#[test]
fn the_same_recipe_gives_the_same_bytes() {
    let first = write_glb(&built(ROBOT));
    let second = write_glb(&built(ROBOT));
    assert_eq!(first, second);
    // No date, no folder name, nothing from this computer.
    let text = String::from_utf8_lossy(&first);
    assert!(!text.contains(env!("CARGO_MANIFEST_DIR")));
    assert!(!text.contains("test.toml"));
}

#[test]
fn toml_and_json_give_the_same_bytes() {
    let toml = "version = 0\nname = \"crate\"\n[palette]\nwood = \"#B07A3F\"\nplank = \"#8A5A2B\"\n[[parts]]\nshape = \"box\"\nsize = [1, 1, 1]\ncolor = \"wood\"\ncolors = { top = \"plank\", bottom = \"plank\" }\n";
    let json = r##"{
  "version": 0,
  "name": "crate",
  "palette": { "wood": "#B07A3F", "plank": "#8A5A2B" },
  "parts": [
    { "shape": "box", "size": [1, 1, 1], "color": "wood",
      "colors": { "top": "plank", "bottom": "plank" } }
  ]
}"##;
    assert_eq!(
        write_glb(&model_from(toml, Syntax::Toml)),
        write_glb(&model_from(json, Syntax::Json))
    );
}

// ---- Duck Mold's own reader --------------------------------------------------

#[test]
fn duck_mold_reads_its_own_file_back() {
    let model = built(ROBOT);
    let read = read_glb(&write_glb(&model), "robot.glb").unwrap();
    assert_eq!(read.name, "robot");
    // The same faces, corner for corner, and the same colours.
    assert_eq!(read.faces, model.faces);
    assert_eq!(read.triangle_count(), 168);
    assert_eq!(read.vertex_count(), 288);
    let names: Vec<&str> = read
        .palette
        .iter()
        .map(|color| color.name.as_str())
        .collect();
    assert_eq!(names, ["steel", "dark", "red", "glow"]);
    // The parts come back as a flat list of the parts that have faces.
    let parts: Vec<(&str, std::ops::Range<usize>)> = read
        .parts
        .iter()
        .map(|part| (part.path.as_str(), part.faces.clone()))
        .collect();
    let leaves: Vec<(&str, std::ops::Range<usize>)> = model
        .leaves()
        .iter()
        .map(|part| (part.path.as_str(), part.faces.clone()))
        .collect();
    assert_eq!(parts, leaves);
    assert_eq!(read.parts[0].bounds, model.parts[0].bounds);
    // And writing that again gives the same file.
    assert_eq!(write_glb(&read), write_glb(&model));
}

#[test]
fn duck_mold_refuses_files_it_did_not_write() {
    let error = read_glb(b"hello", "x.glb").unwrap_err();
    assert_eq!(error.code, Code::BadMesh);
    assert_eq!(error.file, "x.glb");

    // A real .glb from somewhere else: no Duck Mold extras.
    let glb = write_glb(&built(ROBOT));
    let foreign = renamed(&glb);
    let error = read_glb(&foreign, "duck.glb").unwrap_err();
    assert_eq!(error.code, Code::Unsupported);
    assert!(error.message.contains("wasn't written by Duck Mold"));

    let error = read_glb(&glb[..glb.len() - 100], "cut.glb").unwrap_err();
    assert!(error.message.contains("cut short"));
}

// ---- Phase 1, step 5: sockets in the file ----------------------------------

/// The `.glb`'s sockets: name, where and facing, as written.
fn file_sockets(glb: &[u8]) -> Vec<(String, [f64; 3], [f64; 3])> {
    let three = |value: &serde_json::Value| -> [f64; 3] {
        let list = value.as_array().unwrap();
        [0, 1, 2].map(|at| list[at].as_f64().unwrap())
    };
    open(glb).json["extras"]["duck_mold"]["sockets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["name"].as_str().unwrap().to_owned(),
                three(&entry["at"]),
                three(&entry["facing"]),
            )
        })
        .collect()
}

#[test]
fn every_named_socket_is_in_the_file_in_gltfs_directions() {
    let model = built(ROBOT);
    let glb = write_glb(&model);
    let sockets = file_sockets(&glb);
    let names: Vec<&str> = sockets.iter().map(|(name, _, _)| name.as_str()).collect();
    // The model's own first, by bare name; then each part's, the seven
    // built-in ones and then its own.
    assert_eq!(
        names[..9],
        [
            "top",
            "bottom",
            "left",
            "right",
            "front",
            "back",
            "center",
            "body.top",
            "body.bottom"
        ]
    );
    for name in [
        "body.neck",
        "head.top",
        "left_arm.shoulder",
        "right_arm.shoulder",
        "left_leg.bottom",
    ] {
        assert!(names.contains(&name), "{name} is missing from {names:?}");
    }
    // Every part's seven, and the five the body declares, and the arms'
    // shoulders.
    assert_eq!(sockets.len(), 7 + 6 * 7 + 5 + 2);

    // In the file, x is negated, as for the corners.
    let model_sockets = model.named_sockets();
    for ((name, at, facing), socket) in sockets.iter().zip(&model_sockets) {
        assert_eq!(*name, socket.name);
        assert_eq!(*at, [-socket.at.x + 0.0, socket.at.y, socket.at.z]);
        assert_eq!(
            *facing,
            [-socket.facing.x + 0.0, socket.facing.y, socket.facing.z]
        );
    }
    let (_, at, facing) = &sockets[names
        .iter()
        .position(|n| *n == "left_arm.shoulder")
        .unwrap()];
    // The left arm is on the model's left, -x, which is +x in the file.
    assert!(at[0] > 0.0, "{at:?}");
    assert_eq!(*facing, [-1.0, 0.0, 0.0]);
}

#[test]
fn unnamed_parts_have_no_sockets_in_the_file() {
    let model = built(
        "version = 0\n[[parts]]\nname = \"post\"\nshape = \"box\"\n[[parts]]\nshape = \"box\"\nmove = [2, 0, 0]\n[[parts]]\nmove = [4, 0, 0]\n[[parts.parts]]\nname = \"inside\"\nshape = \"box\"\n",
    );
    let names: Vec<String> = file_sockets(&write_glb(&model))
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    assert!(names.contains(&"post.top".to_owned()));
    assert!(names.iter().all(|name| !name.contains('#')), "{names:?}");
    // The model's own seven and the post's seven: nothing from the
    // unnamed part, nor from the part inside the unnamed assembly.
    assert_eq!(names.len(), 14);
}

#[test]
fn the_duck_forge_reader_finds_the_sockets_where_duck_mold_put_them() {
    let model = built(ROBOT);
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    let expected: Vec<(String, [f32; 3], [f32; 3])> = model
        .named_sockets()
        .into_iter()
        .map(|socket| {
            let f = |v: duck_mold_core::math::Vec3| [v.x as f32, v.y as f32, v.z as f32];
            (socket.name, f(socket.at), f(socket.facing))
        })
        .collect();
    let found: Vec<(String, [f32; 3], [f32; 3])> = read
        .sockets
        .iter()
        .map(|socket| (socket.name.clone(), socket.at, socket.facing))
        .collect();
    assert_eq!(found, expected);
    // The head's top is the top of the model, in the middle.
    let top = read.sockets.iter().find(|s| s.name == "head.top").unwrap();
    assert_eq!(top.facing, [0.0, 1.0, 0.0]);
    let highest = read
        .vertices
        .iter()
        .map(|v| v[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(top.at[1], highest);
}

#[test]
fn duck_mold_reads_the_sockets_back() {
    let model = built(ROBOT);
    let read = read_glb(&write_glb(&model), "robot.glb").unwrap();
    assert_eq!(read.named_sockets(), model.named_sockets());
    assert_eq!(write_glb(&read), write_glb(&model));
}

#[test]
fn a_file_from_before_sockets_still_reads() {
    let new = write_glb(&built(ROBOT));
    let old = with_json(&new, |json| {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        value["extras"]["duck_mold"]
            .as_object_mut()
            .unwrap()
            .remove("sockets");
        serde_json::to_string(&value).unwrap()
    });
    assert_eq!(forge::read_duck_mold_glb(&old).unwrap().sockets, Vec::new());
    assert_eq!(
        read_glb(&old, "old.glb").unwrap().named_sockets(),
        Vec::new()
    );
}

#[test]
fn a_damaged_socket_is_refused_by_both_readers() {
    let new = write_glb(&built(ROBOT));
    let damaged = with_json(&new, |json| {
        json.replacen("\"facing\":[0.0,1.0,0.0]", "\"facing\":[0.0,1.0]", 1)
    });
    assert_ne!(damaged, new, "the change was made");
    assert_eq!(
        forge::read_duck_mold_glb(&damaged).unwrap_err(),
        "the socket \"top\" has no facing"
    );
    let issue = read_glb(&damaged, "damaged.glb").unwrap_err();
    assert_eq!(issue.code, Code::BadMesh);
}
