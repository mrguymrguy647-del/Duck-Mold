//! The recipe format as a JSON Schema: `duck-mold schema`.
//!
//! A JSON Schema is a standard description of a JSON document's fields,
//! the kind of value each takes and the words it allows. Editors and AI
//! agents understand it: they can offer field names and catch a misspelt
//! one before Duck Mold is run. A TOML recipe read as JSON is the same
//! document, so one schema describes both.
//!
//! The schema is written by hand in `docs/recipe.schema.json` and built
//! into the program. Duck Mold reads recipes field by field
//! ([`crate::recipe`]), so there is nothing to make it from; the tests
//! keep the two in step instead (`tests/schema.rs`): every field the
//! reader takes is in the schema and every field of the schema is one
//! the reader takes, and every example recipe is valid by it.

/// The schema, as `docs/recipe.schema.json` holds it.
pub const RECIPE_SCHEMA: &str = include_str!("../../../docs/recipe.schema.json");
