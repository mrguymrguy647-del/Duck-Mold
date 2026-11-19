//! The words of `duck-mold edit`, as a library.
//!
//! The `duck-mold` program is a thin layer over Duck Mold's libraries.
//! One piece of it is useful to other programs too: turning the words
//! of an edit (`set head --color red`) into the [`Edit`] the library
//! carries out. Duck Forge's Studio uses it for its `model.edit`
//! command, so that an agent writes an edit the same way everywhere.
//!
//! [`Edit`]: duck_mold_core::edit::Edit

pub mod edit;
pub mod guide;
