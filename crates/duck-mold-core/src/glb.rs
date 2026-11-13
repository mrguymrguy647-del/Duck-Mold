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
//! - one object, one mesh, one list of triangles;
//! - every face has corners of its own, all with the same direction and
//!   colour, so every viewer shades it flat, as Duck Forge does;
//! - colour is stored per corner (`COLOR_0`), as linear light;
//! - the palette, the list of parts and the list of assemblies (parts
//!   made of parts), each with the triangles it owns, and the named
//!   sockets, each with where it is and the way it faces, are kept under
//!   `extras.duck_mold`.
//!
//! glTF's "right" is -x where Duck Forge's is +x, so on the way out x is
//! negated and each triangle's corners are listed the other way round.
//! Reading does the same in reverse.

use std::ops::Range;

use serde::Serialize;

use crate::issue::{Code, Issue};
use crate::math::{Bounds, Transform, Vec3, tidy, vec3};
use crate::model::{Face, Model, Part, PartKind, Socket};
use crate::recipe::{Loc, PaletteColor, hex, rgb_of};

/// The grey of a face nobody coloured: `#B4B4B4`.
pub const UNPAINTED: [u8; 3] = [0xB4, 0xB4, 0xB4];

/// The version of what is kept under `extras.duck_mold`.
pub const EXTRAS_VERSION: i64 = 0;

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

#[derive(Serialize)]
struct Node {
    name: String,
    mesh: u32,
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
    target: u32,
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
}

/// The assemblies among `parts`, and those inside them, outer ones first,
/// each with the triangles of the parts inside it. `leaves` are the
/// parts with a shape of their own, as they were written; a part inside
/// an assembly is named after it (`left_arm/hand` is in `left_arm`), and
/// its triangles are next to the others in it.
fn assembly_entries(parts: &[Part], leaves: &[PartEntry]) -> Vec<PartEntry> {
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
        });
        entries.extend(assembly_entries(&part.parts, leaves));
    }
    entries
}

/// Writes the model as the bytes of a `.glb` file.
pub fn write_glb(model: &Model) -> Vec<u8> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut parts: Vec<PartEntry> = Vec::new();

    for leaf in model.leaves() {
        let first_triangle = indices.len() / 3;
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
            let base = positions.len() as u32;
            for corner in &face.corners {
                // glTF's right is -x.
                positions.push([-corner.x as f32, corner.y as f32, corner.z as f32]);
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
        parts.push(PartEntry {
            name: leaf.path.clone(),
            first_triangle,
            triangles: indices.len() / 3 - first_triangle,
        });
    }

    // The block of numbers: positions, then directions, then colours,
    // then the triangles' corners.
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<BufferView> = Vec::new();
    for (list, target) in [
        (&positions, ARRAY_BUFFER),
        (&normals, ARRAY_BUFFER),
        (&colors, ARRAY_BUFFER),
    ] {
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
            target,
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
        target: ELEMENT_ARRAY_BUFFER,
    });

    // glTF wants the smallest and biggest position written down.
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in &positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    if positions.is_empty() {
        (min, max) = ([0.0; 3], [0.0; 3]);
    }

    let triples = |view: u32, min: Option<[f32; 3]>, max: Option<[f32; 3]>| Accessor {
        buffer_view: view,
        component_type: FLOAT,
        count: positions.len(),
        kind: "VEC3",
        min,
        max,
    };
    let gltf = Gltf {
        asset: Asset {
            version: "2.0",
            generator: "Duck Mold",
        },
        scene: 0,
        scenes: vec![Scene { nodes: vec![0] }],
        nodes: vec![Node {
            name: model.name.clone(),
            mesh: 0,
        }],
        meshes: vec![Mesh {
            name: model.name.clone(),
            primitives: vec![Primitive {
                attributes: Attributes {
                    position: 0,
                    normal: 1,
                    color: 2,
                },
                indices: 3,
                material: 0,
                mode: TRIANGLES,
            }],
        }],
        // One plain, matt material. Without it viewers assume shiny metal.
        materials: vec![Material {
            name: "palette",
            surface: Surface {
                color: [1.0; 4],
                metallic: 0.0,
                roughness: 1.0,
            },
        }],
        accessors: vec![
            triples(0, Some(min), Some(max)),
            triples(1, None, None),
            triples(2, None, None),
            Accessor {
                buffer_view: 3,
                component_type: UNSIGNED_INT,
                count: indices.len(),
                kind: "SCALAR",
                min: None,
                max: None,
            },
        ],
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
                assemblies: assembly_entries(&model.parts, &parts),
                parts,
                sockets: model
                    .named_sockets()
                    .into_iter()
                    .map(|socket| SocketEntry {
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
    if extras["version"].as_i64() != Some(EXTRAS_VERSION) {
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
    let primitive = &gltf["meshes"][0]["primitives"][0];
    let attributes = &primitive["attributes"];
    let layout = "its numbers aren't laid out the way Duck Mold writes them";
    let (corner_count, positions) =
        numbers(&attributes["POSITION"], "VEC3", FLOAT, 3).ok_or_else(|| bad(layout))?;
    let (color_count, colors) =
        numbers(&attributes["COLOR_0"], "VEC3", FLOAT, 3).ok_or_else(|| bad(layout))?;
    let (index_count, indices) =
        numbers(&primitive["indices"], "SCALAR", UNSIGNED_INT, 1).ok_or_else(|| bad(layout))?;
    if color_count != corner_count || index_count % 3 != 0 {
        return Err(bad(layout));
    }
    let float = |list: &[u8], at: usize| {
        f32::from_le_bytes([
            list[at * 4],
            list[at * 4 + 1],
            list[at * 4 + 2],
            list[at * 4 + 3],
        ])
    };
    let index = |at: usize| {
        u32::from_le_bytes([
            indices[at * 4],
            indices[at * 4 + 1],
            indices[at * 4 + 2],
            indices[at * 4 + 3],
        ]) as usize
    };
    if (0..index_count).any(|at| index(at) >= corner_count) {
        return Err(bad("a triangle names a corner that isn't there"));
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

    // A corner, back in Duck Mold's directions.
    let corner = |at: usize| {
        vec3(
            -f64::from(float(positions, at * 3)),
            f64::from(float(positions, at * 3 + 1)),
            f64::from(float(positions, at * 3 + 2)),
        )
        .tidied()
    };
    // A corner's colour as a place in the palette.
    let color_of = |at: usize| -> Option<usize> {
        let mut rgb = [0u8; 3];
        for (channel, value) in rgb.iter_mut().enumerate() {
            let linear = float(colors, at * 3 + channel);
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
    let triangle_count = index_count / 3;
    let mut faces: Vec<Face> = Vec::new();
    // For each triangle, which face it ended up in.
    let mut face_of_triangle: Vec<usize> = Vec::with_capacity(triangle_count);
    let mut at = 0;
    while at < triangle_count {
        let base = index(at * 3);
        let mut fan = 0;
        while at + fan < triangle_count
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
                corners: vec![corner(a), corner(b), corner(c)],
                color: color_of(a),
            });
            fan = 1;
        } else {
            faces.push(Face {
                corners: (base..base + fan + 2).map(corner).collect(),
                color: color_of(base),
            });
        }
        face_of_triangle.extend(std::iter::repeat_n(faces.len() - 1, fan));
        at += fan;
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
        zero: Vec3::ZERO,
    })
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
