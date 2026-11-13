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
use crate::recipe::{Limits, REST, Turns};
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
    Ok(Outcome {
        summary: format!("renamed the pose \"{pose}\" to \"{new_name}\""),
        touched: None,
    })
}

/// `remove-pose`: takes a pose away.
pub(super) fn remove_pose(doc: &mut Doc, pose: &str) -> Result<Outcome, Stop> {
    require_pose(doc, pose)?;
    doc.remove_named_table(POSES, pose).map_err(cant)?;
    Ok(Outcome {
        summary: format!("removed the pose \"{pose}\""),
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
