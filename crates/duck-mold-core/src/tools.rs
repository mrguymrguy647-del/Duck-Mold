//! The operations behind the commands: build a file, inspect it, export
//! it.
//!
//! The `duck-mold` program only reads the command line and prints what
//! these return. Anything else that wants to drive Duck Mold (a window,
//! an MCP server for agents) can call the same functions and get the
//! same answers.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::assemble::assemble;
use crate::glb::{read_glb, write_glb};
use crate::inspect::{Inspection, MaterialUse, inspect};
use crate::issue::{Code, Issue, Report};
use crate::load::{display_path, load};
use crate::math::{Bounds, Transform, Vec3};
use crate::mesh::{MeshFile, read_obj};
use crate::model::{Face, Model, Part, PartKind};
use crate::recipe::Loc;

/// Why an operation gave nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// The command can't be carried out as given: a file that isn't
    /// there, can't be read or written, or is the wrong kind. Exit
    /// code 2.
    Usage(Box<Issue>),
    /// The recipe (or model file) has mistakes. Exit code 1.
    Invalid(Report),
}

impl Failure {
    /// The exit code the program ends with: 1 for a recipe with
    /// mistakes, 2 for a command that can't be carried out.
    pub fn exit_code(&self) -> u8 {
        match self {
            Failure::Usage(_) => 2,
            Failure::Invalid(_) => 1,
        }
    }

    /// The failure as a report: `{"ok": false, "errors": [...]}`.
    pub fn report(&self) -> Report {
        match self {
            Failure::Usage(issue) => Report::from_issues(vec![(**issue).clone()]),
            Failure::Invalid(report) => report.clone(),
        }
    }
}

/// A model that was built, with what was noticed on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// The model.
    pub model: Model,
    /// The file it came from, as it was named.
    pub source: String,
    /// Things that look like mistakes but didn't stop the build.
    pub warnings: Vec<Issue>,
    /// For a mesh file: its materials and how many triangles each has.
    pub materials: Vec<MaterialUse>,
}

/// Reads and builds a file: a recipe (`.toml`, `.json`), or a model to
/// measure (an `.obj` mesh, or a `.glb` that Duck Mold wrote).
pub fn build(path: &Path) -> Result<Built, Failure> {
    let source = display_path(path);
    let ending = path
        .extension()
        .and_then(|ending| ending.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ending.as_str() {
        "toml" | "json" => {
            let mut issues = Vec::new();
            let loaded = load(path, &mut issues).map_err(Failure::Usage)?;
            match assemble(&loaded, &mut issues) {
                Some(model) => Ok(Built {
                    model,
                    source,
                    warnings: Report::from_issues(issues).warnings,
                    materials: Vec::new(),
                }),
                None => Err(Failure::Invalid(Report::from_issues(issues))),
            }
        }
        "glb" => {
            let bytes = read_file(path)?;
            let model = read_glb(&bytes, &source)
                .map_err(|issue| Failure::Invalid(Report::from_issues(vec![*issue])))?;
            Ok(Built {
                model,
                source,
                warnings: Vec::new(),
                materials: Vec::new(),
            })
        }
        "obj" => {
            let text = String::from_utf8(read_file(path)?).map_err(|_| {
                Failure::Usage(Box::new(Issue::error(
                    Code::Io,
                    "",
                    format!("can't read \"{source}\": it isn't text"),
                )))
            })?;
            let mut issues = Vec::new();
            match read_obj(&text, &source, &mut issues) {
                Some(mesh) => Ok(Built {
                    model: model_of_mesh(&mesh, path),
                    source,
                    warnings: Report::from_issues(issues).warnings,
                    materials: mesh
                        .material_counts()
                        .into_iter()
                        .map(|(name, triangles)| MaterialUse { name, triangles })
                        .collect(),
                }),
                None => Err(Failure::Invalid(Report::from_issues(issues))),
            }
        }
        "gltf" => Err(Failure::Usage(Box::new(Issue::error(
            Code::Unsupported,
            "",
            format!(
                "\"{source}\" is a glTF text file, which Duck Mold can't read yet. It reads recipes (.toml, .json), .obj meshes and the .glb files it writes"
            ),
        )))),
        _ => Err(Failure::Usage(Box::new(Issue::error(
            Code::Io,
            "",
            format!(
                "\"{source}\" isn't something Duck Mold reads: give it a recipe (.toml or .json), an .obj mesh or a .glb file it wrote"
            ),
        )))),
    }
}

/// A mesh file as a model of one part, exactly as the file has it: not
/// moved to the ground, not coloured.
fn model_of_mesh(mesh: &MeshFile, path: &Path) -> Model {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("mesh")
        .to_owned();
    let faces: Vec<Face> = mesh
        .triangles
        .iter()
        .map(|corners| Face {
            corners: corners.map(|corner| Vec3::from(corner).tidied()).to_vec(),
            color: None,
        })
        .collect();
    let part = Part {
        name: name.clone(),
        path: name.clone(),
        loc: Loc::default(),
        kind: PartKind {
            shape: "mesh".to_owned(),
            uses: None,
            file: None,
        },
        attached_to: None,
        mirrored: false,
        place: Transform::IDENTITY,
        sockets: Vec::new(),
        bounds: Bounds::around(faces.iter().flat_map(|face| face.corners.iter().copied())),
        faces: 0..faces.len(),
        parts: Vec::new(),
    };
    Model {
        name,
        palette: Vec::new(),
        faces,
        parts: vec![part],
        sockets: Vec::new(),
    }
}

/// Reads a file given on the command line.
fn read_file(path: &Path) -> Result<Vec<u8>, Failure> {
    std::fs::read(path).map_err(|error| {
        let reason = match error.kind() {
            std::io::ErrorKind::NotFound => "there is no such file",
            std::io::ErrorKind::PermissionDenied => "permission denied",
            _ => "it can't be opened",
        };
        Failure::Usage(Box::new(Issue::error(
            Code::Io,
            "",
            format!("can't read \"{}\": {reason}", display_path(path)),
        )))
    })
}

/// `duck-mold inspect`: builds a file in memory and measures it. Writes
/// nothing.
pub fn inspect_file(path: &Path) -> Result<Inspection, Failure> {
    let built = build(path)?;
    let mut inspection = inspect(&built.model, &built.source, built.warnings);
    inspection.materials = built.materials;
    Ok(inspection)
}

/// What `duck-mold export` did: the report on the model, and the file it
/// wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Exported {
    /// The same report `inspect` gives.
    #[serde(flatten)]
    pub inspection: Inspection,
    /// The file that was written, as it was named.
    pub output: String,
    /// How big it is.
    pub bytes: usize,
}

/// `duck-mold export`: builds a recipe and writes the `.glb`. Without
/// `output`, the file goes next to the recipe, with the same name.
/// Nothing is written if the recipe has mistakes.
pub fn export_file(recipe: &Path, output: Option<&Path>) -> Result<Exported, Failure> {
    let is_recipe = recipe
        .extension()
        .and_then(|ending| ending.to_str())
        .is_some_and(|ending| matches!(ending.to_lowercase().as_str(), "toml" | "json"));
    if !is_recipe {
        return Err(Failure::Usage(Box::new(Issue::error(
            Code::Io,
            "",
            format!(
                "\"{}\" isn't a recipe: export needs a recipe, whose name ends in .toml or .json",
                display_path(recipe)
            ),
        ))));
    }
    let built = build(recipe)?;
    let output: PathBuf = match output {
        Some(output) => output.to_path_buf(),
        None => recipe.with_extension("glb"),
    };
    let bytes = write_glb(&built.model);
    write_file(&output, &bytes)?;
    Ok(Exported {
        inspection: inspect(&built.model, &built.source, built.warnings),
        output: display_path(&output),
        bytes: bytes.len(),
    })
}

/// Writes a file the program was asked to write, or says in one sentence
/// why it can't.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    std::fs::write(path, bytes).map_err(|error| {
        let reason = match error.kind() {
            std::io::ErrorKind::NotFound => "that folder doesn't exist",
            std::io::ErrorKind::PermissionDenied => "permission denied",
            _ => "it can't be written there",
        };
        Failure::Usage(Box::new(Issue::error(
            Code::Io,
            "",
            format!("can't write \"{}\": {reason}", display_path(path)),
        )))
    })
}
