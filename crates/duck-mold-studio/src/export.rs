//! Where the model's `.glb` goes: the Model section's lines for it.
//!
//! A recipe can say where its `.glb` belongs (`export =
//! "../assets/robot.glb"`), so that Export puts it straight into a
//! game. The change here is one `set-export` edit, the same as
//! `duck-mold edit <recipe> set-export`. "Export after every change"
//! writes the `.glb` again after each change that builds, so a game
//! that is running shows the change at once.

use std::path::{Path, PathBuf};

use duck_mold_core::edit::Edit;
use duck_mold_view::studio::Studio;
use eframe::egui::{self, RichText};

/// What the Studio knows about exporting, beyond the recipe's own line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Exporting {
    /// True when the Studio is shown inside another program's window
    /// (Duck Forge's Studio): Export reads "Send to game".
    pub hosted: bool,
    /// The folder of the game the model is being made for, if there is
    /// one: its `assets` folder is offered as the place to export to.
    pub game: Option<PathBuf>,
    /// Whether the `.glb` is written again after every change that
    /// builds.
    pub always: bool,
}

impl Exporting {
    /// What the Export button says.
    pub fn button(&self) -> &'static str {
        if self.hosted {
            "Send to game"
        } else {
            "Export .glb"
        }
    }

    /// Where a model for this game would go, as its recipe would write
    /// it: `<game>/assets/<name>.glb`, from the recipe's own folder.
    /// `None` without a game, or if there is no way there from the
    /// recipe's folder (another drive).
    pub fn into_the_game(&self, recipe: &Path) -> Option<String> {
        let game = self.game.as_ref()?;
        let name = recipe.file_stem()?.to_str()?;
        from_folder_of(recipe, &game.join("assets").join(format!("{name}.glb")))
    }
}

/// The way to `file` from the folder `recipe` is in, as a recipe writes
/// it: with `/` between folders and `..` to go up. `None` if there is
/// no such way (on Windows, a file on another drive).
pub fn from_folder_of(recipe: &Path, file: &Path) -> Option<String> {
    // Whole paths, so that the two can be compared folder by folder.
    let whole = |path: &Path| -> PathBuf {
        let folder = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty());
        let folder = folder.unwrap_or(Path::new("."));
        let folder = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
        match path.file_name() {
            Some(name) => folder.join(name),
            None => folder,
        }
    };
    let (recipe, file) = (whole(recipe), whole(file));
    let from: Vec<_> = recipe.parent()?.components().collect();
    let to: Vec<_> = file.components().collect();
    let same = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    // Nothing in common, not even the drive: no way from one to the
    // other.
    if same == 0 {
        return None;
    }
    let mut pieces: Vec<String> = vec!["..".to_owned(); from.len() - same];
    for piece in &to[same..] {
        pieces.push(piece.as_os_str().to_str()?.to_owned());
    }
    Some(pieces.join("/"))
}

/// The Model section's lines about exporting. Gives the `set-export`
/// edit when the place is changed; the tick box changes
/// `exporting.always` itself.
pub fn model_lines(ui: &mut egui::Ui, studio: &Studio, exporting: &mut Exporting) -> Option<Edit> {
    if !studio.is_recipe() {
        return None;
    }
    let mut edit = None;
    ui.add_space(8.0);
    let heading = if exporting.hosted {
        "Send to game"
    } else {
        "Export"
    };
    ui.label(RichText::new(heading).strong()).on_hover_text(
        "Where the model's .glb file is written, for a game. The recipe can say (its \"export\" line); otherwise it goes beside the recipe",
    );
    let line = studio.export_line().map(str::to_owned);
    // Without a line of its own, a model made for a game goes to the
    // game's assets; any other goes beside its recipe.
    let in_the_game = exporting
        .into_the_game(studio.path())
        .filter(|_| line.is_none());
    let file_name = studio
        .export_path()
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let goes_to = match (&line, &in_the_game) {
        (Some(line), _) => line.clone(),
        (None, Some(_)) => format!("the game's assets ({file_name})"),
        (None, None) => format!("beside the recipe ({file_name})"),
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Goes to").weak());
        ui.label(goes_to).on_hover_text(match &line {
            Some(_) => "From the recipe's own folder: its \"export\" line says so",
            None => "The recipe has no \"export\" line",
        });
    });
    if !studio.look_only() {
        ui.horizontal_wrapped(|ui| {
            // Into the game the model is being made for, in one click.
            let suggested = exporting
                .into_the_game(studio.path())
                .filter(|suggested| line.as_ref() != Some(suggested));
            if let Some(suggested) = suggested {
                let set = ui.button("Write it in the recipe");
                if set
                    .on_hover_text(format!(
                        "Write export = \"{suggested}\" in the recipe, so that Duck Mold's own Export, and `duck-mold export`, put the model in the game's assets too"
                    ))
                    .clicked()
                {
                    edit = Some(Edit::SetExport {
                        path: Some(suggested),
                    });
                }
            }
            if !exporting.hosted
                && ui
                    .button("Choose...")
                    .on_hover_text("Choose where the .glb goes: a game's assets folder, say")
                    .clicked()
                && let Some(chosen) = pick_glb(studio)
            {
                match from_folder_of(studio.path(), &chosen) {
                    Some(path) => edit = Some(Edit::SetExport { path: Some(path) }),
                    None => {
                        // Said in the status line by the caller's edit
                        // failing would be odd: there is no edit. So
                        // nothing changes, and the tip says why.
                    }
                }
            }
            if line.is_some()
                && ui
                    .button("Beside the recipe")
                    .on_hover_text("Take the \"export\" line away: the .glb goes beside the recipe again")
                    .clicked()
            {
                edit = Some(Edit::SetExport { path: None });
            }
        });
        ui.checkbox(&mut exporting.always, "Export after every change")
            .on_hover_text(
                "Write the .glb again after each change that builds. A Duck Forge game that is running shows the change at once",
            );
    }
    edit
}

/// Asks where the `.glb` should go.
fn pick_glb(studio: &Studio) -> Option<PathBuf> {
    let now = studio.export_path();
    let mut dialog = rfd::FileDialog::new()
        .set_title("Where the model's .glb goes")
        .add_filter("glTF model", &["glb"]);
    if let Some(folder) = now.parent().filter(|folder| folder.is_dir()) {
        dialog = dialog.set_directory(folder);
    }
    if let Some(name) = now.file_name() {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    dialog.save_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_from_a_recipe_to_a_file_is_written_as_a_recipe_writes_it() {
        let root = std::env::temp_dir().join("duck_mold_export_paths");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("game/art/robots")).unwrap();
        std::fs::create_dir_all(root.join("game/assets")).unwrap();
        let recipe = root.join("game/art/robots/robot.toml");
        assert_eq!(
            from_folder_of(&recipe, &root.join("game/assets/robot.glb")).as_deref(),
            Some("../../assets/robot.glb")
        );
        assert_eq!(
            from_folder_of(&recipe, &root.join("game/art/robots/robot.glb")).as_deref(),
            Some("robot.glb")
        );
        assert_eq!(
            from_folder_of(&recipe, &root.join("game/art/robots/out/robot.glb")).as_deref(),
            Some("out/robot.glb")
        );
        // The game's assets, for a recipe anywhere in the game.
        let exporting = Exporting {
            hosted: true,
            game: Some(root.join("game")),
            always: true,
        };
        assert_eq!(
            exporting.into_the_game(&recipe).as_deref(),
            Some("../../assets/robot.glb")
        );
        assert_eq!(
            exporting
                .into_the_game(&root.join("game/robot.toml"))
                .as_deref(),
            Some("assets/robot.glb")
        );
        assert_eq!(Exporting::default().into_the_game(&recipe), None);
        assert_eq!(exporting.button(), "Send to game");
        assert_eq!(Exporting::default().button(), "Export .glb");
    }
}
