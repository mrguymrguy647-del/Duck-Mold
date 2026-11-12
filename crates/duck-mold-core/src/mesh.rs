//! Meshes read from files, to use as static parts.
//!
//! A [`MeshFile`] is what an imported file comes down to: triangles, each
//! belonging to a named material. Duck Mold treats the material names as
//! the part's face names, so `colors = { Moss = "leaf" }` paints them
//! from the palette like any other face.
//!
//! This phase reads OBJ files. Importing glTF is planned for a later
//! one.

use crate::issue::{Code, Issue};
use crate::source::Pos;

/// A mesh read from a file, already in Duck Mold's directions (x right,
/// y up, z forward) with every triangle's corners in Duck Forge's order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshFile {
    /// The file, as it was named.
    pub file: String,
    /// The triangles: three corners each, every corner an `[x, y, z]`.
    pub triangles: Vec<[[f64; 3]; 3]>,
    /// For each triangle, which entry of `materials` it belongs to.
    pub material_of: Vec<usize>,
    /// The material names, in the order the file first uses them.
    pub materials: Vec<String>,
}

/// The most mistakes reported for one mesh file. A damaged file can have
/// thousands; the first few say enough.
const MAX_REPORTED: usize = 8;

/// The name for faces that come before any material is named.
pub const DEFAULT_MATERIAL: &str = "default";

/// Reads the text of an OBJ file.
///
/// What is read: points (`v`), faces (`f`, of three or more corners; a
/// face with more is cut into triangles) and material names (`usemtl`).
/// Everything else is passed over: normals, texture coordinates,
/// smoothing, object and group names, and the material file. Lines,
/// points and curves are passed over with a note, since only faces can
/// be built.
///
/// An OBJ file is read the way glTF means its directions, which is also
/// how Blender writes OBJ by default: y up, the model facing +z, and x
/// pointing to the model's left. So x is negated and each triangle's
/// corners are listed the other way round, which gives Duck Mold's
/// directions (x right) and Duck Forge's corner order.
///
/// Mistakes (as errors) and notes (as warnings) are added to `issues`.
/// `None` if the file has mistakes or no faces.
pub fn read_obj(text: &str, file: &str, issues: &mut Vec<Issue>) -> Option<MeshFile> {
    let mut mesh = MeshFile {
        file: file.to_owned(),
        ..MeshFile::default()
    };
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut material = DEFAULT_MATERIAL.to_owned();
    let mut mistakes = 0;
    let mut skipped = 0;
    let mut flat = 0;
    let mut mistake = |issues: &mut Vec<Issue>, line: usize, message: String| {
        mistakes += 1;
        if mistakes <= MAX_REPORTED {
            issues.push(
                Issue::error(Code::BadMesh, "", message)
                    .in_file(file)
                    .at(Pos { line, column: 1 }),
            );
        }
    };

    for (at, line) in text.lines().enumerate() {
        let number = at + 1;
        // Anything after a `#` is a comment.
        let line = line.split('#').next().unwrap_or("").trim();
        let mut words = line.split_whitespace();
        let Some(keyword) = words.next() else {
            continue;
        };
        match keyword {
            "v" => {
                let numbers: Vec<Option<f64>> = words
                    .take(3)
                    .map(|word| word.parse::<f64>().ok().filter(|number| number.is_finite()))
                    .collect();
                match numbers.as_slice() {
                    [Some(x), Some(y), Some(z)] => points.push([-x, *y, *z]),
                    _ => {
                        // Keep the count of points right, so that later
                        // faces don't all point at the wrong ones.
                        points.push([0.0; 3]);
                        mistake(
                            issues,
                            number,
                            "a point needs three numbers: v 1.0 2.0 0.5".to_owned(),
                        );
                    }
                }
            }
            "f" => {
                let mut corners: Vec<[f64; 3]> = Vec::new();
                let mut good = true;
                for word in words {
                    // A corner is `7`, `7/2`, `7/2/5` or `7//5`: only the
                    // first number, the point, is used.
                    let first = word.split('/').next().unwrap_or("");
                    let point = match first.parse::<i64>() {
                        // Counting from 1...
                        Ok(index) if index >= 1 => points.get(index as usize - 1),
                        // ...or, with a minus, back from the last point.
                        Ok(index) if index < 0 => points
                            .len()
                            .checked_sub(index.unsigned_abs() as usize)
                            .and_then(|at| points.get(at)),
                        _ => None,
                    };
                    match point {
                        Some(point) => corners.push(*point),
                        None => {
                            good = false;
                            mistake(
                                issues,
                                number,
                                format!(
                                    "this face names the point \"{first}\", but the file has {} points so far (points are counted from 1)",
                                    points.len()
                                ),
                            );
                            break;
                        }
                    }
                }
                if !good {
                    continue;
                }
                if corners.len() < 3 {
                    mistake(
                        issues,
                        number,
                        format!(
                            "a face needs at least three corners; this one has {}",
                            corners.len()
                        ),
                    );
                    continue;
                }
                // Cut the face into a fan of triangles from its first
                // corner, each listed the other way round because x was
                // negated.
                for next in 1..corners.len() - 1 {
                    let triangle = [corners[0], corners[next + 1], corners[next]];
                    if has_no_area(&triangle) {
                        flat += 1;
                        continue;
                    }
                    let slot = match mesh.materials.iter().position(|name| *name == material) {
                        Some(slot) => slot,
                        None => {
                            mesh.materials.push(material.clone());
                            mesh.materials.len() - 1
                        }
                    };
                    mesh.triangles.push(triangle);
                    mesh.material_of.push(slot);
                }
            }
            "usemtl" => {
                // A material's name is the rest of the line.
                let name = line[keyword.len()..].trim();
                material = if name.is_empty() {
                    DEFAULT_MATERIAL.to_owned()
                } else {
                    name.to_owned()
                };
            }
            // Read by other programs, not needed here.
            "vn" | "vt" | "vp" | "o" | "g" | "s" | "mtllib" | "mg" => {}
            // Things that aren't faces.
            _ => skipped += 1,
        }
    }

    if mistakes > MAX_REPORTED {
        issues.push(
            Issue::error(
                Code::BadMesh,
                "",
                format!("and {} more mistakes like these", mistakes - MAX_REPORTED),
            )
            .in_file(file),
        );
    }
    if mistakes > 0 {
        return None;
    }
    if mesh.triangles.is_empty() {
        issues.push(
            Issue::error(
                Code::BadMesh,
                "",
                "this file has no faces to build (it needs `v` lines for points and `f` lines for faces)",
            )
            .in_file(file),
        );
        return None;
    }
    if skipped > 0 {
        issues.push(
            Issue::warning(
                Code::Skipped,
                "",
                format!(
                    "{skipped} lines of this file were skipped: only points, faces and material names are read (no lines, curves or other extras)"
                ),
            )
            .in_file(file),
        );
    }
    if flat > 0 {
        issues.push(
            Issue::warning(
                Code::Skipped,
                "",
                format!("{flat} triangles of this file have no area and were left out"),
            )
            .in_file(file),
        );
    }
    Some(mesh)
}

/// True for a triangle squashed to a line or a point.
fn has_no_area(triangle: &[[f64; 3]; 3]) -> bool {
    let [a, b, c] = triangle.map(|[x, y, z]| (x, y, z));
    let (ab, ac) = (
        (b.0 - a.0, b.1 - a.1, b.2 - a.2),
        (c.0 - a.0, c.1 - a.1, c.2 - a.2),
    );
    let cross = (
        ab.1 * ac.2 - ab.2 * ac.1,
        ab.2 * ac.0 - ab.0 * ac.2,
        ab.0 * ac.1 - ab.1 * ac.0,
    );
    cross.0 * cross.0 + cross.1 * cross.1 + cross.2 * cross.2 <= 1e-24
}

impl MeshFile {
    /// How many triangles each material has, in the file's order.
    pub fn material_counts(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = self
            .materials
            .iter()
            .map(|name| (name.clone(), 0))
            .collect();
        for &material in &self.material_of {
            counts[material].1 += 1;
        }
        counts
    }
}
