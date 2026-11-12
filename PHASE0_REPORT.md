# Phase 0 report

Phase 0 is done: Duck Mold is a library and a command-line program that
turns a text recipe into a game-ready `.glb` model and reports on it in
words and numbers. This report says what works, what doesn't yet, what
was decided along the way, and where Phase 1 should start.

## In short

- All eight steps are built, in the order you gave, one commit per step.
  The only thing moved out is **glTF import**, to Phase 1, as agreed.
- **194 tests pass.** `cargo fmt` and `cargo clippy` (warnings as errors)
  are clean.
- The five examples build to exactly the same bytes every time, and the
  golden-file tests hold that.
- **One thing is not finished, and can't be from here: Duck Forge cannot
  load the models yet.** It has no model loader. A tested reader and a
  handoff note are ready for the Duck Forge session (see "For the Duck
  Forge session" below).

## What works

| Step | What it does | Held by |
|---|---|---|
| 1. Workspace | `duck-mold-core` (the library) and `duck-mold-cli` (the `duck-mold` program). No `unsafe`; every public item has a comment. | The build itself. |
| 2. Recipes | TOML and JSON, the same fields. Every mistake is reported at once with its file, line, field and, where it can be guessed, the fix. | 31 tests in `recipes.rs`, one or more per kind of mistake. |
| 3. Shapes | Box, cylinder, sphere, cone, wedge, with sizes and `segments`; `move`, `rotate`, `scale`, `mirror`. Turns go Duck Forge's way. | Unit tests in `shapes.rs` and `math.rs`: every shape faces outwards (Duck Forge's own test), is closed and flat-faced, and stays so after any move, turn, stretch or flip. |
| 4. Parts and sockets | Seven built-in sockets on every part; declared sockets (`on`, `at`, `from`); `attach` and `to`; `define` and `use`; parts inside parts; a whole recipe file as a part; `origin`. | 33 tests in `assembly.rs`. |
| 5. Colour | A palette of names; `color` for a part, `colors` for faces; a shared palette file. | 15 tests in `colour.rs`. |
| 6. Export | A `.glb` that follows glTF's rules, with vertex colours, the palette and the parts. | 13 tests in `glb.rs`: the file is checked against glTF's rules byte by byte, read by the reader written for Duck Forge, and read back by Duck Mold. |
| 7. `inspect` | Size, triangles, colours, the tree of parts, the ground, floating parts (with the gap), overlapping parts (with the depth). Text for people, `--json` for agents. | 22 tests in `inspect.rs`, 14 in `cli.rs`. |
| 8. Import | An `.obj` file as a static part, coloured from the palette by material name. `inspect` also measures an `.obj` on its own. | 13 tests in `import.rs`. |

Also there:

- **Examples:** crate, table, tree, house and robot, each commented line
  by line, and the crate again as JSON.
- **Golden files** (`tests/golden/`): each example's exact `.glb` and
  exact `inspect --json` report. Seven tests run the real program and
  compare every byte, build twice, compare the TOML and JSON crates, and
  read every golden `.glb` with the Duck Forge reader.
- **Documents:** `README.md` for people, `docs/AI_GUIDE.md` for agents
  (also printed by `duck-mold guide`), `docs/recipe-format.md` (every
  field), `docs/forge-handoff.md`, and `DESIGN.md`, kept in step with
  the code. The guide's and the README's first recipes are tested: they
  must build to exactly the reports printed beside them.

One check outside the test suite: the **official Khronos glTF validator**
(version 2.0.0-dev.3.10) was run by hand on the five golden `.glb` files.
It reports no errors, no warnings, no infos and no hints on any of them.

## How to check it yourself

```bash
cargo test --workspace
```

```bash
cargo run --release -q -- inspect examples/robot.toml
```

```bash
cargo run --release -q -- export examples/robot.toml
```

The last one writes `examples/robot.glb`, which any glTF viewer opens.

## Numbers

- About 8,500 lines of Rust in the two crates (comments included) and
  4,800 lines of tests.
- 194 tests: 46 inside the library's files, 127 in its `tests/` folder,
  21 that run the program.
- Four outside libraries, as designed: `toml`, `serde`, `serde_json`,
  `clap`.
- A release build takes about 13 seconds here; the program is 2 MB.
- The robot example: 384 triangles, a 35 KB `.glb`.

## Known limits

**Duck Forge can't load the models yet.** This is the big one, and it is
on Duck Forge's side.

**Limits of what Duck Mold makes:**

- No glTF import. A `.glb` or `.gltf` named in a recipe gets a "not yet"
  message. `inspect` reads only the `.glb` files Duck Mold wrote.
- Everything is flat-shaded, so round shapes look faceted. That matches
  Duck Forge, which lights each triangle evenly.
- Colours are opaque. No see-through, no glow.
- Where two parts join, the two faces that meet are both still in the
  model, hidden inside it. They cost a few triangles each (a table leg's
  top, for instance). Nothing removes them yet.
- The model is one mesh. Sockets are not written into the `.glb`, so a
  game can't yet ask "where is the robot's hand?".

**Limits of the built-in sockets.** They sit on the box around a part,
which for a sloped shape is not always its surface. A cone's `left`,
`right`, `front` and `back`, and a wedge's `top` and `front`, are in the
air. A part joined there floats. `inspect` reports it, and the guide
warns about it, but it is a trap. See "Decisions I'd revisit".

**Limits of `inspect`:**

- The depth of an overlap is an estimate: it is measured at the corners,
  edge middles and face middles of each part. A test covers the awkward
  case of two crossed bars, but an odd enough overlap could be
  under-measured.
- "Inside" only means something for a closed surface. An `.obj` with
  holes in it can be tested for touching, but not for overlapping or
  being hidden.
- Two faces in the same plane that would flicker in a game are not
  detected.
- It compares every pair of parts whose boxes are within reach. That is
  instant for ordinary models (400 touching bricks: 0.01 s), but a
  cluster of high-detail parts all pushed into each other is slow (20
  spheres of 32 segments, all overlapping: 3.6 s).
- Reading a `.glb` back gives the parts as a flat list, not a tree, and
  cannot tell apart two palette colours that are the same colour.

**Limits of the proof that output is identical everywhere.** The design
goes to some trouble for this (its own sine and cosine, a fixed colour
table, rounded positions, file order everywhere), and the golden tests
hold it on this machine. But they have only been run on Linux. Nothing
has run on Windows or a Mac. See "What Phase 1 should start with".

**Smaller things:**

- `rust-version` says 1.95 (what Duck Quill needs). It was built and
  tested with 1.97 only.
- Names (of parts, sockets, colours) are plain letters, digits, `_` and
  `-`. No accents.
- `--json` output is spread over many lines (every number of a list on
  its own). Fine for a program, long for a person.
- There is no licence, as decided.

## What changed from the approved design

`DESIGN.md` has been updated to match; this is the list, so nothing
changed quietly.

1. **`turn` became `rotate`** (your decision 5). `turn` was one number
   and `rotate` was `[x, y, z]`, so one name now has two forms:
   **one number on an attached part** (the twist around the join),
   **`[x, y, z]` on a free part**. Writing the wrong form is an error
   that shows the right one. This was my reading of the rename; if you
   meant something else, it is one small change.
2. **glTF import moved to Phase 1** (your decision 4).
3. **A part's `kind` in the report** is what it is made of (`box`,
   `assembly`, `mesh`...), with `uses` and `file` beside it. The design
   had `"kind": "use"`, which told you less.
4. **Unnamed parts count from 0**: `#2` is `parts[2]`, so the label and
   the error path agree. The design said `#3`.
5. **Problems are listed file by file, top line down**, rather than in
   the order they were found.
6. **A round shape's radius is measured to its flat sides**, not to its
   corners. So a radius of 0.5 is exactly 1 wide, a part joined to its
   side meets a flat face, and a 4-sided cone is a square pyramid like
   Duck Forge's. The design didn't say either way.
7. **Colour between files.** When a recipe uses another as a part, the
   other's colours join the palette; one name with two different colours
   is an error. The design didn't cover it.
8. **Three more warnings** than designed: a `color` that changes nothing
   (every face inside already has one), faces squashed to no area by an
   extreme `scale`, and things skipped in an `.obj`. And one more error:
   a part scaled down to nothing.
9. **Limits that weren't written down:** at most 1,000,000 triangles,
   parts nested at most 32 deep.
10. **The wording of messages** differs a little from the design's
    samples. The design now shows the real ones.
11. **Three files the design didn't list:** `load.rs` (reading the files
    a recipe names), `mesh.rs` (instead of `import.rs`) and `tools.rs`
    (what the commands do, so a window or an MCP server can call the
    same code later). And `.gitattributes`, so that the byte-for-byte
    tests also hold on Windows, where Git would otherwise change line
    endings.

## Decisions I'd revisit

1. **Built-in sockets on the box, not the surface.** Simple to explain
   and right for boxes, cylinders and spheres, but wrong for the sides of
   a cone and the top of a wedge. Better: sockets that know the shape,
   such as a wedge's `slope` (in the middle of the slope, facing out of
   it). This is the first thing I would change.
2. **`rotate` meaning two things.** It works and the errors are clear,
   but one field with two forms is a little surprising. If it grates in
   use, a second name for the twist (`twist`) would be clearer.
3. **Vertex colours.** Right for Duck Forge as it is today. If games
   want to swap team colours at run time, a palette number per corner is
   a small, compatible addition; a palette texture would be the bigger
   change.
4. **"The inner colour wins."** A defined part's own `color` is kept
   even when the part using it gives another, and the outer one then
   gets a "changes nothing" warning. That allows "fixed red hand, body
   colour up to you", but "use it, but blue" needs the definition to
   leave the colour out. Duck Quill's colour swap on `use` would give
   both.
5. **Hidden faces at joins** are kept. Removing them would cut triangle
   counts by a noticeable share on blocky models.
6. **The count of parts** in the summary counts parts with a shape of
   their own (the robot's 19), not the entries of the recipe's list (its
   11). Both are defensible; this one matches what "floating" and
   "overlapping" talk about.
7. **The fixed rule for sockets that face the same way** (turn round the
   upright axis; roll sideways if the sockets are upright). It is
   predictable, and it made the house's roof work, but it is the least
   obvious rule in the tool.

## For the Duck Forge session

- **Read `docs/forge-handoff.md`.** It lists exactly what a loader must
  support to read Duck Mold's `.glb` files, what it must do to the
  numbers (negate x, swap two corners, turn colours back into 0 to 255),
  and what it can ignore.
- **Copy `docs/forge/duck_mold_glb.rs`.** It is a working reader using
  only the standard library and `serde_json`, which Duck Forge already
  has, so its dependency test stays green. Duck Mold's tests run it
  against every example.
- **Test files:** `tests/golden/*.glb`, each with an `*.inspect.json`
  that says what the loader should find.
- **A comment in Duck Forge disagrees with Duck Mold, and should be
  fixed there.** `Mesh::cube` in Duck Forge's `src/render3d.rs` labels
  its faces in comments as `front (-z)` and `back (+z)`, because -z is
  the face a default camera sees. By your decision 1, a Duck Mold model
  **faces forward (+z)**: its `front` is +z and its `left` is its own
  left, -x. So the two projects use "front" for opposite faces. Nothing
  breaks (they are comments, and a cube is the same from both sides),
  but it will confuse the next reader. Suggested fix: change the cube's
  comments to `near (-z)` and `far (+z)`, and add a line to section 11
  of Duck Forge's `docs/AI_GUIDE.md` saying that a model faces +z.
- **One consequence to know:** a default `Camera3D` looks along +z, so
  it sees a newly loaded model from behind. To face the camera, turn the
  model half round.

## What Phase 1 should start with

In this order:

1. **The Duck Forge loader**, in the Duck Forge repository. Until it
   exists nothing Duck Mold makes can be seen in a game, and seeing a
   model in a real game will find problems no test here can.
2. **Run the tests on Windows and Linux for every push** (Duck Quill has
   such a workflow to copy). That is what turns "the same bytes
   everywhere" from designed into proven. It is small, and I left it out
   only because it wasn't asked for.
3. **Sockets that know the shape** (decision 1 above), before many
   recipes are written against the current ones.
4. **Sockets in the `.glb`**, so a game can attach things at run time: a
   hat on `head.top`, a tool in `left_arm.grip`. The data is already in
   the built model.
5. **A picture.** Phase 0 deliberately has no rendering, and `inspect`
   answers most questions, but neither a person nor an agent can yet
   tell whether a model *looks* right. A small `duck-mold render` to a
   PNG, or the window you have planned, would close that. Duck Forge's
   CPU renderer may be reusable once the loader exists.
6. **glTF import**, moved from Phase 0.

After those, in no fixed order: bones from socket joins (the names
`bones`, `colliders` and `lods` are reserved, and the built model keeps
every part's place and sockets for this), mirror copies ("both arms in
one line"), colour swap on `use`, removing hidden faces, and an MCP
server over `tools.rs`, as Duck Echo has.
