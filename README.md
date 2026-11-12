# Duck Mold

Duck Mold makes game-ready 3D models from text. You describe a model in a
short file, a **recipe**: which parts it has, which part joins which, and
what colour each is. Duck Mold builds the model, tells you in words and
numbers how it came out, and writes a `.glb` file for the
[Duck Forge](https://github.com/mrguymrguy647-del/Duck-Forge) game engine.

It is not a modelling program. It is an assembler: you don't push points
around, you name parts and say how they fit together. It belongs with
[Duck Quill](https://github.com/mrguymrguy647-del/duck-quill) (2D art)
and [Duck Echo](https://github.com/mrguymrguy647-del/Duck-Echo) (sound),
and like them it is made for two kinds of user: people who don't know
3D modelling or Rust, and AI agents.

**An AI agent?** Read [docs/AI_GUIDE.md](docs/AI_GUIDE.md), or run
`duck-mold guide`: the whole tool on one page, from scratch.

This is Phase 0: a command-line program and a library. There is no
window yet.

## A first model

```toml
version = 0
name = "tree"

[palette]
bark = "#6B4A2B"
leaf = "#3F8F4A"

[[parts]]
name = "trunk"
shape = "cylinder"
radius = 0.15
height = 1.2
color = "bark"

[[parts]]
name = "leaves"
shape = "cone"
radius = 0.7
height = 1.6
color = "leaf"
attach = "bottom"       # my socket...
to = "trunk.top"        # ...joins the trunk's socket
```

There are no positions in it. Every part has sockets (named points:
`top`, `bottom`, `left`, `right`, `front`, `back`, `center`), and
`attach` and `to` join one part's socket to another's. The cone sits on
the trunk because its `bottom` is joined to the trunk's `top`.

## Quick start

1. Install Rust from <https://rustup.rs> (version 1.95 or newer).
2. Get this repository, and in its folder run:

```bash
cargo build --release
```

The program is `target/release/duck-mold` (`target\release\duck-mold.exe`
on Windows). The first build takes under a minute.

3. Look at an example without writing anything:

```bash
target/release/duck-mold inspect examples/robot.toml
```

4. Build it:

```bash
target/release/duck-mold export examples/robot.toml
```

That writes `examples/robot.glb`. Any glTF viewer opens it: the 3D
Viewer that comes with Windows, Blender, or an online glTF viewer.

5. See it:

```bash
target/release/duck-mold render examples/robot.toml
```

That writes `examples/robot.png`, four views of the model on one sheet,
without opening a window:

![The robot from the front, the right, the back and three-quarter](tests/golden/robot.png)

## The commands

| Command | What it does |
|---|---|
| `duck-mold inspect <file>` | Checks a recipe, builds it in memory and reports on it. Writes nothing. Also measures an `.obj` mesh, or a `.glb` that Duck Mold wrote. |
| `duck-mold export <recipe>` | Builds the recipe and writes the `.glb` next to it. `-o other.glb` writes it somewhere else. |
| `duck-mold render <file>` | Draws the model and writes a PNG next to it: front, right side, back and three-quarter views. `-o` writes it somewhere else; `--size 384` makes each view 384 pixels square (256 unless you say). Draws an `.obj` or `.glb` as it is, too. |
| `duck-mold guide` | Prints the guide for AI agents. |
| `duck-mold help` | Lists the commands; `duck-mold inspect --help` explains one. |

Add `--json` to `inspect`, `export` or `render` to get the report as
JSON, for an agent or a script.

`render` draws with Duck Forge's own renderer, so the picture shows the
model the way a Duck Forge game draws it. The same recipe always gives
the same picture, byte for byte. Each view is drawn from far away and
zoomed in, so it looks nearly flat, like a drawing, and every view is at
the same scale, so the model is the same size in each.

The program ends with code 0 when it worked, 1 when the recipe has
mistakes, and 2 when the command itself was wrong or a file couldn't be
read or written.

## What `inspect` tells you

```
robot  (examples/robot.toml)
  size         0.90 wide x 1.83 tall x 0.39 deep, in metres (x, y, z)
  triangles    384  (792 corners)
  ground       rests on the ground
  colours      4:  steel 96, dark 180, red 84, glow 24  (triangles each)
  parts        19
    body          box              12 triangles
    panel         box              12 triangles   on body.chest
    head          box              12 triangles   on body.neck
    ...
    right_arm     arm (assembly)   84 triangles   on body.right_arm, mirrored
    ...
  floating     none
  overlapping  none
ok: no problems found
```

- **size**: how big the model is.
- **triangles**: how many triangles it is made of. Fewer is cheaper for
  a game to draw.
- **ground**: whether it stands on the ground.
- **colours**: how much of it each palette colour paints.
- **parts**: every part, what it is and what it is joined to.
- **floating**: parts that touch nothing. Almost always a mistake.
- **overlapping**: parts that poke into other parts, and how deep.
  Sometimes meant (a nose pushed into a head), sometimes not.

So you can tell whether a model came out right without opening it.

## When a recipe has a mistake

Every mistake is listed at once, each with its file, its line, the field
and, where it can be guessed, the fix:

```
error: lamp.toml:12: parts[0].colour: there is no field "colour" here. Did you mean "color"?
error: lamp.toml:19: parts[1].color: there is no colour "bras" in the palette. Did you mean "brass"?
error: lamp.toml:21: parts[1].to: there is no part "bace" in this list. Did you mean "base"?
error: lamp.toml:26: parts[2].segments: 100 is too big. Allowed: 3 to 64.
```

(`parts[0]` is the first part: the counting starts at 0.)

## The ideas

- **The recipe is the model.** The `.glb` is rebuilt from the recipe
  every time, and the same recipe always gives exactly the same file. So
  keep the recipe; the `.glb` can always be made again.
- **Parts and sockets.** A part is a shape (box, cylinder, sphere, cone,
  wedge), a mesh from an `.obj` file, or an assembly of other parts.
  Parts are joined socket to socket, by name. A part can declare sockets
  of its own: `neck = { on = "top" }`.
- **Define once, use many times.** A part under `[define.leg]` can be
  used four times. `mirror = "x"` makes a right arm from a left arm.
- **A small palette.** A model's colours are a short list of names;
  every face gets one of them. `color` paints a whole part, `colors`
  paints single faces: `colors = { top = "grass" }`.
- **Directions.** x is right, y is up, z is forward; 1 unit is about a
  metre. A model faces forward, so its `front` is +z and its `left` is
  -x. These are Duck Forge's directions.

A recipe can be written as TOML (the examples) or as JSON
([examples/crate.json](examples/crate.json)); the fields are the same.

## Examples

| Recipe | Shows |
|---|---|
| [crate.toml](examples/crate.toml) | The smallest useful recipe: one box, two colours. |
| [table.toml](examples/table.toml) | A part defined once and used four times, on sockets the table top declares. |
| [tree.toml](examples/tree.toml) | One tier of needles used three times at three sizes. |
| [house.toml](examples/house.toml) | A roof assembled from two wedges; a door, windows and a chimney placed by sockets. |
| [robot.toml](examples/robot.toml) | Parts made of parts, sockets passed on from inside, and a mirrored arm. |

Each is commented line by line.

## Using a model in Duck Forge

```rust
let robot = Mesh::load_glb("assets/robot.glb")?;
canvas.draw_mesh(&robot, &Transform::at(position), &camera);
```

Duck Forge reads Duck Mold's `.glb` files with the reader in
[docs/forge/duck_mold_glb.rs](docs/forge/duck_mold_glb.rs), a copy of
which is in Duck Forge. [docs/forge-handoff.md](docs/forge-handoff.md)
says what is in the file and how to keep the two copies the same.
A model faces +z, so a default Duck Forge camera sees it from behind:
see the handoff note.

## Documents

| | |
|---|---|
| [docs/AI_GUIDE.md](docs/AI_GUIDE.md) | For AI agents: the commands and the recipe format from scratch. |
| [docs/recipe-format.md](docs/recipe-format.md) | Every field of a recipe, with its default and range. |
| [docs/forge-handoff.md](docs/forge-handoff.md) | What is inside a `.glb`, and what Duck Forge must do to load one. |
| [DESIGN.md](DESIGN.md) | Why Duck Mold is the way it is, decision by decision, in plain words. |
| [PHASE0_REPORT.md](PHASE0_REPORT.md) | What works, what doesn't yet, and what comes next. |

## Project layout

```
Duck-Mold/
├── .github/workflows/ci.yml   the tests on Windows and Linux, for every push
├── Cargo.toml                 the workspace: three crates, built together
├── examples/                  five recipes to read and change
├── docs/                      the guide, the format, the Duck Forge handoff
├── tests/golden/              the examples' exact output (.glb, report, picture)
└── crates/
    ├── duck-mold-core/        the library: everything except pictures and the command line
    │   ├── src/
    │   │   ├── source.rs      TOML or JSON text -> one tree that remembers line numbers
    │   │   ├── issue.rs       mistakes as data: file, line, field, message, fix
    │   │   ├── recipe.rs      the recipe's fields, read from that tree and checked
    │   │   ├── load.rs        the files a recipe names: palettes, recipes, meshes
    │   │   ├── math.rs        points, turns, and an exact sine and cosine
    │   │   ├── shapes.rs      box, cylinder, sphere, cone, wedge
    │   │   ├── mesh.rs        reading .obj files
    │   │   ├── assemble.rs    parts, sockets, joins, nesting, colour
    │   │   ├── model.rs       the built model: a tree of parts and their faces
    │   │   ├── glb.rs         writing (and reading back) .glb
    │   │   ├── inspect.rs     size, ground, floating and overlapping parts
    │   │   └── tools.rs       what the commands do, for any front end
    │   └── tests/             one file of tests per step
    ├── duck-mold-view/        pictures (`render`), drawn by Duck Forge's CPU renderer
    └── duck-mold-cli/         the `duck-mold` program: reads the command line, prints
```

## Working on Duck Mold

```bash
cargo test --workspace
```

```bash
cargo fmt --all -- --check
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

All three must pass before a commit. GitHub runs them again, on Windows
and on Linux, for every push and pull request
(`.github/workflows/ci.yml`).

The tests include **golden files**: `tests/golden/` holds the exact
`.glb` and the exact report for each example, and the tests compare
every byte. If you change an example or the output on purpose, accept
the new files with

```bash
DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test golden
```

and say in the commit why they changed.
