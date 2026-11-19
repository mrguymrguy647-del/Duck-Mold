//! Duck Mold Studio's window.
//!
//! Everything the Studio *does* is in `duck_mold_view::studio`, with no
//! window, and every change it makes to a recipe is a
//! `duck_mold_core::edit::Edit`, the same as `duck-mold edit`. This crate
//! only shows it. It is a library, so that the tests can draw every
//! screen with no window, and so that two programs can show it:
//! `DuckMoldStudio` (the crate `duck-mold-studio-app`, which opens a
//! window for it and carries the icon), and Duck Forge's Studio, which
//! shows it in a tab. [`StudioApp::ui`] draws into whatever space it is
//! given.

pub mod actions;
pub mod app;
pub mod crash;
pub mod export;
pub mod far;
pub mod gallery;
pub mod hits;
pub mod icons;
pub mod images;
pub mod library;
pub mod palette;
pub mod posing;
pub mod properties;
pub mod recent;
pub mod system;
pub mod theme;
pub mod timeline;
pub mod tour;
pub mod view;

pub use app::StudioApp;
