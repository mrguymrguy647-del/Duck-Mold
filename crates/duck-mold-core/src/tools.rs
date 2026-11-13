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
use crate::gltf::{is_duck_mold_glb, read_gltf};
use crate::inspect::{Inspection, MaterialUse, face_report, inspect, socket_report};
use crate::issue::{Code, Issue, Report};
use crate::load::{display_path, load};
use crate::math::{Bounds, Transform, Vec3};
use crate::mesh::{MeshFile, read_obj};
use crate::model::{Face, Model, Part, PartKind};
use crate::recipe::{Loc, PaletteColor};

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
/// measure (an `.obj` mesh, a `.gltf` or `.glb`: a `.glb` that Duck Mold
/// wrote comes back with its parts and palette).
pub fn build(path: &Path) -> Result<Built, Failure> {
    build_watching(path).0
}

/// [`build`], and every file it read, the file itself first, whether or
/// not it built: what a window watches, to build again when one changes.
pub fn build_watching(path: &Path) -> (Result<Built, Failure>, Vec<PathBuf>) {
    let mut files = vec![path.to_path_buf()];
    let built = build_noting(path, &mut files);
    (built, files)
}

fn build_noting(path: &Path, files: &mut Vec<PathBuf>) -> Result<Built, Failure> {
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
            files.clone_from(&loaded.files);
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
        "glb" if is_duck_mold_glb(&read_file(path)?) => {
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
        "glb" | "gltf" => {
            let bytes = read_file(path)?;
            let mut issues = Vec::new();
            let beside = path.parent().unwrap_or(Path::new("."));
            match read_gltf(&bytes, &source, beside, &mut issues) {
                Some(mesh) => {
                    files.extend(mesh.extra_files.iter().cloned());
                    Ok(Built {
                        model: model_of_mesh(&mesh, path),
                        source,
                        warnings: Report::from_issues(issues).warnings,
                        materials: mesh
                            .material_counts()
                            .into_iter()
                            .map(|(name, triangles)| MaterialUse { name, triangles })
                            .collect(),
                    })
                }
                None => Err(Failure::Invalid(Report::from_issues(issues))),
            }
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
        _ => Err(Failure::Usage(Box::new(Issue::error(
            Code::Io,
            "",
            format!(
                "\"{source}\" isn't something Duck Mold reads: give it a recipe (.toml or .json) or a mesh (.obj, .gltf or .glb)"
            ),
        )))),
    }
}

/// A mesh file as a model of one part, exactly as the file has it: not
/// moved to the ground, and coloured only by the colours its materials
/// have in the file (a glTF's; an OBJ's aren't read).
fn model_of_mesh(mesh: &MeshFile, path: &Path) -> Model {
    let mut palette: Vec<PaletteColor> = Vec::new();
    let color_of_material: Vec<Option<usize>> = (0..mesh.materials.len())
        .map(|material| {
            let rgb = mesh.color_of(material)?;
            let name = &mesh.materials[material];
            let at = palette
                .iter()
                .position(|color| color.name == *name)
                .unwrap_or_else(|| {
                    palette.push(PaletteColor {
                        name: name.clone(),
                        rgb,
                        loc: Loc::default(),
                        own: false,
                    });
                    palette.len() - 1
                });
            Some(at)
        })
        .collect();
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("mesh")
        .to_owned();
    let faces: Vec<Face> = mesh
        .triangles
        .iter()
        .zip(&mesh.material_of)
        .map(|(corners, &material)| Face {
            corners: corners.map(|corner| Vec3::from(corner).tidied()).to_vec(),
            color: color_of_material[material],
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
            changes: 0,
        },
        attached_to: None,
        mirrored: false,
        place: Transform::IDENTITY,
        local: Transform::IDENTITY,
        sockets: Vec::new(),
        bounds: Bounds::around(faces.iter().flat_map(|face| face.corners.iter().copied())),
        faces: 0..faces.len(),
        parts: Vec::new(),
        face_names: mesh.materials.clone(),
        face_groups: mesh
            .material_of
            .iter()
            .map(|&material| material as u16)
            .collect(),
        shaped: None,
    };
    Model {
        name,
        palette,
        faces,
        parts: vec![part],
        sockets: Vec::new(),
        zero: Vec3::ZERO,
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

/// `duck-mold inspect --sockets`: [`inspect_file`], with every named
/// socket listed too.
pub fn inspect_file_with_sockets(path: &Path) -> Result<Inspection, Failure> {
    inspect_file_listing(path, true, false)
}

/// `duck-mold inspect --sockets --faces`: [`inspect_file`], with every
/// named socket listed if `sockets`, and every face of every shaped part
/// if `faces`.
pub fn inspect_file_listing(
    path: &Path,
    sockets: bool,
    faces: bool,
) -> Result<Inspection, Failure> {
    let built = build(path)?;
    let mut inspection = inspect(&built.model, &built.source, built.warnings);
    inspection.materials = built.materials;
    if sockets {
        inspection.sockets = socket_report(&built.model);
    }
    if faces {
        inspection.shaped = face_report(&built.model);
    }
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

/// The recipe a new model starts as: one box, standing on the ground,
/// with one colour.
pub const STARTER: &str = r##"# A new Duck Mold model. Sizes are in metres.
version = 0
name = "model"

[palette]
duck = "#FFCC2E"

[[parts]]
name = "body"
shape = "box"
size = [1, 1, 1]
color = "duck"
"##;

/// An example recipe that is inside the program: the Studio's start
/// page shows them, and `duck-mold new --example` copies one out.
#[derive(Debug, Clone, Copy)]
pub struct Example {
    /// Its name: `robot`.
    pub name: &'static str,
    /// What it shows, in a few words.
    pub about: &'static str,
    /// The recipe's text.
    pub recipe: &'static str,
    /// The files the recipe uses, each with where it goes from the
    /// recipe's folder: `parts/lamp.gltf`.
    pub files: &'static [(&'static str, &'static [u8])],
}

/// The examples, in the order the start page shows them.
pub const EXAMPLES: [Example; 12] = [
    Example {
        name: "robot",
        about: "Parts made of parts, and a mirrored arm",
        recipe: include_str!("../../../examples/robot.toml"),
        files: &[],
    },
    Example {
        name: "house",
        about: "A roof of two ramps, with a door, windows and a chimney on sockets",
        recipe: include_str!("../../../examples/house.toml"),
        files: &[],
    },
    Example {
        name: "snowman",
        about: "Eyes, buttons and arms that follow round surfaces",
        recipe: include_str!("../../../examples/snowman.toml"),
        files: &[],
    },
    Example {
        name: "desk",
        about: "Parts made elsewhere: a lamp and a crate from model files",
        recipe: include_str!("../../../examples/desk.toml"),
        files: &[
            (
                "parts/lamp.gltf",
                include_bytes!("../../../examples/parts/lamp.gltf"),
            ),
            (
                "parts/crate.glb",
                include_bytes!("../../../examples/parts/crate.glb"),
            ),
        ],
    },
    Example {
        name: "duck",
        about: "A ball stretched into a body, and one wing used twice",
        recipe: include_str!("../../../examples/duck.toml"),
        files: &[],
    },
    Example {
        name: "dog",
        about: "Capsules: a body lying down and a leg used four times",
        recipe: include_str!("../../../examples/dog.toml"),
        files: &[],
    },
    Example {
        name: "doughnut",
        about: "A torus, with icing, sprinkles and a bevelled plate",
        recipe: include_str!("../../../examples/doughnut.toml"),
        files: &[],
    },
    Example {
        name: "mug",
        about: "A tube that widens with taper, and a ring for a handle",
        recipe: include_str!("../../../examples/mug.toml"),
        files: &[],
    },
    Example {
        name: "temple",
        about: "A pyramid roof on columns, with stairs up the front",
        recipe: include_str!("../../../examples/temple.toml"),
        files: &[],
    },
    Example {
        name: "barn",
        about: "A prism for a ridged roof, and a silo with a round cap",
        recipe: include_str!("../../../examples/barn.toml"),
        files: &[],
    },
    Example {
        name: "slide",
        about: "Stairs up to a platform and a ramp down the other side",
        recipe: include_str!("../../../examples/slide.toml"),
        files: &[],
    },
    Example {
        name: "gate",
        about: "An arch between two towers that are tapered and bevelled",
        recipe: include_str!("../../../examples/gate.toml"),
        files: &[],
    },
];

/// `duck-mold new --example`: writes a copy of an example to `path`,
/// with the files it uses beside it. The recipe must not be there yet;
/// a file it uses that is there already is left as it is.
pub fn new_from_example(path: &Path, example: &str) -> Result<Inspection, Failure> {
    let Some(found) = EXAMPLES.iter().find(|known| known.name == example) else {
        let names = EXAMPLES.iter().map(|known| known.name);
        return Err(Failure::Usage(Box::new(
            Issue::error(
                Code::Io,
                "",
                format!("there is no example called \"{example}\""),
            )
            .got(example)
            .one_of(example, names),
        )));
    };
    new_recipe(path, found.recipe)?;
    let folder = path.parent().unwrap_or(Path::new(""));
    for (name, bytes) in found.files {
        let file = folder.join(name);
        if file.exists() {
            continue;
        }
        if let Some(within) = file.parent() {
            std::fs::create_dir_all(within).map_err(|error| {
                Failure::Usage(Box::new(Issue::error(
                    Code::Io,
                    "",
                    format!(
                        "couldn't make the folder \"{}\": {error}",
                        display_path(within)
                    ),
                )))
            })?;
        }
        write_file(&file, bytes)?;
    }
    inspect_file(path)
}

/// `duck-mold new`: writes the starter recipe to `path`, which must end
/// in `.toml` and must not be there yet. Nothing is ever written over.
pub fn new_file(path: &Path) -> Result<Inspection, Failure> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty() && !stem.contains(['"', '\\']))
        .unwrap_or_else(|| "model".to_owned());
    let text = STARTER.replacen("\"model\"", &format!("\"{name}\""), 1);
    new_recipe(path, &text)?;
    inspect_file(path)
}

/// Writes a new recipe's text to `path`, which must end in `.toml` and
/// must not be there yet.
fn new_recipe(path: &Path, text: &str) -> Result<(), Failure> {
    let refuse = |why: String| Failure::Usage(Box::new(Issue::error(Code::Io, "", why)));
    let is_toml = path
        .extension()
        .and_then(|ending| ending.to_str())
        .is_some_and(|ending| ending.eq_ignore_ascii_case("toml"));
    if !is_toml {
        return Err(refuse(format!(
            "\"{}\" can't be a new recipe: its name must end in .toml",
            display_path(path)
        )));
    }
    if path.exists() {
        return Err(refuse(format!(
            "\"{}\" is already there: new never writes over a file",
            display_path(path)
        )));
    }
    write_file(path, text.as_bytes())
}
