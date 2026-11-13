//! The start page's pictures: the examples, which are inside the
//! program, and the models opened lately, which are drawn when the page
//! is shown.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use duck_mold_core::tools;
use duck_mold_view::render::render_small;
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};

/// How many pixels square a stored picture of an example is.
pub const PICTURE_SIZE: usize = 192;

/// The picture of each example, as a PNG file: drawn from its recipe by
/// the same code as `duck-mold render` (a test keeps them so).
pub const PICTURES: [(&str, &[u8]); 15] = [
    ("robot", include_bytes!("../assets/examples/robot.png")),
    ("house", include_bytes!("../assets/examples/house.png")),
    ("snowman", include_bytes!("../assets/examples/snowman.png")),
    ("desk", include_bytes!("../assets/examples/desk.png")),
    ("duck", include_bytes!("../assets/examples/duck.png")),
    ("dog", include_bytes!("../assets/examples/dog.png")),
    (
        "doughnut",
        include_bytes!("../assets/examples/doughnut.png"),
    ),
    ("mug", include_bytes!("../assets/examples/mug.png")),
    ("temple", include_bytes!("../assets/examples/temple.png")),
    ("barn", include_bytes!("../assets/examples/barn.png")),
    ("slide", include_bytes!("../assets/examples/slide.png")),
    ("gate", include_bytes!("../assets/examples/gate.png")),
    ("chair", include_bytes!("../assets/examples/chair.png")),
    (
        "window_wall",
        include_bytes!("../assets/examples/window_wall.png"),
    ),
    ("rock", include_bytes!("../assets/examples/rock.png")),
];

/// How many pixels square a stored picture of a shape is (the Add menu
/// shows them smaller).
pub const SHAPE_PICTURE_SIZE: usize = 64;

/// The Add menu's picture of each shape, as a PNG file: drawn from
/// [`shape_recipe`] by the same code as `duck-mold render` (a test keeps
/// them so).
pub const SHAPE_PICTURES: [(&str, &[u8]); 12] = [
    ("box", include_bytes!("../assets/shapes/box.png")),
    ("cylinder", include_bytes!("../assets/shapes/cylinder.png")),
    ("sphere", include_bytes!("../assets/shapes/sphere.png")),
    ("cone", include_bytes!("../assets/shapes/cone.png")),
    ("wedge", include_bytes!("../assets/shapes/wedge.png")),
    ("capsule", include_bytes!("../assets/shapes/capsule.png")),
    ("torus", include_bytes!("../assets/shapes/torus.png")),
    ("tube", include_bytes!("../assets/shapes/tube.png")),
    ("pyramid", include_bytes!("../assets/shapes/pyramid.png")),
    ("prism", include_bytes!("../assets/shapes/prism.png")),
    ("stairs", include_bytes!("../assets/shapes/stairs.png")),
    ("arch", include_bytes!("../assets/shapes/arch.png")),
];

/// The recipe a shape's picture is drawn from: the shape alone, at the
/// sizes it has when a recipe gives none, in one colour.
pub fn shape_recipe(shape: &str) -> String {
    format!(
        "version = 0\nname = \"{shape}\"\n\n[palette]\nclay = \"#C9A27E\"\n\n[[parts]]\nshape = \"{shape}\"\ncolor = \"clay\"\n"
    )
}

/// A stored picture, ready to draw.
fn texture_of(ctx: &egui::Context, name: String, bytes: &[u8]) -> Option<TextureHandle> {
    let picture = eframe::icon_data::from_png_bytes(bytes).ok()?;
    let image = ColorImage::from_rgba_unmultiplied(
        [picture.width as usize, picture.height as usize],
        &picture.rgba,
    );
    Some(ctx.load_texture(name, image, TextureOptions::LINEAR))
}

/// The Add menu's pictures of the shapes, read the first time each is
/// asked for.
#[derive(Default)]
pub struct ShapePictures {
    textures: HashMap<&'static str, Option<TextureHandle>>,
}

impl ShapePictures {
    /// The picture of a shape, by name.
    pub fn get(&mut self, ctx: &egui::Context, shape: &str) -> Option<&TextureHandle> {
        let (name, bytes) = SHAPE_PICTURES.iter().find(|(known, _)| *known == shape)?;
        self.textures
            .entry(name)
            .or_insert_with(|| texture_of(ctx, format!("duck_mold_shape_{name}"), bytes))
            .as_ref()
    }
}

/// The start page's pictures, as the screen draws them.
#[derive(Default)]
pub struct Gallery {
    examples: HashMap<&'static str, TextureHandle>,
    /// A model opened lately that can't be built has no picture.
    recent: HashMap<PathBuf, Option<TextureHandle>>,
}

impl Gallery {
    /// The picture of an example, read the first time it is asked for.
    pub fn example(&mut self, ctx: &egui::Context, name: &str) -> Option<&TextureHandle> {
        let (name, bytes) = PICTURES.iter().find(|(known, _)| *known == name)?;
        if !self.examples.contains_key(name) {
            let texture = texture_of(ctx, format!("duck_mold_example_{name}"), bytes)?;
            self.examples.insert(name, texture);
        }
        self.examples.get(name)
    }

    /// The picture of a model opened lately, drawn the first time it is
    /// asked for. `None` if the file doesn't build any more.
    pub fn recent(&mut self, ctx: &egui::Context, file: &Path) -> Option<&TextureHandle> {
        if !self.recent.contains_key(file) {
            let texture = tools::build(file).ok().map(|built| {
                let size = PICTURE_SIZE / 2;
                let pixels = render_small(&built.model, size);
                let image = ColorImage::new(
                    [size, size],
                    pixels
                        .iter()
                        .map(|pixel| {
                            Color32::from_rgb((pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8)
                        })
                        .collect(),
                );
                ctx.load_texture(
                    format!("duck_mold_recent_{}", file.display()),
                    image,
                    TextureOptions::LINEAR,
                )
            });
            self.recent.insert(file.to_path_buf(), texture);
        }
        self.recent.get(file)?.as_ref()
    }

    /// Forgets the pictures of the models opened lately: they are drawn
    /// again the next time the start page is shown.
    pub fn forget_recent(&mut self) {
        self.recent.clear();
    }
}
