//! Duck Mold's library: text recipes in, game-ready 3D models out.
//!
//! A *recipe* is a TOML or JSON file that lists a palette and the parts of
//! a model. This library reads it, checks it, builds the parts, joins them
//! by their sockets, colours every face from the palette and writes a
//! `.glb` file for Duck Forge. It can also measure the result, so an agent
//! can tell whether a model came out right without looking at a picture.
//!
//! The library knows nothing about the command line: the `duck-mold`
//! program is a thin layer over it. `DESIGN.md` at the top of the
//! repository explains every decision in plain words.

/// The version of the recipe format this library reads: `version = 0`.
pub const RECIPE_VERSION: i64 = 0;

pub mod assemble;
pub mod issue;
pub mod load;
pub mod math;
pub mod mesh;
pub mod model;
pub mod recipe;
pub mod shapes;
pub mod source;
