//! Reading TOML or JSON text into one tree that remembers line numbers.
//!
//! A recipe can be written in either language. Both are read into the same
//! [`Node`] tree, where every value and every field name knows the line
//! and column it came from. The rest of the program reads recipes from
//! that tree, so it never has to care which language a file was in, and
//! every mistake can be reported with its line.
//!
//! TOML is read by the `toml` crate, which gives positions. JSON is read
//! by the small reader in this file, because the usual JSON library
//! doesn't say where a value was.

use std::ops::Range;
use std::path::Path;

/// A place in a file: line and column, both counting from 1.
/// `Pos::default()` (line 0) means "not known".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    /// The line, counting from 1.
    pub line: usize,
    /// The column, counting from 1, in characters.
    pub column: usize,
}

/// A value and where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The value.
    pub value: Value,
    /// Where it starts.
    pub pos: Pos,
    /// The bytes of the text it is written in, from its first to just
    /// past its last: what an edit replaces to change it. (Counted in the
    /// text without the invisible mark some Windows editors put first.)
    pub span: Range<usize>,
}

/// One `name = value` of a table, in the order the file has them.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The field's name.
    pub key: String,
    /// Where the name is written.
    pub key_pos: Pos,
    /// The bytes of the name as written (with its quotes, in JSON).
    pub key_span: Range<usize>,
    /// The value.
    pub node: Node,
}

/// Anything a recipe file can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// JSON's `null`. TOML has none.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A whole number.
    Int(i64),
    /// A number with a decimal point.
    Float(f64),
    /// Text.
    Str(String),
    /// A list.
    Array(Vec<Node>),
    /// A table (TOML) or object (JSON): names with values.
    Table(Vec<Entry>),
    /// Something a recipe never uses, such as a TOML date. The text says
    /// what it is, for error messages.
    Other(&'static str),
}

impl Value {
    /// What kind of value this is, in words for an error message:
    /// `the text "big"`, `the number 3`, `a list`.
    pub fn describe(&self) -> String {
        match self {
            Value::Null => "null".to_owned(),
            Value::Bool(b) => format!("{b}"),
            Value::Int(i) => format!("the number {i}"),
            Value::Float(f) => format!("the number {f}"),
            Value::Str(s) => format!("the text \"{s}\""),
            Value::Array(_) => "a list".to_owned(),
            Value::Table(_) => "a table".to_owned(),
            Value::Other(what) => (*what).to_owned(),
        }
    }

    /// The value as JSON, for the `got` field of an issue. Lists and
    /// tables are named rather than copied.
    pub fn to_got(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => (*b).into(),
            Value::Int(i) => (*i).into(),
            Value::Float(f) => (*f).into(),
            Value::Str(s) => s.as_str().into(),
            Value::Array(_) => "a list".into(),
            Value::Table(_) => "a table".into(),
            Value::Other(what) => (*what).into(),
        }
    }

    /// The number, whether it was written `2` or `2.0`.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }
}

/// Which language a file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// TOML: `name = "crate"`.
    Toml,
    /// JSON: `{"name": "crate"}`.
    Json,
}

impl Syntax {
    /// Tells the language from the file's ending: `.toml` or `.json`
    /// (capitals too).
    pub fn from_path(path: &Path) -> Option<Syntax> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "toml" => Some(Syntax::Toml),
            "json" => Some(Syntax::Json),
            _ => None,
        }
    }
}

/// A file that isn't valid TOML or JSON: what is wrong, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxError {
    /// What is wrong.
    pub message: String,
    /// Where.
    pub pos: Pos,
}

/// Reads a whole file's text into a tree.
pub fn parse(text: &str, syntax: Syntax) -> Result<Node, SyntaxError> {
    // Some Windows editors put an invisible mark at the start of a file.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    match syntax {
        Syntax::Toml => parse_toml(text),
        Syntax::Json => JsonReader::new(text).document(),
    }
}

/// Turns "so many bytes into the text" into a line and a column.
struct Lines<'a> {
    text: &'a str,
    /// The byte where each line starts.
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Lines { text, starts }
    }

    fn pos(&self, byte: usize) -> Pos {
        let byte = byte.min(self.text.len());
        // The last line that starts at or before `byte`.
        let line = self.starts.partition_point(|&start| start <= byte) - 1;
        let column = self.text[self.starts[line]..byte].chars().count() + 1;
        Pos {
            line: line + 1,
            column,
        }
    }
}

// ---- TOML -------------------------------------------------------------------

fn parse_toml(text: &str) -> Result<Node, SyntaxError> {
    let lines = Lines::new(text);
    let table = toml::de::DeTable::parse(text).map_err(|error| SyntaxError {
        message: sentence(error.message()),
        pos: error
            .span()
            .map(|span| lines.pos(span.start))
            .unwrap_or_default(),
    })?;
    Ok(Node {
        value: toml_table(table.get_ref(), &lines),
        pos: Pos { line: 1, column: 1 },
        span: 0..text.len(),
    })
}

fn toml_table(table: &toml::de::DeTable<'_>, lines: &Lines<'_>) -> Value {
    let mut entries: Vec<(usize, Entry)> = table
        .iter()
        .map(|(key, value)| {
            let entry = Entry {
                key: key.get_ref().to_string(),
                key_pos: lines.pos(key.span().start),
                key_span: key.span(),
                node: toml_node(value, lines),
            };
            (key.span().start, entry)
        })
        .collect();
    // The file's order, not the alphabet's: the order of parts and of
    // palette colours matters.
    entries.sort_by_key(|(start, _)| *start);
    Value::Table(entries.into_iter().map(|(_, entry)| entry).collect())
}

fn toml_node(value: &toml::Spanned<toml::de::DeValue<'_>>, lines: &Lines<'_>) -> Node {
    use toml::de::DeValue;
    let converted = match value.get_ref() {
        DeValue::String(text) => Value::Str(text.to_string()),
        DeValue::Integer(integer) => {
            match i64::from_str_radix(&integer.as_str().replace('_', ""), integer.radix()) {
                Ok(i) => Value::Int(i),
                Err(_) => Value::Other("a number too big to use"),
            }
        }
        DeValue::Float(float) => match float.as_str().replace('_', "").parse::<f64>() {
            Ok(f) if f.is_finite() => Value::Float(f),
            _ => Value::Other("a number that isn't finite"),
        },
        DeValue::Boolean(b) => Value::Bool(*b),
        DeValue::Datetime(_) => Value::Other("a date or time"),
        DeValue::Array(items) => {
            Value::Array(items.iter().map(|item| toml_node(item, lines)).collect())
        }
        DeValue::Table(table) => toml_table(table, lines),
    };
    Node {
        value: converted,
        pos: lines.pos(value.span().start),
        span: value.span(),
    }
}

/// Keeps the first line of a library's error message, so it fits on one
/// line with the file and line number in front.
fn sentence(message: &str) -> String {
    message.lines().next().unwrap_or("").trim().to_owned()
}

// ---- JSON -------------------------------------------------------------------

/// How deep lists and objects may be nested. Far more than a recipe
/// needs; it stops a broken or hostile file from exhausting memory.
const MAX_DEPTH: usize = 64;

/// A small JSON reader that keeps positions. It reads standard JSON and
/// explains the usual slips (a comma too many, a comment, single quotes).
struct JsonReader<'a> {
    text: &'a str,
    /// How many bytes have been read.
    at: usize,
    lines: Lines<'a>,
}

impl<'a> JsonReader<'a> {
    fn new(text: &'a str) -> Self {
        JsonReader {
            text,
            at: 0,
            lines: Lines::new(text),
        }
    }

    fn error<T>(&self, at: usize, message: impl Into<String>) -> Result<T, SyntaxError> {
        Err(SyntaxError {
            message: message.into(),
            pos: self.lines.pos(at),
        })
    }

    fn peek(&self) -> Option<char> {
        self.text[self.at..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }

    fn skip_space(&mut self) -> Result<(), SyntaxError> {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\n' | '\r' => {
                    self.at += 1;
                }
                '/' | '#' => {
                    return self.error(
                        self.at,
                        "JSON has no comments: use a \"note\" field instead, or write the recipe in TOML",
                    );
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn document(mut self) -> Result<Node, SyntaxError> {
        self.skip_space()?;
        if self.peek().is_none() {
            return self.error(self.at, "the file is empty");
        }
        let node = self.value(0)?;
        self.skip_space()?;
        if self.peek().is_some() {
            return self.error(self.at, "there is something after the end of the recipe");
        }
        Ok(node)
    }

    fn value(&mut self, depth: usize) -> Result<Node, SyntaxError> {
        self.skip_space()?;
        let start = self.at;
        let pos = self.lines.pos(start);
        if depth > MAX_DEPTH {
            return self.error(start, "lists and objects are nested too deeply");
        }
        let value = match self.peek() {
            None => return self.error(start, "the file ends where a value should be"),
            Some('{') => self.object(depth)?,
            Some('[') => self.array(depth)?,
            Some('"') => Value::Str(self.string()?),
            Some('\'') => {
                return self.error(start, "text needs double quotes: \"like this\"");
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.number()?,
            Some(c) if c.is_ascii_alphabetic() => {
                let word: String = self.text[start..]
                    .chars()
                    .take_while(char::is_ascii_alphanumeric)
                    .collect();
                self.at += word.len();
                match word.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "null" => Value::Null,
                    _ => {
                        return self.error(
                            start,
                            format!("\"{word}\" isn't a value: text needs double quotes"),
                        );
                    }
                }
            }
            Some(c) => return self.error(start, format!("a value can't start with `{c}`")),
        };
        Ok(Node {
            value,
            pos,
            span: start..self.at,
        })
    }

    fn object(&mut self, depth: usize) -> Result<Value, SyntaxError> {
        self.at += 1; // the `{`
        let mut entries: Vec<Entry> = Vec::new();
        loop {
            self.skip_space()?;
            match self.peek() {
                Some('}') if entries.is_empty() => {
                    self.at += 1;
                    return Ok(Value::Table(entries));
                }
                Some('}') => {
                    return self.error(
                        self.at,
                        "there is a comma before this `}`: remove it (JSON allows no comma after the last field)",
                    );
                }
                Some('"') => {}
                Some('\'') => {
                    return self.error(self.at, "a field's name needs double quotes: \"name\"");
                }
                None => {
                    return self.error(self.at, "the file ends inside an object: a `}` is missing");
                }
                Some(_) => {
                    return self
                        .error(self.at, "a field's name is expected here, in double quotes");
                }
            }
            let key_start = self.at;
            let key = self.string()?;
            let key_end = self.at;
            if entries.iter().any(|entry| entry.key == key) {
                return self.error(key_start, format!("the field \"{key}\" is written twice"));
            }
            self.skip_space()?;
            if self.peek() != Some(':') {
                return self.error(self.at, format!("a `:` is expected after \"{key}\""));
            }
            self.at += 1;
            let node = self.value(depth + 1)?;
            entries.push(Entry {
                key,
                key_pos: self.lines.pos(key_start),
                key_span: key_start..key_end,
                node,
            });
            self.skip_space()?;
            match self.peek() {
                Some(',') => self.at += 1,
                Some('}') => {
                    self.at += 1;
                    return Ok(Value::Table(entries));
                }
                None => {
                    return self.error(self.at, "the file ends inside an object: a `}` is missing");
                }
                Some(_) => {
                    return self.error(self.at, "a `,` or `}` is expected here");
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, SyntaxError> {
        self.at += 1; // the `[`
        let mut items: Vec<Node> = Vec::new();
        loop {
            self.skip_space()?;
            match self.peek() {
                Some(']') if items.is_empty() => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                Some(']') => {
                    return self.error(
                        self.at,
                        "there is a comma before this `]`: remove it (JSON allows no comma after the last item)",
                    );
                }
                None => {
                    return self.error(self.at, "the file ends inside a list: a `]` is missing");
                }
                Some(_) => {}
            }
            items.push(self.value(depth + 1)?);
            self.skip_space()?;
            match self.peek() {
                Some(',') => self.at += 1,
                Some(']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                None => {
                    return self.error(self.at, "the file ends inside a list: a `]` is missing");
                }
                Some(_) => return self.error(self.at, "a `,` or `]` is expected here"),
            }
        }
    }

    fn string(&mut self) -> Result<String, SyntaxError> {
        let start = self.at;
        self.at += 1; // the opening quote
        let mut text = String::new();
        loop {
            let at = self.at;
            match self.bump() {
                None => return self.error(start, "this text has no closing double quote"),
                Some('"') => return Ok(text),
                Some('\n') => {
                    return self.error(start, "this text has no closing double quote on its line");
                }
                Some('\\') => match self.bump() {
                    Some('"') => text.push('"'),
                    Some('\\') => text.push('\\'),
                    Some('/') => text.push('/'),
                    Some('b') => text.push('\u{8}'),
                    Some('f') => text.push('\u{c}'),
                    Some('n') => text.push('\n'),
                    Some('r') => text.push('\r'),
                    Some('t') => text.push('\t'),
                    Some('u') => text.push(self.unicode_escape(at)?),
                    _ => {
                        return self.error(
                            at,
                            "a backslash must be followed by one of \" \\ / b f n r t u (write \\\\ for a backslash itself)",
                        );
                    }
                },
                Some(c) if (c as u32) < 0x20 => {
                    return self
                        .error(at, "text can't hold this control character; use \\n or \\t");
                }
                Some(c) => text.push(c),
            }
        }
    }

    /// The four hex digits after `\u`, and a second `\uXXXX` when the
    /// first is half of a pair (how JSON writes characters like emoji).
    fn unicode_escape(&mut self, at: usize) -> Result<char, SyntaxError> {
        let first = self.hex4(at)?;
        let code = if (0xD800..0xDC00).contains(&first) {
            if !self.text[self.at..].starts_with("\\u") {
                return self.error(
                    at,
                    "this \\u escape is half of a pair; the other half is missing",
                );
            }
            self.at += 2;
            let second = self.hex4(at)?;
            if !(0xDC00..0xE000).contains(&second) {
                return self.error(at, "these two \\u escapes don't form a character");
            }
            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
        } else {
            first
        };
        match char::from_u32(code) {
            Some(c) => Ok(c),
            None => self.error(at, "this \\u escape isn't a character"),
        }
    }

    fn hex4(&mut self, at: usize) -> Result<u32, SyntaxError> {
        let digits = self.text.get(self.at..self.at + 4).unwrap_or("");
        match u32::from_str_radix(digits, 16) {
            Ok(code) if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_hexdigit()) => {
                self.at += 4;
                Ok(code)
            }
            _ => self.error(at, "\\u must be followed by four hex digits, like \\u00e9"),
        }
    }

    fn number(&mut self) -> Result<Value, SyntaxError> {
        let start = self.at;
        let rest = &self.text[start..];
        let length = rest
            .char_indices()
            .find(|(_, c)| !matches!(c, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
            .map_or(rest.len(), |(at, _)| at);
        let written = &rest[..length];
        self.at += length;
        if !is_json_number(written) {
            return self.error(
                start,
                format!("`{written}` isn't a number JSON understands"),
            );
        }
        if !written.contains(['.', 'e', 'E'])
            && let Ok(i) = written.parse::<i64>()
        {
            return Ok(Value::Int(i));
        }
        match written.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(Value::Float(f)),
            _ => self.error(start, format!("`{written}` is too big a number")),
        }
    }
}

/// JSON's rule for numbers: an optional `-`, digits with no extra leading
/// zero, an optional `.digits`, an optional exponent.
fn is_json_number(written: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let unsigned = written.strip_prefix('-').unwrap_or(written);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let (whole, fraction) = match mantissa.find('.') {
        Some(at) => (&mantissa[..at], Some(&mantissa[at + 1..])),
        None => (mantissa, None),
    };
    let whole_ok = digits(whole) && (whole == "0" || !whole.starts_with('0'));
    let fraction_ok = fraction.is_none_or(digits);
    let exponent_ok = exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)));
    whole_ok && fraction_ok && exponent_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(node: &Node) -> &[Entry] {
        match &node.value {
            Value::Table(entries) => entries,
            other => panic!("expected a table, got {other:?}"),
        }
    }

    fn field<'a>(node: &'a Node, key: &str) -> &'a Entry {
        table(node)
            .iter()
            .find(|entry| entry.key == key)
            .unwrap_or_else(|| panic!("no field {key}"))
    }

    const TOML: &str = "version = 0\nname = \"crate\"\n\n[palette]\nwood = \"#B07A3F\"\n\n[[parts]]\nshape = \"box\"\nsize = [1, 2.5, 1]\n";
    const JSON: &str = "{\n  \"version\": 0,\n  \"name\": \"crate\",\n  \"palette\": { \"wood\": \"#B07A3F\" },\n  \"parts\": [\n    { \"shape\": \"box\", \"size\": [1, 2.5, 1] }\n  ]\n}\n";

    /// The tree without its positions, to compare the two languages.
    fn shape(node: &Node) -> String {
        match &node.value {
            Value::Table(entries) => {
                let fields: Vec<String> = entries
                    .iter()
                    .map(|e| format!("{}={}", e.key, shape(&e.node)))
                    .collect();
                format!("{{{}}}", fields.join(","))
            }
            Value::Array(items) => {
                let items: Vec<String> = items.iter().map(shape).collect();
                format!("[{}]", items.join(","))
            }
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn toml_and_json_give_the_same_tree() {
        let toml = parse(TOML, Syntax::Toml).unwrap();
        let json = parse(JSON, Syntax::Json).unwrap();
        assert_eq!(shape(&toml), shape(&json));
        assert_eq!(
            shape(&toml),
            "{version=Int(0),name=Str(\"crate\"),palette={wood=Str(\"#B07A3F\")},parts=[{shape=Str(\"box\"),size=[Int(1),Float(2.5),Int(1)]}]}"
        );
    }

    #[test]
    fn toml_fields_remember_their_lines() {
        let root = parse(TOML, Syntax::Toml).unwrap();
        assert_eq!(field(&root, "version").key_pos, Pos { line: 1, column: 1 });
        assert_eq!(field(&root, "name").node.pos, Pos { line: 2, column: 8 });
        let palette = &field(&root, "palette").node;
        assert_eq!(field(palette, "wood").key_pos.line, 5);
        let Value::Array(parts) = &field(&root, "parts").node.value else {
            panic!("parts is a list");
        };
        // A `[[parts]]` entry is where its header is.
        assert_eq!(parts[0].pos.line, 7);
        assert_eq!(field(&parts[0], "size").key_pos.line, 9);
    }

    #[test]
    fn json_fields_remember_their_lines() {
        let root = parse(JSON, Syntax::Json).unwrap();
        assert_eq!(field(&root, "version").key_pos, Pos { line: 2, column: 3 });
        assert_eq!(
            field(&root, "name").node.pos,
            Pos {
                line: 3,
                column: 11
            }
        );
        let Value::Array(parts) = &field(&root, "parts").node.value else {
            panic!("parts is a list");
        };
        assert_eq!(parts[0].pos, Pos { line: 6, column: 5 });
    }

    #[test]
    fn toml_keeps_the_files_order() {
        let root = parse("zebra = 1\napple = 2\nmango = 3\n", Syntax::Toml).unwrap();
        let keys: Vec<&str> = table(&root).iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["zebra", "apple", "mango"]);
    }

    #[test]
    fn toml_mistakes_say_where() {
        // The list on line 2 is never closed; the reader notices on line 3.
        let error = parse("version = 0\nsize = [1, 2\nname = 3\n", Syntax::Toml).unwrap_err();
        assert_eq!(error.pos.line, 3);
        assert!(!error.message.is_empty());
    }

    #[test]
    fn json_mistakes_say_where_and_how_to_fix_them() {
        let cases = [
            ("{\n  \"a\": 1,\n}", 3, "comma before this `}`"),
            ("[1, 2,]", 1, "comma before this `]`"),
            ("{ // hi\n}", 1, "no comments"),
            ("{ 'a': 1 }", 1, "double quotes"),
            ("{ \"a\": 'x' }", 1, "double quotes"),
            ("{ \"a\": 1, \"a\": 2 }", 1, "written twice"),
            ("{ \"a\" 1 }", 1, "`:` is expected"),
            ("{ \"a\": 1 \"b\": 2 }", 1, "`,` or `}`"),
            ("{ \"a\": [1 2] }", 1, "`,` or `]`"),
            ("{ \"a\": 01 }", 1, "isn't a number"),
            ("{ \"a\": yes }", 1, "isn't a value"),
            ("{ \"a\": \"x }", 1, "no closing double quote"),
            ("{ \"a\": 1 } x", 1, "after the end"),
            ("", 1, "empty"),
            ("{ \"a\": 1", 1, "`}` is missing"),
            ("{ \"a\": \"\\q\" }", 1, "backslash"),
        ];
        for (text, line, expected) in cases {
            let error = parse(text, Syntax::Json).unwrap_err();
            assert_eq!(error.pos.line, line, "{text}");
            assert!(
                error.message.contains(expected),
                "{text}: {} should mention {expected}",
                error.message
            );
        }
    }

    #[test]
    fn json_reads_every_kind_of_value() {
        let root = parse(
            "{\"t\": true, \"f\": false, \"n\": null, \"i\": -12, \"x\": 1.5e2, \"s\": \"a\\n\\u00e9\\ud83e\\udd86\", \"e\": [], \"o\": {}}",
            Syntax::Json,
        )
        .unwrap();
        assert_eq!(field(&root, "t").node.value, Value::Bool(true));
        assert_eq!(field(&root, "f").node.value, Value::Bool(false));
        assert_eq!(field(&root, "n").node.value, Value::Null);
        assert_eq!(field(&root, "i").node.value, Value::Int(-12));
        assert_eq!(field(&root, "x").node.value, Value::Float(150.0));
        assert_eq!(
            field(&root, "s").node.value,
            Value::Str("a\n\u{e9}\u{1f986}".to_owned())
        );
        assert_eq!(field(&root, "e").node.value, Value::Array(Vec::new()));
        assert_eq!(field(&root, "o").node.value, Value::Table(Vec::new()));
    }

    #[test]
    fn a_windows_mark_and_line_endings_are_fine() {
        let toml = parse("\u{feff}version = 0\r\nname = \"x\"\r\n", Syntax::Toml).unwrap();
        assert_eq!(field(&toml, "name").key_pos.line, 2);
        let json = parse("\u{feff}{\r\n\"version\": 0\r\n}", Syntax::Json).unwrap();
        assert_eq!(field(&json, "version").key_pos.line, 2);
    }

    #[test]
    fn the_file_ending_says_the_language() {
        assert_eq!(
            Syntax::from_path(Path::new("a/robot.toml")),
            Some(Syntax::Toml)
        );
        assert_eq!(
            Syntax::from_path(Path::new("robot.JSON")),
            Some(Syntax::Json)
        );
        assert_eq!(Syntax::from_path(Path::new("robot.glb")), None);
        assert_eq!(Syntax::from_path(Path::new("robot")), None);
    }

    #[test]
    fn deep_nesting_is_refused_not_crashed_on() {
        let deep = "[".repeat(5000);
        let error = parse(&deep, Syntax::Json).unwrap_err();
        assert!(error.message.contains("nested too deeply"));
    }
}
