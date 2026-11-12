//! `duck-mold`: build and inspect 3D models from text recipes.
//!
//! This file only reads the command line and prints. All the work is done
//! by the `duck-mold-core` library.

use clap::Parser;

/// Duck Mold: game-ready 3D models from text recipes.
#[derive(Parser)]
#[command(name = "duck-mold", version)]
struct Cli {}

fn main() {
    Cli::parse();
    println!(
        "duck-mold {} (recipe version {})",
        env!("CARGO_PKG_VERSION"),
        duck_mold_core::RECIPE_VERSION
    );
}
