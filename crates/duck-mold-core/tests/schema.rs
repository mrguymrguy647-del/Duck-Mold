//! The recipe's JSON Schema (`docs/recipe.schema.json`, printed by
//! `duck-mold schema`) is written by hand. These tests keep it in step
//! with the reader: every field the reader knows is in the schema, and
//! nothing else; every shape, collider and axis name agrees; and every
//! example recipe is valid by it.
//!
//! The schema is checked by a small validator below that knows the
//! keywords the schema uses, and panics on any other, so a keyword added
//! to the schema later can't be silently ignored.

use duck_mold_core::recipe::{
    ANIMATION_FIELDS, CHANGE_FIELDS, COLLIDER_FIELDS, Colliders, HitKind, KEY_FIELDS, Limits,
    PART_FIELDS, POSE_LINE_FIELDS, SOCKET_FIELDS, Shape, TOP_FIELDS,
};
use duck_mold_core::schema::RECIPE_SCHEMA;
use duck_mold_core::source::{self, Syntax, parse};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn schema() -> Value {
    serde_json::from_str(RECIPE_SCHEMA).expect("the schema is JSON")
}

fn def<'a>(schema: &'a Value, name: &str) -> &'a Value {
    &schema["$defs"][name]
}

fn keys(object: &Value) -> BTreeSet<String> {
    object["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .cloned()
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

#[test]
fn the_schema_has_every_field_the_reader_knows_and_no_other() {
    let schema = schema();
    assert_eq!(keys(&schema), set(&TOP_FIELDS), "the recipe's own fields");
    assert_eq!(keys(def(&schema, "part")), set(&PART_FIELDS), "a part's");
    assert_eq!(
        keys(def(&schema, "change")),
        set(&CHANGE_FIELDS),
        "a change's"
    );
    assert_eq!(
        keys(def(&schema, "socket")),
        set(&SOCKET_FIELDS),
        "a socket's"
    );
    assert_eq!(keys(def(&schema, "key")), set(&KEY_FIELDS), "a key's");
    assert_eq!(
        keys(def(&schema, "animation")),
        set(&ANIMATION_FIELDS),
        "an animation's"
    );
    let collider = &def(&schema, "part")["properties"]["collider"]["oneOf"][1];
    assert_eq!(keys(collider), set(&COLLIDER_FIELDS), "a collider's");
    let pose_line = &def(&schema, "pose")["additionalProperties"]["oneOf"][1];
    assert_eq!(keys(pose_line), set(&POSE_LINE_FIELDS), "a pose line's");
    let limits = &def(&schema, "part")["properties"]["limits"]["oneOf"][1];
    assert_eq!(keys(limits), set(&Limits::AXES), "the limits' axes");

    // Every field that sizes a shape is a part field.
    for name in Shape::NAMES {
        for field in Shape::size_fields(name) {
            assert!(PART_FIELDS.contains(field), "{name}'s {field}");
        }
    }
}

#[test]
fn the_schema_names_every_shape_hit_shape_and_collider_choice() {
    let schema = schema();
    let part = &def(&schema, "part")["properties"];
    assert_eq!(strings(&part["shape"]["enum"]), set(&Shape::NAMES));
    assert_eq!(
        strings(&schema["properties"]["colliders"]["enum"]),
        set(&Colliders::NAMES)
    );
    assert_eq!(
        strings(&part["collider"]["oneOf"][1]["properties"]["shape"]["enum"]),
        set(&HitKind::NAMES)
    );
    let mut with_none = set(&HitKind::NAMES);
    with_none.insert("none".into());
    assert_eq!(strings(&part["collider"]["oneOf"][0]["enum"]), with_none);
}

/// Every recipe in examples/, as JSON: the TOML ones read as JSON.
fn examples() -> Vec<(PathBuf, Value)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found = Vec::new();
    let mut folders = vec![folder];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            let text = match path.extension().and_then(|e| e.to_str()) {
                Some("toml") | Some("json") => std::fs::read_to_string(&path).unwrap(),
                _ => continue,
            };
            let syntax = Syntax::from_path(&path).unwrap();
            let value = to_json(&parse(&text, syntax).unwrap().value);
            // Recipes are the files with parts; palettes and animation
            // files are other things.
            if value.get("parts").is_some() {
                found.push((path, value));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// A recipe as read, as the JSON it would be.
fn to_json(value: &source::Value) -> Value {
    match value {
        source::Value::Null => Value::Null,
        source::Value::Bool(b) => Value::Bool(*b),
        source::Value::Int(i) => Value::from(*i),
        source::Value::Float(f) => Value::from(*f),
        source::Value::Str(s) => Value::String(s.clone()),
        source::Value::Array(items) => items.iter().map(|n| to_json(&n.value)).collect(),
        source::Value::Table(entries) => Value::Object(
            entries
                .iter()
                .map(|e| (e.key.clone(), to_json(&e.node.value)))
                .collect(),
        ),
        source::Value::Other(what) => panic!("a recipe with a {what}"),
    }
}

#[test]
fn every_example_recipe_is_valid_by_the_schema() {
    let schema = schema();
    let examples = examples();
    assert!(examples.len() >= 20, "found {} examples", examples.len());
    for (path, recipe) in &examples {
        let mut problems = Vec::new();
        check(&schema, &schema, recipe, "", &mut problems);
        assert!(
            problems.is_empty(),
            "{} isn't valid by the schema:\n{}",
            path.display(),
            problems.join("\n")
        );
    }
}

#[test]
fn the_schema_catches_misspelt_fields_and_wrong_values() {
    let schema = schema();
    let bad = [
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1], "colour": "red"}]}"#,
        r#"{"version": 0, "parts": [{"shape": "cube", "size": [1,1,1]}]}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1]}]}"#,
        r#"{"version": 1, "parts": [{"shape": "box", "size": [1,1,1]}]}"#,
        r#"{"version": 0, "parts": []}"#,
        r#"{"version": 0, "palette": {"red": "red"}, "parts": [{"shape": "box", "size": [1,1,1]}]}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1], "name": "a b"}]}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1], "changes": [{"pull": "top"}]}]}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1]}], "poses": {"rest": {}}}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1]}], "animations": {"walk": {"keys": [{"at": 0}]}}}"#,
        r#"{"version": 0, "parts": [{"shape": "box", "size": [1,1,1]}], "export": "a.obj"}"#,
    ];
    for text in bad {
        let recipe: Value = serde_json::from_str(text).unwrap();
        let mut problems = Vec::new();
        check(&schema, &schema, &recipe, "", &mut problems);
        assert!(!problems.is_empty(), "the schema let this through: {text}");
    }
    let good = r##"{"version": 0, "palette": {"red": "#f00"}, "parts": [{"name": "a", "shape": "box", "size": [1,1,1], "color": "red"}]}"##;
    let mut problems = Vec::new();
    check(
        &schema,
        &schema,
        &serde_json::from_str(good).unwrap(),
        "",
        &mut problems,
    );
    assert!(problems.is_empty(), "{problems:?}");
}

// The validator: enough of JSON Schema for this schema.

fn check(root: &Value, schema: &Value, value: &Value, at: &str, problems: &mut Vec<String>) {
    let Some(rules) = schema.as_object() else {
        panic!("a schema that isn't an object at {at}");
    };
    let mut fail =
        |what: String| problems.push(format!("{}: {what}", if at.is_empty() { "/" } else { at }));
    let mut later: Vec<(&Value, &Value, String)> = Vec::new();
    for (keyword, rule) in rules {
        match keyword.as_str() {
            "$schema" | "$id" | "title" | "description" => {}
            "$ref" => {
                let name = rule
                    .as_str()
                    .and_then(|r| r.strip_prefix("#/$defs/"))
                    .expect("refs are to $defs");
                let target = &root["$defs"][name];
                assert!(!target.is_null(), "no $defs/{name}");
                later.push((target, value, at.to_owned()));
            }
            "type" => {
                let ok = match rule.as_str().unwrap() {
                    "object" => value.is_object(),
                    "array" => value.is_array(),
                    "string" => value.is_string(),
                    "number" => value.is_number(),
                    "integer" => value.is_i64() || value.is_u64(),
                    "boolean" => value.is_boolean(),
                    other => panic!("type {other}"),
                };
                if !ok {
                    fail(format!("not a {rule}: {value}"));
                }
            }
            "const" => {
                if !same(value, rule) {
                    fail(format!("not {rule}: {value}"));
                }
            }
            "enum" => {
                if !rule
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|choice| same(value, choice))
                {
                    fail(format!("not one of {rule}: {value}"));
                }
            }
            "pattern" => {
                if let Some(text) = value.as_str()
                    && !matches_pattern(rule.as_str().unwrap(), text)
                {
                    fail(format!("doesn't match {rule}: {value}"));
                }
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
                if let (Some(number), Some(limit)) = (value.as_f64(), rule.as_f64()) {
                    let ok = match keyword.as_str() {
                        "minimum" => number >= limit,
                        "maximum" => number <= limit,
                        "exclusiveMinimum" => number > limit,
                        _ => number < limit,
                    };
                    if !ok {
                        fail(format!("{keyword} {limit}: {number}"));
                    }
                }
            }
            "minItems" | "maxItems" => {
                if let Some(items) = value.as_array() {
                    let limit = rule.as_u64().unwrap() as usize;
                    if (keyword == "minItems" && items.len() < limit)
                        || (keyword == "maxItems" && items.len() > limit)
                    {
                        fail(format!("{keyword} {limit}: {} items", items.len()));
                    }
                }
            }
            "minProperties" | "maxProperties" => {
                if let Some(fields) = value.as_object() {
                    let limit = rule.as_u64().unwrap() as usize;
                    if (keyword == "minProperties" && fields.len() < limit)
                        || (keyword == "maxProperties" && fields.len() > limit)
                    {
                        fail(format!("{keyword} {limit}: {} fields", fields.len()));
                    }
                }
            }
            "required" => {
                if let Some(fields) = value.as_object() {
                    for name in rule.as_array().unwrap() {
                        if !fields.contains_key(name.as_str().unwrap()) {
                            fail(format!("no {name}"));
                        }
                    }
                }
            }
            "items" => {
                if let Some(items) = value.as_array() {
                    for (index, item) in items.iter().enumerate() {
                        later.push((rule, item, format!("{at}/{index}")));
                    }
                }
            }
            "properties" | "additionalProperties" | "propertyNames" => {
                let Some(fields) = value.as_object() else {
                    continue;
                };
                let known = rules.get("properties").and_then(Value::as_object);
                for (name, field) in fields {
                    let place = format!("{at}/{name}");
                    match keyword.as_str() {
                        "properties" => {
                            if let Some(rule) = known.and_then(|known| known.get(name)) {
                                later.push((rule, field, place));
                            }
                        }
                        "additionalProperties" => {
                            if known.is_some_and(|known| known.contains_key(name)) {
                                continue;
                            }
                            match rule {
                                Value::Bool(false) => fail(format!("no field {name:?} here")),
                                Value::Bool(true) => {}
                                _ => later.push((rule, field, place)),
                            }
                        }
                        _ => {
                            let mut inner = Vec::new();
                            check(root, rule, &Value::String(name.clone()), &place, &mut inner);
                            if !inner.is_empty() {
                                fail(format!("bad name {name:?}: {}", inner.join("; ")));
                            }
                        }
                    }
                }
            }
            "oneOf" => {
                let passing = rule
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|choice| {
                        let mut inner = Vec::new();
                        check(root, choice, value, at, &mut inner);
                        inner.is_empty()
                    })
                    .count();
                if passing != 1 {
                    fail(format!("{passing} of oneOf's choices fit: {value}"));
                }
            }
            "not" => {
                let mut inner = Vec::new();
                check(root, rule, value, at, &mut inner);
                if inner.is_empty() {
                    fail(format!("must not be {rule}: {value}"));
                }
            }
            "$defs" => {}
            other => panic!("the test's validator doesn't know the keyword {other:?}"),
        }
    }
    for (rule, value, place) in later {
        check(root, rule, value, &place, problems);
    }
}

/// JSON Schema compares numbers by value: 0 and 0.0 are the same.
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// The three patterns the schema uses, done by hand (no regex library).
fn matches_pattern(pattern: &str, text: &str) -> bool {
    match pattern {
        "^[A-Za-z0-9_-]+$" => {
            !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        }
        "^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{3})$" => text.strip_prefix('#').is_some_and(|hex| {
            (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|c| c.is_ascii_hexdigit())
        }),
        "\\.glb$" => text.ends_with(".glb"),
        other => panic!("the test doesn't know the pattern {other:?}"),
    }
}
