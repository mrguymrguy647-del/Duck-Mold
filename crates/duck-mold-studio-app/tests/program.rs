//! The `DuckMoldStudio` program itself, as far as it goes without
//! opening a window.

use std::process::Command;

fn studio(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_DuckMoldStudio"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn version_and_help_open_no_window() {
    let output = studio(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("DuckMoldStudio {}\n", env!("CARGO_PKG_VERSION"))
    );

    let output = studio(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "Usage: DuckMoldStudio [--look] [FILE]",
        "--look",
        "duck-mold edit",
    ] {
        assert!(
            help.contains(expected),
            "help should mention {expected}:\n{help}"
        );
    }
}
