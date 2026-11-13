//! Phase 4, step 1: shaping a box with `changes`. Pulls and insets are
//! read from the recipe, made in order, named with words, and checked:
//! every shape that is built is closed, faces out everywhere, holds room
//! and doesn't pass through itself; anything else is a mistake that
//! names the change.

use std::collections::HashMap;
use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::edit::{Applied, Edit, apply};
use duck_mold_core::inspect::face_report;
use duck_mold_core::issue::{Code, Issue, Report};
use duck_mold_core::load::load_text;
use duck_mold_core::math::Vec3;
use duck_mold_core::model::{Model, Part};
use duck_mold_core::shaping::{Change, Shaped, shape_box};
use duck_mold_core::source::Syntax;

fn build(text: &str) -> (Option<Model>, Report) {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    (model, Report::from_issues(issues))
}

/// Builds a recipe that must have no errors. (The palette's colours
/// aren't all used everywhere: those warnings are fine.)
fn built(text: &str) -> Model {
    let (model, report) = build(text);
    assert_eq!(report.errors, Vec::new());
    assert!(
        report
            .warnings
            .iter()
            .all(|warning| warning.code == Code::Unused),
        "{:#?}",
        report.warnings
    );
    model.unwrap()
}

fn errors(text: &str) -> Vec<Issue> {
    let (model, report) = build(text);
    assert!(model.is_none(), "this should not build:\n{text}");
    report.errors
}

fn one_error(text: &str) -> Issue {
    let errors = errors(text);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    errors.into_iter().next().unwrap()
}

const PALETTE: &str =
    "version = 0\n\n[palette]\nplaster = \"#E8E0D0\"\nwood = \"#8B5A2B\"\nsky = \"#9FD3F0\"\n\n";

/// A recipe with one part, `wall`, with these extra lines.
fn wall(lines: &str) -> String {
    format!(
        "{PALETTE}[[parts]]\nname = \"wall\"\nshape = \"box\"\nsize = [3, 2.4, 0.2]\ncolor = \"plaster\"\n{lines}\n"
    )
}

/// The design's wall, with a framed window, and its colours.
const WINDOW: &str = r#"changes = [
  { inset = "front", by = 0.6, name = "window" },
  { inset = "window", by = 0.06, name = "glass" },
  { pull = "glass", by = -0.08 },
]
colors = { window = "wood", glass = "sky", glass_side = "wood" }"#;

fn first_part(model: &Model) -> &Part {
    &model.parts[0]
}

fn triangles_by_colour(model: &Model) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = model
        .palette
        .iter()
        .map(|color| (color.name.clone(), 0))
        .collect();
    for face in &model.faces {
        counts[face.color.unwrap()].1 += face.triangle_count();
    }
    counts.retain(|(_, count)| *count > 0);
    counts
}

// ---- reading and building -------------------------------------------------

#[test]
fn the_designs_wall_is_built_with_a_framed_window() {
    let model = built(&wall(WINDOW));
    let part = first_part(&model);
    assert_eq!(part.kind.shape, "box");
    assert_eq!(part.kind.changes, 3);
    assert_eq!(part.kind.label(), "box, 3 changes");
    // 6 box faces, 4 + 4 ring pieces, 4 walls: 18 four-cornered faces.
    assert_eq!(model.triangle_count(), 36);
    assert_eq!(
        triangles_by_colour(&model),
        vec![
            ("plaster".to_owned(), 18),
            ("wood".to_owned(), 16),
            ("sky".to_owned(), 2)
        ]
    );
    // The glass is pushed 8 cm into the 20 cm wall.
    let glass = part.faces_named("glass");
    assert_eq!(glass.len(), 1);
    let glass = &model.faces[glass[0]];
    let z = glass.corners[0].z - model.zero.z;
    assert!(
        glass
            .corners
            .iter()
            .all(|corner| (corner.z - glass.corners[0].z).abs() < 1e-9)
    );
    assert!((z - 0.02).abs() < 1e-9, "{z}");
    // A shaped part keeps what it is made of, by name.
    let shaped = part.shaped.as_ref().unwrap();
    assert_eq!(shaped.faces.len(), 18);
    assert_eq!(shaped.faces_named("front").len(), 4);
}

#[test]
fn a_box_with_no_changes_is_built_as_before() {
    let plain = built(&wall(""));
    let empty = built(&wall("changes = []"));
    assert_eq!(plain.faces, empty.faces);
    assert_eq!(first_part(&empty).kind.changes, 0);
    assert!(first_part(&empty).shaped.is_none());
}

#[test]
fn a_name_for_fewer_faces_beats_one_for_more() {
    for colors in [
        "colors = { sides = \"wood\", front = \"sky\" }",
        "colors = { front = \"sky\", sides = \"wood\" }",
    ] {
        let model = built(&wall(&format!(
            "changes = [{{ inset = \"front\", by = 0.5 }}]\n{colors}"
        )));
        // The front's ring is sky; the inset in the middle is one of the
        // sides (wood), and so are the three other sides.
        assert_eq!(
            triangles_by_colour(&model),
            vec![
                ("plaster".to_owned(), 4),
                ("wood".to_owned(), 6 + 2),
                ("sky".to_owned(), 8)
            ],
            "{colors}"
        );
    }
}

#[test]
fn a_defined_part_is_shaped_once_and_used_twice() {
    let text = format!(
        "{PALETTE}[define.post]\nshape = \"box\"\nsize = [0.2, 1, 0.2]\ncolor = \"wood\"\nchanges = [{{ pull = \"top\", by = 0.3, name = \"tip\" }}]\n\n[[parts]]\nname = \"a\"\nuse = \"post\"\n\n[[parts]]\nname = \"b\"\nuse = \"post\"\nmove = [1, 0.65, 0]\n"
    );
    let model = built(&text);
    for part in &model.parts {
        assert_eq!(part.kind.label(), "post (box, 1 change)");
        let bounds = part.bounds.unwrap();
        assert!((bounds.size().y - 1.3).abs() < 1e-9);
    }
}

#[test]
fn mistakes_in_a_change_say_what_and_where() {
    let cases: [(&str, Code, &str, &str); 9] = [
        (
            "changes = { pull = \"top\", by = 1 }",
            Code::TypeMismatch,
            "parts[0].changes",
            "this needs a list of changes, like [{ pull = \"top\", by = 0.5 }], not a table",
        ),
        (
            "changes = [{ by = 1 }]",
            Code::MissingField,
            "parts[0].changes[0]",
            "a change needs one of \"pull\", \"inset\", \"move\", \"turn\", \"scale\" or \"cut\" to say what it does, like { pull = \"top\", by = 0.5 }",
        ),
        (
            "changes = [{ pull = \"top\", inset = \"top\", by = 1 }]",
            Code::Conflict,
            "parts[0].changes[0].inset",
            "a change does one thing: this one is a \"pull\" already, so make the \"inset\" a change of its own",
        ),
        (
            "changes = [{ pull = \"top\" }]",
            Code::MissingField,
            "parts[0].changes[0]",
            "a pull needs \"by\": how far, in metres, like { pull = \"top\", by = 0.5 }",
        ),
        (
            "changes = [{ pull = \"top\", by = 0 }]",
            Code::OutOfRange,
            "parts[0].changes[0].by",
            "a pull of 0 changes nothing: pull out with a number above 0, or in with one below 0",
        ),
        (
            "changes = [{ inset = \"top\", by = -0.1 }]",
            Code::OutOfRange,
            "parts[0].changes[0].by",
            "an inset is measured in from the edges: it needs a number above 0",
        ),
        (
            "changes = [{ pull = \"top+front\", by = 1 }]",
            Code::TypeMismatch,
            "parts[0].changes[0].pull",
            "\"top+front\" is an edge: a pull is made to faces, like \"top\"",
        ),
        (
            "changes = [{ pull = [], by = 1 }]",
            Code::TypeMismatch,
            "parts[0].changes[0].pull",
            "this needs a face's name, like \"top\", or a list of them, like [\"top\", \"front\"], not an empty list",
        ),
        (
            "changes = [{ pull = \"top\", by = 1, nam = \"x\" }]",
            Code::UnknownField,
            "parts[0].changes[0].nam",
            "there is no field \"nam\" here",
        ),
    ];
    for (lines, code, path, message) in cases {
        let error = one_error(&wall(lines));
        assert_eq!(
            (error.code, error.path.as_str(), error.message.as_str()),
            (code, path, message),
            "{lines}"
        );
    }
    let error = one_error(&wall("changes = [{ pull = \"top\", by = 1, nam = \"x\" }]"));
    assert_eq!(error.suggestion.as_deref(), Some("name"));
}

#[test]
fn only_a_box_without_a_bevel_can_be_shaped() {
    let text = format!(
        "{PALETTE}[[parts]]\nname = \"post\"\nshape = \"cylinder\"\ncolor = \"wood\"\nchanges = [{{ pull = \"top\", by = 1 }}]\n"
    );
    let error = one_error(&text);
    assert_eq!(error.code, Code::Unsupported);
    assert_eq!(
        error.message,
        "only a box can be shaped in this version: \"post\" is a cylinder"
    );
    let text = format!(
        "{PALETTE}[[parts]]\nshape = \"arch\"\ncolor = \"wood\"\nchanges = [{{ pull = \"top\", by = 1 }}]\n"
    );
    assert_eq!(
        one_error(&text).message,
        "only a box can be shaped in this version: this part is an arch"
    );
    let error = one_error(&wall(
        "bevel = 0.02\nchanges = [{ pull = \"top\", by = 1 }]",
    ));
    assert_eq!(error.code, Code::Conflict);
    assert_eq!(error.path, "parts[0].changes");
    let text = format!(
        "{PALETTE}[define.post]\nshape = \"box\"\ncolor = \"wood\"\n\n[[parts]]\nuse = \"post\"\nchanges = [{{ pull = \"top\", by = 1 }}]\n"
    );
    assert_eq!(
        one_error(&text).message,
        "a part made from a definition is shaped in the definition: move \"changes\" to [define.post]"
    );
}

#[test]
fn a_change_that_breaks_the_shape_is_named_with_its_line() {
    // The design's messages.
    let cases = [
        (
            "changes = [\n  { inset = \"front\", by = 0.6, name = \"window\" },\n  { inset = \"window\", by = 0.06, name = \"glass\" },\n  { pull = \"glas\", by = -0.08 },\n]",
            Code::UnknownFace,
            "parts[0].changes[2]",
            "there is no face \"glas\" on \"wall\"",
            Some("glass"),
        ),
        (
            "changes = [\n  { inset = \"front\", by = 0.6, name = \"window\" },\n  { inset = \"window\", by = 0.8 },\n]",
            Code::OutOfRange,
            "parts[0].changes[1]",
            "an inset of 0.8 m is too wide for \"window\": it is 1.2 m across, so the inset must be less than 0.6 m",
            None,
        ),
        (
            "changes = [\n  { inset = \"front\", by = 0.6, name = \"glass\" },\n  { pull = \"glass\", by = -0.3 },\n]",
            Code::BadShape,
            "parts[0].changes[1]",
            "pulling \"glass\" in by 0.3 makes \"glass_side_bottom\" pass through \"back\"",
            None,
        ),
        (
            "changes = [\n  { inset = \"front\", by = 0.6, name = \"glass\" },\n  { pull = \"glass\", by = -0.2 },\n]",
            Code::BadShape,
            "parts[0].changes[1]",
            "pulling \"glass\" in by 0.2 makes \"glass\" pass through \"back\"",
            None,
        ),
    ];
    for (lines, code, path, message, suggestion) in cases {
        let error = one_error(&wall(lines));
        assert_eq!(
            (error.code, error.path.as_str(), error.message.as_str()),
            (code, path, message)
        );
        assert_eq!(error.suggestion.as_deref(), suggestion);
        // The line of the change itself.
        let line = error.line.unwrap();
        let text = wall(lines);
        let written = text.lines().nth(line - 1).unwrap();
        let number: usize = path
            .rsplit('[')
            .next()
            .unwrap()
            .trim_end_matches(']')
            .parse()
            .unwrap();
        let lines: Vec<&str> = lines.lines().skip(1).collect();
        assert_eq!(written, lines[number], "{message}");
    }
}

#[test]
fn colours_name_faces_the_changes_made() {
    let error = one_error(&wall(
        "changes = [{ inset = \"front\", by = 0.5, name = \"window\" }]\ncolors = { windw = \"sky\" }",
    ));
    assert_eq!(error.code, Code::UnknownFace);
    assert_eq!(error.suggestion.as_deref(), Some("window"));
}

// ---- names ------------------------------------------------------------------

fn pull(faces: &[&str], by: f64, name: Option<&str>) -> Change {
    Change::Pull {
        faces: faces.iter().map(|face| (*face).to_owned()).collect(),
        by,
        name: name.map(str::to_owned),
    }
}

fn inset(faces: &[&str], by: f64, name: Option<&str>) -> Change {
    Change::Inset {
        faces: faces.iter().map(|face| (*face).to_owned()).collect(),
        by,
        name: name.map(str::to_owned),
    }
}

fn own_names(shaped: &Shaped) -> Vec<&str> {
    shaped.faces.iter().map(|face| face.name.as_str()).collect()
}

#[test]
fn each_change_names_its_faces_as_the_design_says() {
    // A pull: the face keeps its name; walls by side.
    let shaped = shape_box([1.0; 3], [1.0; 2], &[pull(&["top"], 0.5, None)], None).unwrap();
    assert_eq!(
        own_names(&shaped),
        [
            "top",
            "bottom",
            "left",
            "right",
            "front",
            "back",
            "top_side_left",
            "top_side_front",
            "top_side_right",
            "top_side_back"
        ]
    );
    // With a name.
    let shaped = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[pull(&["front"], 0.5, Some("nose"))],
        None,
    )
    .unwrap();
    assert_eq!(
        own_names(&shaped)[6..],
        ["nose_bottom", "nose_right", "nose_top", "nose_left"]
    );
    assert_eq!(shaped.faces_named("nose").len(), 4);
    // An inset: the middle takes the face's place; the old name is the
    // ring.
    let shaped = shape_box([1.0; 3], [1.0; 2], &[inset(&["front"], 0.1, None)], None).unwrap();
    assert_eq!(
        own_names(&shaped),
        [
            "top",
            "bottom",
            "left",
            "right",
            "front_inset",
            "back",
            "front_inset_rim_bottom",
            "front_inset_rim_right",
            "front_inset_rim_top",
            "front_inset_rim_left"
        ]
    );
    assert_eq!(shaped.faces_named("front").len(), 4);
    // `sides` still stands for the four sides, the inset among them.
    assert_eq!(shaped.faces_named("sides").len(), 3 + 1 + 4);
}

#[test]
fn pulled_walls_along_one_side_share_a_name() {
    let shaped = shape_box(
        [3.0, 1.0, 1.0],
        [1.0; 2],
        &[
            inset(&["top"], 0.1, Some("seat")),
            pull(&["seat"], 0.5, Some("back")),
        ],
        None,
    )
    .unwrap();
    assert_eq!(shaped.faces_named("back_front").len(), 1);
    assert_eq!(
        shaped.faces_named("back").len(),
        5,
        "the box's back, and 4 walls"
    );
}

/// Names after the same changes on many boxes, and with other amounts.
#[test]
fn names_last_through_sizes_and_amounts() {
    let changes = |by: f64| {
        vec![
            inset(&["front"], 0.05, Some("window")),
            pull(&["window"], -by / 4.0, None),
            pull(&["top"], by, Some("chimney")),
            inset(&["chimney_front", "chimney_back"], 0.02, None),
            pull(&["left", "back"], by / 2.0, None),
        ]
    };
    let mut first: Option<Vec<String>> = None;
    for size in [[1.0, 1.0, 1.0], [2.5, 0.6, 1.5], [0.4, 3.0, 0.8]] {
        for by in [0.1, 0.25, 0.4] {
            let shaped = shape_box(size, [1.0; 2], &changes(by), None).unwrap();
            let names = shaped.names();
            match &first {
                None => first = Some(names),
                Some(first) => assert_eq!(first, &names, "{size:?}, {by}"),
            }
        }
    }
}

// ---- every shape stays whole ------------------------------------------------

/// A small random number maker, the same every run.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }

    fn between(&mut self, low: f64, high: f64) -> f64 {
        let share = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        low + (high - low) * share
    }
}

/// Checks a built shape without trusting the checks it was built with:
/// every edge of every polygon is met by exactly one edge going the
/// other way (closed, and facing one way everywhere), it holds room (so
/// that way is out), and no edge pierces a polygon it isn't next to.
fn whole(shaped: &Shaped) -> Result<(), String> {
    let out = shaped.polygons();
    let key = |p: Vec3| (p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
    let mut edges: HashMap<_, usize> = HashMap::new();
    for polygon in &out.polygons {
        if polygon.normal().is_none() {
            return Err("a face with no room".into());
        }
        let count = polygon.corners.len();
        for at in 0..count {
            let edge = (
                key(polygon.corners[at]),
                key(polygon.corners[(at + 1) % count]),
            );
            *edges.entry(edge).or_default() += 1;
        }
    }
    for (&(a, b), &count) in &edges {
        if count != 1 || edges.get(&(b, a)) != Some(&1) {
            return Err("not closed".into());
        }
    }
    let mut volume = 0.0;
    let mut triangles: Vec<[Vec3; 3]> = Vec::new();
    for polygon in &out.polygons {
        for [a, b, c] in polygon.triangles() {
            volume += a.dot(b.cross(c)) / 6.0;
            triangles.push([a, b, c]);
        }
    }
    if volume <= 0.0 {
        return Err(format!("holds {volume}"));
    }
    // An edge of one triangle going right through another, where the
    // two have no corner in the same place.
    let near = 1e-7;
    for one in &triangles {
        for two in &triangles {
            if one
                .iter()
                .any(|p| two.iter().any(|q| (*p - *q).length() < near))
            {
                continue;
            }
            let normal = (two[1] - two[0]).cross(two[2] - two[0]);
            let Some(normal) = normal.normalized() else {
                continue;
            };
            for at in 0..3 {
                let (p, q) = (one[at], one[(at + 1) % 3]);
                let (dp, dq) = (normal.dot(p - two[0]), normal.dot(q - two[0]));
                if dp * dq >= 0.0 || dp.abs() < near || dq.abs() < near {
                    continue;
                }
                let hit = p + (q - p) * (dp / (dp - dq));
                let inside = (0..3).all(|side| {
                    let (a, b) = (two[side], two[(side + 1) % 3]);
                    (b - a).cross(hit - a).dot(normal) > near
                });
                if inside {
                    return Err(format!("{one:?} goes through {two:?}"));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn every_list_of_changes_gives_a_whole_shape_or_names_the_change_that_broke_it() {
    // The same lists every run; DUCK_MOLD_SEED=<number> tries others.
    let seed = std::env::var("DUCK_MOLD_SEED")
        .ok()
        .and_then(|seed| seed.parse::<u64>().ok())
        .map_or(0x9E37_79B9_7F4A_7C15, |seed| seed | 1);
    let mut dice = Dice(seed);
    let (mut made, mut refused) = (0, 0);
    for _ in 0..2000 {
        let size = [
            dice.between(0.2, 3.0),
            dice.between(0.2, 3.0),
            dice.between(0.2, 3.0),
        ];
        let taper = if dice.below(4) == 0 {
            [dice.between(0.3, 1.5), dice.between(0.3, 1.5)]
        } else {
            [1.0; 2]
        };
        let mut changes: Vec<Change> = Vec::new();
        let mut shaped = shape_box(size, taper, &[], None).unwrap();
        for _ in 0..6 {
            let names = shaped.names();
            let mut faces = vec![names[dice.below(names.len())].clone()];
            if dice.below(3) == 0 {
                faces.push(names[dice.below(names.len())].clone());
            }
            let small = size[0].min(size[1]).min(size[2]);
            let kind = dice.below(8);
            let change = if kind == 7 {
                // A corner of the corners there are.
                let corner = dice.below(shaped.corners.len());
                let mut by = [0.0; 3];
                by[dice.below(3)] = dice.between(-small, small) / 2.0;
                Change::Move {
                    faces: vec![shaped.corner_name(corner)],
                    by,
                }
            } else if kind == 5 {
                let sides = [
                    Side::Top,
                    Side::Bottom,
                    Side::Left,
                    Side::Right,
                    Side::Front,
                    Side::Back,
                ];
                Change::Cut {
                    toward: sides[dice.below(6)],
                    at: dice.between(0.0, 0.95),
                    name: None,
                }
            } else if kind == 6 {
                // An edge of the faces there are, if two of them meet.
                let edges = shaped.edges();
                let edge = edges[dice.below(edges.len())];
                let mut by = [0.0; 3];
                by[dice.below(3)] = dice.between(-small, small) / 2.0;
                Change::Move {
                    faces: vec![shaped.edge_name(&edge)],
                    by,
                }
            } else if kind == 2 {
                let mut by = [0.0; 3];
                by[dice.below(3)] = dice.between(-small, small);
                Change::Move { faces, by }
            } else if kind == 3 {
                let mut by = [0.0; 3];
                by[dice.below(3)] = dice.between(-60.0, 60.0);
                Change::Turn { faces, by }
            } else if kind == 4 {
                let mut by = [1.0; 3];
                by[dice.below(3)] = dice.between(0.3, 1.8);
                Change::Scale { faces, by }
            } else if kind == 0 {
                let mut by = dice.between(-small, small);
                if by.abs() < 0.01 {
                    by = 0.05;
                }
                Change::Pull {
                    faces,
                    by,
                    name: None,
                }
            } else {
                Change::Inset {
                    faces,
                    by: dice.between(0.001, small / 2.0),
                    name: None,
                }
            };
            changes.push(change);
            match shape_box(size, taper, &changes, None) {
                Ok(next) => {
                    if let Err(why) = whole(&next) {
                        panic!("{size:?} {taper:?} {changes:#?}: {why}");
                    }
                    shaped = next;
                    made += 1;
                }
                Err(mistake) => {
                    assert_eq!(mistake.change, changes.len() - 1, "{mistake:?}");
                    assert!(
                        matches!(mistake.code, Code::BadShape | Code::OutOfRange),
                        "{mistake:?}"
                    );
                    changes.pop();
                    refused += 1;
                }
            }
        }
    }
    // Both happen, plenty.
    assert!(
        made > 5000 && refused > 800,
        "{made} made, {refused} refused"
    );
}

// ---- inspect --faces ----------------------------------------------------------

#[test]
fn inspect_lists_each_face_its_way_its_size_and_its_neighbours() {
    let model = built(&wall(WINDOW));
    let report = face_report(&model);
    assert_eq!(report.len(), 1);
    let wall = &report[0];
    assert_eq!(
        (wall.part.as_str(), wall.changes, wall.face_count),
        ("wall", 3, 18)
    );
    let glass = wall.faces.iter().find(|face| face.name == "glass").unwrap();
    assert_eq!(glass.facing, "front");
    assert_eq!(glass.size, Some([1.68, 1.08]));
    assert_eq!(
        glass.next_to,
        [
            "glass_side_bottom",
            "glass_side_right",
            "glass_side_top",
            "glass_side_left"
        ]
    );
    // A wall of the hollow faces into it.
    let floor = wall
        .faces
        .iter()
        .find(|face| face.name == "glass_side_bottom")
        .unwrap();
    assert_eq!(floor.facing, "up");
    let front = wall.names.iter().find(|name| name.name == "front").unwrap();
    assert_eq!(
        front.faces,
        [
            "window_rim_bottom",
            "window_rim_right",
            "window_rim_top",
            "window_rim_left"
        ]
    );
}

// ---- the edits -----------------------------------------------------------------

fn done(text: &str, syntax: Syntax, edit: &Edit) -> Applied {
    let path = match syntax {
        Syntax::Toml => Path::new("test.toml"),
        Syntax::Json => Path::new("test.json"),
    };
    apply(text, syntax, path, &path.to_string_lossy(), edit)
        .unwrap_or_else(|failure| panic!("{edit:?} was refused: {:#?}", failure.report()))
}

fn edit_pull(part: &str, faces: &[&str], by: f64, name: Option<&str>) -> Edit {
    Edit::Pull {
        part: part.to_owned(),
        faces: faces.iter().map(|face| (*face).to_owned()).collect(),
        by,
        name: name.map(str::to_owned),
    }
}

fn edit_inset(part: &str, faces: &[&str], by: f64, name: Option<&str>) -> Edit {
    Edit::Inset {
        part: part.to_owned(),
        faces: faces.iter().map(|face| (*face).to_owned()).collect(),
        by,
        name: name.map(str::to_owned),
    }
}

#[test]
fn edits_write_the_designs_wall_line_by_line() {
    let start = format!("{}# the end\n", wall("colors = { window = \"wood\" }"));
    let one = done(
        &start,
        Syntax::Toml,
        &edit_inset("wall", &["front"], 0.6, Some("window")),
    );
    assert_eq!(
        one.summary,
        "inset \"front\" of \"wall\" by 0.6, naming the new faces \"window\""
    );
    assert_eq!(
        one.text,
        start.replace(
            "color = \"plaster\"\n",
            "color = \"plaster\"\nchanges = [\n  { inset = \"front\", by = 0.6, name = \"window\" },\n]\n"
        )
    );
    let two = done(
        &one.text,
        Syntax::Toml,
        &edit_inset("wall", &["window"], 0.06, Some("glass")),
    );
    let three = done(
        &two.text,
        Syntax::Toml,
        &edit_pull("wall", &["glass"], -0.08, None),
    );
    assert_eq!(three.summary, "pulled \"glass\" of \"wall\" in by 0.08");
    assert_eq!(
        three.text,
        start.replace(
            "color = \"plaster\"\n",
            "color = \"plaster\"\nchanges = [\n  { inset = \"front\", by = 0.6, name = \"window\" },\n  { inset = \"window\", by = 0.06, name = \"glass\" },\n  { pull = \"glass\", by = -0.08 },\n]\n"
        )
    );
    assert_eq!(three.lines, Some([16, 16]));
    let model = three.model.unwrap();
    assert_eq!(model.triangle_count(), 36);
}

#[test]
fn a_list_is_added_to_the_way_it_is_written() {
    let lump = "{ pull = [\"top\", \"front\"], by = 0.1, name = \"lump\" }";
    let cases = [
        // On one line, it stays on one line.
        (
            "changes = [{ pull = \"top\", by = 0.5 }]".to_owned(),
            format!("changes = [{{ pull = \"top\", by = 0.5 }}, {lump}]"),
        ),
        // Empty, it becomes a list of lines.
        (
            "changes = []".to_owned(),
            format!("changes = [\n  {lump},\n]"),
        ),
        // With no comma after the last item, one goes there (before its
        // comment), and none after the new one.
        (
            "changes = [\n    { inset = \"top\", by = 0.05 }  # a rim\n]".to_owned(),
            format!("changes = [\n    {{ inset = \"top\", by = 0.05 }},  # a rim\n    {lump}\n]"),
        ),
        // With one, the new item gets one too.
        (
            "changes = [\n    { inset = \"top\", by = 0.05 },  # a rim\n]".to_owned(),
            format!("changes = [\n    {{ inset = \"top\", by = 0.05 }},  # a rim\n    {lump},\n]"),
        ),
    ];
    for (before, after) in cases {
        let result = done(
            &wall(&before),
            Syntax::Toml,
            &edit_pull("wall", &["top", "front"], 0.1, Some("lump")),
        );
        assert_eq!(result.text, wall(&after), "{before}");
    }
}

#[test]
fn json_recipes_get_the_change_on_a_line_of_its_own() {
    let text = r##"{
  "version": 0,
  "palette": { "wood": "#8B5A2B" },
  "parts": [
    {
      "name": "crate",
      "shape": "box",
      "color": "wood"
    }
  ]
}
"##;
    let one = done(
        text,
        Syntax::Json,
        &edit_pull("crate", &["top"], 0.25, None),
    );
    assert_eq!(
        one.text,
        text.replace(
            "      \"color\": \"wood\"\n",
            "      \"color\": \"wood\",\n      \"changes\": [\n        { \"pull\": \"top\", \"by\": 0.25 }\n      ]\n"
        )
    );
    let two = done(
        &one.text,
        Syntax::Json,
        &edit_inset("crate", &["top"], 0.1, Some("lid")),
    );
    assert_eq!(
        two.text,
        text.replace(
            "      \"color\": \"wood\"\n",
            "      \"color\": \"wood\",\n      \"changes\": [\n        { \"pull\": \"top\", \"by\": 0.25 },\n        { \"inset\": \"top\", \"by\": 0.1, \"name\": \"lid\" }\n      ]\n"
        )
    );
    assert_eq!(first_part(two.model.as_ref().unwrap()).kind.changes, 2);
}

#[test]
fn a_part_made_from_a_definition_is_shaped_there() {
    let text = format!(
        "{PALETTE}[define.post]\nshape = \"box\"\nsize = [0.2, 1, 0.2]\ncolor = \"wood\"\n\n[[parts]]\nname = \"a\"\nuse = \"post\"\n\n[[parts]]\nname = \"b\"\nuse = \"post\"\nmove = [1, 0.5, 0]\n"
    );
    let result = done(&text, Syntax::Toml, &edit_pull("b", &["top"], 0.3, None));
    assert_eq!(
        result.summary,
        "pulled \"top\" of \"b\" out by 0.3 (in the definition \"post\")"
    );
    assert!(result.text.contains(
        "color = \"wood\"\nchanges = [\n  { pull = \"top\", by = 0.3 },\n]\n\n[[parts]]\nname = \"a\""
    ));
    let shared = result.shared.unwrap();
    assert_eq!(shared.definition, "post");
    assert_eq!(shared.also_changes, ["a"]);
}

#[test]
fn an_edit_that_breaks_the_shape_writes_nothing() {
    let text = wall("");
    for edit in [
        edit_pull("wall", &["glass"], 0.1, None),
        edit_inset("wall", &["front"], 2.0, None),
        edit_pull("wall", &["front"], -0.5, None),
        edit_pull("wall", &["top+front"], 0.5, None),
    ] {
        let failure = apply(&text, Syntax::Toml, Path::new("t.toml"), "t.toml", &edit);
        assert!(failure.is_err(), "{edit:?}");
    }
    let cylinder =
        format!("{PALETTE}[[parts]]\nname = \"post\"\nshape = \"cylinder\"\ncolor = \"wood\"\n");
    let failure = apply(
        &cylinder,
        Syntax::Toml,
        Path::new("t.toml"),
        "t.toml",
        &edit_pull("post", &["top"], 0.5, None),
    )
    .unwrap_err();
    assert_eq!(
        failure.report().errors[0].message,
        "only a box can be shaped in this version: \"post\" is a cylinder"
    );
}

#[test]
fn a_change_is_set_and_taken_away_by_its_number() {
    let text = wall(WINDOW);
    // One number changes, where it stands.
    let set = done(
        &text,
        Syntax::Toml,
        &Edit::SetChange {
            part: "wall".into(),
            number: 3,
            by: Some(-0.12),
            by_xyz: None,
            at: None,
        },
    );
    assert_eq!(
        set.summary,
        "set change 3 of \"wall\" (pull \"glass\"): by -0.12"
    );
    assert_eq!(
        set.text,
        text.replace(
            "{ pull = \"glass\", by = -0.08 }",
            "{ pull = \"glass\", by = -0.12 }"
        )
    );
    // The last change can't go while a colour names the walls only it
    // made...
    let remove_last = Edit::RemoveChange {
        part: "wall".into(),
        number: Some(3),
    };
    let failure = apply(
        &text,
        Syntax::Toml,
        Path::new("t.toml"),
        "t.toml",
        &remove_last,
    )
    .unwrap_err();
    assert_eq!(
        failure.report().errors[0].path,
        "parts[0].colors.glass_side"
    );
    // ...and goes with its line once none does.
    let text = text.replace(", glass_side = \"wood\"", "");
    let taken = done(&text, Syntax::Toml, &remove_last);
    assert_eq!(
        taken.summary,
        "took away change 3 of \"wall\" (pull \"glass\")"
    );
    assert_eq!(
        taken.text,
        text.replace("  { pull = \"glass\", by = -0.08 },\n", "")
    );
    // One a later change needs is refused, with the mistake it would
    // make.
    let failure = apply(
        &text,
        Syntax::Toml,
        Path::new("t.toml"),
        "t.toml",
        &Edit::RemoveChange {
            part: "wall".into(),
            number: Some(2),
        },
    )
    .unwrap_err();
    let report = failure.report();
    assert_eq!(report.errors[0].path, "parts[0].changes[1]");
    assert_eq!(
        report.errors[0].message,
        "there is no face \"glass\" on \"wall\""
    );
    // Numbers that aren't there.
    for (number, by) in [(0, Some(1.0)), (4, Some(1.0)), (1, None)] {
        assert!(
            apply(
                &text,
                Syntax::Toml,
                Path::new("t.toml"),
                "t.toml",
                &Edit::SetChange {
                    part: "wall".into(),
                    number,
                    by,
                    by_xyz: None,
                    at: None,
                },
            )
            .is_err()
        );
    }
    // All of them, with the colours that name what they made.
    let plain = wall("changes = [{ pull = \"top\", by = 0.5 }]");
    let all = done(
        &plain,
        Syntax::Toml,
        &Edit::RemoveChange {
            part: "wall".into(),
            number: None,
        },
    );
    assert_eq!(all.text, format!("{}\n", wall("").trim_end()));
    assert_eq!(
        all.summary,
        "took away the only change of \"wall\": it is a plain box again"
    );
}

#[test]
fn json_changes_are_set_and_taken_away_too() {
    let text = r##"{
  "version": 0,
  "palette": { "wood": "#8B5A2B" },
  "parts": [
    {
      "name": "crate",
      "shape": "box",
      "color": "wood",
      "changes": [
        { "pull": "top", "by": 0.25 },
        { "inset": "top", "by": 0.1 }
      ]
    }
  ]
}
"##;
    let set = done(
        text,
        Syntax::Json,
        &Edit::SetChange {
            part: "crate".into(),
            number: 1,
            by: Some(0.5),
            by_xyz: None,
            at: None,
        },
    );
    assert_eq!(set.text, text.replace("\"by\": 0.25", "\"by\": 0.5"));
    let taken = done(
        text,
        Syntax::Json,
        &Edit::RemoveChange {
            part: "crate".into(),
            number: Some(2),
        },
    );
    assert_eq!(
        taken.text,
        text.replace(
            "        { \"pull\": \"top\", \"by\": 0.25 },\n        { \"inset\": \"top\", \"by\": 0.1 }\n",
            "        { \"pull\": \"top\", \"by\": 0.25 }\n"
        )
    );
}

#[test]
fn another_shape_drops_the_changes_and_says_so() {
    use duck_mold_core::edit::Changes;
    let text = wall(WINDOW);
    let sphere = done(
        &text,
        Syntax::Toml,
        &Edit::Set {
            part: "wall".into(),
            changes: Changes {
                shape: Some("sphere".into()),
                ..Changes::default()
            },
        },
    );
    assert_eq!(
        sphere.summary,
        "set \"wall\": shape sphere, dropped 3 changes: only a box can be shaped"
    );
    assert!(!sphere.text.contains("changes"));
    // Still a box: they stay.
    let box_again = done(
        &text,
        Syntax::Toml,
        &Edit::Set {
            part: "wall".into(),
            changes: Changes {
                shape: Some("box".into()),
                ..Changes::default()
            },
        },
    );
    assert!(box_again.text.contains("changes = ["));
}

// ---- Phase 4, step 3: move, turn and scale a face ---------------------------------

fn moved(faces: &[&str], by: [f64; 3]) -> Change {
    Change::Move {
        faces: faces.iter().map(|face| (*face).to_owned()).collect(),
        by,
    }
}

#[test]
fn moving_turning_and_scaling_a_face_keeps_every_name() {
    let plain = shape_box([1.0; 3], [1.0; 2], &[], None).unwrap().names();
    // Up 0.5: a taller box.
    let taller = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top"], [0.0, 0.5, 0.0])],
        None,
    )
    .unwrap();
    assert!((taller.volume() - 1.5).abs() < 1e-12);
    assert_eq!(taller.names(), plain);
    // The top half as wide: a frustum.
    let frustum = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[Change::Scale {
            faces: vec!["top".into()],
            by: [0.5; 3],
        }],
        None,
    )
    .unwrap();
    // (1 + 0.25 + 0.5) / 3.
    assert!((frustum.volume() - 1.75 / 3.0).abs() < 1e-12);
    assert_eq!(frustum.names(), plain);
    // The top turned: the sides bend, and are drawn as two triangles
    // each, folded outwards.
    let twisted = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[Change::Turn {
            faces: vec!["top".into()],
            by: [0.0, 30.0, 0.0],
        }],
        None,
    )
    .unwrap();
    assert_eq!(twisted.names(), plain);
    whole(&twisted).unwrap();
    let out = twisted.polygons();
    assert_eq!(out.polygons.len(), 2 + 4 * 2);
    // A face slid sideways: the faces round it lean.
    let leaning = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top"], [0.3, 0.0, 0.0])],
        None,
    )
    .unwrap();
    whole(&leaning).unwrap();
    assert!((leaning.volume() - 1.0).abs() < 1e-12);
}

#[test]
fn a_move_that_turns_the_shape_inside_out_says_so() {
    let mistake = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top"], [0.0, -1.5, 0.0])],
        Some("rock"),
    )
    .unwrap_err();
    assert_eq!(mistake.code, Code::BadShape);
    assert_eq!(
        mistake.message,
        "moving \"top\" by [0, -1.5, 0] turns the shape inside out"
    );
    // Squeezed to nothing.
    let mistake = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top"], [0.0, -1.0, 0.0])],
        None,
    )
    .unwrap_err();
    assert!(mistake.message.contains("flat"), "{}", mistake.message);
}

#[test]
fn moves_turns_and_scales_are_read_and_checked() {
    let model = built(&wall(
        "changes = [\n  { move = \"top\", by = [0, 0.3, 0] },\n  { turn = \"front\", by = [0, 0, 10] },\n  { scale = \"back\", by = 0.5 },\n  { scale = \"left\", by = [1, 0.5, 0.5] },\n]",
    ));
    assert_eq!(first_part(&model).kind.changes, 4);
    let cases: [(&str, Code, &str, &str); 5] = [
        (
            "changes = [{ move = \"top\", by = [0, 0, 0] }]",
            Code::OutOfRange,
            "parts[0].changes[0].by",
            "a move of [0, 0, 0] changes nothing: give how far along x, y and z",
        ),
        (
            "changes = [{ scale = \"top\", by = 1 }]",
            Code::OutOfRange,
            "parts[0].changes[0].by",
            "a scale of 1 changes nothing: above 1 makes the faces bigger, below 1 smaller",
        ),
        (
            "changes = [{ turn = \"top\" }]",
            Code::MissingField,
            "parts[0].changes[0]",
            "a turn needs \"by\": degrees around x, y and z, like { turn = \"top\", by = [0, 15, 0] }",
        ),
        (
            "changes = [{ move = \"top\", by = [0, 1, 0], name = \"lid\" }]",
            Code::Conflict,
            "parts[0].changes[0].name",
            "a move makes no new faces, so it has nothing to name: remove \"name\"",
        ),
        (
            "changes = [{ move = \"top\", by = 0.5 }]",
            Code::TypeMismatch,
            "parts[0].changes[0].by",
            "this needs three numbers, like [0, 0.2, 0], not the number 0.5",
        ),
    ];
    for (lines, code, path, message) in cases {
        let error = one_error(&wall(lines));
        assert_eq!(
            (error.code, error.path.as_str(), error.message.as_str()),
            (code, path, message),
            "{lines}"
        );
    }
}

#[test]
fn a_move_right_after_a_move_of_the_same_faces_adds_to_it() {
    let start = wall("");
    let move_top = |by: [f64; 3]| Edit::MoveFace {
        part: "wall".into(),
        faces: vec!["top".into()],
        by,
    };
    let one = done(&start, Syntax::Toml, &move_top([0.0, 0.2, 0.0]));
    assert_eq!(one.summary, "moved \"top\" of \"wall\" by [0, 0.2, 0]");
    let two = done(&one.text, Syntax::Toml, &move_top([0.1, 0.1, 0.0]));
    assert_eq!(
        two.summary,
        "moved \"top\" of \"wall\" by [0.1, 0.1, 0] (moved by [0.1, 0.3, 0] in all, in one change)"
    );
    assert!(
        two.text
            .contains("changes = [\n  { move = \"top\", by = [0.1, 0.3, 0] },\n]")
    );
    // Back where it was: the line goes.
    let back = done(&two.text, Syntax::Toml, &move_top([-0.1, -0.3, 0.0]));
    assert_eq!(back.text, start);
    assert_eq!(
        back.summary,
        "moved \"top\" of \"wall\" by [-0.1, -0.3, 0]: back where it was, so the change is gone"
    );
    // Another face, or a move after something else: a line of its own.
    let other = done(
        &one.text,
        Syntax::Toml,
        &Edit::MoveFace {
            part: "wall".into(),
            faces: vec!["back".into()],
            by: [0.0, 0.0, -0.1],
        },
    );
    assert_eq!(other.text.matches("{ move = ").count(), 2);

    // Scales multiply, and three alike stay one number.
    let scale_top = |by: f64| Edit::ScaleFace {
        part: "wall".into(),
        faces: vec!["top".into()],
        by: [by; 3],
    };
    let half = done(&start, Syntax::Toml, &scale_top(0.5));
    assert!(half.text.contains("{ scale = \"top\", by = 0.5 }"));
    let quarter = done(&half.text, Syntax::Toml, &scale_top(0.5));
    assert!(quarter.text.contains("{ scale = \"top\", by = 0.25 }"));
    assert_eq!(
        quarter.summary,
        "scaled \"top\" of \"wall\" by 0.5 (scaled by 0.25 in all, in one change)"
    );
    // Turns add.
    let turn = |by: [f64; 3]| Edit::TurnFace {
        part: "wall".into(),
        faces: vec!["top".into()],
        by,
    };
    let once = done(&start, Syntax::Toml, &turn([0.0, 15.0, 0.0]));
    let twice = done(&once.text, Syntax::Toml, &turn([0.0, 15.0, 0.0]));
    assert!(twice.text.contains("{ turn = \"top\", by = [0, 30, 0] }"));
    // The panel reads three numbers, and set-change sets them.
    let facts = duck_mold_core::edit::describe(&twice.text, Syntax::Toml, None, "wall").unwrap();
    assert_eq!(facts.changes[0].by_xyz, Some([0.0, 30.0, 0.0]));
    let set = done(
        &twice.text,
        Syntax::Toml,
        &Edit::SetChange {
            part: "wall".into(),
            number: 1,
            by: None,
            by_xyz: Some([0.0, 45.0, 0.0]),
            at: None,
        },
    );
    assert!(set.text.contains("{ turn = \"top\", by = [0, 45, 0] }"));
}

// ---- Phase 4, step 4: edges and cuts ----------------------------------------------

fn cut(toward: Side, at: f64) -> Change {
    Change::Cut {
        toward,
        at,
        name: None,
    }
}

use duck_mold_core::recipe::Side;

#[test]
fn a_cut_splits_every_face_it_crosses_and_names_the_slices_by_side() {
    let shaped = shape_box([1.0; 3], [1.0; 2], &[cut(Side::Right, 0.5)], None).unwrap();
    assert_eq!(
        own_names(&shaped),
        [
            "top",
            "bottom",
            "left",
            "right",
            "front",
            "back",
            "top_right",
            "bottom_right",
            "front_right",
            "back_right"
        ]
    );
    // Nothing moved.
    assert!((shaped.volume() - 1.0).abs() < 1e-12);
    whole(&shaped).unwrap();
    // The top's piece towards the middle is 0.75 wide; the slice 0.25.
    let top = shaped.faces_named("top")[0];
    let slice = shaped.faces_named("top_right")[0];
    assert!((shaped.extent(top)[0] - 0.75).abs() < 1e-12);
    assert!((shaped.extent(slice)[0] - 0.25).abs() < 1e-12);
    // The new edges know their sides: the top's along the cut is its
    // right; the slice's is its left.
    assert!(shaped.faces[top].sides.contains(&Side::Right));
    assert_eq!(
        shaped
            .neighbours(top)
            .iter()
            .filter(|&&f| shaped.faces[f].name == "top_right")
            .count(),
        1
    );
    // A second cut that crosses the top again: the slice's name is taken,
    // so it gets a number.
    let again = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[cut(Side::Right, 0.8), cut(Side::Right, 0.4)],
        None,
    )
    .unwrap();
    assert_eq!(again.faces_named("top_right_2").len(), 1);
    whole(&again).unwrap();
    // A named cut.
    let named = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[Change::Cut {
            toward: Side::Front,
            at: 0.9,
            name: Some("rim".into()),
        }],
        None,
    )
    .unwrap();
    assert_eq!(named.faces_named("top_rim").len(), 1);
}

#[test]
fn the_designs_chair_is_made_from_one_box() {
    let chair = r#"changes = [
  { cut = "left", at = 0.8 },
  { cut = "right", at = 0.8 },
  { cut = "front", at = 0.8 },
  { cut = "back", at = 0.8 },
  { pull = "bottom_left_front", by = 0.42, name = "leg_lf" },
  { pull = "bottom_right_front", by = 0.42, name = "leg_rf" },
  { pull = "bottom_left_back", by = 0.42, name = "leg_lb" },
  { pull = "bottom_right_back", by = 0.42, name = "leg_rb" },
  { pull = ["top_left_back", "top_back", "top_right_back"], by = 0.5, name = "backrest" },
]"#;
    let text = format!(
        "{PALETTE}[[parts]]\nname = \"chair\"\nshape = \"box\"\nsize = [0.5, 0.06, 0.5]\ncolor = \"wood\"\n{chair}\n"
    );
    let model = built(&text);
    let part = first_part(&model);
    let shaped = part.shaped.as_ref().unwrap();
    whole(shaped).unwrap();
    // Four legs of four walls, the backrest's walls round the outside
    // of its three faces (3 + 1 + 3 + 1 = 8).
    for leg in ["leg_lf", "leg_rf", "leg_lb", "leg_rb"] {
        assert_eq!(shaped.faces_named(leg).len(), 4, "{leg}");
    }
    assert_eq!(shaped.faces_named("backrest").len(), 8);
    assert_eq!(shaped.faces_named("backrest_front").len(), 3);
    let bounds = part.bounds.unwrap();
    assert!((bounds.size().y - 0.98).abs() < 1e-9);
    // Resized wider, every name is still there.
    let wide = text.replace("size = [0.5, 0.06, 0.5]", "size = [0.8, 0.05, 0.6]");
    let wide = built(&wide);
    assert_eq!(
        first_part(&wide).shaped.as_ref().unwrap().names(),
        shaped.names()
    );
}

#[test]
fn an_edge_is_moved_by_the_faces_that_meet_there() {
    let ramp = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top+front"], [0.0, -0.5, 0.0])],
        None,
    )
    .unwrap();
    whole(&ramp).unwrap();
    // Half the front's height gone along its top edge: 1 - 0.25.
    assert!((ramp.volume() - 0.75).abs() < 1e-12);
    // The other way round names the same edge.
    let same = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["front+top"], [0.0, -0.5, 0.0])],
        None,
    )
    .unwrap();
    assert_eq!(ramp.corners, same.corners);
    // Faces that don't meet, or a turn of an edge.
    let mistake = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[moved(&["top+bottom"], [0.0, 0.1, 0.0])],
        Some("crate"),
    )
    .unwrap_err();
    assert_eq!(
        mistake.message,
        "there is no edge \"top+bottom\" on \"crate\": \"top\" and \"bottom\" don't meet"
    );
    let mistake = shape_box(
        [1.0; 3],
        [1.0; 2],
        &[Change::Turn {
            faces: vec!["top+front".into()],
            by: [10.0, 0.0, 0.0],
        }],
        None,
    )
    .unwrap_err();
    assert_eq!(
        mistake.message,
        "only a move can be made to an edge or a corner, not a turn"
    );
    // Edges are listed with names made of their faces.
    let names: Vec<String> = ramp
        .edges()
        .iter()
        .map(|edge| ramp.edge_name(edge))
        .collect();
    assert_eq!(names.len(), 12);
    assert!(names.contains(&"top+front".to_owned()), "{names:?}");
    assert!(names.contains(&"front+left".to_owned()), "{names:?}");
}

#[test]
fn cuts_are_read_and_checked() {
    let cases: [(&str, Code, &str, &str); 3] = [
        (
            "changes = [{ cut = \"back\", at = 1.2 }]",
            Code::OutOfRange,
            "parts[0].changes[0].at",
            "the cut towards \"back\" at 1.2 is outside the part: \"at\" goes from 0 (the middle) to just under 1 (the side)",
        ),
        (
            "changes = [{ cut = \"rght\", at = 0.5 }]",
            Code::TypeMismatch,
            "parts[0].changes[0].cut",
            "a cut faces one of the box's sides: \"top\", \"bottom\", \"left\", \"right\", \"front\" or \"back\", not \"rght\"",
        ),
        (
            "changes = [{ cut = \"right\" }]",
            Code::MissingField,
            "parts[0].changes[0]",
            "a cut needs \"at\": how far from the middle (0) to the side (just under 1), like { cut = \"right\", at = 0.8 }",
        ),
    ];
    for (lines, code, path, message) in cases {
        let error = one_error(&wall(lines));
        assert_eq!(
            (error.code, error.path.as_str(), error.message.as_str()),
            (code, path, message),
            "{lines}"
        );
    }
    let error = one_error(&wall("changes = [{ cut = \"rght\", at = 0.5 }]"));
    assert_eq!(error.suggestion.as_deref(), Some("right"));
}

#[test]
fn edges_and_cuts_are_written_by_their_edits() {
    let start = wall("");
    let cut_edit = done(
        &start,
        Syntax::Toml,
        &Edit::Cut {
            part: "wall".into(),
            toward: "right".into(),
            at: 0.8,
            name: None,
        },
    );
    assert_eq!(cut_edit.summary, "cut \"wall\" towards \"right\" at 0.8");
    assert!(
        cut_edit
            .text
            .contains("changes = [\n  { cut = \"right\", at = 0.8 },\n]")
    );
    let edge = Edit::MoveEdge {
        part: "wall".into(),
        edges: vec!["top+front".into()],
        by: [0.0, -0.1, 0.0],
    };
    let once = done(&cut_edit.text, Syntax::Toml, &edge);
    assert_eq!(
        once.summary,
        "moved the edge \"top+front\" of \"wall\" by [0, -0.1, 0]"
    );
    let twice = done(&once.text, Syntax::Toml, &edge);
    assert!(
        twice
            .text
            .contains("  { move = \"top+front\", by = [0, -0.2, 0] },\n")
    );
    // A cut is moved with set-change --at.
    let moved_cut = done(
        &twice.text,
        Syntax::Toml,
        &Edit::SetChange {
            part: "wall".into(),
            number: 1,
            by: None,
            by_xyz: None,
            at: Some(0.6),
        },
    );
    assert!(moved_cut.text.contains("{ cut = \"right\", at = 0.6 }"));
    // Wrong words are refused before anything is written.
    for bad in [
        Edit::Cut {
            part: "wall".into(),
            toward: "up".into(),
            at: 0.5,
            name: None,
        },
        Edit::Cut {
            part: "wall".into(),
            toward: "right".into(),
            at: 1.0,
            name: None,
        },
        Edit::MoveEdge {
            part: "wall".into(),
            edges: vec!["top".into()],
            by: [0.0, 0.1, 0.0],
        },
    ] {
        assert!(
            apply(&start, Syntax::Toml, Path::new("t.toml"), "t.toml", &bad).is_err(),
            "{bad:?}"
        );
    }
}

// ---- Phase 4, step 5: corners --------------------------------------------------------

#[test]
fn a_box_cut_through_its_middle_three_ways_has_26_corners_to_move() {
    let cuts = [
        cut(Side::Right, 0.0),
        cut(Side::Top, 0.0),
        cut(Side::Front, 0.0),
    ];
    let rock = shape_box([1.0; 3], [1.0; 2], &cuts, None).unwrap();
    assert_eq!(rock.corners.len(), 26);
    assert_eq!(rock.faces.len(), 24);
    whole(&rock).unwrap();
    // The corner the cuts left alone keeps the box's names; the others
    // are named by the slices that meet there.
    let names: Vec<String> = (0..26).map(|corner| rock.corner_name(corner)).collect();
    assert!(names.contains(&"bottom+back+left".to_owned()), "{names:?}");
    assert!(
        names.contains(&"top_right_front+front_right_top+right_top_front".to_owned()),
        "{names:?}"
    );
    // Moved in and out, it is a rock.
    let mut changes = cuts.to_vec();
    changes.push(moved(&["bottom+back+left"], [0.1, 0.1, 0.05]));
    changes.push(moved(
        &["top_right_front+front_right_top+right_top_front"],
        [-0.1, -0.05, -0.1],
    ));
    let shaped = shape_box([1.0; 3], [1.0; 2], &changes, None);
    match shaped {
        Ok(rock) => whole(&rock).unwrap(),
        Err(mistake) => panic!("{mistake:?}"),
    }
    // Too far, through the middle: refused, naming the corner.
    let mut too_far = cuts.to_vec();
    too_far.push(moved(&["bottom+back+left"], [1.2, 1.2, 1.2]));
    let mistake = shape_box([1.0; 3], [1.0; 2], &too_far, Some("rock")).unwrap_err();
    assert_eq!(mistake.change, 3);
    assert_eq!(mistake.code, Code::BadShape);
    assert!(
        mistake
            .message
            .starts_with("moving the corner \"bottom+back+left\" by [1.2, 1.2, 1.2]"),
        "{}",
        mistake.message
    );
}

#[test]
fn a_corner_is_moved_by_its_edit_and_a_second_move_adds_to_it() {
    let start = wall("");
    let corner = Edit::MoveCorner {
        part: "wall".into(),
        corners: vec!["top+front+left".into()],
        by: [0.0, -0.1, 0.0],
    };
    let once = done(&start, Syntax::Toml, &corner);
    assert_eq!(
        once.summary,
        "moved the corner \"top+front+left\" of \"wall\" by [0, -0.1, 0]"
    );
    let twice = done(&once.text, Syntax::Toml, &corner);
    assert!(
        twice
            .text
            .contains("  { move = \"top+front+left\", by = [0, -0.2, 0] },\n")
    );
    // Two names aren't a corner.
    let bad = Edit::MoveCorner {
        part: "wall".into(),
        corners: vec!["top+front".into()],
        by: [0.0, 0.1, 0.0],
    };
    assert!(apply(&start, Syntax::Toml, Path::new("t.toml"), "t.toml", &bad).is_err());
}

// ---- Phase 4, step 6: new faces work like old ones --------------------------------------

#[test]
fn a_socket_sits_on_a_face_a_change_made_and_moves_with_it() {
    let text = wall(&format!(
        "{WINDOW}\n\n[parts.sockets]\nlamp = {{ on = \"glass\" }}\nhook = {{ on = \"window_rim_top\", shift = [0.5, 0, 0] }}"
    ));
    let model = built(&text);
    let part = first_part(&model);
    let socket = |name: &str| {
        part.sockets
            .iter()
            .find(|socket| socket.name == name)
            .unwrap()
            .clone()
    };
    // The glass's middle, 8 cm into the 20 cm wall, facing out.
    let lamp = socket("lamp");
    assert!(
        ((lamp.at - model.zero).z - 0.02).abs() < 1e-9,
        "{:?}",
        lamp.at
    );
    assert!((lamp.at - model.zero).x.abs() < 1e-9);
    assert!((lamp.facing - Vec3::FORWARD).length() < 1e-9);
    // Slid half way to the right of the frame's top piece.
    let hook = socket("hook");
    assert!(
        ((hook.at - model.zero).x - 0.75).abs() < 1e-9,
        "{:?}",
        hook.at
    );
    // Pushed further in, the lamp's socket goes with the glass.
    let deeper = built(&text.replace("by = -0.08", "by = -0.12"));
    let lamp = first_part(&deeper)
        .sockets
        .iter()
        .find(|socket| socket.name == "lamp")
        .unwrap()
        .clone();
    assert!(
        ((lamp.at - deeper.zero).z + 0.02).abs() < 1e-9,
        "{:?}",
        lamp.at
    );
    // A part joined to it sits on the glass.
    let joined = format!(
        "{text}\n[[parts]]\nname = \"bulb\"\nshape = \"sphere\"\nradius = 0.1\ncolor = \"sky\"\nattach = \"back\"\nto = \"wall.lamp\"\n"
    );
    let model = built(&joined);
    let bulb = model.parts.iter().find(|part| part.name == "bulb").unwrap();
    let bounds = bulb.bounds.unwrap();
    assert!(
        ((bounds.min.z - model.zero.z) - 0.02).abs() < 1e-6,
        "{bounds:?}"
    );
}

#[test]
fn a_socket_on_a_face_that_isnt_there_is_a_mistake_with_the_closest() {
    let error = one_error(&wall(&format!(
        "{WINDOW}\n\n[parts.sockets]\nlamp = {{ on = \"glas\" }}"
    )));
    assert_eq!(error.code, Code::UnknownFace);
    assert_eq!(error.path, "parts[0].sockets.lamp.on");
    assert_eq!(
        error.message,
        "\"glas\" is neither a side nor a face of this part"
    );
    assert_eq!(error.suggestion.as_deref(), Some("glass"));
    // On a part that isn't shaped, it is no side, as before.
    let error = one_error(&wall("\n[parts.sockets]\nlamp = { on = \"tp\" }"));
    assert_eq!(error.code, Code::TypeMismatch);
    assert_eq!(error.message, "there is no side \"tp\"");
    assert_eq!(error.suggestion.as_deref(), Some("top"));
}

#[test]
fn a_click_on_a_new_face_makes_a_socket_named_by_it() {
    use duck_mold_core::edit::spot;
    let model = built(&wall(WINDOW));
    let part = first_part(&model);
    // A point on the glass, a little right of its middle.
    let glass = part.faces_named("glass")[0];
    let face = &model.faces[glass];
    let middle = face.corners.iter().fold(Vec3::ZERO, |sum, &c| sum + c) * 0.25;
    let at = middle + duck_mold_core::math::vec3(0.42, 0.0, 0.0);
    let spot = spot(&model, part, at, face.normal().unwrap()).unwrap();
    assert_eq!(spot.on, "glass");
    assert_eq!(spot.shift, [0.5, 0.0, 0.0]);
    // The edit writes it, and the socket is there.
    let text = wall(WINDOW);
    let added = done(
        &text,
        Syntax::Toml,
        &Edit::AddSocket {
            part: "wall".into(),
            name: "lamp".into(),
            on: spot.on.clone(),
            shift: spot.shift,
        },
    );
    assert!(
        added
            .text
            .contains("lamp = { on = \"glass\", shift = [0.5, 0, 0] }")
    );
    let lamp = first_part(added.model.as_ref().unwrap())
        .sockets
        .iter()
        .find(|socket| socket.name == "lamp")
        .unwrap()
        .clone();
    assert!((lamp.at - at).length() < 1e-9);
}

#[test]
fn a_new_face_is_painted_by_its_name() {
    use duck_mold_core::edit::Changes;
    let text = wall("changes = [{ inset = \"front\", by = 0.5, name = \"window\" }]");
    let painted = done(
        &text,
        Syntax::Toml,
        &Edit::Set {
            part: "wall".into(),
            changes: Changes {
                face_colors: vec![("window".into(), "sky".into())],
                ..Changes::default()
            },
        },
    );
    assert!(painted.text.contains("colors = { window = \"sky\" }"));
    assert_eq!(
        triangles_by_colour(painted.model.as_ref().unwrap()),
        vec![("plaster".to_owned(), 18), ("sky".to_owned(), 2)]
    );
}

// ---- Phase 4, step 7: using a shaped part again --------------------------------------------

#[test]
fn a_part_becomes_a_definition_and_the_model_is_the_same() {
    let text = format!(
        "{PALETTE}# The wall.\n[[parts]]\nname = \"wall\"\nshape = \"box\"\nsize = [3, 2.4, 0.2]\ncolor = \"plaster\"\n{WINDOW}\nmove = [0, 1.2, 0]\n\n[parts.sockets]\nlamp = {{ on = \"glass\" }}\n"
    );
    let before = built(&text);
    let made = done(
        &text,
        Syntax::Toml,
        &Edit::Define {
            part: "wall".into(),
            name: "window_wall".into(),
        },
    );
    assert_eq!(
        made.summary,
        "made \"wall\" a definition, \"window_wall\": it is in [define.window_wall], and \"wall\" uses it"
    );
    // The definition holds what it is made of; the part, where it is.
    assert!(made.text.contains("[define.window_wall]\nshape = \"box\"\nsize = [3, 2.4, 0.2]\ncolor = \"plaster\"\nchanges = [\n"));
    assert!(
        made.text
            .contains("sockets = { lamp = { on = \"glass\" } }\n")
    );
    assert!(made.text.contains(
        "\n\n# The wall.\n[[parts]]\nname = \"wall\"\nuse = \"window_wall\"\nmove = [0, 1.2, 0]\n"
    ));
    let after = made.model.unwrap();
    assert_eq!(after.faces, before.faces);
    assert_eq!(
        first_part(&after).kind.label(),
        "window_wall (box, 3 changes)"
    );
    // A part made from a definition already is refused.
    let failure = apply(
        &made.text,
        Syntax::Toml,
        Path::new("t.toml"),
        "t.toml",
        &Edit::Define {
            part: "wall".into(),
            name: "x".into(),
        },
    )
    .unwrap_err();
    let message = &failure.report().errors[0].message;
    assert!(
        message.contains("is made from the definition \"window_wall\" already"),
        "{message}"
    );
}

#[test]
fn a_json_part_becomes_a_definition_too() {
    let text = r##"{
  "version": 0,
  "palette": { "wood": "#8B5A2B" },
  "parts": [
    {
      "name": "crate",
      "shape": "box",
      "color": "wood",
      "changes": [{ "pull": "top", "by": 0.25 }]
    }
  ]
}
"##;
    let made = done(
        text,
        Syntax::Json,
        &Edit::Define {
            part: "crate".into(),
            name: "tall_crate".into(),
        },
    );
    assert!(made.text.contains("\"define\": { \"tall_crate\": { \"shape\": \"box\", \"color\": \"wood\", \"changes\": [{ \"pull\": \"top\", \"by\": 0.25 }] } }"), "{}", made.text);
    assert!(made.text.contains("\"use\": \"tall_crate\""));
    assert_eq!(made.model.unwrap().triangle_count(), 20);
}

#[test]
fn a_part_is_saved_alone_with_the_colours_it_uses() {
    use duck_mold_core::edit::part_recipe;
    let text = wall(WINDOW).replace(
        "color = \"plaster\"\n",
        "color = \"plaster\"\nmove = [2, 1.2, 0]\n",
    );
    let saved = part_recipe(&text, Syntax::Toml, Path::new("house.toml"), "wall").unwrap();
    assert_eq!(
        saved,
        format!(
            "# Saved by `duck-mold save-part` from house.toml.\nversion = 0\nname = \"wall\"\n\n[palette]\nplaster = \"#E8E0D0\"\nwood = \"#8B5A2B\"\nsky = \"#9FD3F0\"\n\n[[parts]]\nname = \"wall\"\nshape = \"box\"\nsize = [3, 2.4, 0.2]\ncolor = \"plaster\"\n{WINDOW}\n"
        )
    );
    // It builds, the same shape, standing on the ground.
    let model = built(&saved);
    assert_eq!(model.triangle_count(), 36);
    // Only the colours it uses come with it.
    let plain = part_recipe(&wall(""), Syntax::Toml, Path::new("house.toml"), "wall").unwrap();
    assert!(plain.contains("[palette]\nplaster = \"#E8E0D0\"\n\n"));
    assert!(!plain.contains("wood"));
}

// ---- Phase 4, step 9: the examples ---------------------------------------------------

#[test]
fn the_shaped_examples_are_closed_and_face_outwards() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    fn shaped_parts<'a>(parts: &'a [Part], into: &mut Vec<&'a Part>) {
        for part in parts {
            if part.shaped.is_some() {
                into.push(part);
            }
            shaped_parts(&part.parts, into);
        }
    }
    for (example, count, corners) in [
        ("chair", 1, None),
        ("window_wall", 1, None),
        ("rock", 1, Some(26)),
        ("house", 2, None),
    ] {
        let built = duck_mold_core::tools::build(&folder.join(format!("{example}.toml")))
            .unwrap_or_else(|_| panic!("{example} builds"));
        assert!(built.warnings.is_empty(), "{example}: {:?}", built.warnings);
        let mut found = Vec::new();
        shaped_parts(&built.model.parts, &mut found);
        assert_eq!(found.len(), count, "{example}");
        for part in found {
            let shaped = part.shaped.as_ref().unwrap();
            whole(shaped).unwrap_or_else(|why| panic!("{example}/{}: {why}", part.path));
            if let Some(corners) = corners {
                assert_eq!(shaped.corners.len(), corners, "{example}");
            }
        }
    }
    // The chair has every name the design gave it.
    let built = duck_mold_core::tools::build(&folder.join("chair.toml")).unwrap();
    let chair = built.model.parts[0].shaped.as_ref().unwrap();
    let names = chair.names();
    for name in [
        "leg_lf",
        "leg_rb",
        "backrest",
        "backrest_front",
        "top",
        "top_left_back",
    ] {
        assert!(names.iter().any(|known| known == name), "{name}: {names:?}");
    }
}
