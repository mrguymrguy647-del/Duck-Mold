//! `duck-mold`: build and inspect 3D models from text recipes.
//!
//! This file only reads the command line and prints. All the work is done
//! by the `duck-mold-core` library ([`duck_mold_core::tools`]).
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
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Inspect { file, json } => match tools::inspect_file(&file) {
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
