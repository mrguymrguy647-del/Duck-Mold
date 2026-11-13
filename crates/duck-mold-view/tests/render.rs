//! `duck-mold render`'s pictures: each view sees the side it says it
//! sees, every view is drawn at the same scale, and the same model always
//! gives the same bytes.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::Issue;
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::source::Syntax;
use duck_mold_view::render::{BACKGROUND, Sheet, render_sheet};

/// Builds a recipe that must have no errors.
fn built(text: &str) -> Model {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", Path::new("."), &mut issues);
    let model = assemble(&loaded, &mut issues);
    let errors: Vec<&Issue> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.expect("a recipe without errors builds")
}

/// A cube with every side its own colour: red front, blue back, yellow
/// right, green left, white top.
const COLOURED_CUBE: &str = r##"
version = 0
name = "cube"

[palette]
red = "#E02020"
blue = "#2020E0"
yellow = "#E0E020"
green = "#20C020"
white = "#F0F0F0"
black = "#101010"

[[parts]]
name = "cube"
shape = "box"
colors = { front = "red", back = "blue", right = "yellow", left = "green", top = "white", bottom = "black" }
"##;

/// Which of the cube's colours a pixel is, going by which of red, green
/// and blue are strong in it (the light makes them darker, but not
/// another colour).
fn colour_name(pixel: u32) -> &'static str {
    if pixel == BACKGROUND.to_u32() {
        return "background";
    }
    let [_, r, g, b] = pixel.to_be_bytes();
    let strong = |channel: u8| channel > 70;
    match (strong(r), strong(g), strong(b)) {
        (true, false, false) => "red",
        (false, false, true) => "blue",
        (true, true, false) => "yellow",
        (false, true, false) => "green",
        (true, true, true) => "white",
        _ => "other",
    }
}

/// The view with this name: where it is in the sheet.
fn view<'a>(sheet: &'a Sheet, name: &str) -> &'a duck_mold_view::ViewPlace {
    sheet
        .views
        .iter()
        .find(|view| view.name == name)
        .unwrap_or_else(|| panic!("no view {name}"))
}

/// The pixel in the middle of a view.
fn middle(sheet: &Sheet, name: &str) -> u32 {
    let view = view(sheet, name);
    sheet.pixel(view.left + view.size / 2, view.top + view.size / 2)
}

/// Every pixel of a view, with where it is in the view.
fn pixels<'a>(sheet: &'a Sheet, name: &str) -> impl Iterator<Item = (usize, usize, u32)> + 'a {
    let view = view(sheet, name).clone();
    // The top rows hold the view's name: leave them out.
    (view.size / 8..view.size).flat_map(move |y| {
        (0..view.size).map(move |x| (x, y, sheet.pixel(view.left + x, view.top + y)))
    })
}

#[test]
fn each_view_sees_the_side_it_names() {
    let sheet = render_sheet(&built(COLOURED_CUBE), "cube", 128);
    assert_eq!(colour_name(middle(&sheet, "front")), "red");
    assert_eq!(colour_name(middle(&sheet, "right")), "yellow");
    assert_eq!(colour_name(middle(&sheet, "back")), "blue");

    // From the front right and above, the front, the right side and the
    // top all show, and nothing else.
    let mut seen: Vec<&str> = pixels(&sheet, "three_quarter")
        .map(|(_, _, pixel)| colour_name(pixel))
        .filter(|&name| name != "background")
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, ["red", "white", "yellow"]);
}

#[test]
fn the_side_view_shows_the_model_facing_right_and_the_front_view_has_its_left_on_the_right() {
    // A grey body with a red nose on its front and a green ear on its left.
    let model = built(
        r##"
version = 0
name = "head"

[palette]
red = "#E02020"
green = "#20C020"

[[parts]]
name = "body"
shape = "box"

[[parts]]
name = "nose"
shape = "box"
size = [0.3, 0.3, 0.3]
color = "red"
attach = "back"
to = "body.front"

[[parts]]
name = "ear"
shape = "box"
size = [0.3, 0.3, 0.3]
color = "green"
attach = "right"
to = "body.left"
"##,
    );
    let sheet = render_sheet(&model, "head", 128);
    let average_x = |name: &str, colour: &str| {
        let xs: Vec<usize> = pixels(&sheet, name)
            .filter(|&(_, _, pixel)| colour_name(pixel) == colour)
            .map(|(x, _, _)| x)
            .collect();
        assert!(!xs.is_empty(), "no {colour} in the {name} view");
        xs.iter().sum::<usize>() as f64 / xs.len() as f64
    };
    let half = 64.0;
    assert!(average_x("right", "red") > half, "the nose points right");
    assert!(
        average_x("front", "green") > half,
        "its left is on our right"
    );
    assert!(
        average_x("back", "green") < half,
        "and on our left from behind"
    );
}

#[test]
fn every_view_is_drawn_at_the_same_scale() {
    // A plain cube looks like the same square from the front, the side
    // and the back.
    let model = built("version = 0\n[[parts]]\nname = \"cube\"\nshape = \"box\"\n");
    let sheet = render_sheet(&model, "cube", 128);
    let area = |name: &str| {
        pixels(&sheet, name)
            .filter(|&(_, _, pixel)| pixel != BACKGROUND.to_u32())
            .count()
    };
    let front = area("front");
    assert!(front > 1000, "the cube fills part of the view: {front}");
    assert_eq!(area("right"), front);
    assert_eq!(area("back"), front);
}

#[test]
fn the_same_model_always_gives_the_same_picture() {
    let model = built(COLOURED_CUBE);
    let first = render_sheet(&model, "cube", 128).to_png();
    let second = render_sheet(&built(COLOURED_CUBE), "cube", 128).to_png();
    assert_eq!(first, second);
    assert_eq!(&first[0..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn size_sets_how_big_each_view_is() {
    let model = built(COLOURED_CUBE);
    // Two views and the line between them across; the same down, then a
    // line and the caption, whose words grow with the views.
    for (size, height) in [(128, 269), (256, 536), (300, 624)] {
        let sheet = render_sheet(&model, "cube", size);
        assert_eq!(
            (sheet.width, sheet.height),
            (2 * size + 1, height),
            "{size}"
        );
        assert_eq!(sheet.pixels.len(), sheet.width * sheet.height);
        assert!(sheet.views.iter().all(|view| view.size == size));
    }
}

#[test]
fn a_model_with_no_faces_gives_empty_views() {
    let model = Model {
        name: "nothing".to_owned(),
        palette: Vec::new(),
        faces: Vec::new(),
        parts: Vec::new(),
        sockets: Vec::new(),
        zero: duck_mold_core::math::Vec3::ZERO,
    };
    let sheet = render_sheet(&model, "nothing", 128);
    for name in ["front", "right", "back", "three_quarter"] {
        assert!(pixels(&sheet, name).all(|(_, _, pixel)| pixel == BACKGROUND.to_u32()));
    }
}

#[test]
fn a_long_caption_is_cut_short_to_fit() {
    let model = built(COLOURED_CUBE);
    let long = "a very long name ".repeat(20);
    let sheet = render_sheet(&model, &long, 128);
    // The caption's row is the last few rows: nothing reaches the right
    // edge.
    let caption = 2 * 128 + 2..sheet.height;
    assert!(
        caption
            .clone()
            .all(|y| sheet.pixel(sheet.width - 1, y) == BACKGROUND.to_u32())
    );
    // But there is a caption.
    assert!(
        caption
            .into_iter()
            .any(|y| { (0..sheet.width).any(|x| sheet.pixel(x, y) != BACKGROUND.to_u32()) })
    );
}
