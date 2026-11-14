//! The timeline: a model's animations in the Studio, and playing them.
//!
//! In Pose mode an animation can be chosen. Its keys are along the
//! timeline, each a pose at a time. Moving through time shows the model
//! as the animation has it then; at a key, its pose is the one a drag
//! changes. Between two keys, a drag makes a new pose there with a key.
//! Play runs it in the view, again and again if it loops. Ghosts are the
//! poses of the keys before and after, drawn faintly.
//!
//! Every change is an edit an agent could run: `move-key`, `set-key`,
//! `remove-key`, `add-key`, `set-animation` and `add-animation`.
//! Choosing, moving through time, playing and ghosts change nothing in
//! the file.
//!
//! An animation can also be played in Part mode, with the Play button
//! in the view: the model moves; anything else the person does stops it.

use duck_mold_core::edit::{self, Edit};
use duck_mold_core::math::Vec3;
use duck_mold_core::motion::{self, Animation, Joint, Motion};
use duck_mold_core::recipe::REST;

use super::shape::Mode;
use super::viewport::Mark;
use super::{Studio, part_in};

/// The step a key snaps to when it is dragged, in seconds.
pub const KEY_STEP: f64 = 0.05;

/// How near two times must be to be the same key's, in seconds.
const SAME_TIME: f64 = 0.0005;

/// The ghost of the key before, and of the key after.
const GHOST_BEFORE: [u8; 3] = [0x6F, 0x9F, 0xD8];
const GHOST_AFTER: [u8; 3] = [0xD8, 0x9F, 0x6F];

/// One key of the animation chosen, as the timeline shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyMark {
    /// Its number, counted from 1, as `inspect` and the key commands
    /// count.
    pub number: usize,
    /// Seconds from the start.
    pub at: f64,
    /// The pose it reaches.
    pub pose: String,
}

impl Studio {
    /// The names of the model's animations.
    pub fn animation_names(&self) -> Vec<String> {
        self.model().map_or_else(Vec::new, |model| {
            model
                .animations
                .iter()
                .map(|animation| animation.name.clone())
                .collect()
        })
    }

    /// The animation chosen, if one is.
    pub fn animation(&self) -> Option<&str> {
        self.animation.as_deref()
    }

    /// The chosen animation as the model has it.
    fn chosen_animation(&self) -> Option<&Animation> {
        let name = self.animation.as_deref()?;
        self.model()?.animation(name)
    }

    /// Chooses an animation: time goes to its start. `None` lets go of
    /// it, and the model stands in the pose chosen. False if the model
    /// has no animation by that name.
    pub fn choose_animation(&mut self, name: Option<&str>) -> bool {
        if let Some(name) = name
            && !self.animation_names().iter().any(|known| known == name)
        {
            return false;
        }
        self.cancel_drag();
        self.animation = name.map(str::to_owned);
        self.playing = false;
        match name {
            Some(name) => {
                self.set_time(0.0);
                self.say(if self.mode == Mode::Pose {
                    format!(
                        "The animation \"{name}\": click a key to change its pose, or drag along the timeline"
                    )
                } else {
                    format!("The animation \"{name}\": Play plays it in the view")
                });
            }
            None => {
                self.at_time = None;
                self.refresh_pose();
            }
        }
        true
    }

    /// How long the chosen animation lasts, in seconds.
    pub fn animation_length(&self) -> f64 {
        self.chosen_animation().map_or(0.0, Animation::length)
    }

    /// Whether the chosen animation loops, and is smooth.
    pub fn animation_settings(&self) -> Option<(bool, bool)> {
        self.chosen_animation()
            .map(|animation| (animation.looping, animation.smooth))
    }

    /// The keys of the chosen animation, in order.
    pub fn keys(&self) -> Vec<KeyMark> {
        self.chosen_animation().map_or_else(Vec::new, |animation| {
            animation
                .keys
                .iter()
                .enumerate()
                .map(|(at, key)| KeyMark {
                    number: at + 1,
                    at: key.at,
                    pose: key.pose.clone(),
                })
                .collect()
        })
    }

    /// The pose the model stands in, for the list of poses: the pose
    /// chosen, or the pose of the key at the time shown. `None` between
    /// two keys, where the model stands in no pose of its own.
    pub fn pose_shown(&self) -> Option<&str> {
        if self.at_time.is_some() && self.key_here().is_none() {
            None
        } else {
            Some(self.pose())
        }
    }

    /// The time shown on the timeline, in seconds, if an animation is
    /// chosen.
    pub fn time(&self) -> Option<f64> {
        self.at_time
    }

    /// The key at the time shown, if the time is a key's.
    pub fn key_here(&self) -> Option<KeyMark> {
        let now = self.at_time?;
        self.keys()
            .into_iter()
            .find(|key| (key.at - now).abs() < SAME_TIME)
    }

    /// Moves through time: the model stands as the chosen animation has
    /// it then. At a key, its pose becomes the pose chosen, so a drag
    /// changes it.
    pub fn set_time(&mut self, seconds: f64) {
        let Some(length) = self.chosen_animation().map(Animation::length) else {
            return;
        };
        let now = if seconds.is_finite() {
            seconds.clamp(0.0, length)
        } else {
            0.0
        };
        self.at_time = Some(now);
        if let Some(key) = self.key_here() {
            self.pose = key.pose;
        }
        self.refresh_pose();
    }

    /// A click on a key: time goes there, and its pose is chosen.
    pub fn click_key(&mut self, number: usize) {
        let Some(key) = self.keys().into_iter().find(|key| key.number == number) else {
            return;
        };
        self.playing = false;
        self.set_time(key.at);
        let others: Vec<usize> = self
            .keys()
            .into_iter()
            .filter(|other| other.pose == key.pose && other.number != number)
            .map(|other| other.number)
            .collect();
        self.say(match others.as_slice() {
            [] => format!("Key {number}: the pose \"{}\"", key.pose),
            [one] => format!(
                "Key {number}: the pose \"{}\". Key {one} uses it too, and changes with it",
                key.pose
            ),
            many => format!(
                "Key {number}: the pose \"{}\". Keys {} use it too, and change with it",
                key.pose,
                many.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    /// The joints of the model at the time shown, if an animation is
    /// chosen and a time is shown.
    pub(super) fn timeline_joints(&self) -> Option<Vec<Joint>> {
        let now = self.at_time?;
        let model = self.model()?;
        let animation = self.chosen_animation()?;
        Some(Motion::of(model, animation).joints_at(now))
    }

    /// Plays the chosen animation from the time shown (from its start if
    /// it is at its end).
    pub fn play(&mut self) -> bool {
        let Some(length) = self.chosen_animation().map(Animation::length) else {
            return false;
        };
        self.cancel_drag();
        if self.at_time.is_none_or(|now| now >= length - SAME_TIME) {
            self.at_time = Some(0.0);
        }
        self.playing = true;
        self.refresh_pose();
        true
    }

    /// Stops playing, where it is.
    pub fn stop(&mut self) {
        if self.playing {
            self.playing = false;
            // Stopped between keys there is no pose to change: say so.
            self.refresh_pose();
        }
    }

    /// True while an animation is playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Space: plays, or stops.
    pub fn toggle_play(&mut self) -> bool {
        if self.playing {
            self.stop();
            true
        } else {
            self.play()
        }
    }

    /// Lets `seconds` of a playing animation pass. At its end it starts
    /// again if it loops, and otherwise stops there.
    pub(super) fn advance(&mut self, seconds: f64) {
        if !self.playing {
            return;
        }
        let Some((length, looping)) = self
            .chosen_animation()
            .map(|animation| (animation.length(), animation.looping))
        else {
            self.playing = false;
            return;
        };
        let mut now = self.at_time.unwrap_or(0.0) + seconds.max(0.0);
        if now >= length {
            if looping && length > 0.0 {
                now %= length;
            } else {
                now = length;
                self.playing = false;
            }
        }
        self.at_time = Some(now);
        if let Some(key) = self.key_here() {
            self.pose = key.pose;
        }
        self.refresh_pose();
    }

    /// Ghosts on, or off.
    pub fn set_ghosts(&mut self, on: bool) {
        self.ghosts = on;
    }

    /// Whether the ghosts are drawn.
    pub fn ghosts(&self) -> bool {
        self.ghosts
    }

    /// The keys before and after the time shown, whose poses are the
    /// ghosts: at a key, its neighbours.
    pub fn ghost_keys(&self) -> (Option<KeyMark>, Option<KeyMark>) {
        let Some(now) = self.at_time else {
            return (None, None);
        };
        let keys = self.keys();
        let before = keys
            .iter()
            .rev()
            .find(|key| key.at < now - SAME_TIME)
            .cloned();
        let after = keys.iter().find(|key| key.at > now + SAME_TIME).cloned();
        (before, after)
    }

    /// The ghosts, as faint outlines of the parts that move: the pose of
    /// the key before in blue, of the key after in orange.
    pub(super) fn ghost_marks(&self) -> Vec<Mark> {
        if self.mode != Mode::Pose || !self.ghosts || self.playing {
            return Vec::new();
        }
        let (Some(model), Some(animation)) = (self.model(), self.chosen_animation()) else {
            return Vec::new();
        };
        let moving: Vec<String> = Motion::of(model, animation)
            .parts()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let offset = self.offset();
        let (before, after) = self.ghost_keys();
        let mut marks = Vec::new();
        for (key, color) in [(before, GHOST_BEFORE), (after, GHOST_AFTER)] {
            let Some(pose) = key.and_then(|key| model.pose(&key.pose)) else {
                continue;
            };
            let ghost = motion::posed(model, &pose.joints);
            for leaf in ghost.leaves() {
                // Only what the animation moves, and what hangs from it.
                let moves = moving
                    .iter()
                    .any(|path| leaf.path == *path || leaf.path.starts_with(&format!("{path}/")))
                    || self.hangs_from_moving(&leaf.path, &moving);
                if !moves {
                    continue;
                }
                for face in &ghost.faces[leaf.faces.clone()] {
                    for (step, &from) in face.corners.iter().enumerate() {
                        let to = face.corners[(step + 1) % face.corners.len()];
                        marks.push(Mark::Thin {
                            from: from + offset,
                            to: to + offset,
                            color,
                        });
                    }
                }
            }
        }
        marks
    }

    /// True if a part hangs, by its joins, from one of `moving`.
    fn hangs_from_moving(&self, path: &str, moving: &[String]) -> bool {
        let Some(model) = self.model() else {
            return false;
        };
        let mut at = model.part(path).and_then(|part| part.parent.clone());
        for _ in 0..64 {
            let Some(parent) = at else {
                return false;
            };
            if moving.contains(&parent) {
                return true;
            }
            at = model.part(&parent).and_then(|part| part.parent.clone());
        }
        false
    }

    /// The edits for a drag between two keys: a new pose there (named
    /// after the animation, `wave_3`), holding every part as the
    /// animation has it at that time and the dragged part as dragged,
    /// and a key with it. `None` when the time shown is a key's, or no
    /// animation is chosen.
    pub(super) fn between_keys_edits(
        &self,
        part: &str,
        turn: [f64; 3],
        move_by: [f64; 3],
    ) -> Option<Vec<Edit>> {
        let now = self.at_time?;
        if self.key_here().is_some() {
            return None;
        }
        let animation = self.animation.clone()?;
        let mut joints = self.timeline_joints()?;
        match joints.iter_mut().find(|joint| joint.part == part) {
            Some(joint) => {
                joint.turn = turn;
                joint.move_by = move_by;
            }
            None => joints.push(Joint {
                part: part.to_owned(),
                loc: Default::default(),
                turn,
                move_by,
            }),
        }
        let name = edit::unused_name(
            &format!("{animation}_{}", self.keys().len() + 1),
            &self.pose_names(),
        );
        let tidy = |numbers: [f64; 3], step: f64| {
            numbers.map(|number| (number / step).round() * step + 0.0)
        };
        let mut edits = vec![Edit::AddPose {
            name: name.clone(),
            from: None,
        }];
        for joint in joints {
            let (turn, move_by) = (tidy(joint.turn, 0.1), tidy(joint.move_by, 0.0001));
            if turn == [0.0; 3] && move_by == [0.0; 3] {
                continue;
            }
            edits.push(Edit::SetPose {
                pose: name.clone(),
                part: joint.part,
                turn: (turn != [0.0; 3]).then_some(turn),
                move_by: (move_by != [0.0; 3]).then_some(move_by),
                rest: false,
            });
        }
        edits.push(Edit::AddKey {
            animation,
            pose: name,
            at: (now / 0.001).round() * 0.001,
        });
        Some(edits)
    }

    /// Makes the edits of a drag between two keys, as one step, and
    /// chooses the new pose.
    pub(super) fn apply_between_keys(&mut self, edits: Vec<Edit>) -> Result<String, String> {
        let name = edits.iter().find_map(|edit| match edit {
            Edit::AddPose { name, .. } => Some(name.clone()),
            _ => None,
        });
        let said = name
            .as_ref()
            .map(|name| format!("Made the pose \"{name}\" here, with a key"));
        let done = self.apply_as(&edits, said);
        if done.is_ok()
            && let Some(name) = name
        {
            self.pose = name;
            self.refresh_pose();
        }
        done
    }

    /// New: a new animation, with a first key at 0 in rest, chosen at
    /// once.
    pub fn new_animation(&mut self) -> Result<String, String> {
        let name = edit::unused_name("animation", &self.animation_names());
        let done = self.apply_as(
            &[
                Edit::AddAnimation {
                    name: name.clone(),
                    looping: false,
                },
                Edit::AddKey {
                    animation: name.clone(),
                    pose: REST.to_owned(),
                    at: 0.0,
                },
            ],
            Some(format!(
                "Added the animation \"{name}\": + Key adds the pose chosen at the time shown"
            )),
        );
        if done.is_ok() {
            self.choose_animation(Some(&name));
        }
        done
    }

    /// Loop on or off: `set-animation --loop`.
    pub fn set_looping(&mut self, looping: bool) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        self.apply(&Edit::SetAnimation {
            animation,
            looping: Some(looping),
            smooth: None,
            length: None,
        })
    }

    /// Smooth on or off: `set-animation --smooth`.
    pub fn set_smooth(&mut self, smooth: bool) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        self.apply(&Edit::SetAnimation {
            animation,
            looping: None,
            smooth: Some(smooth),
            length: None,
        })
    }

    /// + Key: the pose chosen, at the time shown: `add-key`.
    pub fn add_key_here(&mut self) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        let at = (self.at_time.unwrap_or(0.0) / 0.001).round() * 0.001;
        let pose = self.pose.clone();
        let done = self.apply(&Edit::AddKey {
            animation,
            pose,
            at,
        });
        if done.is_ok() {
            self.set_time(at);
        }
        done
    }

    /// Where a key dragged to `seconds` would go: snapped to 0.05 s
    /// (or free), and never past the keys either side of it. Key 1
    /// stays at 0.
    pub fn key_time(&self, number: usize, seconds: f64, free: bool) -> Option<f64> {
        let keys = self.keys();
        let at = number.checked_sub(1)?;
        if at == 0 {
            return Some(0.0);
        }
        let low = keys.get(at - 1)?.at + KEY_STEP / 5.0;
        let high = keys
            .get(at + 1)
            .map_or(f64::INFINITY, |key| key.at - KEY_STEP / 5.0);
        let snapped = if free {
            (seconds / 0.001).round() * 0.001
        } else {
            (seconds / KEY_STEP).round() * KEY_STEP
        };
        Some(((snapped.clamp(low, high)) / 0.001).round() * 0.001 + 0.0)
    }

    /// A key dragged along the timeline and let go: `move-key`.
    pub fn move_key(&mut self, number: usize, seconds: f64, free: bool) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        let at = self
            .key_time(number, seconds, free)
            .ok_or_else(|| format!("\"{animation}\" has no key {number}"))?;
        if self
            .keys()
            .get(number - 1)
            .is_some_and(|key| (key.at - at).abs() < SAME_TIME)
        {
            return Ok(String::new());
        }
        let done = self.apply(&Edit::MoveKey {
            animation,
            key: number,
            at,
        });
        if done.is_ok() {
            self.set_time(at);
        }
        done
    }

    /// A key's menu: Remove (`remove-key`).
    pub fn remove_key(&mut self, number: usize) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        let done = self.apply(&Edit::RemoveKey {
            animation,
            key: number,
        });
        if done.is_ok() {
            let now = self.at_time.unwrap_or(0.0);
            self.set_time(now);
        }
        done
    }

    /// A key's menu: another pose for it (`set-key`).
    pub fn set_key_pose(&mut self, number: usize, pose: &str) -> Result<String, String> {
        let animation = self.chosen_or_why()?;
        let done = self.apply(&Edit::SetKey {
            animation,
            key: number,
            pose: pose.to_owned(),
        });
        if done.is_ok() {
            let now = self.at_time.unwrap_or(0.0);
            self.set_time(now);
        }
        done
    }

    /// The animation chosen, or why nothing can be done to one.
    fn chosen_or_why(&mut self) -> Result<String, String> {
        match self.animation.clone() {
            Some(animation) => Ok(animation),
            None => {
                let why = "Choose an animation first, or make one with New".to_owned();
                self.say(why.clone());
                Err(why)
            }
        }
    }

    /// Keeps the timeline right after the file is read again: an
    /// animation that is gone is let go of, and the time kept within it.
    pub(super) fn keep_timeline(&mut self) {
        if let Some(name) = &self.animation
            && !self.animation_names().contains(name)
        {
            self.animation = None;
            self.at_time = None;
            self.playing = false;
            return;
        }
        if let (Some(now), Some(length)) =
            (self.at_time, self.chosen_animation().map(Animation::length))
        {
            self.at_time = Some(now.clamp(0.0, length));
        }
    }

    /// Where the middle of a part is in the picture as the model is shown
    /// now: for tests and the tour.
    pub fn shown_middle(&self, path: &str) -> Option<Vec3> {
        let part = part_in(&self.shown_model()?.parts, path)?;
        Some(part.bounds?.center() + self.offset())
    }
}
