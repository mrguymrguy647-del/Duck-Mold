//! The things a person can ask for from a menu or a key, and the menu a
//! right-click opens.
//!
//! A menu line and a key both give an [`Action`]; the window carries it
//! out in one place. Every action that changes the model is an edit an
//! agent can run too: this only chooses which.

use duck_mold_core::edit::Made;
use duck_mold_view::studio::viewport::Viewpoint;
use duck_mold_view::studio::{FaceTool, Mode, PoseTool, Studio};
use eframe::egui::{self, RichText};

use crate::library::Thumbs;
use crate::view;

/// Something asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// A copy of each selected part.
    Duplicate,
    /// A mirror image of each selected part.
    MirrorCopy,
    /// Type a new name for the selected part.
    Rename,
    /// Remove the selected parts.
    Remove,
    /// Make the selected parts that are joined free.
    Unjoin,
    /// Join the selected part to this socket: `body.neck`.
    JoinTo(String),
    /// Add a part where the menu was opened: at that point of the part's
    /// surface, or on its top.
    AddHere(Made),
    /// Add a part on the ground, joined to nothing.
    AddFree(Made),
    /// Open the list of what can be added on the ground.
    OpenAdd,
    /// Bring what is selected (or everything) into view.
    Frame,
    /// Stop drawing the selected parts.
    Hide,
    /// Draw every hidden part again.
    ShowAll,
    /// Select every part.
    SelectAll,
    /// Remove the sockets the Studio made that nothing uses.
    TidySockets,
    /// Take back the last change.
    Undo,
    /// Make the change that was taken back again.
    Redo,
    /// Look from the front, the side, the top or three-quarter.
    LookFrom(Viewpoint),
    /// Turn the grid on or off.
    Grid,
    /// Turn the wireframe on or off.
    Wireframe,
    /// Turn the ground shadow on or off.
    Shadow,
    /// Start a new recipe.
    NewModel,
    /// Open another file.
    Open,
    /// Write the `.glb` next to the recipe.
    Export,
    /// Write the four-view picture next to the recipe.
    RenderPicture,
    /// Close the file and go back to the start page.
    StartPage,
    /// Open this file: one opened lately.
    OpenFile(std::path::PathBuf),
    /// Open a copy of this example.
    OpenExample(&'static str),
    /// Show the first run's tips again.
    ShowTips,
    /// Show the list of keys.
    ShowKeys,
    /// Show what this program is.
    About,
    /// Shape the selected box: Face mode, or back to Part mode (Tab).
    Shape,
    /// Take this tool in hand in Face mode.
    FaceTool(FaceTool),
    /// The selected box's edges: Edge mode, or back to Part mode (E).
    EdgeMode,
    /// The selected box's corners: Corner mode, or back to Part mode
    /// (C).
    CornerMode,
    /// Pose mode, or back to Part mode (O).
    PoseMode,
    /// Take this tool in hand in Pose mode.
    PoseTool(PoseTool),
    /// Put the selected part back at rest in the pose chosen.
    PoseBackToRest,
    /// Ask for a name, and make a definition from the selected part.
    MakeDefinition,
    /// Write the selected part alone to the library's folder.
    SaveToLibrary,
}

/// Every key and what it does, for Help's list and the README's.
pub const KEYS: [(&str, &str); 27] = [
    (
        "G, R, S, T, P",
        "The Move, Rotate, Scale, Size and Paint tools",
    ),
    (
        "Tab",
        "Shape the selected box: click its faces. Tab again: back to parts",
    ),
    (
        "E",
        "The selected box's edges: click one to move it. E again: back to parts",
    ),
    (
        "C",
        "The selected box's corners: click one to move it. C again: back to parts",
    ),
    (
        "T, G, R, S, I, K (shaping)",
        "The Pull, Move, Turn, Scale, Inset and Cut tools",
    ),
    (
        "O",
        "Pose mode: turn parts about their joins, in a pose. O again: back to parts",
    ),
    ("R, G (posing)", "Turn with rings, or move with arrows"),
    ("Ctrl held while dragging", "No snapping"),
    ("Delete or Backspace", "Remove what is selected"),
    ("Ctrl+D", "Duplicate what is selected"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+Y or Ctrl+Shift+Z", "Redo"),
    (
        "F",
        "Frame: bring what is selected, or everything, into view",
    ),
    (
        "1, 2, 3, 4",
        "Look from the front, the side, the top, three-quarter",
    ),
    ("Z", "Wireframe on and off"),
    ("H", "Hide what is selected"),
    ("Shift+H", "Show everything hidden"),
    ("F2", "Rename"),
    ("Shift+A", "Add a part on the ground"),
    ("Shift+click", "Add a part to the selection, or take it out"),
    ("Ctrl+A", "Select every part"),
    (
        "Escape",
        "Let go: of a drag, a list, the faces, Face mode, then the selection",
    ),
    ("Ctrl+N", "New model"),
    ("Ctrl+O", "Open"),
    ("Ctrl+E", "Export the .glb"),
    ("Drag, right-drag, wheel", "Turn, slide and zoom the view"),
    ("Right-click a part", "Its menu"),
];

/// The File, Edit, View and Help menus of the top bar. Every line shows
/// its key, so the menus teach the keys. Gives what was chosen.
pub fn menus(ui: &mut egui::Ui, studio: &Studio, recent: &[std::path::PathBuf]) -> Option<Action> {
    let mut chosen = None;
    let edits = !studio.look_only();
    let any = !studio.selection().is_empty();
    let one = studio.selection().len() == 1;
    let mut pick = |ui: &mut egui::Ui, enabled: bool, words: &str, key: &str, action: Action| {
        if line(ui, enabled, words, key) {
            chosen = Some(action);
            ui.close();
        }
    };
    ui.menu_button("File", |ui| {
        pick(ui, true, "New model...", "Ctrl+N", Action::NewModel);
        pick(ui, true, "Open...", "Ctrl+O", Action::Open);
        ui.menu_button("Open an example", |ui| {
            ui.label(RichText::new("Opens a copy of your own").weak().small());
            for example in duck_mold_core::tools::EXAMPLES {
                pick(
                    ui,
                    true,
                    example.name,
                    "",
                    Action::OpenExample(example.name),
                );
            }
        });
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button("Opened lately", |ui| {
                for file in recent {
                    let name = file
                        .file_name()
                        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                    pick(ui, true, &name, "", Action::OpenFile(file.clone()));
                }
            });
        });
        ui.separator();
        pick(ui, true, "Export", "Ctrl+E", Action::Export);
        pick(ui, true, "Render picture", "", Action::RenderPicture);
        ui.separator();
        pick(ui, true, "Back to the start page", "", Action::StartPage);
    });
    ui.menu_button("Edit", |ui| {
        pick(
            ui,
            edits && studio.can_undo().is_some(),
            "Undo",
            "Ctrl+Z",
            Action::Undo,
        );
        pick(
            ui,
            edits && studio.can_redo().is_some(),
            "Redo",
            "Ctrl+Y",
            Action::Redo,
        );
        ui.separator();
        pick(ui, edits && any, "Duplicate", "Ctrl+D", Action::Duplicate);
        pick(ui, edits && any, "Mirror copy", "", Action::MirrorCopy);
        pick(ui, edits && one, "Rename", "F2", Action::Rename);
        pick(ui, edits && any, "Unjoin", "", Action::Unjoin);
        pick(ui, edits && any, "Remove", "Delete", Action::Remove);
        ui.separator();
        pick(ui, edits, "Tidy sockets", "", Action::TidySockets);
        pick(ui, true, "Select all", "Ctrl+A", Action::SelectAll);
    });
    ui.menu_button("View", |ui| {
        for (from, key) in Viewpoint::ALL.into_iter().zip(["1", "2", "3", "4"]) {
            let words = match from {
                Viewpoint::Front => "Front",
                Viewpoint::Side => "Side",
                Viewpoint::Top => "Top",
                Viewpoint::ThreeQuarter => "Three-quarter",
            };
            pick(ui, true, words, key, Action::LookFrom(from));
        }
        pick(ui, true, "Frame", "F", Action::Frame);
        ui.separator();
        let tick = |on: bool, words: &str| {
            if on {
                format!("{words}: on")
            } else {
                format!("{words}: off")
            }
        };
        pick(ui, true, &tick(studio.grid, "Grid"), "", Action::Grid);
        pick(
            ui,
            true,
            &tick(studio.wire, "Wireframe"),
            "Z",
            Action::Wireframe,
        );
        pick(ui, true, &tick(studio.shadow, "Shadow"), "", Action::Shadow);
        ui.separator();
        pick(ui, any, "Hide", "H", Action::Hide);
        pick(
            ui,
            studio.hidden_count() > 0,
            "Show everything hidden",
            "Shift+H",
            Action::ShowAll,
        );
    });
    ui.menu_button("Help", |ui| {
        pick(ui, true, "Show the tips again", "", Action::ShowTips);
        pick(ui, true, "The keys", "", Action::ShowKeys);
        pick(ui, true, "About Duck Mold Studio", "", Action::About);
    });
    chosen
}

/// A line of a menu, with its key at the right. True when clicked.
fn line(ui: &mut egui::Ui, enabled: bool, words: &str, key: &str) -> bool {
    let button = egui::Button::new(words).shortcut_text(key);
    ui.add_enabled(enabled, button).clicked()
}

/// The menu for the selected parts: what a right-click on a part opens,
/// in the picture or in the list. Gives what was chosen. `here` says
/// whether the menu was opened at a point of a part ("Add a part here"
/// then means that point).
pub fn part_menu(ui: &mut egui::Ui, studio: &mut Studio, thumbs: &mut Thumbs) -> Option<Action> {
    let mut chosen = None;
    let selection: Vec<String> = studio.selection().into_iter().map(str::to_owned).collect();
    let one = (selection.len() == 1).then(|| selection[0].clone());
    let any = !selection.is_empty();
    let edits = any && !studio.look_only();
    ui.set_min_width(190.0);
    let title = match (&one, selection.len()) {
        (Some(path), _) => path.clone(),
        (None, 0) => "Nothing selected".to_owned(),
        (None, count) => format!("{count} parts"),
    };
    ui.label(RichText::new(title).weak().small());
    // Posing: back to rest, and the way back.
    if studio.mode() == Mode::Pose {
        if line(
            ui,
            edits && one.is_some() && studio.pose() != "rest",
            "Back to rest",
            "",
        ) {
            chosen = Some(Action::PoseBackToRest);
        }
        for (tool, words, key) in [(PoseTool::Turn, "Turn", "R"), (PoseTool::Move, "Move", "G")] {
            if line(ui, true, words, key) {
                chosen = Some(Action::PoseTool(tool));
            }
        }
        ui.separator();
        if line(ui, true, "Back to Part", "O") {
            chosen = Some(Action::PoseMode);
        }
        if chosen.is_some() {
            ui.close();
        }
        return chosen;
    }
    // Shaping: the tools, and the way back.
    if studio.mode() != Mode::Part {
        let edges = studio.mode() == Mode::Edge;
        let corners = studio.mode() == Mode::Corner;
        for (tool, words, key) in [
            (FaceTool::Pull, "Pull", "T"),
            (FaceTool::Move, "Move", "G"),
            (FaceTool::Turn, "Turn", "R"),
            (FaceTool::Scale, "Scale", "S"),
            (FaceTool::Inset, "Inset", "I"),
            (FaceTool::Cut, "Cut", "K"),
        ] {
            if (edges && !matches!(tool, FaceTool::Move | FaceTool::Cut))
                || (corners && tool != FaceTool::Move)
            {
                continue;
            }
            if line(ui, true, words, key) {
                chosen = Some(Action::FaceTool(tool));
            }
        }
        ui.separator();
        if corners {
            if line(ui, true, "Faces", "Tab") {
                chosen = Some(Action::Shape);
            }
            if line(ui, true, "Back to Part", "C") {
                chosen = Some(Action::CornerMode);
            }
        } else if edges {
            if line(ui, true, "Faces", "Tab") {
                chosen = Some(Action::Shape);
            }
            if line(ui, true, "Back to Part", "E") {
                chosen = Some(Action::EdgeMode);
            }
        } else {
            if line(ui, true, "Edges", "E") {
                chosen = Some(Action::EdgeMode);
            }
            if line(ui, true, "Back to Part", "Tab") {
                chosen = Some(Action::Shape);
            }
        }
        if chosen.is_some() {
            ui.close();
        }
        return chosen;
    }
    if line(ui, edits && one.is_some(), "Shape", "Tab") {
        chosen = Some(Action::Shape);
    }
    if line(ui, edits && one.is_some(), "Edges", "E") {
        chosen = Some(Action::EdgeMode);
    }
    if line(ui, edits && one.is_some(), "Corners", "C") {
        chosen = Some(Action::CornerMode);
    }
    if line(ui, true, "Pose", "O") {
        chosen = Some(Action::PoseMode);
    }
    if line(ui, edits && one.is_some(), "Make a definition...", "") {
        chosen = Some(Action::MakeDefinition);
    }
    if line(ui, edits && one.is_some(), "Save to the library", "") {
        chosen = Some(Action::SaveToLibrary);
    }
    if line(ui, edits, "Duplicate", "Ctrl+D") {
        chosen = Some(Action::Duplicate);
    }
    if line(ui, edits, "Mirror copy", "") {
        chosen = Some(Action::MirrorCopy);
    }
    if line(ui, edits && one.is_some(), "Rename", "F2") {
        chosen = Some(Action::Rename);
    }
    if line(ui, edits, "Remove", "Delete") {
        chosen = Some(Action::Remove);
    }
    ui.separator();
    // What it could be joined to, part by part, socket by socket.
    let joinable = one
        .as_deref()
        .filter(|_| edits)
        .map(|path| studio.joinable(path))
        .unwrap_or_default();
    ui.add_enabled_ui(!joinable.is_empty(), |ui| {
        ui.menu_button("Join to", |ui| {
            for (part, sockets) in &joinable {
                ui.menu_button(part, |ui| {
                    for socket in sockets {
                        if ui.button(socket).clicked() {
                            chosen = Some(Action::JoinTo(format!("{part}.{socket}")));
                        }
                    }
                });
            }
        });
    });
    let joined = selection.iter().any(|path| {
        studio
            .part(path)
            .is_some_and(|part| part.attached_to.is_some())
    });
    if line(ui, edits && joined, "Unjoin", "") {
        chosen = Some(Action::Unjoin);
    }
    ui.add_enabled_ui(edits && one.is_some(), |ui| {
        ui.menu_button("Add a part here", |ui| {
            if let Some(made) = view::choices(ui, studio, thumbs) {
                chosen = Some(Action::AddHere(made));
            }
        });
    });
    ui.separator();
    if line(ui, true, "Frame", "F") {
        chosen = Some(Action::Frame);
    }
    if line(ui, any, "Hide", "H") {
        chosen = Some(Action::Hide);
    }
    if studio.hidden_count() > 0 && line(ui, true, "Show everything hidden", "Shift+H") {
        chosen = Some(Action::ShowAll);
    }
    if chosen.is_some() {
        ui.close();
    }
    chosen
}

/// The keys that ask for something, when no field is being typed in.
/// (Undo, redo and the tools' keys are the window's own.)
pub fn keys(ui: &egui::Ui) -> Option<Action> {
    use egui::{Key, Modifiers};
    ui.input_mut(|input| {
        let mut pressed = |modifiers: Modifiers, key: Key| input.consume_key(modifiers, key);
        if pressed(Modifiers::COMMAND, Key::N) {
            Some(Action::NewModel)
        } else if pressed(Modifiers::COMMAND, Key::O) {
            Some(Action::Open)
        } else if pressed(Modifiers::COMMAND, Key::E) {
            Some(Action::Export)
        } else if pressed(Modifiers::NONE, Key::Num1) {
            Some(Action::LookFrom(Viewpoint::Front))
        } else if pressed(Modifiers::NONE, Key::Num2) {
            Some(Action::LookFrom(Viewpoint::Side))
        } else if pressed(Modifiers::NONE, Key::Num3) {
            Some(Action::LookFrom(Viewpoint::Top))
        } else if pressed(Modifiers::NONE, Key::Num4) {
            Some(Action::LookFrom(Viewpoint::ThreeQuarter))
        } else if pressed(Modifiers::NONE, Key::Z) {
            Some(Action::Wireframe)
        } else if pressed(Modifiers::COMMAND, Key::D) {
            Some(Action::Duplicate)
        } else if pressed(Modifiers::COMMAND, Key::A) {
            Some(Action::SelectAll)
        } else if pressed(Modifiers::SHIFT, Key::A) {
            Some(Action::OpenAdd)
        } else if pressed(Modifiers::SHIFT, Key::H) {
            Some(Action::ShowAll)
        } else if pressed(Modifiers::NONE, Key::H) {
            Some(Action::Hide)
        } else if pressed(Modifiers::NONE, Key::F) {
            Some(Action::Frame)
        } else if pressed(Modifiers::NONE, Key::F2) {
            Some(Action::Rename)
        } else if pressed(Modifiers::NONE, Key::Delete) || pressed(Modifiers::NONE, Key::Backspace)
        {
            Some(Action::Remove)
        } else {
            None
        }
    })
}
