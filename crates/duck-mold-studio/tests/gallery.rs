//! The start page's pictures of the examples are drawn from the example
//! recipes by the same code as `duck-mold render`, and stored in
//! `assets/examples/`, from where they are put inside the program. The
//! Add menu's pictures of the shapes are drawn the same way, each from a
//! recipe of the shape alone, and stored in `assets/shapes/`.
//!
//! These tests draw each again and compare it with the stored file, so
//! a picture can't drift from its recipe. After changing an example (or
//! the renderer) on purpose, store the new pictures with
//!
//! ```sh
//! DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-studio --test gallery
//! ```

use std::path::Path;

use duck_mold_core::recipe::Shape;
use duck_mold_core::tools;
use duck_mold_studio::gallery;
use duck_mold_view::render::{png_of, render_small};

#[test]
fn every_examples_picture_is_drawn_from_its_recipe() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let accept = std::env::var_os("DUCK_MOLD_ACCEPT").is_some();
    for example in tools::EXAMPLES {
        let recipe = here
            .join("../../examples")
            .join(format!("{}.toml", example.name));
        let built = tools::build(&recipe).unwrap_or_else(|_| panic!("{} builds", example.name));
        let pixels = render_small(&built.model, gallery::PICTURE_SIZE);
        let drawn = png_of(gallery::PICTURE_SIZE, gallery::PICTURE_SIZE, &pixels);
        let stored = here
            .join("assets/examples")
            .join(format!("{}.png", example.name));
        if accept {
            std::fs::write(&stored, &drawn).unwrap();
            continue;
        }
        let kept = std::fs::read(&stored).unwrap_or_default();
        assert!(
            kept == drawn,
            "the picture of the example \"{}\" is no longer what its recipe draws: see the top of this file",
            example.name
        );
        // And it is the one inside the program.
        let inside = gallery::PICTURES
            .iter()
            .find(|(name, _)| *name == example.name)
            .unwrap_or_else(|| panic!("the program has no picture of {}", example.name));
        assert!(inside.1 == kept.as_slice(), "{}", example.name);
    }
    assert_eq!(gallery::PICTURES.len(), tools::EXAMPLES.len());
}

#[test]
fn every_shapes_picture_is_drawn_from_its_recipe() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let accept = std::env::var_os("DUCK_MOLD_ACCEPT").is_some();
    let folder =
        std::env::temp_dir().join(format!("duck_mold_shape_pictures_{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    for shape in Shape::NAMES {
        let recipe = folder.join(format!("{shape}.toml"));
        std::fs::write(&recipe, gallery::shape_recipe(shape)).unwrap();
        let built = tools::build(&recipe).unwrap_or_else(|_| panic!("{shape} builds"));
        assert!(built.warnings.is_empty(), "{shape}: {:?}", built.warnings);
        let pixels = render_small(&built.model, gallery::SHAPE_PICTURE_SIZE);
        let drawn = png_of(
            gallery::SHAPE_PICTURE_SIZE,
            gallery::SHAPE_PICTURE_SIZE,
            &pixels,
        );
        let stored = here.join("assets/shapes").join(format!("{shape}.png"));
        if accept {
            std::fs::write(&stored, &drawn).unwrap();
            continue;
        }
        let kept = std::fs::read(&stored).unwrap_or_default();
        assert!(
            kept == drawn,
            "the picture of the shape \"{shape}\" is no longer what its recipe draws: see the top of this file"
        );
        let inside = gallery::SHAPE_PICTURES
            .iter()
            .find(|(name, _)| *name == shape)
            .unwrap_or_else(|| panic!("the program has no picture of {shape}"));
        assert!(inside.1 == kept.as_slice(), "{shape}");
    }
    assert_eq!(gallery::SHAPE_PICTURES.len(), Shape::NAMES.len());
    let _ = std::fs::remove_dir_all(&folder);
}
