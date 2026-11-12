//! Step 8: an OBJ file can be used as a static part, and inspected on
//! its own.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::{Allowed, Code, Issue, Report};
use duck_mold_core::load::load;
use duck_mold_core::mesh::{MeshFile, read_obj};
use duck_mold_core::model::Model;
use duck_mold_core::tools;

/// A box 2 wide (x), 1 tall (y), 4 deep (z), standing on y = 0, the way
/// a modelling program writes one: four-cornered faces, corners
/// counter-clockwise seen from outside, two materials.
const BOX: &str = "\
# A box.
o Box
v -1 0 -2
v 1 0 -2
v 1 1 -2
v -1 1 -2
v -1 0 2
v 1 0 2
v 1 1 2
v -1 1 2
vn 0 0 1
vt 0.5 0.5
usemtl Stone
s off
f 1 4 3 2
f 5 6 7 8
f 1 5 8 4
f 2 3 7 6
f 1 2 6 5
usemtl Moss
f 4/1/1 8/1/1 7/1/1 3/1/1
";

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

fn read(text: &str) -> (Option<MeshFile>, Vec<Issue>) {
    let mut issues = Vec::new();
    let mesh = read_obj(text, "rock.obj", &mut issues);
    (mesh, issues)
}

fn mesh(text: &str) -> MeshFile {
    let (mesh, issues) = read(text);
    assert_eq!(issues, Vec::new());
    mesh.unwrap()
}

/// Builds `recipe` in a folder that also holds `rock.obj`.
fn build(name: &str, obj: &str, recipe: &str) -> (Option<Model>, Report) {
    let folder = folder(name);
    std::fs::write(folder.join("rock.obj"), obj).unwrap();
    std::fs::write(folder.join("scene.toml"), recipe).unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("scene.toml"), &mut issues).unwrap();
    let model = assemble(&loaded, &mut issues);
    (model, Report::from_issues(issues))
}

fn only(issues: &[Issue], code: Code) -> &Issue {
    let found: Vec<&Issue> = issues.iter().filter(|issue| issue.code == code).collect();
    assert_eq!(found.len(), 1, "expected one {code} in {issues:#?}");
    found[0]
}

// ---- reading the file --------------------------------------------------------

#[test]
fn an_obj_files_faces_become_triangles_by_material() {
    let mesh = mesh(BOX);
    assert_eq!(mesh.file, "rock.obj");
    assert_eq!(mesh.triangles.len(), 12);
    assert_eq!(mesh.materials, ["Stone", "Moss"]);
    assert_eq!(
        mesh.material_counts(),
        [("Stone".to_owned(), 10), ("Moss".to_owned(), 2)]
    );
}

#[test]
fn the_mesh_faces_outwards_in_duck_molds_order() {
    // The same test as for the built-in shapes: for corners a, b, c,
    // `(b - a) x (c - a)` points away from the inside.
    let inside = [0.0, 0.5, 0.0];
    for [a, b, c] in mesh(BOX).triangles {
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let normal = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let outwards: f64 = (0..3)
            .map(|axis| normal[axis] * ((a[axis] + b[axis] + c[axis]) / 3.0 - inside[axis]))
            .sum();
        assert!(outwards > 0.0, "a triangle faces inwards");
    }
}

#[test]
fn the_files_x_points_to_the_models_left() {
    // As in glTF, and as Blender writes OBJ: the model faces +z and its
    // left side is at +x. In Duck Mold, left is -x.
    let mesh = mesh("v 3 0 0\nv 3 1 0\nv 3 0 1\nf 1 2 3\n");
    assert!(mesh.triangles[0].iter().all(|corner| corner[0] == -3.0));
    // Up and forward are unchanged.
    assert_eq!(mesh.triangles[0][0], [-3.0, 0.0, 0.0]);
    assert_eq!(mesh.triangles[0][1], [-3.0, 0.0, 1.0]);
    assert_eq!(mesh.triangles[0][2], [-3.0, 1.0, 0.0]);
}

#[test]
fn obj_files_come_in_many_spellings() {
    // Windows line endings, a point with a fourth number, corners with
    // texture and normal numbers, points counted back from the end, a
    // comment at the end of a line, and no material at all.
    let text = "v 0 0 0 1.0\r\nv 1 0 0\r\nv 1 1 0\r\nv 0 1 0   # the last corner\r\nf -4//1 -3//1 -2//1 -1//1\r\n";
    let mesh = mesh(text);
    assert_eq!(mesh.triangles.len(), 2);
    assert_eq!(mesh.materials, ["default"]);

    // A material name with spaces and dots.
    let mesh = self::mesh("v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl Rock Moss.001\nf 1 2 3\n");
    assert_eq!(mesh.materials, ["Rock Moss.001"]);

    // A five-cornered face is three triangles.
    let mesh = self::mesh("v 0 0 0\nv 2 0 0\nv 3 1 0\nv 1 2 0\nv -1 1 0\nf 1 2 3 4 5\n");
    assert_eq!(mesh.triangles.len(), 3);
}

#[test]
fn things_that_are_not_faces_are_skipped_with_a_note() {
    let (mesh, issues) = read("v 0 0 0\nv 1 0 0\nv 0 1 0\nl 1 2\np 3\ncurv 0 1 1 2\nf 1 2 3\n");
    assert_eq!(mesh.unwrap().triangles.len(), 1);
    let note = only(&issues, Code::Skipped);
    assert!(!note.is_error());
    assert_eq!(note.file, "rock.obj");
    assert!(
        note.message
            .starts_with("3 lines of this file were skipped"),
        "{}",
        note.message
    );

    // A triangle squashed flat is left out, also with a note.
    let (mesh, issues) = read("v 0 0 0\nv 1 0 0\nv 2 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 4\n");
    assert_eq!(mesh.unwrap().triangles.len(), 1);
    assert!(
        only(&issues, Code::Skipped)
            .message
            .starts_with("1 triangles of this file have no area")
    );
}

#[test]
fn mistakes_in_the_file_name_its_line() {
    let (mesh, issues) = read("v 0 0 0\nv 1 zero 0\nv 0 1 0\nf 1 2 3\nf 1 2 9\nf 1 2\nf 0 1 2\n");
    assert!(mesh.is_none());
    let found: Vec<(Option<usize>, &str)> = issues
        .iter()
        .map(|issue| (issue.line, issue.message.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (Some(2), "a point needs three numbers: v 1.0 2.0 0.5"),
            (
                Some(5),
                "this face names the point \"9\", but the file has 3 points so far (points are counted from 1)"
            ),
            (
                Some(6),
                "a face needs at least three corners; this one has 2"
            ),
            (
                Some(7),
                "this face names the point \"0\", but the file has 3 points so far (points are counted from 1)"
            ),
        ]
    );
    assert!(
        issues
            .iter()
            .all(|issue| issue.code == Code::BadMesh && issue.file == "rock.obj")
    );

    let (mesh, issues) = read("# nothing here\nv 0 0 0\n");
    assert!(mesh.is_none());
    assert!(
        only(&issues, Code::BadMesh)
            .message
            .contains("no faces to build")
    );

    // A badly damaged file doesn't fill the screen.
    let broken: String = (0..50).map(|_| "f 1 2 3\n").collect();
    let (_, issues) = read(&broken);
    assert_eq!(issues.len(), 9);
    assert_eq!(issues[8].message, "and 42 more mistakes like these");
}

// ---- using it as a part ------------------------------------------------------

const SCENE: &str = r##"
version = 0
name = "scene"

[palette]
stone = "#8C8C88"
leaf = "#3F8F4A"
red = "#D0453A"

[[parts]]
name = "ground"
shape = "box"
size = [4, 0.2, 4]
color = "leaf"

[[parts]]
name = "rock"
file = "rock.obj"
scale = 0.5
color = "stone"
colors = { Moss = "leaf" }
attach = "bottom"
to = "ground.top"

[[parts]]
name = "flag"
shape = "box"
size = [0.1, 0.5, 0.1]
color = "red"
attach = "bottom"
to = "rock.top"
"##;

#[test]
fn a_mesh_file_is_a_part_like_any_other() {
    let (model, report) = build("import_scene", BOX, SCENE);
    assert_eq!(report.errors, Vec::new());
    assert_eq!(report.warnings, Vec::new());
    let model = model.unwrap();
    let rock = &model.parts[1];
    // Scaled to half size, sitting on the ground, with a flag on top.
    let bounds = rock.bounds.unwrap();
    assert_eq!(bounds.size().tidied().to_array(), [1.0, 0.5, 2.0]);
    assert_eq!(bounds.min.y, 0.2);
    assert_eq!(model.parts[2].bounds.unwrap().min.y, 0.7);
    assert_eq!(rock.kind.shape, "mesh");
    assert!(
        rock.kind
            .file
            .as_deref()
            .unwrap()
            .ends_with("import_scene/rock.obj")
    );
    assert!(rock.kind.label().ends_with("rock.obj (mesh)"));
    // It has the seven built-in sockets.
    assert_eq!(rock.sockets.len(), 7);

    // Coloured from the palette: the material Moss is leaf, the rest
    // takes the part's colour.
    let faces = &model.faces[rock.faces.clone()];
    let count = |name: &str| {
        let at = model
            .palette
            .iter()
            .position(|color| color.name == name)
            .unwrap();
        faces.iter().filter(|face| face.color == Some(at)).count()
    };
    assert_eq!((count("stone"), count("leaf")), (10, 2));
    // The mossy face is the top.
    let moss = faces.iter().find(|face| face.color == Some(1)).unwrap();
    assert_eq!(moss.normal().unwrap().to_array(), [0.0, 1.0, 0.0]);

    // Measured like any model: nothing floats, nothing overlaps.
    let inspection = duck_mold_core::inspect::inspect(&model, "scene.toml", Vec::new());
    assert_eq!(inspection.floating, Vec::new());
    assert_eq!(inspection.overlapping, Vec::new());
    assert_eq!(inspection.triangles, 36);
    assert_eq!(inspection.parts[1].kind, "mesh");
}

#[test]
fn the_face_names_of_a_mesh_are_its_materials() {
    let (model, report) = build(
        "import_bad_material",
        BOX,
        &SCENE.replace(
            "colors = { Moss = \"leaf\" }",
            "colors = { moss = \"leaf\", top = \"leaf\" }",
        ),
    );
    assert!(model.is_none());
    let faces: Vec<&Issue> = report
        .errors
        .iter()
        .filter(|issue| issue.code == Code::UnknownFace)
        .collect();
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].path, "parts[1].colors.moss");
    assert_eq!(faces[0].suggestion.as_deref(), Some("Moss"));
    assert_eq!(
        faces[0].allowed,
        Some(Allowed::OneOf {
            options: vec!["Stone".to_owned(), "Moss".to_owned()]
        })
    );
}

#[test]
fn a_mesh_with_no_colour_is_grey_with_a_warning() {
    let (model, report) = build(
        "import_grey",
        BOX,
        "version = 0\n[[parts]]\nfile = \"rock.obj\"\n",
    );
    assert!(model.unwrap().faces.iter().all(|face| face.color.is_none()));
    assert_eq!(only(&report.warnings, Code::NoColor).path, "parts[0]");
}

#[test]
fn a_mesh_file_can_be_mirrored_turned_and_given_sockets() {
    let (model, report) = build(
        "import_placed",
        BOX,
        "version = 0\norigin = \"rock.bottom\"\n[palette]\nstone = \"#8C8C88\"\n[[parts]]\nname = \"rock\"\nfile = \"rock.obj\"\ncolor = \"stone\"\nrotate = [0, 90, 0]\nmirror = \"x\"\n[parts.sockets]\nflag = { on = \"top\", shift = [0.5, 0, 0] }\n[[parts]]\nname = \"flag\"\nshape = \"box\"\nsize = [0.1, 0.5, 0.1]\ncolor = \"stone\"\nattach = \"bottom\"\nto = \"rock.flag\"\n",
    );
    assert_eq!(report.errors, Vec::new());
    let model = model.unwrap();
    // Turned a quarter: 4 wide and 2 deep.
    assert_eq!(
        model.parts[0].bounds.unwrap().size().tidied().to_array(),
        [4.0, 1.0, 2.0]
    );
    // The flag stands on the socket, wherever the turn and flip put it.
    let flag = model.parts[1].bounds.unwrap();
    assert_eq!(flag.min.y, 1.0);
    let socket = model.parts[0]
        .sockets
        .iter()
        .find(|socket| socket.name == "flag")
        .unwrap();
    assert_eq!(flag.center().tidied().x, socket.at.x);
    assert_eq!(flag.center().tidied().z, socket.at.z);
    // Flipped once: its triangles were reversed, and still face outwards.
    assert!(model.parts[0].mirrored);
    let middle = model.parts[0].bounds.unwrap().center();
    for face in &model.faces[model.parts[0].faces.clone()] {
        assert!(face.normal().unwrap().dot(face.corners[0] - middle) > 0.0);
    }
}

#[test]
fn mistakes_about_a_mesh_file_are_reported_once_where_they_belong() {
    // The file isn't there: reported at the recipe's line.
    let folder = folder("import_missing");
    std::fs::write(
        folder.join("scene.toml"),
        "version = 0\n[[parts]]\nfile = \"models/rock.obj\"\n",
    )
    .unwrap();
    let mut issues = Vec::new();
    let loaded = load(&folder.join("scene.toml"), &mut issues).unwrap();
    assert!(assemble(&loaded, &mut issues).is_none());
    let issue = only(&issues, Code::FileNotFound);
    assert_eq!(
        (issue.path.as_str(), issue.line),
        ("parts[0].file", Some(3))
    );
    assert!(issue.message.contains("models/rock.obj"));
    assert_eq!(issues.len(), 1);

    // The file is damaged: reported in the mesh file, with its line.
    let (model, report) = build(
        "import_damaged",
        "v 0 0 0\nv 1 0 0\nf 1 2 3\n",
        "version = 0\n[[parts]]\nname = \"rock\"\nfile = \"rock.obj\"\n[[parts]]\nshape = \"box\"\nattach = \"bottom\"\nto = \"rock.top\"\n",
    );
    assert!(model.is_none());
    // One error, and nothing more about the part attached to it.
    assert_eq!(report.errors.len(), 1, "{:#?}", report.errors);
    assert_eq!(report.errors[0].code, Code::BadMesh);
    assert!(report.errors[0].file.ends_with("import_damaged/rock.obj"));
    assert_eq!(report.errors[0].line, Some(3));
}

#[test]
fn notes_about_a_mesh_file_come_through_as_warnings() {
    let (model, report) = build(
        "import_notes",
        &format!("{BOX}l 1 2\n"),
        "version = 0\n[palette]\nstone = \"#8C8C88\"\n[[parts]]\nfile = \"rock.obj\"\ncolor = \"stone\"\n",
    );
    assert!(model.is_some());
    assert!(
        only(&report.warnings, Code::Skipped)
            .file
            .ends_with("rock.obj")
    );
}

// ---- inspecting a mesh file on its own ---------------------------------------

#[test]
fn inspect_measures_an_obj_file_and_lists_its_materials() {
    let folder = folder("import_inspect");
    std::fs::write(folder.join("rock.obj"), BOX).unwrap();
    let report = tools::inspect_file(&folder.join("rock.obj")).unwrap();
    assert_eq!(report.name, "rock");
    // Exactly as the file has it: not moved, not scaled.
    assert_eq!(report.size, [2.0, 1.0, 4.0]);
    assert_eq!(report.bounds.min, [-1.0, 0.0, -2.0]);
    assert_eq!(report.triangles, 12);
    assert!(report.ground.rests_on_ground);
    let materials: Vec<(&str, usize)> = report
        .materials
        .iter()
        .map(|material| (material.name.as_str(), material.triangles))
        .collect();
    assert_eq!(materials, [("Stone", 10), ("Moss", 2)]);
    assert_eq!(report.parts.len(), 1);
    assert_eq!(
        (report.parts[0].name.as_str(), report.parts[0].kind.as_str()),
        ("rock", "mesh")
    );
    assert_eq!(report.warnings, Vec::new());

    let text = report.to_text();
    assert!(
        text.contains("  materials    2:  Stone 10, Moss 2  (triangles each)\n"),
        "{text}"
    );
    assert!(
        text.contains("  colours      0:  (no colour) 12  (triangles each)\n"),
        "{text}"
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(
        json["materials"][1],
        serde_json::json!({ "name": "Moss", "triangles": 2 })
    );

    // A damaged file is a failure with exit code 1; a missing one, 2.
    std::fs::write(folder.join("bad.obj"), "v 0 0 0\nf 1 2 3\n").unwrap();
    let failure = tools::inspect_file(&folder.join("bad.obj")).unwrap_err();
    assert_eq!(failure.exit_code(), 1);
    assert_eq!(failure.report().errors[0].code, Code::BadMesh);
    assert_eq!(
        tools::inspect_file(&folder.join("gone.obj"))
            .unwrap_err()
            .exit_code(),
        2
    );
}
