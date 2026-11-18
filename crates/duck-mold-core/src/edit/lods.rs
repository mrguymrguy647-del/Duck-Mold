//! The edits of Phase 7's far-away versions: `set-lods` (the recipe's
//! line) and `set-lod` (a part's own). Like every edit they work on the
//! recipe's text and change only the lines they must.

use super::doc::{Doc, Val};
use super::{Outcome, Stop, cant, find, problem, set};
use crate::issue::Code;
use crate::recipe::{Lods, TOP_FIELDS};

/// `set-lods`: the recipe's line. `None` takes it away: no far-away
/// versions.
pub(super) fn set_lods(doc: &mut Doc, to: Option<Lods>) -> Result<Outcome, Stop> {
    let summary = match to {
        Some(Lods::Auto) => {
            doc.set_field(&[], "lods", &Val::Str("auto".to_owned()), &TOP_FIELDS)
                .map_err(cant)?;
            "the model now has two far-away versions, at distances worked out from its size (lods = \"auto\")".to_owned()
        }
        Some(Lods::At([near, far])) => {
            if !(near > 0.0 && far > near && far.is_finite()) {
                return Err(problem(
                    Code::OutOfRange,
                    format!(
                        "far-away versions are at two distances, both above 0, the second further: not {near} and {far}"
                    ),
                ));
            }
            doc.set_field(
                &[],
                "lods",
                &Val::List(vec![Val::Num(near), Val::Num(far)]),
                &TOP_FIELDS,
            )
            .map_err(cant)?;
            format!("the model now has two far-away versions, from {near} m and from {far} m")
        }
        None => {
            if doc.entry(&[], "lods").is_none() {
                return Err(problem(
                    Code::MissingField,
                    "the recipe has no \"lods\" to take away: it has no far-away versions",
                ));
            }
            doc.remove_field(&[], "lods").map_err(cant)?;
            "took \"lods\" away: the model has no far-away versions now".to_owned()
        }
    };
    Ok(Outcome {
        summary,
        touched: None,
    })
}

/// `set-lod`: `keep` writes `lod = "keep"` on the part (never
/// simplified or left out); `auto` takes it away.
pub(super) fn set_lod(doc: &mut Doc, part: &str, keep: bool) -> Result<Outcome, Stop> {
    let found = find(doc, part)?;
    let table = found.path();
    let summary = if keep {
        set(doc, &table, "lod", Val::Str("keep".to_owned()))?;
        format!("\"{part}\" is kept whole in the far-away versions (lod = \"keep\")")
    } else {
        if doc.entry(&table, "lod").is_none() {
            return Err(problem(
                Code::MissingField,
                format!(
                    "\"{part}\" has no \"lod\" to take away: the far-away versions already simplify it"
                ),
            ));
        }
        doc.remove_field(&table, "lod").map_err(cant)?;
        format!("\"{part}\" is simplified in the far-away versions like the rest")
    };
    Ok(Outcome {
        summary,
        touched: Some(found),
    })
}
