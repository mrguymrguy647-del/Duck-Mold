//! The edits of Phase 6's hit shapes: `set-colliders` (the recipe's line)
//! and `set-collider` (a part's own). Like every edit they work on the
//! recipe's text and change only the lines they must.

use super::doc::{Doc, Val};
use super::{Outcome, Stop, cant, find, problem, set};
use crate::hit::fitted_numbers;
use crate::issue::Code;
use crate::model::Model;
use crate::recipe::{Colliders, HitKind, Limits, OwnHitShape, TOP_FIELDS};

/// What `set-collider` does to a part's `collider`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColliderTo {
    /// Takes the part's own away: it follows `colliders` again.
    Fitted,
    /// `collider = "none"`: no hit shape.
    None,
    /// A shape: fitted to the part when no number is given; with
    /// numbers, those (and the fitted ones for what isn't given).
    Shape {
        /// Which shape.
        kind: HitKind,
        /// A box's size.
        size: Option<[f64; 3]>,
        /// A sphere's or a capsule's radius.
        radius: Option<f64>,
        /// A capsule's length, end to end.
        length: Option<f64>,
        /// Where its middle is, in the part's own directions.
        center: Option<[f64; 3]>,
        /// Which way a capsule lies: 0, 1 or 2 for x, y or z.
        along: Option<usize>,
    },
}

/// `set-colliders`: the recipe's line. `None` takes it away.
pub(super) fn set_colliders(doc: &mut Doc, to: Option<Colliders>) -> Result<Outcome, Stop> {
    let summary = match to {
        Some(colliders) => {
            doc.set_field(
                &[],
                "colliders",
                &Val::Str(colliders.name().to_owned()),
                &TOP_FIELDS,
            )
            .map_err(cant)?;
            match colliders {
                Colliders::Parts => {
                    "every part now has a hit shape, fitted to it (colliders = \"parts\")"
                        .to_owned()
                }
                Colliders::Whole(kind) => format!(
                    "the model now has one {} round all of it as its hit shape",
                    kind.name()
                ),
            }
        }
        None => {
            if doc.entry(&[], "colliders").is_none() {
                return Err(problem(
                    Code::MissingField,
                    "the recipe has no \"colliders\" to take away",
                ));
            }
            doc.remove_field(&[], "colliders").map_err(cant)?;
            "took \"colliders\" away: only parts with their own have a hit shape now".to_owned()
        }
    };
    Ok(Outcome {
        summary,
        touched: None,
    })
}

/// A number as a recipe keeps a hit shape's: to a tenth of a millimetre.
fn kept(number: f64) -> f64 {
    super::tidy_number(number, 4)
}

/// `set-collider`: a part's own hit shape.
pub(super) fn set_collider(
    doc: &mut Doc,
    part: &str,
    to: ColliderTo,
    model: Option<&Model>,
) -> Result<Outcome, Stop> {
    let found = find(doc, part)?;
    let table = found.path();
    let summary = match to {
        ColliderTo::Fitted => {
            if doc.entry(&table, "collider").is_none() {
                return Err(problem(
                    Code::MissingField,
                    format!(
                        "\"{part}\" has no hit shape of its own to take away: it follows \"colliders\" already"
                    ),
                ));
            }
            doc.remove_field(&table, "collider").map_err(cant)?;
            format!("\"{part}\" has no hit shape of its own now: it follows \"colliders\"")
        }
        ColliderTo::None => {
            set(doc, &table, "collider", Val::Str("none".to_owned()))?;
            format!("\"{part}\" has no hit shape now: a hit can't name it")
        }
        ColliderTo::Shape {
            kind,
            size,
            radius,
            length,
            center,
            along,
        } => {
            let numbers =
                size.is_some() || radius.is_some() || length.is_some() || center.is_some();
            check_numbers(kind, size, radius, length, along)?;
            if !numbers && along.is_none() {
                set(doc, &table, "collider", Val::Str(kind.name().to_owned()))?;
                format!(
                    "\"{part}\" has a {} for its hit shape, fitted to it",
                    kind.name()
                )
            } else {
                // What isn't given is the fitted shape's.
                let fitted = model
                    .and_then(|model| {
                        let built = model.part(part)?;
                        fitted_numbers(model, built, kind, along)
                    })
                    .ok_or_else(|| {
                        problem(
                            Code::Unsupported,
                            format!(
                                "the numbers of \"{part}\"'s hit shape can't be worked out while the recipe has mistakes: put them right first, or give every number"
                            ),
                        )
                    })?;
                let own = OwnHitShape {
                    kind,
                    center: center.unwrap_or(fitted.center),
                    size: size.unwrap_or(fitted.size),
                    radius: radius.unwrap_or(fitted.radius),
                    length: length.unwrap_or(fitted.length),
                    along,
                };
                if kind == HitKind::Capsule && own.length < 2.0 * own.radius {
                    return Err(problem(
                        Code::OutOfRange,
                        format!(
                            "a capsule's length is from end to end, round ends and all, so at least twice its radius ({}): not {}",
                            kept(2.0 * own.radius),
                            kept(own.length)
                        ),
                    ));
                }
                set(doc, &table, "collider", own_val(&own))?;
                format!(
                    "\"{part}\" has a {} of its own for its hit shape: {}",
                    kind.name(),
                    own_words(&own)
                )
            }
        }
    };
    Ok(Outcome {
        summary,
        touched: Some(found),
    })
}

/// Stops if a number doesn't belong to the shape, or is out of range.
fn check_numbers(
    kind: HitKind,
    size: Option<[f64; 3]>,
    radius: Option<f64>,
    length: Option<f64>,
    along: Option<usize>,
) -> Result<(), Stop> {
    let wrong = |field: &str| {
        Err(problem(
            Code::Conflict,
            format!(
                "a {} has no --{field}: a box has --size, a sphere --radius, a capsule --radius, --length and --along",
                kind.name()
            ),
        ))
    };
    match kind {
        HitKind::Box if radius.is_some() => return wrong("radius"),
        HitKind::Box if length.is_some() => return wrong("length"),
        HitKind::Sphere if size.is_some() => return wrong("size"),
        HitKind::Sphere if length.is_some() => return wrong("length"),
        HitKind::Capsule if size.is_some() => return wrong("size"),
        _ => {}
    }
    if along.is_some() && kind != HitKind::Capsule {
        return wrong("along");
    }
    let positive = size.is_none_or(|size| size.iter().all(|side| *side > 0.0 && *side <= 1000.0))
        && radius.is_none_or(|radius| radius > 0.0 && radius <= 1000.0)
        && length.is_none_or(|length| length > 0.0 && length <= 1000.0);
    if !positive {
        return Err(problem(
            Code::OutOfRange,
            "a hit shape's sizes are in metres, more than 0 and at most 1000",
        ));
    }
    Ok(())
}

/// A hit shape of a part's own, as the recipe writes it:
/// `{ shape = "sphere", radius = 0.25 }`.
fn own_val(own: &OwnHitShape) -> Val {
    let mut fields = vec![("shape".to_owned(), Val::Str(own.kind.name().to_owned()))];
    if own.center.iter().any(|number| kept(*number) != 0.0) {
        fields.push(("center".to_owned(), Val::three(own.center.map(kept))));
    }
    match own.kind {
        HitKind::Box => fields.push(("size".to_owned(), Val::three(own.size.map(kept)))),
        HitKind::Sphere => fields.push(("radius".to_owned(), Val::Num(kept(own.radius)))),
        HitKind::Capsule => {
            fields.push(("radius".to_owned(), Val::Num(kept(own.radius))));
            fields.push(("length".to_owned(), Val::Num(kept(own.length))));
            if let Some(along) = own.along {
                fields.push(("along".to_owned(), Val::Str(Limits::AXES[along].to_owned())));
            }
        }
    }
    Val::Table(fields)
}

/// The same, for a sentence: `radius 0.25`.
fn own_words(own: &OwnHitShape) -> String {
    let number = |value: f64| format!("{}", kept(value));
    match own.kind {
        HitKind::Box => format!(
            "size {} x {} x {}",
            number(own.size[0]),
            number(own.size[1]),
            number(own.size[2])
        ),
        HitKind::Sphere => format!("radius {}", number(own.radius)),
        HitKind::Capsule => format!("radius {}, {} long", number(own.radius), number(own.length)),
    }
}
