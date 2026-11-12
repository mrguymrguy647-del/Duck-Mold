//! Pictures of Duck Mold models, drawn by Duck Forge's CPU renderer.
//!
//! `duck-mold render` ([`render`]) saves one PNG with four views of a
//! model, without opening a window. [`studio`] is everything Duck Mold
//! Studio does, also with no window: the file that is open, the camera,
//! what is selected, the edits, and the picture in its viewport. The
//! Studio's window (the `duck-mold-studio` crate) only shows that.
//!
//! `render`'s four views are front, right side, back and three-quarter. The
//! drawing is Duck Forge's own CPU renderer, used as a library: the
//! pictures show a model the way Duck Forge draws it.
//!
//! **The same model always gives the same picture**, byte for byte. Duck
//! Mold turns the model for each view itself, with its own sine and
//! cosine (the ones that make every computer write the same `.glb`), and
//! leaves Duck Forge's camera looking straight ahead, so Duck Forge does
//! no turning of its own. What Duck Forge then does is plain arithmetic,
//! except for one `tan` for its zoom; the tests compare the examples'
//! pictures byte for byte on Windows and Linux to show that it agrees.
//!
//! The library `duck-mold-core` knows nothing of this crate or of Duck
//! Forge: pictures are an extra on top.

pub mod render;
pub mod studio;

pub use render::{Rendered, Sheet, ViewPlace, render_file, render_sheet};
