//! `duck-mold`: build and inspect 3D models from text recipes.
//!
//! This file only reads the command line and prints. All the work is done
//! by the libraries: `duck-mold-core` ([`duck_mold_core::tools`]) and, for
//! pictures, `duck-mold-view` ([`duck_mold_view::render`]). The Studio is
//! a program of its own, which `studio` and `preview` start.
//!
//! Exit codes: 0 it worked (warnings included), 1 the recipe has errors,
//! 2 the command line was wrong or a file couldn't be read or written.
//!
//! Reports go to standard output; problems written for people go to
//! standard error. With `--json`, standard output holds exactly one JSON
//! document, also when it failed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use duck_mold_core::issue::{Issue, Report};
use duck_mold_core::tools::{self, Failure};
use duck_mold_view::render;

use duck_mold_cli::{check, edit, guide, mcp};

mod studio;

/// Duck Mold makes game-ready 3D models for Duck Forge from text recipes.
///
/// A recipe is a TOML or JSON file that lists a palette and the parts of
/// a model. Parts are joined by named sockets, not by coordinates.
#[derive(Parser)]
#[command(
    name = "duck-mold",
    version,
    after_help = "Exit codes: 0 it worked, 1 the recipe has errors, 2 the command was wrong or a file couldn't be read or written.\nThe recipe format: docs/recipe-format.md. For AI agents: duck-mold guide."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// With --json: spread the JSON over lines, indented, for people to
    /// read. Without it, --json prints one line.
    #[arg(long, global = true)]
    pretty: bool,
}

/// Whether `--pretty` was given.
static PRETTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Subcommand)]
enum Command {
    /// Check a recipe, build it in memory and report on it. Writes nothing.
    ///
    /// Reports the model's size, triangles, colours and parts, whether it
    /// rests on the ground, and which parts float or overlap. A recipe
    /// with mistakes gets every mistake listed, with its file, line and
    /// field, and exit code 1.
    Inspect {
        /// A recipe (.toml or .json), an .obj mesh, or a .glb that Duck
        /// Mold wrote.
        file: PathBuf,
        /// Print the report as JSON, for agents.
        #[arg(long)]
        json: bool,
        /// Also list every named socket and where it is: the names a game
        /// can ask the .glb for, like head.top.
        #[arg(long)]
        sockets: bool,
        /// Also list the faces of every shaped part (one with
        /// "changes"): each name, the way it faces, its size and the
        /// faces next to it. These are the names changes and colours use.
        #[arg(long)]
        faces: bool,
        /// Also list the tree the model moves by: every part, the join
        /// it turns about, the part it hangs from, and how far it may
        /// turn (its "limits").
        #[arg(long)]
        joins: bool,
        /// Also list every hit shape (collider): its shape, where its
        /// middle is, and its size, radius or length.
        #[arg(long)]
        colliders: bool,
        /// Report on the model as it stands in this pose instead of at
        /// rest: its size, what floats and what overlaps then, and
        /// whether the pose breaks a part's limits.
        #[arg(long, value_name = "NAME")]
        pose: Option<String>,
        /// Report on one of the model's animations instead: its keys,
        /// and the worst that any of its frames (30 a second) gets
        /// wrong: parts that float or overlap, limits broken, how far
        /// it leaves the ground, and whether a loop ends as it starts.
        #[arg(long, value_name = "NAME", conflicts_with_all = ["pose", "sockets", "faces", "joins", "colliders"])]
        animation: Option<String>,
        /// What a game's click or shot would hit: the first hit shape on a
        /// ray from this point (x,y,z, in metres, in the model), going
        /// --toward. With --pose, or --animation and --at, as the model
        /// stands then.
        #[arg(
            long,
            value_name = "X,Y,Z",
            requires = "toward",
            allow_hyphen_values = true
        )]
        ray: Option<String>,
        /// Which way the ray goes, for --ray: x,y,z (any length).
        #[arg(
            long,
            value_name = "X,Y,Z",
            requires = "ray",
            allow_hyphen_values = true
        )]
        toward: Option<String>,
        /// For --ray with --animation: the time, in seconds.
        #[arg(long, value_name = "SECONDS", requires = "ray")]
        at: Option<f64>,
    },
    /// Build a recipe and write the model as a .glb file.
    ///
    /// Nothing is written if the recipe has mistakes.
    Export {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// Where to write the model. By default: where the recipe's
        /// `export` line says, or else next to the recipe, with the
        /// same name and the ending .glb.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Print the full report as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Draw a model and save the picture as a PNG: front, right side,
    /// back and three-quarter views on one sheet. Opens no window.
    ///
    /// The same recipe always gives the same picture, byte for byte.
    /// Nothing is written if the recipe has mistakes.
    Render {
        /// A recipe (.toml or .json), an .obj mesh, or a .glb that Duck
        /// Mold wrote.
        file: PathBuf,
        /// Where to write the picture. By default: next to the file, with
        /// the same name and the ending .png.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// How big each view is, in pixels (128 to 2048). The picture is
        /// twice as wide, and twice as tall plus a line for the caption.
        #[arg(long, value_name = "PIXELS", default_value_t = render::DEFAULT_SIZE)]
        size: usize,
        /// Print the full report as JSON, for agents, with where each view
        /// is in the picture.
        #[arg(long)]
        json: bool,
        /// Draw the model as it stands in this pose instead of at rest.
        /// The picture is then named after the pose too: robot.wave.png.
        #[arg(long, value_name = "NAME")]
        pose: Option<String>,
        /// Draw one of the model's animations instead, as a strip of
        /// frames side by side, evenly spaced in time, each with its
        /// time in its corner. The picture is named robot.walk.strip.png.
        #[arg(long, value_name = "NAME", conflicts_with = "pose")]
        animation: Option<String>,
        /// With --animation: how many frames the strip has (2 to 60).
        #[arg(long, value_name = "NUMBER", default_value_t = render::DEFAULT_FRAMES, requires = "animation")]
        frames: usize,
        /// With --animation: where the frames are drawn from: front,
        /// right, back, three-quarter (from the front right, a little
        /// above; the default) or top (from straight above, the front at
        /// the bottom).
        #[arg(long, value_name = "VIEW", requires = "animation")]
        view: Option<String>,
        /// Draw the hit shapes (colliders) over the model, as thin cyan
        /// outlines: in every view, and in every frame of a strip.
        #[arg(long)]
        colliders: bool,
        /// Draw one of the model's far-away versions instead (1 or 2: see
        /// `lods` in the recipe), the simpler model a game draws when it
        /// is far off. The picture is named robot.far1.png.
        #[arg(long, value_name = "1|2", conflicts_with = "animation")]
        far: Option<usize>,
    },
    /// Open the Studio: the window where a model is built with the mouse.
    ///
    /// Click a part to select it, change it in the panel on the right,
    /// click a socket to join a new part there. Every change is written
    /// to the recipe at once, as the same edit `duck-mold edit` makes, and
    /// a recipe saved by another program is picked up. Needs a screen.
    Studio {
        /// A recipe (.toml or .json) to edit. Without one, the Studio
        /// opens on its start page.
        file: Option<PathBuf>,
    },
    /// Open the Studio to look at a model, changing nothing: drag to
    /// orbit, right-drag to pan, roll the wheel to zoom, Escape to quit.
    ///
    /// When the recipe, or a file it uses, is saved, the model is built
    /// again. Mistakes are shown in the window, and the last good model
    /// stays up. Needs a screen; on a machine without one, use render.
    Preview {
        /// A recipe (.toml or .json), or a mesh (.obj, .gltf or .glb).
        file: PathBuf,
    },
    /// Start a new recipe: one box, standing on the ground. Or, with
    /// --example, a copy of one of the examples.
    ///
    /// Never writes over a file that is already there.
    New {
        /// The recipe to make. Its name must end in .toml.
        recipe: PathBuf,
        /// Start from a copy of this example instead of from one box:
        /// robot, house, snowman, desk, duck, dog, doughnut, mug, temple,
        /// barn, slide or gate. The files it uses are written beside it.
        #[arg(long, value_name = "NAME")]
        example: Option<String>,
        /// Print the report as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Write one part of a recipe alone, as a recipe of its own: for the
    /// parts library. What it is made of (its definition's, if it uses
    /// one) and the colours it uses go in; where it stands doesn't. The
    /// recipe doesn't change. Never writes over a file.
    SavePart {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// The part, as inspect names it: chair, or left_arm/hand.
        part: String,
        /// The new recipe to write: parts/chair.toml. Its name must end
        /// in .toml.
        file: PathBuf,
        /// Print the report as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Make a new image for a part and put it on: a PNG that is
    /// see-through all over, with the proportions of what it covers and
    /// as many pixels to the metre as the model's other images, written
    /// in textures/ beside the recipe. Paint on it with Duck Quill or
    /// any picture program. Never writes over a file.
    NewImage {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// The part, as inspect names it: head, or left_arm/hand.
        part: String,
        /// One face of the part, by name: front. Without it, the image
        /// is for the whole part.
        #[arg(long)]
        face: Option<String>,
        /// The image's file, from the recipe's folder. Without it:
        /// textures/<model>_<part>_<face>.png.
        #[arg(long, value_name = "FILE.png")]
        file: Option<String>,
        /// How many pixels wide and high: 64x48. Without it, a size
        /// worked out from what the image covers.
        #[arg(long, value_name = "WxH")]
        size: Option<String>,
        /// Print the result as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Open a part's image in Duck Quill to paint it (the Studio's
    /// "Paint in Duck Quill"). Duck Quill opens the PNG with a guide of
    /// the faces over it and paper the colour of the part; each save
    /// there shows on the model in the Studio at once. A part without an
    /// image gets a new one first (see new-image).
    Paint {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// The part, as inspect names it: head, or left_arm/hand.
        part: String,
        /// One face of the part, by name: front.
        #[arg(long)]
        face: Option<String>,
        /// Where Duck Quill's program is, if it isn't found by itself.
        /// Remembered for next time.
        #[arg(long, value_name = "PROGRAM")]
        duck_quill: Option<PathBuf>,
        /// Open the PNG in whatever this computer uses for pictures
        /// instead: any program that saves it works.
        #[arg(long)]
        system: bool,
        /// Start nothing: only say what would be started.
        #[arg(long)]
        dry_run: bool,
        /// Print the result as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Write the guide for a part's image: a see-through PNG of thin
    /// lines, the shape of the faces the image covers, with where other
    /// parts are joined on. Duck Quill draws it over the picture.
    ImageGuide {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// The part, as inspect names it.
        part: String,
        /// One face of the part, by name: front.
        #[arg(long)]
        face: Option<String>,
        /// Where to write it. Without it: beside the image, as
        /// <image>.guide.png.
        #[arg(short, long, value_name = "FILE.png")]
        output: Option<PathBuf>,
        /// Print the result as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Write an animation of a recipe, and the poses it uses, to a small
    /// file of their own: for the animation library. Another model takes
    /// it with `edit <recipe> add-animation <name> --from <file>`. The
    /// recipe doesn't change. Never writes over a file.
    SaveAnimation {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// The animation: walk.
        animation: String,
        /// The file to write: animations/walk.toml. Its name must end in
        /// .toml.
        file: PathBuf,
        /// Print the result as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Change a recipe without typing it: the same edits the Studio
    /// makes. `duck-mold edit --help` lists them all, grouped.
    ///
    /// Only what has to change in the file is changed: comments, order
    /// and layout stay. Prints one line saying what changed, and any
    /// problems (--report: inspect's whole report). A change that would
    /// leave the recipe with more mistakes than it had is refused, and
    /// nothing is written.
    #[command(after_help = edit::GROUPS)]
    Edit {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        #[command(subcommand)]
        change: Option<Box<edit::Change>>,
        /// Make many edits at once: one a line in this file (`-` reads
        /// them from standard input), each as it would follow `duck-mold
        /// edit <recipe>`. Blank lines and lines starting with # are
        /// skipped. They are made in turn and written once, or not at
        /// all if any is refused.
        #[arg(long, value_name = "FILE")]
        batch: Option<String>,
        /// Print the result as one line of JSON, for agents: the edit
        /// first, then what it did, the lines that changed and the
        /// problems.
        #[arg(long, global = true)]
        json: bool,
        /// Also print inspect's whole report on the recipe after the
        /// change (under "report" with --json).
        #[arg(long, global = true)]
        report: bool,
        /// Say what would happen, and write nothing.
        #[arg(long, global = true)]
        dry_run: bool,
    },
    /// Print the guide for AI agents: one page to start, or one topic
    /// of the full guide.
    ///
    /// With no topic: the page to start with (the commands, a recipe,
    /// the loop, the rules). With a topic: that part of the guide. `all`:
    /// the whole guide (docs/AI_GUIDE.md).
    Guide {
        /// recipes, edits, shaping, motion, hits, images, export,
        /// mistakes, or all.
        topic: Option<String>,
        /// List the topics.
        #[arg(long, conflicts_with = "topic")]
        list: bool,
    },
    /// Everything at once: inspect, every animation frame by frame, the
    /// images, and the four-view sheet; then one list of problems, worst
    /// first, each with the command that fixes it.
    ///
    /// Mistakes come first, then warnings a game would see (a part in
    /// the air, an animation that jumps), then small things (an unused
    /// colour). Each problem has `fix`, the command that surely fixes
    /// it, or else `advice`, what to do in words: it never guesses a fix.
    /// The sheet is written as `render` writes it (robot.png). Exit code
    /// 1 if the recipe has mistakes; with --json the report is printed
    /// then too.
    Check {
        /// A recipe (.toml or .json), or a model to look at (.obj, .glb).
        file: PathBuf,
        /// Print the report as JSON, for agents.
        #[arg(long)]
        json: bool,
        /// Where to write the sheet. By default: next to the file, with
        /// the same name and the ending .png.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// How big each view of the sheet is, in pixels (128 to 2048).
        #[arg(long, value_name = "PIXELS", default_value_t = render::DEFAULT_SIZE)]
        size: usize,
        /// Draw no sheet, and write nothing.
        #[arg(long, conflicts_with_all = ["output", "size"])]
        no_render: bool,
    },
    /// Serve Duck Mold's commands as tools for an AI app, over MCP (the
    /// Model Context Protocol), on standard input and output.
    ///
    /// The app starts it; you don't run it yourself. The tools: guide,
    /// inspect, check, edit, render (which returns the picture), export
    /// and new, each with the same arguments as its command. Claude Code:
    /// `claude mcp add duck-mold -- duck-mold mcp`. The README says how
    /// for Claude Desktop and Codex. Nothing goes over the network.
    Mcp {
        /// The folder file names start from. By default, the folder the
        /// app starts it in (Claude Desktop's is not one you chose).
        #[arg(long, value_name = "FOLDER")]
        folder: Option<PathBuf>,
    },
    /// Print the recipe format as a JSON Schema (draft 2020-12).
    ///
    /// For editors and agents that check a recipe before running it: it
    /// catches misspelt fields and wrong kinds of value. `inspect` and
    /// `check` find much more (names that must exist, sizes that fit).
    /// The same schema is docs/recipe.schema.json.
    Schema,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    PRETTY.store(cli.pretty, std::sync::atomic::Ordering::Relaxed);
    match cli.command {
        Command::Inspect {
            file,
            json,
            pose,
            animation,
            ray: Some(ray),
            toward: Some(toward),
            at,
            ..
        } => {
            let point = |text: &str, what: &str| {
                duck_mold_view::ray::parse_point(text).ok_or_else(|| {
                    tools::Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                        duck_mold_core::issue::Code::OutOfRange,
                        "",
                        format!("--{what} needs three numbers: x,y,z, like --{what} 0,1.4,5"),
                    )))
                })
            };
            let asked = point(&ray, "ray").and_then(|from| Ok((from, point(&toward, "toward")?)));
            let report = asked.and_then(|(from, toward)| {
                duck_mold_view::ray::ray_file(
                    &file,
                    from,
                    toward,
                    pose.as_deref(),
                    animation.as_deref().map(|name| (name, at.unwrap_or(0.0))),
                )
            });
            match report {
                Ok(report) => {
                    if json {
                        say_json(&report);
                    } else {
                        say(report.to_text().trim_end());
                    }
                    ExitCode::SUCCESS
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::Inspect {
            file,
            json,
            animation: Some(animation),
            ..
        } => match tools::inspect_animation_file(&file, &animation) {
            Ok(inspection) => {
                if json {
                    say_json(&inspection);
                } else {
                    say(inspection.to_text().trim_end());
                    complain_about(&inspection.warnings);
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::Inspect {
            file,
            json,
            sockets,
            faces,
            joins,
            colliders,
            pose,
            animation: None,
            ..
        } => match tools::inspect_file_listing(
            &file,
            &tools::Also {
                sockets,
                faces,
                joins,
                colliders,
                pose,
            },
        ) {
            Ok(inspection) => {
                if json {
                    say_json(&inspection);
                } else {
                    say(inspection.to_text().trim_end());
                    complain_about(&inspection.warnings);
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::Export {
            recipe,
            output,
            json,
        } => match tools::export_file(&recipe, output.as_deref()) {
            Ok(exported) => {
                if json {
                    say_json(&exported);
                } else {
                    say(&format!(
                        "wrote {} ({} bytes): {}",
                        exported.output, exported.bytes, exported.inspection.summary
                    ));
                    complain_about(&exported.inspection.warnings);
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::Render {
            file,
            output,
            size,
            json,
            animation: Some(animation),
            frames,
            view,
            colliders,
            ..
        } => match render::render_animation_file(
            &file,
            output.as_deref(),
            size,
            &animation,
            frames,
            view.as_deref(),
            colliders,
        ) {
            Ok(rendered) => {
                if json {
                    say_json(&rendered);
                } else {
                    say(&format!(
                        "wrote {} ({} x {} pixels, {} bytes): {}",
                        rendered.output,
                        rendered.width,
                        rendered.height,
                        rendered.bytes,
                        rendered.inspection.summary
                    ));
                    complain_about(&rendered.inspection.warnings);
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::Render {
            file,
            output,
            size,
            json,
            pose,
            colliders,
            far,
            ..
        } => {
            match render::render_file(
                &file,
                output.as_deref(),
                size,
                pose.as_deref(),
                far,
                colliders,
            ) {
                Ok(rendered) => {
                    if json {
                        say_json(&rendered);
                    } else {
                        say(&format!(
                            "wrote {} ({} x {} pixels, {} bytes): {}",
                            rendered.output,
                            rendered.width,
                            rendered.height,
                            rendered.bytes,
                            rendered.inspection.summary
                        ));
                        complain_about(&rendered.inspection.warnings);
                    }
                    ExitCode::SUCCESS
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::Studio { file } => match studio::run(file.as_deref(), false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => fail(&failure, false),
        },
        Command::Preview { file } => match studio::run(Some(&file), true) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => fail(&failure, false),
        },
        Command::New {
            recipe,
            example,
            json,
        } => match match &example {
            Some(example) => tools::new_from_example(&recipe, example),
            None => tools::new_file(&recipe),
        } {
            Ok(inspection) => {
                if json {
                    say_json(&inspection);
                } else {
                    say(&format!("wrote {}", inspection.source));
                    say(inspection.to_text().trim_end());
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::SavePart {
            recipe,
            part,
            file,
            json,
        } => match tools::save_part(&recipe, &part, &file) {
            Ok(inspection) => {
                if json {
                    say_json(&inspection);
                } else {
                    say(&format!("wrote {}", inspection.source));
                    say(inspection.to_text().trim_end());
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::NewImage {
            recipe,
            part,
            face,
            file,
            size,
            json,
        } => {
            let size = match size.as_deref().map(image_size) {
                None => None,
                Some(Ok(size)) => Some(size),
                Some(Err(why)) => {
                    return fail(
                        &Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                            duck_mold_core::issue::Code::OutOfRange,
                            "",
                            why,
                        ))),
                        json,
                    );
                }
            };
            match duck_mold_core::images::new_image(
                &recipe,
                &part,
                face.as_deref(),
                file.as_deref(),
                size,
            ) {
                Ok(made) => {
                    if json {
                        say_json(&serde_json::json!({
                            "ok": true,
                            "file": made.plan.file,
                            "width": made.plan.size[0],
                            "height": made.plan.size[1],
                            "metres": made.plan.metres,
                            "pixels_per_metre":
                                (made.plan.pixels_per_metre() * 10.0).round() / 10.0,
                            "part": made.plan.part,
                            "face": made.plan.face,
                            "summary": made.summary(),
                            "edit": made.edited.to_json(false),
                        }));
                    } else {
                        say(&made.summary());
                        say(&made.edited.edit.summary);
                    }
                    ExitCode::SUCCESS
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::Paint {
            recipe,
            part,
            face,
            duck_quill,
            system,
            dry_run,
            json,
        } => paint(
            &recipe,
            &part,
            face.as_deref(),
            duck_quill,
            system,
            dry_run,
            json,
        ),
        Command::ImageGuide {
            recipe,
            part,
            face,
            output,
            json,
        } => {
            let studio = duck_mold_view::studio::Studio::open(&recipe, true);
            let found = studio
                .image_shown(&part, face.as_deref())
                .zip(studio.guide(&part, face.as_deref()));
            let Some((shown, png)) = found else {
                return fail(&no_image(&studio, &recipe, &part, face.as_deref()), json);
            };
            let output = output.unwrap_or_else(|| shown.image.path.with_extension("guide.png"));
            match tools::write_file(&output, &png) {
                Ok(()) => {
                    let written = duck_mold_core::load::display_path(&output);
                    if json {
                        say_json(&serde_json::json!({
                            "ok": true,
                            "wrote": written,
                            "image": shown.image.file,
                        }));
                    } else {
                        say(&format!(
                            "wrote {written}: the guide for {}",
                            shown.image.file
                        ));
                    }
                    ExitCode::SUCCESS
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::SaveAnimation {
            recipe,
            animation,
            file,
            json,
        } => match tools::save_animation(&recipe, &animation, &file) {
            Ok(text) => {
                let written = duck_mold_core::load::display_path(&file);
                if json {
                    say_json(&serde_json::json!({
                        "wrote": written,
                        "animation": animation,
                        "text": text,
                    }));
                } else {
                    say(&format!("wrote {written}"));
                    say(text.trim_end());
                }
                ExitCode::SUCCESS
            }
            Err(failure) => fail(&failure, json),
        },
        Command::Edit {
            recipe,
            change,
            batch,
            json,
            report,
            dry_run,
        } => {
            let usage = |message: String| {
                Failure::Usage(Box::new(Issue::error(
                    duck_mold_core::issue::Code::TypeMismatch,
                    "",
                    message,
                )))
            };
            let done = match (change, batch) {
                (Some(change), None) => match change.to_edits() {
                    Ok(edits) => duck_mold_core::edit::edit_file_many(&recipe, &edits, dry_run),
                    Err(message) => Err(usage(message)),
                },
                (None, Some(batch)) => edit::read_batch(&batch)
                    .map_err(usage)
                    .and_then(|edits| {
                        duck_mold_core::edit::edit_file_batch(&recipe, &edits, dry_run)
                    }),
                (Some(_), Some(_)) => Err(usage(
                    "give one edit or --batch with a file of them, not both".to_owned(),
                )),
                (None, None) => Err(usage(
                    "say which edit to make (duck-mold edit --help lists them), or --batch with a file of them".to_owned(),
                )),
            };
            match done {
                Ok(edited) => {
                    if json {
                        say_json(&edited.to_json(report));
                    } else {
                        let info = &edited.edit;
                        say(&match (dry_run, info.written) {
                            (true, _) => format!(
                                "would edit {}: {} (a dry run: nothing was written)",
                                info.file, info.summary
                            ),
                            (false, true) => format!("edited {}: {}", info.file, info.summary),
                            (false, false) => format!(
                                "{} is already that way: {} changed nothing",
                                info.file, info.command
                            ),
                        });
                        if let Some(definition) = &info.definition {
                            say(&if info.also_changes.is_empty() {
                                format!("  this changed the definition \"{definition}\"")
                            } else {
                                format!(
                                    "  this changed the definition \"{definition}\", so it also changes: {}",
                                    info.also_changes.join(", ")
                                )
                            });
                        }
                        match &edited.inspection {
                            Some(inspection) => {
                                if report {
                                    say(inspection.to_text().trim_end());
                                }
                                complain_about(&inspection.warnings);
                            }
                            None => complain(
                                Report::from_issues(edited.errors.clone())
                                    .to_string()
                                    .trim_end(),
                            ),
                        }
                    }
                    if edited.inspection.is_some() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::Guide { topic, list } => {
            let text = match (topic.as_deref(), list) {
                (_, true) => guide::list(),
                (None, false) => guide::START.to_owned(),
                (Some("all"), false) => guide::full(),
                (Some(name), false) => match guide::topic(name) {
                    Some(text) => text,
                    None => {
                        let names = guide::TOPICS.iter().map(|(name, _)| *name).chain(["all"]);
                        let issue = Issue::error(
                            duck_mold_core::issue::Code::OutOfRange,
                            "",
                            format!("there is no topic \"{name}\" in the guide"),
                        )
                        .got(name)
                        .one_of(name, names);
                        return fail(&Failure::Usage(Box::new(issue)), false);
                    }
                },
            };
            say(text.trim_end());
            ExitCode::SUCCESS
        }
        Command::Check {
            file,
            json,
            output,
            size,
            no_render,
        } => {
            let options = check::Options {
                render: !no_render,
                output,
                size,
            };
            match check::check_file(&file, &options) {
                Ok(checked) => {
                    if json {
                        say_json(&checked);
                    } else {
                        say(checked.to_text().trim_end());
                    }
                    if checked.ok {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(failure) => fail(&failure, json),
            }
        }
        Command::Mcp { folder } => {
            if let Some(folder) = folder
                && let Err(error) = std::env::set_current_dir(&folder)
            {
                eprintln!(
                    "error: can't work in the folder \"{}\": {error}",
                    folder.display()
                );
                return ExitCode::from(2);
            }
            let program = match std::env::current_exe() {
                Ok(program) => program,
                Err(error) => {
                    eprintln!("error: can't find the duck-mold program itself: {error}");
                    return ExitCode::from(2);
                }
            };
            let stdin = std::io::stdin();
            match mcp::serve(stdin.lock(), std::io::stdout().lock(), &program) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Schema => {
            say(duck_mold_core::schema::RECIPE_SCHEMA.trim_end());
            ExitCode::SUCCESS
        }
    }
}

/// Reports a failure and gives the exit code for it: the mistakes as
/// lines for people on standard error, or as one JSON document on
/// standard output.
fn fail(failure: &Failure, json: bool) -> ExitCode {
    let report = failure.report();
    if json {
        say_json(&report);
    } else {
        match failure {
            // A wrong command gets one plain line.
            Failure::Usage(issue) => complain(&issue.to_string()),
            Failure::Invalid(_) => complain(report.to_string().trim_end()),
        }
    }
    ExitCode::from(failure.exit_code())
}

/// Prints the warnings of a model that was built, and the closing line,
/// for people.
fn complain_about(warnings: &[Issue]) {
    let report = Report::from_issues(warnings.to_vec());
    complain(report.to_string().trim_end());
}

/// Prints one JSON document: on one line, or spread out with `--pretty`.
fn say_json<T: serde::Serialize>(value: &T) {
    let text = if PRETTY.load(std::sync::atomic::Ordering::Relaxed) {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    };
    say(&text.expect("a report is plain data"));
}

/// Writes to standard output. If nobody is reading any more (as in
/// `duck-mold inspect robot.toml | head -3`), that isn't a problem: stop
/// quietly, like other command-line tools.
fn say(text: &str) {
    if let Err(error) = writeln!(std::io::stdout().lock(), "{text}") {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        complain(&format!("error: can't write the output: {error}"));
        std::process::exit(2);
    }
}

/// Writes to standard error. If even that fails there is nowhere left to
/// say so; the exit code still tells.
fn complain(text: &str) {
    let _ = writeln!(std::io::stderr().lock(), "{text}");
}

/// A size written `64x48`: how many pixels wide and high.
fn image_size(written: &str) -> Result<[u32; 2], String> {
    let size = written
        .split_once(['x', 'X'])
        .and_then(|(wide, high)| Some([wide.trim().parse().ok()?, high.trim().parse().ok()?]));
    size.ok_or_else(|| {
        format!("--size needs how many pixels wide and high, like 64x48, not \"{written}\"")
    })
}

/// Why a part has no image to paint or to draw a guide for: the
/// recipe's own mistakes if it doesn't build, or that there is none.
fn no_image(
    studio: &duck_mold_view::studio::Studio,
    recipe: &Path,
    part: &str,
    face: Option<&str>,
) -> Failure {
    let shown = duck_mold_core::load::display_path(recipe);
    let message = if studio.model().is_none() {
        format!("\"{shown}\" doesn't build: mend it first (duck-mold inspect {shown})")
    } else if studio.part(part).is_none() {
        format!("there is no part \"{part}\" in {shown}")
    } else {
        match face {
            Some(face) => format!(
                "the face \"{face}\" of \"{part}\" has no image: make one with duck-mold new-image {shown} {part} --face {face}"
            ),
            None => {
                format!("\"{part}\" has no image: make one with duck-mold new-image {shown} {part}")
            }
        }
    };
    Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
        duck_mold_core::issue::Code::Io,
        "",
        message,
    )))
}

/// `duck-mold paint`: opens a part's image in Duck Quill, or in the
/// system's picture program.
fn paint(
    recipe: &Path,
    part: &str,
    face: Option<&str>,
    duck_quill: Option<PathBuf>,
    system: bool,
    dry_run: bool,
    json: bool,
) -> ExitCode {
    use duck_mold_view::quill::{self, With};
    use duck_mold_view::studio::Studio;

    // A part without an image gets a new one first.
    let mut made = None;
    if Studio::open(recipe, true).image_shown(part, face).is_none() && !dry_run {
        match duck_mold_core::images::new_image(recipe, part, face, None, None) {
            Ok(new) => made = Some(new.summary()),
            Err(failure) => return fail(&failure, json),
        }
    }
    let studio = Studio::open(recipe, true);
    if studio.image_shown(part, face).is_none() {
        return fail(&no_image(&studio, recipe, part, face), json);
    }
    // The program: the one named, or the one remembered, or the one
    // found.
    let settings = quill::settings_folder();
    let remembered = settings.as_deref().and_then(quill::remembered);
    let with = if system {
        Some(With::System)
    } else if let Some(program) = duck_quill {
        if !program.is_file() {
            return fail(
                &Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                    duck_mold_core::issue::Code::Io,
                    "",
                    format!("there is no program at \"{}\"", program.display()),
                ))),
                json,
            );
        }
        Some(With::DuckQuill(program))
    } else {
        match remembered {
            Some(With::System) => Some(With::System),
            Some(With::DuckQuill(program)) => {
                quill::find_duck_quill_here(Some(&program)).map(With::DuckQuill)
            }
            None => quill::find_duck_quill_here(None).map(With::DuckQuill),
        }
    };
    let Some(with) = with else {
        let image = studio
            .image_shown(part, face)
            .map(|shown| shown.image.path.display().to_string())
            .unwrap_or_default();
        return fail(
            &Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                duck_mold_core::issue::Code::Io,
                "",
                format!(
                    "Duck Quill isn't on this computer, or Duck Mold can't find it. Say where it is with --duck-quill <program> (remembered for next time), or open the image in any picture program with --system: it is a plain PNG, {image}"
                ),
            ))),
            json,
        );
    };
    let guides = settings
        .as_deref()
        .map(|folder| folder.join("guides"))
        .unwrap_or_else(|| std::env::temp_dir().join("duck-mold-guides"));
    let job = match studio.paint_job(part, face, with.clone(), &guides) {
        Ok(job) => job,
        Err(why) => {
            return fail(
                &Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                    duck_mold_core::issue::Code::Io,
                    "",
                    why,
                ))),
                json,
            );
        }
    };
    let said = if dry_run {
        Ok("nothing was started (--dry-run)".to_owned())
    } else {
        if let Some(settings) = &settings {
            quill::remember(settings, &with);
        }
        job.start()
    };
    match said {
        Ok(said) => {
            if json {
                say_json(&serde_json::json!({
                    "ok": true,
                    "image": job.image,
                    "guide": job.guide,
                    "paper": format!("#{:02X}{:02X}{:02X}", job.paper[0], job.paper[1], job.paper[2]),
                    "with": match &job.with {
                        With::DuckQuill(_) => "duck-quill",
                        With::System => "system",
                    },
                    "command": job.words(),
                    "made": made,
                    "summary": said,
                }));
            } else {
                if let Some(made) = made {
                    say(&made);
                }
                say(&job.words().join(" "));
                say(&said);
            }
            ExitCode::SUCCESS
        }
        Err(why) => fail(
            &Failure::Usage(Box::new(duck_mold_core::issue::Issue::error(
                duck_mold_core::issue::Code::Io,
                "",
                why,
            ))),
            json,
        ),
    }
}
