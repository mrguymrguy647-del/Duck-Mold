//! The Studio's icon is a picture of a duck made in Duck Mold:
//! `examples/duck.toml`, drawn by the same code as `duck-mold render`.
//!
//! This test draws it again and compares it with the stored files, so
//! the icon can't drift from the recipe. After changing the duck (or
//! the renderer) on purpose, store the new icon with
//!
//! ```sh
//! DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-studio --test icon
//! ```
//!
//! - `assets/icon.png`: 256 pixels square, see-through around the duck.
//!   The window's icon.
//! - `assets/icon.ico`: the same at 256, 48, 32 and 16 pixels. Windows
//!   shows it for `DuckMoldStudio.exe` (`build.rs` puts it in).

use std::path::Path;

use duck_mold_core::tools;
use duck_mold_view::render::{BACKGROUND, render_small};

/// How big the duck is drawn before it is cut out and shrunk.
const DRAWN: usize = 1536;

/// The duck, `size` pixels square, as red, green, blue and how solid
/// (0 = see-through), row by row from the top.
///
/// It is drawn big, cut out with a little room around it so that it
/// fills the square, and shrunk: each pixel of the icon is the average
/// of `finer` x `finer` points of the big picture, which smooths its
/// edges. Whole numbers only, so every computer gets the same icon.
fn duck(size: usize, finer: usize) -> Vec<u8> {
    let recipe = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/duck.toml");
    let built = tools::build(&recipe).expect("the duck recipe builds");
    let pixels = render_small(&built.model, DRAWN);
    let background = BACKGROUND.to_u32();

    // The box around the duck in the big picture.
    let (mut left, mut top, mut right, mut bottom) = (DRAWN, DRAWN, 0, 0);
    for y in 0..DRAWN {
        for x in 0..DRAWN {
            if pixels[y * DRAWN + x] != background {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    assert!(left <= right, "nothing was drawn");
    // A square around it, a twelfth bigger than the duck.
    let side = (right - left).max(bottom - top) + 1;
    let side = side + side / 12;
    let start_x = (left + right + 1) as i64 / 2 - side as i64 / 2;
    let start_y = (top + bottom + 1) as i64 / 2 - side as i64 / 2;

    let points = size * finer;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (mut red, mut green, mut blue, mut solid) = (0u32, 0u32, 0u32, 0u32);
            for fine_y in 0..finer {
                for fine_x in 0..finer {
                    let from_x = start_x + ((x * finer + fine_x) * side / points) as i64;
                    let from_y = start_y + ((y * finer + fine_y) * side / points) as i64;
                    let inside =
                        (0..DRAWN as i64).contains(&from_x) && (0..DRAWN as i64).contains(&from_y);
                    if !inside {
                        continue;
                    }
                    let pixel = pixels[from_y as usize * DRAWN + from_x as usize];
                    if pixel != background {
                        red += (pixel >> 16) & 0xff;
                        green += (pixel >> 8) & 0xff;
                        blue += pixel & 0xff;
                        solid += 1;
                    }
                }
            }
            let all = (finer * finer) as u32;
            if solid == 0 {
                rgba.extend([0, 0, 0, 0]);
            } else {
                rgba.extend([
                    (red / solid) as u8,
                    (green / solid) as u8,
                    (blue / solid) as u8,
                    (solid * 255 / all) as u8,
                ]);
            }
        }
    }
    rgba
}

fn png_of(size: usize, rgba: &[u8]) -> Vec<u8> {
    let mut file = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut file, size as u32, size as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(rgba).unwrap();
    }
    file
}

/// One picture of an `.ico` the old way, which Windows wants for the
/// small sizes: rows from the bottom up, blue first, then a row of bits
/// saying which pixels are see-through.
fn bitmap_of(size: usize, rgba: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend(40u32.to_le_bytes()); // the header's own length
    data.extend((size as i32).to_le_bytes());
    data.extend((size as i32 * 2).to_le_bytes()); // picture and bits
    data.extend(1u16.to_le_bytes());
    data.extend(32u16.to_le_bytes()); // bits for each pixel
    data.extend([0u8; 24]);
    for y in (0..size).rev() {
        for x in 0..size {
            let at = (y * size + x) * 4;
            data.extend([rgba[at + 2], rgba[at + 1], rgba[at], rgba[at + 3]]);
        }
    }
    let row_bytes = size.div_ceil(32) * 4;
    for y in (0..size).rev() {
        let mut row = vec![0u8; row_bytes];
        for x in 0..size {
            if rgba[(y * size + x) * 4 + 3] == 0 {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        data.extend(row);
    }
    data
}

fn ico_of(pictures: &[(usize, Vec<u8>)]) -> Vec<u8> {
    let mut file = Vec::new();
    file.extend(0u16.to_le_bytes());
    file.extend(1u16.to_le_bytes()); // an icon
    file.extend((pictures.len() as u16).to_le_bytes());
    let mut at = 6 + 16 * pictures.len();
    for (size, data) in pictures {
        // 256 is written as 0: the size is one byte.
        let side = if *size >= 256 { 0 } else { *size as u8 };
        file.extend([side, side, 0, 0]);
        file.extend(1u16.to_le_bytes());
        file.extend(32u16.to_le_bytes());
        file.extend((data.len() as u32).to_le_bytes());
        file.extend((at as u32).to_le_bytes());
        at += data.len();
    }
    for (_, data) in pictures {
        file.extend(data);
    }
    file
}

fn check(name: &str, found: &[u8]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name);
    if std::env::var_os("DUCK_MOLD_ACCEPT").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, found).unwrap();
        return;
    }
    let stored = std::fs::read(&path).unwrap_or_default();
    assert!(
        stored == found,
        "{name} isn't the picture of examples/duck.toml any more. If the duck was changed on purpose, store the new icon with: DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-studio --test icon"
    );
}

#[test]
fn the_icon_is_the_duck_recipe_drawn() {
    let big = duck(256, 4);
    // The duck is in it, with see-through corners.
    assert_eq!(big[3], 0, "the top left corner is see-through");
    let solid = big.chunks(4).filter(|pixel| pixel[3] == 255).count();
    assert!(
        solid > 256 * 256 / 6,
        "{solid} solid pixels: where is the duck?"
    );
    let yellow = big
        .chunks(4)
        // Yellow in light or in shade: much more red and green than blue.
        .filter(|pixel| pixel[3] == 255 && pixel[0] > pixel[2] + 60 && pixel[1] > pixel[2] + 40)
        .count();
    assert!(yellow > solid / 2, "the duck is mostly duck yellow");

    check("icon.png", &png_of(256, &big));
    let ico = ico_of(&[
        (256, png_of(256, &big)),
        (48, bitmap_of(48, &duck(48, 6))),
        (32, bitmap_of(32, &duck(32, 8))),
        (16, bitmap_of(16, &duck(16, 12))),
    ]);
    check("icon.ico", &ico);
}
