//! `duck-mold mcp`: Duck Mold as tools for an AI app, over MCP (the
//! Model Context Protocol).
//!
//! The app starts `duck-mold mcp` and talks to it through its standard
//! input and output: one JSON-RPC 2.0 message a line, each way. Nothing
//! goes over the network. Each tool is one `duck-mold` command, with the
//! same arguments, named the same: a call is turned into that command's
//! words and the program runs itself with them, so a tool can never do
//! anything the command can't, and answers just as the command does with
//! `--json`.
//!
//! Written without an MCP library: MCP is JSON messages, which Duck Mold
//! already reads and writes.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::guide;

/// The MCP versions this server speaks, newest first. An app that asks
/// for one of them gets it; any other gets the newest.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the server tells the app about itself, for the AI to read.
const INSTRUCTIONS: &str = "Duck Mold makes game-ready 3D models (.glb) from recipes: TOML files that list a palette and the parts of a model and which part joins which. Call `guide` first (no topic) for the one page you need. Then: `new` makes a recipe, `edit` changes it (never write the file by hand), `check` reports every problem worst first with the command that fixes it, `render` draws it and returns the picture, `export` writes the .glb. File names are relative to the folder the app started Duck Mold in, or whole paths.";

/// Serves MCP on these streams until the input ends. `program` is the
/// `duck-mold` program the tools run.
pub fn serve(input: impl BufRead, mut output: impl Write, program: &Path) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = answer_line(&line, program) {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The answer to one line: `None` for a notification, which has none.
pub fn answer_line(line: &str, program: &Path) -> Option<Value> {
    let message: Value = match serde_json::from_str(line) {
        Ok(message) => message,
        Err(error) => {
            return Some(failure(
                Value::Null,
                -32700,
                &format!("that line isn't JSON: {error}"),
            ));
        }
    };
    match message {
        // A batch (MCP 2025-03-26): the answers in one list.
        Value::Array(messages) if !messages.is_empty() => {
            let answers: Vec<Value> = messages
                .iter()
                .filter_map(|message| answer(message, program))
                .collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
        message => answer(&message, program),
    }
}

/// The answer to one message.
fn answer(message: &Value, program: &Path) -> Option<Value> {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // An answer from the app (this server asks nothing), or nonsense.
        if message.get("result").is_some() || message.get("error").is_some() {
            return None;
        }
        return Some(failure(
            id.unwrap_or(Value::Null),
            -32600,
            "a request needs \"jsonrpc\": \"2.0\", a \"method\" and, unless it is a notification, an \"id\"",
        ));
    };
    // A notification (no id) is never answered: `notifications/initialized`,
    // `notifications/cancelled`...
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call(&params, program),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("there is no method \"{method}\""))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => failure(id, code, &message),
    })
}

fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|asked| PROTOCOL_VERSIONS.contains(asked))
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "duck-mold",
            "title": "Duck Mold",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": INSTRUCTIONS,
    })
}

/// The tools, as `tools/list` gives them.
pub fn tools() -> Vec<Value> {
    let topics: Vec<&str> = guide::TOPICS
        .iter()
        .map(|(name, _)| *name)
        .chain(["all"])
        .collect();
    let file = |what: &str| json!({ "type": "string", "description": what });
    let flag = |what: &str| json!({ "type": "boolean", "description": what });
    let name = |what: &str| json!({ "type": "string", "description": what });
    vec![
        json!({
            "name": "guide",
            "title": "Guide",
            "description": "The guide to Duck Mold for AI agents. With no topic: the one page to start with (the commands, a recipe, the loop, the rules). Read it first. With a topic, that part of the full guide; `all` is the whole guide.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "topic": { "type": "string", "enum": topics, "description": "recipes, edits, shaping, motion, hits, images, export, mistakes, or all" },
                },
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        }),
        json!({
            "name": "inspect",
            "title": "Inspect a model",
            "description": "`duck-mold inspect <file> --json`: build a recipe in memory (or read an .obj or .glb) and report its size, parts, ground, problems. Writes nothing. Options add lists (sockets, faces, joins, colliders), report it in a pose, report an animation frame by frame, or say what a ray hits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file("A recipe (.toml, .json), an .obj, or a .glb"),
                    "sockets": flag("Also list every named socket and where it is"),
                    "faces": flag("Also list the faces of every shaped part"),
                    "joins": flag("Also list what every part turns about, and how far"),
                    "colliders": flag("Also list every hit shape and its numbers"),
                    "pose": name("Report on the model as it stands in this pose"),
                    "animation": name("Report on this animation, frame by frame, instead"),
                    "ray": name("Say what a ray from this point hits: x,y,z like 0,1.4,5 (needs toward)"),
                    "toward": name("The point the ray goes toward: x,y,z"),
                    "at": { "type": "number", "description": "With ray and animation: the time in seconds" },
                },
                "required": ["file"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        }),
        json!({
            "name": "check",
            "title": "Check a recipe",
            "description": "`duck-mold check <file> --json`: everything at once (inspect, every animation, the images, the four-view sheet) and one list of problems, worst first: errors, then warnings a game would see, then small things. Each has `fix`, the command that surely fixes it, or `advice` in words. Run it after every change.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file("A recipe (.toml, .json), or a model to look at (.obj, .glb)"),
                    "output": file("Where to write the sheet; by default beside the file, as robot.png"),
                    "size": { "type": "integer", "minimum": 128, "maximum": 2048, "description": "How big each view of the sheet is, in pixels (default 256)" },
                    "no_render": flag("Draw no sheet, and write nothing"),
                },
                "required": ["file"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false },
        }),
        json!({
            "name": "edit",
            "title": "Edit a recipe",
            "description": "`duck-mold edit <recipe> <edit> --json`: change a recipe the safe way. Give one edit as its words, as they follow `duck-mold edit <recipe>` (\"set trunk --color bark\", \"add lamp --shape sphere --radius 0.1 --to desk.top\"), or several in `edits`, made in turn and written once, or not at all if any is refused. An edit that would add a mistake is refused and nothing is written. The `edits` topic of the guide lists them all.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "recipe": file("The recipe (.toml or .json)"),
                    "edit": name("One edit, as its words: \"set trunk --color bark\". Words with spaces go in quotes"),
                    "edits": { "type": "array", "items": { "type": "string" }, "minItems": 1, "description": "Several edits, one a string, made as one batch" },
                    "dry_run": flag("Say what would happen, and write nothing"),
                    "report": flag("Also give inspect's whole report after the change"),
                },
                "required": ["recipe"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false },
        }),
        json!({
            "name": "render",
            "title": "Draw a model",
            "description": "`duck-mold render <file> --json`: draw the model from four sides on one sheet (or an animation as a strip of frames), write it as a PNG, and return the picture itself to look at. Look: is every part where you meant, facing the right way, the right colour?",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file("A recipe (.toml, .json), an .obj, or a .glb"),
                    "output": file("Where to write the picture; by default beside the file, as robot.png"),
                    "size": { "type": "integer", "minimum": 128, "maximum": 2048, "description": "How big each view is, in pixels (default 256)" },
                    "pose": name("Draw the model in this pose"),
                    "animation": name("Draw this animation as a strip of frames"),
                    "frames": { "type": "integer", "minimum": 2, "maximum": 60, "description": "With animation: how many frames (default 8)" },
                    "view": { "type": "string", "enum": ["front", "right", "back", "three-quarter", "top"], "description": "With animation: where the frames are drawn from" },
                    "colliders": flag("Draw the hit shapes too"),
                    "far": { "type": "integer", "enum": [1, 2], "description": "Draw far-away version 1 or 2 instead" },
                },
                "required": ["file"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false },
        }),
        json!({
            "name": "export",
            "title": "Export a .glb",
            "description": "`duck-mold export <recipe> --json`: write the model as a .glb for Duck Forge or any glTF program: where the recipe's `export` line says, or beside it. Nothing is written if the recipe has mistakes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "recipe": file("The recipe (.toml or .json)"),
                    "output": file("Where to write the .glb instead"),
                },
                "required": ["recipe"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false },
        }),
        json!({
            "name": "new",
            "title": "Start a recipe",
            "description": "`duck-mold new <recipe> --json`: start a new recipe, one box standing on the ground; or, with `example`, a copy of one of the examples (robot, house, duck, dog, chair...). Never writes over a file that is there.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "recipe": file("The recipe to make; its name ends in .toml"),
                    "example": name("Copy this example instead: robot, house, duck, dog, chair..."),
                },
                "required": ["recipe"],
                "additionalProperties": false,
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false },
        }),
    ]
}

/// `tools/call`: runs the tool's command. A command that fails is still
/// a result, with `isError`: the AI reads why and tries again. Only a
/// call that names no tool, or the wrong arguments, is a protocol error.
fn call(params: &Value, program: &Path) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "tools/call needs the tool's \"name\"".to_owned()))?;
    let empty = json!({});
    let arguments = params.get("arguments").unwrap_or(&empty);
    if !arguments.is_object() {
        return Err((-32602, "\"arguments\" is an object".to_owned()));
    }
    if !tools().iter().any(|tool| tool["name"] == name) {
        return Err((-32602, format!("there is no tool \"{name}\"")));
    }
    let command = match words(name, arguments) {
        Ok(command) => command,
        Err(problem) => return Ok(said(&problem, true)),
    };
    let ran = match run(program, &command) {
        Ok(ran) => ran,
        Err(problem) => return Ok(said(&problem, true)),
    };
    let mut content = vec![json!({ "type": "text", "text": ran.text })];
    // The picture itself, for an AI that can see.
    if name == "render" && ran.ok {
        let written = serde_json::from_str::<Value>(&ran.text)
            .ok()
            .and_then(|report| report["output"].as_str().map(PathBuf::from));
        if let Some(png) = written.and_then(|path| std::fs::read(path).ok()) {
            content.push(json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" }));
        }
    }
    Ok(json!({ "content": content, "isError": !ran.ok }))
}

fn said(text: &str, error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": error })
}

/// What a command printed, and whether it worked.
struct Ran {
    text: String,
    ok: bool,
}

/// A command, as the words after `duck-mold`, and what it reads on its
/// standard input.
#[derive(Debug, Clone, PartialEq)]
pub struct Words {
    /// The words.
    pub args: Vec<String>,
    /// For `edit` with several edits: the batch, one a line.
    pub input: Option<String>,
}

/// The command a tool call stands for: its words, after `duck-mold`.
pub fn words(tool: &str, arguments: &Value) -> Result<Words, String> {
    let object = arguments.as_object().cloned().unwrap_or_default();
    let mut args: Vec<String> = Vec::new();
    let mut input = None;
    // Each argument once, in order; anything left over is a mistake.
    let mut left = object;
    let mut take = |key: &str| left.remove(key);
    let text = |key: &str, value: Value| -> Result<String, String> {
        match value {
            Value::String(text) if !text.is_empty() => Ok(text),
            Value::Number(number) => Ok(number.to_string()),
            other => Err(format!("\"{key}\" is a word or a number, not {other}")),
        }
    };
    let truth = |key: &str, value: Value| -> Result<bool, String> {
        value
            .as_bool()
            .ok_or_else(|| format!("\"{key}\" is true or false, not {value}"))
    };
    let required = |key: &str, take: &mut dyn FnMut(&str) -> Option<Value>| {
        take(key)
            .ok_or_else(|| format!("the tool {tool} needs \"{key}\""))
            .and_then(|value| text(key, value))
    };
    match tool {
        "guide" => {
            args.push("guide".into());
            if let Some(topic) = take("topic") {
                args.push(text("topic", topic)?);
            }
        }
        "inspect" | "check" | "render" | "export" | "new" => {
            let first = if matches!(tool, "export" | "new") {
                "recipe"
            } else {
                "file"
            };
            args.push(tool.into());
            args.push(required(first, &mut take)?);
            args.push("--json".into());
            let options: &[&str] = match tool {
                "inspect" => &[
                    "sockets",
                    "faces",
                    "joins",
                    "colliders",
                    "pose",
                    "animation",
                    "ray",
                    "toward",
                    "at",
                ],
                "check" => &["output", "size", "no_render"],
                "render" => &[
                    "output",
                    "size",
                    "pose",
                    "animation",
                    "frames",
                    "view",
                    "colliders",
                    "far",
                ],
                "export" => &["output"],
                _ => &["example"],
            };
            for &option in options {
                let Some(value) = take(option) else { continue };
                let flag = format!("--{}", option.replace('_', "-"));
                if value.is_boolean() {
                    if truth(option, value)? {
                        args.push(flag);
                    }
                } else {
                    args.push(flag);
                    args.push(text(option, value)?);
                }
            }
        }
        "edit" => {
            args.push("edit".into());
            args.push(required("recipe", &mut take)?);
            args.push("--json".into());
            for option in ["dry_run", "report"] {
                if let Some(value) = take(option)
                    && truth(option, value)?
                {
                    args.push(format!("--{}", option.replace('_', "-")));
                }
            }
            match (take("edit"), take("edits")) {
                (Some(edit), None) => {
                    let edit = text("edit", edit)?;
                    let split = split_words(&edit)?;
                    if split.is_empty() {
                        return Err("\"edit\" is empty: give the edit's words".into());
                    }
                    args.extend(split);
                }
                (None, Some(Value::Array(edits))) if !edits.is_empty() => {
                    let mut lines = Vec::new();
                    for edit in edits {
                        let edit = text("edits", edit)?;
                        if edit.contains('\n') {
                            return Err("each of \"edits\" is one edit, on one line".into());
                        }
                        lines.push(edit);
                    }
                    args.push("--batch".into());
                    args.push("-".into());
                    input = Some(lines.join("\n") + "\n");
                }
                (None, Some(_)) => return Err("\"edits\" is a list of edits".into()),
                (Some(_), Some(_)) => {
                    return Err("give \"edit\" (one) or \"edits\" (several), not both".into());
                }
                (None, None) => {
                    return Err(
                        "the tool edit needs \"edit\" (one edit's words) or \"edits\" (a list)"
                            .into(),
                    );
                }
            }
        }
        _ => return Err(format!("there is no tool \"{tool}\"")),
    }
    if let Some(extra) = left.keys().next() {
        return Err(format!("the tool {tool} has no argument \"{extra}\""));
    }
    Ok(Words { args, input })
}

/// Splits an edit's words as a shell would: on spaces, with words in
/// double or single quotes kept whole.
pub fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err(format!("a quote isn't closed in: {line}"));
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

fn run(program: &Path, words: &Words) -> Result<Ran, String> {
    let mut child = Command::new(program)
        .args(&words.args)
        .stdin(if words.input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("couldn't run {}: {error}", program.display()))?;
    if let (Some(input), Some(mut stdin)) = (&words.input, child.stdin.take()) {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| format!("couldn't give the edits: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("the command didn't finish: {error}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned();
    let errors = String::from_utf8_lossy(&output.stderr);
    // With --json everything is on standard output; standard error has
    // only what clap says about wrong words.
    if !errors.trim().is_empty() && text.is_empty() {
        text = errors.trim_end().to_owned();
    }
    Ok(Ran {
        text,
        ok: output.status.success(),
    })
}

/// Bytes as base64, for a picture inside a JSON message.
pub fn base64(bytes: &[u8]) -> String {
    const LETTERS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(LETTERS[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_right() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn words_split_as_a_shell_would() {
        assert_eq!(
            split_words("set trunk --color bark").unwrap(),
            ["set", "trunk", "--color", "bark"]
        );
        assert_eq!(
            split_words(r#"add-color "dark red" '#8B0000'"#).unwrap(),
            ["add-color", "dark red", "#8B0000"]
        );
        assert_eq!(split_words("  ").unwrap(), Vec::<String>::new());
        assert_eq!(split_words("a \"\" b").unwrap(), ["a", "", "b"]);
        assert!(split_words("set \"trunk").is_err());
    }

    #[test]
    fn a_tool_call_is_the_command_with_the_same_arguments() {
        let words = words(
            "render",
            &json!({ "file": "robot.toml", "animation": "walk", "frames": 4, "colliders": true }),
        )
        .unwrap();
        assert_eq!(
            words.args,
            [
                "render",
                "robot.toml",
                "--json",
                "--animation",
                "walk",
                "--frames",
                "4",
                "--colliders"
            ]
        );
        let batch = super::words(
            "edit",
            &json!({ "recipe": "r.toml", "edits": ["set a --color red", "remove b"], "dry_run": true }),
        )
        .unwrap();
        assert_eq!(
            batch.args,
            ["edit", "r.toml", "--json", "--dry-run", "--batch", "-"]
        );
        assert_eq!(
            batch.input.as_deref(),
            Some("set a --color red\nremove b\n")
        );
        assert!(super::words("inspect", &json!({})).is_err());
        assert!(super::words("inspect", &json!({ "file": "a.toml", "colour": 1 })).is_err());
        assert!(super::words("edit", &json!({ "recipe": "a.toml" })).is_err());
    }
}
