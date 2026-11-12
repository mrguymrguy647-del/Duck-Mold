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
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use duck_mold_core::issue::{Issue, Report};
use duck_mold_core::tools::{self, Failure};
use duck_mold_view::render;

mod edit;
mod studio;

/// Duck Mold makes game-ready 3D models for Duck Forge from text recipes.
///
/// A recipe is a TOML or JSON file that lists a palette and the parts of
/// a model. Parts are joined by named sockets, not by coordinates.
#[derive(Parser)]
#[command(
    name = "duck-mold",
    version,
    after_help = "Exit codes: 0 it worked, 1 the recipe has errors, 2 the command was wrong or a file couldn't be read or written.\nThe recipe format: docs/recipe-format.md. For AI agents: docs/AI_GUIDE.md."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

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
    },
    /// Build a recipe and write the model as a .glb file.
    ///
    /// Nothing is written if the recipe has mistakes.
    Export {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        /// Where to write the model. By default: next to the recipe, with
        /// the same name and the ending .glb.
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
    /// Start a new recipe: one box, standing on the ground.
    ///
    /// Never writes over a file that is already there.
    New {
        /// The recipe to make. Its name must end in .toml.
        recipe: PathBuf,
        /// Print the report as JSON, for agents.
        #[arg(long)]
        json: bool,
    },
    /// Make one change to a recipe without typing it: add, remove, rename,
    /// join, set, duplicate, add-color, set-color, add-socket, set-socket.
    ///
    /// Only what has to change in the file is changed: comments, order
    /// and layout stay. Prints the same report as inspect, for the recipe
    /// after the change. A change that would leave the recipe with more
    /// mistakes than it had is refused, and nothing is written. These are
    /// the same edits the Studio makes.
    Edit {
        /// The recipe (.toml or .json).
        recipe: PathBuf,
        #[command(subcommand)]
        change: Box<edit::Change>,
        /// Print the report as JSON, for agents, with what the edit did
        /// under "edit".
        #[arg(long, global = true)]
        json: bool,
        /// Say what would happen, and write nothing.
        #[arg(long, global = true)]
        dry_run: bool,
    },
    /// Print the guide for AI agents: how to write recipes and use these
    /// commands, from scratch.
    Guide,
}

/// The guide, built into the program, so that an agent that has only
/// the program can still learn to use it.
const GUIDE: &str = include_str!("../../../docs/AI_GUIDE.md");

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Inspect {
            file,
            json,
            sockets,
        } => match if sockets {
            tools::inspect_file_with_sockets(&file)
        } else {
            tools::inspect_file(&file)
        } {
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
        } => match render::render_file(&file, output.as_deref(), size) {
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
        Command::Studio { file } => match studio::run(file.as_deref(), false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => fail(&failure, false),
        },
        Command::Preview { file } => match studio::run(Some(&file), true) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => fail(&failure, false),
        },
        Command::New { recipe, json } => match tools::new_file(&recipe) {
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
        Command::Edit {
            recipe,
            change,
            json,
            dry_run,
        } => {
            let edit = match change.to_edit() {
                Ok(edit) => edit,
                Err(message) => {
                    let issue =
                        Issue::error(duck_mold_core::issue::Code::TypeMismatch, "", message);
                    return fail(&Failure::Usage(Box::new(issue)), json);
                }
            };
            match duck_mold_core::edit::edit_file(&recipe, &edit, dry_run) {
                Ok(edited) => {
                    if json {
                        say_json(&edited.to_json());
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
                                format!("  it is inside the definition \"{definition}\"")
                            } else {
                                format!(
                                    "  it is inside the definition \"{definition}\", so this also changes: {}",
                                    info.also_changes.join(", ")
                                )
                            });
                        }
                        match &edited.inspection {
                            Some(inspection) => {
                                say(inspection.to_text().trim_end());
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
        Command::Guide => {
            say(GUIDE.trim_end());
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

fn say_json<T: serde::Serialize>(value: &T) {
    say(&serde_json::to_string_pretty(value).expect("a report is plain data"));
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
