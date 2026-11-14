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
duck-mold inspect lamp.toml --sockets # also list every named socket: head.top, ...
```

```bash
duck-mold edit lamp.toml set shade --color glow --json   # change it without typing (section 10)
```

(`duck-mold studio lamp.toml` opens the Studio, a window where a person
builds the model with the mouse, and `duck-mold preview lamp.toml` opens
it to look only. Both need a screen and wait until the window is closed:
as an agent, use `render` to see the model and `edit` to change it. The
Studio makes the same edits you do, in the same file, and picks up every
change you save while it is open.)

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
says where, and draws a mesh file (`.obj`, `.gltf`, `.glb`) as it is,
too.

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

To start from something that already works, copy one of the twelve
examples that are inside the program, and change it:

```bash
target/release/duck-mold new my_robot.toml --example robot
```

The examples are `robot`, `house`, `snowman`, `desk`, `duck`, `dog`
(capsules), `doughnut` (a torus), `mug` (a tube), `temple` (a pyramid
and stairs), `barn` (a prism), `slide` (stairs), `gate` (an arch,
taper and bevel), and three made by shaping one box: `chair` (cuts and
pulls), `window_wall` (insets) and `rock` (corners moved). `new` never
writes over a file. Without `--example`
it starts a recipe with one box in it.

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
| `poses` | The model's poses, by name: which parts are turned, and how far (section 11). |
| `animations` | The model's animations, by name: poses at times (section 11). |

A part (an entry of `parts`, or of `define`) gets its shape from
**exactly one** of:

| Field | The part is |
|---|---|
| `shape = "box"` | a built-in shape (table below) |
| `use = "leg"` | a copy of a part from `define` |
| `file = "wheel.toml"` | another recipe, or a mesh: `.obj`, `.gltf` or `.glb`. The path starts from the recipe's folder. |
| `parts = [ ... ]` | an assembly: a list of parts of its own |

Its other fields:

| Field | |
|---|---|
| `name` | Letters, digits, `_`, `-`. Needed if another part attaches to it. |
| `color` | A palette name: the colour of the whole part. |
| `colors` | Colours for single faces: `{ top = "grass", sides = "dirt" }`. |
| `changes` | A box only: changes made to its shape once it is built, in order. See "Shaping a box" below. |
| `attach`, `to` | Join my socket `attach` to `to = "part.socket"`, a socket of another part in the same list. Always both. |
| `twist` | Attached part only: one number, degrees to twist it around the join. |
| `rotate` | Free part only: `[x, y, z]` degrees, around x, then y, then z. |
| `move` | Free part only: `[x, y, z]`, where its middle goes. |
| `scale` | One number or `[x, y, z]`, above zero. |
| `mirror` | `"x"`, `"y"` or `"z"`: flips the part, its sockets too. `mirror = "x"` makes a right arm from a left arm. |
| `limits` | How far the part may turn when the model is posed: `{ x = [-30, 40], y = [-80, 80] }`, or `"fixed"`. Without it, the part turns freely (section 11). |
| `sockets` | The part's own named sockets (section 6). |
| `note` | For people. Ignored. |

A part in `define` has no `name`, `attach`, `to`, `twist`, `move`,
`rotate`, `scale`, `mirror` or `limits`: those go on the part that uses
it. (The parts inside a definition can have them.)

The shapes. All are centred on their own middle.

| `shape` | Size fields (default) | `segments` | Face names | Triangles |
|---|---|---|---|---|
| `box` | `size = [x, y, z]` (`[1, 1, 1]`) | none | `top`, `bottom`, `left`, `right`, `front`, `back`; `sides` for the last four | 12 |
| `cylinder` | `radius` (0.5), `height` (1); upright | 3 to 64 (16) | `top`, `bottom`, `side` | 4 × segments − 4 |
| `sphere` | `radius` (0.5) | 4 to 64 (12) | `side` | segments × (segments − 2), for even counts |
| `cone` | `radius` (0.5), `height` (1); point up | 3 to 64 (16) | `side`, `bottom` | 2 × segments − 2 |
| `wedge` | `size = [x, y, z]` (`[1, 1, 1]`); a ramp, full height at the back, sloping down to the front edge | none | `slope`, `bottom`, `back`, `left`, `right` | 8 |
| `capsule` | `radius` (0.25), `height` (1, round ends included: at least twice the radius); a pill, upright | 3 to 64 (16) | `side`, `top`, `bottom` | 224 for 16 segments |
| `torus` | `radius` (0.5, to the middle of the ring), `tube` (0.15, how thick: less than `radius`); a ring lying flat | 3 to 64 (16) | `side` | 256 for 16 segments |
| `tube` | `radius` (0.5, outside), `wall` (0.1, less than `radius`), `height` (1); a pipe, open at both ends | 3 to 64 (16) | `side`, `inside`, `top`, `bottom` | 8 × segments |
| `pyramid` | `size = [x, y, z]`; four slopes to a point | none | `front`, `back`, `left`, `right`, `bottom`; `sides` for the slopes | 6 |
| `prism` | `size = [x, y, z]`; a roof: slopes left and right, ridge from front to back | none | `left`, `right`, `front`, `back`, `bottom` | 8 |
| `stairs` | `size = [x, y, z]`, `steps` (4, 1 to 64); up from the front edge to full height at the back | none | `top`, `front`, `back`, `left`, `right`, `bottom` | 12 × steps |
| `arch` | `size = [x, y, z]`, `wall` (0.2: beside and above the doorway; less than half the width and less than the height); a wall with a round-topped doorway from front to back | 2 to 64 (8): pieces of the round top | `front`, `back`, `left`, `right`, `top`, `bottom`, `inside` | 4 × segments + 24 |

Two options:

- **`bevel = 0.05`** on a `box` or a `cylinder` cuts its edges off by
  that many metres (less than half its smallest side). `bevel_steps`
  (1 to 4, default 1) makes the cut rounder. The new faces are called
  `bevel`, for `colors`. A bevelled box is 44 triangles with one step.
- **`taper = 0.5`** on a `box`, a `cylinder` or a `tube` makes the top
  that share of the bottom's width (0.01 to 10); `taper = [0.5, 1]`
  narrows x and z differently.

Use them sparingly: a bevel on every box multiplies the triangles.

**Shaping a box: `changes`.** A box (with or without `taper`, without
`bevel`) can have a list of changes, made to it one after the other:

```toml
[[parts]]
name = "wall"
shape = "box"
size = [3, 2.4, 0.2]
color = "plaster"
changes = [
  { inset = "front", by = 0.6, name = "window" },   # a face 0.6 m in from every edge of the front
  { inset = "window", by = 0.06, name = "glass" },  # the window's frame: 6 cm
  { pull = "glass", by = -0.08 },                   # the glass pushed 8 cm in
]
colors = { window = "wood", glass = "sky" }
```

| Change | What it does | New faces |
|---|---|---|
| `{ pull = "top", by = 0.5 }` | Pulls the face out the way it faces, by `by` metres, with new walls round it. A negative `by` pushes it in (a hollow). Several faces, `pull = ["a", "b"]`, that touch are pulled as one piece. | The face keeps its name. Walls `<face>_side_<side>` (`top_side_front`...); with `name = "leg"`: `leg_front`..., and `leg` for all of them. |
| `{ inset = "front", by = 0.1 }` | A smaller face inside the face, `by` metres in from every edge (less than half its width). The shape doesn't change: there are more faces to work with. | The middle: `<face>_inset`, or `name`. The ring round it: `<middle>_rim_<side>`; the face's old name now stands for the ring. |
| `{ move = "top", by = [0, 0, 0.2] }` | Moves the face by x, y, z metres; the faces round it stretch to follow. | None. |
| `{ turn = "top", by = [0, 15, 0] }` | Turns the face about its middle: degrees around x, then y, then z. Faces round it bend (each is drawn as two triangles). | None. |
| `{ scale = "top", by = 0.5 }` | Makes the face bigger or smaller about its middle: one number, or `[x, y, z]`. `scale = "top", by = 0.5` on a box makes a frustum. | None. |
| `{ move = "top+front", by = [0, 0.1, 0] }` | Moves an edge: where the faces `top` and `front` meet (in either order). | None. |
| `{ move = "top+front+left", by = [0, 0.1, 0] }` | Moves a corner: where those three faces meet. A face that bends is drawn as two triangles. | None. |
| `{ cut = "right", at = 0.8 }` | Cuts all the way round, with a flat cut facing right, 0.8 of the way from the middle to the right side of the box (0 to just under 1). Every face it crosses becomes two; nothing moves. | The piece towards the middle keeps the name; the slice towards the side is `<face>_<side>` (`top_right`), or `<face>_<name>`. |

- **Names are words, never numbers**, and never change when the size or
  an amount does. `duck-mold inspect <recipe> --faces` lists every face
  of every shaped part: its name, which way it faces, its size and the
  faces next to it, and the names that stand for several faces.
- A name can stand for several faces, and works wherever a face name
  does: in `colors`, and in later changes. A name for fewer faces beats
  one for more in `colors` (`front` beats `sides`).
- A name a change would make that an earlier change made already gets a
  number: `top_side_front_2`. A `name` you give is used as it is.
- **Amounts are metres**, in the part's own directions, before its
  `scale`, `mirror`, `rotate` and `move`.
- **A change that would break the shape is an error naming it**: a face
  that isn't there (`unknown_face`), an inset too wide (`out_of_range`,
  with the widest that fits), or a shape squeezed flat, turned inside out
  or passing through itself (`bad_shape`). Nothing is built.
- **New faces work like old ones**: `colors = { glass = "sky" }`, and
  sockets on them: `lamp = { on = "glass" }` sits at the middle of the
  glass, facing out of it, and moves with it (`shift` slides it across
  the face, -1 to 1 each way). `add-socket <part> <name> --on glass`
  writes one.
- Only a box can be shaped in this version; a part that `use`s a
  definition is shaped in the definition.

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
middle of its slope (the same place as `slope`). A **pyramid**'s `top`
is its point and a **prism**'s the middle of its ridge. A **torus**'s
and a **tube**'s `top` and `bottom` are in the middle of the opening,
level with the rim. An **arch**'s `bottom` is on the ground between its
feet, and its `front` and `back` are in the mouth of the doorway. On a
**bevelled** box, a socket at the very edge of a face is on the cut and
leans with it, as one on a **tapered** side leans with the side: add
`facing = "front"` (or whichever) to make it look straight out. On a box, and on a
cylinder's or sphere's sides, they are where the box is. A part made of parts, and a part made from a
file, follow the same rule, through every face inside them; only where
nothing lies straight behind the box side (a gap between two posts)
does the socket stay on the box. **The whole model's own sockets stay
on its box** (so do a recipe file's, used as a part): they say how big
it is and where the ground is.

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

**Sockets go into the `.glb`**, so a game can put things where the
recipe says: a hat on `head.top`, a tool in `left_arm.grip`, a part
inside an assembly as `left_arm/hand.bottom`, the model's own as `top`.
Only named parts' sockets are written: name every part a game will
need. `inspect --sockets` lists them all, with where each is and the
way it faces.

**How a joined part is turned.** It is turned as little as possible to
make its socket face the other one. A part whose `bottom` joins a `top`
is not turned at all. A part whose `top` joins a `right` socket lies
down, pointing right, its front still in front. When both sockets face
the same way, the part is turned right round the upright axis (it stays
the right way up); if they face up or down, it rolls over sideways (its
front stays in front).

**`twist` on a joined part** turns it around the join, clockwise seen
from the side the other socket faces. On a socket that faces up,
`twist = 90` is the same turn as `rotate = [0, 90, 0]` on a free part:
forward turns to the right. A joined part has no `rotate`, and a free
part no `twist`; writing one where the other belongs is an error that
shows the right one.

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
3. **`floating`.** Each entry is a part up in the air that touches
   nothing: `{ "part": "knob", "nearest": "peg", "gap": 0.1 }`. Almost
   always a mistake: attach it to a socket, or move it by `gap`. A part
   standing on the ground (its lowest point within a millimetre of the
   model's), and everything touching it, never floats: two things side
   by side on a floor are fine.
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
   for a part made of parts, `mesh` for a mesh file); `uses` names its
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
| `unsupported` | Not available: see-through colours, compressed glTF. |
| `file_not_found` | A file the recipe names can't be read. Paths start from the recipe's folder. |
| `bad_mesh` | A mistake inside a mesh file (an `.obj`'s with its line). |
| `bad_shape` | A change in a part's `changes` would squeeze a face flat, turn the shape inside out, or make it pass through itself. Use a smaller `by`, or change other faces. |
| `io` | The file on the command line can't be read or written. |

Warnings (the model is still built; exit code 0):

| `code` | Meaning |
|---|---|
| `no_color` | A part has faces with no colour: they are plain grey. Give it a `color`. |
| `unused` | A palette colour or a defined part that nothing uses, or a `color` that changes nothing because every face inside already has one. |
| `floating` | A part touches nothing. |
| `limit_broken` | A pose turns a part past its `limits`, or moves a part that is `"fixed"`. The model still builds. |
| `loop_jumps` | An animation loops and doesn't end on the pose it starts with. The model still builds. |
| `unknown_pose` | A key, or a command, names a pose the recipe doesn't have. `rest` is always one. |
| `unknown_animation` | A command names an animation the recipe doesn't have. |
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

**Use a mesh made elsewhere.** `file = "rock.obj"`, `file = "lamp.gltf"`
or `file = "lamp.glb"`. Run `duck-mold inspect lamp.gltf` first: it lists
the file's size and its material names, which are the face names for
`colors`. Use `scale` if it was made in other units (`scale = 0.01` for
centimetres). A glTF material you give no colour keeps the file's own
base colour, added to the palette under the material's name, with a
`no_color` warning that lists them: give the part a `color`, or `colors`
by material name, to choose your own. A `.glb` Duck Mold wrote brings its
palette, so it looks as it did (`examples/desk.toml`). Only the mesh is
read: animations, bones and textures are left out with a note, and a
compressed file is refused.

**Choose where the model's zero point is.** `origin = "pole.bottom"`
puts it at a socket of one part instead of under the middle of the
whole model.

## 10. Changing a recipe with `edit`

You can always change the recipe's text yourself. `duck-mold edit` makes
the common changes for you, safely: it changes only the characters it
has to (comments, order and layout stay), builds the result before it
writes, and prints the same report as `inspect`, so one command is the
edit and the check. These are the edits a person makes with the mouse in
Duck Mold Studio, one for one.

```bash
duck-mold edit robot.toml add hat --shape cone --to head.top --color red --json
```

| Command | What it does |
|---|---|
| `duck-mold edit <recipe> add <name> --to <part.socket>` | Adds a part joined to a socket. Made of `--shape box` (or any other shape's name), `--use <defined part>` or `--file <recipe or mesh>`. Also takes `--attach <own socket>`, the shape's sizes (`--size x,y,z`, `--radius`, `--tube`, `--wall`, `--height`, `--segments`, `--steps`), its options (`--bevel`, `--bevel-steps`, `--taper n` or `--taper x,z`), `--color`, `--twist`. |
| `duck-mold edit <recipe> add <name>` (no `--to`) | Adds a part joined to nothing, in the model's own list. It is stood on the ground, clear of the other parts: from the middle of the model towards its front (+z), 10 cm at a time, until 10 cm separates it from everything else; or exactly at `--stand x,z`. Or give its middle exactly with `--move x,y,z`. Also takes `--rotate x,y,z` and `--scale n` (or `x,y,z`), and what it is made of, its sizes and `--color` as above. Move it later with `set <name> --move`. |
| `duck-mold edit <recipe> remove <part>` | Removes a part. Refused, with their names, if other parts hang from it; `--with-attached` removes those too. |
| `duck-mold edit <recipe> rename <part> <new-name>` | Renames a part, and every `to`, `from` and `origin` that names it. |
| `duck-mold edit <recipe> join <part> --to <part.socket>` | Moves a part's join to another socket. `--attach` changes its own socket. A part placed with `move` loses it. |
| `duck-mold edit <recipe> set <part> ...` | Changes a part: `--shape`, `--size x,y,z`, `--radius`, `--tube`, `--wall`, `--height`, `--segments`, `--steps`, `--bevel` (`0` makes the edges sharp again), `--bevel-steps`, `--taper n` or `--taper x,z` (`1` makes the sides straight again), `--color`, `--face-color top=plank` (repeatable; `top=none` takes the face's own colour away again), `--twist`. Several at once are one edit. A new shape drops the sizes, options and face colours it doesn't have. |
| `duck-mold edit <recipe> set <part> --move x,y,z` | Puts a **free** part (one that isn't joined) there: its middle, in metres, measured from the recipe's own zero. Also `--rotate x,y,z` (degrees around x, then y, then z) and `--scale n` or `--scale x,y,z`. A joined part refuses `--move` and `--rotate`, and says what to do instead; `--scale` works on any part. |
| `duck-mold edit <recipe> unjoin <part>` | Makes a joined part free, exactly where it stands: its `attach`, `to` and `twist` become a `move` and a `rotate`. The model doesn't change; the part can then be moved anywhere. |
| `duck-mold edit <recipe> duplicate <part>` | Copies a part, comments and all, straight after it. `--name` names the copy (otherwise `hat_2`); `--to` joins it elsewhere. Parts hanging from it are not copied. A copy of a joined part, with no `--to`, never sits on the part it copies: it goes to the next socket of the same part that nothing is joined to and that faces the same way (in the order they are written: the next table leg's socket); else to a new socket (`spot`) made beside the original's, 10 cm past it on the same side, if there is room; else (in the model's own list) it stands free on clear ground in front of the model. The summary says which. |
| `duck-mold edit <recipe> duplicate <part> --mirror` | A mirror image: a right arm from a left arm. The copy gets `mirror = "x"`, its twist is reversed, and `left` and `right` are swapped in its name and in the socket it joins (`body.left_arm` becomes `body.right_arm`), unless `--name` and `--to` say otherwise. |
| `duck-mold edit <recipe> add-color <name> <#RRGGBB>` | Adds a colour to the recipe's palette. |
| `duck-mold edit <recipe> set-color <name> <#RRGGBB>` | Changes a palette colour. Everything painted with it changes. |
| `duck-mold edit <recipe> remove-color <name>` | Takes a colour out of the palette. Refused if anything is painted with it, naming what; `--replace-with <other colour>` paints those with the other colour first. |
| `duck-mold edit <recipe> add-socket <part> <name> --on <side>` | Declares a socket on a side of a part (`top`, `bottom`, `left`, `right`, `front`, `back`), or on any face of a shaped box by its name (`--on glass`), slid across it by `--shift x,y,z` (each -1 to 1). It sits on the surface there (section 6). This is how to place a part anywhere on another: add a socket, then `add` or `join` to it. |
| `duck-mold edit <recipe> set-socket <part.socket>` | Moves a socket declared that way: `--shift x,y,z`, `--on <side>`. Whatever is joined to it moves with it. |
| `duck-mold edit <recipe> set-socket <part.socket> --facing <way>` | Turns a socket declared that way: `up`, `down`, `left`, `right`, `front`, `back` writes `facing`; `out` takes it away, so the socket faces straight out of its side again. Whatever is joined to it turns to match. |
| `duck-mold edit <recipe> rename-socket <part.socket> <new name>` | Renames a socket a part declares, and everything that names it: the `to` of each part joined to it, the part's own `attach`, and a socket that passes it on with `from`. |
| `duck-mold edit <recipe> remove-socket <part.socket>` | Removes a declared socket. Refused if anything uses it, naming what. |
| `duck-mold edit <recipe> tidy-sockets` | Removes the sockets the Studio made (`spot`, `spot_2`...) that nothing uses any more. The Studio does this itself when it leaves one unused. |
| `duck-mold edit <recipe> set-limits <part> --x <low,high> --y ... --z ...` | Says how far a part may turn when the model is posed (section 11). Each of `--x`, `--y`, `--z` takes two angles (`-30,40`), `free` (any amount) or `none` (not that way); directions not given stay as they are. `--fixed`: it never turns or moves. `--none`: no limits again. |
| `duck-mold edit <recipe> set-pose <pose> <part> --turn x,y,z` | Turns a part in a pose, in degrees about its pivot (section 11); `--move x,y,z` slides it, in metres. Makes the pose if it isn't there. `--rest` puts the part back at rest in that pose. |
| `duck-mold edit <recipe> add-pose <name>` | A new pose with every part at rest; `--from <pose>` makes it a copy of another. |
| `duck-mold edit <recipe> rename-pose <pose> <new name>` | Renames a pose where it stands. |
| `duck-mold edit <recipe> remove-pose <pose>` | Removes a pose. Refused while a key of an animation uses it; `--with-keys` takes those keys away too. |
| `duck-mold edit <recipe> mirror-pose <pose> --as <name>` | The mirror image of a pose, left to right: `left` and `right` swap in the parts' names, turns around y and z go the other way (inside a mirrored part they keep their numbers), a move to one side goes to the other. `--as` an existing pose replaces it; without `--as` the pose itself is mirrored. Make `step_left`, then `mirror-pose step_left --as step_right`. |
| `duck-mold edit <recipe> add-animation <name>` | A new animation with no keys yet (section 11). `--loop`: it starts again after its last key. |
| `duck-mold edit <recipe> add-key <animation> <pose> --at <seconds>` | The animation reaches that pose at that time. The key goes where its time puts it; the first key is at 0. The pose is one of the recipe's, or `rest`. |
| `duck-mold edit <recipe> move-key <animation> <number> --at <seconds>` | Gives a key another time. Keys are numbered from 1, as `inspect --animation` prints them. Key 1 stays at 0. |
| `duck-mold edit <recipe> set-key <animation> <number> --pose <pose>` | Gives a key another pose. |
| `duck-mold edit <recipe> remove-key <animation> <number>` | Removes a key. If it is the first, the keys after it move earlier so the animation still starts at 0. |
| `duck-mold edit <recipe> set-animation <animation> --loop true\|false --smooth true\|false --length <seconds>` | Any of the three. `--length` scales every key's time: the whole animation gets faster or slower. |
| `duck-mold edit <recipe> rename-animation <animation> <new name>` | Renames an animation where it stands. |
| `duck-mold edit <recipe> remove-animation <animation>` | Removes an animation. Its poses stay. |
| `duck-mold edit <recipe> pull <part> <faces> --by <metres>` | Adds a pull to a box's `changes` (section 5): the faces (one, or several with commas: `top_left,top`) pulled out by `--by` metres, with walls round them; below 0 pushes them in. `--name` names the walls. |
| `duck-mold edit <recipe> inset <part> <faces> --by <metres>` | Adds an inset to a box's `changes`: a smaller face inside each face, `--by` metres in from its edges. `--name` names the face in the middle. |
| `duck-mold edit <recipe> set-change <part> <number> --by <amount>` | Changes the amount of one change, counted from 1 as they are written: one number, or `x,y,z` for a move, a turn or a scale. Later changes, and every name, stay. |
| `duck-mold edit <recipe> move-face <part> <faces> --by x,y,z` | Adds a move of faces to a box's `changes`. Right after a move of the same faces, it adds to that one instead of making a second line. |
| `duck-mold edit <recipe> turn-face <part> <faces> --by x,y,z` | Adds a turn of faces (degrees around x, y, z, about their middle). Adds to a turn of the same faces right before it. |
| `duck-mold edit <recipe> scale-face <part> <faces> --by n` | Adds a scale of faces (one number, or `x,y,z`). Multiplies a scale of the same faces right before it. |
| `duck-mold edit <recipe> move-edge <part> <a+b> --by x,y,z` | Adds a move of an edge (where faces `a` and `b` meet; several with commas). Adds to a move of the same edges right before it. |
| `duck-mold edit <recipe> define <part> <name>` | Makes a definition from a part of the model's own list: its shape and sizes (or file), changes, colours and own sockets move to `[define.<name>]`, and the part becomes `use = "<name>"` where it stands. Add more with `add <new> --use <name>`; shaping one shapes them all. |
| `duck-mold edit <recipe> move-corner <part> <a+b+c> --by x,y,z` | Adds a move of a corner (where faces `a`, `b` and `c` meet; several with commas). Adds to a move of the same corners right before it. A rock: cut a box through the middle three ways (`cut` right, top and front, `--at 0`), then move its corners in and out. |
| `duck-mold edit <recipe> cut <part> --toward <side> --at <share>` | Adds a cut all the way round, facing a side, `--at` of the way from the middle (0) to that side (just under 1). `--name` names the slices. `set-change --at` moves it. |
| `duck-mold edit <recipe> remove-change <part> <number>` | Takes one change away (`--all`: every one, and the part is a plain box again). Refused if a later change or a colour names a face only that change made: the mistakes it would cause are listed. |

- **A part is named as `inspect` prints it**: `head`, `left_arm/hand`,
  or `#2` for a part with no name. `--to` takes the same
  (`left_arm/hand.bottom`).
- **`--json`** prints `inspect`'s report for the recipe after the edit,
  plus `"edit"`: `{ "command": "add", "summary": "added \"hat\" (cone)
  on head.top", "file": "robot.toml", "written": true, "lines": [198,
  206] }` (`lines`: the first and last line that changed).
- **`--dry-run`** does everything but write: read the report, then
  decide.
- **An edit that would leave the recipe with more mistakes than it had
  is refused**: exit code 1, the mistakes listed, nothing written. An
  edit that can't be carried out (no such part, a name already taken) is
  exit code 2, with one line saying why and often the fix.
- **A part inside a defined part** (`left_arm/hand`, where `left_arm` is
  `use = "arm"`) is changed in the definition, so every part using it
  changes. The report says so: `"definition": "arm", "also_changes":
  ["right_arm/hand"]`.
- **`duck-mold save-part <recipe> <part> <file>`** (not an `edit`: the
  recipe doesn't change) writes the part alone as a recipe of its own,
  with the palette colours it uses and none of where it stands: put it
  in the `parts` folder next to a recipe and it is in the Studio's
  library. It never writes over a file.
- **A part made from a defined part** (`left_eye`, which is `use =
  "eye"`): `set --shape`, `--size`, `--radius`, `--height`,
  `--segments`, `--color` and `--face-color` change the definition,
  because that is where they are written, so every part using it
  changes, and the report names them the same way. Its `--twist`, its
  join and its name are its own.
- **A part inside another recipe file** is refused: edit that file.
- If the file was saved by someone else while the edit was being worked
  out, the edit is worked out again on the new text, so neither change
  is lost.
- When `--attach` isn't given, a new part joins by the side opposite the
  socket (`bottom` on a `top`), otherwise by its `bottom`.
- In the Studio a person can select several parts and move, turn,
  scale, colour, copy or remove them together. In the file that is
  the same edit, once for each part: run the command once for each.
  Hiding a part in the Studio changes nothing in the file.
- In the Studio, Face mode (Tab) shapes a box by hand: a drag of its
  Pull arrow is a `pull`, of its Inset knob an `inset`, and the panel's
  Changes list is `set-change` and `remove-change`. Read the result
  with `inspect --faces`.
- In the Studio, Pose mode (O) turns parts by their rings: a drag is
  one `set-pose`, and the poses list's New, Copy, Mirror, Rename and
  Remove are `add-pose`, `add-pose --from`, `mirror-pose`,
  `rename-pose` and `remove-pose`. The Limits in the panel are
  `set-limits`.
- **`pull`, `inset`, `set-change` and `remove-change` on a part made
  from a defined part** change the definition, like `set --shape`.
- **`set --shape` to anything but a box drops a box's changes** (only a
  box can be shaped), and says so: `dropped 3 changes`.
- Not edits: `mirror` (except through `duplicate --mirror`), sockets
  written with `at` or `from`, and `define`. Type those in the recipe.

## 11. Models that move

A model moves by its joins. There are no bones to place: **every part
turns about its pivot** (the socket it is joined to, or its own middle
if it isn't joined), and turning a part carries everything that hangs
from it. A part made of parts turns as a whole (`left_arm`), and the
parts inside it turn too (`left_arm/elbow`). Parts turn rigidly; nothing
bends.

`duck-mold inspect <recipe> --joins` prints the tree: every part, what
it turns about, what it hangs from, and how far it may turn.

```text
  joins        23: what each part turns about, and how far
    body                        turns about its middle   free
      panel                     on body.chest            fixed
      head                      on body.neck             x -30..40, y -80..80
        antenna                 on head.antenna          free
      left_arm                  on body.left_arm         free
        left_arm/upper          turns about its middle   free
          left_arm/elbow        on upper.bottom          x 0..140
```

With `--json` each entry of `joins` has `part`, `parent` (the part it
hangs from; `null` for the model itself), `joined_to`, `pivot` and
`limits`.

**A turn is `[x, y, z]` degrees**, around x, then y, then z, like
`rotate`:

| | Around | On a character facing forward |
|---|---|---|
| x | left to right | A nod. Positive swings a hanging arm or leg forward, and tips a head back. |
| y | up | A turn. Positive turns a head to its right. A door swings around y. |
| z | forward | A tilt. Positive leans the top to its left; it lifts the right arm out sideways (the left arm lifts with a negative z). |

The x, y and z are those of the list the part is written in, as it
stands at rest: the model's own for a part of the model; a definition's
own for a part inside it, wherever that definition is used. So a
mirrored arm bends as a mirror image with the same numbers.

**`limits`** says how far a part may turn. It is written on the part,
with its join:

```toml
limits = { x = [-30, 40], y = [-80, 80] }   # a neck: nods and turns, doesn't tilt
limits = { x = [0, 140] }                   # an elbow or a knee: one way only
limits = { y = [0, 110] }                   # a door on its hinge
limits = { x = "free" }                     # a wheel: round and round
limits = "fixed"                            # never moves: a badge, a window
```

- No `limits`: the part turns any way, any amount.
- With `limits`: only the directions named turn, each between its two
  angles (the first 0 or less, the second 0 or more) or `"free"`.
- `"fixed"`: it neither turns nor moves in any pose. `limits = {}`: it
  doesn't turn, but a pose may still move it (a drawer).
- Limits on a part inside a definition go wherever the definition is
  used. `duplicate --mirror` mirrors a part's limits.

### Poses

A pose is the model with some parts turned or moved. It is written at
the top of the recipe, by name, with a line for every part that isn't
at rest:

```toml
[poses.wave]
left_arm = [0, 0, -150]            # its turn: degrees around x, then y, then z
"left_arm/elbow" = [0, 0, -30]     # a part inside a part: its path, in quotes
head = [0, -15, 0]

[poses.crouch]
body = { move = [0, -0.15, 0] }    # moved, in metres; everything on it goes too
left_leg = { turn = [40, 0, 0] }   # the long way of writing a turn
```

- A part the pose doesn't name is at rest. `rest` is always a pose: the
  model as it is built. It can't be written or changed.
- A turn is about the part's pivot; what hangs from the part goes with
  it. So a walk turns `left_leg` and `right_leg`, not the feet.
- `move` is from where the part stands at rest, in the same directions
  as the turn. Move the root part (the robot's `body`) to move the
  whole model: a crouch, a jump.
- There is no reaching (IK): a foot isn't put somewhere, the hip and
  knee are turned. Keep feet on the ground by moving the body until
  they touch: `inspect --pose` says how far the lowest point is from
  the ground.
- A pose that names a part that isn't there is an error with the
  closest name. A pose that turns a part past its `limits` is a warning
  (`limit_broken`): the model still builds and exports.

**Check a pose** with `duck-mold inspect <recipe> --pose <name>`: the
usual report, measured on the model as the pose puts it, plus `moves`
(the parts the pose turns) and `limits` (what it breaks). A part that
floats or overlaps in the pose and doesn't at rest is marked "(not so
at rest)". In `--json` that is `pose`: `name`, `moves`,
`limits_broken`, `new_overlapping`, `new_floating`. With `--sockets`
the sockets are listed where the pose puts them.

**Look at a pose** with `duck-mold render <recipe> --pose <name>`: the
four views, written as `<recipe>.<pose>.png`.

**Make both sides alike**: pose one side, then `mirror-pose`.

```bash
duck-mold edit robot.toml set-pose step_left left_leg --turn 30,0,0
duck-mold edit robot.toml set-pose step_left right_leg --turn -30,0,0
duck-mold edit robot.toml set-pose step_left left_arm --turn -25,0,0
duck-mold edit robot.toml set-pose step_left right_arm --turn 25,0,0
duck-mold edit robot.toml mirror-pose step_left --as step_right
duck-mold inspect robot.toml --pose step_left --json
duck-mold render robot.toml --pose step_left
```

Renaming a part with `rename` renames its lines in every pose (and the
lines of the parts inside it); removing a part removes them, and the
summary says how many.

### Animations

An animation is a list of **keys**: a pose, and the time it is reached.
It is written at the top of the recipe, by name:

```toml
[animations.walk]
loop = true                          # starts again after its last key
keys = [
  { at = 0, pose = "step_left" },    # seconds from the start; the first key is at 0
  { at = 0.4, pose = "step_right" },
  { at = 0.8, pose = "step_left" },  # a loop ends on the pose it starts with
]

[animations.roll]                    # a cart's wheel
loop = true
smooth = false                       # straight from pose to pose, at a steady speed
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "wheel_round" },  # wheel = [360, 0, 0]: once round
]
```

- `at` is in seconds. The first key is at 0; each key is later than the
  one before. The animation lasts until its last key.
- `pose` is one of the recipe's poses, or `rest`. The same pose can be
  used by many keys and many animations.
- Between two keys every number of a part's turn and move goes from one
  pose's to the next's. So `[0, 0, 0]` to `[360, 0, 0]` is a full turn,
  and `[0, 0, 0]` to `[0, 0, 0]` is no motion.
- `smooth` (true unless written `false`): the motion flows through each
  pose without stopping, slows to a stop where a part turns back (the
  top of a wave), and starts and ends gently. It never goes past a
  pose on the way to it, so poses within a part's limits keep the whole
  animation within them. `smooth = false` suits wheels and clock hands.
- `loop` (false unless written `true`): a loop must end on the pose it
  starts with, or the model jumps when it starts again; that is the
  warning `loop_jumps`. (A wheel at 360 is where it was at 0: fine.)
- A walk doesn't move the model along the ground: the game does that.
  Animate in place.

**Check an animation** with `duck-mold inspect <recipe> --animation
<name>`. It looks at every frame, 30 a second, and reports the worst it
finds and when:

```text
robot, the animation "walk"  (robot.toml)
  length       0.8 s  (25 frames at 30 a second)
  plays        round and round (it loops)
  motion       smooth: it flows through each pose, and starts and ends gently
  keys         1  0 s    step_left
               2  0.4 s  step_right
               3  0.8 s  step_left
  moves        left_arm, right_arm, left_leg, right_leg
  ground       its lowest point sinks 0.04 into the ground at 0.1 s
  floating     none in any frame
  overlapping  left_leg/thigh is 0.045 inside body at 0 s   (not so at rest)
  limits       none broken in any frame
  loop         ends as it starts
```

With `--json`: `animation`, `length`, `loops`, `smooth`, `keys` (each
with `key`, `at`, `pose`), `frames`, `moves`, `ground`
(`stays_on_ground`, `lowest_y`, `lowest_at`, `highest_y`,
`highest_at`), `floating`, `overlapping` and `limits_broken` (each
with `at`, the time it is worst, and `new`: not so at rest), and for a
loop `ends_as_it_starts`. The plain `inspect --json` lists
`animations`: each `name`, `length`, `loops`, `keys`.

**Look at an animation** with `duck-mold render <recipe> --animation
<name>`: a strip of 8 frames side by side, evenly spaced in time, all
from the same place, each with its time in its corner, written as
`<recipe>.<animation>.strip.png`. `--frames <2 to 60>` for another
number; `--view front|right|back|three-quarter` for where they are
drawn from (a walk shows best from `right`, a wave from `front`). Read
the picture to see the motion; `--json` says where each frame is
(`strip`: `at`, `left`, `top`, `size`).

```bash
duck-mold edit robot.toml add-animation walk --loop
duck-mold edit robot.toml add-key walk step_left --at 0
duck-mold edit robot.toml add-key walk step_right --at 0.4
duck-mold edit robot.toml add-key walk step_left --at 0.8
duck-mold inspect robot.toml --animation walk --json
duck-mold render robot.toml --animation walk --view right
```

Renaming a pose renames it in every key that uses it. A pose that keys
use can't be removed from under them: change the keys, or
`remove-pose <pose> --with-keys`.

### In a game

`duck-mold export` writes the animations and the poses into the `.glb`
(as standard glTF animations, thirty samples a second, so other glTF
programs play them too). Nothing extra to ask for. In Duck Forge:

```rust
let mut robot = AnimatedModel::load("assets/robot.glb")?; // duckforge::glb::AnimatedModel
robot.play("idle");                                       // by the animation's name
if walking { robot.play("walk") } else { robot.play("idle") } // every frame is fine
robot.update(ctx.dt());
canvas.draw_mesh(robot.mesh(), &Transform::at(position), &camera);
```

`play_once`, `play_looping`, `stop`, `resume`, `rest`, `set_speed`,
`set_pose("wave")`, `turn("head", vec3(0.0, 30.0, 0.0))` and
`socket("head.top")` (where it is now) are there too. So the names you
give animations, poses and parts are the names game code uses: keep
them plain (`idle`, `walk`, `wave`, `jump`, `open`). `examples/robot.toml`
has limits, six poses and three animations to copy from.

## 12. Mistakes to avoid

- **Typing positions when a socket would do.** A part placed with
  `move` floats or overlaps as soon as a size changes. Attach it.
- **`attach` without `to`** (or the other way round): they go together.
- **`to` naming a part in another list**, or one with no `name`. Parts
  attach to parts in their own list.
- **`rotate` on an attached part.** A joined part turns around its join
  with one number: `twist = 90`. (Duck Mold's first version wrote that
  as `rotate = 90`; it is now an error that shows `twist`.)
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

## 13. What it can't do yet

Plan around these; don't look for them: no textures or UVs, no smooth
shading (every face is flat, as in Duck Forge), no see-through or
glowing colours, no booleans (cutting one shape out of another), no
bones, animation, colliders or LODs (`bones`, `colliders` and `lods` are
reserved names). There is a window, Duck Mold Studio, but it is for
people: everything it does, you do with `edit`.

## 14. Files for reference

- `examples/`: crate, table, tree, house, robot, desk, snowman, duck.
  Read `robot.toml` for nested parts, `snowman.toml` and `duck.toml` for
  sockets on curved surfaces, and `desk.toml` for parts from files.
- `docs/recipe-format.md`: every field, default and range.
- `docs/forge-handoff.md`: what is in the `.glb`, and how Duck Forge
  reads it.
- `DESIGN.md`: why each thing is the way it is.
