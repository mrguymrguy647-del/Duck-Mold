//! install.sh, at the top of the repository: it downloads duck-mold for
//! this machine, checks its fingerprint, and installs it; a download
//! whose fingerprint is wrong installs nothing. Tried here against a
//! release made in a folder (file:// addresses), not the internet.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = "9.9.9";

fn top() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The name install.sh downloads on this machine.
fn asset() -> String {
    let os = Command::new("uname").arg("-s").output().unwrap();
    let machine = Command::new("uname").arg("-m").output().unwrap();
    let end = match (
        String::from_utf8_lossy(&os.stdout).trim(),
        String::from_utf8_lossy(&machine.stdout).trim(),
    ) {
        ("Darwin", _) => "macos".to_owned(),
        ("Linux", "x86_64") => "linux-x86_64".to_owned(),
        ("Linux", "aarch64") => "linux-arm64".to_owned(),
        (os, machine) => panic!("no release file for {os} {machine}"),
    };
    format!("duck-mold-{VERSION}-{end}")
}

fn sha256(file: &Path) -> String {
    let output = Command::new("sh")
        .arg("-c")
        .arg("if command -v sha256sum >/dev/null; then sha256sum \"$0\"; else shasum -a 256 \"$0\"; fi")
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}

/// A release in a folder: the program under this machine's name, and
/// SHA256SUMS with `fingerprint` for it (its real one if `None`).
fn release(name: &str, fingerprint: Option<&str>) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("install-{name}"));
    let _ = std::fs::remove_dir_all(&folder);
    let files = folder.join("releases").join(format!("v{VERSION}"));
    std::fs::create_dir_all(&files).unwrap();
    let program = files.join(asset());
    std::fs::copy(env!("CARGO_BIN_EXE_duck-mold"), &program).unwrap();
    let sum = fingerprint.map_or_else(|| sha256(&program), str::to_owned);
    std::fs::write(
        files.join("SHA256SUMS"),
        format!(
            "{sum}  {}\n0000  duck-mold-{VERSION}-source.tar.gz\n",
            asset()
        ),
    )
    .unwrap();
    folder
}

fn install(folder: &Path) -> Output {
    Command::new("sh")
        .arg(top().join("install.sh"))
        .env(
            "DUCK_MOLD_DOWNLOAD_URL",
            format!("file://{}", folder.join("releases").display()),
        )
        .env("DUCK_MOLD_VERSION", VERSION)
        .env("DUCK_MOLD_DIR", folder.join("bin"))
        .output()
        .expect("sh runs")
}

#[test]
fn install_sh_installs_a_checked_duck_mold() {
    let folder = release("good", None);
    let output = install(&folder);
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed = folder.join("bin").join("duck-mold");
    let version = Command::new(&installed).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        format!("duck-mold {}", env!("CARGO_PKG_VERSION"))
    );
    assert!(said.contains("Installed: duck-mold"), "{said}");
    assert!(
        said.contains("PATH"),
        "says how to put it on the PATH: {said}"
    );

    // Again, over it: still fine.
    assert!(install(&folder).status.success());
}

#[test]
fn install_sh_installs_nothing_if_the_fingerprint_is_wrong() {
    let folder = release("tampered", Some(&"ab".repeat(32)));
    let output = install(&folder);
    assert!(!output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(said.contains("fingerprint is wrong"), "{said}");
    assert!(!folder.join("bin").join("duck-mold").exists());
}

#[test]
fn install_sh_says_so_when_the_version_isnt_there() {
    let folder = release("missing", None);
    let output = Command::new("sh")
        .arg(top().join("install.sh"))
        .env(
            "DUCK_MOLD_DOWNLOAD_URL",
            format!("file://{}", folder.join("releases").display()),
        )
        .env("DUCK_MOLD_VERSION", "0.0.1")
        .env("DUCK_MOLD_DIR", folder.join("bin"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("couldn't download"));
    assert!(!folder.join("bin").exists());
}
