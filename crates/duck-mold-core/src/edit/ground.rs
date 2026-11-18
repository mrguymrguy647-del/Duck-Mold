//! `fit-ground`: stands an animation's poses on the ground.
//!
//! A walk copied onto a model with shorter legs sinks its feet into the
//! ground at each step. For each pose the animation's keys use, this
//! stands the model in that pose, finds its lowest point, and changes
//! the up-and-down move of the part every other part hangs from (the
//! body) so that the lowest point is on the ground. Only that one
//! number of each pose changes.

use std::path::Path;

use super::motion::{add_key, add_pose, joint_at, require_animation, set_pose};
use super::{Outcome, Stop, problem, tidy_number};
use crate::issue::Code;
use crate::model::{Model, Part};
use crate::motion::{Motion, frame_times, posed};
use crate::recipe::REST;

use super::doc::Doc;

/// Less than this far from the ground is on it: a twentieth of a
/// millimetre, what a recipe's numbers keep.
const ON_GROUND: f64 = 0.00005;
/// A body moved up more than this in a pose is lifted on purpose: a leap.
const LIFTED: f64 = 0.0005;
/// Sinking deeper than this between keys gets a key of its own: 3 mm.
pub const DIP: f64 = 0.003;
/// How many keys fitting between keys adds, at most.
const MOST_KEYS_ADDED: usize = 6;
/// How near a key added between keys may be to the keys beside it.
const KEY_GAP: f64 = 0.05;

/// Every part of the model, however deep.
fn every_part(model: &Model) -> Vec<&Part> {
    fn gather<'a>(parts: &'a [Part], into: &mut Vec<&'a Part>) {
        for part in parts {
            into.push(part);
            gather(&part.parts, into);
        }
    }
    let mut all = Vec::new();
    gather(&model.parts, &mut all);
    all
}

/// The part a part hangs from at the top of the tree of joins.
fn root_of<'a>(model: &'a Model, part: &'a Part) -> &'a Part {
    let mut now = part;
    while let Some(parent) = now.parent.as_deref().and_then(|path| model.part(path)) {
        now = parent;
    }
    now
}

/// The part every other part hangs from: the one named, or the only
/// one that has parts hanging from it.
fn body_of<'a>(model: &'a Model, named: Option<&str>) -> Result<&'a Part, Stop> {
    let all = every_part(model);
    let hanging = |root: &Part| {
        all.iter()
            .filter(|part| part.path != root.path && root_of(model, part).path == root.path)
            .count()
    };
    let roots: Vec<&Part> = model
        .parts
        .iter()
        .filter(|part| part.parent.is_none())
        .collect();
    if let Some(name) = named {
        let Some(part) = model.part(name) else {
            return Err(problem(
                Code::UnknownPart,
                format!("the model has no part \"{name}\""),
            )
            .got(name)
            .one_of(name, roots.iter().map(|part| part.path.as_str())));
        };
        if part.parent.is_some() {
            let top = root_of(model, part);
            return Err(problem(
                Code::Conflict,
                format!(
                    "\"{name}\" hangs from \"{}\": give the part the others hang from, like --part {}",
                    top.path, top.path
                ),
            ));
        }
        return Ok(part);
    }
    let with_parts: Vec<&Part> = roots
        .iter()
        .copied()
        .filter(|root| hanging(root) > 0)
        .collect();
    match (with_parts.as_slice(), roots.as_slice()) {
        ([one], _) | ([], [one]) => Ok(one),
        (many, _) => {
            let names: Vec<&str> = if many.is_empty() {
                roots.iter().map(|part| part.path.as_str()).collect()
            } else {
                many.iter().map(|part| part.path.as_str()).collect()
            };
            Err(problem(
                Code::Conflict,
                format!(
                    "more than one part has others hanging from it ({}): say which to move up and down, like --part {}",
                    names.join(", "),
                    names[0]
                ),
            ))
        }
    }
}

/// The part every other part hangs from, found by itself, if there is
/// one: what `fit-ground` would move. `None` for a model with no such
/// part (free wheels on a cart).
pub fn body_part(model: &Model) -> Option<String> {
    body_of(model, None).ok().map(|part| part.path.clone())
}

/// The lowest point of the part and everything that hangs from it.
fn lowest(model: &Model, body: &str) -> Option<f64> {
    every_part(model)
        .into_iter()
        .filter(|part| part.parts.is_empty() && root_of(model, part).path == body)
        .flat_map(|part| model.faces[part.faces.clone()].iter())
        .flat_map(|face| face.corners.iter().map(|corner| corner.y))
        .reduce(f64::min)
}

/// `fit-ground`.
pub(super) fn fit_ground(
    doc: &mut Doc,
    animation: &str,
    named: Option<&str>,
    all: bool,
    keys_only: bool,
    model: Option<&Model>,
    path: &Path,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    let Some(model) = model else {
        return Err(problem(
            Code::Unsupported,
            "the poses can't be stood on the ground while the recipe has mistakes: put them right first",
        ));
    };
    let Some(played) = model.animation(animation) else {
        return Err(problem(
            Code::Unsupported,
            format!(
                "the animation \"{animation}\" can't be played while it has mistakes: put them right first"
            ),
        ));
    };
    let body = body_of(model, named)?;
    let mut poses: Vec<String> = Vec::new();
    for key in &played.keys {
        if key.pose != REST && !poses.contains(&key.pose) {
            poses.push(key.pose.clone());
        }
    }
    if poses.is_empty() {
        return Err(problem(
            Code::Conflict,
            format!(
                "the animation \"{animation}\" only uses rest: the model as it is built, which a pose can't change"
            ),
        ));
    }

    let mut fitted: Vec<String> = Vec::new();
    let mut leaps: Vec<String> = Vec::new();
    for name in &poses {
        let Some(pose) = model.pose(name) else {
            continue;
        };
        let (_, moved) = joint_at(doc, name, &body.path).unwrap_or(([0.0; 3], [0.0; 3]));
        if moved[1] > LIFTED && !all {
            leaps.push(name.clone());
            continue;
        }
        let standing = posed(model, &pose.joints);
        let Some(low) = lowest(&standing, &body.path) else {
            continue;
        };
        if low.abs() < ON_GROUND {
            continue;
        }
        let height = tidy_number(moved[1] - low, 4);
        set_pose(
            doc,
            name,
            &body.path,
            None,
            Some([moved[0], height, moved[2]]),
            false,
        )?;
        fitted.push(format!(
            "\"{name}\" ({} {})",
            if low < 0.0 { "up" } else { "down" },
            tidy_number(low.abs(), 4)
        ));
    }

    // What else uses the poses that changed.
    let changed: Vec<&str> = poses
        .iter()
        .filter(|name| {
            fitted
                .iter()
                .any(|said| said.starts_with(&format!("\"{name}\"")))
        })
        .map(String::as_str)
        .collect();
    let mut others: Vec<&str> = model
        .animations
        .iter()
        .filter(|other| {
            other.name != animation
                && other
                    .keys
                    .iter()
                    .any(|key| changed.contains(&key.pose.as_str()))
        })
        .map(|other| other.name.as_str())
        .collect();
    others.dedup();

    let mut summary = if fitted.is_empty() {
        format!("the poses of \"{animation}\" stand on the ground already")
    } else {
        format!(
            "stood the poses of \"{animation}\" on the ground, moving \"{}\" in {}",
            body.path,
            fitted.join(", ")
        )
    };
    if !leaps.is_empty() {
        summary.push_str(&format!(
            "; left {} alone: {} the body (a leap; --all fits {} too)",
            leaps
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(" and "),
            if leaps.len() == 1 {
                "it lifts"
            } else {
                "they lift"
            },
            if leaps.len() == 1 { "it" } else { "them" }
        ));
    }
    if !others.is_empty() {
        summary.push_str(&format!(
            "; {} {} the same poses, so {} changed too",
            others
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(" and "),
            if others.len() == 1 { "uses" } else { "use" },
            if others.len() == 1 {
                "it is"
            } else {
                "they are"
            }
        ));
    }
    // Between the keys: where the smooth curve between two keys still
    // sinks into the ground, a key is added there, standing on it.
    let display = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let added = if keys_only {
        Vec::new()
    } else {
        fit_between_keys(doc, animation, &body.path, path, &display)?
    };
    if !added.is_empty() {
        let (times, depths): (Vec<String>, Vec<String>) = added
            .iter()
            .map(|(at, sank)| {
                (
                    format!("{} s", tidy_number(*at, 2)),
                    format!("{} mm", tidy_number(sank * 1000.0, 0)),
                )
            })
            .unzip();
        summary.push_str(&format!(
            "; added {} between keys, at {}, where it sank {}",
            if added.len() == 1 {
                "a key".to_owned()
            } else {
                format!("{} keys", added.len())
            },
            and_list(&times),
            and_list(&depths)
        ));
    }
    if (!fitted.is_empty() || !added.is_empty())
        && let (Some(new_model), _) = super::build(&doc.text, doc.syntax, path, &display)
        && let Some(played) = new_model.animation(animation)
    {
        let moving = Motion::of(&new_model, played);
        let mut worst: Option<(f64, f64)> = None;
        for at in frame_times(played.length()) {
            let frame = posed(&new_model, &moving.joints_at(at));
            if let Some(low) = lowest(&frame, &body.path)
                && worst.is_none_or(|(known, _)| low < known)
            {
                worst = Some((low, at));
            }
        }
        match worst {
            Some((low, at)) if low < -ON_GROUND => {
                summary.push_str(&format!(
                    "; between keys it now sinks at most {} (at {} s)",
                    tidy_number(-low, 4),
                    tidy_number(at, 3)
                ));
            }
            _ => summary.push_str("; it doesn't sink into the ground between keys either"),
        }
    }
    Ok(Outcome {
        summary,
        touched: None,
    })
}

/// "a", "a and b", "a, b and c".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The frame between two keys where the model sinks deepest into the
/// ground, as `(how low, when)`. Only frames between the first key and
/// the last are looked at: the last key of a loop and its first are the
/// same moment.
pub fn deepest_between_keys(model: &Model, animation: &str, body: &str) -> Option<(f64, f64)> {
    let played = model.animation(animation)?;
    let (first, last) = (played.keys.first()?.at, played.keys.last()?.at);
    let moving = Motion::of(model, played);
    let mut worst: Option<(f64, f64)> = None;
    for at in frame_times(played.length()) {
        if at <= first || at >= last {
            continue;
        }
        let frame = posed(model, &moving.joints_at(at));
        if let Some(low) = lowest(&frame, body)
            && worst.is_none_or(|(known, _)| low < known)
        {
            worst = Some((low, at));
        }
    }
    worst
}

/// Adds keys where the animation sinks more than [`DIP`] between keys,
/// the deepest first, at most [`MOST_KEYS_ADDED`]: each a new pose, made
/// from where every part is at that moment, with the body moved up to
/// stand on the ground. Gives when each was added and how deep it sank.
fn fit_between_keys(
    doc: &mut Doc,
    animation: &str,
    body: &str,
    path: &Path,
    display: &str,
) -> Result<Vec<(f64, f64)>, Stop> {
    let tidy =
        |numbers: [f64; 3], step: f64| numbers.map(|number| (number / step).round() * step + 0.0);
    let mut added = Vec::new();
    for _ in 0..MOST_KEYS_ADDED {
        let (Some(model), _) = super::build(&doc.text, doc.syntax, path, display) else {
            break;
        };
        let Some(played) = model.animation(animation) else {
            break;
        };
        let Some((low, at)) = deepest_between_keys(&model, animation, body) else {
            break;
        };
        if low >= -DIP {
            break;
        }
        // When: to a hundredth of a second, and not too near the keys
        // beside it.
        let before = played
            .keys
            .iter()
            .map(|key| key.at)
            .filter(|time| *time <= at)
            .fold(f64::NEG_INFINITY, f64::max);
        let after = played
            .keys
            .iter()
            .map(|key| key.at)
            .filter(|time| *time > at)
            .fold(f64::INFINITY, f64::min);
        if after - before < KEY_GAP * 2.0 {
            break;
        }
        let (earliest, latest) = (
            ((before + KEY_GAP) * 100.0).ceil() / 100.0,
            ((after - KEY_GAP) * 100.0).floor() / 100.0,
        );
        if earliest > latest {
            break;
        }
        let when = ((at * 100.0).round() / 100.0).clamp(earliest, latest);
        // The pose: every part where it is then, its numbers tidied as a
        // recipe writes them; then the body lifted to stand on the ground.
        let mut joints = Motion::of(&model, played).joints_at(when);
        for joint in &mut joints {
            joint.turn = tidy(joint.turn, 0.1);
            joint.move_by = tidy(joint.move_by, 0.00001);
        }
        let lifted = lowest(&posed(&model, &joints), body).unwrap_or(0.0);
        let taken: Vec<String> = model.poses.iter().map(|pose| pose.name.clone()).collect();
        let name = super::unused_name(&format!("{animation}_2"), &taken);
        add_pose(doc, &name, None)?;
        let mut body_done = false;
        for joint in &joints {
            let mut move_by = joint.move_by;
            if joint.part == body {
                move_by[1] -= lifted;
                body_done = true;
            }
            let (turn, move_by) = (joint.turn, tidy(move_by, 0.00001));
            if turn == [0.0; 3] && move_by == [0.0; 3] {
                continue;
            }
            set_pose(
                doc,
                &name,
                &joint.part,
                (turn != [0.0; 3]).then_some(turn),
                (move_by != [0.0; 3]).then_some(move_by),
                false,
            )?;
        }
        if !body_done && lifted.abs() >= ON_GROUND {
            set_pose(
                doc,
                &name,
                body,
                None,
                Some(tidy([0.0, -lifted, 0.0], 0.00001)),
                false,
            )?;
        }
        add_key(doc, animation, &name, when)?;
        added.push((when, -low));
    }
    Ok(added)
}
