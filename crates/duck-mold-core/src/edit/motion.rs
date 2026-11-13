//! The edits of Phase 5: how far a part may turn (`limits`), and, from
//! the later steps, poses and animations.
//!
//! They are kept apart from the other edits only because that file is
//! long; they work the same way, on the recipe's text, and change only
//! the lines they must.

use super::doc::{Doc, Step, Val, key, with};
use super::{Outcome, Stop, find, problem, set, tidy_number};
use crate::issue::Code;
use crate::recipe::{Limits, Turns};
use crate::source::Value;

/// What `set-limits` does to a part's `limits`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LimitsTo {
    /// Takes them away: the part turns any way, any amount.
    None,
    /// `limits = "fixed"`: the part neither turns nor moves.
    Fixed,
    /// Changes how far it turns around x, y and z. A direction that is
    /// `None` here stays as it is. With no limits before, the directions
    /// not given don't turn: `limits` names the ones that may.
    Turns([Option<Turns>; 3]),
}

/// The `limits` a part's table has in the recipe as it is written, read
/// without complaint: what can't be understood counts as not there.
pub(super) fn limits_at(doc: &Doc, table: &[Step]) -> Limits {
    let Some(node) = doc.node(&with(table, key("limits"))) else {
        return Limits::Free;
    };
    match &node.value {
        Value::Str(word) if word == "fixed" => Limits::Fixed,
        Value::Table(entries) => {
            let mut turns = [Turns::No; 3];
            for entry in entries {
                let Some(axis) = Limits::AXES.iter().position(|axis| *axis == entry.key) else {
                    continue;
                };
                turns[axis] = match &entry.node.value {
                    Value::Str(word) if word == "free" => Turns::Any,
                    Value::Array(items) => match items.as_slice() {
                        [low, high] => match low.value.as_number().zip(high.value.as_number()) {
                            Some((low, high)) => Turns::Between(low, high),
                            None => Turns::No,
                        },
                        _ => Turns::No,
                    },
                    _ => Turns::No,
                };
            }
            Limits::Turns(turns)
        }
        _ => Limits::Free,
    }
}

/// `limits` as a recipe writes it: `"fixed"`, or a table of the
/// directions that turn. `None` for no limits at all.
pub(super) fn limits_val(limits: Limits) -> Option<Val> {
    match limits {
        Limits::Free => None,
        Limits::Fixed => Some(Val::Str("fixed".to_owned())),
        Limits::Turns(turns) => Some(Val::Table(
            Limits::AXES
                .iter()
                .zip(turns)
                .filter_map(|(axis, turns)| {
                    let value = match turns {
                        Turns::No => return None,
                        Turns::Any => Val::Str("free".to_owned()),
                        Turns::Between(low, high) => Val::List(vec![
                            Val::Num(tidy_number(low, 2)),
                            Val::Num(tidy_number(high, 2)),
                        ]),
                    };
                    Some(((*axis).to_owned(), value))
                })
                .collect(),
        )),
    }
}

/// `set-limits`: says how far a part may turn when the model is posed.
pub(super) fn set_limits(doc: &mut Doc, part: &str, to: LimitsTo) -> Result<Outcome, Stop> {
    let found = find(doc, part)?;
    let table = found.path();
    let before = limits_at(doc, &table);
    let after = match to {
        LimitsTo::None => Limits::Free,
        LimitsTo::Fixed => Limits::Fixed,
        LimitsTo::Turns(changes) => {
            if changes.iter().all(Option::is_none) {
                return Err(problem(
                    Code::MissingField,
                    "nothing to set: give --x, --y or --z (two angles like -30,40, or free, or none), or --fixed, or --none",
                ));
            }
            // Without a table yet, nothing turns but what is given now.
            let mut turns = match before {
                Limits::Turns(turns) => turns,
                Limits::Free | Limits::Fixed => [Turns::No; 3],
            };
            for ((axis, turns), change) in Limits::AXES.iter().zip(&mut turns).zip(changes) {
                let Some(change) = change else {
                    continue;
                };
                if let Turns::Between(low, high) = change {
                    if !(low <= 0.0 && high >= 0.0) {
                        return Err(problem(
                            Code::OutOfRange,
                            format!(
                                "the part stands at 0 when the model is at rest, so the limits around {axis} must have 0 between them: the first 0 or less and the second 0 or more, not {low},{high}"
                            ),
                        ));
                    }
                    if low < -36_000.0 || high > 36_000.0 {
                        return Err(problem(
                            Code::OutOfRange,
                            format!(
                                "a limit is between -36000 and 36000 degrees; for a part that turns round and round, like a wheel, write --{axis} free"
                            ),
                        ));
                    }
                }
                *turns = change;
            }
            Limits::Turns(turns)
        }
    };
    match limits_val(after) {
        Some(value) => set(doc, &table, "limits", value)?,
        None => {
            if doc.entry(&table, "limits").is_some() {
                doc.remove_field(&table, "limits").map_err(super::cant)?;
            }
        }
    }
    let summary = match after {
        Limits::Free if before == Limits::Free => {
            format!("\"{part}\" has no limits to take away: it turns freely already")
        }
        Limits::Free => format!("took the limits of \"{part}\" away: it turns freely"),
        Limits::Fixed => format!("fixed \"{part}\": it never turns or moves in a pose"),
        limits => format!("set the limits of \"{part}\": {}", limits.describe()),
    };
    Ok(Outcome {
        summary,
        touched: Some(found),
    })
}
