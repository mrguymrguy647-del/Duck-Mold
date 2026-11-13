//! Reading a recipe from disk, with every file it names.
//!
//! A recipe can name other files: a shared palette
//! (`palette = "palette.toml"`), another recipe used as a part
//! (`file = "wheel.toml"`) or a mesh (`file = "rock.obj"`,
//! `file = "lamp.gltf"`, `file = "lamp.glb"`). [`load`] reads
//! them all, so that what comes out needs no more reading: every
//! [`Source::File`] has become a recipe or a mesh, and the colours of
//! every file are gathered into one palette for the model.
//!
//! Paths inside a recipe start from that recipe's folder.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::gltf::read_gltf;
use crate::issue::{Code, Issue};
use crate::mesh::read_obj;
use crate::recipe::{self, Loc, PaletteColor, PaletteDef, PartDef, Recipe, Source, hex};
use crate::source::{self, Syntax};

/// A recipe and everything it names, read and checked file by file.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The recipe. No [`Source::File`] is left in it.
    pub recipe: Recipe,
    /// The model's palette: the recipe's own colours first, then any new
    /// names from the recipes it uses as parts.
    pub palette: Vec<PaletteColor>,
    /// Every file that was read, the recipe itself first: what a window
    /// watches, to build the model again when one is saved.
    pub files: Vec<PathBuf>,
}

/// Reads the recipe at `path` and every file it names.
///
/// Mistakes in the files are added to `issues`; a recipe still comes back
/// so that later checks can find more. `Err` is only for the file named
/// on the command line itself: it isn't there, can't be read, or doesn't
/// end in `.toml` or `.json`.
pub fn load(path: &Path, issues: &mut Vec<Issue>) -> Result<Loaded, Box<Issue>> {
    let display = display_path(path);
    let Some(syntax) = Syntax::from_path(path) else {
        return Err(Box::new(Issue::error(
            Code::Io,
            "",
            format!("\"{display}\" isn't a recipe: a recipe's name ends in .toml or .json"),
        )));
    };
    let text = std::fs::read_to_string(path).map_err(|error| {
        Box::new(Issue::error(
            Code::Io,
            "",
            format!("can't read \"{display}\": {}", reason(&error)),
        ))
    })?;
    let mut loader = Loader {
        issues,
        palette: Vec::new(),
        open: Vec::new(),
        files: vec![path.to_path_buf()],
    };
    let recipe = loader.recipe(&text, syntax, path, &display, true);
    Ok(Loaded {
        recipe,
        palette: loader.palette,
        files: loader.files,
    })
}

/// Reads a recipe from text instead of a file, for tests and for tools
/// that hold a recipe in memory. `folder` is where the files it names are
/// looked for; `display` is its name in messages.
pub fn load_text(
    text: &str,
    syntax: Syntax,
    display: &str,
    folder: &Path,
    issues: &mut Vec<Issue>,
) -> Loaded {
    let mut loader = Loader {
        issues,
        palette: Vec::new(),
        open: Vec::new(),
        files: Vec::new(),
    };
    let recipe = loader.recipe(text, syntax, &folder.join(display), display, true);
    Loaded {
        recipe,
        palette: loader.palette,
        files: loader.files,
    }
}

/// Reads a recipe from text that belongs to the file at `path`, without
/// reading that file: for an edit, which builds the text it is about to
/// write. The files the recipe names are looked for beside `path`;
/// `display` is the recipe's name in messages.
pub fn load_text_at(
    text: &str,
    syntax: Syntax,
    path: &Path,
    display: &str,
    issues: &mut Vec<Issue>,
) -> Loaded {
    let mut loader = Loader {
        issues,
        palette: Vec::new(),
        open: Vec::new(),
        files: vec![path.to_path_buf()],
    };
    let recipe = loader.recipe(text, syntax, path, display, true);
    Loaded {
        recipe,
        palette: loader.palette,
        files: loader.files,
    }
}

struct Loader<'a> {
    issues: &'a mut Vec<Issue>,
    /// The model's palette so far.
    palette: Vec<PaletteColor>,
    /// The recipe files being read right now, outermost first, to catch a
    /// file that uses itself.
    open: Vec<(PathBuf, String)>,
    /// Every file read so far.
    files: Vec<PathBuf>,
}

impl Loader<'_> {
    /// Reads one recipe's text, then the files it names.
    fn recipe(
        &mut self,
        text: &str,
        syntax: Syntax,
        path: &Path,
        display: &str,
        top: bool,
    ) -> Recipe {
        let default_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("model")
            .to_owned();
        let root = match source::parse(text, syntax) {
            Ok(root) => root,
            Err(error) => {
                self.issues.push(
                    Issue::error(Code::Syntax, "", error.message)
                        .in_file(display)
                        .at(error.pos),
                );
                // Nothing could be read: an empty recipe, and no further
                // complaints about what it lacks.
                return Recipe {
                    file: display.to_owned(),
                    name: default_name,
                    description: None,
                    palette: PaletteDef::Inline(Vec::new()),
                    define: Vec::new(),
                    parts: Vec::new(),
                    sockets: Vec::new(),
                    origin: None,
                    poses: Vec::new(),
                };
            }
        };
        let mut recipe = recipe::decode(&root, display, &default_name, self.issues);

        let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.open.push((key, display.to_owned()));
        let folder = path.parent().unwrap_or(Path::new("")).to_path_buf();

        // The palette: written here, or in a file.
        let colors = match std::mem::replace(&mut recipe.palette, PaletteDef::Inline(Vec::new())) {
            PaletteDef::Inline(colors) => colors
                .into_iter()
                .map(|color| PaletteColor { own: top, ..color })
                .collect(),
            PaletteDef::File(named) => self.palette_file(&named.text, &named.loc, &folder, display),
        };
        for color in &colors {
            self.add_color(color);
        }
        recipe.palette = PaletteDef::Inline(colors);

        // The files that parts name.
        for define in &mut recipe.define {
            self.resolve(&mut define.part, &folder, display);
        }
        for part in &mut recipe.parts {
            self.resolve(part, &folder, display);
        }
        self.open.pop();
        recipe
    }

    /// Adds a colour to the model's palette. The same name with the same
    /// colour twice is fine (two recipes sharing a palette); with two
    /// different colours it is a mistake.
    fn add_color(&mut self, color: &PaletteColor) {
        match self.palette.iter().find(|known| known.name == color.name) {
            None => self.palette.push(color.clone()),
            Some(known) if known.rgb == color.rgb => {}
            Some(known) => {
                let message = format!(
                    "the colour \"{}\" is {} here but {} in {}: make them the same, or rename one",
                    color.name,
                    hex(color.rgb),
                    hex(known.rgb),
                    known.loc.file
                );
                self.issues.push(color.loc.error(Code::Conflict, message));
            }
        }
    }

    /// Reads `palette = "palette.toml"`.
    fn palette_file(
        &mut self,
        written: &str,
        loc: &Loc,
        folder: &Path,
        display: &str,
    ) -> Vec<PaletteColor> {
        let path = folder.join(written);
        let shown = display_join(display, written);
        let Some(syntax) = Syntax::from_path(&path) else {
            self.issues.push(loc.error(
                Code::Unsupported,
                format!(
                    "\"{written}\" can't be a palette file: its name must end in .toml or .json"
                ),
            ));
            return Vec::new();
        };
        let Some(text) = self.read(&path, written, loc) else {
            return Vec::new();
        };
        match source::parse(&text, syntax) {
            Ok(root) => recipe::decode_palette_file(&root, &shown, self.issues)
                .into_iter()
                .map(|color| PaletteColor {
                    own: false,
                    ..color
                })
                .collect(),
            Err(error) => {
                self.issues.push(
                    Issue::error(Code::Syntax, "", error.message)
                        .in_file(shown)
                        .at(error.pos),
                );
                Vec::new()
            }
        }
    }

    /// Reads a file a recipe names. A file that can't be read is a
    /// mistake in the recipe, reported where the recipe names it.
    fn read(&mut self, path: &Path, written: &str, loc: &Loc) -> Option<String> {
        self.files.push(path.to_path_buf());
        match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) => {
                self.issues.push(loc.error(
                    Code::FileNotFound,
                    format!(
                        "can't read \"{written}\": {}. Paths start from the recipe's own folder",
                        reason(&error)
                    ),
                ));
                None
            }
        }
    }

    /// Replaces every `file = "..."` in a part, and in the parts inside
    /// it, with what the file holds.
    fn resolve(&mut self, part: &mut PartDef, folder: &Path, display: &str) {
        match &mut part.source {
            Source::Parts(parts) => {
                for inner in parts {
                    self.resolve(inner, folder, display);
                }
            }
            Source::File(named) => {
                let named = named.clone();
                part.source = self.file_part(&named.text, &named.loc, folder, display);
            }
            _ => {}
        }
    }

    fn file_part(&mut self, written: &str, loc: &Loc, folder: &Path, display: &str) -> Source {
        let path = folder.join(written);
        let shown = display_join(display, written);
        let ending = path
            .extension()
            .and_then(|ending| ending.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ending.as_str() {
            "toml" | "json" => {
                let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                if self.open.iter().any(|(open, _)| *open == key) {
                    let mut chain: Vec<&str> =
                        self.open.iter().map(|(_, name)| name.as_str()).collect();
                    chain.push(&shown);
                    self.issues.push(loc.error(
                        Code::Cycle,
                        format!(
                            "this recipe ends up using itself as a part ({})",
                            chain.join(" > ")
                        ),
                    ));
                    return Source::Broken;
                }
                let Some(text) = self.read(&path, written, loc) else {
                    return Source::Broken;
                };
                let syntax = Syntax::from_path(&path).unwrap_or(Syntax::Toml);
                let before = self.issues.iter().filter(|issue| issue.is_error()).count();
                let recipe = self.recipe(&text, syntax, &path, &shown, false);
                let after = self.issues.iter().filter(|issue| issue.is_error()).count();
                if after > before && recipe.parts.is_empty() {
                    // The file couldn't be read at all; that is reported.
                    Source::Broken
                } else {
                    Source::Recipe(Box::new(recipe))
                }
            }
            "obj" => {
                let Some(text) = self.read(&path, written, loc) else {
                    return Source::Broken;
                };
                match read_obj(&text, &shown, self.issues) {
                    Some(mesh) => Source::Mesh(Arc::new(mesh)),
                    None => Source::Broken,
                }
            }
            "glb" | "gltf" => {
                self.files.push(path.clone());
                let bytes = match std::fs::read(&path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        self.issues.push(loc.error(
                            Code::FileNotFound,
                            format!(
                                "can't read \"{written}\": {}. Paths start from the recipe's own folder",
                                reason(&error)
                            ),
                        ));
                        return Source::Broken;
                    }
                };
                let beside = path.parent().unwrap_or(folder);
                match read_gltf(&bytes, &shown, beside, self.issues) {
                    Some(mesh) => {
                        self.files.extend(mesh.extra_files.iter().cloned());
                        // A model Duck Mold wrote brings its palette, as a
                        // recipe used as a part does.
                        if mesh.from_duck_mold {
                            for (name, rgb) in mesh.materials.iter().zip(&mesh.colors) {
                                if let Some(rgb) = rgb {
                                    self.add_color(&PaletteColor {
                                        name: name.clone(),
                                        rgb: *rgb,
                                        loc: loc.clone(),
                                        own: false,
                                    });
                                }
                            }
                        }
                        Source::Mesh(Arc::new(mesh))
                    }
                    None => Source::Broken,
                }
            }
            _ => {
                self.issues.push(loc.error(
                    Code::Unsupported,
                    format!(
                        "\"{written}\" can't be used as a part: a part's file is a recipe (.toml or .json) or a mesh (.obj, .gltf or .glb)"
                    ),
                ));
                Source::Broken
            }
        }
    }
}

/// Why a file couldn't be read, in a few words.
fn reason(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::NotFound => "there is no such file",
        std::io::ErrorKind::PermissionDenied => "permission denied",
        std::io::ErrorKind::InvalidData => "it isn't text",
        _ => "it can't be opened",
    }
}

/// A path as it is shown in messages: as given, with `/` between folders
/// on every system, so that reports are the same on Windows and Linux.
pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The name, for messages, of a file that the recipe `parent` names as
/// `written`: `examples/robot.toml` and `parts/arm.toml` give
/// `examples/parts/arm.toml`.
fn display_join(parent: &str, written: &str) -> String {
    let written = written.replace('\\', "/");
    if written.starts_with('/') || written.contains(':') {
        return written;
    }
    let mut pieces: Vec<&str> = parent.split('/').collect();
    pieces.pop(); // the parent's own file name
    for piece in written.split('/') {
        match piece {
            "" | "." => {}
            ".." if pieces
                .last()
                .is_some_and(|last| *last != ".." && !last.is_empty()) =>
            {
                pieces.pop();
            }
            other => pieces.push(other),
        }
    }
    pieces.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_in_messages_follow_the_recipes_folder() {
        assert_eq!(
            display_join("examples/robot.toml", "parts/arm.toml"),
            "examples/parts/arm.toml"
        );
        assert_eq!(display_join("robot.toml", "arm.toml"), "arm.toml");
        assert_eq!(
            display_join("a/b/robot.toml", "../shared/palette.toml"),
            "a/shared/palette.toml"
        );
        assert_eq!(
            display_join("robot.toml", "../palette.toml"),
            "../palette.toml"
        );
        assert_eq!(display_join("a/robot.toml", "./x.obj"), "a/x.obj");
        assert_eq!(display_join("a/robot.toml", "/abs/x.obj"), "/abs/x.obj");
        assert_eq!(
            display_join("a\\robot.toml".replace('\\', "/").as_str(), "sub\\x.obj"),
            "a/sub/x.obj"
        );
    }
}
