//! Duck Mold Studio: the window where a model is built with the mouse.
//!
//! `DuckMoldStudio` opens the start page; `DuckMoldStudio robot.toml`
//! opens that recipe (dropping a recipe on the program does the same);
//! `DuckMoldStudio --look robot.toml` only shows it, which is what
//! `duck-mold preview` starts.

// On Windows, a release build opens no black console window behind the
// Studio. Debug builds keep it, so `cargo run` shows messages.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

use duck_mold_studio::{StudioApp, theme};
use eframe::egui;

const HELP: &str = "\
Duck Mold Studio: build a 3D model with the mouse.

Usage: DuckMoldStudio [--look] [FILE]

  FILE     A recipe (.toml or .json) to edit, or a model (.obj, .gltf,
           .glb) to look at. Without one, the start page opens.
  --look   Only look at FILE: nothing can be changed.
  --version, --help

Every change the Studio makes is a `duck-mold edit` command.";

fn main() -> eframe::Result {
    let mut look_only = false;
    let mut file: Option<PathBuf> = None;
    for word in std::env::args_os().skip(1) {
        match word.to_str() {
            Some("--look") => look_only = true,
            Some("--version" | "-V") => {
                println!("DuckMoldStudio {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--help" | "-h") => {
                println!("{HELP}");
                return Ok(());
            }
            _ => file = Some(PathBuf::from(word)),
        }
    }

    let app = match file {
        Some(file) => StudioApp::with_file(file, look_only),
        None => StudioApp::new(),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app.title())
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Duck Mold Studio",
        options,
        Box::new(|creation| {
            creation.egui_ctx.set_theme(egui::ThemePreference::Dark);
            theme::apply_brand(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
