//! `duck-mold check`: everything an agent asks after a change, in one
//! call.
//!
//! It builds and inspects the model, looks at every animation frame by
//! frame, draws the four-view sheet, and gives one list of problems,
//! worst first: mistakes, then warnings that change what a game sees,
//! then small things. Each has the command that fixes it when one surely
//! does (`fix`), and otherwise what to do, in words (`advice`). It never
//! guesses a fix.

use std::path::{Path, PathBuf};

use duck_mold_core::inspect::{ImagesReport, Inspection, inspect, inspect_animation};
use duck_mold_core::issue::{Code, Issue, Report, in_file_order, shell_word};
use duck_mold_core::load::display_path;
use duck_mold_core::tools::{self, Failure};
use duck_mold_view::render;
use serde::Serialize;

/// How bad a problem is, worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    /// A mistake: nothing is built until it is fixed.
    Error,
    /// It builds, but a game would see it: a part in the air, a model
    /// that jumps, a broken limit.
    Warning,
    /// It builds and looks right: something unused, or a note.
    Small,
}

/// One problem, with what to do about it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    /// How bad it is: `error`, `warning` or `small`.
    pub rank: Rank,
    /// The problem, as `inspect` gives it (with `fix`, the command that
    /// fixes it, when one surely does).
    #[serde(flatten)]
    pub issue: Issue,
    /// What to do, in words, when no single command surely fixes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advice: Option<String>,
}

/// One animation, looked at frame by frame.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnimationCheck {
    /// Its name.
    pub name: String,
    /// How long it lasts, in seconds.
    pub length: f64,
    /// Whether it loops.
    pub loops: bool,
    /// How many frames were looked at.
    pub frames: usize,
    /// How many of the problems are its own.
    pub problems: usize,
}

/// The sheet `check` drew.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Picture {
    /// Where it was written, as it was named.
    pub output: String,
    /// How wide, in pixels.
    pub width: usize,
    /// How tall, in pixels.
    pub height: usize,
    /// How big the file is.
    pub bytes: usize,
}

/// Everything `check` found.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Checked {
    /// True when there are no mistakes. Warnings don't count.
    pub ok: bool,
    /// The whole report in one line.
    pub summary: String,
    /// The file that was checked, as it was named.
    pub source: String,
    /// How many problems of each rank.
    pub errors: usize,
    /// (See `errors`.)
    pub warnings: usize,
    /// (See `errors`.)
    pub small: usize,
    /// Every problem, worst first.
    pub problems: Vec<Problem>,
    /// What to do next, in a sentence.
    pub next: String,
    /// The four-view sheet, when the model was built and drawn.
    pub picture: Option<Picture>,
    /// Every animation, looked at frame by frame.
    pub animations: Vec<AnimationCheck>,
    /// The model's images, when it has any.
    pub images: Option<ImagesReport>,
    /// `inspect`'s whole report, when the model was built.
    pub inspection: Option<Inspection>,
}

/// What `check` is asked to do besides checking.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Draw the sheet.
    pub render: bool,
    /// Where to write it; by default next to the file, as `render` does.
    pub output: Option<PathBuf>,
    /// How big each view is, in pixels.
    pub size: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            render: true,
            output: None,
            size: render::DEFAULT_SIZE,
        }
    }
}

/// `duck-mold check`: the report on a file. A recipe with mistakes is a
/// report too (`ok` false); only a file that can't be read, or a wrong
/// option, is a [`Failure`].
pub fn check_file(path: &Path, options: &Options) -> Result<Checked, Failure> {
    if options.render && !(render::SMALLEST_SIZE..=render::LARGEST_SIZE).contains(&options.size) {
        return Err(Failure::Usage(Box::new(
            Issue::error(
                Code::OutOfRange,
                "size",
                format!("--size {} is not a size a view can be", options.size),
            )
            .got(options.size)
            .range(render::SMALLEST_SIZE as f64, render::LARGEST_SIZE as f64)
            .suggest(format!("--size {}", render::DEFAULT_SIZE)),
        )));
    }
    let built = match tools::build(path) {
        Ok(built) => built,
        Err(Failure::Invalid(report)) => return Ok(not_built(path, report)),
        Err(failure) => return Err(failure),
    };
    let source = built.source.clone();
    let mut inspection = inspect(&built.model, &source, built.warnings.clone());
    inspection.materials = built.materials.clone();
    let mut issues = inspection.warnings.clone();

    // Every animation, frame by frame: what it gets wrong that the
    // model at rest doesn't.
    let mut animations = Vec::new();
    for animation in &built.model.animations {
        let looked = inspect_animation(&inspection, &built.model, animation, &source);
        let before = issues.len();
        for warning in &looked.warnings {
            if !issues.contains(warning) {
                issues.push(warning.clone());
            }
        }
        let place = format!("animations.{}", animation.name);
        for broken in &looked.limits_broken {
            issues.push(
                Issue::warning(
                    Code::LimitBroken,
                    place.clone(),
                    format!(
                        "\"{}\" {} at {}",
                        animation.name,
                        broken.text,
                        seconds(broken.at)
                    ),
                )
                .in_file(&source)
                .on_part(broken.part.clone()),
            );
        }
        for float in looked.floating.iter().filter(|float| float.new) {
            issues.push(
                Issue::warning(
                    Code::Floating,
                    place.clone(),
                    format!(
                        "in \"{}\" the part \"{}\" comes off: at {} it floats {} from \"{}\"",
                        animation.name,
                        float.part,
                        seconds(float.at),
                        metres(float.gap),
                        float.nearest
                    ),
                )
                .in_file(&source)
                .on_part(float.part.clone()),
            );
        }
        animations.push(AnimationCheck {
            name: animation.name.clone(),
            length: looked.length,
            loops: looked.loops,
            frames: looked.frames,
            problems: issues.len() - before,
        });
    }

    let picture = if options.render {
        let sheet =
            render::render_sheet_with(&built.model, &inspection.headline(), options.size, false);
        let png = sheet.to_png();
        let output = options
            .output
            .clone()
            .unwrap_or_else(|| path.with_extension("png"));
        tools::write_file(&output, &png)?;
        Some(Picture {
            output: display_path(&output),
            width: sheet.width,
            height: sheet.height,
            bytes: png.len(),
        })
    } else {
        None
    };

    let mut checked = gather(&source, issues, inspection.summary.clone());
    checked.images = inspection.images.clone();
    checked.picture = picture;
    checked.animations = animations;
    checked.inspection = Some(inspection);
    checked.next = next(&checked);
    Ok(checked)
}

/// The report on a recipe that has mistakes: nothing was built.
fn not_built(path: &Path, report: Report) -> Checked {
    let source = display_path(path);
    let summary = report.closing_line();
    let issues = report.errors.into_iter().chain(report.warnings).collect();
    let mut checked = gather(&source, issues, summary);
    checked.next = next(&checked);
    checked
}

/// Sorts the issues into problems, worst first, each with what to do.
fn gather(source: &str, mut issues: Vec<Issue>, summary: String) -> Checked {
    in_file_order(&mut issues);
    let mut problems: Vec<Problem> = Vec::new();
    for issue in issues {
        if problems.iter().any(|known| known.issue == issue) {
            continue;
        }
        let advice = if issue.fix.is_some() {
            None
        } else {
            advice(&issue, source)
        };
        problems.push(Problem {
            rank: rank(&issue),
            issue,
            advice,
        });
    }
    // Stable: within a rank, the file's order stays.
    problems.sort_by_key(|problem| problem.rank);
    let count = |rank| problems.iter().filter(|p| p.rank == rank).count();
    Checked {
        ok: count(Rank::Error) == 0,
        summary,
        source: source.to_owned(),
        errors: count(Rank::Error),
        warnings: count(Rank::Warning),
        small: count(Rank::Small),
        problems,
        next: String::new(),
        picture: None,
        animations: Vec::new(),
        images: None,
        inspection: None,
    }
}

/// How bad an issue is.
pub fn rank(issue: &Issue) -> Rank {
    if issue.is_error() {
        return Rank::Error;
    }
    match issue.code {
        Code::Unused | Code::Skipped | Code::MixedPixels | Code::Hidden => Rank::Small,
        // A mesh's materials that keep the file's own colours: it looks
        // as the file does.
        Code::NoColor if issue.message.contains("keep the file's own") => Rank::Small,
        _ => Rank::Warning,
    }
}

/// What to do about an issue that no single command surely fixes, in
/// words: `None` when its own message says it already.
fn advice(issue: &Issue, source: &str) -> Option<String> {
    let file = shell_word(if issue.file.is_empty() {
        source
    } else {
        &issue.file
    });
    // A mistake's own message says what to do: where, what was allowed,
    // and the word that was probably meant.
    if issue.is_error() {
        return None;
    }
    // The part it is about, by the path `edit` takes, when it is known.
    let part = issue
        .part
        .as_deref()
        .map_or_else(|| "<part>".to_owned(), shell_word);
    let words = match issue.code {
        Code::Floating if issue.path.starts_with("animations.") => format!(
            "the pose at that time moves it away: change that pose (`duck-mold edit {file} set-pose <pose> {part} ...`) or the part's limits (`duck-mold edit {file} set-limits {part} ...`)"
        ),
        Code::Floating => format!(
            "join it to the part it should touch: `duck-mold edit {file} join {part} --to <other>.<socket>`; or move it until it touches. Render to see where it is"
        ),
        Code::Hidden => format!(
            "if nobody is meant to see it, `duck-mold edit {file} remove {part}`; otherwise move it out or make it bigger"
        ),
        Code::OffGround => format!(
            "a model stands on y = 0, with its zero point at its bottom: look at its `origin` and at any `move` that lifts or sinks its lowest part (`duck-mold render {file}` shows it)"
        ),
        Code::LimitBroken => format!(
            "turn the part less in that pose (`duck-mold edit {file} set-pose <pose> {part} ...`), or let it turn further (`duck-mold edit {file} set-limits {part} ...`)"
        ),
        Code::LoopJumps => format!(
            "add a last key with the first key's pose, at the animation's end: `duck-mold edit {file} add-key <animation> <seconds> <pose>`"
        ),
        Code::NoColor if rank(issue) == Rank::Small => return None,
        Code::NoColor => format!(
            "give the part a colour from the palette: `duck-mold edit {file} set {part} --color <colour>`"
        ),
        Code::HitShape => format!(
            "change the part's own hit-shape numbers, or let it be fitted again: `duck-mold edit {file} set-collider {part} fitted`"
        ),
        Code::Unused if issue.message.contains("is defined but never used") => format!(
            "use it (`duck-mold edit {file} add <name> --use <definition> --to <part>.<socket>`), or delete its [define] section if it isn't needed"
        ),
        _ => return None,
    };
    Some(words)
}

/// What to do next, in a sentence.
fn next(checked: &Checked) -> String {
    let file = shell_word(&checked.source);
    if checked.errors > 0 {
        format!(
            "Fix the {} first: each says where it is. Nothing is built until then. Then run `duck-mold check {file}` again.",
            if checked.errors == 1 {
                "error"
            } else {
                "errors"
            }
        )
    } else if checked.warnings > 0 {
        format!(
            "It builds. Look at the {}: each changes what a game sees. Fix them, then check again.",
            if checked.warnings == 1 {
                "warning"
            } else {
                "warnings"
            }
        )
    } else {
        let look = match &checked.picture {
            Some(picture) => format!(
                "If you can see images, look at {} first: is every part where you meant? ",
                picture.output
            ),
            None => String::new(),
        };
        format!("{look}When it is right: `duck-mold export {file}`.")
    }
}

impl Checked {
    /// The report for people.
    pub fn to_text(&self) -> String {
        let mut out = format!("{}: {}\n", self.source, self.summary);
        for problem in &self.problems {
            out.push_str(&format!("{}\n", problem.issue));
            if let Some(fix) = &problem.issue.fix {
                out.push_str(&format!("  fix: {fix}\n"));
            }
            if let Some(advice) = &problem.advice {
                out.push_str(&format!("  to do: {advice}\n"));
            }
        }
        for animation in &self.animations {
            out.push_str(&format!(
                "animation {}: {}, {}, {} frames looked at, {}\n",
                animation.name,
                seconds(animation.length),
                if animation.loops {
                    "loops"
                } else {
                    "plays once"
                },
                animation.frames,
                match animation.problems {
                    0 => "no problems".to_owned(),
                    1 => "1 problem".to_owned(),
                    n => format!("{n} problems"),
                }
            ));
        }
        if let Some(picture) = &self.picture {
            out.push_str(&format!(
                "picture: {} ({} x {} pixels)\n",
                picture.output, picture.width, picture.height
            ));
        }
        out.push_str(&format!(
            "{} {}, {} {}, {} small\n",
            self.errors,
            if self.errors == 1 { "error" } else { "errors" },
            self.warnings,
            if self.warnings == 1 {
                "warning"
            } else {
                "warnings"
            },
            self.small
        ));
        out.push_str(&format!("next: {}\n", self.next));
        out
    }
}

/// `0.5 s`.
fn seconds(at: f64) -> String {
    let text = format!("{at:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} s")
}

/// `4 mm`, `2.5 cm`, `1.2 m`.
fn metres(length: f64) -> String {
    let short = |value: f64| {
        let text = format!("{value:.1}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    if length < 0.01 {
        format!("{} mm", short(length * 1000.0))
    } else if length < 1.0 {
        format!("{} cm", short(length * 100.0))
    } else {
        format!("{} m", short(length))
    }
}
