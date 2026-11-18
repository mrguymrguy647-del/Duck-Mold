//! Images: the PNG pictures a recipe puts on its parts.
//!
//! A [`Picture`] is one image file, read into plain pixels. A [`ImageSheet`]
//! is all of a model's images side by side in one larger picture: what
//! goes inside the `.glb`, and what Duck Forge draws with (it draws one
//! picture for each mesh).
//!
//! Reading never crashes: a file that isn't a PNG, is damaged or is too
//! big gives a sentence saying so.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::model::Model;

/// The most pixels an image can have along one side.
pub const MAX_SIDE: u32 = 1024;

/// The most pixels the sheet of all a model's images can have along one
/// side.
pub const MAX_SHEET: u32 = 4096;

/// The colour of a face nobody coloured, where an image is see-through
/// over it: the plain grey such a face is built in.
pub const NO_COLOR: [u8; 3] = [180, 180, 180];

/// How many times an image may repeat across one face (an imported
/// mesh's bricks): more is shown once, with a warning.
pub const MAX_REPEATS: i64 = 8;

/// One image, as plain pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// The file, as messages and reports name it: `textures/face.png`,
    /// with the folders of the recipe that named it in front, as other
    /// files are shown.
    pub file: String,
    /// How many pixels wide.
    pub width: u32,
    /// How many pixels high.
    pub height: u32,
    /// The pixels, row by row from the top: red, green, blue and how
    /// solid (255 is solid, 0 is see-through).
    pub pixels: Vec<[u8; 4]>,
    /// A number worked out from the size and the pixels: two pictures
    /// with different pixels have (almost surely) different stamps. A
    /// quick way to tell whether a picture changed.
    pub stamp: u64,
}

/// A picture's stamp: a number worked out from its size and pixels.
pub fn stamp_of(width: u32, height: u32, pixels: &[[u8; 4]]) -> u64 {
    let mut stamp: u64 = 0xcbf2_9ce4_8422_2325;
    let mut take = |byte: u8| {
        stamp ^= u64::from(byte);
        stamp = stamp.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for byte in width.to_le_bytes().into_iter().chain(height.to_le_bytes()) {
        take(byte);
    }
    for pixel in pixels {
        for &byte in pixel {
            take(byte);
        }
    }
    stamp
}

impl Picture {
    /// The pixel at `x` across and `y` down.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[(y * self.width + x) as usize]
    }

    /// Whether every pixel is solid: the colour under it never shows.
    pub fn is_solid(&self) -> bool {
        self.pixels.iter().all(|pixel| pixel[3] == 255)
    }
}

/// Why an image couldn't be used: the end of a sentence that starts
/// with the file's name, and the likely fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadPicture {
    /// What is wrong: `isn't a PNG picture`.
    pub what: String,
    /// What to do about it: `Save it as a PNG`.
    pub fix: &'static str,
}

/// Reads a PNG file's bytes into pixels.
///
/// Every kind of PNG is read (grey, with a palette, 16 bits a colour,
/// interlaced); what comes out is always red, green, blue and how
/// solid, 8 bits each.
pub fn read_png(bytes: &[u8], file: &str) -> Result<Picture, BadPicture> {
    read_png_up_to(bytes, file, MAX_SIDE)
}

/// [`read_png`], for a picture that may be up to `most` pixels along a
/// side: the sheet inside a `.glb` can be bigger than one image.
pub fn read_png_up_to(bytes: &[u8], file: &str, most: u32) -> Result<Picture, BadPicture> {
    const PNG_START: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if !bytes.starts_with(&PNG_START) {
        let what = if bytes.starts_with(&[0xFF, 0xD8]) {
            "isn't a PNG picture (it is a JPEG)"
        } else if bytes.starts_with(b"GIF8") {
            "isn't a PNG picture (it is a GIF)"
        } else if bytes.starts_with(b"BM") {
            "isn't a PNG picture (it is a BMP)"
        } else if bytes.is_empty() {
            "is empty"
        } else {
            "isn't a PNG picture"
        };
        return Err(BadPicture {
            what: what.to_owned(),
            fix: "Save it as a PNG",
        });
    }
    let damaged = |why: &str| BadPicture {
        what: format!("is damaged: {why}"),
        fix: "Save it again",
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| damaged(&plain(&error)))?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if width == 0 || height == 0 {
        return Err(damaged("it has no pixels"));
    }
    if width > most || height > most {
        return Err(BadPicture {
            what: format!(
                "is {width} x {height} pixels, and an image can be {most} x {most} at most"
            ),
            fix: "Make it smaller",
        });
    }
    let Some(size) = reader.output_buffer_size() else {
        return Err(damaged("it says it is bigger than it can be"));
    };
    let mut data = vec![0u8; size];
    let frame = reader
        .next_frame(&mut data)
        .map_err(|error| damaged(&plain(&error)))?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(damaged("its colours can't be read")),
    };
    let count = (width as usize) * (height as usize);
    if data.len() < count * channels {
        return Err(damaged("it ends before its picture does"));
    }
    let pixels = data[..count * channels]
        .chunks_exact(channels)
        .map(|pixel| match channels {
            1 => [pixel[0], pixel[0], pixel[0], 255],
            2 => [pixel[0], pixel[0], pixel[0], pixel[1]],
            3 => [pixel[0], pixel[1], pixel[2], 255],
            _ => [pixel[0], pixel[1], pixel[2], pixel[3]],
        })
        .collect::<Vec<[u8; 4]>>();
    Ok(Picture {
        file: file.to_owned(),
        width,
        height,
        stamp: stamp_of(width, height, &pixels),
        pixels,
    })
}

/// The PNG library's reason, in the words these messages use.
fn plain(error: &png::DecodingError) -> String {
    match error {
        png::DecodingError::IoError(_) => "it ends before its picture does".to_owned(),
        png::DecodingError::Format(_) => "what is in it isn't a picture a PNG can hold".to_owned(),
        png::DecodingError::LimitsExceeded => "it is too big to read".to_owned(),
        _ => "it can't be read".to_owned(),
    }
}

/// Reads an image file's bytes, remembering the last picture read from
/// each path: an edit builds the model again, and the same bytes give
/// the same picture without reading them twice.
pub fn read_png_at(
    path: &std::path::Path,
    bytes: Vec<u8>,
    file: &str,
) -> Result<Arc<Picture>, BadPicture> {
    type Seen = BTreeMap<std::path::PathBuf, (Vec<u8>, String, Arc<Picture>)>;
    static SEEN: OnceLock<Mutex<Seen>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(seen) = seen.lock()
        && let Some((before, name, picture)) = seen.get(path)
        && *before == bytes
        && name == file
    {
        return Ok(picture.clone());
    }
    let picture = Arc::new(read_png(&bytes, file)?);
    if let Ok(mut seen) = seen.lock() {
        // A long-running Studio opens many models: keep the list short.
        if seen.len() >= 256 {
            seen.clear();
        }
        seen.insert(
            path.to_path_buf(),
            (bytes, file.to_owned(), picture.clone()),
        );
    }
    Ok(picture)
}

/// Writes pixels as a PNG file's bytes: the same pixels always give
/// the same bytes, on every computer. `solid` leaves out how solid each
/// pixel is (every one is).
pub fn write_png(width: u32, height: u32, pixels: &[[u8; 4]], solid: bool) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(if solid {
            png::ColorType::Rgb
        } else {
            png::ColorType::Rgba
        });
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Balanced);
        let mut writer = encoder
            .write_header()
            .expect("writing to memory can't fail");
        let data: Vec<u8> = if solid {
            pixels
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect()
        } else {
            pixels.iter().flatten().copied().collect()
        };
        writer
            .write_image_data(&data)
            .expect("writing to memory can't fail");
    }
    out
}

/// One image on the sheet: an image, laid over one colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Which of the model's pictures.
    pub picture: usize,
    /// The palette colour it is laid over, where it is see-through.
    /// `None` when the image is solid everywhere (no colour shows), or
    /// when the face has no colour (plain grey shows).
    pub over: Option<usize>,
    /// Where the image's own top-left pixel is on the sheet. (The cell
    /// can reach further left and up, where an image is wrapped round a
    /// shape and a face lies across its two ends.)
    pub x: i64,
    /// See `x`.
    pub y: i64,
}

/// All of a model's images side by side in one picture.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageSheet {
    /// How many pixels wide: a power of two.
    pub width: u32,
    /// How many pixels high: a power of two.
    pub height: u32,
    /// The pixels, row by row from the top. Every one is solid.
    pub pixels: Vec<[u8; 4]>,
    /// The images on it.
    pub cells: Vec<Cell>,
    /// For each face of the model, which of `cells` it shows. `None`
    /// for a face without an image. (For a face of a far-away version,
    /// ask [`ImageSheet::cell`].)
    pub cell_of: Vec<Option<usize>>,
    /// A place on the sheet that is plain white, across and down from 0
    /// to 1: where a `.glb` points the corners of a face without an
    /// image, so that the face shows its own colour.
    pub plain: [f64; 2],
}

/// How big the plain white block on the sheet is, along each side.
const PLAIN: i64 = 2;

/// The pixels kept free round each image on the sheet, filled with the
/// image's own edge, so its neighbour never shows at an edge.
const BORDER: i64 = 2;

/// What a sheet has to hold for one image over one colour.
struct Wanted {
    picture: usize,
    over: Option<usize>,
    /// The pixels of the image that faces show, as columns and rows
    /// counted from the image's own top-left: from, and up to but not
    /// including. Beyond 0 to its size, the image repeats.
    across: (i64, i64),
    down: (i64, i64),
}

/// Where everything goes on a sheet, before any pixel is painted.
struct Plan {
    wanted: Vec<Wanted>,
    cell_of: Vec<Option<usize>>,
    /// Each one's size on the sheet, its border included.
    sizes: Vec<(i64, i64)>,
    /// Each one's top-left corner on the sheet.
    places: Vec<(i64, i64)>,
    width: u32,
    height: u32,
}

/// Works out what a model's sheet has to hold and where each image
/// goes on it. `Ok(None)` for a model without images.
fn plan(model: &Model) -> Result<Option<Plan>, String> {
    let mut wanted: Vec<Wanted> = Vec::new();
    let mut cell_of = Vec::with_capacity(model.faces.len());
    // The model's own faces, then those of its far-away versions, which
    // show the same images (a simpler shape can reach a little further
    // past the ends of a wrapped image).
    let own = model.faces.iter().map(|face| (face, true));
    let far = model
        .far
        .iter()
        .flat_map(|far| far.model.faces.iter().map(|face| (face, false)));
    for (face, is_own) in own.chain(far) {
        let Some(image) = &face.image else {
            if is_own {
                cell_of.push(None);
            }
            continue;
        };
        let picture = &model.pictures[image.picture];
        let over = if picture.is_solid() { None } else { face.color };
        let (width, height) = (f64::from(picture.width), f64::from(picture.height));
        // The pixels this face shows: a hair inside its edges, so a
        // face that ends exactly on a pixel's edge doesn't claim the
        // next pixel.
        let range = |of: fn(&[f64; 2]) -> f64, size: f64, pixels: i64| {
            let low = image.at.iter().map(of).fold(f64::INFINITY, f64::min);
            let high = image.at.iter().map(of).fold(f64::NEG_INFINITY, f64::max);
            let from = ((low * size + 1e-6).floor() as i64).max(-MAX_REPEATS * pixels);
            let to = ((high * size - 1e-6).ceil() as i64).min((MAX_REPEATS + 1) * pixels);
            (from.min(to - 1), to.max(from + 1))
        };
        let across = range(|at| at[0], width, i64::from(picture.width));
        let down = range(|at| at[1], height, i64::from(picture.height));
        let at = match wanted
            .iter()
            .position(|known| known.picture == image.picture && known.over == over)
        {
            Some(at) => {
                let known = &mut wanted[at];
                known.across = (known.across.0.min(across.0), known.across.1.max(across.1));
                known.down = (known.down.0.min(down.0), known.down.1.max(down.1));
                at
            }
            None => {
                wanted.push(Wanted {
                    picture: image.picture,
                    over,
                    across,
                    down,
                });
                wanted.len() - 1
            }
        };
        if is_own {
            cell_of.push(Some(at));
        }
    }
    if wanted.is_empty() {
        return Ok(None);
    }

    let mut sizes: Vec<(i64, i64)> = wanted
        .iter()
        .map(|cell| {
            (
                cell.across.1 - cell.across.0 + 2 * BORDER,
                cell.down.1 - cell.down.0 + 2 * BORDER,
            )
        })
        .collect();
    // Last, the plain white block.
    sizes.push((PLAIN + 2 * BORDER, PLAIN + 2 * BORDER));
    // Tallest first, in rows: the smallest sheet they fit on, trying
    // 16 x 8, 16 x 16, 32 x 16, 32 x 32...
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&at| (-sizes[at].1, -sizes[at].0, at));
    let mut side = 16u32;
    while side <= MAX_SHEET {
        for height in [side / 2, side] {
            if let Some(places) = in_rows(&order, &sizes, i64::from(side), i64::from(height)) {
                return Ok(Some(Plan {
                    wanted,
                    cell_of,
                    sizes,
                    places,
                    width: side,
                    height,
                }));
            }
        }
        side *= 2;
    }
    let mut biggest: Vec<&Arc<Picture>> = wanted
        .iter()
        .map(|cell| &model.pictures[cell.picture])
        .collect();
    biggest.sort_by_key(|picture| std::cmp::Reverse(picture.width * picture.height));
    biggest.dedup_by(|a, b| Arc::ptr_eq(a, b));
    let named: Vec<String> = biggest
        .iter()
        .take(2)
        .map(|picture| {
            format!(
                "\"{}\" ({} x {})",
                picture.file, picture.width, picture.height
            )
        })
        .collect();
    Err(format!(
        "the images of this model don't fit together in {MAX_SHEET} x {MAX_SHEET} pixels, the most a model can have. The biggest: {}. Use smaller or fewer images",
        named.join(" and ")
    ))
}

impl ImageSheet {
    /// How big the sheet of a model's images is, across and down,
    /// without making it. `Ok(None)` for a model without images. `Err`
    /// says, as a sentence, why they don't fit.
    pub fn size(model: &Model) -> Result<Option<(u32, u32)>, String> {
        Ok(plan(model)?.map(|plan| (plan.width, plan.height)))
    }

    /// Makes the sheet of a model's images. `Ok(None)` for a model
    /// without images. `Err` says, as a sentence, why they don't fit.
    pub fn of(model: &Model) -> Result<Option<ImageSheet>, String> {
        let Some(Plan {
            wanted,
            cell_of,
            sizes,
            places,
            width,
            height,
        }) = plan(model)?
        else {
            return Ok(None);
        };
        let mut pixels = vec![[0u8, 0, 0, 255]; (width as usize) * (height as usize)];
        let mut cells = Vec::with_capacity(wanted.len());
        for (at, cell) in wanted.iter().enumerate() {
            let picture = &model.pictures[cell.picture];
            let under = cell
                .over
                .and_then(|color| model.palette.get(color))
                .map_or(NO_COLOR, |color| color.rgb);
            let (left, top) = places[at];
            let (wide, high) = sizes[at];
            for row in 0..high {
                // The row and column of the image, held inside what
                // faces show (the border repeats the edge), then round
                // the image where it repeats.
                let down = (cell.down.0 + row - BORDER).clamp(cell.down.0, cell.down.1 - 1);
                let y = down.rem_euclid(i64::from(picture.height)) as u32;
                for column in 0..wide {
                    let across =
                        (cell.across.0 + column - BORDER).clamp(cell.across.0, cell.across.1 - 1);
                    let x = across.rem_euclid(i64::from(picture.width)) as u32;
                    let place = ((top + row) * i64::from(width) + left + column) as usize;
                    pixels[place] = over(picture.pixel(x, y), under);
                }
            }
            cells.push(Cell {
                picture: cell.picture,
                over: cell.over,
                x: left + BORDER - cell.across.0,
                y: top + BORDER - cell.down.0,
            });
        }
        // The plain white block.
        let (left, top) = places[wanted.len()];
        let (wide, high) = sizes[wanted.len()];
        for row in 0..high {
            for column in 0..wide {
                pixels[((top + row) * i64::from(width) + left + column) as usize] = [255; 4];
            }
        }
        let middle = (BORDER + PLAIN / 2) as f64;
        Ok(Some(ImageSheet {
            width,
            height,
            pixels,
            cells,
            cell_of,
            plain: [
                (left as f64 + middle) / f64::from(width),
                (top as f64 + middle) / f64::from(height),
            ],
        }))
    }

    /// Which of `cells` a face shows: a face of the model the sheet
    /// was made for, or of one of its far-away versions. `None` for a
    /// face without an image.
    pub fn cell(&self, model: &Model, face: &crate::model::Face) -> Option<usize> {
        let image = face.image.as_ref()?;
        let over = if model.pictures.get(image.picture)?.is_solid() {
            None
        } else {
            face.color
        };
        self.cells
            .iter()
            .position(|cell| cell.picture == image.picture && cell.over == over)
    }

    /// Where a point of a face's image is on the sheet, from 0 to 1
    /// across and down: `at` is the point on the image itself.
    pub fn place(&self, model: &Model, cell: usize, at: [f64; 2]) -> [f64; 2] {
        let cell = &self.cells[cell];
        let picture = &model.pictures[cell.picture];
        [
            (cell.x as f64 + at[0] * f64::from(picture.width)) / f64::from(self.width),
            (cell.y as f64 + at[1] * f64::from(picture.height)) / f64::from(self.height),
        ]
    }

    /// The sheet as a PNG file's bytes.
    pub fn png(&self) -> Vec<u8> {
        write_png(self.width, self.height, &self.pixels, true)
    }
}

/// Puts rectangles in rows on a sheet, in `order`. Where each one's
/// top-left corner goes, or `None` if they don't fit.
fn in_rows(
    order: &[usize],
    sizes: &[(i64, i64)],
    width: i64,
    height: i64,
) -> Option<Vec<(i64, i64)>> {
    let mut places = vec![(0, 0); sizes.len()];
    let (mut left, mut top, mut row_height) = (0, 0, 0);
    for &at in order {
        let (wide, high) = sizes[at];
        if wide > width {
            return None;
        }
        if left + wide > width {
            top += row_height;
            left = 0;
            row_height = 0;
        }
        if top + high > height {
            return None;
        }
        places[at] = (left, top);
        left += wide;
        row_height = row_height.max(high);
    }
    Some(places)
}

/// A pixel laid over a colour: the pixel where it is solid, the colour
/// where it is see-through, a mix in between. Always solid.
pub fn over(pixel: [u8; 4], under: [u8; 3]) -> [u8; 4] {
    let solid = u32::from(pixel[3]);
    let mix = |top: u8, bottom: u8| {
        ((u32::from(top) * solid + u32::from(bottom) * (255 - solid) + 127) / 255) as u8
    };
    [
        mix(pixel[0], under[0]),
        mix(pixel[1], under[1]),
        mix(pixel[2], under[2]),
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn four_pixels() -> Vec<[u8; 4]> {
        vec![
            [255, 0, 0, 255],
            [0, 255, 0, 128],
            [0, 0, 255, 0],
            [10, 20, 30, 255],
        ]
    }

    #[test]
    fn a_png_written_is_read_back_the_same() {
        let bytes = write_png(2, 2, &four_pixels(), false);
        let picture = read_png(&bytes, "four.png").unwrap();
        assert_eq!((picture.width, picture.height), (2, 2));
        assert_eq!(picture.pixels, four_pixels());
        assert!(!picture.is_solid());
        assert_eq!(picture.pixel(1, 1), [10, 20, 30, 255]);
    }

    #[test]
    fn a_file_that_is_not_a_png_says_what_it_is() {
        let jpeg = read_png(&[0xFF, 0xD8, 0xFF, 0xE0], "photo.png").unwrap_err();
        assert_eq!(jpeg.what, "isn't a PNG picture (it is a JPEG)");
        assert_eq!(read_png(b"", "empty.png").unwrap_err().what, "is empty");
        assert_eq!(
            read_png(b"hello there", "text.png").unwrap_err().what,
            "isn't a PNG picture"
        );
    }

    #[test]
    fn a_png_cut_off_anywhere_is_a_sentence_and_never_a_crash() {
        let bytes = write_png(2, 2, &four_pixels(), false);
        // The last 12 bytes only say "the end": cut there, the picture
        // is whole. Anywhere before, it is damaged.
        for length in 8..bytes.len() {
            match read_png(&bytes[..length], "cut.png") {
                Err(cut) => assert!(cut.what.starts_with("is damaged"), "{length}: {}", cut.what),
                Ok(picture) => {
                    assert!(length >= bytes.len() - 12, "{length}");
                    assert_eq!(picture.pixels, four_pixels());
                }
            }
        }
        // One wrong byte anywhere: a sentence, or a picture, never a crash.
        for at in 8..bytes.len() {
            let mut wrong = bytes.clone();
            wrong[at] ^= 0x55;
            let _ = read_png(&wrong, "wrong.png");
        }
    }

    #[test]
    fn an_image_that_is_too_big_says_its_size() {
        let wide = MAX_SIDE + 1;
        let bytes = write_png(wide, 1, &vec![[0, 0, 0, 255]; wide as usize], true);
        let bad = read_png(&bytes, "wide.png").unwrap_err();
        assert_eq!(
            bad.what,
            "is 1025 x 1 pixels, and an image can be 1024 x 1024 at most"
        );
        assert_eq!(bad.fix, "Make it smaller");
    }

    #[test]
    fn a_pixel_over_a_colour_mixes_by_how_solid_it_is() {
        assert_eq!(over([255, 0, 0, 255], [1, 2, 3]), [255, 0, 0, 255]);
        assert_eq!(over([255, 0, 0, 0], [1, 2, 3]), [1, 2, 3, 255]);
        assert_eq!(over([255, 255, 255, 128], [0, 0, 0]), [128, 128, 128, 255]);
    }
}
