//! Duck Mold Studio's window.
//!
//! Everything the Studio *does* is in `duck_mold_view::studio`, with no
//! window, and every change it makes to a recipe is a
//! `duck_mold_core::edit::Edit`, the same as `duck-mold edit`. This crate
//! only shows it. It is a library as well as a program, so the tests can
//! draw every screen with no window, and so the Studio can later be shown
//! inside Duck Forge's Studio: [`StudioApp::ui`] draws into whatever space
//! it is given.

pub mod app;
pub mod properties;
pub mod theme;
pub mod view;

pub use app::StudioApp;
