//! Importing glTF: `.gltf` and `.glb` files as static parts.
//!
//! glTF is the common 3D format; most modelling programs save it. A file
//! comes in as a [`MeshFile`], like an OBJ: triangles, each belonging to a
//! named material, so `colors = { glass = "yellow" }` paints a material
//! from the palette like any face. Each material's base colour comes too,
//! for faces the recipe gives no colour.
//!
//! What is read:
//!
//! - `.glb` (one file) and `.gltf` (JSON text, with its numbers in a
//!   separate `.bin` file or written inside it as base64 text);
//! - every mesh in the file's main scene, each placed, turned and sized as
//!   its object (node) says, by numbers or by a matrix, all merged;
//! - triangles in all three of glTF's layouts (separate, strips, fans),
//!   with corner numbers of any size, or none;
//! - material names and base colours.
//!
//! What is left out, with a note: lines and points, skins, animations,
//! shape keys (morph targets), cameras and textures. Normals and texture
//! coordinates are passed over without one: Duck Mold works out its own
//! normals and has no textures. What is refused, with a sentence:
//! compressed meshes, packed (quantized) numbers, and any extension the
//! file says it can't be read without.
//!
//! glTF's directions are the ones Duck Mold exports: y up, the model
//! facing +z, x to its left. So x is negated and each triangle's corners
//! are listed the other way round, which gives Duck Mold's directions and
//! Duck Forge's order; a model exported and imported again comes back
//! where it was. A `.glb` that Duck Mold wrote keeps its palette: its
//! materials are its palette colours.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::glb::{SRGB_TO_LINEAR, read_glb};
use crate::issue::{Code, Issue};
use crate::mesh::{DEFAULT_MATERIAL, MeshFile, has_no_area};

/// How deep objects may sit inside objects. Real files are a few deep; a
/// file whose objects contain each other in a loop stops here.
const MAX_NODE_DEPTH: usize = 64;

/// Reads the bytes of a `.gltf` or `.glb` file. `folder` is where the
/// file is, for the `.bin` files it names; `file` is its name in
/// messages.
///
/// Mistakes (as errors) and notes (as warnings) are added to `issues`.
/// `None` if the file can't be read or has no triangles.
pub fn read_gltf(
    bytes: &[u8],
    file: &str,
    folder: &Path,
    issues: &mut Vec<Issue>,
) -> Option<MeshFile> {
    let mut reader = Reader {
        file,
        folder,
        notes: BTreeSet::new(),
        read: Vec::new(),
    };
    match reader.read(bytes) {
        Ok(mut mesh) => {
            mesh.extra_files = reader.read;
            for note in reader.notes {
                issues.push(Issue::warning(Code::Skipped, "", note).in_file(file));
            }
            Some(mesh)
        }
        Err(Refusal(code, message)) => {
            issues.push(Issue::error(code, "", message).in_file(file));
            None
        }
    }
}

/// Whether these are the bytes of a `.glb` that Duck Mold wrote, which
/// [`crate::glb::read_glb`] reads with its parts and palette.
pub fn is_duck_mold_glb(bytes: &[u8]) -> bool {
    bytes.starts_with(b"glTF")
        && split_glb(bytes).is_ok_and(|(json, _)| {
            serde_json::from_slice::<Value>(json)
                .is_ok_and(|json| json["extras"]["duck_mold"].is_object())
        })
}

/// Why a file can't be read: the code, and a sentence.
struct Refusal(Code, String);

/// A file that is damaged, or isn't glTF.
fn bad(message: impl Into<String>) -> Refusal {
    Refusal(Code::BadMesh, message.into())
}

/// A file that needs something Duck Mold doesn't do.
fn unsupported(message: impl Into<String>) -> Refusal {
    Refusal(Code::Unsupported, message.into())
}

struct Reader<'a> {
    file: &'a str,
    folder: &'a Path,
    /// What was left out, said once each.
    notes: BTreeSet<String>,
    /// The other files read: a `.gltf`'s `.bin`.
    read: Vec<PathBuf>,
}

/// A 4 x 4 matrix, row by row: turns, stretches and moves a point.
type Matrix = [[f64; 4]; 4];

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

impl Reader<'_> {
    fn read(&mut self, bytes: &[u8]) -> Result<MeshFile, Refusal> {
        let (json_bytes, bin) = if bytes.starts_with(b"glTF") {
            split_glb(bytes)?
        } else {
            (bytes, None)
        };
        // Some programs start text files with a byte-order mark.
        let json_bytes = json_bytes
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(json_bytes);
        let json: Value = serde_json::from_slice(json_bytes).map_err(|error| {
            bad(format!(
                "this isn't a glTF file: its JSON can't be read ({error})"
            ))
        })?;
        let version = json["asset"]["version"].as_str().unwrap_or("");
        if !version.starts_with("2.") {
            return Err(unsupported(format!(
                "this is glTF version \"{version}\"; Duck Mold reads glTF 2.0"
            )));
        }
        if let Some(needed) = json["extensionsRequired"].as_array()
            && !needed.is_empty()
        {
            let names: Vec<&str> = needed.iter().filter_map(Value::as_str).collect();
            return Err(unsupported(format!(
                "this file can't be read without {}, which Duck Mold doesn't do: save it again without compression or other extensions",
                names.join(", ")
            )));
        }

        // A model Duck Mold wrote: read it as such, with its palette.
        if bin.is_some() && json["extras"]["duck_mold"].is_object() {
            return self.duck_mold(bytes);
        }

        let buffers = self.buffers(&json, bin)?;
        let mut mesh = MeshFile {
            file: self.file.to_owned(),
            ..MeshFile::default()
        };
        let mut flat = 0;
        let roots = scene_roots(&json)?;
        let mut on_the_way = Vec::new();
        for root in roots {
            self.node(
                &json,
                &buffers,
                root,
                &IDENTITY,
                &mut on_the_way,
                &mut mesh,
                &mut flat,
            )?;
        }
        if json["animations"].as_array().is_some_and(|a| !a.is_empty()) {
            self.note("its animations were left out: a part is the model as it stands");
        }
        if json["cameras"].as_array().is_some_and(|c| !c.is_empty()) {
            self.note("its cameras were left out");
        }
        let textured = json["materials"].as_array().is_some_and(|materials| {
            materials
                .iter()
                .any(|material| material["pbrMetallicRoughness"]["baseColorTexture"].is_object())
        });
        if textured {
            self.note("its textures were left out: a material's colour is its base colour alone");
        }
        if flat > 0 {
            self.note(&format!(
                "{flat} triangles of this file have no area and were left out"
            ));
        }
        if mesh.triangles.is_empty() {
            return Err(bad(
                "this file has no triangles to build: its main scene has no meshes made of triangles",
            ));
        }
        Ok(mesh)
    }

    fn note(&mut self, text: &str) {
        self.notes.insert(text.to_owned());
    }

    /// A `.glb` that Duck Mold wrote: its faces, with its palette colours
    /// as the materials.
    fn duck_mold(&mut self, bytes: &[u8]) -> Result<MeshFile, Refusal> {
        let model = read_glb(bytes, self.file).map_err(|issue| bad(issue.message))?;
        let mut mesh = MeshFile {
            file: self.file.to_owned(),
            from_duck_mold: true,
            ..MeshFile::default()
        };
        // The palette colours it uses, in the palette's order, so that
        // they join the model's palette in that order; then the faces
        // nobody coloured, if there are any.
        let used: BTreeSet<Option<usize>> = model.faces.iter().map(|face| face.color).collect();
        let order: Vec<Option<usize>> = (0..model.palette.len())
            .map(Some)
            .chain([None])
            .filter(|color| used.contains(color))
            .collect();
        for &color in &order {
            match color.and_then(|at| model.palette.get(at)) {
                Some(color) => {
                    mesh.materials.push(color.name.clone());
                    mesh.colors.push(Some(color.rgb));
                }
                None => {
                    mesh.materials.push(DEFAULT_MATERIAL.to_owned());
                    mesh.colors.push(None);
                }
            }
        }
        for face in &model.faces {
            let slot = order
                .iter()
                .position(|&color| color == face.color)
                .expect("every face's colour is listed");
            for triangle in face.triangles() {
                mesh.triangles
                    .push(triangle.map(|corner| corner.to_array()));
                mesh.material_of.push(slot);
            }
        }
        Ok(mesh)
    }

    /// Every buffer's bytes: the `.glb`'s own block, a `.bin` file beside
    /// the `.gltf`, or base64 text inside it.
    fn buffers(&mut self, json: &Value, bin: Option<&[u8]>) -> Result<Vec<Vec<u8>>, Refusal> {
        let mut buffers = Vec::new();
        for (at, buffer) in list(&json["buffers"]).iter().enumerate() {
            let length = whole(&buffer["byteLength"])
                .ok_or_else(|| bad(format!("buffer {at} has no byteLength")))?;
            let bytes = match buffer["uri"].as_str() {
                None => match (at, bin) {
                    (0, Some(bin)) => bin.to_vec(),
                    _ => {
                        return Err(bad(format!(
                            "buffer {at} names no file and there is no block of numbers for it"
                        )));
                    }
                },
                Some(uri) if uri.starts_with("data:") => {
                    let (_, text) = uri.split_once(";base64,").ok_or_else(|| {
                        bad(format!(
                            "buffer {at} is written inside the file, but not as base64"
                        ))
                    })?;
                    base64(text)
                        .ok_or_else(|| bad(format!("buffer {at}'s base64 text is broken")))?
                }
                Some(uri) => {
                    let name = percent_decoded(uri);
                    self.read.push(self.folder.join(&name));
                    std::fs::read(self.folder.join(&name)).map_err(|_| {
                        bad(format!(
                            "can't read \"{name}\", the file that holds its numbers: it must be in the same folder as the .gltf"
                        ))
                    })?
                }
            };
            if bytes.len() < length {
                return Err(bad(format!(
                    "buffer {at} should have {length} bytes but has {}",
                    bytes.len()
                )));
            }
            buffers.push(bytes);
        }
        Ok(buffers)
    }

    /// One object of the scene, the objects inside it, and their meshes.
    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        json: &Value,
        buffers: &[Vec<u8>],
        at: usize,
        parent: &Matrix,
        on_the_way: &mut Vec<usize>,
        mesh: &mut MeshFile,
        flat: &mut usize,
    ) -> Result<(), Refusal> {
        if on_the_way.contains(&at) || on_the_way.len() >= MAX_NODE_DEPTH {
            return Err(bad("its objects contain each other in a loop"));
        }
        let node = &json["nodes"][at];
        if !node.is_object() {
            return Err(bad(format!("it names object {at}, which isn't there")));
        }
        let place = multiply(parent, &local_matrix(node)?);
        if node["skin"].is_u64() {
            self.note("its skins (bones) were left out: a part is the model as it stands");
        }
        if let Some(index) = whole(&node["mesh"]) {
            self.mesh(json, buffers, index, &place, mesh, flat)?;
        }
        on_the_way.push(at);
        for child in list(&node["children"]) {
            let child = whole(child).ok_or_else(|| bad("an object's children aren't numbers"))?;
            self.node(json, buffers, child, &place, on_the_way, mesh, flat)?;
        }
        on_the_way.pop();
        Ok(())
    }

    /// One mesh, placed by `place`.
    fn mesh(
        &mut self,
        json: &Value,
        buffers: &[Vec<u8>],
        index: usize,
        place: &Matrix,
        mesh: &mut MeshFile,
        flat: &mut usize,
    ) -> Result<(), Refusal> {
        let source = &json["meshes"][index];
        if !source.is_object() {
            return Err(bad(format!("it names mesh {index}, which isn't there")));
        }
        // A mirrored object turns its triangles inside out.
        let mirrored = determinant(place) < 0.0;
        for primitive in list(&source["primitives"]) {
            let mode = whole(&primitive["mode"]).unwrap_or(4);
            if mode < 4 {
                self.note("its lines and points were left out: only triangles can be built");
                continue;
            }
            if mode > 6 {
                return Err(bad(format!("a mesh has the unknown drawing mode {mode}")));
            }
            if primitive["targets"]
                .as_array()
                .is_some_and(|targets| !targets.is_empty())
            {
                self.note("its shape keys (morph targets) were left out: each mesh comes in its first shape");
            }
            let position = whole(&primitive["attributes"]["POSITION"])
                .ok_or_else(|| bad("a mesh has no POSITION, so no corners"))?;
            let points = read_points(json, buffers, position)?;
            let corners: Vec<usize> = match whole(&primitive["indices"]) {
                Some(indices) => read_indices(json, buffers, indices)?,
                None => (0..points.len()).collect(),
            };
            if let Some(&wrong) = corners.iter().find(|&&corner| corner >= points.len()) {
                return Err(bad(format!(
                    "a triangle names corner {wrong}, but its mesh has {}",
                    points.len()
                )));
            }
            let triangles: Vec<[usize; 3]> = match mode {
                4 => {
                    if !corners.len().is_multiple_of(3) {
                        return Err(bad(format!(
                            "a list of triangles has {} corners, which isn't a multiple of three",
                            corners.len()
                        )));
                    }
                    corners.chunks(3).map(|t| [t[0], t[1], t[2]]).collect()
                }
                // A strip: each corner after the first two makes a
                // triangle with the two before it, every other one turned
                // round so they all face the same way.
                5 => (0..corners.len().saturating_sub(2))
                    .map(|at| {
                        if at % 2 == 0 {
                            [corners[at], corners[at + 1], corners[at + 2]]
                        } else {
                            [corners[at], corners[at + 2], corners[at + 1]]
                        }
                    })
                    .collect(),
                // A fan: every triangle shares the first corner.
                _ => (0..corners.len().saturating_sub(2))
                    .map(|at| [corners[at + 1], corners[at + 2], corners[0]])
                    .collect(),
            };

            let slot = self.material_slot(json, primitive, mesh);
            for [a, b, c] in triangles {
                // Into Duck Mold's directions: x negated, so the corners
                // are listed the other way round (unless a mirror already
                // turned them).
                let corner = |at: usize| {
                    let [x, y, z] = apply(place, points[at]);
                    [-x + 0.0, y, z]
                };
                let triangle = if mirrored {
                    [corner(a), corner(b), corner(c)]
                } else {
                    [corner(a), corner(c), corner(b)]
                };
                if has_no_area(&triangle) {
                    *flat += 1;
                    continue;
                }
                mesh.triangles.push(triangle);
                mesh.material_of.push(slot);
            }
        }
        Ok(())
    }

    /// Which of the mesh's materials a part of a mesh has, adding it the
    /// first time it is used.
    fn material_slot(&self, json: &Value, primitive: &Value, mesh: &mut MeshFile) -> usize {
        let (name, color) = match whole(&primitive["material"]) {
            Some(at) => {
                let material = &json["materials"][at];
                let name = material["name"]
                    .as_str()
                    .filter(|name| !name.trim().is_empty())
                    .map_or_else(|| format!("material_{at}"), str::to_owned);
                // glTF's default base colour is white.
                let factor = &material["pbrMetallicRoughness"]["baseColorFactor"];
                let channel = |at: usize| factor[at].as_f64().unwrap_or(1.0);
                let color = material
                    .is_object()
                    .then(|| [0, 1, 2].map(|at| srgb(channel(at))));
                (name, color)
            }
            None => (DEFAULT_MATERIAL.to_owned(), None),
        };
        match mesh.materials.iter().position(|known| *known == name) {
            Some(slot) => slot,
            None => {
                mesh.materials.push(name);
                mesh.colors.push(color);
                mesh.materials.len() - 1
            }
        }
    }
}

/// The JSON block and the block of numbers of a `.glb`.
fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), Refusal> {
    let word = |at: usize| -> Result<usize, Refusal> {
        let four = bytes
            .get(
                at..at
                    .checked_add(4)
                    .ok_or_else(|| bad("the file is cut short"))?,
            )
            .ok_or_else(|| bad("the file is cut short"))?;
        Ok(u32::from_le_bytes([four[0], four[1], four[2], four[3]]) as usize)
    };
    if word(4)? != 2 {
        return Err(unsupported("this .glb isn't glTF version 2"));
    }
    if word(8)? != bytes.len() {
        return Err(bad("the file is cut short or has extra bytes"));
    }
    let mut json = None;
    let mut bin = None;
    let mut at = 12;
    while at < bytes.len() {
        let length = word(at)?;
        let kind = bytes
            .get(at + 4..at + 8)
            .ok_or_else(|| bad("the file is cut short"))?;
        let start = at + 8;
        let end = start
            .checked_add(length)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| bad("a block of the file runs past its end"))?;
        match kind {
            b"JSON" if json.is_none() => json = Some(&bytes[start..end]),
            b"BIN\0" if bin.is_none() => bin = Some(&bytes[start..end]),
            _ => {} // a block for something else: passed over
        }
        at = end;
    }
    let json = json.ok_or_else(|| bad("the .glb has no JSON block"))?;
    Ok((json, bin))
}

/// The objects the main scene starts from: the scene `scene` names, the
/// first scene, or, in a file with no scenes, every object that isn't
/// inside another.
fn scene_roots(json: &Value) -> Result<Vec<usize>, Refusal> {
    let scenes = list(&json["scenes"]);
    if !scenes.is_empty() {
        let at = whole(&json["scene"]).unwrap_or(0);
        let scene = scenes
            .get(at)
            .ok_or_else(|| bad(format!("its main scene is scene {at}, which isn't there")))?;
        return list(&scene["nodes"])
            .iter()
            .map(|node| whole(node).ok_or_else(|| bad("a scene's objects aren't numbers")))
            .collect();
    }
    let nodes = list(&json["nodes"]);
    let inside: BTreeSet<usize> = nodes
        .iter()
        .flat_map(|node| list(&node["children"]).iter().filter_map(whole))
        .collect();
    Ok((0..nodes.len()).filter(|at| !inside.contains(at)).collect())
}

/// An object's own place: its `matrix`, or its `translation`, `rotation`
/// and `scale`, done scale first.
fn local_matrix(node: &Value) -> Result<Matrix, Refusal> {
    if let Some(numbers) = node["matrix"].as_array() {
        let numbers: Option<Vec<f64>> = numbers.iter().map(Value::as_f64).collect();
        let numbers = numbers
            .filter(|numbers| numbers.len() == 16)
            .ok_or_else(|| bad("an object's matrix isn't 16 numbers"))?;
        // glTF writes a matrix column by column.
        let mut matrix = IDENTITY;
        for (column, row) in (0..4).flat_map(|column| (0..4).map(move |row| (column, row))) {
            matrix[row][column] = numbers[column * 4 + row];
        }
        return Ok(matrix);
    }
    let numbers = |field: &str, count: usize, default: &[f64]| -> Result<Vec<f64>, Refusal> {
        match node[field].as_array() {
            None => Ok(default.to_vec()),
            Some(list) => list
                .iter()
                .map(Value::as_f64)
                .collect::<Option<Vec<f64>>>()
                .filter(|list| list.len() == count)
                .ok_or_else(|| bad(format!("an object's {field} isn't {count} numbers"))),
        }
    };
    let t = numbers("translation", 3, &[0.0, 0.0, 0.0])?;
    let q = numbers("rotation", 4, &[0.0, 0.0, 0.0, 1.0])?;
    let s = numbers("scale", 3, &[1.0, 1.0, 1.0])?;
    let length = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let (x, y, z, w) = if length > 0.0 {
        (q[0] / length, q[1] / length, q[2] / length, q[3] / length)
    } else {
        (0.0, 0.0, 0.0, 1.0)
    };
    let turn = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let mut matrix = IDENTITY;
    for row in 0..3 {
        for column in 0..3 {
            matrix[row][column] = turn[row][column] * s[column];
        }
        matrix[row][3] = t[row];
    }
    Ok(matrix)
}

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (row, out_row) in out.iter_mut().enumerate() {
        for (column, cell) in out_row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[row][k] * b[k][column]).sum();
        }
    }
    out
}

fn apply(matrix: &Matrix, [x, y, z]: [f64; 3]) -> [f64; 3] {
    let row = |r: usize| matrix[r][0] * x + matrix[r][1] * y + matrix[r][2] * z + matrix[r][3];
    [row(0), row(1), row(2)]
}

/// Negative for a matrix that mirrors.
fn determinant(m: &Matrix) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Where an accessor's numbers are: the bytes they start at, how far
/// apart each element is, how many elements, and the bytes they come
/// from.
struct Span<'b> {
    bytes: &'b [u8],
    start: usize,
    stride: usize,
    count: usize,
}

impl Span<'_> {
    /// The `number`-th element's bytes start here.
    fn at(&self, number: usize) -> usize {
        self.start + number * self.stride
    }
}

/// Finds an accessor's bytes, checking every size against the file
/// before using it. `size` is how big one element is.
fn span<'b>(
    json: &Value,
    buffers: &'b [Vec<u8>],
    accessor: &Value,
    size: usize,
) -> Result<Span<'b>, Refusal> {
    if accessor["sparse"].is_object() {
        return Err(unsupported(
            "it stores some corners as changes to others (a sparse accessor), which Duck Mold doesn't read",
        ));
    }
    let count = whole(&accessor["count"]).ok_or_else(|| bad("a list of numbers has no count"))?;
    let view_at =
        whole(&accessor["bufferView"]).ok_or_else(|| bad("a list of numbers has no place"))?;
    let view = &json["bufferViews"][view_at];
    let buffer_at = whole(&view["buffer"]).ok_or_else(|| bad("a buffer view has no buffer"))?;
    let bytes = buffers
        .get(buffer_at)
        .ok_or_else(|| bad(format!("it names buffer {buffer_at}, which isn't there")))?;
    let view_start = whole(&view["byteOffset"]).unwrap_or(0);
    let view_length =
        whole(&view["byteLength"]).ok_or_else(|| bad("a buffer view has no byteLength"))?;
    let stride = whole(&view["byteStride"]).unwrap_or(size);
    if stride < size {
        return Err(bad(
            "a list of numbers' elements overlap (byteStride too small)",
        ));
    }
    let start = view_start
        .checked_add(whole(&accessor["byteOffset"]).unwrap_or(0))
        .ok_or_else(|| bad("a list of numbers starts past the end of the file"))?;
    let end = match count {
        0 => Some(start),
        _ => (count - 1)
            .checked_mul(stride)
            .and_then(|last| last.checked_add(start))
            .and_then(|last| last.checked_add(size)),
    };
    let view_end = view_start.checked_add(view_length);
    match (end, view_end) {
        (Some(end), Some(view_end)) if end <= view_end && view_end <= bytes.len() => Ok(Span {
            bytes,
            start,
            stride,
            count,
        }),
        _ => Err(bad("a list of numbers runs past the end of its buffer")),
    }
}

/// An accessor's `[x, y, z]` points, as 32-bit decimals.
fn read_points(json: &Value, buffers: &[Vec<u8>], at: usize) -> Result<Vec<[f64; 3]>, Refusal> {
    let accessor = &json["accessors"][at];
    if accessor["type"].as_str() != Some("VEC3") {
        return Err(bad("a mesh's corners aren't three numbers each"));
    }
    if accessor["componentType"].as_u64() != Some(5126) {
        return Err(unsupported(
            "its corners are packed into whole numbers (quantized), which Duck Mold doesn't read: save it again without compression",
        ));
    }
    let span = span(json, buffers, accessor, 12)?;
    Ok((0..span.count)
        .map(|number| {
            let at = span.at(number);
            [0, 1, 2].map(|axis| {
                let four = &span.bytes[at + axis * 4..at + axis * 4 + 4];
                f64::from(f32::from_le_bytes([four[0], four[1], four[2], four[3]]))
            })
        })
        .collect())
}

/// An accessor's corner numbers: 8, 16 or 32-bit whole numbers.
fn read_indices(json: &Value, buffers: &[Vec<u8>], at: usize) -> Result<Vec<usize>, Refusal> {
    let accessor = &json["accessors"][at];
    if accessor["type"].as_str() != Some("SCALAR") {
        return Err(bad("a mesh's corner numbers aren't one number each"));
    }
    let size = match accessor["componentType"].as_u64() {
        Some(5121) => 1,
        Some(5123) => 2,
        Some(5125) => 4,
        _ => return Err(bad("a mesh's corner numbers aren't whole numbers")),
    };
    let span = span(json, buffers, accessor, size)?;
    Ok((0..span.count)
        .map(|number| {
            let at = span.at(number);
            let b = &span.bytes[at..at + size];
            match size {
                1 => usize::from(b[0]),
                2 => usize::from(u16::from_le_bytes([b[0], b[1]])),
                _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
            }
        })
        .collect())
}

/// A whole number from the JSON, if it is one.
fn whole(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
}

/// A JSON list, or an empty one.
fn list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

/// A linear colour channel (0 to 1, as glTF stores colours) as the usual
/// 0 to 255: the nearest entry of Duck Mold's own table, so every
/// computer gives the same number.
fn srgb(linear: f64) -> u8 {
    let linear = linear.clamp(0.0, 1.0) as f32;
    let above = SRGB_TO_LINEAR.partition_point(|&entry| entry < linear);
    let nearest = match above {
        0 => 0,
        256 => 255,
        _ if linear - SRGB_TO_LINEAR[above - 1] <= SRGB_TO_LINEAR[above] - linear => above - 1,
        _ => above,
    };
    nearest as u8
}

/// Base64 text as bytes. `None` if it isn't base64.
fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let digits = digits
        .strip_suffix(b"==")
        .or_else(|| digits.strip_suffix(b"="))
        .unwrap_or(&digits);
    if digits.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let mut bits = 0u32;
        for (at, &c) in chunk.iter().enumerate() {
            bits |= value(c)? << (18 - 6 * at);
        }
        let bytes = bits.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(out)
}

/// A file name as a glTF file writes it, with `%20` back to a space.
fn percent_decoded(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_reads_with_and_without_padding() {
        assert_eq!(base64("SGVsbG8=").unwrap(), b"Hello");
        assert_eq!(base64("SGVsbG8h").unwrap(), b"Hello!");
        assert_eq!(base64("SGk").unwrap(), b"Hi");
        assert_eq!(base64("").unwrap(), b"");
        assert!(base64("S").is_none());
        assert!(base64("SG*k").is_none());
    }

    #[test]
    fn colours_come_back_exactly() {
        for (at, &linear) in SRGB_TO_LINEAR.iter().enumerate() {
            assert_eq!(usize::from(srgb(f64::from(linear))), at);
        }
        assert_eq!(srgb(-1.0), 0);
        assert_eq!(srgb(2.0), 255);
    }

    #[test]
    fn spaces_in_file_names_are_decoded() {
        assert_eq!(percent_decoded("my%20lamp.bin"), "my lamp.bin");
        assert_eq!(percent_decoded("100%"), "100%");
    }

    #[test]
    fn a_quarter_turn_around_y_turns_forward_to_the_side() {
        // glTF's rotation [0, sin 45°, 0, cos 45°] turns +z to +x.
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let node = serde_json::json!({ "rotation": [0.0, half, 0.0, half], "translation": [1.0, 2.0, 3.0] });
        let matrix = local_matrix(&node).ok().unwrap();
        let [x, y, z] = apply(&matrix, [0.0, 0.0, 1.0]);
        assert!((x - 2.0).abs() < 1e-12 && (y - 2.0).abs() < 1e-12 && (z - 3.0).abs() < 1e-12);
    }
}
