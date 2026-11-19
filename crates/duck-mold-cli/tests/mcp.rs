//! `duck-mold mcp`, driven as an AI app drives it: started as its own
//! program, spoken to through its input and output, a JSON message a
//! line.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// An app with `duck-mold mcp` started, in a folder of its own.
struct App {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    folder: PathBuf,
    next_id: u64,
}

impl App {
    fn start(name: &str) -> App {
        let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mcp-{name}"));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_duck-mold"))
            .arg("mcp")
            .current_dir(&folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("duck-mold mcp starts");
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        App {
            child,
            input,
            output,
            folder,
            next_id: 1,
        }
    }

    fn send_line(&mut self, line: &str) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{line}").unwrap();
        input.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        let read = self.output.read_line(&mut line).unwrap();
        assert!(read > 0, "the server stopped");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON: {e}: {line}"))
    }

    /// Sends a request and gives its answer.
    fn ask(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send_line(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string(),
        );
        let answer = self.read();
        assert_eq!(answer["jsonrpc"], "2.0");
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    fn notify(&mut self, method: &str) {
        self.send_line(&json!({ "jsonrpc": "2.0", "method": method }).to_string());
    }

    /// Says hello, as every app does first.
    fn hello(&mut self) -> Value {
        let answer = self.ask(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" },
            }),
        );
        self.notify("notifications/initialized");
        answer["result"].clone()
    }

    /// Calls a tool: its result.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self.ask(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        assert!(answer.get("error").is_none(), "{answer}");
        answer["result"].clone()
    }

    fn text(result: &Value) -> &str {
        assert_eq!(result["content"][0]["type"], "text");
        result["content"][0]["text"].as_str().unwrap()
    }

    fn json(result: &Value) -> Value {
        serde_json::from_str(App::text(result)).unwrap()
    }

    /// Closes its input: the server ends.
    fn finish(mut self) {
        drop(self.input.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "the server ends well when its input ends");
    }
}

#[test]
fn it_says_hello_and_lists_its_tools() {
    let mut app = App::start("hello");
    let hello = app.hello();
    assert_eq!(hello["protocolVersion"], "2025-06-18");
    assert_eq!(hello["serverInfo"]["name"], "duck-mold");
    assert_eq!(hello["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(hello["capabilities"]["tools"].is_object());
    assert!(hello["instructions"].as_str().unwrap().contains("guide"));

    // The notification got no answer: the next line is the ping's.
    assert_eq!(app.ask("ping", json!({}))["result"], json!({}));

    let tools = app.ask("tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "guide", "inspect", "check", "edit", "render", "export", "new"
        ]
    );
    for tool in tools.as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["description"].as_str().unwrap().len() > 40);
    }
    app.finish();
}

#[test]
fn an_app_asking_for_an_unknown_version_gets_the_newest() {
    let mut app = App::start("version");
    let answer = app.ask(
        "initialize",
        json!({ "protocolVersion": "1999-01-01", "capabilities": {}, "clientInfo": { "name": "old", "version": "0" } }),
    );
    assert_eq!(
        answer["result"]["protocolVersion"],
        duck_mold_cli::mcp::PROTOCOL_VERSIONS[0]
    );
    let older = app.ask(
        "initialize",
        json!({ "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "old", "version": "0" } }),
    );
    assert_eq!(older["result"]["protocolVersion"], "2024-11-05");
    app.finish();
}

#[test]
fn every_tool_works_as_its_command_does() {
    let mut app = App::start("tools");
    app.hello();

    // guide: the start page, and a topic.
    let start = app.call("guide", json!({}));
    assert_eq!(start["isError"], false);
    assert!(App::text(&start).starts_with("# Duck Mold: start here"));
    let topic = app.call("guide", json!({ "topic": "shaping" }));
    assert!(App::text(&topic).starts_with("# Duck Mold guide: shaping"));

    // new: an example to work on.
    let made = app.call("new", json!({ "recipe": "duck.toml", "example": "duck" }));
    assert_eq!(made["isError"], false, "{made}");
    assert!(app.folder.join("duck.toml").is_file());

    // inspect, with an option.
    let inspected =
        App::json(&app.call("inspect", json!({ "file": "duck.toml", "sockets": true })));
    assert_eq!(inspected["ok"], true);
    assert!(!inspected["sockets"].as_array().unwrap().is_empty());

    // edit: one edit, as its words.
    let palette = &inspected["palette"];
    let colour = palette[0]["name"].as_str().unwrap().to_owned();
    let part = inspected["parts"][0]["name"].as_str().unwrap().to_owned();
    let edited = app.call(
        "edit",
        json!({ "recipe": "duck.toml", "edit": format!("set {part} --color {colour}") }),
    );
    assert_eq!(edited["isError"], false, "{edited}");
    assert_eq!(App::json(&edited)["ok"], true);

    // A batch with one bad edit is refused whole, and changes nothing.
    let before = std::fs::read(app.folder.join("duck.toml")).unwrap();
    let refused = app.call(
        "edit",
        json!({ "recipe": "duck.toml", "edits": [format!("set {part} --color {colour}"), "set nobody --color nothing"] }),
    );
    assert_eq!(refused["isError"], true, "{refused}");
    assert_eq!(App::json(&refused)["ok"], false);
    assert_eq!(std::fs::read(app.folder.join("duck.toml")).unwrap(), before);

    // render: the picture comes back, and is a PNG.
    let rendered = app.call("render", json!({ "file": "duck.toml", "size": 128 }));
    assert_eq!(rendered["isError"], false, "{rendered}");
    assert_eq!(App::json(&rendered)["output"], "duck.png");
    let image = &rendered["content"][1];
    assert_eq!(image["type"], "image");
    assert_eq!(image["mimeType"], "image/png");
    let png = decode_base64(image["data"].as_str().unwrap());
    assert_eq!(png, std::fs::read(app.folder.join("duck.png")).unwrap());
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    // check.
    let checked = App::json(&app.call("check", json!({ "file": "duck.toml", "no_render": true })));
    assert_eq!(checked["ok"], true);
    assert!(checked["problems"].is_array());

    // export.
    let exported = app.call("export", json!({ "recipe": "duck.toml" }));
    assert_eq!(exported["isError"], false, "{exported}");
    assert!(app.folder.join("duck.glb").is_file());
    app.finish();
}

#[test]
fn a_command_that_fails_is_a_result_the_ai_can_read() {
    let mut app = App::start("fails");
    app.hello();
    // A file that isn't there: the command's own report, as an error.
    let missing = app.call("inspect", json!({ "file": "nothing.toml" }));
    assert_eq!(missing["isError"], true);
    assert_eq!(App::json(&missing)["ok"], false);
    // Wrong arguments for the tool: said in words.
    let wrong = app.call("inspect", json!({ "file": "a.toml", "colour": "red" }));
    assert_eq!(wrong["isError"], true);
    assert!(App::text(&wrong).contains("colour"));
    let none = app.call("edit", json!({ "recipe": "a.toml" }));
    assert_eq!(none["isError"], true);
    // A guide topic that isn't one.
    let topic = app.call("guide", json!({ "topic": "cooking" }));
    assert_eq!(topic["isError"], true);
    app.finish();
}

#[test]
fn broken_and_unknown_messages_get_an_error_and_it_keeps_going() {
    let mut app = App::start("broken");
    app.hello();

    app.send_line("this is not JSON");
    let answer = app.read();
    assert_eq!(answer["error"]["code"], -32700);
    assert_eq!(answer["id"], Value::Null);

    app.send_line(r#"{"jsonrpc":"2.0","id":"x"}"#);
    assert_eq!(app.read()["error"]["code"], -32600);

    let unknown = app.ask("tools/explode", json!({}));
    assert_eq!(unknown["error"]["code"], -32601);

    let no_tool = app.ask("tools/call", json!({ "name": "explode", "arguments": {} }));
    assert_eq!(no_tool["error"]["code"], -32602);

    // A notification it doesn't know is not answered.
    app.notify("notifications/whatever");

    // A batch: the answers come back in one list.
    app.send_line(
        r#"[{"jsonrpc":"2.0","id":10,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/cancelled"},{"jsonrpc":"2.0","id":11,"method":"ping"}]"#,
    );
    let batch = app.read();
    let ids: Vec<&Value> = batch.as_array().unwrap().iter().map(|a| &a["id"]).collect();
    assert_eq!(ids, [&json!(10), &json!(11)]);

    // Still there.
    assert_eq!(app.ask("ping", json!({}))["result"], json!({}));
    app.finish();
}

fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("not base64: {c}"),
        }
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let real: Vec<u8> = chunk.iter().copied().filter(|&c| c != b'=').collect();
        let mut n = 0u32;
        for (i, &c) in real.iter().enumerate() {
            n |= value(c) << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..real.len() - 1]);
    }
    out
}
