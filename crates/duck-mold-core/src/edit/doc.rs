//! A recipe's text, and the small replacements that change it.
//!
//! An edit never writes a recipe out afresh. It finds the few characters
//! that have to change (a value, a line to add, a block to remove) and
//! replaces only those, so comments, blank lines, the order of things
//! and the way numbers are written all stay as they were. After every
//! replacement the text is read again, so the next one works from where
//! things now are.
//!
//! The same functions serve TOML and JSON. *What* to change is decided
//! once, by place in the recipe's tree (`parts[2].radius`); *how it is
//! written* differs: TOML has headers (`[[parts]]`) and lines, JSON has
//! braces and commas.

use std::ops::Range;

use crate::source::{self, Entry, Node, Syntax, SyntaxError, Value};

/// One step into the recipe's tree: a field of a table, or an item of a
/// list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The field with this name.
    Key(String),
    /// The item at this place, counting from 0.
    Index(usize),
}

/// A place in the recipe's tree, from the top: `parts[2].sockets`.
pub type TreePath = Vec<Step>;

/// A field's name as a step.
pub fn key(name: &str) -> Step {
    Step::Key(name.to_owned())
}

/// The path with one more step.
pub fn with(path: &[Step], step: Step) -> TreePath {
    let mut longer = path.to_vec();
    longer.push(step);
    longer
}

/// The path as error messages write it: `parts[2].sockets`.
pub fn show(path: &[Step]) -> String {
    let mut out = String::new();
    for step in path {
        match step {
            Step::Key(name) if out.is_empty() => out.push_str(name),
            Step::Key(name) => {
                out.push('.');
                out.push_str(name);
            }
            Step::Index(at) => out.push_str(&format!("[{at}]")),
        }
    }
    out
}

/// A value to write into a recipe.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    /// A number.
    Num(f64),
    /// Text.
    Str(String),
    /// A list.
    List(Vec<Val>),
    /// Names with values, written on one line: `{ on = "top" }`.
    Table(Vec<(String, Val)>),
}

impl Val {
    /// Three numbers as a list.
    pub fn three(numbers: [f64; 3]) -> Val {
        Val::List(numbers.into_iter().map(Val::Num).collect())
    }

    /// The value as it is written in a recipe, on one line.
    pub fn write(&self, syntax: Syntax) -> String {
        match self {
            Val::Num(number) => number_text(*number),
            Val::Str(text) => quoted(text),
            Val::List(items) => {
                let items: Vec<String> = items.iter().map(|item| item.write(syntax)).collect();
                format!("[{}]", items.join(", "))
            }
            Val::Table(fields) if fields.is_empty() => "{}".to_owned(),
            Val::Table(fields) => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| field_text(name, value, syntax))
                    .collect();
                format!("{{ {} }}", fields.join(", "))
            }
        }
    }
}

/// `name = value` in TOML, `"name": value` in JSON.
fn field_text(name: &str, value: &Val, syntax: Syntax) -> String {
    match syntax {
        Syntax::Toml => format!("{} = {}", toml_key(name), value.write(syntax)),
        Syntax::Json => format!("{}: {}", quoted(name), value.write(syntax)),
    }
}

/// A TOML field name: bare if it can be, in quotes otherwise.
fn toml_key(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare { name.to_owned() } else { quoted(name) }
}

/// Text in double quotes, the same in TOML and JSON.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A number as a recipe writes it: `2`, `0.35`, never `0.30000000000000004`.
pub fn number_text(number: f64) -> String {
    let rounded = (number * 1e6).round() / 1e6 + 0.0;
    if rounded == rounded.trunc() && rounded.abs() < 1e15 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

/// Why a change can't be made: one sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cant(pub String);

type Done = Result<(), Cant>;

fn cant<T>(message: impl Into<String>) -> Result<T, Cant> {
    Err(Cant(message.into()))
}

/// How a table or a list is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// The whole file.
    Root,
    /// Under a TOML header: `[palette]`, `[[parts]]`.
    Header,
    /// Between braces or brackets, in TOML or JSON.
    Inline,
    /// By dotted names only (`a.b = 1`): there is no one place it is
    /// written.
    Implicit,
}

/// A recipe's text with its tree, ready to be changed.
#[derive(Debug, Clone)]
pub struct Doc {
    /// The text, without the invisible mark some editors put first.
    pub text: String,
    /// TOML or JSON.
    pub syntax: Syntax,
    /// The tree read from `text`.
    pub root: Node,
}

impl Doc {
    /// Reads a recipe's text.
    pub fn parse(text: &str, syntax: Syntax) -> Result<Doc, SyntaxError> {
        let root = source::parse(text, syntax)?;
        Ok(Doc {
            text: text.to_owned(),
            syntax,
            root,
        })
    }

    /// The value at a place in the tree.
    pub fn node(&self, path: &[Step]) -> Option<&Node> {
        let mut node = &self.root;
        for step in path {
            node = match (step, &node.value) {
                (Step::Key(name), Value::Table(entries)) => {
                    &entries.iter().find(|entry| entry.key == *name)?.node
                }
                (Step::Index(at), Value::Array(items)) => items.get(*at)?,
                _ => return None,
            };
        }
        Some(node)
    }

    /// The fields of the table at a place in the tree.
    pub fn entries(&self, path: &[Step]) -> Option<&[Entry]> {
        match &self.node(path)?.value {
            Value::Table(entries) => Some(entries),
            _ => None,
        }
    }

    /// One field of the table at a place in the tree.
    pub fn entry(&self, table: &[Step], name: &str) -> Option<&Entry> {
        self.entries(table)?.iter().find(|entry| entry.key == name)
    }

    /// The text of a field, if it is text.
    pub fn text_at(&self, table: &[Step], name: &str) -> Option<&str> {
        match &self.entry(table, name)?.node.value {
            Value::Str(text) => Some(text),
            _ => None,
        }
    }

    /// The items of the list at a place in the tree.
    pub fn items(&self, path: &[Step]) -> Option<&[Node]> {
        match &self.node(path)?.value {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The line ending the file uses.
    fn newline(&self) -> &'static str {
        if self.text.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// Replaces some bytes of the text and reads it again.
    fn splice(&mut self, range: Range<usize>, with: &str) -> Done {
        let mut text = String::with_capacity(self.text.len() + with.len());
        text.push_str(&self.text[..range.start]);
        text.push_str(with);
        text.push_str(&self.text[range.end..]);
        match source::parse(&text, self.syntax) {
            Ok(root) => {
                self.text = text;
                self.root = root;
                Ok(())
            }
            Err(error) => cant(format!(
                "the change would leave the file unreadable at line {} ({}), so it wasn't made",
                error.pos.line, error.message
            )),
        }
    }

    /// The start of the line a byte is on.
    fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |found| found + 1)
    }

    /// Just past the end of the line a byte is on, its line ending
    /// included.
    fn line_end(&self, at: usize) -> usize {
        self.text[at..]
            .find('\n')
            .map_or(self.text.len(), |found| at + found + 1)
    }

    /// The spaces and tabs a line starts with, for the line a byte is on.
    fn indent_at(&self, at: usize) -> &str {
        let start = self.line_start(at);
        let line = &self.text[start..];
        let end = line
            .find(|c: char| c != ' ' && c != '\t')
            .unwrap_or(line.len());
        &line[..end]
    }

    fn form(&self, path: &[Step]) -> Form {
        if path.is_empty() {
            return match self.syntax {
                Syntax::Toml => Form::Root,
                Syntax::Json => Form::Inline,
            };
        }
        let Some(node) = self.node(path) else {
            return Form::Implicit;
        };
        if self.syntax == Syntax::Json {
            return Form::Inline;
        }
        let written = &self.text[node.span.clone()];
        match &node.value {
            Value::Table(_) if written.starts_with('[') => Form::Header,
            Value::Table(_) if written.starts_with('{') => Form::Inline,
            Value::Array(items) => match items.first() {
                Some(first) if self.text[first.span.clone()].starts_with("[[") => Form::Header,
                _ => Form::Inline,
            },
            _ => Form::Implicit,
        }
    }

    /// Every TOML header in the file, in order: where it starts, and the
    /// place in the tree it opens.
    fn headers(&self) -> Vec<(usize, TreePath)> {
        fn walk(doc: &Doc, node: &Node, path: &mut TreePath, into: &mut Vec<(usize, TreePath)>) {
            match &node.value {
                Value::Table(entries) => {
                    if !path.is_empty() && doc.text[node.span.clone()].starts_with('[') {
                        into.push((node.span.start, path.clone()));
                    }
                    for entry in entries {
                        path.push(key(&entry.key));
                        walk(doc, &entry.node, path, into);
                        path.pop();
                    }
                }
                Value::Array(items) => {
                    for (at, item) in items.iter().enumerate() {
                        if doc.text[item.span.clone()].starts_with("[[") {
                            path.push(Step::Index(at));
                            walk(doc, item, path, into);
                            path.pop();
                        }
                    }
                }
                _ => {}
            }
        }
        let mut headers = Vec::new();
        if self.syntax == Syntax::Toml {
            walk(self, &self.root, &mut Vec::new(), &mut headers);
            headers.sort_by_key(|(start, _)| *start);
        }
        headers
    }

    /// Where a header's own lines start: the comment lines directly above
    /// it belong to it.
    fn lead(&self, header_start: usize) -> usize {
        let mut start = self.line_start(header_start);
        while start > 0 {
            let above = self.line_start(start - 1);
            if self.text[above..start].trim_start().starts_with('#') {
                start = above;
            } else {
                break;
            }
        }
        start
    }

    /// Where the lines under a header end: at the next header (with its
    /// comments), or the end of the file.
    fn body_end(&self, header_start: usize, headers: &[(usize, TreePath)]) -> usize {
        headers
            .iter()
            .find(|(start, _)| *start > header_start)
            .map_or(self.text.len(), |(start, _)| self.lead(*start))
    }

    /// Where everything under a header ends, its sub-tables included: at
    /// the next header that isn't inside it.
    fn block_end(&self, path: &[Step], headers: &[(usize, TreePath)]) -> usize {
        let Some(node) = self.node(path) else {
            return self.text.len();
        };
        headers
            .iter()
            .find(|(start, other)| *start > node.span.start && !other.starts_with(path))
            .map_or(self.text.len(), |(start, _)| self.lead(*start))
    }

    /// The header for a place in the tree: `parts[2].parts` is
    /// `parts.parts`.
    fn header_name(path: &[Step]) -> String {
        let names: Vec<String> = path
            .iter()
            .filter_map(|step| match step {
                Step::Key(name) => Some(toml_key(name)),
                Step::Index(_) => None,
            })
            .collect();
        names.join(".")
    }

    /// The line endings needed before new lines put at `at`, so that a
    /// blank line separates them from what is above.
    fn gap_before(&self, at: usize) -> String {
        let before = &self.text[..at];
        if before.trim().is_empty() {
            return String::new();
        }
        let newline = self.newline();
        let blank = format!("{newline}{newline}");
        if before.ends_with(&blank) || before.ends_with("\n\n") {
            String::new()
        } else if before.ends_with('\n') {
            newline.to_owned()
        } else {
            blank
        }
    }

    // ---- changing values -------------------------------------------------------

    /// Replaces the value at a place in the tree.
    pub fn set_value(&mut self, path: &[Step], value: &Val) -> Done {
        let Some(node) = self.node(path) else {
            return cant(format!("there is no {} in the recipe", show(path)));
        };
        let span = node.span.clone();
        self.splice(span, &value.write(self.syntax))
    }

    /// Sets a field of a table: replaces its value if it is there, adds
    /// it if not. `order` is the usual order of the table's fields: a
    /// new field goes after the last one that comes before it there.
    pub fn set_field(&mut self, table: &[Step], name: &str, value: &Val, order: &[&str]) -> Done {
        if self.entry(table, name).is_some() {
            return self.set_value(&with(table, key(name)), value);
        }
        let Some(entries) = self.entries(table) else {
            return cant(format!("there is no table {} in the recipe", show(table)));
        };
        let rank = |name: &str| order.iter().position(|known| *known == name);
        let new_rank = rank(name).unwrap_or(usize::MAX);
        let newline = self.newline();
        match self.form(table) {
            Form::Implicit => cant(format!(
                "{} is written with dotted names, which can't be added to: write it as a table",
                show(table)
            )),
            Form::Root | Form::Header => {
                // The table's own lines: from under its header to the
                // next header.
                let headers = self.headers();
                let (from, to) = match self.form(table) {
                    Form::Root => (
                        0,
                        headers
                            .first()
                            .map_or(self.text.len(), |(start, _)| self.lead(*start)),
                    ),
                    _ => {
                        let header = self.node(table).expect("checked above").span.clone();
                        (
                            self.line_end(header.end),
                            self.body_end(header.start, &headers),
                        )
                    }
                };
                let own = |entry: &&Entry| (from..to).contains(&entry.key_span.start);
                let anchor = entries
                    .iter()
                    .filter(own)
                    .filter(|entry| rank(&entry.key).unwrap_or(usize::MAX) <= new_rank)
                    .max_by_key(|entry| entry.key_span.start);
                let at = match anchor {
                    Some(entry) => self.line_end(entry.node.span.end),
                    // Nothing comes before it: straight under the header
                    // (or, at the top of the file, above the first field).
                    None => match entries.iter().filter(own).map(|e| e.key_span.start).min() {
                        Some(first) if from == 0 => self.line_start(first),
                        _ => from,
                    },
                };
                let line = format!("{}{newline}", field_text(name, value, self.syntax));
                // The line above may be the file's last, with no line
                // ending of its own.
                let line = if at > 0 && !self.text[..at].ends_with('\n') {
                    format!("{newline}{line}")
                } else {
                    line
                };
                self.splice(at..at, &line)
            }
            Form::Inline => {
                let span = self.node(table).expect("checked above").span.clone();
                let field = field_text(name, value, self.syntax);
                if entries.is_empty() {
                    return self.splice(span, &format!("{{ {field} }}"));
                }
                let many_lines = self.text[span.clone()].contains('\n');
                let anchor = entries
                    .iter()
                    .filter(|entry| rank(&entry.key).unwrap_or(usize::MAX) <= new_rank)
                    .max_by_key(|entry| entry.key_span.start);
                match anchor {
                    Some(entry) => {
                        let at = entry.node.span.end;
                        let text = if many_lines {
                            let indent = self.indent_at(entry.key_span.start).to_owned();
                            format!(",{newline}{indent}{field}")
                        } else {
                            format!(", {field}")
                        };
                        self.splice(at..at, &text)
                    }
                    None => {
                        let at = entries[0].key_span.start;
                        let text = if many_lines {
                            let indent = self.indent_at(at).to_owned();
                            format!("{field},{newline}{indent}")
                        } else {
                            format!("{field}, ")
                        };
                        self.splice(at..at, &text)
                    }
                }
            }
        }
    }

    /// Takes a field out of a table. Fine if it isn't there.
    pub fn remove_field(&mut self, table: &[Step], name: &str) -> Done {
        let Some(entries) = self.entries(table) else {
            return Ok(());
        };
        let Some(at) = entries.iter().position(|entry| entry.key == name) else {
            return Ok(());
        };
        let entry = &entries[at];
        let range = match self.form(table) {
            Form::Implicit => {
                return cant(format!(
                    "{} is written with dotted names, which can't be changed here",
                    show(table)
                ));
            }
            Form::Root | Form::Header => {
                self.line_start(entry.key_span.start)..self.line_end(entry.node.span.end)
            }
            Form::Inline => {
                if at + 1 < entries.len() {
                    entry.key_span.start..entries[at + 1].key_span.start
                } else if at > 0 {
                    entries[at - 1].node.span.end..entry.node.span.end
                } else {
                    entry.key_span.start..entry.node.span.end
                }
            }
        };
        self.splice(range, "")
    }

    /// Adds a table under a header of its own (TOML) or as a field
    /// (JSON): `[palette]` at the top of a recipe, `[parts.sockets]`
    /// under a part.
    pub fn add_table(
        &mut self,
        parent: &[Step],
        name: &str,
        fields: &[(String, Val)],
        order: &[&str],
    ) -> Done {
        let in_lines = matches!(self.form(parent), Form::Root | Form::Header);
        if !in_lines {
            return self.set_field(parent, name, &Val::Table(fields.to_vec()), order);
        }
        let newline = self.newline();
        let headers = self.headers();
        // Under the parent's own fields, before its sub-tables.
        let at = if parent.is_empty() {
            headers
                .first()
                .map_or(self.text.len(), |(start, _)| self.lead(*start))
        } else {
            let header = self.node(parent).expect("the parent is there").span.start;
            self.body_end(header, &headers)
        };
        let mut block = self.gap_before(at);
        let header = Self::header_name(&with(parent, key(name)));
        block.push_str(&format!("[{header}]{newline}"));
        for (field, value) in fields {
            block.push_str(&field_text(field, value, Syntax::Toml));
            block.push_str(newline);
        }
        if at < self.text.len() {
            block.push_str(newline);
        }
        self.splice(at..at, &block)
    }

    // ---- changing lists --------------------------------------------------------

    /// Adds a table to the end of a list of tables. Gives its place.
    pub fn append_item(&mut self, list: &[Step], fields: &[(String, Val)]) -> Result<usize, Cant> {
        let Some(items) = self.items(list) else {
            return cant(format!("there is no list {} in the recipe", show(list)));
        };
        let count = items.len();
        let Some(last) = items.last() else {
            return cant(format!("the list {} is empty", show(list)));
        };
        let newline = self.newline();
        match self.form(list) {
            Form::Header => {
                let headers = self.headers();
                let at = self.block_end(&with(list, Step::Index(count - 1)), &headers);
                let mut block = self.gap_before(at);
                block.push_str(&format!("[[{}]]{newline}", Self::header_name(list)));
                for (name, value) in fields {
                    block.push_str(&field_text(name, value, Syntax::Toml));
                    block.push_str(newline);
                }
                if at < self.text.len() {
                    block.push_str(newline);
                }
                self.splice(at..at, &block)?;
            }
            _ if self.syntax == Syntax::Toml => {
                return cant(format!(
                    "the list {} is written on one line; to add to it, write its items under [[{}]] headers",
                    show(list),
                    Self::header_name(list)
                ));
            }
            _ => {
                let span = last.span.clone();
                let indent = self.indent_at(span.start).to_owned();
                let many_lines = self.text[span.clone()].contains('\n');
                let item = if many_lines {
                    // One more step in than the item itself, as its
                    // neighbour is written.
                    let inner = match &last.value {
                        Value::Table(entries) if !entries.is_empty() => {
                            self.indent_at(entries[0].key_span.start).to_owned()
                        }
                        _ => format!("{indent}  "),
                    };
                    let lines: Vec<String> = fields
                        .iter()
                        .map(|(name, value)| {
                            format!("{inner}{}", field_text(name, value, Syntax::Json))
                        })
                        .collect();
                    format!(
                        "{{{newline}{}{newline}{indent}}}",
                        lines.join(&format!(",{newline}"))
                    )
                } else {
                    Val::Table(fields.to_vec()).write(Syntax::Json)
                };
                let starts_line = self.text[self.line_start(span.start)..span.start]
                    .trim()
                    .is_empty();
                let text = if starts_line {
                    format!(",{newline}{indent}{item}")
                } else {
                    format!(", {item}")
                };
                self.splice(span.end..span.end, &text)?;
            }
        }
        Ok(count)
    }

    /// The bytes of one item of a list of tables, with what separates it
    /// from the next: what to take out to remove it.
    fn item_range(&self, list: &[Step], at: usize) -> Result<Range<usize>, Cant> {
        let Some(items) = self.items(list) else {
            return cant(format!("there is no list {} in the recipe", show(list)));
        };
        let Some(item) = items.get(at) else {
            return cant(format!("{} has no item {at}", show(list)));
        };
        match self.form(list) {
            Form::Header => {
                let headers = self.headers();
                let mut start = self.lead(item.span.start);
                let end = self.block_end(&with(list, Step::Index(at)), &headers);
                if end == self.text.len() {
                    // The last block of the file: the blank line above
                    // it goes too.
                    while start > 0 && self.text[..start].ends_with("\n\n") {
                        start -= 1;
                    }
                    while start > 1 && self.text[..start].ends_with("\n\r\n") {
                        start -= 2;
                    }
                }
                Ok(start..end)
            }
            _ => Ok(if at + 1 < items.len() {
                item.span.start..items[at + 1].span.start
            } else if at > 0 {
                items[at - 1].span.end..item.span.end
            } else {
                item.span.clone()
            }),
        }
    }

    /// Takes one item out of a list of tables.
    pub fn remove_item(&mut self, list: &[Step], at: usize) -> Done {
        let range = self.item_range(list, at)?;
        self.splice(range, "")
    }

    /// Copies one item of a list of tables, comments and all, and puts
    /// the copy straight after it (at `at + 1`).
    pub fn copy_item(&mut self, list: &[Step], at: usize) -> Done {
        let Some(item) = self.items(list).and_then(|items| items.get(at)) else {
            return cant(format!("{} has no item {at}", show(list)));
        };
        let newline = self.newline();
        match self.form(list) {
            Form::Header => {
                let headers = self.headers();
                let start = self.lead(item.span.start);
                let end = self.block_end(&with(list, Step::Index(at)), &headers);
                let copy = self.text[start..end].trim_end().to_owned();
                let mut block = self.gap_before(end);
                block.push_str(&copy);
                block.push_str(newline);
                if end < self.text.len() {
                    block.push_str(newline);
                }
                self.splice(end..end, &block)
            }
            _ => {
                let span = item.span.clone();
                let copy = self.text[span.clone()].to_owned();
                let starts_line = self.text[self.line_start(span.start)..span.start]
                    .trim()
                    .is_empty();
                let text = if starts_line {
                    format!(",{newline}{}{copy}", self.indent_at(span.start))
                } else {
                    format!(", {copy}")
                };
                self.splice(span.end..span.end, &text)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 4] = ["name", "shape", "radius", "color"];

    fn toml(text: &str) -> Doc {
        Doc::parse(text, Syntax::Toml).unwrap()
    }

    fn json(text: &str) -> Doc {
        Doc::parse(text, Syntax::Json).unwrap()
    }

    fn parts(at: usize) -> TreePath {
        vec![key("parts"), Step::Index(at)]
    }

    #[test]
    fn numbers_are_written_short() {
        assert_eq!(number_text(2.0), "2");
        assert_eq!(number_text(0.1 + 0.2), "0.3");
        assert_eq!(number_text(-0.0), "0");
        assert_eq!(
            Val::three([1.0, 0.5, -2.25]).write(Syntax::Toml),
            "[1, 0.5, -2.25]"
        );
    }

    #[test]
    fn a_value_is_replaced_where_it_stands() {
        let mut doc = toml("[[parts]]\nradius   =   0.5   # how wide\nname = \"a\"\n");
        doc.set_value(&with(&parts(0), key("radius")), &Val::Num(0.2))
            .unwrap();
        assert_eq!(
            doc.text,
            "[[parts]]\nradius   =   0.2   # how wide\nname = \"a\"\n"
        );
    }

    #[test]
    fn a_new_field_goes_in_the_usual_order_and_before_sub_tables() {
        let text = "# a ball\n[[parts]]\nname = \"a\"\ncolor = \"red\"\n[parts.sockets]\nx = { on = \"top\" }\n\n[[parts]]\nname = \"b\"\n";
        let mut doc = toml(text);
        doc.set_field(&parts(0), "radius", &Val::Num(1.0), &ORDER)
            .unwrap();
        assert_eq!(
            doc.text,
            "# a ball\n[[parts]]\nname = \"a\"\nradius = 1\ncolor = \"red\"\n[parts.sockets]\nx = { on = \"top\" }\n\n[[parts]]\nname = \"b\"\n"
        );
        // The last part of a file with no line ending at its end.
        let mut doc = toml("[[parts]]\nname = \"b\"");
        doc.set_field(&parts(0), "color", &Val::Str("red".into()), &ORDER)
            .unwrap();
        assert_eq!(doc.text, "[[parts]]\nname = \"b\"\ncolor = \"red\"\n");
    }

    #[test]
    fn fields_go_into_tables_on_one_line_too() {
        let mut doc = toml("[[parts]]\ncolors = { top = \"red\" }\nempty = {}\n");
        let colors = with(&parts(0), key("colors"));
        doc.set_field(&colors, "side", &Val::Str("blue".into()), &[])
            .unwrap();
        doc.set_field(&with(&parts(0), key("empty")), "a", &Val::Num(1.0), &[])
            .unwrap();
        assert_eq!(
            doc.text,
            "[[parts]]\ncolors = { top = \"red\", side = \"blue\" }\nempty = { a = 1 }\n"
        );
        doc.remove_field(&colors, "top").unwrap();
        assert_eq!(
            doc.text.lines().nth(1),
            Some("colors = { side = \"blue\" }")
        );
    }

    #[test]
    fn json_fields_keep_the_files_layout() {
        let text = "{\n  \"parts\": [\n    {\n      \"name\": \"a\",\n      \"color\": \"red\"\n    }\n  ]\n}\n";
        let mut doc = json(text);
        doc.set_field(&parts(0), "radius", &Val::Num(1.0), &ORDER)
            .unwrap();
        assert_eq!(
            doc.text,
            "{\n  \"parts\": [\n    {\n      \"name\": \"a\",\n      \"radius\": 1,\n      \"color\": \"red\"\n    }\n  ]\n}\n"
        );
        doc.remove_field(&parts(0), "radius").unwrap();
        assert_eq!(doc.text, text);
        doc.remove_field(&parts(0), "color").unwrap();
        assert_eq!(
            doc.text,
            "{\n  \"parts\": [\n    {\n      \"name\": \"a\"\n    }\n  ]\n}\n"
        );
    }

    #[test]
    fn a_part_is_added_after_the_last_of_its_list_and_removed_whole() {
        let text = "version = 0\n\n# first\n[[parts]]\nname = \"a\"\n[parts.sockets]\nx = { on = \"top\" }\n\n# the end\n[sockets]\ny = { on = \"top\" }\n";
        let mut doc = toml(text);
        let list = vec![key("parts")];
        let fields = vec![
            ("name".to_owned(), Val::Str("b".into())),
            ("shape".to_owned(), Val::Str("box".into())),
        ];
        assert_eq!(doc.append_item(&list, &fields).unwrap(), 1);
        assert_eq!(
            doc.text,
            "version = 0\n\n# first\n[[parts]]\nname = \"a\"\n[parts.sockets]\nx = { on = \"top\" }\n\n[[parts]]\nname = \"b\"\nshape = \"box\"\n\n# the end\n[sockets]\ny = { on = \"top\" }\n"
        );
        doc.remove_item(&list, 1).unwrap();
        assert_eq!(doc.text, text);
        // Removing the first takes its comment and its sub-table.
        doc.append_item(&list, &fields).unwrap();
        doc.remove_item(&list, 0).unwrap();
        assert_eq!(
            doc.text,
            "version = 0\n\n[[parts]]\nname = \"b\"\nshape = \"box\"\n\n# the end\n[sockets]\ny = { on = \"top\" }\n"
        );
    }

    #[test]
    fn the_last_block_of_a_file_is_removed_with_the_blank_line_above_it() {
        let mut doc = toml("[[parts]]\nname = \"a\"\n\n[[parts]]\nname = \"b\"\n");
        doc.remove_item(&[key("parts")], 1).unwrap();
        assert_eq!(doc.text, "[[parts]]\nname = \"a\"\n");
        let mut doc = toml("[[parts]]\r\nname = \"a\"\r\n\r\n[[parts]]\r\nname = \"b\"\r\n");
        doc.remove_item(&[key("parts")], 1).unwrap();
        assert_eq!(doc.text, "[[parts]]\r\nname = \"a\"\r\n");
    }

    #[test]
    fn a_part_inside_a_part_is_added_inside_it() {
        let text = "[[parts]]\nname = \"arm\"\n[[parts.parts]]\nname = \"upper\"\n\n[[parts]]\nname = \"leg\"\n";
        let mut doc = toml(text);
        let list = vec![key("parts"), Step::Index(0), key("parts")];
        doc.append_item(&list, &[("name".to_owned(), Val::Str("hand".into()))])
            .unwrap();
        assert_eq!(
            doc.text,
            "[[parts]]\nname = \"arm\"\n[[parts.parts]]\nname = \"upper\"\n\n[[parts.parts]]\nname = \"hand\"\n\n[[parts]]\nname = \"leg\"\n"
        );
        assert_eq!(doc.items(&list).unwrap().len(), 2);
    }

    #[test]
    fn a_copy_goes_straight_after_with_its_comments() {
        let mut doc = toml("# a\n[[parts]]\nname = \"a\" # its name\n\n[[parts]]\nname = \"b\"\n");
        doc.copy_item(&[key("parts")], 0).unwrap();
        assert_eq!(
            doc.text,
            "# a\n[[parts]]\nname = \"a\" # its name\n\n# a\n[[parts]]\nname = \"a\" # its name\n\n[[parts]]\nname = \"b\"\n"
        );
        let mut doc = json("{ \"parts\": [\n  { \"name\": \"a\" },\n  { \"name\": \"b\" }\n] }");
        doc.copy_item(&[key("parts")], 1).unwrap();
        doc.remove_item(&[key("parts")], 0).unwrap();
        assert_eq!(
            doc.text,
            "{ \"parts\": [\n  { \"name\": \"b\" },\n  { \"name\": \"b\" }\n] }"
        );
    }

    #[test]
    fn json_items_are_added_the_way_their_neighbours_are_written() {
        let mut doc = json("{\n  \"parts\": [\n    {\n      \"name\": \"a\"\n    }\n  ]\n}\n");
        let fields = vec![
            ("name".to_owned(), Val::Str("b".into())),
            ("size".to_owned(), Val::three([1.0, 2.0, 3.0])),
        ];
        doc.append_item(&[key("parts")], &fields).unwrap();
        assert_eq!(
            doc.text,
            "{\n  \"parts\": [\n    {\n      \"name\": \"a\"\n    },\n    {\n      \"name\": \"b\",\n      \"size\": [1, 2, 3]\n    }\n  ]\n}\n"
        );
        let mut doc = json("{ \"parts\": [{ \"name\": \"a\" }] }");
        doc.append_item(&[key("parts")], &fields).unwrap();
        assert_eq!(
            doc.text,
            "{ \"parts\": [{ \"name\": \"a\" }, { \"name\": \"b\", \"size\": [1, 2, 3] }] }"
        );
    }

    #[test]
    fn a_new_table_gets_a_header_of_its_own() {
        let mut doc = toml("version = 0\n\n[[parts]]\nname = \"a\"\n\n[[parts]]\nname = \"b\"\n");
        let colour = vec![("wood".to_owned(), Val::Str("#B07A3F".into()))];
        doc.add_table(&[], "palette", &colour, &[]).unwrap();
        let socket = vec![(
            "x".to_owned(),
            Val::Table(vec![("on".to_owned(), Val::Str("top".into()))]),
        )];
        doc.add_table(&parts(0), "sockets", &socket, &[]).unwrap();
        assert_eq!(
            doc.text,
            "version = 0\n\n[palette]\nwood = \"#B07A3F\"\n\n[[parts]]\nname = \"a\"\n\n[parts.sockets]\nx = { on = \"top\" }\n\n[[parts]]\nname = \"b\"\n"
        );
        assert!(doc.entry(&with(&parts(0), key("sockets")), "x").is_some());
    }

    #[test]
    fn a_list_on_one_line_cant_be_added_to_in_toml() {
        let mut doc = toml("parts = [{ name = \"a\" }]\n");
        let error = doc
            .append_item(
                &[key("parts")],
                &[("name".to_owned(), Val::Str("b".into()))],
            )
            .unwrap_err();
        assert!(error.0.contains("[[parts]]"), "{}", error.0);
        // But a value in it can be changed.
        doc.set_value(&with(&parts(0), key("name")), &Val::Str("z".into()))
            .unwrap();
        assert_eq!(doc.text, "parts = [{ name = \"z\" }]\n");
    }
}
