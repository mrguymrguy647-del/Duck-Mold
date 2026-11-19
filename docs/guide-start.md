# Duck Mold: start here

Duck Mold makes game-ready 3D models from text. A **recipe** (a TOML or
JSON file) lists a palette and the parts of a model, and which part
joins which: there are no coordinates to work out. `duck-mold` builds
it, tells you in words and numbers how it came out, draws it, and writes
a `.glb` file for the Duck Forge game engine (any glTF program opens it
too).

This page is what you need first. `duck-mold guide <topic>` prints the
rest a part at a time, and `duck-mold guide all` prints the whole guide.

## The commands

```bash
duck-mold new lamp.toml                  # a new recipe: one box (--example robot copies an example)
duck-mold inspect lamp.toml --json       # build it in memory: size, parts, ground, problems
duck-mold check lamp.toml --json         # everything at once: problems worst first, each with its fix
duck-mold edit lamp.toml set shade --color glow    # change it; never edit the text by hand
duck-mold edit lamp.toml --batch edits.txt         # many edits, one a line, written once or not at all
duck-mold render lamp.toml               # draw it: lamp.png, four views on one sheet
duck-mold export lamp.toml               # write lamp.glb
duck-mold guide recipes                  # one part of the guide (guide --list lists them)
duck-mold schema                         # the recipe format as a JSON Schema
```

Add `--json` to any of them for one line of JSON on standard output
(`--pretty` spreads it out). `duck-mold <command> --help` explains one;
`duck-mold edit --help` lists every edit, grouped.

## A recipe

```toml
version = 0
name = "tree"

[palette]
bark = "#6B4A2B"
leaf = "#3F8F4A"

[[parts]]
name = "trunk"
shape = "cylinder"   # box, sphere, cylinder, cone, capsule, torus, tube, wedge, pyramid, prism, stairs, arch
radius = 0.15
height = 1.2
color = "bark"

[[parts]]
name = "leaves"
shape = "cone"
radius = 0.7
height = 1.6
color = "leaf"
attach = "bottom"    # my socket...
to = "trunk.top"     # ...joins the trunk's socket
```

Sizes are in metres; +y is up and the model faces +z. Every part has
sockets (`top`, `bottom`, `left`, `right`, `front`, `back`, `center`).

## The loop

1. Make a recipe: `new`, with `--example` to start from one that works.
2. Change it with `edit` (`--dry-run` to see first). Each edit changes
   only what it must, and is refused if it would add a mistake.
3. Run `check --json` (or `inspect --json`). If `"ok"` is false, fix
   every error: each names a line, a field and usually the fix.
4. If you can see images, `render` and look: is each part where you
   meant, facing the right way, the right colour?
5. When it is right, `export`.

## Rules

- **Change recipes with `edit`**, not by hand: it keeps the file valid
  and its comments, and says what changed.
- **Check with `check` or `inspect`** after every change; read the
  numbers, not only "ok".
- **Look with `render`** when you can read images.
- Exit codes: **0** it worked (warnings too), **1** the recipe has
  mistakes, **2** the command was wrong or a file couldn't be read or
  written. With `--json`, standard output is one JSON document, also
  when it failed.
- The same recipe always gives the same bytes.

## The topics

`duck-mold guide <topic>`: `recipes` (every field, sockets, common
models), `edits` (every edit command), `shaping` (pulling and cutting a
box), `motion` (limits, poses, animations), `hits` (hit shapes for
games), `images` (pictures on faces), `export` (into a game; far-away
versions), `mistakes` (reading the report, errors, what to avoid).
