//! Phase 7b, step 1: `render` draws a model's images, through Duck
//! Forge's CPU renderer, the right way round and with sharp pixels.

use std::path::{Path, PathBuf};

use duck_mold_core::assemble::assemble;
use duck_mold_core::issue::Issue;
use duck_mold_core::load::load_text;
use duck_mold_core::model::Model;
use duck_mold_core::picture::write_png;
use duck_mold_core::source::Syntax;
use duck_mold_view::render::{BACKGROUND, Sheet, render_sheet};

/// A folder for one test, with `quad.png` (4 x 4, solid: red top-left,
/// green top-right, blue bottom-left, yellow bottom-right) and
/// `mouth.png` (4 x 2: see-through but for a black bottom row).
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("view_images")
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    let quad: Vec<[u8; 4]> = (0..16)
        .map(|at| match (at % 4 < 2, at / 4 < 2) {
            (true, true) => [224, 32, 32, 255],
            (false, true) => [32, 192, 32, 255],
            (true, false) => [32, 32, 224, 255],
            (false, false) => [224, 224, 32, 255],
        })
        .collect();
    std::fs::write(folder.join("quad.png"), write_png(4, 4, &quad, false)).unwrap();
    let mouth: Vec<[u8; 4]> = (0..8)
        .map(|at| {
            if at >= 4 {
                [0, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect();
    std::fs::write(folder.join("mouth.png"), write_png(4, 2, &mouth, false)).unwrap();
    folder
}

fn built(folder: &Path, text: &str) -> Model {
    let mut issues = Vec::new();
    let loaded = load_text(text, Syntax::Toml, "test.toml", folder, &mut issues);
    let model = assemble(&loaded, &mut issues);
    let errors: Vec<&Issue> = issues.iter().filter(|issue| issue.is_error()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    model.expect("a recipe without errors builds")
}

/// Which colour a pixel is, going by which of red, green and blue are
/// strong in it (the light makes them darker, but not another colour).
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
        (true, true, true) => "light",
        (false, false, false) => "black",
        _ => "other",
    }
}

/// The part of a view the model covers: left, top, right and bottom,
/// in the sheet's pixels.
fn covered(sheet: &Sheet, name: &str) -> [usize; 4] {
    let view = sheet.views.iter().find(|view| view.name == name).unwrap();
    let (mut left, mut top, mut right, mut bottom) = (usize::MAX, usize::MAX, 0, 0);
    // The top rows hold the view's name: leave them out.
    for y in view.top + view.size / 8..view.top + view.size {
        for x in view.left..view.left + view.size {
            if sheet.pixel(x, y) != BACKGROUND.to_u32() {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        }
    }
    [left, top, right, bottom]
}

/// The pixel a share of the way across and down what the model covers.
fn pixel_at(sheet: &Sheet, name: &str, across: f64, down: f64) -> u32 {
    let [left, top, right, bottom] = covered(sheet, name);
    sheet.pixel(
        left + ((right - left) as f64 * across).round() as usize,
        top + ((bottom - top) as f64 * down).round() as usize,
    )
}

const CUBE: &str = r##"
version = 0
name = "cube"

[palette]
steel = "#8A939B"

[[parts]]
name = "cube"
shape = "box"
color = "steel"
image = "quad.png"
"##;

#[test]
fn an_image_is_drawn_the_right_way_round_on_every_side() {
    let sheet = render_sheet(&built(&folder("cube"), CUBE), "cube", 128);
    for view in ["front", "right", "back"] {
        let named = |across: f64, down: f64| colour_name(pixel_at(&sheet, view, across, down));
        assert_eq!(named(0.25, 0.25), "red", "{view}");
        assert_eq!(named(0.75, 0.25), "green", "{view}");
        assert_eq!(named(0.25, 0.75), "blue", "{view}");
        assert_eq!(named(0.75, 0.75), "yellow", "{view}");
    }
}

#[test]
fn an_images_pixels_stay_sharp() {
    // Seen straight on, a face shows exactly the image's four colours:
    // no in-between colours where two of its pixels meet.
    let sheet = render_sheet(&built(&folder("sharp"), CUBE), "cube", 256);
    let [left, top, right, bottom] = covered(&sheet, "front");
    let mut seen: Vec<u32> = Vec::new();
    for y in top..=bottom {
        for x in left..=right {
            let pixel = sheet.pixel(x, y);
            if !seen.contains(&pixel) {
                seen.push(pixel);
            }
        }
    }
    assert_eq!(seen.len(), 4, "{seen:x?}");
    // The image is 4 pixels across and the face about 150 on the
    // screen: the red runs right up to the green.
    let middle = (left + right) / 2;
    let row = top + (bottom - top) / 4;
    assert_eq!(colour_name(sheet.pixel(middle - 2, row)), "red");
    assert_eq!(colour_name(sheet.pixel(middle + 3, row)), "green");
}

#[test]
fn the_colour_shows_where_an_image_is_see_through() {
    let model = built(
        &folder("mouth"),
        &CUBE.replace("image = \"quad.png\"", "images = { front = \"mouth.png\" }"),
    );
    let sheet = render_sheet(&model, "cube", 128);
    // The top half of the mouth image is see-through: steel, lit.
    assert_eq!(colour_name(pixel_at(&sheet, "front", 0.5, 0.25)), "light");
    // Its bottom half is black.
    assert_eq!(colour_name(pixel_at(&sheet, "front", 0.5, 0.75)), "black");
    // The back has no image: steel all over.
    assert_eq!(colour_name(pixel_at(&sheet, "back", 0.5, 0.75)), "light");
}

#[test]
fn the_same_model_with_images_always_gives_the_same_picture() {
    let folder = folder("same");
    let first = render_sheet(&built(&folder, CUBE), "cube", 128);
    let second = render_sheet(&built(&folder, CUBE), "cube", 128);
    assert_eq!(first, second);
}
