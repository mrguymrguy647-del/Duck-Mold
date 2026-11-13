//! `duck-mold render`'s pictures: each view sees the side it says it
//! sees, every view is drawn at the same scale, and the same model always
//! gives the same bytes.

use std::path::Path;

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::Issue;
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::source::Syntax;
use duck_mold_view::render::{BACKGROUND, Sheet, render_sheet, render_strip, strip_times};

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
        carried: Vec::new(),
        poses: Vec::new(),
        animations: Vec::new(),
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

// ---- an animation as a strip of frames -------------------------------------------------

/// A red pole on a white base. In `down` it lies over to the model's
/// left, which is the left of a picture from the front too.
const FLAG: &str = r##"
version = 0
name = "flag"

[palette]
red = "#E02020"
white = "#F0F0F0"

[[parts]]
name = "base"
shape = "box"
size = [0.4, 0.1, 0.4]
color = "white"

[[parts]]
name = "pole"
shape = "box"
size = [0.1, 1.0, 0.1]
color = "red"
attach = "bottom"
to = "base.top"

[poses.down]
pole = [0, 0, -90]

[animations.fall]
smooth = false
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "down" },
]

[animations.swing]
loop = true
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "down" },
  { at = 2, pose = "rest" },
]
"##;

/// How many red pixels a frame has, how many of them are in its top
/// half, and how far from its left edge the leftmost is.
fn red_in(strip: &duck_mold_view::Strip, frame: usize) -> (usize, usize, usize) {
    let place = &strip.frames[frame];
    let (mut all, mut top, mut leftmost) = (0, 0, place.size);
    // The top rows hold the frame's time: leave them out.
    for y in place.size / 8..place.size {
        for x in 0..place.size {
            let pixel = strip.pixels[(place.top + y) * strip.width + place.left + x];
            if colour_name(pixel) == "red" {
                all += 1;
                top += usize::from(y < place.size / 2);
                leftmost = leftmost.min(x);
            }
        }
    }
    (all, top, leftmost)
}

#[test]
fn a_strip_shows_an_animation_frame_by_frame_from_one_place() {
    let model = built(FLAG);
    let fall = model.animation("fall").unwrap();
    let strip = render_strip(&model, fall, "flag", 128, 5, "front");
    // Five frames side by side, a line between each two, and the caption
    // under them.
    assert_eq!(strip.width, 5 * 128 + 4);
    assert!(strip.height > 128 && strip.height < 128 + 32);
    assert_eq!(strip.view, "front");
    let times: Vec<f64> = strip.frames.iter().map(|frame| frame.at).collect();
    assert_eq!(times, [0.0, 0.25, 0.5, 0.75, 1.0]);
    let lefts: Vec<usize> = strip.frames.iter().map(|frame| frame.left).collect();
    assert_eq!(lefts, [0, 129, 258, 387, 516]);
    // The pole stands in the first frame and lies over to the left in
    // the last: its red leaves the top half and reaches far to the left.
    let (all, top, leftmost) = red_in(&strip, 0);
    assert!(
        all > 100 && top > all / 2 && leftmost > 64,
        "{all} {top} {leftmost}"
    );
    let (all_after, top_after, leftmost_after) = red_in(&strip, 4);
    assert!(
        top_after == 0 && leftmost_after < 32,
        "{all_after} {top_after} {leftmost_after}"
    );
    // Half way, it is half way over.
    let (_, top_between, leftmost_between) = red_in(&strip, 2);
    assert!(top_between > 0 && top_between < top, "{top_between}");
    assert!(leftmost_between < leftmost && leftmost_between > leftmost_after);
    // It is the same pole all the way: the same size in every frame,
    // because every frame is drawn from the same place. (A pole on its
    // side shows the same area as one standing.)
    assert!(
        all.abs_diff(all_after) < all / 8,
        "{all} then {all_after} red pixels"
    );
    // The same animation always gives the same picture.
    assert_eq!(
        strip.to_png(),
        render_strip(&model, fall, "flag", 128, 5, "front").to_png()
    );
    // Another view is another picture.
    assert_ne!(
        strip.pixels,
        render_strip(&model, fall, "flag", 128, 5, "three-quarter").pixels
    );
    assert_eq!(
        render_strip(&model, fall, "flag", 128, 5, "three-quarter").view,
        "three_quarter"
    );
}

#[test]
fn a_loops_strip_stops_a_step_before_its_end() {
    let model = built(FLAG);
    // One that plays once ends on its last moment.
    assert_eq!(
        strip_times(model.animation("fall").unwrap(), 3),
        [0.0, 0.5, 1.0]
    );
    // One that loops ends where it starts, so its last frame would be
    // its first again: the strip stops a step short.
    assert_eq!(
        strip_times(model.animation("swing").unwrap(), 4),
        [0.0, 0.5, 1.0, 1.5]
    );
    let strip = render_strip(
        &model,
        model.animation("swing").unwrap(),
        "flag",
        128,
        4,
        "front",
    );
    let (_, top, _) = red_in(&strip, 0);
    let (_, top_down, leftmost_down) = red_in(&strip, 2);
    assert!(top > 100);
    assert!(
        top_down == 0 && leftmost_down < 32,
        "{top_down} {leftmost_down}"
    );
}
