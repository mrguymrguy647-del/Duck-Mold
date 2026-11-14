//! The starter animations that come inside the program: `idle`, `walk`,
//! `wave` and `jump` for a two-legged character, `open` for a door and
//! `roll` for four wheels.
//!
//! Each is a small file like the ones `duck-mold save-animation`
//! writes, and is copied into a recipe the same way: `duck-mold edit
//! <recipe> add-animation walk --starter walk`. An animation only names
//! parts, so it works on any model whose parts have those names.

/// The names of a two-legged character's parts, which `idle`, `walk`,
/// `wave` and `jump` move. Each stands at rest with the arms hanging
/// down. (An elbow or a knee inside them is a bonus the starters don't
/// need.)
pub const TWO_LEGS: [&str; 6] = [
    "body",
    "head",
    "left_arm",
    "right_arm",
    "left_leg",
    "right_leg",
];

/// A starter animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Starter {
    /// Its name, which is also the animation's.
    pub name: &'static str,
    /// What kind of model it is for, in a few words.
    pub made_for: &'static str,
    /// What it does, in a sentence.
    pub does: &'static str,
    /// The parts it moves.
    pub parts: &'static [&'static str],
    /// The file, as TOML.
    pub text: &'static str,
}

/// The six starters, in the order they are listed.
pub const STARTERS: [Starter; 6] = [
    Starter {
        name: "idle",
        made_for: "two legs",
        does: "Stands and breathes: the head nods a little and the arms sway. Loops.",
        parts: &["head", "left_arm", "right_arm"],
        text: include_str!("../starters/idle.toml"),
    },
    Starter {
        name: "walk",
        made_for: "two legs",
        does: "Legs and arms swing against each other. Loops, in place.",
        parts: &["left_leg", "right_leg", "left_arm", "right_arm"],
        text: include_str!("../starters/walk.toml"),
    },
    Starter {
        name: "wave",
        made_for: "two legs",
        does: "Raises the right arm, waves twice, lowers it.",
        parts: &["right_arm"],
        text: include_str!("../starters/wave.toml"),
    },
    Starter {
        name: "jump",
        made_for: "two legs",
        does: "Crouches, rises 40 cm, lands, straightens.",
        parts: &["body", "left_leg", "right_leg", "left_arm", "right_arm"],
        text: include_str!("../starters/jump.toml"),
    },
    Starter {
        name: "open",
        made_for: "a door",
        does: "The part called door swings 100 degrees on its hinge. Backwards, it closes.",
        parts: &["door"],
        text: include_str!("../starters/open.toml"),
    },
    Starter {
        name: "roll",
        made_for: "four wheels",
        does: "Each wheel turns once round a second, steadily. Loops.",
        parts: &[
            "wheel_front_left",
            "wheel_front_right",
            "wheel_back_left",
            "wheel_back_right",
        ],
        text: include_str!("../starters/roll.toml"),
    },
];

/// The starter called `name`.
pub fn starter(name: &str) -> Option<&'static Starter> {
    STARTERS.iter().find(|starter| starter.name == name)
}
