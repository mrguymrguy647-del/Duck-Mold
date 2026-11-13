//! The start page's pictures of the examples are drawn from the example
//! recipes by the same code as `duck-mold render`, and stored in
//! `assets/examples/`, from where they are put inside the program.
//!
//! This test draws each again and compares it with the stored file, so
//! a picture can't drift from its recipe. After changing an example (or
//! the renderer) on purpose, store the new pictures with
//!
//! ```sh
//! DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-studio --test gallery
//! ```

use std::path::Path;

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
