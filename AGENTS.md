# Duck Mold, for AI agents

Duck Mold makes game-ready 3D models from text. A **recipe** (a TOML or
JSON file) lists a palette and the parts of a model and which part joins
which; `duck-mold` builds it, reports on it in words and numbers, draws
it, and writes a `.glb` for the Duck Forge game engine (or any glTF
program). Duck Mold Studio is the same thing with a window, for people.

## Making models with it

Start with the guide: it is one page, and the rest comes a topic at a
time.

```bash
duck-mold guide                    # read this first
duck-mold guide --list             # the topics: recipes, edits, shaping, motion...
duck-mold new lamp.toml            # a new recipe (--example robot copies an example)
duck-mold edit lamp.toml set box --color red    # change it
duck-mold check lamp.toml --json   # every problem, worst first, with its fix
duck-mold render lamp.toml         # draw it: lamp.png
duck-mold export lamp.toml         # write lamp.glb
```

In an app that speaks MCP, `duck-mold mcp` gives the same commands as
tools: `claude mcp add duck-mold -- duck-mold mcp`.

The rules:

- **Change recipes with `duck-mold edit`, not by hand.** It keeps the
  file valid and its comments, and refuses a change that adds a mistake.
- **Check after every change** with `duck-mold check` (or `duck-mold inspect`).
  Read the numbers, not only `"ok"`. Each problem has `fix`, the command
  that fixes it, or `advice`.
- **Look with `duck-mold render`** when you can read images.
- Exit codes: 0 worked (warnings too), 1 the recipe has mistakes, 2 the
  command was wrong or a file couldn't be read or written.

## Working on the code

A Rust workspace (Rust 1.97, edition 2024):

| Crate | What it is |
|---|---|
| `crates/duck-mold-core` | Recipes, building, inspecting, editing, `.glb` in and out. No window, no drawing |
| `crates/duck-mold-view` | Drawing: `render`, the preview, the Studio's viewport |
| `crates/duck-mold-cli` | The `duck-mold` program: commands, the guide, `check`, `mcp` |
| `crates/duck-mold-studio` | Duck Mold Studio, the window |
| `crates/duck-mold-studio-app` | The `DuckMoldStudio` program that starts it |

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p duck-mold-cli -- guide                 # run the program from the source
./start-duck-mold-studio.sh                         # build and start the Studio (Linux, Mac)
```

**Before a commit:** the tests, `cargo fmt --all` and clippy with
`-D warnings`, all clean. CI runs them on Windows, Linux and macOS.

Stored results that tests compare with, and how to accept a change you
meant:

- The JSON and `.glb` of every example, in `tests/golden`:
  `DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test golden`.
- The Studio's screens, in `crates/duck-mold-studio/tests/screens`
  (Linux only, drawn by a software driver):
  `UPDATE_SNAPSHOTS=1 cargo test -p duck-mold-studio --test pictures`.
  Look at every picture that changed before you keep it.

**With no internet**: once `cargo fetch` has run (or from the source
download, `duck-mold-<version>-source.tar.gz`, which has every library
in it), `cargo build --offline -p duck-mold-cli --release` builds
`duck-mold` with nothing downloaded.

Other things to know:

- `docs/AI_GUIDE.md` is the full guide; `duck-mold guide <topic>` cuts
  its topics out at the `<!-- topic: ... -->` marks, and
  `docs/guide-start.md` is the first page. A change to the recipe format
  changes `docs/recipe-format.md`, `docs/recipe.schema.json` (a test
  keeps it in step with the reader) and the guide.
- `DESIGN.md` says why things are as they are, a section a phase.
- Words for people, in messages and docs, are plain English with no
  jargon: "hit shape", not "collider", where people read it.
- The same recipe always gives the same bytes: keep it so.
- Duck Forge comes from GitHub at a fixed commit (`Cargo.toml`); its
  copy of the `.glb` reader must match the writer here.
