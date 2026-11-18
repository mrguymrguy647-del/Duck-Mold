//! Phase 7a, step 8: far-away versions (LODs).
//!
//! `lods = "auto"` gives a model two simpler versions: Far 1 with half
//! the segments and one-step bevels, Far 2 with a quarter, no bevels and
//! the small parts left out. They are written in the `.glb` with glTF's
//! `MSFT_lod`, and the reader written for Duck Forge reads them back.

use std::path::Path;

use duck_mold_core::assemble::{LEAVE_OUT_BELOW, assemble};
use duck_mold_core::glb::write_glb;
use duck_mold_core::inspect::{far_summary, far_words, inspect};
use duck_mold_core::issue::{Code, Issue};
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::recipe::Lods;
use duck_mold_core::source::Syntax;

/// The reader written for Duck Forge, exactly as it will be copied there.
#[path = "../../../docs/forge/duck_mold_glb.rs"]
#[allow(dead_code)]
mod forge;

fn build(text: &str) -> (Option<Model>, Vec<Issue>) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, issues)
}

fn built(text: &str) -> Model {
    let (model, issues) = build(text);
    let errors: Vec<_> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.unwrap()
}

fn errors(text: &str) -> Vec<Issue> {
    build(text)
        .1
        .into_iter()
        .filter(|issue| issue.is_error())
        .collect()
}

/// A 2 m tall figure: a body, a round head with a bevelled hat, two
/// small eyes (one kept), stairs to stand on, and an arm made of parts.
const FIGURE: &str = r##"
version = 0
name = "figure"
lods = "auto"

[palette]
skin = "#E0B090"
cloth = "#3050A0"
dark = "#202020"

[[parts]]
name = "body"
shape = "cylinder"
radius = 0.3
height = 1.2
segments = 24
color = "cloth"
[parts.sockets]
side = { on = "right", shift = [0, 0.6, 0] }

[[parts]]
name = "head"
shape = "sphere"
radius = 0.25
segments = 24
color = "skin"
attach = "bottom"
to = "body.top"
[parts.sockets]
left_eye = { on = "front", shift = [-0.4, 0.3, 0] }
right_eye = { on = "front", shift = [0.4, 0.3, 0] }

[[parts]]
name = "hat"
shape = "box"
size = [0.4, 0.2, 0.4]
bevel = 0.04
bevel_steps = 3
color = "dark"
attach = "bottom"
to = "head.top"

[[parts]]
name = "left_eye"
shape = "sphere"
radius = 0.03
color = "dark"
attach = "back"
to = "head.left_eye"

[[parts]]
name = "right_eye"
shape = "sphere"
radius = 0.03
color = "dark"
attach = "back"
to = "head.right_eye"
lod = "keep"

[[parts]]
name = "step"
shape = "stairs"
size = [0.6, 0.3, 0.6]
steps = 4
color = "dark"
attach = "top"
to = "body.bottom"

[[parts]]
name = "arm"
attach = "top"
to = "body.side"
[[parts.parts]]
name = "upper"
shape = "capsule"
radius = 0.06
height = 0.5
segments = 16
color = "cloth"
[[parts.parts]]
name = "hand"
shape = "sphere"
radius = 0.07
segments = 12
color = "skin"
attach = "top"
to = "upper.bottom"
"##;

fn triangles_of(model: &Model, path: &str) -> usize {
    let part = model
        .leaves()
        .into_iter()
        .find(|part| part.path == path)
        .unwrap_or_else(|| panic!("no part {path}"));
    model.faces[part.faces.clone()]
        .iter()
        .map(|face| face.corners.len() - 2)
        .sum()
}

#[test]
fn a_recipe_without_lods_has_no_far_versions() {
    let model = built(&FIGURE.replace("lods = \"auto\"\n", ""));
    assert!(model.far.is_empty());
    let model = built(&FIGURE.replace("lods = \"auto\"", "lods = \"none\""));
    assert!(model.far.is_empty());
}

#[test]
fn auto_puts_them_at_8_and_20_times_the_largest_side() {
    assert_eq!(Lods::Auto.distances(1.83), [14.6, 36.6]);
    assert_eq!(Lods::At([12.0, 30.0]).distances(1.83), [12.0, 30.0]);
    let model = built(FIGURE);
    let size = model.bounds().unwrap().size();
    let largest = size.x.max(size.y).max(size.z);
    let distances: Vec<f64> = model.far.iter().map(|far| far.distance).collect();
    assert_eq!(distances, Lods::Auto.distances(largest));
    let model = built(&FIGURE.replace("lods = \"auto\"", "lods = [12, 30]"));
    let distances: Vec<f64> = model.far.iter().map(|far| far.distance).collect();
    assert_eq!(distances, [12.0, 30.0]);
}

#[test]
fn wrong_lods_are_explained() {
    for wrong in ["\"far\"", "[30, 12]", "[0, 12]", "[12]", "true"] {
        let issues = errors(&FIGURE.replace("lods = \"auto\"", &format!("lods = {wrong}")));
        let issue = issues
            .first()
            .unwrap_or_else(|| panic!("{wrong} was taken"));
        assert_eq!(issue.code, Code::TypeMismatch, "{wrong}");
        assert_eq!(issue.path, "lods");
        assert!(issue.message.contains("\"auto\""), "{}", issue.message);
        assert_eq!(issue.line, Some(4), "{wrong}");
    }
    let issues = errors(&FIGURE.replace("lod = \"keep\"", "lod = \"always\""));
    assert_eq!(issues[0].path, "parts[4].lod");
    assert!(
        issues[0].message.contains("\"keep\""),
        "{}",
        issues[0].message
    );
}

#[test]
fn far_1_halves_the_segments_and_far_2_quarters_them() {
    let model = built(FIGURE);
    let [far_1, far_2] = [&model.far[0].model, &model.far[1].model];
    // A 24-sided cylinder: 24 sides, and two 24-cornered ends.
    assert_eq!(triangles_of(&model, "body"), 24 * 2 + 22 * 2);
    assert_eq!(triangles_of(far_1, "body"), 12 * 2 + 10 * 2);
    assert_eq!(triangles_of(far_2, "body"), 6 * 2 + 4 * 2);
    // A sphere never goes below 6 segments.
    assert!(triangles_of(far_2, "head") < triangles_of(far_1, "head"));
    assert!(triangles_of(far_1, "head") < triangles_of(&model, "head"));
    assert_eq!(
        triangles_of(far_2, "arm/hand"),
        triangles_of(far_1, "arm/hand")
    );
    // A bevel: one step in Far 1, square in Far 2.
    assert!(triangles_of(far_1, "hat") < triangles_of(&model, "hat"));
    assert_eq!(triangles_of(far_2, "hat"), 12);
    // Stairs: the same in Far 1, one sloped block in Far 2.
    assert_eq!(triangles_of(far_1, "step"), triangles_of(&model, "step"));
    assert_eq!(triangles_of(far_2, "step"), 8);
    // Each version has fewer triangles than the one before.
    let counts: Vec<usize> = model.far.iter().map(|far| far.triangles()).collect();
    assert!(model.triangle_count() > counts[0] && counts[0] > counts[1]);
}

#[test]
fn far_2_leaves_out_the_small_parts_unless_they_say_keep() {
    let model = built(FIGURE);
    assert!(model.far[0].left_out.is_empty());
    // The eyes are 6 cm across, under 4% of the figure's height.
    let size = model.bounds().unwrap().size();
    assert!(0.06 < LEAVE_OUT_BELOW * size.x.max(size.y).max(size.z));
    assert_eq!(model.far[1].left_out, ["left_eye"]);
    // `lod = "keep"` keeps the right eye whole: not even simplified.
    assert_eq!(
        triangles_of(&model.far[1].model, "right_eye"),
        triangles_of(&model, "right_eye")
    );
    assert_eq!(
        model.far[1].triangles(),
        model.far[1].model.triangle_count() - triangles_of(&model.far[1].model, "left_eye")
    );
    // As a game draws it: the eye has no faces, everything else is as
    // it was.
    let drawn = model.far[1].drawn();
    assert_eq!(drawn.triangle_count(), model.far[1].triangles());
    assert_eq!(triangles_of(&drawn, "left_eye"), 0);
    for part in drawn.leaves() {
        if part.path != "left_eye" {
            assert_eq!(
                triangles_of(&drawn, &part.path),
                triangles_of(&model.far[1].model, &part.path)
            );
        }
    }
    let arm = drawn.part("arm").unwrap();
    assert_eq!(
        arm.faces.len(),
        drawn.part("arm/upper").unwrap().faces.len() + drawn.part("arm/hand").unwrap().faces.len()
    );
}

#[test]
fn keep_on_an_assembly_keeps_everything_inside_it() {
    let model = built(&FIGURE.replace(
        "name = \"arm\"\nattach",
        "name = \"arm\"\nlod = \"keep\"\nattach",
    ));
    for far in &model.far {
        assert_eq!(
            triangles_of(&far.model, "arm/upper"),
            triangles_of(&model, "arm/upper")
        );
        assert_eq!(
            triangles_of(&far.model, "arm/hand"),
            triangles_of(&model, "arm/hand")
        );
    }
}

#[test]
fn every_version_keeps_every_parts_place_colours_joins_and_hit_shapes() {
    let model = built(&FIGURE.replace("lods = \"auto\"", "lods = \"auto\"\ncolliders = \"parts\""));
    for far in &model.far {
        let paths = |model: &Model| -> Vec<String> {
            model
                .all_parts()
                .iter()
                .map(|part| part.path.clone())
                .collect()
        };
        assert_eq!(paths(&far.model), paths(&model));
        // Only triangles change: every part is where the model has it,
        // even one joined to a simpler sphere (which would otherwise sit
        // a little further in) or to a box without its bevel.
        for (part, near) in far.model.all_parts().iter().zip(model.all_parts()) {
            assert_eq!(part.parent, near.parent, "{}", part.path);
            assert_eq!(part.pivot, near.pivot, "{}", part.path);
            assert_eq!(part.place, near.place, "{}", part.path);
            assert_eq!(part.sockets, near.sockets, "{}", part.path);
            assert_eq!(part.limits, near.limits);
        }
        assert_eq!(far.model.colliders, model.colliders);
        assert_eq!(far.model.zero, model.zero);
        // The colours each part uses.
        for (part, near) in far.model.leaves().iter().zip(model.leaves()) {
            let colors = |model: &Model, faces: &std::ops::Range<usize>| {
                let mut colors: Vec<Option<usize>> = model.faces[faces.clone()]
                    .iter()
                    .map(|face| face.color)
                    .collect();
                colors.sort();
                colors.dedup();
                colors
            };
            assert_eq!(colors(&far.model, &part.faces), colors(&model, &near.faces));
        }
        assert_eq!(far.model.palette, model.palette);
    }
}

#[test]
fn inspect_reports_them() {
    let model = built(FIGURE);
    let report = inspect(&model, "figure.toml", Vec::new());
    assert_eq!(report.far_versions.len(), 2);
    assert_eq!(report.far_versions[0].name, "Far 1");
    assert_eq!(report.far_versions[1].left_out, ["left_eye"]);
    let words = far_words(&far_summary(&model));
    assert!(words.starts_with("2  (Far 1 at "), "{words}");
    assert!(words.contains(" m: "), "{words}");
    assert!(words.ends_with("leaving out 1 small part)"), "{words}");
    assert!(report.to_text().contains("  far versions 2  (Far 1 at "));
    let without = inspect(
        &built(&FIGURE.replace("lods = \"auto\"\n", "")),
        "f",
        Vec::new(),
    );
    assert!(without.far_versions.is_empty());
    assert!(!without.to_text().contains("far versions"));
}

/// The `.glb`'s description, as JSON.
fn json_of(glb: &[u8]) -> serde_json::Value {
    let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&glb[20..20 + length]).unwrap()
}

#[test]
fn the_glb_lists_them_with_msft_lod() {
    let model = built(FIGURE);
    let json = json_of(&write_glb(&model));
    assert_eq!(json["extensionsUsed"], serde_json::json!(["MSFT_lod"]));
    assert!(json["extensionsRequired"].is_null());
    let lods = json["extras"]["duck_mold"]["lods"].as_array().unwrap();
    assert_eq!(lods.len(), 2);
    let nodes = json["nodes"].as_array().unwrap();
    // The far objects come after the tree's, and none of them is in it.
    let in_tree = 1 + model.all_parts().len();
    let children: Vec<u64> = nodes
        .iter()
        .flat_map(|node| node["children"].as_array().cloned().unwrap_or_default())
        .map(|child| child.as_u64().unwrap())
        .collect();
    assert!(children.iter().all(|child| (*child as usize) < in_tree));
    // Every part with triangles has two, named after it.
    let head = nodes.iter().find(|node| node["name"] == "head").unwrap();
    let ids = head["extensions"]["MSFT_lod"]["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 2);
    for (at, id) in ids.iter().enumerate() {
        let far = &nodes[id.as_u64().unwrap() as usize];
        assert_eq!(far["name"], format!("head (far {})", at + 1));
        assert!(far["mesh"].is_u64());
        assert!(far["translation"].is_null());
    }
    // The eye Far 2 leaves out has an object with no triangles.
    let eye = nodes
        .iter()
        .find(|node| node["name"] == "left_eye")
        .unwrap();
    let far_2 = eye["extensions"]["MSFT_lod"]["ids"][1].as_u64().unwrap();
    assert!(nodes[far_2 as usize]["mesh"].is_null());
    // An assembly has no triangles of its own, so no far objects.
    let arm = nodes.iter().find(|node| node["name"] == "arm").unwrap();
    assert!(arm["extensions"].is_null());
    // A file without them has neither.
    let json = json_of(&write_glb(&built(&FIGURE.replace("lods = \"auto\"\n", ""))));
    assert!(json["extensionsUsed"].is_null());
    assert!(json["extras"]["duck_mold"]["lods"].is_null());
}

#[test]
fn the_duck_forge_reader_reads_them_back() {
    let model = built(FIGURE);
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    assert_eq!(read.far.len(), 2);
    for (far, made) in read.far.iter().zip(&model.far) {
        assert_eq!(far.distance, made.distance as f32);
        assert_eq!(far.model.triangles.len(), made.triangles());
        assert!(far.model.far.is_empty());
        // The same joins, palette, sockets and animations as the full
        // model; only the triangles and their ranges differ.
        assert_eq!(far.model.joins.len(), read.joins.len());
        for (join, full) in far.model.joins.iter().zip(&read.joins) {
            assert_eq!(
                (&join.name, join.parent, join.pivot),
                (&full.name, full.parent, full.pivot)
            );
            assert!(join.first_vertex + join.vertices <= far.model.vertices.len());
        }
        assert_eq!(far.model.palette, read.palette);
        assert_eq!(far.model.sockets, read.sockets);
        // Hit shapes are the full model's: a game tests hits the same
        // whichever version it draws.
        assert_eq!(far.model.colliders, read.colliders);
        // The parts own every triangle, one after the other.
        let mut next = 0;
        for part in &far.model.parts {
            assert_eq!(part.first_triangle, next, "{}", part.name);
            next += part.triangles;
        }
        assert_eq!(next, far.model.triangles.len());
        let arm = far
            .model
            .assemblies
            .iter()
            .find(|a| a.name == "arm")
            .unwrap();
        let inside: usize = far
            .model
            .parts
            .iter()
            .filter(|part| part.name.starts_with("arm/"))
            .map(|part| part.triangles)
            .sum();
        assert_eq!(arm.triangles, inside);
        // A pose moves it as it moves the full model: here, at rest.
        let rest = vec![forge::GlbPlace::REST; far.model.joins.len()];
        assert_eq!(far.model.posed(&rest).len(), far.model.vertices.len());
    }
    // The eye left out has no triangles in Far 2.
    let eye = read.far[1]
        .model
        .parts
        .iter()
        .find(|p| p.name == "left_eye")
        .unwrap();
    assert_eq!(eye.triangles, 0);
    // The far triangles sit where the far model has them.
    let head = read.far[0]
        .model
        .parts
        .iter()
        .find(|p| p.name == "head")
        .unwrap();
    let made = &model.far[0].model;
    let part = made
        .leaves()
        .into_iter()
        .find(|p| p.path == "head")
        .unwrap();
    let lowest_made = made.faces[part.faces.clone()]
        .iter()
        .flat_map(|face| face.corners.iter().map(|corner| corner.y))
        .fold(f64::INFINITY, f64::min);
    let triangles =
        &read.far[0].model.triangles[head.first_triangle..head.first_triangle + head.triangles];
    let lowest_read = triangles
        .iter()
        .flat_map(|triangle| {
            triangle
                .corners
                .map(|corner| read.far[0].model.vertices[corner][1])
        })
        .fold(f32::INFINITY, f32::min);
    assert!((f64::from(lowest_read) - lowest_made).abs() < 1e-5);
}

#[test]
fn a_file_without_them_reads_with_none() {
    let model = built(&FIGURE.replace("lods = \"auto\"\n", ""));
    let read = forge::read_duck_mold_glb(&write_glb(&model)).unwrap();
    assert!(read.far.is_empty());
}

#[test]
fn the_reader_refuses_broken_far_versions_with_a_sentence() {
    let glb = write_glb(&built(FIGURE));
    let length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json = std::str::from_utf8(&glb[20..20 + length]).unwrap();
    let with = |json: String| {
        let mut bytes = json.into_bytes();
        while !bytes.len().is_multiple_of(4) {
            bytes.push(b' ');
        }
        let bin = &glb[20 + length..];
        let mut out = Vec::new();
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend(((12 + 8 + bytes.len() + bin.len()) as u32).to_le_bytes());
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(bytes);
        out.extend(bin);
        out
    };
    let lods = json
        .find("\"lods\":[")
        .map(|at| &json[at..at + json[at..].find(']').unwrap() + 1])
        .unwrap();
    let error =
        forge::read_duck_mold_glb(&with(json.replace(lods, "\"lods\":[30,12]"))).unwrap_err();
    assert!(error.contains("further than the last"), "{error}");
    // A far object that is one of the tree's own.
    let error = forge::read_duck_mold_glb(&with(json.replacen(
        "\"MSFT_lod\":{\"ids\":[",
        "\"MSFT_lod\":{\"ids\":[1,",
        1,
    )))
    .unwrap_err();
    assert!(
        error.contains("has no object for far-away version 1"),
        "{error}"
    );
}
