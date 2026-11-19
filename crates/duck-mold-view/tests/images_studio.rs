//! Phase 7b, step 3: images in the Studio, with no window.
//!
//! The Paint tool with an image in hand, a part's images set and taken
//! off, a new image made, a PNG from elsewhere copied in, and an image
//! saved by another program showing at once. Every change to the recipe
//! is the same `set` edit `duck-mold edit` makes.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use duck_mold_core::math::{Vec3, vec3};
use duck_mold_core::picture::{read_png, write_png};
use duck_mold_view::studio::Studio;
use duck_mold_view::studio::handles::Tool;
use duck_mold_view::studio::images::Brush;

const VIEW: (usize, usize) = (640, 480);

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// The plank crate, with its images, in a folder of its own.
fn plank_crate(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("images_studio")
        .join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("textures")).unwrap();
    for file in [
        "plank_crate.toml",
        "textures/planks.png",
        "textures/lid.png",
    ] {
        std::fs::copy(examples().join(file), folder.join(file)).unwrap();
    }
    folder.join("plank_crate.toml")
}

/// Writes a file as another program saves it: new bytes, and a time
/// later than before, so the change shows even where the clock is
/// coarse.
fn save(path: &Path, bytes: &[u8]) {
    let before = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::now());
    std::fs::write(path, bytes).unwrap();
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(before + Duration::from_secs(2)).unwrap();
}

fn at_screen(studio: &Studio, point: Vec3) -> (f64, f64) {
    studio
        .orbit
        .on_screen(point + studio.offset(), VIEW.0, VIEW.1)
        .unwrap()
}

/// The middle of the crate's front, and of its top, on the screen.
fn front(studio: &Studio) -> (f64, f64) {
    at_screen(studio, vec3(0.0, 0.5, 0.5))
}
fn top(studio: &Studio) -> (f64, f64) {
    at_screen(studio, vec3(0.0, 1.0, 0.0))
}

fn recipe_line(recipe: &Path, starts: &str) -> Option<String> {
    std::fs::read_to_string(recipe)
        .unwrap()
        .lines()
        .find(|line| line.starts_with(starts))
        .map(str::to_owned)
}

/// The image on a face of the crate, as the model has it.
fn image_on(studio: &Studio, face: &str) -> Option<String> {
    let model = studio.model().unwrap();
    let part = model.part("box").unwrap();
    let number = part.faces_named(face)[0];
    model.faces[number].image.as_ref().map(|image| {
        model.pictures[image.picture]
            .file
            .rsplit('/')
            .next()
            .unwrap()
            .to_owned()
    })
}

#[test]
fn the_studio_lists_a_models_images_as_the_recipe_names_them() {
    let recipe = plank_crate("lists");
    let studio = Studio::open(&recipe, false);
    let images = studio.images();
    let files: Vec<&str> = images.iter().map(|image| image.file.as_str()).collect();
    assert_eq!(files, ["textures/planks.png", "textures/lid.png"]);
    assert_eq!(images[0].name(), "planks.png");
    assert_eq!(
        (images[0].picture.width, images[0].picture.height),
        (32, 32)
    );
    assert_eq!(
        images[1].path,
        recipe.parent().unwrap().join("textures/lid.png")
    );
    // The image files are watched, like the recipe.
    let watched = studio.watched();
    assert!(
        watched
            .iter()
            .any(|path| path.ends_with("textures/planks.png"))
    );
    assert!(
        watched
            .iter()
            .any(|path| path.ends_with("textures/lid.png"))
    );
}

#[test]
fn the_paint_tool_puts_the_image_in_hand_on_the_face_clicked() {
    let recipe = plank_crate("paint");
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    // A colour is what it paints with until an image is picked.
    assert_eq!(studio.brush(), Some(Brush::Colour("wood".to_owned())));
    studio.pick_image(Some("textures/lid.png"));
    assert_eq!(studio.tool, Tool::Paint);
    assert_eq!(
        studio.brush(),
        Some(Brush::Image("textures/lid.png".to_owned()))
    );
    assert!(
        studio.status().starts_with("Painting with lid.png"),
        "{}",
        studio.status()
    );

    // Over the front: what a click would do, then the click.
    let at = front(&studio);
    studio.hover(VIEW, Some(at));
    assert_eq!(
        studio.paint_hint(false).as_deref(),
        Some("front of box: put lid.png on it")
    );
    assert_eq!(
        studio.paint_hint(true).as_deref(),
        Some("all of box: put lid.png on it")
    );
    let said = studio.paint_at(VIEW, at, false).unwrap();
    assert!(said.contains("front image textures/lid.png"), "{said}");
    assert_eq!(image_on(&studio, "front").as_deref(), Some("lid.png"));
    assert_eq!(image_on(&studio, "back").as_deref(), Some("planks.png"));
    assert_eq!(
        recipe_line(&recipe, "images = ").unwrap(),
        "images = { top = \"textures/lid.png\", bottom = \"textures/lid.png\", front = \"textures/lid.png\" }"
    );
    // One edit, undone as any other.
    studio.undo().unwrap();
    assert_eq!(image_on(&studio, "front").as_deref(), Some("planks.png"));
}

#[test]
fn no_image_takes_a_faces_image_off_whatever_the_part_has() {
    let recipe = plank_crate("no_image");
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.pick_image(None);
    assert_eq!(studio.brush(), Some(Brush::NoImage));
    // The front shows the part's own planks: "none" is written to keep
    // them off it.
    let at = front(&studio);
    studio.hover(VIEW, Some(at));
    assert_eq!(
        studio.paint_hint(false).as_deref(),
        Some("front of box: take its image off")
    );
    studio.paint_at(VIEW, at, false).unwrap();
    assert_eq!(image_on(&studio, "front"), None);
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("front = \"none\"")
    );
    // The top has an image of its own, the lid, and the part has planks:
    // its line becomes "none" too, so the planks don't show there.
    let at = top(&studio);
    studio.paint_at(VIEW, at, false).unwrap();
    assert_eq!(image_on(&studio, "top"), None);
    assert!(
        recipe_line(&recipe, "images = ")
            .unwrap()
            .contains("top = \"none\"")
    );
    // The colour is what is left.
    let model = studio.model().unwrap();
    let face = model.part("box").unwrap().faces_named("top")[0];
    assert_eq!(model.faces[face].color, Some(0));
}

#[test]
fn shift_click_puts_one_image_on_the_whole_part() {
    let recipe = plank_crate("whole");
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.pick_image(Some("textures/lid.png"));
    let at = front(&studio);
    studio.paint_at(VIEW, at, true).unwrap();
    for face in ["top", "bottom", "left", "right", "front", "back"] {
        assert_eq!(
            image_on(&studio, face).as_deref(),
            Some("lid.png"),
            "{face}"
        );
    }
    assert_eq!(
        recipe_line(&recipe, "image = ").unwrap(),
        "image = \"textures/lid.png\""
    );
    // The single faces' own images went: the part shows one image.
    assert_eq!(recipe_line(&recipe, "images = "), None);

    // "No image" on the whole part takes it off.
    studio.pick_image(None);
    studio.paint_at(VIEW, at, true).unwrap();
    assert!(studio.model().unwrap().pictures.is_empty());
    assert_eq!(recipe_line(&recipe, "image = "), None);
}

#[test]
fn picking_a_colour_puts_the_image_down_and_paints_under_images() {
    let recipe = plank_crate("colour");
    let mut studio = Studio::open(&recipe, false);
    studio.hover(VIEW, None);
    studio.add_colour("red", [0xD0, 0x45, 0x3A]).unwrap();
    studio.pick_image(Some("textures/lid.png"));
    studio.pick_colour(Some("red"));
    assert_eq!(studio.brush(), Some(Brush::Colour("red".to_owned())));
    let at = front(&studio);
    studio.hover(VIEW, Some(at));
    assert_eq!(
        studio.paint_hint(false).as_deref(),
        Some("front of box: paint it red")
    );
    studio.paint_at(VIEW, at, false).unwrap();
    // The face is red underneath, and still has its planks on top.
    let model = studio.model().unwrap();
    let face = model.part("box").unwrap().faces_named("front")[0];
    assert_eq!(model.palette[model.faces[face].color.unwrap()].name, "red");
    assert_eq!(image_on(&studio, "front").as_deref(), Some("planks.png"));
}

#[test]
fn a_new_image_is_made_see_through_with_the_models_pixel_size() {
    let recipe = plank_crate("new");
    let mut studio = Studio::open(&recipe, false);
    // For the front of the crate: a metre square, at the 32 pixels to
    // the metre the planks and the lid have.
    let plan = studio.plan_image("box", Some("front")).unwrap();
    assert_eq!(plan.file, "textures/plank_crate_box_front.png");
    assert_eq!(plan.metres, [1.0, 1.0]);
    assert_eq!(plan.size, [32, 32]);
    assert_eq!(plan.like_the_others, Some(32.0));
    assert_eq!(plan.with_long_side(64), [64, 64]);
    assert!(!plan.path.exists(), "planning writes nothing");

    let said = studio.new_image(&plan, plan.size).unwrap();
    assert_eq!(
        said,
        "Made textures/plank_crate_box_front.png (32 x 32 pixels, see-through) and put it on the face \"front\" of \"box\""
    );
    let picture = read_png(&std::fs::read(&plan.path).unwrap(), "new").unwrap();
    assert_eq!((picture.width, picture.height), (32, 32));
    assert!(picture.pixels.iter().all(|pixel| pixel[3] == 0));
    assert_eq!(
        image_on(&studio, "front").as_deref(),
        Some("plank_crate_box_front.png")
    );
    assert_eq!(studio.last_written(), Some(plan.path.as_path()));
    // Undo takes the line out of the recipe; the file stays.
    studio.undo().unwrap();
    assert_eq!(image_on(&studio, "front").as_deref(), Some("planks.png"));
    assert!(plan.path.exists());

    // A name that is taken isn't written over: the next plan has
    // another.
    let again = studio.plan_image("box", Some("front")).unwrap();
    assert_eq!(again.file, "textures/plank_crate_box_front_2.png");
    assert_eq!(
        studio.new_image(&plan, plan.size).unwrap_err(),
        "textures/plank_crate_box_front.png is already there"
    );
    // A face that isn't there is said, and nothing is written.
    let wrong = studio.plan_image("box", Some("frnt")).unwrap_err();
    assert!(wrong.contains("no face called \"frnt\""), "{wrong}");
}

#[test]
fn a_png_from_elsewhere_is_copied_beside_the_recipe() {
    let recipe = plank_crate("use");
    let folder = recipe.parent().unwrap().to_path_buf();
    let elsewhere = folder.parent().unwrap().join("use_elsewhere");
    let _ = std::fs::remove_dir_all(&elsewhere);
    std::fs::create_dir_all(&elsewhere).unwrap();
    let label = elsewhere.join("label.png");
    std::fs::write(&label, write_png(8, 4, &[[200, 30, 30, 255]; 32], false)).unwrap();

    let mut studio = Studio::open(&recipe, false);
    let said = studio.use_image("box", Some("front"), &label).unwrap();
    assert_eq!(
        said,
        "Copied label.png to textures/label.png, beside the recipe, and put it on front of box"
    );
    assert!(folder.join("textures/label.png").is_file());
    assert_eq!(image_on(&studio, "front").as_deref(), Some("label.png"));

    // The same picture again is used where it already is.
    studio.use_image("box", Some("back"), &label).unwrap();
    assert_eq!(image_on(&studio, "back").as_deref(), Some("label.png"));
    assert!(!folder.join("textures/label_2.png").exists());

    // Another picture with the same name gets a name of its own.
    std::fs::write(&label, write_png(8, 4, &[[30, 30, 200, 255]; 32], false)).unwrap();
    studio.use_image("box", Some("left"), &label).unwrap();
    assert!(folder.join("textures/label_2.png").is_file());
    assert_eq!(image_on(&studio, "left").as_deref(), Some("label_2.png"));

    // A PNG already inside the recipe's folder is named where it is.
    let said = studio
        .use_image("box", Some("right"), &folder.join("textures/lid.png"))
        .unwrap();
    assert!(said.contains("right image textures/lid.png"), "{said}");

    // A file that isn't a PNG is said, and nothing is copied or changed.
    let photo = elsewhere.join("photo.png");
    std::fs::write(&photo, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
    let before = std::fs::read_to_string(&recipe).unwrap();
    assert_eq!(
        studio.use_image("box", None, &photo).unwrap_err(),
        "photo.png isn't a PNG picture (it is a JPEG). Save it as a PNG"
    );
    assert!(!folder.join("textures/photo.png").exists());
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);
}

#[test]
fn an_image_saved_by_another_program_shows_at_once_and_is_no_step_for_undo() {
    let recipe = plank_crate("saved");
    let planks = recipe.parent().unwrap().join("textures/planks.png");
    let mut studio = Studio::open(&recipe, false);
    let builds = studio.builds();
    let before = studio.images()[0].picture.stamp;
    assert!(!studio.check_files(), "nothing has changed yet");

    // Another program saves the planks, all red.
    save(
        &planks,
        &write_png(32, 32, &[[255, 0, 0, 255]; 1024], false),
    );
    assert!(studio.check_files(), "the model is built again");
    assert_eq!(studio.builds(), builds + 1);
    let now = &studio.images()[0];
    assert_ne!(now.picture.stamp, before);
    assert_eq!(now.picture.pixel(3, 3), [255, 0, 0, 255]);
    assert!(!studio.has_mistakes());
    // The recipe didn't change: nothing to undo, and no wall.
    assert_eq!(studio.can_undo(), None);
    assert_eq!(studio.undo_stops(), None);
}

#[test]
fn an_image_half_saved_is_not_a_mistake_until_it_stays_that_way() {
    let recipe = plank_crate("half");
    let planks = recipe.parent().unwrap().join("textures/planks.png");
    let whole = std::fs::read(&planks).unwrap();
    let mut studio = Studio::open(&recipe, false);

    // A picture program has written the first half.
    save(&planks, &whole[..whole.len() / 2]);
    assert!(!studio.check_files(), "looked at again next time");
    assert!(!studio.has_mistakes());
    // It finishes: the image is whole again, and nothing was ever wrong.
    save(&planks, &whole);
    assert!(studio.check_files());
    assert!(!studio.has_mistakes());

    // A file that stays damaged is a mistake, said plainly; the model
    // stays on the screen.
    save(&planks, &whole[..whole.len() / 2]);
    assert!(!studio.check_files());
    assert!(studio.check_files());
    assert!(studio.has_mistakes());
    let problem = &studio.problems()[0].text;
    assert!(
        problem.contains("\"textures/planks.png\" is damaged") && problem.contains("Save it again"),
        "{problem}"
    );
    assert!(
        studio.model().is_some(),
        "the last model that built is still shown"
    );
    // Deleted: said too.
    std::fs::remove_file(&planks).unwrap();
    assert!(!studio.check_files());
    assert!(studio.check_files());
    assert!(
        studio.problems()[0]
            .text
            .contains("there is no file \"textures/planks.png\" beside plank_crate.toml"),
        "{}",
        studio.problems()[0].text
    );
    // Put back: it builds again.
    save(&planks, &whole);
    assert!(studio.check_files());
    assert!(!studio.has_mistakes());
}

#[test]
fn a_model_being_looked_at_changes_nothing() {
    let recipe = plank_crate("look");
    let mut studio = Studio::open(&recipe, true);
    studio.hover(VIEW, None);
    studio.pick_image(None);
    assert_eq!(studio.paint_hint(false), None);
    let before = std::fs::read_to_string(&recipe).unwrap();
    let plan = studio.plan_image("box", None).unwrap();
    assert!(studio.new_image(&plan, plan.size).is_err());
    assert!(!plan.path.exists());
    assert_eq!(std::fs::read_to_string(&recipe).unwrap(), before);
    assert_eq!(
        studio.images().len(),
        2,
        "they are still listed, to look at"
    );
}

// ---- the round trip with a picture program (step 4) ------------------------

mod round_trip {
    use super::*;
    use duck_mold_view::quill::{PaintJob, With};

    /// The robot with an empty image on the front of its head, in a
    /// folder of its own.
    fn robot(test: &str) -> PathBuf {
        let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("round_trip")
            .join(test);
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let recipe = folder.join("robot.toml");
        std::fs::copy(examples().join("robot.toml"), &recipe).unwrap();
        duck_mold_core::images::new_image(&recipe, "head", Some("front"), None, None).unwrap();
        recipe
    }

    /// The pixels of a PNG, and its size.
    fn pixels_of(png: &[u8]) -> (u32, u32, Vec<[u8; 4]>) {
        let picture = duck_mold_core::picture::read_png_up_to(png, "guide", 4096).unwrap();
        (picture.width, picture.height, picture.pixels)
    }

    #[test]
    fn the_guide_outlines_the_face_and_where_parts_are_joined_on() {
        let recipe = robot("guide");
        let studio = Studio::open(&recipe, false);
        let shown = studio.image_shown("head", Some("front")).unwrap();
        assert_eq!(shown.image.file, "textures/robot_head_front.png");
        assert_eq!(
            (shown.image.picture.width, shown.image.picture.height),
            (32, 24)
        );
        assert_eq!(shown.faces.len(), 1);
        // The head is steel: that is what shows under the image.
        assert_eq!(shown.under, [0x8A, 0x93, 0x9B]);

        let (wide, high, pixels) = pixels_of(&studio.guide("head", Some("front")).unwrap());
        // Sixteen times the image's size: thin lines, readable words.
        assert_eq!((wide, high), (512, 384));
        let drawn = |x: u32, y: u32| pixels[(y * wide + x) as usize][3] == 255;
        // The face's outline runs round the edge.
        for (x, y) in [(0, 0), (256, 0), (511, 192), (256, 383), (0, 192)] {
            assert!(drawn(x, y), "{x},{y}");
        }
        // Mostly see-through: lines, not a picture.
        let solid = pixels.iter().filter(|pixel| pixel[3] == 255).count();
        assert!(solid > 1500 && solid < pixels.len() / 10, "{solid}");
        assert!(pixels.iter().all(|pixel| pixel[3] == 0 || pixel[3] == 255));
        // The middle of the face is clear.
        assert!(!drawn(256, 300));
        // The two eyes are joined on the upper half: a dotted box each,
        // one on each side of the middle, and nothing like it below.
        let dots = |from: u32, to: u32, top: u32, bottom: u32| {
            (top..bottom)
                .flat_map(|y| (from..to).map(move |x| (x, y)))
                .filter(|&(x, y)| drawn(x, y))
                .count()
        };
        assert!(dots(40, 216, 60, 260) > 100, "the eye on one side");
        assert!(dots(296, 472, 60, 260) > 100, "the eye on the other");
        assert_eq!(dots(40, 472, 270, 370), 0, "nothing joined below the eyes");

        // A part without an image has no guide.
        assert!(studio.guide("body", None).is_none());
        assert!(studio.image_shown("body", None).is_none());
    }

    #[test]
    fn the_job_names_the_image_its_guide_and_the_colour_under_it() {
        let recipe = robot("job");
        let folder = recipe.parent().unwrap().to_path_buf();
        let studio = Studio::open(&recipe, false);
        let guides = folder.join("guides");
        let quill = folder.join("DuckQuill");
        let job = studio
            .paint_job(
                "head",
                Some("front"),
                With::DuckQuill(quill.clone()),
                &guides,
            )
            .unwrap();
        assert_eq!(job.image, folder.join("textures/robot_head_front.png"));
        assert_eq!(job.paper, [0x8A, 0x93, 0x9B]);
        // The guide is written in Duck Mold's own folder, never beside
        // the recipe.
        let guide = job.guide.clone().unwrap();
        assert_eq!(guide, guides.join("robot_head_front.guide.png"));
        assert!(guide.is_file());
        let words = job.words();
        assert_eq!(words[0], quill.display().to_string());
        assert_eq!(
            words[2..],
            [
                "--guide".to_owned(),
                guide.display().to_string(),
                "--paper".to_owned(),
                "#8A939B".to_owned()
            ]
        );
        assert!(!folder.join("textures/robot_head_front.guide.png").exists());

        // Another picture program gets the image alone.
        let plain = studio
            .paint_job("head", Some("front"), With::System, &guides)
            .unwrap();
        assert_eq!(plain.guide, None);
        // A part without an image says so.
        assert_eq!(
            studio
                .paint_job("body", None, With::System, &guides)
                .unwrap_err(),
            "body has no image yet: make one first"
        );
    }

    /// A program that stands in for Duck Quill: started with the image
    /// as its first word, it saves a painted picture over it, as Duck
    /// Quill does on Ctrl+S.
    fn stand_in(folder: &Path, painted: &[u8]) -> PathBuf {
        std::fs::write(folder.join("painted.png"), painted).unwrap();
        if cfg!(windows) {
            let program = folder.join("stand-in.cmd");
            std::fs::write(
                &program,
                "@echo off\r\ncopy /Y \"%~dp0painted.png\" \"%~1\" >nul\r\n",
            )
            .unwrap();
            program
        } else {
            let program = folder.join("stand-in.sh");
            std::fs::write(
                &program,
                "#!/bin/sh\ncp \"$(dirname \"$0\")/painted.png\" \"$1\"\n",
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            program
        }
    }

    #[test]
    fn a_save_in_the_picture_program_shows_on_the_model_at_once() {
        let recipe = robot("save");
        let folder = recipe.parent().unwrap().to_path_buf();
        let mut studio = Studio::open(&recipe, false);
        let image = folder.join("textures/robot_head_front.png");
        // The new image is see-through all over: the head's front is
        // plain steel.
        assert!(
            studio.images()[0]
                .picture
                .pixels
                .iter()
                .all(|pixel| pixel[3] == 0)
        );

        // What the stand-in will "paint": a red mouth across the bottom
        // rows, the rest left see-through.
        let mut painted = vec![[0u8, 0, 0, 0]; 32 * 24];
        for y in 16..19 {
            for x in 8..24 {
                painted[y * 32 + x] = [200, 30, 30, 255];
            }
        }
        let program = stand_in(&folder, &write_png(32, 24, &painted, false));
        let job: PaintJob = studio
            .paint_job(
                "head",
                Some("front"),
                With::DuckQuill(program),
                &folder.join("guides"),
            )
            .unwrap();
        let said = job.start().unwrap();
        assert!(
            said.starts_with("Opened robot_head_front.png in Duck Quill"),
            "{said}"
        );

        // The Studio looks at its files twice a second. Within a few
        // looks, the model has the mouth.
        let started = std::time::Instant::now();
        let mut built = false;
        while started.elapsed() < Duration::from_secs(20) {
            if studio.check_files() {
                built = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(built, "the saved image was never noticed");
        assert!(
            !studio.has_mistakes(),
            "{:?}",
            studio.problems().first().map(|p| &p.text)
        );
        let picture = &studio.images()[0].picture;
        assert_eq!(picture.pixel(12, 17), [200, 30, 30, 255]);
        assert_eq!(picture.pixel(2, 2)[3], 0);
        assert_eq!(
            std::fs::read(&image).unwrap(),
            std::fs::read(folder.join("painted.png")).unwrap()
        );
        // The recipe wasn't touched, so Undo has nothing new, and the
        // model's face shows the image.
        assert_eq!(studio.undo_stops(), None);
        let model = studio.model().unwrap();
        let front = model.part("head").unwrap().faces_named("front")[0];
        assert!(model.faces[front].image.is_some());
        // What the picture of the model shows there now: the mouth's
        // red, over the head's steel.
        let sheet = duck_mold_core::picture::ImageSheet::of(model)
            .unwrap()
            .unwrap();
        let cell = sheet.cell(model, &model.faces[front]).unwrap();
        let at = |across: f64, down: f64| {
            let [x, y] = sheet.place(model, cell, [across, down]);
            sheet.pixels[((y * f64::from(sheet.height)) as u32 * sheet.width
                + (x * f64::from(sheet.width)) as u32) as usize]
        };
        assert_eq!(at(0.5, 17.5 / 24.0), [200, 30, 30, 255]);
        assert_eq!(at(0.1, 0.1), [0x8A, 0x93, 0x9B, 255]);
    }

    #[test]
    fn a_program_that_isnt_there_is_said_in_a_sentence() {
        let recipe = robot("missing_program");
        let folder = recipe.parent().unwrap().to_path_buf();
        let studio = Studio::open(&recipe, false);
        let job = studio
            .paint_job(
                "head",
                Some("front"),
                With::DuckQuill(folder.join("nowhere").join("DuckQuill")),
                &folder.join("guides"),
            )
            .unwrap();
        let why = job.start().unwrap_err();
        assert!(why.starts_with("Couldn't start Duck Quill ("), "{why}");
    }
}

// ---- step 5: a part's file brings its own pictures ---------------------------------

/// The signpost, with the sign's file and its picture, in a folder of
/// its own.
fn signpost(test: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("images_studio")
        .join(test);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("parts")).unwrap();
    for file in ["signpost.toml", "parts/sign.gltf", "parts/sign_post.png"] {
        std::fs::copy(examples().join(file), folder.join(file)).unwrap();
    }
    folder.join("signpost.toml")
}

#[test]
fn a_parts_own_pictures_are_listed_and_the_recipe_has_the_last_word() {
    let recipe = signpost("own_pictures");
    let mut studio = Studio::open(&recipe, false);
    studio.select(Some("sign"));
    let facts = studio.facts().unwrap();
    let own = studio.own_images("sign", &facts);
    let listed: Vec<(&str, u32, u32)> = own
        .iter()
        .map(|image| {
            (
                image.face.as_str(),
                image.picture.width,
                image.picture.height,
            )
        })
        .collect();
    assert_eq!(listed, [("post", 8, 16), ("board", 64, 32), ("cap", 16, 8)]);
    // Only a PNG on the disk can be picked to paint with: the post's is
    // one, the two inside the file aren't.
    let brushes: Vec<String> = studio
        .images()
        .into_iter()
        .map(|image| image.file)
        .collect();
    assert_eq!(brushes, ["parts/sign_post.png"]);
    // A part that isn't from a mesh file has none.
    studio.select(Some("mound"));
    let mound = studio.facts().unwrap();
    assert!(studio.own_images("mound", &mound).is_empty());

    // Taking one off writes "none", and it is no longer the file's.
    studio.set_image("sign", Some("board"), None).unwrap();
    let text = std::fs::read_to_string(&recipe).unwrap();
    assert!(text.contains("images = { board = \"none\" }"), "{text}");
    studio.select(Some("sign"));
    let facts = studio.facts().unwrap();
    assert_eq!(facts.face_images, [("board".to_owned(), "none".to_owned())]);
    let own: Vec<String> = studio
        .own_images("sign", &facts)
        .into_iter()
        .map(|image| image.face)
        .collect();
    assert_eq!(own, ["post", "cap"]);

    // "own" shows it again, and Undo goes back through both.
    studio
        .set_image("sign", Some("board"), Some("own"))
        .unwrap();
    let facts = studio.facts().unwrap();
    assert_eq!(studio.own_images("sign", &facts).len(), 3);
    studio.undo().unwrap();
    studio.undo().unwrap();
    assert_eq!(
        std::fs::read_to_string(&recipe).unwrap(),
        std::fs::read_to_string(examples().join("signpost.toml")).unwrap()
    );

    // The post's own picture, put on the board by the Paint tool: the
    // file's layout carries it.
    studio.pick_image(Some("parts/sign_post.png"));
    assert_eq!(
        studio.brush(),
        Some(Brush::Image("parts/sign_post.png".into()))
    );
    studio
        .set_image("sign", Some("board"), Some("parts/sign_post.png"))
        .unwrap();
    let model = studio.model().unwrap();
    let names: Vec<&str> = model
        .pictures
        .iter()
        .map(|picture| picture.file.as_str())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names[0].ends_with("parts/sign_post.png") && names[1].ends_with("sign.gltf: cap"));

    // The picture beside the .gltf is watched like any file the recipe
    // names: saved by another program, it shows at once.
    let post = recipe.parent().unwrap().join("parts/sign_post.png");
    let red = write_png(8, 16, &[[200, 30, 30, 255]; 128], true);
    save(&post, &red);
    assert!(studio.check_files());
    assert_eq!(
        studio.model().unwrap().pictures[0].pixel(0, 0),
        [200, 30, 30, 255]
    );
}
