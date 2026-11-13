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

// ==== Duck Mold's .glb reader, version 4 ====================================
//
// Version 4: files laid out as a tree of parts (Duck Mold's Phase 5), each
// part an object at the join it turns about: `joins`, and each socket's
// `join`. Files from before still load, as they always did. Version 3: the
// model's named sockets, in `sockets`, so a game can put a
// hat on `head.top`. Version 2: a part made of parts (an "assembly") has a
// triangle range too, in `assemblies`; and sizes read from the file are
// checked before they are added or multiplied, so a damaged file is refused
// instead of overflowing. Version 1 was Duck Mold's Phase 0.

/// A model read from a Duck Mold `.glb`, in Duck Forge's directions:
/// x is right, y is up, z is forward, 1 unit is about a metre. The model
/// faces forward (+z) and stands on y = 0.
#[derive(Debug, Clone, PartialEq)]
pub struct GlbModel {
    /// The model's name.
    pub name: String,
    /// The corner points. Corners of a part that are in the same place
    /// are shared, so a box has 8. In a file laid out as a tree, no two
    /// parts share a corner (each part's are next to each other: see
    /// [`GlbJoin`]), so that a part can move by itself.
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
    /// The model's sockets: named points, each facing a way, where things
    /// join. The model's own have bare names (`top`), a part's are
    /// `part.socket` (`head.top`), a part inside an assembly keeps its
    /// path (`left_arm/hand.bottom`). Parts without a name have none
    /// here. Empty for files from before version 3.
    pub sockets: Vec<GlbSocket>,
    /// The tree the model moves by: one join for every part and every
    /// assembly, each hanging from the one it is joined to (or inside),
    /// and always after it in this list, so that one pass from first to
    /// last meets every join after the one it hangs from. Empty for a
    /// file that isn't laid out as a tree (from before version 4): such
    /// a model can be drawn, not posed.
    pub joins: Vec<GlbJoin>,
}

/// One join of the model's tree: a part (or an assembly), the point it
/// turns about, and what it hangs from.
#[derive(Debug, Clone, PartialEq)]
pub struct GlbJoin {
    /// The part's name, as in `parts` and `assemblies`: `head`,
    /// `left_arm`, `left_arm/hand`.
    pub name: String,
    /// The join it hangs from, as a place in `joins`: turning that one
    /// carries this one along. `None`: it hangs from the model itself.
    pub parent: Option<usize>,
    /// The point it turns about, in the model's own space, as the model
    /// stands at rest: the socket it is joined to, or its own middle.
    pub pivot: [f32; 3],
    /// The first of the part's own corners, as a place in `vertices`.
    pub first_vertex: usize,
    /// How many corners it has of its own: none for an assembly, whose
    /// corners are those of the parts inside it.
    pub vertices: usize,
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

/// One named socket: a point on the model and the way it faces, in the
/// model's own space (before the model is moved or turned), like the
/// vertices.
#[derive(Debug, Clone, PartialEq)]
pub struct GlbSocket {
    /// Its name: `top`, `head.top`, `left_arm/hand.bottom`.
    pub name: String,
    /// Where it is.
    pub at: [f32; 3],
    /// The way it faces, 1 long: something joined there faces the other
    /// way, the two pressed together.
    pub facing: [f32; 3],
    /// The join that carries it, as a place in `joins`: when that part
    /// moves, the socket moves with it. `None` for a socket of the whole
    /// model that no part carries, and in files from before version 4.
    pub join: Option<usize>,
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

    // ---- The description. Version 0 of it (Duck Mold's Phases 0 to 4)
    // is one object with one list of triangles. Version 1 (Phase 5) is a
    // tree of objects, one for every part, each with its own triangles,
    // measured from the point the part turns about.
    let extras = &gltf["extras"]["duck_mold"];
    let tree = match extras["version"].as_i64() {
        Some(0) => false,
        Some(1) => true,
        Some(_) => {
            return Err(
                "this .glb was written by a newer Duck Mold than this reader knows: update Duck Forge"
                    .into(),
            );
        }
        None => {
            return Err(
                "this .glb wasn't written by Duck Mold (no extras.duck_mold version)".into(),
            );
        }
    };

    // ---- The tree: which object hangs from which, and where each is.
    // An object's place is its own shift added to those of everything it
    // hangs from. At rest nothing is turned or stretched: only shifted.
    let mut pivots: Vec<[f64; 3]> = Vec::new();
    let mut parents: Vec<Option<usize>> = Vec::new();
    // The objects in the order of a walk down the tree from the model
    // (object 0), so each comes after the one it hangs from.
    let mut walk: Vec<usize> = Vec::new();
    if tree {
        let nodes = gltf["nodes"]
            .as_array()
            .ok_or("a .glb laid out as a tree has no objects")?;
        pivots = vec![[0.0; 3]; nodes.len()];
        parents = vec![None; nodes.len()];
        let mut reached = vec![false; nodes.len()];
        let mut waiting = vec![0usize];
        while let Some(node) = waiting.pop() {
            let object = nodes
                .get(node)
                .ok_or("an object names one that isn't there")?;
            if std::mem::replace(&mut reached[node], true) {
                return Err("an object hangs from two others, or from itself".into());
            }
            if ["rotation", "scale", "matrix"]
                .iter()
                .any(|field| !object[*field].is_null())
            {
                return Err(
                    "an object is turned or stretched at rest, which Duck Mold never writes".into(),
                );
            }
            let mut place = match parents[node] {
                Some(parent) => pivots[parent],
                None => [0.0; 3],
            };
            match object["translation"].as_array().map(Vec::as_slice) {
                None => {}
                Some([x, y, z]) => match (x.as_f64(), y.as_f64(), z.as_f64()) {
                    (Some(x), Some(y), Some(z)) => {
                        // x is negated, like the points.
                        place = [place[0] - x, place[1] + y, place[2] + z];
                    }
                    _ => return Err("an object's place isn't three numbers".into()),
                },
                Some(_) => return Err("an object's place isn't three numbers".into()),
            }
            // Kept as 32-bit numbers, like every number of the file: the
            // corners of its part were measured from exactly there.
            pivots[node] = place.map(|number| f64::from(number as f32));
            walk.push(node);
            // Pushed last to first, so they are taken first to last.
            let children = object["children"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            for child in children.iter().rev() {
                let child = child
                    .as_u64()
                    .and_then(|child| usize::try_from(child).ok())
                    .filter(|child| *child < nodes.len())
                    .ok_or("an object names one that isn't there")?;
                parents[child] = Some(node);
                waiting.push(child);
            }
        }
    }

    // ---- The lumps of triangles: the whole model's in a file from
    // before; in a tree, each part's own, in the order of `parts`. Each
    // lump is read the same way.
    //
    // Points: glTF's "right" is -x and Duck Forge's is +x, so x is
    // negated. Corners of a lump in the same place become one vertex.
    // Triangles: negating x turns every triangle inside out, so the
    // second and third corners swap: that gives Duck Forge's order,
    // clockwise seen from the front. All three corners of a triangle
    // carry the same colour; it is read from the first.
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut triangles: Vec<GlbTriangle> = Vec::new();
    // In a tree: for each object, the name of its part and its corners.
    let mut named: Vec<Option<(String, usize, usize)>> = vec![None; pivots.len()];
    if tree {
        for (list, with_shape) in [(&extras["parts"], true), (&extras["assemblies"], false)] {
            for entry in list.as_array().map(Vec::as_slice).unwrap_or_default() {
                let name = entry["name"].as_str().ok_or("a part has no name")?;
                let node = entry["node"]
                    .as_u64()
                    .and_then(|node| usize::try_from(node).ok())
                    .filter(|node| *node > 0 && *node < pivots.len() && parents[*node].is_some())
                    .ok_or_else(|| format!("the part \"{name}\" has no object in the tree"))?;
                if named[node].is_some() {
                    return Err(format!(
                        "the part \"{name}\" shares its object with another"
                    ));
                }
                let first_vertex = vertices.len();
                if with_shape {
                    // The parts are in the order of their triangles.
                    if entry["first_triangle"].as_u64() != Some(triangles.len() as u64) {
                        return Err(format!("the part \"{name}\" isn't where its triangles are"));
                    }
                    // (A part with no triangles has no mesh.)
                    let before = triangles.len();
                    if let Some(mesh) = gltf["nodes"][node]["mesh"].as_u64() {
                        add_lump(
                            &gltf,
                            bin,
                            &gltf["meshes"][mesh as usize]["primitives"][0],
                            pivots[node],
                            &mut vertices,
                            &mut triangles,
                        )?;
                    }
                    if entry["triangles"].as_u64() != Some((triangles.len() - before) as u64) {
                        return Err(format!(
                            "the part \"{name}\" names triangles that aren't there"
                        ));
                    }
                }
                named[node] = Some((name.to_owned(), first_vertex, vertices.len() - first_vertex));
            }
        }
    } else {
        add_lump(
            &gltf,
            bin,
            &gltf["meshes"][0]["primitives"][0],
            [0.0; 3],
            &mut vertices,
            &mut triangles,
        )?;
    }

    // ---- The joins: every object but the model itself, in the order of
    // the walk.
    let mut join_of_node: Vec<Option<usize>> = vec![None; pivots.len()];
    let mut joins: Vec<GlbJoin> = Vec::new();
    for &node in walk.iter().skip(1) {
        let (name, first_vertex, count) = named[node]
            .take()
            .ok_or("an object of the tree is no part of the model")?;
        join_of_node[node] = Some(joins.len());
        joins.push(GlbJoin {
            name,
            parent: parents[node].and_then(|parent| join_of_node[parent]),
            pivot: pivots[node].map(|number| number as f32 + 0.0),
            first_vertex,
            vertices: count,
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

    // The sockets, with x negated like the points.
    let mut sockets = Vec::new();
    for entry in extras["sockets"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let name = entry["name"].as_str().ok_or("a socket has no name")?;
        let three = |field: &str| match entry[field].as_array().map(Vec::as_slice) {
            Some([x, y, z]) => match (x.as_f64(), y.as_f64(), z.as_f64()) {
                (Some(x), Some(y), Some(z)) => {
                    Ok([-(x as f32) + 0.0, y as f32 + 0.0, z as f32 + 0.0])
                }
                _ => Err(format!("the socket \"{name}\" has a broken {field}")),
            },
            _ => Err(format!("the socket \"{name}\" has no {field}")),
        };
        // The object that carries it; the model itself carries the rest.
        let join = match entry["node"].as_u64() {
            None | Some(0) => None,
            Some(node) => Some(
                usize::try_from(node)
                    .ok()
                    .and_then(|node| join_of_node.get(node).copied().flatten())
                    .ok_or_else(|| {
                        format!("the socket \"{name}\" is on an object that isn't there")
                    })?,
            ),
        };
        sockets.push(GlbSocket {
            name: name.to_owned(),
            at: three("at")?,
            facing: three("facing")?,
            join,
        });
    }

    Ok(GlbModel {
        name: extras["name"].as_str().unwrap_or("model").to_owned(),
        vertices,
        triangles,
        palette,
        parts,
        assemblies,
        sockets,
        joins,
    })
}

/// Reads one lump of triangles (the whole model's in a file from before
/// version 4, one part's in a tree) and adds its points and triangles to
/// the model's. `from` is where its points are measured from.
fn add_lump(
    gltf: &serde_json::Value,
    bin: &[u8],
    primitive: &serde_json::Value,
    from: [f64; 3],
    vertices: &mut Vec<[f32; 3]>,
    triangles: &mut Vec<GlbTriangle>,
) -> Result<(), String> {
    if primitive["mode"].as_u64().unwrap_or(4) != 4 {
        return Err("the mesh isn't a list of triangles".into());
    }
    // The numbers 5126 and 5125 are glTF's names for "32-bit decimal"
    // and "32-bit whole number".
    let positions = numbers(
        gltf,
        bin,
        &primitive["attributes"]["POSITION"],
        "VEC3",
        5126,
    )?;
    let colors = numbers(gltf, bin, &primitive["attributes"]["COLOR_0"], "VEC3", 5126)?;
    let indices = numbers(gltf, bin, &primitive["indices"], "SCALAR", 5125)?;
    let corner_count = positions.len() / 12;
    if colors.len() != positions.len() || indices.len() % 12 != 0 {
        return Err("the lists of positions, colours and triangles don't match".into());
    }
    let mut vertex_of_corner: Vec<usize> = Vec::with_capacity(corner_count);
    let mut seen: HashMap<[u32; 3], usize> = HashMap::new();
    for corner in 0..corner_count {
        // Measured from `from` (zero, in a file from before), and
        // added up with more digits than are kept. `+ 0.0` turns -0
        // into 0, so that the two count as one place.
        let point = [
            (from[0] - f64::from(f32_at(positions, corner * 3))) as f32 + 0.0,
            (from[1] + f64::from(f32_at(positions, corner * 3 + 1))) as f32 + 0.0,
            (from[2] + f64::from(f32_at(positions, corner * 3 + 2))) as f32 + 0.0,
        ];
        let vertex = *seen.entry(point.map(f32::to_bits)).or_insert_with(|| {
            vertices.push(point);
            vertices.len() - 1
        });
        vertex_of_corner.push(vertex);
    }
    for triangle in 0..indices.len() / 12 {
        let corner =
            |which: usize| u32_at(indices, (triangle * 3 + which) * 4).map(|at| at as usize);
        let (a, b, c) = (corner(0)?, corner(1)?, corner(2)?);
        if a >= corner_count || b >= corner_count || c >= corner_count {
            return Err(format!(
                "triangle {} names a corner that isn't there",
                triangles.len()
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
    Ok(())
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

// ==== end of Duck Mold's .glb reader, version 4 =============================
