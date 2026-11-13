//! Several parts selected at once, hiding parts, and the things done to
//! what is selected: copy, mirror, remove, unjoin, colour.
//!
//! Each of those is the same edit an agent would run, once for each
//! part, made together as one step to undo.

use std::ops::Range;

use duck_mold_core::edit::{self, Changes, Edit};
use duck_mold_core::math::Bounds;
use duck_mold_core::model::{Model, Part};
use duck_mold_core::source::Syntax;

use super::{Studio, part_in};

/// The path of what a part is in, if it is in a part.
fn around(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(around, _)| around)
}

fn own_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether `path` is `other`, or inside it.
fn within(path: &str, other: &str) -> bool {
    path == other
        || path
            .strip_prefix(other)
            .is_some_and(|rest| rest.starts_with('/'))
}

impl Studio {
    // ---- which parts are selected ---------------------------------------------

    /// Every selected part's path, in the order they were selected: the
    /// last is the one the properties panel shows.
    pub fn selection(&self) -> Vec<&str> {
        self.also
            .iter()
            .map(String::as_str)
            .chain(self.selected.as_deref())
            .collect()
    }

    pub(super) fn selection_owned(&self) -> Vec<String> {
        self.selection().into_iter().map(str::to_owned).collect()
    }

    /// Whether the part with this path is one of those selected.
    pub fn is_selected(&self, path: &str) -> bool {
        self.selection().contains(&path)
    }

    /// Adds a part to the selection, or takes it out if it is in it: a
    /// Shift+click. True if the part is selected afterwards.
    pub fn toggle(&mut self, path: &str) -> bool {
        if self.part(path).is_none() {
            return false;
        }
        if self.selected.as_deref() == Some(path) {
            self.selected = self.also.pop();
            return false;
        }
        if let Some(at) = self.also.iter().position(|other| other == path) {
            self.also.remove(at);
            return false;
        }
        // One of two parts, where one is inside the other, would be
        // moved twice: the newer choice wins.
        self.also
            .retain(|other| !within(other, path) && !within(path, other));
        if let Some(last) = self.selected.take()
            && !within(&last, path)
            && !within(path, &last)
        {
            self.also.push(last);
        }
        self.selected = Some(path.to_owned());
        true
    }

    /// A Shift+click in the viewport: the part under the pointer joins
    /// the selection, or leaves it. A click on nothing changes nothing.
    pub fn click_adding(&mut self, size: (usize, usize), at: (f64, f64)) -> Option<&str> {
        let model = self.model()?;
        let hit =
            super::viewport::pick(model, &self.orbit, self.offset(), size, at, &self.unseen())?;
        // The whole part it belongs to, not the piece inside it; unless
        // a piece of it is selected already, which is then the one meant.
        let outer = model
            .parts
            .iter()
            .find(|part| part.faces.contains(&hit.face))?
            .path
            .clone();
        let chosen = self
            .selection()
            .into_iter()
            .find(|path| {
                within(path, &outer)
                    && self
                        .part(path)
                        .is_some_and(|part| part.faces.contains(&hit.face))
            })
            .map_or(outer, str::to_owned);
        self.toggle(&chosen);
        self.selected.as_deref()
    }

    /// Selects every part of the model's own list.
    pub fn select_all(&mut self) {
        let mut all: Vec<String> = self.model().map_or_else(Vec::new, |model| {
            model
                .parts
                .iter()
                .filter(|part| !self.is_hidden(&part.path))
                .map(|part| part.path.clone())
                .collect()
        });
        self.selected = all.pop();
        self.also = all;
    }

    // ---- hiding -----------------------------------------------------------------

    /// Whether a part isn't drawn: it is hidden, or what it is in is.
    pub fn is_hidden(&self, path: &str) -> bool {
        self.hidden.iter().any(|hidden| within(path, hidden))
    }

    /// The parts that were hidden, by path.
    pub fn hidden(&self) -> &[String] {
        &self.hidden
    }

    /// How many parts were hidden by name (not counting what is inside
    /// them).
    pub fn hidden_count(&self) -> usize {
        self.hidden.len()
    }

    /// Stops drawing a part, or draws it again. Only the picture
    /// changes: the recipe doesn't, and the part is exported like any
    /// other. A hidden part is no longer selected.
    pub fn set_hidden(&mut self, path: &str, hidden: bool) {
        self.hidden.retain(|other| !within(other, path));
        if hidden && self.part(path).is_some() {
            self.hidden.push(path.to_owned());
            self.also.retain(|other| !within(other, path));
            if self
                .selected
                .as_deref()
                .is_some_and(|selected| within(selected, path))
            {
                self.selected = self.also.pop();
            }
        }
        let count = self.hidden.len();
        self.say(match (hidden, count) {
            (true, 1) => {
                format!("Hid \"{path}\". It is still in the model, and in what Export writes")
            }
            (true, count) => format!("Hid \"{path}\": {count} hidden"),
            (false, 0) => format!("Showing \"{path}\" again"),
            (false, count) => format!("Showing \"{path}\" again: {count} still hidden"),
        });
    }

    /// Hides every selected part. Says how many.
    pub fn hide_selected(&mut self) -> usize {
        let chosen = self.selection_owned();
        for path in &chosen {
            self.set_hidden(path, true);
        }
        if chosen.len() > 1 {
            self.say(format!(
                "Hid {} parts: {} hidden. They are still in the model",
                chosen.len(),
                self.hidden.len()
            ));
        }
        chosen.len()
    }

    /// Hides again the parts that were hidden when the file was last
    /// open: those of `paths` that are still in it. Says so, if any were.
    pub fn hide_again(&mut self, paths: &[String]) -> usize {
        for path in paths {
            if self.part(path).is_some() && !self.hidden.contains(path) {
                self.hidden.push(path.clone());
            }
        }
        let count = self.hidden.len();
        if count > 0 {
            self.say(match count {
                1 => "1 part is hidden, as it was when this file was last open: View > Show all draws it again".to_owned(),
                count => format!(
                    "{count} parts are hidden, as they were when this file was last open: View > Show all draws them again"
                ),
            });
        }
        count
    }

    /// Draws every hidden part again. Says how many there were.
    pub fn show_all(&mut self) -> usize {
        let count = self.hidden.len();
        self.hidden.clear();
        self.say(match count {
            0 => "Nothing was hidden".to_owned(),
            1 => "Showing the hidden part again".to_owned(),
            count => format!("Showing all {count} hidden parts again"),
        });
        count
    }

    /// The faces of the hidden parts, in the model as the file builds
    /// it.
    pub(super) fn unseen(&self) -> Vec<Range<usize>> {
        self.unseen_in(self.model())
    }

    /// The faces of the hidden parts in `model` (which may be what a
    /// drag would build).
    pub(super) fn unseen_in(&self, model: Option<&Model>) -> Vec<Range<usize>> {
        let Some(model) = model else {
            return Vec::new();
        };
        self.hidden
            .iter()
            .filter_map(|path| part_in(&model.parts, path))
            .map(|part| part.faces.clone())
            .collect()
    }

    // ---- what is done to the selection ----------------------------------------

    /// The parts in the same list as `path`.
    fn siblings(&self, path: &str) -> &[Part] {
        match around(path) {
            Some(around) => self.part(around).map(|part| part.parts.as_slice()),
            None => self.model().map(|model| model.parts.as_slice()),
        }
        .unwrap_or_default()
    }

    /// Every part that hangs from `path`, and from those, by path.
    pub(super) fn hanging_from(&self, path: &str) -> Vec<String> {
        let list = self.siblings(path);
        let mut names = vec![own_name(path).to_owned()];
        loop {
            let more: Vec<String> = list
                .iter()
                .filter(|part| !names.contains(&part.name))
                .filter(|part| {
                    part.attached_to
                        .as_deref()
                        .and_then(|to| to.rsplit_once('.'))
                        .is_some_and(|(target, _)| names.iter().any(|name| name == target))
                })
                .map(|part| part.name.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            names.extend(more);
        }
        names
            .into_iter()
            .skip(1)
            .map(|name| match around(path) {
                Some(around) => format!("{around}/{name}"),
                None => name,
            })
            .collect()
    }

    /// How many parts that aren't selected hang from the selected ones:
    /// what "Remove" would take with it.
    pub fn hanging_from_selection(&self) -> usize {
        let chosen = self.selection();
        let mut others: Vec<String> = chosen
            .iter()
            .flat_map(|path| self.hanging_from(path))
            .filter(|path| !chosen.iter().any(|chosen| within(path, chosen)))
            .collect();
        others.sort();
        others.dedup();
        others.len()
    }

    /// Removes every selected part: a `remove` edit for each, as one
    /// step. With `with_attached`, what hangs from them goes too;
    /// without, the edit is refused if anything does.
    pub fn remove_selected(&mut self, with_attached: bool) -> Result<String, String> {
        let chosen = self.selection_owned();
        if chosen.is_empty() {
            return Err("Nothing is selected".to_owned());
        }
        // Whatever hangs from a removed part goes with it when it is
        // itself selected: it is removed once, by the first.
        let all_selected = self.hanging_from_selection() == 0;
        let mut gone: Vec<String> = Vec::new();
        let mut edits = Vec::new();
        for path in &chosen {
            if gone.iter().any(|gone| within(path, gone)) {
                continue;
            }
            edits.push(Edit::Remove {
                part: path.clone(),
                with_attached: with_attached || all_selected,
            });
            gone.push(path.clone());
            gone.extend(self.hanging_from(path));
        }
        let say = (chosen.len() > 1).then(|| format!("Removed {} parts", chosen.len()));
        let said = self.apply_as(&edits, say)?;
        self.select(None);
        Ok(said)
    }

    /// Copies every selected part: a `duplicate` edit for each, as one
    /// step. A copy of a part that is joined is joined to the same
    /// socket. A copy of a free part stands beside what was copied, so
    /// it can be seen and taken hold of. The copies are selected.
    pub fn duplicate_selected(&mut self, mirror: bool) -> Result<String, String> {
        let chosen = self.selection_owned();
        if chosen.is_empty() {
            return Err("Nothing is selected".to_owned());
        }
        let mut edits: Vec<Edit> = chosen
            .iter()
            .map(|path| Edit::Duplicate {
                part: path.clone(),
                name: None,
                to: None,
                mirror,
            })
            .collect();
        // Where the copies would be, and which of them stand free: tried
        // in memory first.
        if !mirror && let Some(syntax) = Syntax::from_path(&self.path) {
            let before: Vec<String> = self.outline().into_iter().map(|row| row.path).collect();
            let wide = chosen
                .iter()
                .filter_map(|path| self.part(path)?.bounds)
                .reduce(|all, one| Bounds {
                    min: all.min.min(one.min),
                    max: all.max.max(one.max),
                })
                .map_or(0.0, |bounds| bounds.size().x);
            let step = self.snap_step();
            let aside = ((wide + step) / step).ceil() * step;
            if let Ok(tried) =
                edit::apply_many(&self.text, syntax, &self.path, &self.shown_as, &edits)
            {
                let mut new = Vec::new();
                fn paths(parts: &[Part], into: &mut Vec<String>) {
                    for part in parts {
                        into.push(part.path.clone());
                        paths(&part.parts, into);
                    }
                }
                if let Some(model) = &tried.model {
                    paths(&model.parts, &mut new);
                }
                for path in new.iter().filter(|path| !before.contains(path)) {
                    // Only the copy itself, not what is inside it, and
                    // only one in the model's own list: inside a part,
                    // "beside" is that part's business.
                    if around(path).is_some() {
                        continue;
                    }
                    let Some(facts) =
                        edit::describe(&tried.text, syntax, tried.model.as_ref(), path)
                    else {
                        continue;
                    };
                    if facts.to.is_some() {
                        continue;
                    }
                    let mut to = facts.position;
                    to[0] = ((to[0] + aside) * 10_000.0).round() / 10_000.0;
                    edits.push(Edit::Set {
                        part: path.clone(),
                        changes: Changes {
                            move_to: Some(to),
                            ..Changes::default()
                        },
                    });
                }
            }
        }
        let say = (chosen.len() > 1).then(|| {
            format!(
                "{} {} parts",
                if mirror { "Mirrored" } else { "Copied" },
                chosen.len()
            )
        });
        self.apply_as(&edits, say)
    }

    /// Makes every selected part that is joined free, right where it
    /// stands: an `unjoin` edit for each, as one step.
    pub fn unjoin_selected(&mut self) -> Result<String, String> {
        let edits: Vec<Edit> = self
            .selection()
            .into_iter()
            .filter(|path| {
                self.part(path)
                    .is_some_and(|part| part.attached_to.is_some())
            })
            .map(|path| Edit::Unjoin {
                part: path.to_owned(),
            })
            .collect();
        if edits.is_empty() {
            return Err("Nothing selected is joined".to_owned());
        }
        self.apply_all(&edits)
    }

    /// Gives every selected part a palette colour: a `set --color` edit
    /// for each, as one step.
    pub fn colour_selected(&mut self, color: &str) -> Result<String, String> {
        let edits: Vec<Edit> = self
            .selection()
            .into_iter()
            .map(|path| Edit::Set {
                part: path.to_owned(),
                changes: Changes {
                    color: Some(color.to_owned()),
                    ..Changes::default()
                },
            })
            .collect();
        if edits.is_empty() {
            return Err("Nothing is selected".to_owned());
        }
        let say = (edits.len() > 1).then(|| format!("Coloured {} parts {color}", edits.len()));
        self.apply_as(&edits, say)
    }
}
