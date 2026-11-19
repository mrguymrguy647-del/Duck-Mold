//! `duck-mold guide`: the guide for AI agents, a part at a time.
//!
//! `duck-mold guide` prints one page to start with
//! (`docs/guide-start.md`); `duck-mold guide <topic>` prints one part of
//! the full guide (`docs/AI_GUIDE.md`); `duck-mold guide all` prints all
//! of it. The topics are cut from the full guide itself, at the marks
//! `<!-- topic: shaping -->` and `<!-- /topic: shaping -->` around each
//! passage, so there is one text to keep right. A topic can collect
//! several passages, and passages of different topics can overlap.

/// The page `duck-mold guide` prints.
pub const START: &str = include_str!("../../../docs/guide-start.md");

/// The whole guide, `docs/AI_GUIDE.md`, built into the program so that
/// an agent that has only the program can still learn to use it.
pub const FULL: &str = include_str!("../../../docs/AI_GUIDE.md");

/// The topics, in the order they are listed, each with what it covers.
pub const TOPICS: [(&str, &str); 8] = [
    (
        "recipes",
        "every field of a recipe, shapes, colours, sockets, and how to build common things",
    ),
    ("edits", "every edit command, and how edit reports"),
    (
        "shaping",
        "shaping a box: pulls, insets, moved faces, edges and corners, cuts",
    ),
    (
        "motion",
        "join limits, poses, animations, events, fitting to the ground",
    ),
    (
        "hits",
        "hit shapes: which part a game's click or shot finds",
    ),
    ("images", "pictures on parts and faces, and painting them"),
    (
        "export",
        "the .glb, far-away versions, and using a model in a game",
    ),
    (
        "mistakes",
        "reading the report, errors and warnings, what to avoid, what it can't do",
    ),
];

/// One topic's text: its passages of the full guide, one after the
/// other, with the marks taken out, under a heading that names it.
/// `None` if there is no such topic.
pub fn topic(name: &str) -> Option<String> {
    let (_, about) = TOPICS.iter().find(|(topic, _)| *topic == name)?;
    let begin = format!("<!-- topic: {name} -->");
    let end = format!("<!-- /topic: {name} -->");
    let mut passages: Vec<String> = Vec::new();
    let mut inside: Option<Vec<&str>> = None;
    for line in FULL.lines() {
        let trimmed = line.trim();
        if trimmed == begin {
            inside = Some(Vec::new());
        } else if trimmed == end {
            if let Some(lines) = inside.take() {
                passages.push(lines.join("\n").trim().to_owned());
            }
        } else if let Some(lines) = &mut inside
            && !is_mark(trimmed)
        {
            lines.push(line);
        }
    }
    Some(format!(
        "# Duck Mold guide: {name}\n\n({about}. `duck-mold guide --list` lists the topics; `duck-mold guide all` prints the whole guide.)\n\n{}\n",
        passages.join("\n\n")
    ))
}

/// The list `duck-mold guide --list` prints: one topic a line.
pub fn list() -> String {
    let widest = TOPICS.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    let mut out = String::from("Topics, for `duck-mold guide <topic>`:\n");
    for (name, about) in TOPICS {
        out.push_str(&format!("  {name:<widest$}  {about}\n"));
    }
    out.push_str(&format!("  {:<widest$}  the whole guide\n", "all"));
    out
}

/// The whole guide, with the topic marks taken out.
pub fn full() -> String {
    FULL.lines()
        .filter(|line| !is_mark(line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_mark(line: &str) -> bool {
    line.starts_with("<!-- topic: ") || line.starts_with("<!-- /topic: ")
}
