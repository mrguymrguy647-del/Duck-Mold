//! Meshes read from files, to use as static parts.
//!
//! A [`MeshFile`] is what an imported file comes down to: triangles, each
//! belonging to a named material. Duck Mold treats the material names as
//! the part's face names, so `colors = { Moss = "leaf" }` paints them
//! from the palette like any other face.

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
