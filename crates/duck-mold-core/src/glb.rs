//! Writing a model as a `.glb` file, and reading one back.
//!
//! A `.glb` is the one-file form of glTF 2.0, the common 3D format. It is
//! a 12-byte header, a block of JSON that describes the model, and a
//! block of plain numbers: positions, the direction each face points,
//! colours, and which corners make each triangle.
//!
//! Duck Mold writes a deliberately narrow kind of it (`DESIGN.md`,
//! section 8), so that Duck Forge can read it with what it already has:
//!
//! - a tree of objects: the model, and under it one for every part, at
//!   the join it turns about, hanging from the part it is joined to. A
//!   part with a shape of its own has its triangles there, measured
//!   from its pivot. (Until Phase 5 the file was one object with one
//!   list of triangles; such files are still read.)
//! - every face has corners of its own, all with the same direction and
//!   colour, so every viewer shades it flat, as Duck Forge does;
//! - colour is stored per corner (`COLOR_0`), as linear light;
//! - the palette, the list of parts and the list of assemblies (parts
//!   made of parts), each with the triangles it owns and its object,
//!   and the named sockets, each with where it is, the way it faces and
//!   the object that carries it, are kept under `extras.duck_mold`.
//!
//! glTF's "right" is -x where Duck Forge's is +x, so on the way out x is
//! negated and each triangle's corners are listed the other way round.
//! Reading does the same in reverse.

use std::ops::Range;

use serde::Serialize;

use crate::issue::{Code, Issue};
use crate::math::{Bounds, Mat3, Transform, Vec3, tidy, vec3};
use crate::model::{Face, Model, Part, PartKind, Socket};
use crate::recipe::{Loc, PaletteColor, hex, rgb_of};

/// The grey of a face nobody coloured: `#B4B4B4`.
pub const UNPAINTED: [u8; 3] = [0xB4, 0xB4, 0xB4];

/// The version of what is kept under `extras.duck_mold`, and of how the
/// file is laid out. 0: one object with every triangle in one list
/// (Phases 0 to 4). 1: a tree of objects, one for every part, each with
/// its own triangles (Phase 5).
pub const EXTRAS_VERSION: i64 = 1;

/// The usual `#RRGGBB` numbers (sRGB, 0 to 255) as linear light, which is
/// how glTF wants vertex colours. A fixed table rather than a formula, so
/// that every computer writes the same bytes.
#[rustfmt::skip]
pub const SRGB_TO_LINEAR: [f32; 256] = [
    0.0, 0.000303527, 0.000607054, 0.000910581, 0.001214108, 0.001517635, 0.001821162,
    0.0021246888, 0.002428216, 0.0027317428, 0.00303527, 0.0033465358, 0.0036765074,
    0.004024717, 0.004391442, 0.0047769533, 0.0051815165, 0.0056053917, 0.006048833,
    0.0065120906, 0.00699541, 0.007499032, 0.008023193, 0.008568126, 0.009134059, 0.009721218,
    0.010329823, 0.010960094, 0.011612245, 0.012286488, 0.0129830325, 0.013702083, 0.014443844,
    0.015208514, 0.015996294, 0.016807375, 0.017641954, 0.01850022, 0.019382361, 0.020288562,
    0.02121901, 0.022173885, 0.023153367, 0.024157632, 0.02518686, 0.026241222, 0.027320892,
    0.02842604, 0.029556835, 0.030713445, 0.031896032, 0.033104766, 0.034339808, 0.035601314,
    0.03688945, 0.038204372, 0.039546236, 0.0409152, 0.04231141, 0.04373503, 0.045186203,
    0.046665087, 0.048171826, 0.049706567, 0.051269457, 0.052860647, 0.054480277, 0.05612849,
    0.05780543, 0.059511237, 0.061246052, 0.063010015, 0.064803265, 0.06662594, 0.06847817,
    0.070360094, 0.07227185, 0.07421357, 0.07618538, 0.07818742, 0.08021982, 0.08228271,
    0.08437621, 0.08650046, 0.08865558, 0.09084171, 0.093058966, 0.09530747, 0.09758735,
    0.099898726, 0.10224173, 0.104616486, 0.107023105, 0.10946171, 0.11193243, 0.114435375,
    0.116970666, 0.11953843, 0.122138776, 0.12477182, 0.12743768, 0.13013647, 0.13286832,
    0.13563333, 0.13843161, 0.14126329, 0.14412847, 0.14702727, 0.14995979, 0.15292615,
    0.15592647, 0.15896083, 0.16202937, 0.1651322, 0.1682694, 0.17144111, 0.1746474,
    0.17788842, 0.18116425, 0.18447499, 0.18782078, 0.19120169, 0.19461784, 0.19806932,
    0.20155625, 0.20507874, 0.20863687, 0.21223076, 0.2158605, 0.2195262, 0.22322796,
    0.22696587, 0.23074006, 0.23455058, 0.23839757, 0.24228112, 0.24620132, 0.25015828,
    0.2541521, 0.25818285, 0.26225066, 0.2663556, 0.2704978, 0.2746773, 0.27889428, 0.28314874,
    0.28744084, 0.29177064, 0.29613826, 0.30054379, 0.3049873, 0.30946892, 0.31398872,
    0.31854677, 0.3231432, 0.3277781, 0.33245152, 0.33716363, 0.34191442, 0.34670407,
    0.3515326, 0.35640013, 0.3613068, 0.3662526, 0.3712377, 0.37626213, 0.38132602, 0.38642943,
    0.39157248, 0.39675522, 0.40197778, 0.4072402, 0.4125426, 0.41788507, 0.42326766,
    0.4286905, 0.43415365, 0.43965718, 0.4452012, 0.4507858, 0.45641103, 0.462077, 0.4677838,
    0.47353148, 0.47932017, 0.48514995, 0.49102086, 0.49693298, 0.5028865, 0.50888133,
    0.5149177, 0.52099556, 0.5271151, 0.5332764, 0.5394795, 0.54572445, 0.55201143, 0.5583404,
    0.5647115, 0.57112485, 0.57758045, 0.58407843, 0.59061885, 0.59720176, 0.60382736,
    0.61049557, 0.6172066, 0.6239604, 0.63075715, 0.63759685, 0.6444797, 0.65140563,
    0.65837485, 0.6653873, 0.67244315, 0.6795425, 0.6866853, 0.69387174, 0.7011019, 0.70837575,
    0.7156935, 0.7230551, 0.73046076, 0.7379104, 0.7454042, 0.7529422, 0.7605245, 0.76815116,
    0.7758222, 0.7835378, 0.7912979, 0.7991027, 0.80695224, 0.8148466, 0.82278574, 0.8307699,
    0.838799, 0.8468732, 0.8549926, 0.8631572, 0.8713671, 0.8796224, 0.8879231, 0.8962694,
    0.9046612, 0.91309863, 0.92158186, 0.9301109, 0.9386857, 0.9473065, 0.9559733, 0.9646863,
    0.9734453, 0.9822506, 0.9911021, 1.0,
];

// ---- writing ----------------------------------------------------------------

/// The numbers glTF uses for kinds of data.
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const TRIANGLES: u32 = 4;

#[derive(Serialize)]
struct Gltf {
    asset: Asset,
    scene: u32,
    scenes: Vec<Scene>,
    nodes: Vec<Node>,
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    accessors: Vec<Accessor>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
    buffers: Vec<Buffer>,
    extras: Extras,
}

#[derive(Serialize)]
struct Asset {
    version: &'static str,
    generator: &'static str,
}

#[derive(Serialize)]
struct Scene {
    nodes: Vec<u32>,
}

/// One object of the file: the model itself, or one of its parts. A
/// part's object is at its pivot, measured from the object it hangs
/// from, so that turning it turns the part about its join.
#[derive(Serialize)]
struct Node {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mesh: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<u32>,
}

#[derive(Serialize)]
struct Mesh {
    name: String,
    primitives: Vec<Primitive>,
}

#[derive(Serialize)]
struct Primitive {
    attributes: Attributes,
    indices: u32,
    material: u32,
    mode: u32,
}

#[derive(Serialize)]
struct Attributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
    #[serde(rename = "COLOR_0")]
    color: u32,
}

#[derive(Serialize)]
struct Material {
    name: &'static str,
    #[serde(rename = "pbrMetallicRoughness")]
    surface: Surface,
}

#[derive(Serialize)]
struct Surface {
    #[serde(rename = "baseColorFactor")]
    color: [f32; 4],
    #[serde(rename = "metallicFactor")]
    metallic: f32,
    #[serde(rename = "roughnessFactor")]
    roughness: f32,
}

#[derive(Serialize)]
struct Accessor {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "byteOffset", skip_serializing_if = "is_zero")]
    offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<[f32; 3]>,
}

#[derive(Serialize)]
struct BufferView {
    buffer: u32,
    #[serde(rename = "byteOffset")]
    offset: usize,
    #[serde(rename = "byteLength")]
    length: usize,
    /// How far apart the entries are, for a list several parts share
    /// (glTF wants it said then). Not for the triangles' corners.
    #[serde(rename = "byteStride", skip_serializing_if = "Option::is_none")]
    stride: Option<usize>,
    target: u32,
}

fn is_zero(number: &usize) -> bool {
    *number == 0
}

#[derive(Serialize)]
struct Buffer {
    #[serde(rename = "byteLength")]
    length: usize,
}

#[derive(Serialize)]
struct Extras {
    duck_mold: DuckMold,
}

/// What Duck tools read and other programs ignore.
#[derive(Serialize)]
struct DuckMold {
    version: i64,
    name: String,
    palette: Vec<PaletteEntry>,
    /// The parts with a shape of their own, in order: together they own
    /// every triangle, once each.
    parts: Vec<PartEntry>,
    /// The parts made of parts, outer ones first: each owns the triangles
    /// of every part inside it, which come one after the other.
    assemblies: Vec<PartEntry>,
    /// Every named socket ([`Model::named_sockets`]), in glTF's
    /// directions like the corners: `head.top`, where it is and the way
    /// it faces, so a game can put a hat there.
    sockets: Vec<SocketEntry>,
}

#[derive(Serialize)]
struct SocketEntry {
    name: String,
    at: [f64; 3],
    facing: [f64; 3],
    /// The object that carries it when the model is posed: a place in
    /// `nodes` (0 is the model itself).
    node: usize,
}

/// A point or direction in glTF's directions: x negated (`+ 0.0` keeps
/// a zero from being written `-0.0`).
fn to_gltf(v: Vec3) -> [f64; 3] {
    [-v.x + 0.0, v.y + 0.0, v.z + 0.0]
}

#[derive(Serialize)]
struct PaletteEntry {
    name: String,
    hex: String,
}

#[derive(Serialize)]
struct PartEntry {
    name: String,
    first_triangle: usize,
    triangles: usize,
    /// Its object: a place in `nodes`.
    node: usize,
}

/// A point as the nearest 32-bit numbers to it, which is how a file
/// holds numbers.
fn as_stored(point: Vec3) -> Vec3 {
    vec3(
        f64::from(point.x as f32),
        f64::from(point.y as f32),
        f64::from(point.z as f32),
    )
}

/// How far a corner is from its part's pivot, along one direction, as
/// the 32-bit number to store: the one that, added to the pivot again
/// and kept as a 32-bit number, gives the corner as a file from before
/// Phase 5 held it. When no number does (the corner is much nearer zero
/// than the pivot is), the nearest: then the sum is off in its last
/// digits, by less than a ten-thousandth of a millimetre for a model a
/// few metres big.
fn from_pivot(corner: f64, pivot: f64) -> f32 {
    let wanted = corner as f32;
    let nearest = (corner - pivot) as f32;
    let gives = |own: f32| (pivot + f64::from(own)) as f32 == wanted;
    let mut down = nearest;
    let mut up = nearest;
    for _ in 0..3 {
        if gives(down) {
            return down;
        }
        if gives(up) {
            return up;
        }
        down = down.next_down();
        up = up.next_up();
    }
    nearest
}

/// The assemblies among `parts`, and those inside them, outer ones first,
/// each with the triangles of the parts inside it. `leaves` are the
/// parts with a shape of their own, as they were written; a part inside
/// an assembly is named after it (`left_arm/hand` is in `left_arm`), and
/// its triangles are next to the others in it.
fn assembly_entries(
    parts: &[Part],
    leaves: &[PartEntry],
    node_of: &dyn Fn(&str) -> usize,
) -> Vec<PartEntry> {
    let mut entries = Vec::new();
    for part in parts.iter().filter(|part| !part.parts.is_empty()) {
        let inside = format!("{}/", part.path);
        let mine: Vec<&PartEntry> = leaves
            .iter()
            .filter(|leaf| leaf.name.starts_with(&inside))
            .collect();
        entries.push(PartEntry {
            name: part.path.clone(),
            first_triangle: mine.first().map_or(0, |leaf| leaf.first_triangle),
            triangles: mine.iter().map(|leaf| leaf.triangles).sum(),
            node: node_of(&part.path),
        });
        entries.extend(assembly_entries(&part.parts, leaves, node_of));
    }
    entries
}

/// Writes the model as the bytes of a `.glb` file.
///
/// The file holds the model as a tree of objects: the model itself, and
/// under it one for every part, each at its pivot and hanging from the
/// part it is joined to. A part with a shape of its own has its
/// triangles there, measured from its pivot.
pub fn write_glb(model: &Model) -> Vec<u8> {
    // The objects: the model first, then every part in the recipe's
    // order.
    let all = model.all_parts();
    let node_of = |path: &str| {
        all.iter()
            .position(|part| part.path == path)
            .map_or(0, |at| at + 1)
    };
    // A reader adds a corner's place in its part to the part's pivot, as
    // 32-bit numbers (what the file holds). So that the sums come out as
    // the corners were, a pivot is used as the nearest 32-bit number to
    // it: then a corner at zero, on the ground or on the middle line, is
    // at exactly zero again.
    let pivot_of = |path: Option<&str>| {
        path.and_then(|path| all.iter().find(|part| part.path == path))
            .map_or(Vec3::ZERO, |part| as_stored(part.pivot))
    };
    let mut nodes: Vec<Node> = vec![Node {
        name: model.name.clone(),
        mesh: None,
        translation: None,
        children: Vec::new(),
    }];
    for part in &all {
        let from = pivot_of(part.parent.as_deref());
        nodes.push(Node {
            name: part.path.clone(),
            mesh: None,
            translation: Some(to_gltf(as_stored(part.pivot) - from)),
            children: Vec::new(),
        });
    }
    for (at, part) in all.iter().enumerate() {
        let parent = part.parent.as_deref().map_or(0, node_of);
        nodes[parent].children.push(at as u32 + 1);
    }

    // The numbers, every part's one after the other: positions (measured
    // from the part's pivot), directions, colours, and the triangles'
    // corners (counted from the part's first corner).
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut parts: Vec<PartEntry> = Vec::new();
    let mut meshes: Vec<Mesh> = Vec::new();
    let mut accessors: Vec<Accessor> = Vec::new();

    for leaf in model.leaves() {
        let first_triangle = indices.len() / 3;
        let first_corner = positions.len();
        for face in &model.faces[leaf.faces.clone()] {
            // A face squashed to nothing has no direction and is left out.
            let Some(normal) = face.normal() else {
                continue;
            };
            let rgb = match face.color {
                Some(color) => model.palette[color].rgb,
                None => UNPAINTED,
            };
            let linear = rgb.map(|channel| SRGB_TO_LINEAR[channel as usize]);
            let base = (positions.len() - first_corner) as u32;
            let pivot = as_stored(leaf.pivot);
            for corner in &face.corners {
                // glTF's right is -x.
                let own = [
                    from_pivot(corner.x, pivot.x),
                    from_pivot(corner.y, pivot.y),
                    from_pivot(corner.z, pivot.z),
                ];
                positions.push([-own[0] + 0.0, own[1] + 0.0, own[2] + 0.0]);
                normals.push([
                    tidy(-normal.x) as f32,
                    tidy(normal.y) as f32,
                    tidy(normal.z) as f32,
                ]);
                colors.push(linear);
            }
            // The face as a fan of triangles, each listed the other way
            // round because x was negated.
            for at in 1..face.corners.len() as u32 - 1 {
                indices.extend([base, base + at + 1, base + at]);
            }
        }
        let node = node_of(&leaf.path);
        let triangles = indices.len() / 3 - first_triangle;
        parts.push(PartEntry {
            name: leaf.path.clone(),
            first_triangle,
            triangles,
            node,
        });
        if triangles == 0 {
            continue;
        }
        // glTF wants the smallest and biggest position written down.
        let own = &positions[first_corner..];
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for position in own {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
        let first = accessors.len() as u32;
        for (view, min, max) in [(0, Some(min), Some(max)), (1, None, None), (2, None, None)] {
            accessors.push(Accessor {
                buffer_view: view,
                offset: first_corner * 12,
                component_type: FLOAT,
                count: own.len(),
                kind: "VEC3",
                min,
                max,
            });
        }
        accessors.push(Accessor {
            buffer_view: 3,
            offset: first_triangle * 12,
            component_type: UNSIGNED_INT,
            count: triangles * 3,
            kind: "SCALAR",
            min: None,
            max: None,
        });
        nodes[node].mesh = Some(meshes.len() as u32);
        meshes.push(Mesh {
            name: leaf.path.clone(),
            primitives: vec![Primitive {
                attributes: Attributes {
                    position: first,
                    normal: first + 1,
                    color: first + 2,
                },
                indices: first + 3,
                material: 0,
                mode: TRIANGLES,
            }],
        });
    }

    // The block of numbers: positions, then directions, then colours,
    // then the triangles' corners.
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<BufferView> = Vec::new();
    for list in [&positions, &normals, &colors] {
        let offset = bin.len();
        for triple in list {
            for number in triple {
                bin.extend(number.to_le_bytes());
            }
        }
        views.push(BufferView {
            buffer: 0,
            offset,
            length: bin.len() - offset,
            stride: Some(12),
            target: ARRAY_BUFFER,
        });
    }
    let offset = bin.len();
    for index in &indices {
        bin.extend(index.to_le_bytes());
    }
    views.push(BufferView {
        buffer: 0,
        offset,
        length: bin.len() - offset,
        stride: None,
        target: ELEMENT_ARRAY_BUFFER,
    });

    let gltf = Gltf {
        asset: Asset {
            version: "2.0",
            generator: "Duck Mold",
        },
        scene: 0,
        scenes: vec![Scene { nodes: vec![0] }],
        nodes,
        meshes,
        // One plain, matt material. Without it viewers assume shiny metal.
        materials: vec![Material {
            name: "palette",
            surface: Surface {
                color: [1.0; 4],
                metallic: 0.0,
                roughness: 1.0,
            },
        }],
        accessors,
        buffer_views: views,
        buffers: vec![Buffer { length: bin.len() }],
        extras: Extras {
            duck_mold: DuckMold {
                version: EXTRAS_VERSION,
                name: model.name.clone(),
                palette: model
                    .palette
                    .iter()
                    .map(|color| PaletteEntry {
                        name: color.name.clone(),
                        hex: hex(color.rgb),
                    })
                    .collect(),
                assemblies: assembly_entries(&model.parts, &parts, &node_of),
                parts,
                sockets: model
                    .named_sockets()
                    .into_iter()
                    .map(|socket| SocketEntry {
                        node: model
                            .socket_carrier(&socket.name)
                            .map_or(0, |path| node_of(&path)),
                        name: socket.name,
                        at: to_gltf(socket.at),
                        facing: to_gltf(socket.facing),
                    })
                    .collect(),
            },
        },
    };

    // The file: a header, the JSON (padded with spaces to a multiple of
    // four bytes), the numbers (already a multiple of four).
    let mut json = serde_json::to_vec(&gltf).expect("a glTF description is plain data");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend(b"glTF");
    glb.extend(2u32.to_le_bytes());
    glb.extend((total as u32).to_le_bytes());
    glb.extend((json.len() as u32).to_le_bytes());
    glb.extend(b"JSON");
    glb.extend(json);
    glb.extend((bin.len() as u32).to_le_bytes());
    glb.extend(b"BIN\0");
    glb.extend(bin);
    glb
}

// ---- reading ----------------------------------------------------------------

/// Reads a `.glb` that Duck Mold wrote back into a model, to measure it.
///
/// The parts come back as a tree, rebuilt from the parts and assemblies
/// the file lists (a file from before assemblies were written gives a
/// flat list), every face with the corners it was written with, and the
/// named sockets with their parts. What isn't stored doesn't come back:
/// which socket a part was joined to, what it was made from, or the
/// sockets of unnamed parts. Other glTF files are refused:
/// importing them is planned for a later phase.
pub fn read_glb(bytes: &[u8], file: &str) -> Result<Model, Box<Issue>> {
    let bad = |message: &str| {
        Box::new(
            Issue::error(
                Code::BadMesh,
                "",
                format!("this isn't a .glb file Duck Mold can read: {message}"),
            )
            .in_file(file),
        )
    };
    let u32_at = |at: usize| -> Option<usize> {
        let four = bytes.get(at..at + 4)?;
        Some(u32::from_le_bytes([four[0], four[1], four[2], four[3]]) as usize)
    };

    if bytes.get(0..4) != Some(b"glTF") {
        return Err(bad("it doesn't start with \"glTF\""));
    }
    if u32_at(4) != Some(2) {
        return Err(bad("it isn't glTF version 2"));
    }
    let json_length = u32_at(12).ok_or_else(|| bad("it is cut short"))?;
    if bytes.get(16..20) != Some(b"JSON") {
        return Err(bad("its first block isn't JSON"));
    }
    let json_end = 20 + json_length;
    let json = bytes
        .get(20..json_end)
        .ok_or_else(|| bad("it is cut short"))?;
    let bin_length = u32_at(json_end).ok_or_else(|| bad("it has no block of numbers"))?;
    if bytes.get(json_end + 4..json_end + 8) != Some(b"BIN\0") {
        return Err(bad("its second block isn't numbers"));
    }
    let bin = bytes
        .get(json_end + 8..json_end + 8 + bin_length)
        .ok_or_else(|| bad("it is cut short"))?;
    let gltf: serde_json::Value =
        serde_json::from_slice(json).map_err(|_| bad("its description isn't valid JSON"))?;

    let extras = &gltf["extras"]["duck_mold"];
    if !extras.is_object() {
        return Err(Box::new(
            Issue::error(
                Code::Unsupported,
                "",
                "this .glb wasn't written by Duck Mold, so it has no parts or palette to read back",
            )
            .in_file(file),
        ));
    }
    let version = extras["version"].as_i64();
    if !matches!(version, Some(0 | EXTRAS_VERSION)) {
        return Err(bad("it was written by a newer Duck Mold"));
    }

    // One list of numbers, found through the description.
    let numbers = |accessor: &serde_json::Value,
                   kind: &str,
                   component: u32,
                   per: usize|
     -> Option<(usize, &[u8])> {
        let accessor = &gltf["accessors"][accessor.as_u64()? as usize];
        if accessor["type"].as_str()? != kind
            || accessor["componentType"].as_u64()? != component as u64
        {
            return None;
        }
        let count = usize::try_from(accessor["count"].as_u64()?).ok()?;
        let view = &gltf["bufferViews"][accessor["bufferView"].as_u64()? as usize];
        // Checked, so a damaged file can't overflow the sums.
        let offset = usize::try_from(
            view["byteOffset"]
                .as_u64()
                .unwrap_or(0)
                .checked_add(accessor["byteOffset"].as_u64().unwrap_or(0))?,
        )
        .ok()?;
        let length = count.checked_mul(per)?.checked_mul(4)?;
        Some((count, bin.get(offset..offset.checked_add(length)?)?))
    };
    let layout = "its numbers aren't laid out the way Duck Mold writes them";
    let float = |list: &[u8], at: usize| {
        f32::from_le_bytes([
            list[at * 4],
            list[at * 4 + 1],
            list[at * 4 + 2],
            list[at * 4 + 3],
        ])
    };

    // The objects of a file laid out as a tree (version 1): which one
    // each hangs from, and where each is in the model. In a file from
    // before, there is one object and everything is measured from zero.
    let node_count = gltf["nodes"].as_array().map_or(0, Vec::len);
    let mut parent_of: Vec<Option<usize>> = vec![None; node_count];
    for (at, node) in gltf["nodes"].as_array().into_iter().flatten().enumerate() {
        for child in node["children"].as_array().into_iter().flatten() {
            let child = child
                .as_u64()
                .and_then(|child| usize::try_from(child).ok())
                .filter(|child| *child < node_count && *child != at)
                .ok_or_else(|| bad(layout))?;
            if parent_of[child].replace(at).is_some() {
                return Err(bad(layout));
            }
        }
    }
    let shift_of = |node: usize| -> Option<Vec3> {
        match gltf["nodes"][node]["translation"].as_array() {
            None => Some(Vec3::ZERO),
            Some(list) => match list.as_slice() {
                [x, y, z] => Some(vec3(-x.as_f64()?, y.as_f64()?, z.as_f64()?)),
                _ => None,
            },
        }
    };
    // Where an object is: its own shift and those of everything it
    // hangs from. (No more steps than there are objects: a ring is
    // refused.)
    let place_of = |node: usize| -> Option<Vec3> {
        let mut at = shift_of(node)?;
        let mut above = parent_of.get(node).copied().flatten();
        for _ in 0..node_count {
            let Some(next) = above else {
                return Some(at);
            };
            at = at + shift_of(next)?;
            above = parent_of[next];
        }
        None
    };

    // The lumps of triangles: one for the whole model in a file from
    // before, one for every part with a shape of its own in a tree. Each
    // is its corners, their colours, its triangles, and where its corners
    // are measured from.
    struct Lump<'a> {
        corners: usize,
        positions: &'a [u8],
        colors: &'a [u8],
        triangles: usize,
        indices: &'a [u8],
        from: Vec3,
    }
    let lump = |primitive: &serde_json::Value, from: Vec3| -> Option<Lump> {
        let attributes = &primitive["attributes"];
        let (corners, positions) = numbers(&attributes["POSITION"], "VEC3", FLOAT, 3)?;
        let (color_count, colors) = numbers(&attributes["COLOR_0"], "VEC3", FLOAT, 3)?;
        let (index_count, indices) = numbers(&primitive["indices"], "SCALAR", UNSIGNED_INT, 1)?;
        (color_count == corners && index_count % 3 == 0).then_some(Lump {
            corners,
            positions,
            colors,
            triangles: index_count / 3,
            indices,
            from,
        })
    };
    let mut lumps: Vec<Lump> = Vec::new();
    // For a tree: the object of each part and assembly, by name.
    let mut node_of_part: Vec<(String, usize)> = Vec::new();
    if version == Some(0) {
        lumps.push(
            lump(&gltf["meshes"][0]["primitives"][0], Vec3::ZERO).ok_or_else(|| bad(layout))?,
        );
    } else {
        for (list, with_shape) in [(&extras["parts"], true), (&extras["assemblies"], false)] {
            for entry in list.as_array().map(Vec::as_slice).unwrap_or_default() {
                let (Some(name), Some(node)) = (
                    entry["name"].as_str(),
                    entry["node"]
                        .as_u64()
                        .and_then(|node| usize::try_from(node).ok())
                        .filter(|node| *node < node_count),
                ) else {
                    return Err(bad("its list of parts is damaged"));
                };
                node_of_part.push((name.to_owned(), node));
                if !with_shape {
                    continue;
                }
                let from = place_of(node).ok_or_else(|| bad(layout))?;
                // A part with no triangles has no lump.
                if let Some(mesh) = gltf["nodes"][node]["mesh"].as_u64() {
                    let primitive = &gltf["meshes"][mesh as usize]["primitives"][0];
                    lumps.push(lump(primitive, from).ok_or_else(|| bad(layout))?);
                }
            }
        }
    }
    for lump in &lumps {
        let index = |at: usize| float_index(lump.indices, at);
        if (0..lump.triangles * 3).any(|at| index(at) >= lump.corners) {
            return Err(bad("a triangle names a corner that isn't there"));
        }
    }

    // The palette.
    let mut palette = Vec::new();
    for entry in extras["palette"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let (Some(name), Some(rgb)) = (
            entry["name"].as_str(),
            entry["hex"].as_str().and_then(rgb_of),
        ) else {
            return Err(bad("its palette is damaged"));
        };
        palette.push(PaletteColor {
            name: name.to_owned(),
            rgb,
            loc: Loc::default(),
            own: false,
        });
    }

    // A corner, back in Duck Mold's directions and in the model.
    let corner = |lump: &Lump, at: usize| {
        (vec3(
            -f64::from(float(lump.positions, at * 3)),
            f64::from(float(lump.positions, at * 3 + 1)),
            f64::from(float(lump.positions, at * 3 + 2)),
        ) + lump.from)
            .tidied()
    };
    // A corner's colour as a place in the palette.
    let color_of = |lump: &Lump, at: usize| -> Option<usize> {
        let mut rgb = [0u8; 3];
        for (channel, value) in rgb.iter_mut().enumerate() {
            let linear = float(lump.colors, at * 3 + channel);
            // The table rises steadily, so this finds the nearest entry.
            let above = SRGB_TO_LINEAR
                .partition_point(|&entry| entry < linear)
                .min(255);
            let below = above.saturating_sub(1);
            *value = if (SRGB_TO_LINEAR[above] - linear).abs()
                <= (linear - SRGB_TO_LINEAR[below]).abs()
            {
                above as u8
            } else {
                below as u8
            };
        }
        palette.iter().position(|color| color.rgb == rgb)
    };

    // The faces. Duck Mold writes a face as a fan of triangles from its
    // first corner: (b, b+2, b+1), (b, b+3, b+2), ... Reading the fan
    // back gives the face as it was.
    let triangle_count: usize = lumps.iter().map(|lump| lump.triangles).sum();
    let mut faces: Vec<Face> = Vec::new();
    // For each triangle, which face it ended up in.
    let mut face_of_triangle: Vec<usize> = Vec::with_capacity(triangle_count);
    for lump in &lumps {
        let index = |at: usize| float_index(lump.indices, at);
        let mut at = 0;
        while at < lump.triangles {
            let base = index(at * 3);
            let mut fan = 0;
            while at + fan < lump.triangles
                && index((at + fan) * 3) == base
                && index((at + fan) * 3 + 1) == base + fan + 2
                && index((at + fan) * 3 + 2) == base + fan + 1
            {
                fan += 1;
            }
            if fan == 0 {
                // Not a fan: a single triangle, turned back the right way.
                let [a, b, c] = [index(at * 3), index(at * 3 + 2), index(at * 3 + 1)];
                faces.push(Face {
                    corners: vec![corner(lump, a), corner(lump, b), corner(lump, c)],
                    color: color_of(lump, a),
                });
                fan = 1;
            } else {
                faces.push(Face {
                    corners: (base..base + fan + 2).map(|at| corner(lump, at)).collect(),
                    color: color_of(lump, base),
                });
            }
            face_of_triangle.extend(std::iter::repeat_n(faces.len() - 1, fan));
            at += fan;
        }
    }

    // The parts: each with a shape of its own from "parts", each made of
    // parts from "assemblies", as a tree again. A part goes inside the
    // assembly its name starts with: `left_arm/hand` inside `left_arm`.
    let mut entries: Vec<(String, Range<usize>, bool)> = Vec::new();
    for (list, assembly) in [(&extras["parts"], false), (&extras["assemblies"], true)] {
        for entry in list.as_array().map(Vec::as_slice).unwrap_or_default() {
            let whole = |field: &str| entry[field].as_u64().and_then(|n| usize::try_from(n).ok());
            let (Some(name), Some(first), Some(count)) = (
                entry["name"].as_str(),
                whole("first_triangle"),
                whole("triangles"),
            ) else {
                return Err(bad("its list of parts is damaged"));
            };
            if first
                .checked_add(count)
                .is_none_or(|end| end > triangle_count)
            {
                return Err(bad("its list of parts doesn't match its triangles"));
            }
            let own = if count == 0 {
                faces.len()..faces.len()
            } else {
                face_of_triangle[first]..face_of_triangle[first + count - 1] + 1
            };
            entries.push((name.to_owned(), own, assembly));
        }
    }
    let mut parts = part_tree("", &entries, &faces);
    // In a tree, every part says what it turns about and hangs from.
    for (name, node) in &node_of_part {
        let pivot = place_of(*node).ok_or_else(|| bad(layout))?.tidied();
        let parent = parent_of[*node]
            .and_then(|above| node_of_part.iter().find(|(_, node)| *node == above))
            .map(|(name, _)| name.clone());
        if let Some(part) = find_part(&mut parts, name) {
            part.pivot = pivot;
            part.parent = parent;
        }
    }

    // The sockets: the model's own by bare name, a part's after its path.
    // Files from before sockets were written have none.
    let mut sockets = Vec::new();
    for entry in extras["sockets"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let three = |field: &str| -> Option<Vec3> {
            let list = entry[field].as_array()?;
            match list.as_slice() {
                [x, y, z] => Some(vec3(-x.as_f64()? + 0.0, y.as_f64()?, z.as_f64()?)),
                _ => None,
            }
        };
        let (Some(name), Some(at), Some(facing)) =
            (entry["name"].as_str(), three("at"), three("facing"))
        else {
            return Err(bad("its list of sockets is damaged"));
        };
        let socket = |name: &str| Socket {
            name: name.to_owned(),
            at,
            facing,
        };
        match name.rsplit_once('.') {
            None => sockets.push(socket(name)),
            Some((path, own)) => {
                if let Some(part) = find_part(&mut parts, path) {
                    part.sockets.push(socket(own));
                }
            }
        }
    }
    Ok(Model {
        name: extras["name"].as_str().unwrap_or("model").to_owned(),
        palette,
        faces,
        parts,
        sockets,
        carried: Vec::new(),
        zero: Vec3::ZERO,
    })
}

/// The whole number number `at` in a list of 32-bit ones.
fn float_index(list: &[u8], at: usize) -> usize {
    u32::from_le_bytes([
        list[at * 4],
        list[at * 4 + 1],
        list[at * 4 + 2],
        list[at * 4 + 3],
    ]) as usize
}

/// The part with this path (`left_arm/hand`), however deep it is.
fn find_part<'a>(parts: &'a mut [Part], path: &str) -> Option<&'a mut Part> {
    for part in parts {
        if part.path == path {
            return Some(part);
        }
        if path.starts_with(&format!("{}/", part.path)) {
            return find_part(&mut part.parts, path);
        }
    }
    None
}

/// The parts inside the assembly `within` (`""` for the top of the
/// model), with the parts inside them, in the order of their faces.
///
/// A part's place is the nearest assembly that its name starts with, so a
/// file from before assemblies were written (Duck Mold's Phase 0) gives a
/// flat list, as it always did.
fn part_tree(within: &str, entries: &[(String, Range<usize>, bool)], faces: &[Face]) -> Vec<Part> {
    let is_assembly = |name: &str| {
        entries
            .iter()
            .any(|(other, _, assembly)| *assembly && other == name)
    };
    // The nearest assembly a name is inside: `a/b/c` is in `a/b` if that
    // is an assembly, else in `a`, else at the top.
    let place_of = |name: &str| -> String {
        let mut place = name;
        while let Some((outer, _)) = place.rsplit_once('/') {
            if is_assembly(outer) {
                return outer.to_owned();
            }
            place = outer;
        }
        String::new()
    };
    let mut children: Vec<&(String, Range<usize>, bool)> = entries
        .iter()
        .filter(|(name, _, _)| place_of(name) == within)
        .collect();
    children.sort_by_key(|(_, own, _)| own.start);
    children
        .into_iter()
        .map(|(name, own, assembly)| Part {
            name: name.rsplit('/').next().unwrap_or(name).to_owned(),
            path: name.clone(),
            loc: Loc::default(),
            kind: PartKind {
                shape: if *assembly { "assembly" } else { "part" }.to_owned(),
                uses: None,
                file: None,
                changes: 0,
            },
            attached_to: None,
            mirrored: false,
            place: Transform::IDENTITY,
            local: Transform::IDENTITY,
            // (A file laid out as a tree says these; see `read_glb`.)
            pivot: Bounds::around(
                faces[own.clone()]
                    .iter()
                    .flat_map(|face| face.corners.iter().copied()),
            )
            .map_or(Vec3::ZERO, |bounds| bounds.center().tidied()),
            parent: name.rsplit_once('/').map(|(outer, _)| outer.to_owned()),
            axes: Mat3::IDENTITY,
            limits: crate::recipe::Limits::Free,
            carried: Vec::new(),
            sockets: Vec::new(),
            bounds: Bounds::around(
                faces[own.clone()]
                    .iter()
                    .flat_map(|face| face.corners.iter().copied()),
            ),
            face_names: Vec::new(),
            face_groups: Vec::new(),
            shaped: None,
            faces: own.clone(),
            parts: if *assembly {
                part_tree(name, entries, faces)
            } else {
                Vec::new()
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_table_is_the_srgb_curve() {
        assert_eq!(SRGB_TO_LINEAR[0], 0.0);
        assert_eq!(SRGB_TO_LINEAR[255], 1.0);
        for (value, &linear) in SRGB_TO_LINEAR.iter().enumerate() {
            // The standard formula, worked out here with ordinary maths.
            let c = value as f64 / 255.0;
            let expected = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            assert!((f64::from(linear) - expected).abs() < 1e-7, "entry {value}");
            // And it rises all the way, so no two colours share a number.
            if value > 0 {
                assert!(linear > SRGB_TO_LINEAR[value - 1]);
            }
        }
    }
}
