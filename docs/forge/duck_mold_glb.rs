//! Reads the `.glb` files Duck Mold writes, for Duck Forge.
//!
//! This file is written to be copied into Duck Forge (its `src/glb.rs`).
//! It needs only the standard library and `serde_json`, which Duck Forge
//! already uses for sprite sheets, so it adds no new dependency.
//!
//! It is deliberately separate from Duck Mold's own code: it shares
//! nothing with the exporter, so a mistake in one cannot hide a mistake
//! in the other. Duck Mold's tests run it against every example model.
//!
//! What comes out is exactly what a Duck Forge `Mesh` holds: a list of
//! points, and triangles with three corners (clockwise seen from the
//! front) and one colour each. Duck Forge's `Mesh::load_glb` does that.
//!
//! **Copying it.** Everything between the two `// ====` marker lines below
//! is the reader, and Duck Forge's `src/glb.rs` holds the same lines, byte
//! for byte; each project keeps its own introduction and imports above
//! them, and Duck Forge its own functions below. The first marker says the
//! reader's version. To change the reader: change it here, raise the
//! version, run Duck Mold's tests, and copy the lines between the markers
//! into Duck Forge. Duck Mold's `docs/forge-handoff.md` has the command
//! that checks the two copies are the same.
//!
//! It reads the narrow kind of `.glb` that Duck Mold writes (Duck Mold's
//! `docs/forge-handoff.md` describes it), not glTF in general: anything
//! else is refused with a sentence saying why.

use std::collections::HashMap;

// ==== Duck Mold's .glb reader, version 2 ====================================
//
// Version 2: a part made of parts (an "assembly") has a triangle range too,
// in `assemblies`; and sizes read from the file are checked before they are
// added or multiplied, so a damaged file is refused instead of overflowing.
// Version 1 was Duck Mold's Phase 0.

/// A model read from a Duck Mold `.glb`, in Duck Forge's directions:
/// x is right, y is up, z is forward, 1 unit is about a metre. The model
/// faces forward (+z) and stands on y = 0.
#[derive(Debug, Clone, PartialEq)]
pub struct GlbModel {
    /// The model's name.
    pub name: String,
    /// The corner points. Corners in the same place are shared, so a box
    /// has 8.
    pub vertices: Vec<[f32; 3]>,
    /// The triangles.
    pub triangles: Vec<GlbTriangle>,
    /// The model's palette, by name.
    pub palette: Vec<GlbColor>,
    /// The model's parts and which triangles each owns, in order. Each
    /// part has a shape of its own; together they own every triangle,
    /// once each.
    pub parts: Vec<GlbPart>,
    /// The parts made of other parts (a robot's arm made of an upper arm,
    /// an elbow and a hand, say), each with the range of all its parts'
    /// triangles, outer ones before the parts inside them. A part inside
    /// one is named after it: `left_arm/hand` is in `left_arm`. Empty for
    /// a model with none, and for files from before version 2.
    pub assemblies: Vec<GlbPart>,
}

/// One triangle: three places in `vertices`, clockwise seen from the
/// front, and its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbTriangle {
    /// The three corners.
    pub corners: [usize; 3],
    /// Red, green and blue, 0 to 255: one of the palette's colours, or
    /// the grey `180, 180, 180` of a face nobody coloured.
    pub color: [u8; 3],
}

/// One palette colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbColor {
    /// Its name in the recipe.
    pub name: String,
    /// Red, green and blue, 0 to 255.
    pub rgb: [u8; 3],
}

/// One part of the model, or one assembly, and its triangles: those
/// from `first_triangle` on, `triangles` of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbPart {
    /// Its name; a part inside another is written `left_arm/hand`.
    pub name: String,
    /// The first of its triangles, as a place in `triangles`.
    pub first_triangle: usize,
    /// How many triangles it has.
    pub triangles: usize,
}

/// Reads the bytes of a `.glb` file written by Duck Mold.
pub fn read_duck_mold_glb(bytes: &[u8]) -> Result<GlbModel, String> {
    // ---- The file: a 12-byte header, then blocks. Each block is its
    // length, a 4-letter kind, and its bytes. Duck Mold writes two: the
    // description as JSON, then the numbers.
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err("not a .glb file: it doesn't start with \"glTF\"".into());
    }
    if u32_at(bytes, 4)? != 2 {
        return Err("not glTF version 2".into());
    }
    if u32_at(bytes, 8)? as usize != bytes.len() {
        return Err("the file is cut short or has extra bytes".into());
    }
    let json_length = u32_at(bytes, 12)? as usize;
    if &bytes[16..20] != b"JSON" {
        return Err("the first block isn't JSON".into());
    }
    let json_end = 20 + json_length;
    let json = bytes
        .get(20..json_end)
        .ok_or("the JSON block is cut short")?;
    let bin_length = u32_at(bytes, json_end)? as usize;
    if bytes.get(json_end + 4..json_end + 8) != Some(b"BIN\0") {
        return Err("the second block isn't the numbers".into());
    }
    let bin = bytes
        .get(json_end + 8..json_end + 8 + bin_length)
        .ok_or("the block of numbers is cut short")?;
    let gltf: serde_json::Value = serde_json::from_slice(json)
        .map_err(|error| format!("the JSON block is broken: {error}"))?;

    // ---- The description. Duck Mold writes one mesh with one list of
    // triangles.
    let extras = &gltf["extras"]["duck_mold"];
    if extras["version"].as_i64() != Some(0) {
        return Err("this .glb wasn't written by Duck Mold (no extras.duck_mold version 0)".into());
    }
    let primitive = &gltf["meshes"][0]["primitives"][0];
    if primitive["mode"].as_u64().unwrap_or(4) != 4 {
        return Err("the mesh isn't a list of triangles".into());
    }
    // The numbers 5126 and 5125 are glTF's names for "32-bit decimal"
    // and "32-bit whole number".
    let positions = numbers(
        &gltf,
        bin,
        &primitive["attributes"]["POSITION"],
        "VEC3",
        5126,
    )?;
    let colors = numbers(
        &gltf,
        bin,
        &primitive["attributes"]["COLOR_0"],
        "VEC3",
        5126,
    )?;
    let indices = numbers(&gltf, bin, &primitive["indices"], "SCALAR", 5125)?;
    let corner_count = positions.len() / 12;
    if colors.len() != positions.len() || indices.len() % 12 != 0 {
        return Err("the lists of positions, colours and triangles don't match".into());
    }

    // ---- Points. glTF's "right" is -x and Duck Forge's is +x, so x is
    // negated. Corners in the same place become one vertex.
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut vertex_of_corner: Vec<usize> = Vec::with_capacity(corner_count);
    let mut seen: HashMap<[u32; 3], usize> = HashMap::new();
    for corner in 0..corner_count {
        let point = [
            // `+ 0.0` turns -0 into 0, so that the two count as one place.
            -f32_at(positions, corner * 3) + 0.0,
            f32_at(positions, corner * 3 + 1) + 0.0,
            f32_at(positions, corner * 3 + 2) + 0.0,
        ];
        let vertex = *seen.entry(point.map(f32::to_bits)).or_insert_with(|| {
            vertices.push(point);
            vertices.len() - 1
        });
        vertex_of_corner.push(vertex);
    }

    // ---- Triangles. Negating x turns every triangle inside out, so the
    // second and third corners swap: that gives Duck Forge's order,
    // clockwise seen from the front. All three corners of a triangle
    // carry the same colour; it is read from the first.
    let mut triangles = Vec::with_capacity(indices.len() / 12);
    for triangle in 0..indices.len() / 12 {
        let corner =
            |which: usize| u32_at(indices, (triangle * 3 + which) * 4).map(|at| at as usize);
        let (a, b, c) = (corner(0)?, corner(1)?, corner(2)?);
        if a >= corner_count || b >= corner_count || c >= corner_count {
            return Err(format!(
                "triangle {triangle} names a corner that isn't there"
            ));
        }
        triangles.push(GlbTriangle {
            corners: [
                vertex_of_corner[a],
                vertex_of_corner[c],
                vertex_of_corner[b],
            ],
            color: [
                srgb(f32_at(colors, a * 3)),
                srgb(f32_at(colors, a * 3 + 1)),
                srgb(f32_at(colors, a * 3 + 2)),
            ],
        });
    }

    // ---- What Duck Mold adds: the name, the palette, the parts.
    let mut palette = Vec::new();
    for entry in extras["palette"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let name = entry["name"]
            .as_str()
            .ok_or("a palette colour has no name")?;
        let hex = entry["hex"]
            .as_str()
            .ok_or("a palette colour has no hex code")?;
        let channel = |at: usize| {
            hex.get(at..at + 2)
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .ok_or_else(|| format!("the palette colour \"{name}\" isn't written #RRGGBB"))
        };
        palette.push(GlbColor {
            name: name.to_owned(),
            rgb: [channel(1)?, channel(3)?, channel(5)?],
        });
    }
    let parts = part_list(&extras["parts"], "part", triangles.len())?;
    let assemblies = part_list(&extras["assemblies"], "assembly", triangles.len())?;

    Ok(GlbModel {
        name: extras["name"].as_str().unwrap_or("model").to_owned(),
        vertices,
        triangles,
        palette,
        parts,
        assemblies,
    })
}

/// A list of parts (or of assemblies) with their triangles, each checked
/// against the `count` of triangles there are. A missing list is empty.
fn part_list(list: &serde_json::Value, what: &str, count: usize) -> Result<Vec<GlbPart>, String> {
    let a = if what.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let mut parts = Vec::new();
    for entry in list.as_array().map(Vec::as_slice).unwrap_or_default() {
        let whole = |field: &str| {
            entry[field]
                .as_u64()
                .and_then(|number| usize::try_from(number).ok())
                .ok_or_else(|| format!("{a} {what} has no {}", field.replace('_', " ")))
        };
        let part = GlbPart {
            name: entry["name"]
                .as_str()
                .ok_or_else(|| format!("{a} {what} has no name"))?
                .to_owned(),
            first_triangle: whole("first_triangle")?,
            triangles: whole("triangles")?,
        };
        // Checked, so a damaged file can't overflow the sum.
        if part
            .first_triangle
            .checked_add(part.triangles)
            .is_none_or(|end| end > count)
        {
            return Err(format!(
                "the {what} \"{}\" names triangles that aren't there",
                part.name
            ));
        }
        parts.push(part);
    }
    Ok(parts)
}

/// The bytes of one list of numbers, found through the description: an
/// "accessor" says how many there are and of what kind, and points at a
/// "buffer view", which says where in the block of numbers they start.
fn numbers<'a>(
    gltf: &serde_json::Value,
    bin: &'a [u8],
    accessor: &serde_json::Value,
    kind: &str,
    component: u64,
) -> Result<&'a [u8], String> {
    let accessor =
        &gltf["accessors"][accessor.as_u64().ok_or("a list of numbers is missing")? as usize];
    if accessor["type"].as_str() != Some(kind)
        || accessor["componentType"].as_u64() != Some(component)
    {
        return Err("a list of numbers isn't the kind Duck Mold writes".into());
    }
    let per: usize = if kind == "VEC3" { 12 } else { 4 };
    let count = accessor["count"]
        .as_u64()
        .ok_or("a list of numbers has no count")?;
    let view = &gltf["bufferViews"][accessor["bufferView"]
        .as_u64()
        .ok_or("a list of numbers has no place")? as usize];
    // Checked sums: a damaged file can claim more numbers than any file
    // could hold, and that must be a refusal, not a crash.
    let span = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(per))
        .zip(
            view["byteOffset"]
                .as_u64()
                .unwrap_or(0)
                .checked_add(accessor["byteOffset"].as_u64().unwrap_or(0))
                .and_then(|start| usize::try_from(start).ok()),
        )
        .and_then(|(length, start)| Some(start..start.checked_add(length)?));
    span.and_then(|span| bin.get(span))
        .ok_or_else(|| "a list of numbers runs past the end of the file".to_owned())
}

/// A 32-bit whole number, stored smallest byte first.
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    let four = bytes.get(at..at + 4).ok_or("the file is cut short")?;
    Ok(u32::from_le_bytes([four[0], four[1], four[2], four[3]]))
}

/// The 32-bit decimal number number `at` in a list.
fn f32_at(list: &[u8], at: usize) -> f32 {
    let four = &list[at * 4..at * 4 + 4];
    f32::from_le_bytes([four[0], four[1], four[2], four[3]])
}

/// glTF stores colours as linear light, from 0 to 1. This turns one back
/// into the usual 0 to 255 of `Color::rgb` (the sRGB curve). It gives
/// back exactly the number the palette had.
fn srgb(linear: f32) -> u8 {
    let curved = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (curved * 255.0).round().clamp(0.0, 255.0) as u8
}

// ==== end of Duck Mold's .glb reader, version 2 =============================
