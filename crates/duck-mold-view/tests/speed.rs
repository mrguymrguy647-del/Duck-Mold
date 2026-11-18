//! How fast the Studio is with a big model: 50 parts, about 20,000
//! triangles. Run by hand, in release, and read the times:
//!
//!     cargo test --release -p duck-mold-view --test speed -- --ignored --nocapture
//!
//! Each time is the middle one of several tries.

use std::path::{Path, PathBuf};
use std::time::Instant;

use duck_mold_core::edit::{Changes, Edit, Sizes};
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::Grip;

/// A recipe with 50 parts: a floor and 49 round parts on it, 7 x 7, each
/// a sphere or a cylinder with enough sides to make about 20,000
/// triangles in all, every other one joined to the one before.
pub fn big_recipe() -> String {
    let mut text = String::from(
        "version = 0\nname = \"big\"\ncolliders = \"parts\"\n\n[palette]\nstone = \"#9A9A9A\"\nred = \"#C04040\"\nblue = \"#4060C0\"\n\n[[parts]]\nname = \"floor\"\nshape = \"box\"\nsize = [8, 0.2, 8]\ncolor = \"stone\"\n",
    );
    for row in 0..7 {
        for column in 0..7 {
            let at = row * 7 + column;
            let (x, z) = (column as f64 - 3.0, row as f64 - 3.0);
            if at % 2 == 0 {
                text.push_str(&format!(
                    "\n[[parts]]\nname = \"ball_{at}\"\nshape = \"sphere\"\nradius = 0.35\nsegments = 26\ncolor = \"red\"\nmove = [{x}, 0.45, {z}]\n"
                ));
            } else {
                text.push_str(&format!(
                    "\n[[parts]]\nname = \"post_{at}\"\nshape = \"cylinder\"\nradius = 0.2\nheight = 0.8\nsegments = 64\ncolor = \"blue\"\nmove = [{x}, 0.5, {z}]\n"
                ));
            }
        }
    }
    text
}

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// The middle time of `tries` runs of `work`, in milliseconds.
fn time(tries: usize, mut work: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..tries)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

#[test]
#[ignore = "run by hand, in release, to measure"]
fn a_big_model_in_the_studio() {
    let recipe = folder("speed_big").join("big.toml");
    std::fs::write(&recipe, big_recipe()).unwrap();
    let opened = time(3, || {
        let _ = Studio::open(&recipe, false);
    });
    let mut studio = Studio::open(&recipe, false);
    let model = studio.model().expect("it builds");
    let triangles: usize = model.faces.iter().map(|face| face.corners.len() - 2).sum();
    let parts = model.leaves().len();
    eprintln!("big model: {parts} parts, {triangles} triangles");
    eprintln!("open the file and build it   {opened:7.1} ms");

    // A frame of the viewport, as the window draws it: 780 x 590 at
    // 1280 x 720, 1560 x 1180 at 2560 x 1440. The orbit changes each
    // time so that nothing is kept from the frame before.
    for (width, height) in [(780, 590), (1560, 1180)] {
        let drawn = time(7, || {
            studio.orbit.round += 1.0;
            let _ = studio.picture(width, height);
        });
        eprintln!("draw a frame {width} x {height}     {drawn:7.1} ms");
    }
    studio.shadow = false;
    let no_shadow = time(7, || {
        studio.orbit.round += 1.0;
        let _ = studio.picture(1560, 1180);
    });
    eprintln!("draw 1560 x 1180, no shadow   {no_shadow:6.1} ms");
    studio.shadow = true;
    studio.grid = false;
    let no_grid = time(7, || {
        studio.orbit.round += 1.0;
        let _ = studio.picture(1560, 1180);
    });
    eprintln!("draw 1560 x 1180, no grid     {no_grid:6.1} ms");
    studio.grid = true;
    studio.select(Some("ball_24"));
    let selected = time(7, || {
        studio.orbit.round += 1.0;
        let _ = studio.picture(780, 590);
    });
    eprintln!("draw a frame, a part selected {selected:6.1} ms");

    // One edit from the panel: the recipe written, read and built again.
    let mut radius = 0.35;
    let edited = time(5, || {
        radius += 0.01;
        studio
            .apply(&Edit::Set {
                part: "ball_24".into(),
                changes: Changes {
                    sizes: Sizes {
                        radius: Some(radius),
                        ..Sizes::default()
                    },
                    ..Changes::default()
                },
            })
            .unwrap();
    });
    eprintln!("one edit (panel field)        {edited:6.1} ms");

    // A drag of the Move arrow: each step works out the edit and builds
    // the model it would make, to show.
    let size = (780, 590);
    studio.hover(size, None);
    let _ = studio.picture(size.0, size.1);
    let grip = studio.grip_at(Grip::Arrow(0)).expect("an arrow");
    let at = studio.on_screen(grip).expect("the arrow is in view");
    studio.grab(size, at);
    let mut x = at.0;
    let dragged = time(7, || {
        x += 4.0;
        studio.drag(size, (x, at.1), false);
        let _ = studio.picture(size.0, size.1);
    });
    eprintln!("one step of a drag + frame    {dragged:6.1} ms");
    let worked_out = time(7, || {
        x += 4.0;
        studio.drag(size, (x, at.1), false);
    });
    eprintln!("  of which the edit and model {worked_out:6.1} ms");
    let _ = studio.release();

    // A click that picks a part.
    let clicked = time(7, || {
        let _ = studio.click(size, (390.0, 295.0));
    });
    eprintln!("a click that picks            {clicked:6.1} ms");
}
