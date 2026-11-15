//! The edits of Phase 5: how far a part may turn (`limits`), and, from
//! the later steps, poses and animations.
//!
//! They are kept apart from the other edits only because that file is
//! long; they work the same way, on the recipe's text, and change only
//! the lines they must.

use super::doc::{Doc, Step, Val, key, with};
use super::{Outcome, Stop, cant, find, problem, set, tidy_number};
use crate::issue::Code;
use crate::math::{Mat3, Vec3, vec3};
use crate::model::{Model, Part};
use crate::motion::directions;
use crate::recipe::{ANIMATION_FIELDS, KEY_FIELDS, LONGEST_ANIMATION, Limits, REST, Turns};
use crate::source::{Node, Syntax, Value};

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
                doc.remove_field(&table, "limits").map_err(cant)?;
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

// ---- poses -----------------------------------------------------------------------------

/// The group of tables the recipe keeps its poses in.
const POSES: &str = "poses";

/// The names of the recipe's poses, in the order they are written.
pub(super) fn pose_names(doc: &Doc) -> Vec<String> {
    doc.entries(&[key(POSES)])
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect()
}

/// A name a pose can have: a name like any other, and not `rest`.
fn check_pose_name(name: &str) -> Result<(), Stop> {
    if name == REST {
        return Err(problem(
            Code::Reserved,
            "\"rest\" is the model as the recipe builds it, and is always there: it can't be changed as a pose. To change it, change the parts themselves; or give the pose another name",
        ));
    }
    super::check_name(name)
}

/// Stops with the names there are, if the recipe has no pose `name`.
fn require_pose(doc: &Doc, name: &str) -> Result<(), Stop> {
    check_pose_name(name)?;
    let names = pose_names(doc);
    if names.iter().any(|known| known == name) {
        return Ok(());
    }
    Err(problem(
        Code::UnknownPose,
        format!("there is no pose \"{name}\" in this recipe"),
    )
    .got(name)
    .one_of(name, names))
}

/// Three numbers of a list node.
fn three_at(node: &Node) -> Option<[f64; 3]> {
    match &node.value {
        Value::Array(items) => match items.as_slice() {
            [x, y, z] => Some([
                x.value.as_number()?,
                y.value.as_number()?,
                z.value.as_number()?,
            ]),
            _ => None,
        },
        _ => None,
    }
}

/// What a pose says about a part, as it is written: its turn and its
/// move. `None` if the pose doesn't name the part (or the line can't be
/// read).
fn joint_at(doc: &Doc, pose: &str, part: &str) -> Option<([f64; 3], [f64; 3])> {
    let node = doc.node(&[key(POSES), key(pose), key(part)])?;
    match &node.value {
        Value::Array(_) => Some((three_at(node)?, [0.0; 3])),
        Value::Table(entries) => {
            let field = |name: &str| {
                entries
                    .iter()
                    .find(|entry| entry.key == name)
                    .and_then(|entry| three_at(&entry.node))
                    .unwrap_or([0.0; 3])
            };
            Some((field("turn"), field("move")))
        }
        _ => None,
    }
}

/// A part's line in a pose: its turn alone, or `{ turn = ..., move = ...
/// }` when it also moves.
fn joint_val(turn: [f64; 3], move_by: [f64; 3]) -> Val {
    if move_by == [0.0; 3] {
        return Val::three(turn);
    }
    let mut fields = Vec::new();
    if turn != [0.0; 3] {
        fields.push(("turn".to_owned(), Val::three(turn)));
    }
    fields.push(("move".to_owned(), Val::three(move_by)));
    Val::Table(fields)
}

/// Three numbers as a recipe writes them: `[0, 0, -150]`.
fn written(numbers: [f64; 3]) -> String {
    Val::three(numbers).write(Syntax::Toml)
}

/// `set-pose`: turns or moves a part in a pose, or puts it back at
/// rest. Makes the pose if it isn't there.
pub(super) fn set_pose(
    doc: &mut Doc,
    pose: &str,
    part: &str,
    turn: Option<[f64; 3]>,
    move_by: Option<[f64; 3]>,
    rest: bool,
) -> Result<Outcome, Stop> {
    check_pose_name(pose)?;
    // The part must be there (with the closest name, if it isn't).
    find(doc, part)?;
    let table = [key(POSES), key(pose)];
    let exists = doc.node(&table).is_some();
    let before = joint_at(doc, pose, part);
    if rest {
        if turn.is_some() || move_by.is_some() {
            return Err(problem(
                Code::Conflict,
                "--rest puts the part back at rest in the pose: leave out --turn and --move",
            ));
        }
        require_pose(doc, pose)?;
        if doc.entry(&table, part).is_some() {
            doc.remove_field(&table, part).map_err(cant)?;
        }
        return Ok(Outcome {
            summary: match before {
                Some(_) => format!("\"{part}\" is at rest in the pose \"{pose}\" again"),
                None => format!("\"{part}\" is at rest in the pose \"{pose}\" already"),
            },
            touched: None,
        });
    }
    if turn.is_none() && move_by.is_none() {
        return Err(problem(
            Code::MissingField,
            "nothing to set: give --turn x,y,z (degrees), --move x,y,z (metres), or --rest",
        ));
    }
    let (old_turn, old_move) = before.unwrap_or(([0.0; 3], [0.0; 3]));
    let new_turn = turn
        .unwrap_or(old_turn)
        .map(|degrees| tidy_number(degrees, 2));
    let new_move = move_by
        .unwrap_or(old_move)
        .map(|metres| tidy_number(metres, 4));
    if new_turn.iter().any(|degrees| degrees.abs() > 36_000.0) {
        return Err(problem(
            Code::OutOfRange,
            "a turn is between -36000 and 36000 degrees each way: that is a hundred times round",
        ));
    }
    if new_move.iter().any(|metres| metres.abs() > 10_000.0) {
        return Err(problem(
            Code::OutOfRange,
            "a move is between -10000 and 10000 metres each way",
        ));
    }
    let at_rest = new_turn == [0.0; 3] && new_move == [0.0; 3];
    if at_rest {
        // Nothing left to say about it: its line goes. (A pose that
        // isn't there yet is made all the same: it is the rest pose
        // under a name, ready for a key.)
        if !exists {
            doc.add_named_table(POSES, pose, &[]).map_err(cant)?;
        } else if doc.entry(&table, part).is_some() {
            doc.remove_field(&table, part).map_err(cant)?;
        }
    } else {
        let value = joint_val(new_turn, new_move);
        if exists {
            doc.set_field(&table, part, &value, &[]).map_err(cant)?;
        } else {
            doc.add_named_table(POSES, pose, &[(part.to_owned(), value)])
                .map_err(cant)?;
        }
    }
    let mut did = Vec::new();
    if turn.is_some() {
        did.push(format!("turned \"{part}\" to {}", written(new_turn)));
    }
    if move_by.is_some() {
        did.push(match turn {
            Some(_) => format!("moved it by {}", written(new_move)),
            None => format!("moved \"{part}\" by {}", written(new_move)),
        });
    }
    Ok(Outcome {
        summary: format!(
            "{} in the pose \"{pose}\"{}{}",
            did.join(" and "),
            if exists { "" } else { ", a new pose" },
            if at_rest { ": it is at rest there" } else { "" }
        ),
        touched: None,
    })
}

/// What a pose says, as it is written: each part's line, copied as it
/// is.
fn pose_lines(doc: &Doc, pose: &str) -> Vec<(String, Val)> {
    let table = [key(POSES), key(pose)];
    doc.entries(&table)
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| {
            let value = doc.raw(&with(&table, key(&entry.key)))?;
            Some((entry.key.clone(), value))
        })
        .collect()
}

/// `add-pose`: a new pose, empty (every part at rest) or a copy of
/// another.
pub(super) fn add_pose(doc: &mut Doc, name: &str, from: Option<&str>) -> Result<Outcome, Stop> {
    check_pose_name(name)?;
    if pose_names(doc).iter().any(|known| known == name) {
        return Err(problem(
            Code::DuplicateName,
            format!("there is already a pose called \"{name}\": change it with set-pose, or choose another name"),
        )
        .got(name));
    }
    let lines = match from {
        // A copy of `rest` is a pose with every part at rest.
        Some(from) if from != REST => {
            require_pose(doc, from)?;
            pose_lines(doc, from)
        }
        _ => Vec::new(),
    };
    doc.add_named_table(POSES, name, &lines).map_err(cant)?;
    Ok(Outcome {
        summary: match from {
            Some(from) => format!("added the pose \"{name}\", a copy of \"{from}\""),
            None => format!(
                "added the pose \"{name}\": every part is at rest in it until set-pose turns one"
            ),
        },
        touched: None,
    })
}

/// `rename-pose`: a pose gets another name, and everything that names
/// it follows.
pub(super) fn rename_pose(doc: &mut Doc, pose: &str, new_name: &str) -> Result<Outcome, Stop> {
    require_pose(doc, pose)?;
    check_pose_name(new_name)?;
    if pose_names(doc).iter().any(|known| known == new_name) {
        return Err(problem(
            Code::DuplicateName,
            format!("there is already a pose called \"{new_name}\""),
        )
        .got(new_name));
    }
    doc.rename_named_table(POSES, pose, new_name)
        .map_err(cant)?;
    // The keys that use it follow.
    let mut keys = 0;
    for animation in animation_names(doc) {
        for (at, (_, used)) in keys_of(doc, &animation)?.iter().enumerate() {
            if used == pose {
                doc.set_value(
                    &with(&key_at(&animation, at), key("pose")),
                    &Val::Str(new_name.to_owned()),
                )
                .map_err(cant)?;
                keys += 1;
            }
        }
    }
    Ok(Outcome {
        summary: match keys {
            0 => format!("renamed the pose \"{pose}\" to \"{new_name}\""),
            1 => format!(
                "renamed the pose \"{pose}\" to \"{new_name}\", and in the key that uses it"
            ),
            _ => format!(
                "renamed the pose \"{pose}\" to \"{new_name}\", and in the {keys} keys that use it"
            ),
        },
        touched: None,
    })
}

/// `remove-pose`: takes a pose away. While a key of an animation uses
/// it, that is refused, unless `with_keys`: then those keys go too.
pub(super) fn remove_pose(doc: &mut Doc, pose: &str, with_keys: bool) -> Result<Outcome, Stop> {
    require_pose(doc, pose)?;
    // The keys that use it: each animation with their places.
    let mut users: Vec<(String, Vec<usize>)> = Vec::new();
    for animation in animation_names(doc) {
        let places: Vec<usize> = keys_of(doc, &animation)?
            .iter()
            .enumerate()
            .filter(|(_, (_, used))| used == pose)
            .map(|(at, _)| at)
            .collect();
        if !places.is_empty() {
            users.push((animation, places));
        }
    }
    let keys: usize = users.iter().map(|(_, places)| places.len()).sum();
    let where_used = || {
        let animations: Vec<String> = users
            .iter()
            .map(|(animation, places)| {
                let numbers: Vec<String> =
                    places.iter().map(|place| (place + 1).to_string()).collect();
                format!(
                    "\"{animation}\" ({} {})",
                    if places.len() == 1 { "key" } else { "keys" },
                    numbers.join(", ")
                )
            })
            .collect();
        format!(
            "the {} {}",
            if users.len() == 1 {
                "animation"
            } else {
                "animations"
            },
            animations.join(" and ")
        )
    };
    if keys > 0 && !with_keys {
        return Err(problem(
            Code::Conflict,
            format!(
                "the pose \"{pose}\" is used by {}: change those keys with set-key first, or take them away with the pose: remove-pose {pose} --with-keys",
                where_used()
            ),
        ));
    }
    let used = where_used();
    for (animation, places) in &users {
        drop_keys(doc, animation, places)?;
    }
    doc.remove_named_table(POSES, pose).map_err(cant)?;
    Ok(Outcome {
        summary: match keys {
            0 => format!("removed the pose \"{pose}\""),
            1 => format!("removed the pose \"{pose}\", and the key that used it, in {used}"),
            _ => {
                format!("removed the pose \"{pose}\", and the {keys} keys that used it, in {used}")
            }
        },
        touched: None,
    })
}

/// A part's path with `left` and `right` swapped in every name of it:
/// `left_arm/elbow` becomes `right_arm/elbow`.
fn other_side(path: &str) -> String {
    path.split('/')
        .map(super::swap_sides)
        .collect::<Vec<_>>()
        .join("/")
}

/// What a joint of `part` becomes on `other`, its mirror image left to
/// right: the turn and the move that make the mirror image of the
/// motion.
fn mirrored_joint(
    part: &Part,
    other: &Part,
    turn: [f64; 3],
    move_by: [f64; 3],
) -> ([f64; 3], [f64; 3]) {
    let mirror = Mat3::stretch(vec3(-1.0, 1.0, 1.0));
    // From the directions `part` turns in, through the mirror, to the
    // directions `other` turns in.
    let (from, to) = (directions(&part.axes), directions(&other.axes));
    let across = to.transposed() * mirror * from;
    let rows = across.rows;
    let off_diagonal = (0..3)
        .flat_map(|row| (0..3).map(move |column| (row, column)))
        .filter(|(row, column)| row != column)
        .map(|(row, column)| rows[row][column].abs())
        .fold(0.0, f64::max);
    let new_turn = if off_diagonal < 1e-9 {
        // The usual case: the two parts' directions are the same, or
        // mirror images. Each number keeps its size (a wheel turned
        // three times round still is) and may change its sign: a turn
        // around one direction goes the other way when just one of the
        // other two directions is flipped.
        let sign = [
            rows[0][0].signum(),
            rows[1][1].signum(),
            rows[2][2].signum(),
        ];
        [
            turn[0] * sign[1] * sign[2],
            turn[1] * sign[0] * sign[2],
            turn[2] * sign[0] * sign[1],
        ]
    } else {
        (across * Mat3::from_rotate(turn) * across.transposed()).to_rotate()
    };
    let moved = mirror * (part.axes * Vec3::from(move_by));
    let new_move = match other.axes.inverse() {
        Some(back) => (back * moved).to_array(),
        None => (across * Vec3::from(move_by)).to_array(),
    };
    (
        new_turn.map(|degrees| tidy_number(degrees, 2)),
        new_move.map(|metres| tidy_number(metres, 4)),
    )
}

/// `mirror-pose`: the mirror image of a pose, left to right, as a pose
/// of its own (`as_name`) or in its place.
pub(super) fn mirror_pose(
    doc: &mut Doc,
    pose: &str,
    as_name: Option<&str>,
    model: Option<&Model>,
) -> Result<Outcome, Stop> {
    require_pose(doc, pose)?;
    if let Some(name) = as_name {
        check_pose_name(name)?;
    }
    let Some(model) = model else {
        return Err(problem(
            Code::Unsupported,
            "the recipe has mistakes, so where its parts are isn't known: fix them first",
        ));
    };
    let table = [key(POSES), key(pose)];
    let parts: Vec<String> = doc
        .entries(&table)
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect();
    let mut lines: Vec<(String, Val)> = Vec::new();
    for part in &parts {
        let other = other_side(part);
        let (Some(here), Some(there)) = (model.part(part), model.part(&other)) else {
            return Err(problem(
                Code::UnknownPart,
                format!(
                    "the pose turns \"{part}\", and its mirror image would turn \"{other}\", but the model has no part with that name"
                ),
            )
            .got(other.as_str()));
        };
        let Some((turn, move_by)) = joint_at(doc, pose, part) else {
            continue;
        };
        let (turn, move_by) = mirrored_joint(here, there, turn, move_by);
        if turn != [0.0; 3] || move_by != [0.0; 3] {
            lines.push((other, joint_val(turn, move_by)));
        }
    }
    let target = as_name.unwrap_or(pose);
    let replaced = as_name.is_some_and(|name| pose_names(doc).iter().any(|known| known == name));
    if as_name.is_none() || replaced {
        // In place: its lines go, and the new ones come, where it stands.
        let target_table = [key(POSES), key(target)];
        let old: Vec<String> = doc
            .entries(&target_table)
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        for line in old {
            doc.remove_field(&target_table, &line).map_err(cant)?;
        }
        for (part, value) in &lines {
            doc.set_field(&target_table, part, value, &[])
                .map_err(cant)?;
        }
    } else {
        doc.add_named_table(POSES, target, &lines).map_err(cant)?;
    }
    Ok(Outcome {
        summary: match as_name {
            None => format!("mirrored the pose \"{pose}\", left to right"),
            Some(name) if replaced => format!(
                "mirrored the pose \"{pose}\" as \"{name}\", in place of what \"{name}\" was"
            ),
            Some(name) => format!("mirrored the pose \"{pose}\" as \"{name}\", a new pose"),
        },
        touched: None,
    })
}

/// After a part is renamed: every pose line for it, and for the parts
/// inside it, is renamed too. `olds` are the paths it had (one, or one
/// for every part that uses the definition it is in).
pub(super) fn poses_follow_rename(
    doc: &mut Doc,
    olds: &[String],
    new_name: &str,
) -> Result<(), Stop> {
    for pose in pose_names(doc) {
        let table = [key(POSES), key(&pose)];
        let lines: Vec<String> = doc
            .entries(&table)
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        for line in lines {
            for old in olds {
                let under = line
                    .strip_prefix(old.as_str())
                    .filter(|rest| rest.is_empty() || rest.starts_with('/'));
                let Some(rest) = under else {
                    continue;
                };
                let new = match old.rsplit_once('/') {
                    Some((around, _)) => format!("{around}/{new_name}{rest}"),
                    None => format!("{new_name}{rest}"),
                };
                doc.rename_field(&table, &line, &new).map_err(cant)?;
                break;
            }
        }
    }
    Ok(())
}

/// After parts are removed: every pose line for them, and for the parts
/// inside them, goes too. Says how many lines went.
pub(super) fn poses_forget(doc: &mut Doc, gone: &[String]) -> Result<usize, Stop> {
    let mut count = 0;
    for pose in pose_names(doc) {
        let table = [key(POSES), key(&pose)];
        let lines: Vec<String> = doc
            .entries(&table)
            .unwrap_or_default()
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        for line in lines {
            let is_gone = gone.iter().any(|old| {
                line.strip_prefix(old.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
            });
            if is_gone {
                doc.remove_field(&table, &line).map_err(cant)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

// ---- animations ------------------------------------------------------------------------

/// The group of tables the recipe keeps its animations in.
const ANIMATIONS: &str = "animations";

/// The names of the recipe's animations, in the order they are written.
pub(super) fn animation_names(doc: &Doc) -> Vec<String> {
    doc.entries(&[key(ANIMATIONS)])
        .unwrap_or_default()
        .iter()
        .map(|entry| entry.key.clone())
        .collect()
}

/// Stops with the names there are, if the recipe has no animation
/// `name`.
fn require_animation(doc: &Doc, name: &str) -> Result<(), Stop> {
    let names = animation_names(doc);
    if names.iter().any(|known| known == name) {
        return Ok(());
    }
    Err(problem(
        Code::UnknownAnimation,
        format!("there is no animation \"{name}\" in this recipe"),
    )
    .got(name)
    .one_of(name, names))
}

/// Stops if the recipe has no pose `name` (`rest` is always one).
fn require_any_pose(doc: &Doc, name: &str) -> Result<(), Stop> {
    if name == REST {
        return Ok(());
    }
    let names = pose_names(doc);
    if names.iter().any(|known| known == name) {
        return Ok(());
    }
    Err(problem(
        Code::UnknownPose,
        format!("there is no pose \"{name}\" in this recipe"),
    )
    .got(name)
    .one_of(name, std::iter::once(REST.to_owned()).chain(names)))
}

/// Where an animation's list of keys is.
fn keys_at(animation: &str) -> Vec<Step> {
    vec![key(ANIMATIONS), key(animation), key("keys")]
}

/// Where one key of an animation is, counted from 0.
fn key_at(animation: &str, at: usize) -> Vec<Step> {
    with(&keys_at(animation), Step::Index(at))
}

/// An animation's keys as they are written: each one's time and pose.
fn keys_of(doc: &Doc, animation: &str) -> Result<Vec<(f64, String)>, Stop> {
    let list = keys_at(animation);
    let Some(node) = doc.node(&list) else {
        return Ok(Vec::new());
    };
    let unreadable = || {
        problem(
            Code::TypeMismatch,
            format!(
                "the keys of the animation \"{animation}\" can't be read: each is written like {{ at = 0.4, pose = \"wave\" }}. Put them right in the recipe first"
            ),
        )
    };
    let Value::Array(items) = &node.value else {
        return Err(unreadable());
    };
    items
        .iter()
        .map(|item| {
            let Value::Table(entries) = &item.value else {
                return Err(unreadable());
            };
            let field = |name: &str| entries.iter().find(|entry| entry.key == name);
            let at = field("at").and_then(|entry| entry.node.value.as_number());
            let pose = field("pose").and_then(|entry| match &entry.node.value {
                Value::Str(pose) => Some(pose.clone()),
                _ => None,
            });
            match (at, pose) {
                (Some(at), Some(pose)) => Ok((at, pose)),
                _ => Err(unreadable()),
            }
        })
        .collect()
}

/// A key as it is written: `{ at = 0.4, pose = "wave" }`, and its
/// events if it has any: `event = "footstep"`.
fn key_val(at: f64, pose: &str, events: &[String]) -> Val {
    let mut fields = vec![
        ("at".to_owned(), Val::Num(at)),
        ("pose".to_owned(), Val::Str(pose.to_owned())),
    ];
    if let Some(events) = events_val(events) {
        fields.push(("event".to_owned(), events));
    }
    Val::Table(fields)
}

/// Events as a key writes them: one name, or a list of them. `None` for
/// none.
fn events_val(events: &[String]) -> Option<Val> {
    match events {
        [] => None,
        [one] => Some(Val::Str(one.clone())),
        many => Some(Val::List(
            many.iter().map(|name| Val::Str(name.clone())).collect(),
        )),
    }
}

/// The events of each of an animation's keys, as they are written.
fn events_of(doc: &Doc, animation: &str) -> Vec<Vec<String>> {
    let Some(node) = doc.node(&keys_at(animation)) else {
        return Vec::new();
    };
    let Value::Array(items) = &node.value else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let Value::Table(entries) = &item.value else {
                return Vec::new();
            };
            match entries
                .iter()
                .find(|entry| entry.key == "event")
                .map(|entry| &entry.node.value)
            {
                Some(Value::Str(one)) => vec![one.clone()],
                Some(Value::Array(many)) => many
                    .iter()
                    .filter_map(|item| match &item.value {
                        Value::Str(name) => Some(name.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        })
        .collect()
}

/// `set-event`: a name on a key, for a game to hear when the animation
/// passes it.
pub(super) fn set_event(
    doc: &mut Doc,
    animation: &str,
    number: usize,
    name: &str,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    super::check_name(name)?;
    let keys = keys_of(doc, animation)?;
    let place = key_number(animation, &keys, number)?;
    let mut events = events_of(doc, animation)
        .get(place)
        .cloned()
        .unwrap_or_default();
    if events.iter().any(|known| known == name) {
        return Err(problem(
            Code::Conflict,
            format!(
                "key {number} of \"{animation}\" has the event \"{name}\" already: it is heard once each time the animation passes the key"
            ),
        ));
    }
    events.push(name.to_owned());
    let value = events_val(&events).expect("one at least");
    doc.set_field(&key_at(animation, place), "event", &value, &KEY_FIELDS)
        .map_err(cant)?;
    Ok(Outcome {
        summary: format!(
            "key {number} of \"{animation}\", at {} (the pose \"{}\"), has the event \"{name}\"{}",
            seconds(keys[place].0),
            keys[place].1,
            if events.len() > 1 {
                format!(" now too: {}", events.join(", "))
            } else {
                String::new()
            }
        ),
        touched: None,
    })
}

/// `remove-event`: takes one event off a key, or every one.
pub(super) fn remove_event(
    doc: &mut Doc,
    animation: &str,
    number: usize,
    name: Option<&str>,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    let keys = keys_of(doc, animation)?;
    let place = key_number(animation, &keys, number)?;
    let mut events = events_of(doc, animation)
        .get(place)
        .cloned()
        .unwrap_or_default();
    if events.is_empty() {
        return Err(problem(
            Code::MissingField,
            format!("key {number} of \"{animation}\" has no events to take away"),
        ));
    }
    let gone = match name {
        Some(name) => {
            let Some(at) = events.iter().position(|known| known == name) else {
                return Err(problem(
                    Code::UnknownEvent,
                    format!(
                        "key {number} of \"{animation}\" has no event \"{name}\": it has {}",
                        events
                            .iter()
                            .map(|known| format!("\"{known}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
                .got(name)
                .one_of(name, events.iter().map(String::as_str)));
            };
            vec![events.remove(at)]
        }
        None => std::mem::take(&mut events),
    };
    let table = key_at(animation, place);
    match events_val(&events) {
        Some(value) => doc.set_field(&table, "event", &value, &KEY_FIELDS),
        None => doc.remove_field(&table, "event"),
    }
    .map_err(cant)?;
    Ok(Outcome {
        summary: format!(
            "took {} off key {number} of \"{animation}\"",
            gone.iter()
                .map(|name| format!("the event \"{name}\""))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        touched: None,
    })
}

/// A time as a sentence says it: `0.4 s`.
fn seconds(at: f64) -> String {
    format!("{} s", tidy_number(at, 4))
}

/// A time as a recipe keeps it: to a tenth of a thousandth of a second.
fn tidy_time(at: f64) -> f64 {
    tidy_number(at, 4)
}

/// Two times that are the same key's.
fn same_time(one: f64, other: f64) -> bool {
    (one - other).abs() < 0.00005
}

/// Stops unless `at` is a time a key can have.
fn check_time(at: f64) -> Result<(), Stop> {
    if at.is_finite() && (0.0..=LONGEST_ANIMATION).contains(&at) {
        return Ok(());
    }
    Err(problem(
        Code::OutOfRange,
        format!(
            "a key's time is in seconds from the animation's start, from 0 to {LONGEST_ANIMATION}"
        ),
    )
    .got(at))
}

/// The key `number` of an animation, counted from 1 as `inspect` prints
/// them: its place counted from 0, or a stop that says how many there
/// are.
fn key_number(animation: &str, keys: &[(f64, String)], number: usize) -> Result<usize, Stop> {
    if (1..=keys.len()).contains(&number) {
        return Ok(number - 1);
    }
    Err(problem(
        Code::OutOfRange,
        match keys.len() {
            0 => format!("the animation \"{animation}\" has no keys"),
            1 => format!("the animation \"{animation}\" has one key: key 1"),
            count => format!(
                "the animation \"{animation}\" has {count} keys: they are numbered from 1 to {count}"
            ),
        },
    )
    .got(number))
}

/// `add-animation`: a new animation, with no keys yet.
pub(super) fn add_animation(doc: &mut Doc, name: &str, looping: bool) -> Result<Outcome, Stop> {
    super::check_name(name)?;
    if animation_names(doc).iter().any(|known| known == name) {
        return Err(problem(
            Code::DuplicateName,
            format!("there is already an animation called \"{name}\": give it keys with add-key, or choose another name"),
        )
        .got(name));
    }
    let mut fields = Vec::new();
    if looping {
        fields.push(("loop".to_owned(), Val::Raw("true".to_owned())));
    }
    fields.push(("keys".to_owned(), Val::List(Vec::new())));
    doc.add_named_table(ANIMATIONS, name, &fields)
        .map_err(cant)?;
    Ok(Outcome {
        summary: format!(
            "added the animation \"{name}\"{}: it has no keys yet; give it its first with add-key {name} <pose> --at 0",
            if looping { ", which loops" } else { "" }
        ),
        touched: None,
    })
}

/// `set-animation`: whether an animation loops, whether it is smooth,
/// and how long it lasts.
pub(super) fn set_animation(
    doc: &mut Doc,
    animation: &str,
    looping: Option<bool>,
    smooth: Option<bool>,
    length: Option<f64>,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    if looping.is_none() && smooth.is_none() && length.is_none() {
        return Err(problem(
            Code::MissingField,
            "nothing to set: give --loop true or false, --smooth true or false, --length in seconds, or more than one",
        ));
    }
    let table = [key(ANIMATIONS), key(animation)];
    let mut done = Vec::new();
    // Each is written only when it isn't what an animation is anyway: it
    // doesn't loop, and it is smooth.
    let flag = |doc: &mut Doc, name: &str, to: bool, usual: bool| -> Result<(), Stop> {
        if to == usual {
            if doc.entry(&table, name).is_some() {
                doc.remove_field(&table, name).map_err(cant)?;
            }
        } else {
            doc.set_field(&table, name, &Val::Raw(to.to_string()), &ANIMATION_FIELDS)
                .map_err(cant)?;
        }
        Ok(())
    };
    if let Some(looping) = looping {
        flag(doc, "loop", looping, false)?;
        done.push(if looping {
            "loops".to_owned()
        } else {
            "plays once".to_owned()
        });
    }
    if let Some(smooth) = smooth {
        flag(doc, "smooth", smooth, true)?;
        done.push(if smooth {
            "is smooth".to_owned()
        } else {
            "goes straight from pose to pose".to_owned()
        });
    }
    if let Some(length) = length {
        let keys = keys_of(doc, animation)?;
        let now = keys.last().map_or(0.0, |(at, _)| *at);
        if keys.len() < 2 || now <= 0.0 {
            return Err(problem(
                Code::Conflict,
                format!(
                    "the animation \"{animation}\" has {}, so it has no length to change: it lasts from its first key to its last",
                    match keys.len() {
                        0 => "no keys".to_owned(),
                        1 => "one key".to_owned(),
                        count => format!("{count} keys"),
                    }
                ),
            ));
        }
        if !(length.is_finite() && length > 0.0 && length <= LONGEST_ANIMATION) {
            return Err(problem(
                Code::OutOfRange,
                format!("--length is in seconds, more than 0 and at most {LONGEST_ANIMATION}"),
            )
            .got(length));
        }
        // Every key's time is scaled; the last is the length exactly.
        let last = keys.len() - 1;
        let times: Vec<f64> = keys
            .iter()
            .enumerate()
            .map(|(at, (time, _))| {
                if at == last {
                    tidy_time(length)
                } else {
                    tidy_time(time * length / now)
                }
            })
            .collect();
        if times.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(problem(
                Code::OutOfRange,
                format!(
                    "at {} the keys of \"{animation}\" would be too close together to tell apart: choose a longer length",
                    seconds(length)
                ),
            )
            .got(length));
        }
        for (at, time) in times.iter().enumerate() {
            if *time != keys[at].0 {
                doc.set_value(&with(&key_at(animation, at), key("at")), &Val::Num(*time))
                    .map_err(cant)?;
            }
        }
        done.push(format!(
            "lasts {} (it was {})",
            seconds(length),
            seconds(now)
        ));
    }
    Ok(Outcome {
        summary: format!("the animation \"{animation}\" {}", done.join(", and ")),
        touched: None,
    })
}

/// `rename-animation`.
pub(super) fn rename_animation(
    doc: &mut Doc,
    animation: &str,
    new_name: &str,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    super::check_name(new_name)?;
    if animation_names(doc).iter().any(|known| known == new_name) {
        return Err(problem(
            Code::DuplicateName,
            format!("there is already an animation called \"{new_name}\""),
        )
        .got(new_name));
    }
    doc.rename_named_table(ANIMATIONS, animation, new_name)
        .map_err(cant)?;
    Ok(Outcome {
        summary: format!("renamed the animation \"{animation}\" to \"{new_name}\""),
        touched: None,
    })
}

/// `remove-animation`. Its poses stay.
pub(super) fn remove_animation(doc: &mut Doc, animation: &str) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    doc.remove_named_table(ANIMATIONS, animation)
        .map_err(cant)?;
    Ok(Outcome {
        summary: format!("removed the animation \"{animation}\" (its poses stay)"),
        touched: None,
    })
}

/// `add-key`: an animation reaches a pose at a time. The key goes where
/// its time puts it among the others.
pub(super) fn add_key(
    doc: &mut Doc,
    animation: &str,
    pose: &str,
    at: f64,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    require_any_pose(doc, pose)?;
    check_time(at)?;
    let at = tidy_time(at);
    let keys = keys_of(doc, animation)?;
    if keys.is_empty() && at != 0.0 {
        return Err(problem(
            Code::OutOfRange,
            format!(
                "an animation starts with a key at 0, and \"{animation}\" has no keys yet: add this one with --at 0, or add the pose it starts from first"
            ),
        )
        .got(at));
    }
    if let Some(same) = keys.iter().position(|(time, _)| same_time(*time, at)) {
        return Err(problem(
            Code::Conflict,
            format!(
                "\"{animation}\" already has a key at {}: key {}, the pose \"{}\". Change its pose with set-key {animation} {} --pose {pose}, or choose another time",
                seconds(at),
                same + 1,
                keys[same].1,
                same + 1
            ),
        ));
    }
    let place = keys.iter().filter(|(time, _)| *time < at).count();
    let list = keys_at(animation);
    let table = [key(ANIMATIONS), key(animation)];
    let value = key_val(at, pose, &[]);
    if doc.node(&list).is_none() {
        doc.set_field(&table, "keys", &Val::Lines(vec![value]), &ANIMATION_FIELDS)
            .map_err(cant)?;
    } else {
        doc.insert_item(&list, place, &value).map_err(cant)?;
    }
    Ok(Outcome {
        summary: format!(
            "added key {} to \"{animation}\": the pose \"{pose}\" at {}",
            place + 1,
            seconds(at)
        ),
        touched: None,
    })
}

/// `move-key`: a key gets another time. If that takes it past another
/// key, it changes places with it.
pub(super) fn move_key(
    doc: &mut Doc,
    animation: &str,
    number: usize,
    at: f64,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    check_time(at)?;
    let at = tidy_time(at);
    let keys = keys_of(doc, animation)?;
    let place = key_number(animation, &keys, number)?;
    let (was, pose) = keys[place].clone();
    if same_time(was, at) {
        return Ok(Outcome {
            summary: format!(
                "key {number} of \"{animation}\" is at {} already",
                seconds(at)
            ),
            touched: None,
        });
    }
    if place == 0 {
        return Err(problem(
            Code::Conflict,
            format!(
                "key 1 is where \"{animation}\" starts, at 0, and stays there: move the keys after it. To make the whole animation faster or slower: set-animation {animation} --length <seconds>"
            ),
        ));
    }
    if let Some(same) = keys.iter().position(|(time, _)| same_time(*time, at)) {
        return Err(problem(
            Code::Conflict,
            format!(
                "\"{animation}\" already has a key at {}: key {}, the pose \"{}\"",
                seconds(at),
                same + 1,
                keys[same].1
            ),
        ));
    }
    let list = keys_at(animation);
    // Its place among the others, by its new time.
    let new_place = keys
        .iter()
        .enumerate()
        .filter(|(other, (time, _))| *other != place && *time < at)
        .count();
    if new_place == place {
        doc.set_value(&with(&key_at(animation, place), key("at")), &Val::Num(at))
            .map_err(cant)?;
    } else {
        let events = events_of(doc, animation)
            .get(place)
            .cloned()
            .unwrap_or_default();
        doc.remove_item(&list, place).map_err(cant)?;
        doc.insert_item(&list, new_place, &key_val(at, &pose, &events))
            .map_err(cant)?;
    }
    Ok(Outcome {
        summary: format!(
            "moved key {number} of \"{animation}\" (the pose \"{pose}\") from {} to {}{}",
            seconds(was),
            seconds(at),
            if new_place == place {
                String::new()
            } else {
                format!(": it is key {} now", new_place + 1)
            }
        ),
        touched: None,
    })
}

/// `set-key`: a key reaches another pose.
pub(super) fn set_key(
    doc: &mut Doc,
    animation: &str,
    number: usize,
    pose: &str,
) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    require_any_pose(doc, pose)?;
    let keys = keys_of(doc, animation)?;
    let place = key_number(animation, &keys, number)?;
    doc.set_value(
        &with(&key_at(animation, place), key("pose")),
        &Val::Str(pose.to_owned()),
    )
    .map_err(cant)?;
    Ok(Outcome {
        summary: format!(
            "key {number} of \"{animation}\", at {}, is the pose \"{pose}\" now (it was \"{}\")",
            seconds(keys[place].0),
            keys[place].1
        ),
        touched: None,
    })
}

/// Takes keys out of an animation, by their places counted from 0. If
/// the first goes, the keys that stay start as much earlier as it takes
/// for the first of them to be at 0. Gives how much earlier.
fn drop_keys(doc: &mut Doc, animation: &str, places: &[usize]) -> Result<f64, Stop> {
    let keys = keys_of(doc, animation)?;
    let list = keys_at(animation);
    let staying: Vec<usize> = (0..keys.len()).filter(|at| !places.contains(at)).collect();
    let Some(&first) = staying.first() else {
        doc.set_value(&list, &Val::List(Vec::new())).map_err(cant)?;
        return Ok(0.0);
    };
    let earlier = keys[first].0;
    if earlier > 0.0 {
        for &at in &staying {
            doc.set_value(
                &with(&key_at(animation, at), key("at")),
                &Val::Num(tidy_time(keys[at].0 - earlier)),
            )
            .map_err(cant)?;
        }
    }
    let mut going: Vec<usize> = places.to_vec();
    going.sort_unstable();
    going.dedup();
    for &at in going.iter().rev() {
        doc.remove_item(&list, at).map_err(cant)?;
    }
    Ok(earlier)
}

/// `remove-key`.
pub(super) fn remove_key(doc: &mut Doc, animation: &str, number: usize) -> Result<Outcome, Stop> {
    require_animation(doc, animation)?;
    let keys = keys_of(doc, animation)?;
    let place = key_number(animation, &keys, number)?;
    let earlier = drop_keys(doc, animation, &[place])?;
    Ok(Outcome {
        summary: format!(
            "removed key {number} of \"{animation}\" (the pose \"{}\" at {}){}",
            keys[place].1,
            seconds(keys[place].0),
            if earlier > 0.0 {
                format!(
                    ": the keys after it are {} earlier, so that the animation still starts at 0",
                    seconds(earlier)
                )
            } else {
                String::new()
            }
        ),
        touched: None,
    })
}

// ---- the animation library ------------------------------------------------------------

/// The poses an animation's keys use, each once, in the order they come
/// (`rest` isn't one: it is always there).
fn poses_used(keys: &[(f64, String)]) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for (_, pose) in keys {
        if pose != REST && !used.contains(pose) {
            used.push(pose.clone());
        }
    }
    used
}

/// What a pose says, line by line, written again from what it means (so
/// a pose read from JSON can go into TOML).
fn pose_vals(doc: &Doc, pose: &str) -> Vec<(String, Val)> {
    let table = [key(POSES), key(pose)];
    let Some(node) = doc.node(&table) else {
        return Vec::new();
    };
    let Value::Table(entries) = &node.value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| Some((entry.key.clone(), super::val_of(&entry.node)?)))
        .collect()
}

/// Two poses that say the same, whatever order their lines are in.
fn same_pose(one: &[(String, Val)], other: &[(String, Val)]) -> bool {
    let written = |lines: &[(String, Val)]| {
        let mut lines: Vec<String> = lines
            .iter()
            .map(|(part, value)| format!("{part}={}", value.write(Syntax::Toml)))
            .collect();
        lines.sort();
        lines
    };
    written(one) == written(other)
}

/// The paths of every part of a built model.
fn part_paths(model: &Model) -> Vec<String> {
    fn walk(parts: &[Part], out: &mut Vec<String>) {
        for part in parts {
            out.push(part.path.clone());
            walk(&part.parts, out);
        }
    }
    let mut out = Vec::new();
    walk(&model.parts, &mut out);
    out
}

/// Names in a sentence: `a, b and c`.
fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => format!("\"{one}\""),
        [rest @ .., last] => format!(
            "{} and \"{last}\"",
            rest.iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `add-animation --from`: copies an animation, and the poses its keys
/// use, from another recipe (or a file `save-animation` wrote, or a
/// starter) into this one. The source's animation called `name` is
/// taken, or, if it has only one, that one. A pose whose name is taken
/// here by a pose that says something else comes in with a number; one
/// that says the same is used as it is.
pub(super) fn add_animation_from(
    doc: &mut Doc,
    name: &str,
    wanted: &str,
    looping: bool,
    (source, shown): (&Doc, &str),
    skip_missing: bool,
    model: Option<&Model>,
) -> Result<Outcome, Stop> {
    super::check_name(name)?;
    if animation_names(doc).iter().any(|known| known == name) {
        return Err(problem(
            Code::DuplicateName,
            format!(
                "there is already an animation called \"{name}\": choose another name for the copy"
            ),
        )
        .got(name));
    }
    let offered = animation_names(source);
    let taken = if offered.iter().any(|known| known == wanted) {
        wanted.to_owned()
    } else if let [only] = offered.as_slice() {
        only.clone()
    } else if offered.is_empty() {
        return Err(problem(
            Code::UnknownAnimation,
            format!("{shown} has no animations to copy"),
        ));
    } else {
        return Err(problem(
            Code::UnknownAnimation,
            format!(
                "{shown} has more than one animation and none called \"{wanted}\": name the one to copy"
            ),
        )
        .got(wanted)
        .one_of(wanted, offered));
    };
    let keys = keys_of(source, &taken)?;
    if keys.is_empty() {
        return Err(problem(
            Code::Unsupported,
            format!("the animation \"{taken}\" in {shown} has no keys: there is nothing to copy"),
        ));
    }
    let Some(model) = model else {
        return Err(problem(
            Code::Unsupported,
            "this recipe has mistakes that stop it being built, so which parts it has isn't known: put them right first",
        ));
    };
    let have = part_paths(model);
    let used = poses_used(&keys);
    let mut poses: Vec<(String, Vec<(String, Val)>)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for pose in &used {
        if source.node(&[key(POSES), key(pose)]).is_none() {
            return Err(problem(
                Code::UnknownPose,
                format!(
                    "a key of \"{taken}\" in {shown} uses the pose \"{pose}\", which isn't there"
                ),
            ));
        }
        let mut lines = pose_vals(source, pose);
        for (part, _) in &lines {
            if !have.contains(part) && !missing.contains(part) {
                missing.push(part.clone());
            }
        }
        lines.retain(|(part, _)| have.contains(part));
        poses.push((pose.clone(), lines));
    }
    if !missing.is_empty() && !skip_missing {
        let two_legs = crate::starters::TWO_LEGS.join(", ");
        return Err(problem(
            Code::UnknownPart,
            format!(
                "the animation moves {} {}, which this model doesn't have. Give the parts those names (a two-legged character's are {two_legs}), or add --skip-missing to take the rest",
                if missing.len() == 1 {
                    "the part"
                } else {
                    "the parts"
                },
                listed(&missing),
            ),
        ));
    }
    if poses.iter().all(|(_, lines)| lines.is_empty()) && !poses.is_empty() {
        return Err(problem(
            Code::UnknownPart,
            format!(
                "none of the parts the animation moves are in this model: it moves {}",
                listed(&missing)
            ),
        ));
    }
    // Each pose's name here.
    let mut names: Vec<(String, String)> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    let mut numbered: Vec<String> = Vec::new();
    for (pose, lines) in &poses {
        check_pose_name(pose)?;
        let mut count = 1;
        let here = loop {
            let candidate = if count == 1 {
                pose.clone()
            } else {
                format!("{pose}_{count}")
            };
            if !pose_names(doc).contains(&candidate) {
                doc.add_named_table(POSES, &candidate, lines)
                    .map_err(cant)?;
                added.push(candidate.clone());
                break candidate;
            }
            if same_pose(&pose_vals(doc, &candidate), lines) {
                break candidate;
            }
            count += 1;
        };
        if here != *pose {
            numbered.push(format!("\"{pose}\" as \"{here}\""));
        }
        names.push((pose.clone(), here));
    }
    let table = [key(ANIMATIONS), key(&taken)];
    let setting = |field: &str| {
        source
            .node(&with(&table, key(field)))
            .and_then(|node| node.value.as_bool())
    };
    let mut fields = Vec::new();
    let loops = looping || setting("loop") == Some(true);
    if loops {
        fields.push(("loop".to_owned(), Val::Raw("true".to_owned())));
    }
    if setting("smooth") == Some(false) {
        fields.push(("smooth".to_owned(), Val::Raw("false".to_owned())));
    }
    let renamed = |pose: &str| {
        names
            .iter()
            .find(|(from, _)| from == pose)
            .map_or(pose.to_owned(), |(_, here)| here.clone())
    };
    fields.push((
        "keys".to_owned(),
        Val::Lines(
            keys.iter()
                .zip(
                    events_of(source, &taken)
                        .into_iter()
                        .chain(std::iter::repeat(Vec::new())),
                )
                .map(|((at, pose), events)| key_val(*at, &renamed(pose), &events))
                .collect(),
        ),
    ));
    doc.add_named_table(ANIMATIONS, name, &fields)
        .map_err(cant)?;
    let mut summary = if taken == name {
        format!(
            "added the animation \"{name}\" from {shown}, with {} keys",
            keys.len()
        )
    } else {
        format!(
            "added the animation \"{name}\", a copy of \"{taken}\" from {shown}, with {} keys",
            keys.len()
        )
    };
    match added.as_slice() {
        [] => {}
        [one] => summary.push_str(&format!(" and the pose \"{one}\"")),
        _ => summary.push_str(&format!(" and the poses {}", listed(&added))),
    }
    if !numbered.is_empty() {
        summary.push_str(&format!(
            " (names taken by other poses here, so they came in as: {})",
            numbered.join(", ")
        ));
    }
    let reused: Vec<String> = names
        .iter()
        .map(|(_, here)| here.clone())
        .filter(|here| !added.contains(here))
        .collect();
    if !reused.is_empty() {
        summary.push_str(&format!(
            "; it uses {} {} already here, which {} the same",
            if reused.len() == 1 {
                "the pose"
            } else {
                "the poses"
            },
            listed(&reused),
            if reused.len() == 1 { "says" } else { "say" },
        ));
    }
    if !missing.is_empty() {
        summary.push_str(&format!(
            "; it left out what it does to {}, which this model doesn't have",
            listed(&missing)
        ));
    }
    Ok(Outcome {
        summary,
        touched: None,
    })
}

/// An animation of a recipe and the poses it uses, as a small file of
/// their own: what `duck-mold save-animation` writes, and what
/// `add-animation --from` reads. `display` is the recipe's name, for the
/// first line. Always TOML.
pub fn animation_recipe(
    text: &str,
    syntax: Syntax,
    display: &str,
    animation: &str,
) -> Result<String, crate::tools::Failure> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let usage = |stop: Stop| crate::tools::Failure::Usage(Box::new(stop.0.in_file(display)));
    let doc = Doc::parse(text, syntax).map_err(|_| {
        usage(problem(
            Code::Syntax,
            format!("\"{display}\" can't be read: check it with duck-mold check"),
        ))
    })?;
    require_animation(&doc, animation).map_err(usage)?;
    let keys = keys_of(&doc, animation).map_err(usage)?;
    let used = poses_used(&keys);
    let mut parts: Vec<String> = Vec::new();
    let mut out = format!("# Saved by `duck-mold save-animation` from {display}.\n");
    let mut body = String::new();
    for pose in &used {
        let lines = pose_vals(&doc, pose);
        if doc.node(&[key(POSES), key(pose)]).is_none() {
            return Err(usage(problem(
                Code::UnknownPose,
                format!(
                    "a key of \"{animation}\" uses the pose \"{pose}\", which isn't in the recipe: put that right first"
                ),
            )));
        }
        body.push_str(&format!(
            "\n[{}]\n",
            Doc::header_name(&[key(POSES), key(pose)])
        ));
        for (part, value) in &lines {
            if !parts.contains(part) {
                parts.push(part.clone());
            }
            body.push_str(&format!(
                "{}\n",
                super::doc::field_text(part, value, Syntax::Toml)
            ));
        }
    }
    let table = [key(ANIMATIONS), key(animation)];
    body.push_str(&format!("\n[{}]\n", Doc::header_name(&table)));
    for field in ["loop", "smooth"] {
        if let Some(on) = doc
            .node(&with(&table, key(field)))
            .and_then(|node| node.value.as_bool())
        {
            body.push_str(&format!("{field} = {on}\n"));
        }
    }
    let events = events_of(&doc, animation);
    let keys = Val::Lines(
        keys.iter()
            .enumerate()
            .map(|(at, (time, pose))| {
                key_val(*time, pose, events.get(at).map_or(&[][..], Vec::as_slice))
            })
            .collect(),
    );
    body.push_str(&format!("keys = {}\n", keys.write(Syntax::Toml)));
    if !parts.is_empty() {
        out.push_str(&format!("# It moves: {}.\n", parts.join(", ")));
    }
    out.push_str(&format!(
        "# To use it: duck-mold edit <recipe> add-animation {animation} --from <this file>\n"
    ));
    out.push_str(&format!(
        "version = 0\nname = {}\n",
        Val::Str(animation.to_owned()).write(Syntax::Toml)
    ));
    out.push_str(&body);
    Ok(out)
}

/// An animation a file offers to copy: what the animation library
/// shows of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Offered {
    /// Its name.
    pub name: String,
    /// The parts it moves, in the order its poses name them.
    pub parts: Vec<String>,
    /// How many keys it has.
    pub keys: usize,
    /// How long it lasts, in seconds: its last key's time.
    pub length: f64,
    /// True if it loops.
    pub looping: bool,
}

/// The animations in a recipe's text (or a file `save-animation`
/// wrote), each with the parts it moves. `None` if the text can't be
/// read; an animation whose keys can't be read is left out.
pub fn animations_in(text: &str, syntax: Syntax) -> Option<Vec<Offered>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let doc = Doc::parse(text, syntax).ok()?;
    Some(
        animation_names(&doc)
            .into_iter()
            .filter_map(|name| {
                let keys = keys_of(&doc, &name).ok()?;
                let mut parts: Vec<String> = Vec::new();
                for pose in poses_used(&keys) {
                    for (part, _) in pose_vals(&doc, &pose) {
                        if !parts.contains(&part) {
                            parts.push(part);
                        }
                    }
                }
                let looping = doc
                    .node(&[key(ANIMATIONS), key(&name), key("loop")])
                    .and_then(|node| node.value.as_bool())
                    == Some(true);
                Some(Offered {
                    length: keys.iter().map(|(at, _)| *at).fold(0.0, f64::max),
                    keys: keys.len(),
                    parts,
                    looping,
                    name,
                })
            })
            .collect(),
    )
}
