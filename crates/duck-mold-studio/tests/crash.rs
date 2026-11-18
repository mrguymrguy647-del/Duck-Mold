//! A crash writes a note, and the next start finds it once. In a test
//! program of its own: the crash hook is for the whole program.

use std::path::Path;

use duck_mold_studio::crash;

#[test]
fn a_crash_writes_a_note_that_the_next_start_finds_once() {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("crash_note");
    let _ = std::fs::remove_dir_all(&folder);
    let settings = folder.join("settings");
    let recipe = folder.join("robot.toml");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/robot.toml"),
        &recipe,
    )
    .unwrap();

    crash::install(settings.clone());
    crash::remember(Some(&recipe), "Set \"head\": colour red");
    crash::remember(Some(&recipe), "Set \"head\": colour red");
    crash::remember(Some(&recipe), "Moved \"arm\"");
    let crashed = std::panic::catch_unwind(|| panic!("something went wrong"));
    assert!(crashed.is_err());

    let notes: Vec<_> = std::fs::read_dir(&settings)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    let note = std::fs::read_to_string(&notes[0]).unwrap();
    assert!(note.starts_with(&format!(
        "Duck Mold Studio {} closed unexpectedly.",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(note.contains("Message:  something went wrong"), "{note}");
    // The place in the code, with / or \ as the system writes paths.
    assert!(note.contains("crash.rs:"), "{note}");
    assert!(
        note.contains(&format!("Open:     {}", recipe.display())),
        "{note}"
    );
    assert!(
        note.contains("the model\nis saved up to its last change"),
        "{note}"
    );
    // Each thing said once, oldest first.
    assert!(
        note.contains("  Set \"head\": colour red\n  Moved \"arm\"\n"),
        "{note}"
    );

    // The next start finds it, with the file that was open; the one
    // after doesn't.
    let found = crash::take_unseen(&settings).expect("the note is found");
    assert_eq!(found.note, notes[0]);
    assert_eq!(found.file.as_deref(), Some(recipe.as_path()));
    assert_eq!(crash::take_unseen(&settings), None);
}
