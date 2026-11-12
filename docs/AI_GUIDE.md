# Duck Mold: a guide for AI agents

Duck Mold makes game-ready 3D models from text. You write a **recipe** (a
TOML or JSON file that lists a palette and the parts of a model), and the
`duck-mold` program builds it and tells you, in words and numbers, how it
came out. The numbers answer most questions; when you can read images,
`duck-mold render` draws the model too, so you can check it looks right.

This guide is all you need. The same text is printed by `duck-mold guide`.

## 1. The loop

```bash
duck-mold inspect lamp.toml --json    # check it, build it in memory, measure it
```

```bash
duck-mold render lamp.toml            # draw it: lamp.png, four views on one sheet
```

```bash
duck-mold export lamp.toml            # write lamp.glb next to the recipe
```

1. Write or edit the recipe.
2. Run `inspect --json`. If `"ok"` is false, fix every entry of `"errors"`
   (each names a line, a field and usually the fix) and run it again.
3. Read the report: `size`, `triangles`, `ground`, `floating`,
   `overlapping`, `warnings`. Compare them with what you meant (section 7).
4. If you can look at images, run `render` and look at the PNG: is each
   part where you meant, facing the right way, the right colour?
5. When it is right, run `export`.

`render` draws a sheet of four views, each a square (256 pixels unless
`--size` says otherwise): **front** (top left, looking the model in the
face, so its left is on your right), **right side** (top right, the
model facing right), **back** (bottom left) and **three-quarter** (bottom
right, from the front right and above). Every view is at the same scale.
Under them is the model's name, size and triangle count. A light grey
face is one nobody coloured. With `--json`, `views` says where each view
is in the picture. It writes the PNG next to the recipe unless `-o`
says where, and draws an `.obj` or `.glb` as it is, too.

Exit codes: **0** it worked (warnings included), **1** the recipe has
errors, **2** the command was wrong or a file couldn't be read or written.
With `--json`, standard output is exactly one JSON document, also when it
failed. Without it, the report is on standard output and the problems are
on standard error.

The same recipe always gives exactly the same bytes, so a change you see
is a change you made.

## 2. Set up (once)

You need Rust (`cargo`). From the repository folder:

```bash
cargo build --release
```

The program is `target/release/duck-mold` (`duck-mold.exe` on Windows).
It isn't put on the PATH: give its full path, or run it through cargo
with `cargo run --release -q -- inspect examples/robot.toml`. Check that
it works:

```bash
target/release/duck-mold inspect examples/robot.toml
```

## 3. A first recipe

```toml
version = 0
name = "lamp"

[palette]
brass = "#B8963E"
shade = "#F2E6C8"

[[parts]]
name = "base"
shape = "cylinder"
radius = 0.2
height = 0.05
color = "brass"

[[parts]]
name = "stem"
shape = "cylinder"
radius = 0.02
height = 0.6
color = "brass"
attach = "bottom"       # my socket...
to = "base.top"         # ...joins that part's socket

[[parts]]
name = "shade"
shape = "cone"
radius = 0.25
height = 0.3
color = "shade"
attach = "bottom"
to = "stem.top"
```

`duck-mold inspect lamp.toml` prints:

```
lamp  (lamp.toml)
  size         0.50 wide x 0.95 tall x 0.50 deep, in metres (x, y, z)
  triangles    150  (256 corners)
  ground       rests on the ground
  colours      2:  brass 120, shade 30  (triangles each)
  parts        3
    base    cylinder   60 triangles
    stem    cylinder   60 triangles   on base.top
    shade   cone       30 triangles   on stem.top
  floating     none
  overlapping  none
ok: no problems found
```

There are no coordinates in that recipe. Parts are joined **socket to
socket, by name**. That is the main idea: prefer sockets to positions.

## 4. Directions and units

- **x is right, y is up, z is forward.** 1 unit is about a metre. These
  are Duck Forge's directions.
- **A model faces forward (+z).** `front` is +z, `back` is -z, `left` is
  -x, `right` is +x, `top` is +y, `bottom` is -y. Left and right are the
  model's own: a character's left arm is at -x.
- **Angles are degrees.**
- **The ground is y = 0.** By default a finished model stands on it,
  centred over its zero point.

## 5. The recipe on one page

The top of the file:

| Field | |
|---|---|
| `version` | Required. `0`. |
| `name` | The model's name. Default: the file's name. |
| `description`, `note` | For people. Ignored. |
| `palette` | Colour names: `name = "#RRGGBB"`. Or the path of a palette file, to share one between models. |
| `define` | Parts defined once, to `use` many times. |
| `parts` | Required. The model's parts, at least one. |
| `sockets` | Extra sockets on the whole model. |
| `origin` | The model's zero point: a socket, like `"bottom"` (the default), `"center"` or `"trunk.bottom"`. |

A part (an entry of `parts`, or of `define`) gets its shape from
**exactly one** of:

| Field | The part is |
|---|---|
| `shape = "box"` | a built-in shape (table below) |
| `use = "leg"` | a copy of a part from `define` |
| `file = "wheel.toml"` | another recipe, or an `.obj` mesh. The path starts from the recipe's folder. |
| `parts = [ ... ]` | an assembly: a list of parts of its own |

Its other fields:

| Field | |
|---|---|
| `name` | Letters, digits, `_`, `-`. Needed if another part attaches to it. |
| `color` | A palette name: the colour of the whole part. |
| `colors` | Colours for single faces: `{ top = "grass", sides = "dirt" }`. |
| `attach`, `to` | Join my socket `attach` to `to = "part.socket"`, a socket of another part in the same list. Always both. |
| `rotate` | **Attached part:** one number, degrees to twist around the join. **Free part:** `[x, y, z]` degrees, around x, then y, then z. |
| `move` | Free part only: `[x, y, z]`, where its middle goes. |
| `scale` | One number or `[x, y, z]`, above zero. |
| `mirror` | `"x"`, `"y"` or `"z"`: flips the part, its sockets too. `mirror = "x"` makes a right arm from a left arm. |
| `sockets` | The part's own named sockets (section 6). |
| `note` | For people. Ignored. |

A part in `define` has no `name`, `attach`, `to`, `move`, `rotate`,
`scale` or `mirror`: those go on the part that uses it.

The shapes. All are centred on their own middle.

| `shape` | Size fields (default) | `segments` | Face names | Triangles |
|---|---|---|---|---|
| `box` | `size = [x, y, z]` (`[1, 1, 1]`) | none | `top`, `bottom`, `left`, `right`, `front`, `back`; `sides` for the last four | 12 |
| `cylinder` | `radius` (0.5), `height` (1); upright | 3 to 64 (16) | `top`, `bottom`, `side` | 4 × segments − 4 |
| `sphere` | `radius` (0.5) | 4 to 64 (12) | `side` | segments × (segments − 2), for even counts |
| `cone` | `radius` (0.5), `height` (1); point up | 3 to 64 (16) | `side`, `bottom` | 2 × segments − 2 |
| `wedge` | `size = [x, y, z]` (`[1, 1, 1]`); a ramp, full height at the back, sloping down to the front edge | none | `slope`, `bottom`, `back`, `left`, `right` | 8 |

Use few segments: 8 is usually enough for a game model. A 4-sided cone
is a square pyramid; a 4-sided cylinder is a box. A round shape's radius
is measured to the middle of a flat side, so radius 0.5 is exactly 1
wide (when the sides come in fours).

Sizes go from 0.001 to 1000. A model can have at most 1,000,000
triangles and a palette at most 256 colours.

**Colour rules.** A `color` must be a palette name; a `"#RRGGBB"` there
is an error that tells you to add it to the palette. `colors` beats
`color`. A part's `color` is the colour of everything inside it that has
none of its own, so the nearest setting wins. A face with no colour at
all is built plain grey, with a warning. An assembly has no faces of its
own: `colors` on it is an error; colour the parts inside.

## 6. Sockets

A socket is a named point on a part with a direction it faces. Two
sockets join face to face: the points meet and the directions oppose.

**Every part has seven sockets built in**: `top`, `bottom`, `left`,
`right`, `front`, `back` (the middle of each side, facing out) and
`center`. A **wedge** has an eighth, `slope`: the middle of its slope,
facing straight out of it.

**On a shape, a socket sits on its surface, never in the air.** Each
built-in socket starts in the middle of a side of the box around the
shape; if the shape isn't there, the socket moves straight in until it
touches the shape, and faces straight out of the surface. So a
**cone**'s `left`, `right`, `front` and `back` are halfway up its slant,
tilted up with it, and a **wedge**'s `top` and `front` are both the
middle of its slope (the same place as `slope`). On a box, and on a
cylinder's or sphere's sides, they are where the box is. An assembly, or
a part made from a file, keeps its sockets on the box around it.

Declare more in a part's `sockets` table, one of three ways:

```toml
[parts.sockets]
neck     = { on = "top" }                              # the middle of a side
left_leg = { on = "bottom", shift = [-0.5, 0, 0] }     # slid halfway to the left edge
hook     = { at = [0.2, 0.9, 0], facing = "front" }    # an exact point in the part's own space
grip     = { from = "hand.bottom" }                    # a socket of a part inside this assembly
```

- `shift` slides a socket across its side: each number runs from -1 (the
  left, bottom or back edge) to 1 (the right, top or front edge). The
  number for the direction the side faces must be 0. Sockets placed this
  way keep their place when the part changes size. On a round or sloped
  side the socket then moves straight in onto the surface and faces out
  of it, as the built-in ones do: eyes `on = "front"` of a sphere sit on
  the sphere and look out of it (`examples/snowman.toml`). A shift that
  misses the shape altogether is an error.
- `at` is in the part's own units, measured from its middle. `facing` is
  `up`, `down`, `left`, `right`, `front`, `back` or `[x, y, z]`; without
  it the socket faces up. `facing` also works with `on`, and wins over
  the surface's: `{ on = "top", shift = [0.5, 0, 0.5], facing = "up" }`
  on a wedge stands a chimney upright on its slope.
- `from` passes a socket of an inner part to the outside of an assembly.

**How a joined part is turned.** It is turned as little as possible to
make its socket face the other one. A part whose `bottom` joins a `top`
is not turned at all. A part whose `top` joins a `right` socket lies
down, pointing right, its front still in front. When both sockets face
the same way, the part is turned right round the upright axis (it stays
the right way up); if they face up or down, it rolls over sideways (its
front stays in front).

**`rotate` on a joined part** twists it around the join, clockwise seen
from the side the other socket faces. On a socket that faces up,
`rotate = 90` is the same turn as `rotate = [0, 90, 0]` on a free part:
forward turns to the right.

`mirror` and `scale` are done to a part before it is joined, sockets
included: a mirrored arm's shoulder socket is on its other side.

Parts in one list form a tree. `to` can only name a part **in the same
list**, by its `name`. To reach inside an assembly, give the assembly a
socket with `from`.

## 7. Reading the report

`inspect --json` prints (shortened):

```json
{
  "ok": true,
  "summary": "lamp: 0.50 x 0.95 x 0.50 m, 150 triangles, 3 parts, 2 colours, rests on the ground, nothing floating, nothing overlapping",
  "source": "lamp.toml",
  "name": "lamp",
  "size": [0.5, 0.95, 0.5],
  "bounds": { "min": [-0.25, 0.0, -0.25], "max": [0.25, 0.95, 0.25] },
  "triangles": 150,
  "vertices": 256,
  "ground": { "rests_on_ground": true, "lowest_y": 0.0 },
  "palette": [ { "name": "brass", "hex": "#B8963E", "triangles": 120 } ],
  "parts": [
    { "name": "stem", "kind": "cylinder", "triangles": 60,
      "size": [0.04, 0.6, 0.04],
      "bounds": { "min": [-0.02, 0.05, -0.02], "max": [0.02, 0.65, 0.02] },
      "colors": ["brass"], "attached_to": "base.top", "parts": [] }
  ],
  "floating": [],
  "overlapping": [],
  "warnings": [],
  "errors": []
}
```

All lengths are metres; `size` and every point are `[x, y, z]`.

What to check, in this order:

1. **`ok` and `errors`.** Fix all errors first.
2. **`size`.** Is the model the size you meant? A chair is about 1 tall;
   a 10-metre chair means a wrong `size`, `scale` or socket.
3. **`floating`.** Each entry is a part that touches nothing:
   `{ "part": "knob", "nearest": "peg", "gap": 0.1 }`. Almost always a
   mistake: attach it to a socket, or move it by `gap`.
4. **`overlapping`.** Each entry is one part poking into another:
   `{ "a": "peg", "b": "base", "depth": 0.3, "hidden": false }` means
   `peg` is 0.3 deep inside `base`. Overlap is often meant (a nose
   pushed into a head), so it is a fact, not an error: decide whether
   you meant it. `"hidden": true` means `a` is entirely inside `b` and
   can never be seen: a mistake, or a wasted part.
5. **`ground`.** `rests_on_ground` should be true unless you set
   `origin` on purpose.
6. **`parts`.** Each part's `bounds` says where it ended up, and
   `attached_to` what it is joined to. `kind` is its shape (`assembly`
   for a part made of parts, `mesh` for an `.obj`); `uses` names its
   `define`. A part inside another is named with a slash:
   `left_arm/hand`.
7. **`palette`.** How many triangles each colour paints.
   `unpainted_triangles` appears when some faces have no colour.
8. **`triangles`.** Keep it low; drop `segments` if it is high.
9. **`warnings`.** Things that are probably mistakes (below).

Two parts *touch* when their surfaces come within a millimetre. The
count of parts in the summary is of parts that have a shape of their own
(not assemblies). `depth` is measured at corners, edge middles and face
middles, so it is a good estimate rather than an exact figure.

## 8. Errors and warnings

Every problem is reported at once. As text, one per line:

```
error: lamp.toml:12: parts[0].colour: there is no field "colour" here. Did you mean "color"?
error: lamp.toml:19: parts[1].color: there is no colour "bras" in the palette. Did you mean "brass"?
error: lamp.toml:21: parts[1].to: there is no part "bace" in this list. Did you mean "base"?
error: lamp.toml:26: parts[2].segments: 100 is too big. Allowed: 3 to 64.
error: lamp.toml:28: parts[2].attach: this part has no socket "botom". Did you mean "bottom"?
```

As JSON (`"ok": false`, with `errors` and `warnings`), each is:

```json
{ "severity": "error", "code": "unknown_field", "file": "lamp.toml",
  "line": 12, "column": 1, "path": "parts[0].colour",
  "message": "there is no field \"colour\" here", "got": "colour",
  "allowed": { "options": ["name", "shape", "..."] },
  "suggestion": "color" }
```

`path` is the field (`parts[2]` is the third part: counting starts at
0). `suggestion` is the word that was probably meant: use it.
`allowed` is `{ "options": [...] }` or `{ "min": ..., "max": ... }`.

| `code` | What to do |
|---|---|
| `syntax` | The file isn't valid TOML or JSON. Fix the line it names. |
| `unknown_field` | A misspelt field. Use `suggestion`. The field is spelt `color`, not `colour`. |
| `missing_field` | Add the field the message names. |
| `type_mismatch` | Wrong kind of value; the message shows an example. |
| `out_of_range` | Use a number inside `allowed`. |
| `unsupported_version` | `version` must be `0`. |
| `unknown_shape`, `unknown_color`, `unknown_part`, `unknown_socket`, `unknown_face` | A name that doesn't exist. `allowed.options` lists the ones that do. |
| `duplicate_name` | Two parts, or two sockets, with one name. |
| `conflict` | Two fields that can't go together (`attach` with `move`; two sources of shape). The message says which to remove. |
| `cycle` | A part that uses itself, or parts attached in a ring. The message shows the chain. |
| `reserved` | `bones`, `colliders` and `lods` are kept for a later phase. Remove them. |
| `unsupported` | Not available yet: see-through colours, glTF import. |
| `file_not_found` | A file the recipe names can't be read. Paths start from the recipe's folder. |
| `bad_mesh` | A mistake inside an `.obj` file, with its line. |
| `io` | The file on the command line can't be read or written. |

Warnings (the model is still built; exit code 0):

| `code` | Meaning |
|---|---|
| `no_color` | A part has faces with no colour: they are plain grey. Give it a `color`. |
| `unused` | A palette colour or a defined part that nothing uses, or a `color` that changes nothing because every face inside already has one. |
| `floating` | A part touches nothing. |
| `hidden` | A part is entirely inside another. |
| `off_ground` | The model's lowest point isn't at y = 0. |
| `skipped` | Something in a mesh file was passed over. |

## 9. How to build common things

**Stack things.** Join `bottom` to `top`:

```toml
attach = "bottom"
to = "trunk.top"
```

**Put things side by side.** Join `left` to `right`, or `back` to
`front`.

**Four legs under a table.** Declare four sockets on the top's
underside with `shift`, define one leg, use it four times
(`examples/table.toml`).

**Stand something against a wall, on the floor.** Give the wall a
socket on the bottom edge of its side, and the thing a socket at the
foot of its back (`examples/house.toml`, the door):

```toml
# on the wall
door = { on = "front", shift = [0, -1, 0] }
# on the door
foot = { on = "back", shift = [0, -1, 0] }
```

**A left and a right of the same part.** Define it once with its socket
on the side that touches the body; use it twice, the second time with
`mirror = "x"` (`examples/robot.toml`, the arms).

**A part made of parts.** Put a `parts` list in a `define`, give it
sockets with `from`, and `use` it like any part (`examples/robot.toml`).

**A ridged roof.** Two wedges, the second attached `back` to the
first's `back` (`examples/house.toml`).

**Sink one part into another on purpose.** Use an `at` socket inside
the part (`examples/tree.toml`). `inspect` will list it under
`overlapping`; that is expected.

**Lay a part on its side.** A free part: `rotate = [0, 0, 90]`. A
joined part: join the socket that should touch.

**Share a palette between models.** `palette = "palette.toml"`, where
that file is a plain list of `name = "#RRGGBB"`.

**Use a mesh made elsewhere.** `file = "rock.obj"`. Run
`duck-mold inspect rock.obj` first: it lists the file's size and its
material names, which are the face names for `colors`. Use `scale` if
it was made in other units (`scale = 0.01` for centimetres).

**Choose where the model's zero point is.** `origin = "pole.bottom"`
puts it at a socket of one part instead of under the middle of the
whole model.

## 10. Mistakes to avoid

- **Typing positions when a socket would do.** A part placed with
  `move` floats or overlaps as soon as a size changes. Attach it.
- **`attach` without `to`** (or the other way round): they go together.
- **`to` naming a part in another list**, or one with no `name`. Parts
  attach to parts in their own list.
- **`rotate = [0, 90, 0]` on an attached part.** There it is one
  number: `rotate = 90`.
- **A hex code in `color`.** Colours go in the palette, by name.
- **`colour`.** The fields are spelt `color` and `colors`.
- **Left and right swapped.** They are the model's own: left is -x,
  and the model faces +z.
- **Expecting a part on a slope to stand upright.** A cone's sides and
  a wedge's `top`, `front` and `slope` face out of the slope, so what you
  join there leans with it. Declare a socket with `facing = "up"` to
  stand something upright on a slope.
- **Too many segments.** 64 on a sphere is 3,968 triangles. Try 8.
- **`colors` on an assembly.** Colour the parts inside it, or give the
  assembly a plain `color`.
- **Negative `scale`.** Use `mirror`.

## 11. What it can't do yet

Plan around these; don't look for them: no textures or UVs, no smooth
shading (every face is flat, as in Duck Forge), no see-through or
glowing colours, no booleans (cutting one shape out of another), no
bones, animation, colliders or LODs (`bones`, `colliders` and `lods` are
reserved names), no glTF import (only `.obj`), no window.

## 12. Files for reference

- `examples/`: crate, table, tree, house, robot, snowman. Read
  `robot.toml` for nested parts and `snowman.toml` for sockets on curved
  surfaces.
- `docs/recipe-format.md`: every field, default and range.
- `docs/forge-handoff.md`: what is in the `.glb`, and how Duck Forge
  reads it.
- `DESIGN.md`: why each thing is the way it is.
