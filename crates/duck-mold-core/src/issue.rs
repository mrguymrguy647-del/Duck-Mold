//! Problems, as data.
//!
//! Every mistake Duck Mold finds becomes an [`Issue`]: a short fixed
//! `code` an agent can act on, the file, line and field it is about, a
//! sentence for people, and, where one is obvious, what was allowed and a
//! suggestion. A [`Report`] holds all the issues of one run, so everything
//! can be fixed in one edit instead of one mistake at a time.
//!
//! The shape is Duck Echo's, with the file name added.

use std::fmt;

use serde::Serialize;

use crate::source::Pos;

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The recipe can't be built.
    Error,
    /// The recipe builds, but something looks like a mistake.
    Warning,
}

/// The short fixed word that says what kind of problem it is.
///
/// The first six are the same words Duck Echo uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    /// The file is not valid TOML or JSON.
    Syntax,
    /// A field name that doesn't exist here.
    UnknownField,
    /// A field that must be there is missing.
    MissingField,
    /// The wrong kind of value (text where a number goes, and so on).
    TypeMismatch,
    /// A number outside what is allowed.
    OutOfRange,
    /// The `version` is not one this program understands.
    UnsupportedVersion,
    /// A `shape` that doesn't exist.
    UnknownShape,
    /// A colour name that isn't in the palette.
    UnknownColor,
    /// A part name that isn't in the list, or isn't defined.
    UnknownPart,
    /// A socket name the part doesn't have.
    UnknownSocket,
    /// A face name the shape doesn't have.
    UnknownFace,
    /// Two things with one name.
    DuplicateName,
    /// Two fields that can't be used together.
    Conflict,
    /// Something that uses itself, directly or through others.
    Cycle,
    /// A name kept for a later phase (`bones`, `colliders`, `lods`).
    Reserved,
    /// Something this phase doesn't do yet.
    Unsupported,
    /// A file the recipe names can't be read.
    FileNotFound,
    /// A mesh file with a mistake in it.
    BadMesh,
    /// A problem with reading or writing a file given on the command line.
    Io,
    /// Something that is written but changes nothing.
    Unused,
    /// A face with no colour.
    NoColor,
    /// A part that touches nothing.
    Floating,
    /// A part entirely inside another.
    Hidden,
    /// A model that does not rest on the ground.
    OffGround,
    /// A note about a mesh file: something in it was skipped.
    Skipped,
    /// A change to a shape that would squeeze a face flat, turn the
    /// shape inside out or make it pass through itself.
    BadShape,
    /// A pose that turns a part further than its limits allow, or moves
    /// a part that is fixed.
    LimitBroken,
    /// A pose's name that the recipe doesn't have.
    UnknownPose,
}

impl Code {
    /// The code as it is written in JSON: `unknown_field`, `out_of_range`...
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Syntax => "syntax",
            Code::UnknownField => "unknown_field",
            Code::MissingField => "missing_field",
            Code::TypeMismatch => "type_mismatch",
            Code::OutOfRange => "out_of_range",
            Code::UnsupportedVersion => "unsupported_version",
            Code::UnknownShape => "unknown_shape",
            Code::UnknownColor => "unknown_color",
            Code::UnknownPart => "unknown_part",
            Code::UnknownSocket => "unknown_socket",
            Code::UnknownFace => "unknown_face",
            Code::DuplicateName => "duplicate_name",
            Code::Conflict => "conflict",
            Code::Cycle => "cycle",
            Code::Reserved => "reserved",
            Code::Unsupported => "unsupported",
            Code::FileNotFound => "file_not_found",
            Code::BadMesh => "bad_mesh",
            Code::Io => "io",
            Code::Unused => "unused",
            Code::NoColor => "no_color",
            Code::Floating => "floating",
            Code::Hidden => "hidden",
            Code::OffGround => "off_ground",
            Code::Skipped => "skipped",
            Code::BadShape => "bad_shape",
            Code::LimitBroken => "limit_broken",
            Code::UnknownPose => "unknown_pose",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a field may be.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Allowed {
    /// Any number from `min` to `max`, both included.
    Range {
        /// The smallest allowed value.
        min: f64,
        /// The largest allowed value.
        max: f64,
    },
    /// One of a list of words.
    OneOf {
        /// The words that would have been accepted.
        options: Vec<String>,
    },
}

/// One problem.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    /// An error (nothing is built) or a warning (it still builds).
    pub severity: Severity,
    /// The kind of problem, as a short fixed word.
    pub code: Code,
    /// The file it is in, as it was named on the command line or in the
    /// recipe. Empty when the problem isn't in a file.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub file: String,
    /// The line, counting from 1, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The column, counting from 1, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// The field, as a path: `parts[3].attach`, `define.arm.color`.
    /// Empty for the file as a whole.
    pub path: String,
    /// What is wrong, as a sentence.
    pub message: String,
    /// What was written, when that helps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<serde_json::Value>,
    /// What would have been accepted, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Allowed>,
    /// The likely fix: the word that was probably meant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Issue {
    /// A new error with a code, a field path and a sentence.
    pub fn error(code: Code, path: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            severity: Severity::Error,
            code,
            file: String::new(),
            line: None,
            column: None,
            path: path.into(),
            message: message.into(),
            got: None,
            allowed: None,
            suggestion: None,
        }
    }

    /// A new warning with a code, a field path and a sentence.
    pub fn warning(code: Code, path: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            severity: Severity::Warning,
            ..Issue::error(code, path, message)
        }
    }

    /// Says which file the problem is in.
    pub fn in_file(mut self, file: impl Into<String>) -> Self {
        self.file = file.into();
        self
    }

    /// Says where in the file the problem is.
    pub fn at(mut self, pos: Pos) -> Self {
        if pos.line > 0 {
            self.line = Some(pos.line);
            self.column = Some(pos.column);
        }
        self
    }

    /// Adds what was written.
    pub fn got(mut self, value: impl Into<serde_json::Value>) -> Self {
        self.got = Some(value.into());
        self
    }

    /// Adds the range of numbers that is allowed.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.allowed = Some(Allowed::Range { min, max });
        self
    }

    /// Adds the words that are allowed. If one of them is close to
    /// `written`, it becomes the suggestion.
    pub fn one_of<I, S>(mut self, written: &str, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        if self.suggestion.is_none() {
            self.suggestion = closest(written, options.iter().map(String::as_str));
        }
        self.allowed = Some(Allowed::OneOf { options });
        self
    }

    /// Adds the likely fix.
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// True for an error, false for a warning.
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// One line for people:
/// `error: robot.toml:31: parts[3].atach: there is no field "atach". Did you mean "attach"?`
impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.severity {
            Severity::Error => "error: ",
            Severity::Warning => "warning: ",
        })?;
        if !self.file.is_empty() {
            write!(f, "{}:", self.file)?;
            if let Some(line) = self.line {
                write!(f, "{line}:")?;
            }
            f.write_str(" ")?;
        }
        if !self.path.is_empty() {
            write!(f, "{}: ", self.path)?;
        }
        f.write_str(&self.message)?;
        if !self.message.ends_with(['.', '?', '!']) {
            f.write_str(".")?;
        }
        match (&self.suggestion, &self.allowed) {
            (Some(suggestion), _) => write!(f, " Did you mean \"{suggestion}\"?")?,
            (None, Some(Allowed::OneOf { options })) if !options.is_empty() => {
                write!(f, " It can be: {}.", options.join(", "))?;
            }
            _ => {}
        }
        if let Some(Allowed::Range { min, max }) = &self.allowed {
            write!(f, " Allowed: {} to {}.", number(*min), number(*max))?;
        }
        Ok(())
    }
}

/// A number the way a person writes it: `3`, not `3.0`.
fn number(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Everything found in one run.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Report {
    /// True when there are no errors. Warnings don't count.
    pub ok: bool,
    /// The problems that stop the build.
    pub errors: Vec<Issue>,
    /// The things that look like mistakes but still build.
    pub warnings: Vec<Issue>,
}

impl Report {
    /// Sorts a list of issues into errors and warnings, each in the
    /// order of the file's lines. The same issue found twice (a mistake
    /// inside a part that is used twice) is kept once.
    pub fn from_issues(mut issues: Vec<Issue>) -> Self {
        in_file_order(&mut issues);
        let mut report = Report {
            ok: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        for issue in issues {
            let list = if issue.is_error() {
                &mut report.errors
            } else {
                &mut report.warnings
            };
            if !list.contains(&issue) {
                list.push(issue);
            }
        }
        report.ok = report.errors.is_empty();
        report
    }

    /// The last line for people: `ok: no problems found`, or a count.
    pub fn closing_line(&self) -> String {
        let plural = |n: usize, word: &str| {
            if n == 1 {
                format!("1 {word}")
            } else {
                format!("{n} {word}s")
            }
        };
        match (self.errors.len(), self.warnings.len()) {
            (0, 0) => "ok: no problems found".to_owned(),
            (0, warnings) => format!("ok, with {}", plural(warnings, "warning")),
            (errors, 0) => format!("{}. Nothing was built.", plural(errors, "error")),
            (errors, warnings) => format!(
                "{}, {}. Nothing was built.",
                plural(errors, "error"),
                plural(warnings, "warning")
            ),
        }
    }
}

/// Every error, then every warning, one per line, then the closing line.
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for issue in self.errors.iter().chain(&self.warnings) {
            writeln!(f, "{issue}")?;
        }
        writeln!(f, "{}", self.closing_line())
    }
}

/// Puts issues in reading order: file by file (in the order the files
/// first come up), and within a file from the top line down. Issues
/// that have no line come after those that do.
pub fn in_file_order(issues: &mut [Issue]) {
    let mut files: Vec<String> = Vec::new();
    for issue in issues.iter() {
        if !files.contains(&issue.file) {
            files.push(issue.file.clone());
        }
    }
    // A stable sort: issues on the same line keep the order they were
    // found in.
    issues.sort_by_key(|issue| {
        (
            files.iter().position(|file| *file == issue.file),
            issue.line.unwrap_or(usize::MAX),
            issue.column.unwrap_or(usize::MAX),
        )
    });
}

/// The option closest to `written`, if one is close enough to be what was
/// meant: `atach` gives `attach`, `colour` gives `color`.
pub fn closest<'a>(written: &str, options: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let written = written.to_lowercase();
    let mut best: Option<(usize, &str)> = None;
    for option in options {
        let distance = edit_distance(&written, &option.to_lowercase());
        if best.is_none_or(|(least, _)| distance < least) {
            best = Some((distance, option));
        }
    }
    let (distance, option) = best?;
    // A third of the letters may differ, and always at least one.
    let limit = (written.chars().count().max(option.chars().count()) / 3).max(1);
    (distance <= limit).then(|| option.to_owned())
}

/// How many single-letter changes (add, remove, swap for another) turn
/// one word into the other.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    // `row[j]` is the distance between the first letters of `a` seen so far
    // and the first `j` letters of `b`.
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &letter_a) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &letter_b) in b.iter().enumerate() {
            let swap = diagonal + usize::from(letter_a != letter_b);
            diagonal = row[j + 1];
            row[j + 1] = swap.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_words_are_suggested_and_far_ones_are_not() {
        let fields = ["attach", "to", "rotate", "color", "colors"];
        assert_eq!(closest("atach", fields), Some("attach".to_owned()));
        assert_eq!(closest("colour", fields), Some("color".to_owned()));
        assert_eq!(closest("Rotate", fields), Some("rotate".to_owned()));
        assert_eq!(closest("banana", fields), None);
        assert_eq!(closest("nek", ["neck", "top"]), Some("neck".to_owned()));
        assert_eq!(closest("x", std::iter::empty()), None);
    }

    #[test]
    fn edit_distance_counts_changes() {
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }

    #[test]
    fn an_issue_reads_as_one_line() {
        let issue = Issue::error(
            Code::UnknownField,
            "parts[3].atach",
            "there is no field \"atach\"",
        )
        .in_file("robot.toml")
        .at(Pos {
            line: 31,
            column: 1,
        })
        .one_of("atach", ["attach", "to"]);
        assert_eq!(
            issue.to_string(),
            "error: robot.toml:31: parts[3].atach: there is no field \"atach\". Did you mean \"attach\"?"
        );

        let range = Issue::error(Code::OutOfRange, "parts[0].segments", "200 is too many")
            .in_file("a.toml")
            .at(Pos {
                line: 4,
                column: 12,
            })
            .range(3.0, 64.0);
        assert_eq!(
            range.to_string(),
            "error: a.toml:4: parts[0].segments: 200 is too many. Allowed: 3 to 64."
        );

        let options = Issue::error(
            Code::UnknownShape,
            "parts[0].shape",
            "there is no shape \"blob\"",
        )
        .one_of("blob", ["box", "cone"]);
        assert_eq!(
            options.to_string(),
            "error: parts[0].shape: there is no shape \"blob\". It can be: box, cone."
        );
    }

    #[test]
    fn an_issue_is_json_an_agent_can_read() {
        let issue = Issue::warning(Code::Unused, "palette.rust", "this colour is never used")
            .in_file("robot.toml")
            .at(Pos { line: 6, column: 1 });
        let json = serde_json::to_value(&issue).unwrap();
        assert_eq!(json["severity"], "warning");
        assert_eq!(json["code"], "unused");
        assert_eq!(json["file"], "robot.toml");
        assert_eq!(json["line"], 6);
        assert_eq!(json["path"], "palette.rust");
        // Fields with nothing to say are left out.
        assert!(json.get("suggestion").is_none());
        assert!(json.get("allowed").is_none());
    }

    #[test]
    fn issues_are_put_in_reading_order() {
        let at = |file: &str, line: usize, path: &str| {
            Issue::error(Code::Syntax, path, "x")
                .in_file(file)
                .at(Pos { line, column: 1 })
        };
        let mut issues = vec![
            at("robot.toml", 30, "c"),
            at("arm.toml", 2, "d"),
            at("robot.toml", 4, "a"),
            Issue::warning(Code::OffGround, "e", "x").in_file("robot.toml"),
            at("robot.toml", 4, "b"),
        ];
        in_file_order(&mut issues);
        let order: Vec<&str> = issues.iter().map(|issue| issue.path.as_str()).collect();
        // File by file, top to bottom; no line comes last; ties keep
        // their order.
        assert_eq!(order, ["a", "b", "c", "e", "d"]);
    }

    #[test]
    fn a_report_counts_and_drops_repeats() {
        let twice = Issue::error(Code::UnknownColor, "define.leg.color", "no colour \"oka\"");
        let report = Report::from_issues(vec![
            twice.clone(),
            twice,
            Issue::warning(Code::Unused, "palette.rust", "never used"),
        ]);
        assert!(!report.ok);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(
            report.closing_line(),
            "1 error, 1 warning. Nothing was built."
        );
        assert_eq!(
            Report::from_issues(Vec::new()).closing_line(),
            "ok: no problems found"
        );
    }
}
