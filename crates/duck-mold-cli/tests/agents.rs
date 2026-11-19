//! AGENTS.md and CLAUDE.md, at the top of the repository, tell agents
//! what to run. Everything they name must be there: every `duck-mold`
//! command, every crate and test that `-p` and `--test` name, every file
//! and folder.

use std::path::{Path, PathBuf};
use std::process::Command;

fn top() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(top().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Every word after `duck-mold` in the text: the commands it names.
fn commands(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .split("duck-mold ")
        .skip(1)
        .filter_map(|after| after.split_whitespace().next())
        .map(|word| word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-'))
        .filter(|word| {
            word.starts_with(|c: char| c.is_ascii_lowercase())
                && word.chars().all(|c| c.is_ascii_lowercase() || c == '-')
        })
        .map(str::to_owned)
        .collect();
    found.sort();
    found.dedup();
    found
}

#[test]
fn every_command_agents_md_names_is_there() {
    let text = read("AGENTS.md") + &read("CLAUDE.md");
    let commands = commands(&text);
    for command in [
        "guide", "new", "edit", "check", "render", "export", "mcp", "inspect",
    ] {
        assert!(
            commands.contains(&command.to_owned()),
            "AGENTS.md names {command}"
        );
    }
    for command in &commands {
        let output = Command::new(env!("CARGO_BIN_EXE_duck-mold"))
            .args([command.as_str(), "--help"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "AGENTS.md names `duck-mold {command}`, which isn't a command"
        );
    }
}

#[test]
fn every_crate_test_and_file_agents_md_names_is_there() {
    let text = read("AGENTS.md");
    let top = top();
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == '`' || c == '(' || c == ')' || c == ',')
        .filter(|word| !word.is_empty())
        .collect();
    let mut crates_named = 0;
    for pair in words.windows(2) {
        match pair[0] {
            "-p" => {
                crates_named += 1;
                assert!(
                    top.join("crates")
                        .join(pair[1])
                        .join("Cargo.toml")
                        .is_file(),
                    "no crate {}",
                    pair[1]
                );
            }
            "--test" => {
                let found = std::fs::read_dir(top.join("crates")).unwrap().any(|krate| {
                    krate
                        .unwrap()
                        .path()
                        .join("tests")
                        .join(format!("{}.rs", pair[1]))
                        .is_file()
                });
                assert!(found, "no test {}", pair[1]);
            }
            _ => {}
        }
    }
    assert!(crates_named >= 3);
    for word in &words {
        let word = word.trim_end_matches(['.', ':', ';']);
        let looks_like_a_path = (word.starts_with("crates/")
            || word.starts_with("docs/")
            || word.starts_with("tests/")
            || word.starts_with("./")
            || word.ends_with(".md"))
            && !word.contains('<');
        if looks_like_a_path {
            assert!(
                top.join(word).exists(),
                "AGENTS.md names {word}, which isn't there"
            );
        }
    }
    assert!(
        read("CLAUDE.md").starts_with("@AGENTS.md"),
        "CLAUDE.md brings in AGENTS.md"
    );
}
