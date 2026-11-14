# Duck Mold: design for Phase 0

**Status: approved and built.** This document says what Phase 0 is and
why, in plain words. It was written before any code, approved with five
decisions (section 16), and has been kept in step with what was built:
where the building changed a detail, the text here was changed to match,
and `PHASE0_REPORT.md` lists those changes. Section 17 is the design
for Phase 1, and section 18 the design for Phase 2 (the Studio).

Duck Mold turns a text file (a *recipe*) into a game-ready 3D model for
Duck Forge. It is an assembler, not a modelling program: you name parts,
say which socket joins which, pick colours from a palette, and it builds
the mesh. Phase 0 is the foundation: a library and a command-line program,
no window.

---

## 1. What I read first

The three sibling projects were not in folders next to this one, so I
made read-only copies from GitHub and read their READMEs, agent guides,
format documents and the relevant source. This is what I found and what
Duck Mold does with it.

| What the siblings do | Where | What Duck Mold does |
|---|---|---|
| A recipe is TOML in a file and JSON in a tool call: one set of fields, two ways to write it. `version = 0` is required. A misspelt field is an error, never ignored. | Duck Echo | The same. |
| A problem is data: a code, the field's path, the line, what was allowed, and a suggestion. With `--json`, exactly one JSON document is printed, failures included. | Duck Echo | The same shape, plus the file name. |
| Exit code 0 when it worked, 1 when the recipe is wrong, 2 for a wrong command line or a file that can't be read or written. | Duck Echo | The same. |
| A palette is a list of names with `"#RRGGBB"` colours. Things are defined once (`define`) and used many times (`use`). Any item may carry a `note` for people. Angles are degrees. Fields are spelt `color`; the documents say "colour". | Duck Quill | The same. |
| x is right, y is up, z is forward. 1 unit is about a metre. Built-in shapes are 1 unit big and centred. A transform scales, then rotates around x, then y, then z, then moves. | Duck Forge | The same, to the letter (section 3). |
| A mesh is triangles, **each with one flat colour**, lit evenly (flat shading). One texture per mesh at most. A triangle's corners go clockwise seen from the front. | Duck Forge | This decides how colour is exported (section 7). |
| Few dependencies, each one justified. No `unsafe`. Docs: a README for people, `docs/AI_GUIDE.md` for agents, decisions explained in plain words. | all three | The same. |

Two things I did not expect:

1. **Duck Forge cannot load model files yet.** Its guide says "no model
   files (build shapes in code, or merge the built-in ones)", and its
   Phase 3 design lists "3D model files (OBJ, glTF)" under *left out on
   purpose*. So there is no glTF import to check against. Section 8 says
   what was done instead: a `.glb` kept so simple that Duck Forge can
   read it with what it already has, and a tested reader to prove it.
2. **None of the three projects has a licence.** There is nothing to
   match. See decision 3 in section 16.

---

## 2. The idea in one picture

```
 robot.toml  (or robot.json)            the recipe: the source of truth
      │
      ▼
 read and check      every mistake at once, each with file, line, field, fix
      │
      ▼
 build the parts     boxes, cylinders, spheres, cones, wedges, imported meshes
      │
      ▼
 assemble            join sockets by name; nested assemblies become one tree
      │
      ▼
 colour              every face gets one colour from the palette
      │
      ├──►  duck-mold export   →  robot.glb    (for Duck Forge and any 3D viewer)
      └──►  duck-mold inspect  →  size, triangles, parts, floating, overlaps, ground
```

The mesh is never edited and never stored as a source. It is rebuilt
from the recipe every time, and the same recipe always gives the same
bytes (section 11).

Four words carry the whole design:

- **Part**: a thing in the model. A box, a cylinder, an imported mesh, or
  an assembly of other parts.
- **Socket**: a named point on a part, with a direction. Parts are joined
  socket to socket.
- **Palette**: the model's short list of named colours.
- **Recipe**: the file that lists the palette and the parts.

---

## 3. Directions, units and angles

These match Duck Forge exactly, so a number in a recipe means the same as
the number in game code.

- **x is right, y is up, z is forward.** 1 unit is about a metre.
- **A model faces forward (+z).** Its `front` is the +z side, `back` is
  -z, `left` is -x, `right` is +x, `top` is +y, `bottom` is -y. Left and
  right are the model's own left and right, and also yours when you stand
  behind it looking the way it faces.
- **Angles are degrees.** `rotate = [x, y, z]` turns around x, then y,
  then z, in Duck Forge's directions: a positive y angle turns forward
  towards the right. (Duck Forge's code uses radians; a recipe's
  `rotate = [0, 90, 0]` is `rotated(vec3(0.0, PI / 2.0, 0.0))` there.)
- **Order:** a part is mirrored, then scaled, then rotated, then moved.
  The last three are Duck Forge's `Transform` order.
- **The ground is y = 0.** By default a finished model stands on it
  (section 5, "Where the model's zero point is").

One note on the word "front". Duck Forge's cube code calls its -z face
"front" because that is the face a default camera sees. For a model with a
face and two arms, "front" has to be where it faces, or "left arm" would
mean two different things. I chose "a model faces forward", which is also
what glTF says. It is decision 1 in section 16, because every character
you ever make depends on it.

---

## 4. The recipe

A recipe is one file. TOML is the comfortable way to write it; JSON has
exactly the same fields and is handy for agents and tools. The file's
ending says which it is (`.toml` or `.json`).

### A first recipe

```toml
version = 0
name = "crate"

[palette]
wood  = "#B07A3F"
plank = "#8A5A2B"

[[parts]]
shape = "box"
size = [1, 1, 1]
color = "wood"
colors = { top = "plank", bottom = "plank" }
```

The same in JSON:

```json
{
  "version": 0,
  "name": "crate",
  "palette": { "wood": "#B07A3F", "plank": "#8A5A2B" },
  "parts": [
    { "shape": "box", "size": [1, 1, 1], "color": "wood",
      "colors": { "top": "plank", "bottom": "plank" } }
  ]
}
```

Both give the same `.glb`, byte for byte. A test holds that.

### A recipe with sockets

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
to = "trunk.top"        # ...joins this part's socket
```

No coordinates anywhere: the cone's `bottom` sits on the trunk's `top`.
Every part has those sockets built in.

### The top of the file

| Field | Needed? | What it is |
|---|---|---|
| `version` | yes | `0`. Any other number is refused with a clear message. |
| `name` | no | The model's name, for reports and inside the `.glb`. Default: the file's name without its ending. |
| `description` | no | A sentence for people and agents. |
| `note` | no | Anything for people. Ignored. (TOML also has `#` comments; JSON has none, so `note` is its comment.) |
| `palette` | no | The colours: a table of names, or the path of a palette file (section 6). |
| `define` | no | Parts defined once, to `use` many times. |
| `parts` | yes | The parts of the model, at least one. |
| `sockets` | no | Extra sockets on the whole model, for when another recipe uses this one as a part. |
| `origin` | no | Where the model's zero point is. Default `"bottom"`. |
| `bones`, `colliders`, `lods` | reserved | Not available yet (section 5, "Room for later"). Using one gives a clear "not yet" message, not "unknown field". |

### A part

Every entry of `parts`, and every entry of `define`, is a part. A part
gets its shape from **exactly one** of these four fields:

| Field | The part is... |
|---|---|
| `shape = "box"` | one of the five built-in shapes (table below) |
| `use = "leg"` | a copy of a part from `define` |
| `file = "wheel.toml"` | another recipe, or a mesh file (`.obj`; section 12). The path starts from the recipe's folder. |
| `parts = [ ... ]` | an assembly: a list of parts of its own (nesting) |

The other fields of a part:

| Field | What it is |
|---|---|
| `name` | What to call it: letters, digits, `_` and `-`. Needed if another part attaches to it. Names in one list must differ. An unnamed part is called by its place in the list, counting from 0 as error messages do: `#2` is `parts[2]`. |
| `color` | A palette name: the part's colour. |
| `colors` | Colours for single faces, by face name: `{ top = "grass" }`. |
| `attach`, `to` | Join my socket `attach` to the socket `to = "part.socket"` of another part in the same list. |
| `twist` | *(Phase 1.)* With `attach`: one number, the degrees to twist the part around the join. |
| `rotate` | Without `attach`: `[x, y, z]` degrees. (In Phase 0 it was also the twist; see below.) |
| `move` | Without `attach`: `[x, y, z]`, where the part's centre goes. |
| `scale` | One number, or `[x, y, z]`. Must be above zero. |
| `mirror` | `"x"`, `"y"` or `"z"`: flips the part across that direction, sockets too. `mirror = "x"` turns a left arm into a right arm. |
| `sockets` | The part's own named sockets (section 5). |
| `note` | For people. Ignored. |
| `bones`, `colliders`, `lods` | Reserved, as above. |

A part is placed in one of two ways. With `attach` and `to` it goes where
the sockets say; `move` is refused (the message says to move the socket),
and `twist` is one number that turns it around the join. Without them
it sits at the centre of its list, moved and turned by `move` and
`rotate = [x, y, z]`: the escape hatch for when you do want to type a
position. (The first draft had two fields, `turn` and `rotate`; they
became one name, `rotate`, by decision 5, with two forms. Phase 1 split
them again as `twist` and `rotate`, since one field meaning two things
surprised (section 17, step 4). Writing either on the wrong kind of part
is an error that shows the right one.)

### The five shapes

All are centred on their own middle, like Duck Forge's.

| `shape` | Size fields (default) | `segments` | Face names for `colors` |
|---|---|---|---|
| `box` | `size = [x, y, z]` (`[1, 1, 1]`) | none | `top`, `bottom`, `left`, `right`, `front`, `back`, and `sides` for the last four together |
| `cylinder` | `radius` (0.5), `height` (1). Stands upright. | 3 to 64 (16) | `top`, `bottom`, `side` |
| `sphere` | `radius` (0.5) | 4 to 64 (12) | `side` |
| `cone` | `radius` (0.5), `height` (1). Point at the top. | 3 to 64 (16) | `side`, `bottom` |
| `wedge` | `size = [x, y, z]` (`[1, 1, 1]`). A ramp: full height at the back, sloping down to the front edge. | none | `slope`, `bottom`, `back`, `left`, `right` |

`segments` is how many flat sides a round shape has: fewer is cheaper and
more angular. A box and a wedge are flat already, so they have none. Two
wedges back to back make a roof; a 4-sided cone is a pyramid.

Triangle counts are fixed and documented, so an agent can predict them: a
box is 12, a wedge 8, a cylinder `4 × segments - 4`, a cone
`2 × segments - 2`, a sphere `segments × (segments - 2)` for even counts.

### Mistakes

Every problem is reported at once, not only the first, each on one line:
the file, the line, the field, what is wrong, and a fix where one is
obvious.

```
error: robot.toml:12: define.arm.segments: 200 is too big. Allowed: 3 to 64.
error: robot.toml:31: parts[3].atach: there is no field "atach" here. Did you mean "attach"?
error: robot.toml:32: parts[3].to: the part "body" has no socket "nek". Did you mean "neck"?
error: robot.toml:40: parts[4].color: "#D0453A" is not a palette name. Add it to [palette] under a name (for example accent = "#D0453A") and write that name here.
warning: robot.toml:6: palette.rust: the colour "rust" is never used.
4 errors, 1 warning. Nothing was built.
```

They come file by file, from the top line down. When there is no
suggestion to make, the line ends with what is allowed instead
(`It can be: box, cylinder, sphere, cone, wedge.`).

Things that are errors: a syntax mistake, an unknown or misspelt field, a
missing or doubled field, a wrong kind of value, a number out of range, an
unknown shape, colour, part or socket, two parts with one name, a part
that uses itself (the message shows the chain: `arm > hand > arm`), both
`attach` and `move`, a file that isn't there. Things that are warnings (the
model is still built): a palette colour or a defined part never used, a
part with no colour, and what `inspect` finds (section 9).

---

## 5. Parts and sockets

### What a socket is

A socket is a point on a part with a direction it faces, like the stud on
a toy brick. Two sockets join **face to face**: the points meet and the
directions oppose, the way two magnets snap together.

**Every part has seven sockets built in**, on the box around it: `top`,
`bottom`, `left`, `right`, `front`, `back` (the middle of each side,
facing outwards) and `center`. For many models these are all you need.

In Phase 0 they were on the box, which is the surface for a box, a
cylinder and a sphere, but not everywhere on a sloped shape: a cone's
side sockets and a wedge's `top` and `front` were in the air beside the
slope. **From Phase 1 a shape's sockets know the shape** (section 17,
step 3): one that would be in the air moves straight in onto the
surface and faces out of it, and a wedge has an eighth, `slope`.

A part can declare more, in one of three ways:

```toml
[parts.sockets]
neck     = { on = "top" }                              # the middle of a side
left_leg = { on = "bottom", shift = [-0.5, 0, 0] }     # slid halfway towards the left edge
hook     = { at = [0.2, 0.9, 0], facing = "front" }    # an exact point, in the part's own units
grip     = { from = "hand.bottom" }                    # a socket of a part inside this assembly
```

- `on` names a side of the box around the part. `shift` slides the socket
  across that side: each number goes from -1 (the left, bottom or back
  edge) to 1 (the right, top or front edge), so sockets keep their place
  when the part changes size. The number for the direction the side
  itself faces stays 0.
- `at` is the escape hatch: an exact position. `facing` is `up`, `down`,
  `left`, `right`, `front`, `back`, or a direction `[x, y, z]`.
- `from` passes on a socket of an inner part, so an assembly can offer
  "the hand's grip" without the outside knowing what is inside.

### How joining works

`attach = "bottom"`, `to = "trunk.top"` means: move and turn me so that
my `bottom` meets the trunk's `top`, facing it.

- **The part is turned as little as possible** to make the two sockets
  face each other. A head whose `bottom` faces down, joined to a body
  whose `neck` faces up, is not turned at all. That keeps a part's front
  to the front whenever it can.
- If the part has to turn right round (both sockets face the same way),
  it turns around the upright axis, so it stays the right way up. If the
  sockets face up or down, it rolls over sideways instead, so its front
  still faces front. One fixed rule, so the result never depends on luck.
- **`twist`**, on a joined part, then turns it around the join by one
  number of degrees, clockwise when you look at the join from the side
  the other socket faces. For a socket that faces up, `twist = 90` is
  the same turn as `rotate = [0, 90, 0]` on a free part. (Phase 0 called
  it `rotate = 90`.)
- `mirror` and `scale` happen to the part first, sockets included, so a
  mirrored arm's shoulder socket is on its other side, ready to join.

Parts in one list form a tree: each attached part hangs from another. A
part that hangs from itself, directly or through others, is an error that
shows the chain.

### Nesting

An assembly is a part whose shape is a list of parts. Define it once and
use it like any other part:

```toml
[define.arm]                       # an assembly of two parts
[define.arm.sockets]
shoulder = { on = "right", shift = [0, 0.8, 0] }

[[define.arm.parts]]
name = "upper"
shape = "box"
size = [0.16, 0.6, 0.16]
color = "steel"

[[define.arm.parts]]
name = "hand"
shape = "sphere"
radius = 0.11
color = "red"
attach = "top"
to = "upper.bottom"

[[parts]]
name = "left_arm"
use = "arm"
attach = "shoulder"
to = "body.left_arm"

[[parts]]
name = "right_arm"
use = "arm"
mirror = "x"                       # the same arm, flipped
attach = "shoulder"
to = "body.right_arm"
```

Assemblies can hold assemblies, to any depth, and a whole recipe file can
be a part of another (`file = "wheel.toml"`), which is how a library of
parts grows. In reports, a part inside another is written with a slash:
`left_arm/hand`.

### Where the model's zero point is

A game places a model by its zero point. `origin` says where that is:

- `origin = "bottom"` (the default): the middle of the underside of the
  box around the whole model. The model stands on the ground, the way a
  Duck Quill sprite stands on its pivot.
- `origin = "trunk.bottom"`: a socket of one part. For a flag on a pole,
  that keeps the pole, not the flag, over the zero point.
- Any other built-in or declared socket of the model: `"center"`, `"top"`...

### Room for later: bones, colliders, LODs

You asked me to reserve room without building anything. Three things do
that:

1. **The names are taken.** `bones`, `colliders` and `lods` are reserved
   in every part and at the top of the file. Today they give a clear
   message (`"bones" is reserved for rigs and isn't available yet`), so no
   recipe can use them for something else, and no recipe silently carries
   data that does nothing. That is Duck Echo's rule: a field that exists,
   works.
2. **The assembly is kept, not flattened.** Inside the library the built
   model is a tree of placed parts, each with its name, its position and
   turn, its sockets and the triangles it owns. The triangle soup is made
   from that tree at the very end. A rig can later take a joint from
   every socket join, a collider can be fitted to each part's box, and a
   LOD can swap a part for a simpler one, all from data that is already
   there.
3. **The `.glb` remembers the parts.** It carries each part's name and
   which triangles are its own, and each assembly's name and the
   triangles of every part inside it (section 8).

---

## 6. Colour in a recipe

```toml
[palette]
steel = "#8A939B"
red   = "#D0453A"
```

- A colour is `"#RRGGBB"` or `"#RGB"`. See-through colours (`#RRGGBBAA`)
  are refused in this phase with a "not yet" message.
- **A part's `color` must be a palette name.** A hex code there is an
  error that tells you to add it to the palette. That is what keeps "one
  small palette" true. A palette holds at most 256 colours.
- `color` paints the whole part. `colors` paints single faces by name and
  wins over `color`: `colors = { top = "grass", sides = "dirt" }`.
- On an assembly, `color` is the colour for everything inside that has
  none of its own. So a defined arm can fix its red hand and leave its
  body to whoever uses it: `use = "arm"`, `color = "gold"`.
- A face left with no colour at all is built in a plain grey, with a
  warning, so you can block a model out first and colour it after.
- **A shared palette.** `palette = "../palette.toml"` reads the names from
  a file (a plain list of `name = "#RRGGBB"`), so every model in a game
  can share one. I read your "one small shared palette" as both: all the
  parts of a model share it, and models can share one file. Colours from a
  shared file are not "never used" warnings.

---

## 7. How colour is stored and exported

**Decision: vertex colours, not a palette texture.**

Inside Duck Mold, every triangle remembers *which palette entry* it has
(a number), not a red-green-blue value. The colour is looked up at the
very end. When the model is written to a `.glb`, each corner of each
triangle gets its colour written next to its position (in glTF this is
called `COLOR_0`, a "vertex colour"). The three corners of a triangle
always carry the same colour, so the triangle is one flat colour.

The other way would be a palette texture: a tiny picture with one pixel
per palette colour, and for every corner a pair of numbers (a "UV") that
points at a pixel. Why I chose vertex colours:

1. **It is what a Duck Forge mesh is.** A Duck Forge triangle is three
   corners and one colour. Vertex colours turn into that directly. A
   palette texture would need Duck Forge to decode a picture from inside
   the file and look every triangle's colour up in it, to arrive at the
   same thing.
2. **No UVs in this phase.** A palette texture needs a UV on every
   corner. UVs are on your "not in this phase" list, and filling that
   slot now would be in the way when real textures come.
3. **One file, nothing to lose.** No picture inside or beside the model.
4. **Exact.** The colours that come out are the palette's colours, to the
   last digit (a test checks every palette colour survives the trip).

What a palette texture would have given, honestly: recolouring a model in
the game by swapping one small picture (team colours), and slightly
smaller files. The first is still possible: the palette is stored in the
`.glb` by name, so a loader can map colours back to names and swap them.
If that turns out to matter, adding a per-corner palette number is a
small, compatible addition later.

One technical detail, so it is on record: glTF wants vertex colours as
*linear* light, not the usual `#RRGGBB` numbers (sRGB). Duck Mold
converts on the way out with a fixed table of 256 numbers, so every
computer writes the same bytes, and other viewers show the right
colours. A reader converts back with the same table and gets the exact
palette values.

---

## 8. The `.glb` file, and what Duck Forge needs

A `.glb` is the one-file form of glTF 2.0, the common 3D format. Duck
Mold writes a deliberately narrow kind of it:

| | |
|---|---|
| Layout | One scene, one object (named after the recipe), one mesh, one list of triangles. Every part is merged into it, as Duck Forge's guide recommends ("one draw instead of dozens"). (From Phase 5 the file is a tree with an object for every part, so that parts can move: section 21. Duck Forge still puts them together and draws one mesh.) |
| Positions | `POSITION`: three decimal numbers per corner (32-bit). |
| Normals | `NORMAL`: which way each face points, the same for all corners of a face, so every viewer shades it flat like Duck Forge does. |
| Colours | `COLOR_0`: three decimal numbers per corner, linear. |
| Triangles | A list of corner numbers, always 32-bit. Faces do not share corners with other faces. |
| Material | One plain, matt material. No textures, no UVs. |
| Extras | Under `extras.duck_mold`: the format version, the recipe's name, the palette (names and `#RRGGBB`), the list of parts with the triangles each one owns, and the list of assemblies (parts made of parts) with the triangles of every part inside each, and (from Phase 1) every named socket with where it is and the way it faces. Other programs ignore this; Duck tools read it. (Assemblies were added after Phase 0, when Duck Forge wanted to recolour a whole arm; sockets in Phase 1, so a game can put a hat on `head.top`; `docs/forge-handoff.md` describes all three lists.) |
| Not written | Cameras, lights, animations, skins, textures, a date, your folder names. |

**Directions in the file.** glTF also says y is up and z is forward, but
its "right" is -x where Duck Forge's is +x (one is a left-handed system,
the other right-handed). So on the way out Duck Mold negates x and
reverses each triangle's corner order. Nothing is distorted: the model
faces forward in both, and its left arm is its left arm in both. A reader
for Duck Forge does the same two steps in reverse.

**What this means for Duck Forge.** Duck Forge has no loader, so "loads
in Duck Forge" needs something on its side. The file above can be read
with what Duck Forge already depends on (`serde_json`, which it uses for
sprite sheets): the format is a 12-byte header, a block of JSON and a
block of plain numbers. No new library, so Duck Forge's test that guards
its dependency list stays green.

In Phase 0:

1. The `.glb` is written as above and checked against the glTF rules in
   tests (required fields, sizes, alignment, bounds). The official
   Khronos glTF validator was also run on the five examples by hand: no
   errors, warnings, infos or hints.
2. A **small, separate reader**, `docs/forge/duck_mold_glb.rs`, turns a
   Duck Mold `.glb` into exactly what a Duck Forge `Mesh` holds: a list
   of points, and triangles with clockwise corners and a colour each. It
   uses only the standard library and `serde_json`, shares no code with
   the exporter (so a mistake in one cannot hide a mistake in the
   other), and is tested against all five examples. It is written to be
   copied into Duck Forge as `Mesh::load_glb`.
3. `docs/forge-handoff.md` lists exactly what a loader must support, for
   the session that adds one to Duck Forge.

Putting that reader into Duck Forge itself is a follow-up session in the
Duck Forge repository (decision 2): this one can only write to Duck
Mold.

---

## 9. The commands

The program is `duck-mold`. Three commands, plus help; Phase 1 adds
`render` and `preview`.

| Command | What it does |
|---|---|
| `duck-mold inspect <file> [--json] [--sockets]` | Checks and builds a recipe in memory and reports on it. Also takes an `.obj` mesh, or a `.glb` that Duck Mold wrote, to measure it as it is. Writes nothing. *(Phase 1:)* `--sockets` adds every named socket, where it is and the way it faces: the names a game can ask the `.glb` for. |
| `duck-mold export <recipe> [-o <file.glb>] [--json]` | Builds the recipe and writes the `.glb` (default: next to the recipe, same name). Ends with a one-line summary; `--json` gives the full inspect report. |
| `duck-mold render <file> [-o <file.png>] [--size N] [--json]` | *(Phase 1.)* Draws the model and writes a PNG: four views on one sheet (section 17, step 2). Takes what `inspect` takes. |
| `duck-mold preview <file>` | *(Phase 1.)* Opens a window showing the model: drag to orbit, wheel to zoom, Escape to quit; builds it again when a file it reads is saved (section 17, step 7). |
| `duck-mold guide` | Prints the agent guide, which is built into the program, so an agent with only the program can learn it. |
| `duck-mold help`, `--help`, `--version` | Every command; `duck-mold <command> --help` for one. |

There is no separate `check` command: a recipe with mistakes makes
`inspect` (and `export`) print the mistakes and stop with exit code 1, so
`inspect` *is* the check. One command fewer to learn.

**Exit codes:** 0 it worked (warnings included), 1 the recipe has
errors, 2 the command line was wrong or a file couldn't be read or
written.

**Where output goes:** reports go to standard output; problems written
for people go to standard error. With `--json`, standard output holds
exactly one JSON document, also when it failed.

### What `inspect` reports

For a robot with a box body and head, two box arms with red undersides
and two cylinder legs:

```
robot  (examples/robot.toml)
  size         1.02 wide x 1.65 tall x 0.40 deep, in metres (x, y, z)
  triangles    168  (288 corners)
  ground       rests on the ground
  colours      4:  steel 42, dark 120, red 4, glow 2  (triangles each)
  parts        6
    body         box              12 triangles
    head         box              12 triangles   on body.neck
    left_arm     arm (box)        12 triangles   on body.left_arm
    right_arm    arm (box)        12 triangles   on body.right_arm, mirrored
    left_leg     leg (cylinder)   60 triangles   on body.left_leg
    right_leg    leg (cylinder)   60 triangles   on body.right_leg
  floating     none
  overlapping  none
ok: no problems found
```

What each line means, exactly:

- **size**: the box around every corner of the model.
- **triangles**: how many, and how many corners are stored.
- **ground**: `rests on the ground` when the lowest point is within a
  millimetre of y = 0; otherwise `floats 0.25 above the ground` or `sinks
  0.10 into the ground`.
- **colours**: each palette colour used, with its triangle count.
- **parts**: the tree of parts, with what each is, its triangles, and
  what it is joined to.
- **floating**: parts that touch nothing. Two parts *touch* when their
  surfaces come within a millimetre. Starting from the first part,
  everything reachable by touching is the body; a part outside it is
  floating, and the report says how far it is from the nearest part:
  `left_ear floats: 0.05 from head`.
- **overlapping**: pairs of parts where one pokes more than a millimetre
  into the other, with how deep: `leaves is 0.10 inside trunk`. Depth is
  measured at the corners and edge middles of each part, so it is a good
  estimate, not an exact volume. A part entirely inside another is called
  `hidden` and is a warning, since nobody will ever see it. Overlap is
  often meant (a nose pushed into a head), so it is reported as a fact,
  not a mistake.

Floating parts, hidden parts and a model that does not rest on the ground
are warnings. They never change the exit code: the report is the check,
and an agent reads the fields.

---

## 10. The JSON output

`--json` prints one document. For `inspect` on the same robot (two of
its six parts shown here, to keep the example short):

```json
{
  "ok": true,
  "summary": "robot: 1.02 x 1.65 x 0.40 m, 168 triangles, 6 parts, 4 colours, rests on the ground, nothing floating, nothing overlapping",
  "source": "examples/robot.toml",
  "name": "robot",
  "size": [1.02, 1.65, 0.4],
  "bounds": { "min": [-0.51, 0.0, -0.2], "max": [0.51, 1.65, 0.2] },
  "triangles": 168,
  "vertices": 288,
  "ground": { "rests_on_ground": true, "lowest_y": 0.0 },
  "palette": [
    { "name": "steel", "hex": "#8A939B", "triangles": 42 },
    { "name": "dark",  "hex": "#3B4148", "triangles": 120 },
    { "name": "red",   "hex": "#D0453A", "triangles": 4 },
    { "name": "glow",  "hex": "#FFE08A", "triangles": 2 }
  ],
  "parts": [
    { "name": "body", "kind": "box", "triangles": 12,
      "size": [0.7, 0.8, 0.4],
      "bounds": { "min": [-0.35, 0.5, -0.2], "max": [0.35, 1.3, 0.2] },
      "colors": ["steel"], "attached_to": null, "parts": [] },
    { "name": "left_arm", "kind": "box", "uses": "arm", "triangles": 12,
      "size": [0.16, 0.6, 0.16],
      "bounds": { "min": [-0.51, 0.63, -0.08], "max": [-0.35, 1.23, 0.08] },
      "colors": ["steel", "red"], "attached_to": "body.left_arm", "parts": [] }
  ],
  "floating": [],
  "overlapping": [],
  "warnings": [],
  "errors": []
}
```

- `summary` comes first and says the whole report in one line, as Duck
  Echo's `analyze` does.
- All lengths are metres; `size` and every point are `[x, y, z]`.
- `floating` entries: `{ "part": "left_ear", "nearest": "head", "gap": 0.05 }`.
- `overlapping` entries: `{ "a": "leaves", "b": "trunk", "depth": 0.1, "hidden": false }`.
- `export --json` prints the same report plus `"output": "robot.glb"`
  and `"bytes"`.
- A part's `kind` is what it is made of: `box`, `cylinder`, `sphere`,
  `cone`, `wedge`, `assembly` (a part made of parts) or `mesh`. `uses`
  names the defined part it came from and `file` the file, when there is
  one. `mirrored` appears when it is true.
- Every palette colour is listed, with 0 triangles if nothing uses it.
  `unpainted_triangles` appears when some faces have no colour.
- For an `.obj` or a Duck Mold `.glb` the report has the same fields. A
  `.glb` gives its tree of parts back (names, assemblies and triangles;
  not what each was made from or joined to); an `.obj` is one part, and
  the report adds `materials`, the file's material names with their
  triangle counts.

When the recipe has mistakes:

```json
{
  "ok": false,
  "errors": [
    {
      "severity": "error",
      "code": "unknown_field",
      "file": "robot.toml",
      "line": 31,
      "column": 1,
      "path": "parts[3].atach",
      "message": "there is no field \"atach\" here",
      "got": "atach",
      "allowed": { "options": ["name", "shape", "use", "file", "parts", "size", "radius", "height", "segments", "color", "colors", "attach", "to", "twist", "rotate", "move", "scale", "mirror", "sockets", "note"] },
      "suggestion": "attach"
    }
  ],
  "warnings": []
}
```

This is Duck Echo's issue format with `file` added. `code` is a short
fixed word an agent can act on: `syntax`, `unknown_field`,
`missing_field`, `type_mismatch`, `out_of_range`, `unsupported_version`,
`unknown_shape`, `unknown_color`, `unknown_part`, `unknown_socket`,
`unknown_face`, `duplicate_name`, `conflict`, `cycle`, `reserved`,
`unsupported`, `file_not_found`, `bad_mesh`, `io`; and for warnings
`unused`, `no_color`, `floating`, `hidden`, `off_ground`, `skipped`. The
first six are Duck Echo's own names. `allowed` is either `options` or a
`min` and `max`. `docs/AI_GUIDE.md`, section 8, says what to do about
each.

---

## 11. The same recipe, the same bytes

This is rule 1, and it needs care in a few places:

- **No luck.** No random numbers, no clock, no reading of the computer's
  settings. The file holds no date and no folder names.
- **Its own sine and cosine.** The maths that the operating system
  supplies can differ in the last digit between Windows, Linux and Mac.
  Duck Echo avoids it for sound; Duck Mold does the same for shapes, with
  a small sine and cosine of its own built from plain arithmetic. A
  quarter turn is exact: turning by 90 degrees gives exactly 0 and 1, not
  0.00000004.
- **Order is the file's order.** Parts, palette entries and definitions
  are handled in the order they are written, never in the order of a
  hash table.
- **Tidy numbers.** Positions are worked out with double precision,
  rounded to a thousandth of a millimetre, and then stored. That removes
  noise like `-0.0000001` where 0 was meant.
- **Tests that hold it.** Section 14.

---

## 12. Importing meshes (OBJ), last

A part can take its shape from a file, and is then a static part like any
other: it has the seven built-in sockets, can declare more, and can be
scaled, mirrored, attached and coloured.

```toml
[[parts]]
name = "rock"
file = "models/rock.obj"
scale = 0.01                          # this file was made in centimetres
color = "stone"
colors = { Moss = "leaf" }            # a material name in the file -> a palette colour
attach = "bottom"
to = "ground.top"
```

- **Colour** still comes from the palette. `color` paints it all; in
  `colors`, the "face names" are the file's material names. The file's
  own colours and textures are not used. `duck-mold inspect rock.obj`
  lists the material names, the size and the triangle count, so you know
  what to write.
- **OBJ:** points and faces (faces with more than three corners are cut
  into triangles), material names. Normals, texture coordinates and the
  material file are passed over; lines and curves are skipped with a
  note. A mistake in the file is reported with the mesh file's own name
  and line.
- **Directions:** a file is read as glTF means them (y up, facing +z,
  right-handed), which is also what Blender writes for OBJ by default.
  For a file made another way, `rotate` and `scale` on the part fix it.
- The recipe plus the file it names are the source of truth together. The
  same two always give the same output.

**glTF import moved to Phase 1** (decision 4). The first draft of this
section planned both formats and advised keeping OBJ and letting glTF go
first if time ran short. OBJ is small and well defined; glTF has many
ways to store the same numbers, and reading other programs' files is
where the time goes. What Phase 0 does read is its own `.glb`, for
`inspect`. (Phase 1 added glTF: section 17, step 6, and
`docs/recipe-format.md`, "Files as parts".)

---

## 13. How the code is laid out

```
Duck-Mold/
├── .github/workflows/ci.yml   the tests on Windows and Linux, for every push
├── Cargo.toml                 the workspace: both crates, shared settings
├── DESIGN.md                  this file
├── README.md                  for people
├── PHASE0_REPORT.md           what works, known limits, what comes next
├── PHASE1_REPORT.md           the same for Phase 1
├── docs/
│   ├── AI_GUIDE.md            for agents: the CLI from scratch (also `duck-mold guide`)
│   ├── recipe-format.md       every field, default and range
│   ├── forge-handoff.md       what a Duck Forge loader must support
│   └── forge/duck_mold_glb.rs the reader written for Duck Forge
├── examples/                  crate, table, tree, house, robot, snowman (.toml), and crate.json
├── tests/golden/              the examples' exact .glb files and reports
└── crates/
    ├── duck-mold-core/        the library: everything except the command line
    │   ├── src/
    │   │   ├── source.rs      TOML or JSON text -> one tree that remembers line numbers
    │   │   ├── issue.rs       problems: code, file, line, field, message, fix
    │   │   ├── recipe.rs      the recipe's fields, read from that tree and checked
    │   │   ├── load.rs        the files a recipe names: palettes, recipes, meshes
    │   │   ├── math.rs        points, turns, and the exact sine and cosine
    │   │   ├── shapes.rs      box, cylinder, sphere, cone, wedge
    │   │   ├── mesh.rs        reading .obj files
    │   │   ├── assemble.rs    parts, sockets, joins, nesting, colour -> a tree of placed parts
    │   │   ├── model.rs       the built model: the part tree and its coloured faces
    │   │   ├── glb.rs         writing (and reading back) .glb
    │   │   ├── inspect.rs     size, counts, ground, floating, overlapping
    │   │   └── tools.rs       what the commands do, for any front end
    │   └── tests/             one file of tests per step
    ├── duck-mold-view/        (Phase 1) pictures and the window, by Duck Forge
    │   ├── src/render.rs      `render`: turn the model for each view, draw, lay out the sheet
    │   ├── src/preview.rs     `preview`: a Duck Forge game that shows the model and watches its files
    │   └── tests/             each view sees what it says; same bytes; the window, without a window
    └── duck-mold-cli/         the `duck-mold` program: commands, text and JSON output
        └── tests/             the program itself, and the golden-file tests
```

Two crates, as you asked: the library does the work and knows nothing
about the command line, so a GUI or an MCP server can sit on the same
library later, as Duck Echo's does. Phase 1 adds a third,
`duck-mold-view`, so that the library also knows nothing of pictures or
of Duck Forge.

Rust edition 2024, like Duck Forge and Duck Quill. `unsafe` is forbidden
and every public item must have a comment, as in Duck Echo.

**Dependencies, and why each is there:**

| Crate | Used by | Why |
|---|---|---|
| `toml` | core | Reads TOML and tells me where every field is, for line numbers. |
| `serde`, `serde_json` | core | Writes JSON (reports, and the JSON block inside a `.glb`) and reads glTF. |
| `clap` | cli | Reads the command line and writes `--help` from the same comments, so help can't drift from what the program does. Duck Echo uses it too. |
| `duckforge` | view | *(Phase 1.)* Duck Forge itself, from GitHub, pinned to one commit on its `main`, with its graphics card and sound turned off: its CPU renderer draws `render`'s pictures and the preview, and its window (the `preview` feature, on by default) shows the preview. It brings `png` and `gif`, and with the window `winit` and `softbuffer`. |
| `png` | view | *(Phase 1.)* Writes the pictures. The same library and version Duck Forge uses. |

That is all. Things written here rather than imported, because each is
small and the wording or the bytes matter: the JSON reader for recipes
(so JSON recipes get line numbers too, which `serde_json` cannot give),
"did you mean" suggestions, the `.glb` writer and reader, and the OBJ
reader.

---

## 14. Tests and the quality bar

- **Every step has tests**, written with that step and committed with it.
- **Golden files.** Each of the five examples has its `.glb`, its
  `inspect --json` report and (from Phase 1) its `render` picture stored
  in `tests/golden/`. A test runs the
  real program, rebuilds them and compares every byte. If one changes,
  the test fails and shows what changed; a change that is meant is
  accepted with `DUCK_MOLD_ACCEPT=1 cargo test -p duck-mold-cli --test
  golden`, and the commit says why.
- **Same in, same out.** Building twice gives the same bytes; the TOML
  and JSON forms of one recipe give the same bytes.
- **Mistakes are tested too.** Each kind of error has a test for its
  file, line, field and suggestion.
- **The Duck Forge reader** (section 8) reads every golden `.glb` and
  must find the same triangles and exact palette colours.
- **The documents are tested.** The first recipe in the agent guide and
  in the README must build to exactly the report printed beside it, and
  every field the guide lists must be one the program knows.
- **Shapes face outwards.** The same test Duck Forge has for its own
  meshes, for every shape and every example.
- `cargo fmt` and `cargo clippy` with warnings as errors, clean at every
  commit.
- **On Windows and Linux.** From Phase 1, GitHub runs the formatting
  check, the build, every test and clippy on both, for every push and
  pull request (`.github/workflows/ci.yml`, copied from Duck Quill's).
- One commit per step, in your order: workspace, recipes, shapes, parts
  and sockets, colour, export, inspect, import; then the examples, the
  documents, and the report.

---

## 15. Left out on purpose

Besides your list (GUI, rendering, booleans, rigging, animation, UVs,
texture painting, sculpting), these are left out of Phase 0. Each is a
candidate for later, and none needs a change to what is designed here:

- **Importing glTF** (section 12).
- **Sockets in the `.glb`**, so a game can put a hat on a head at run
  time. A natural next step, as named empty points.
- **Mirror copies** (one line that makes both arms). In Phase 0 `mirror`
  flips a part; you write the second arm as a second part.
- **Swapping colours on a used part** by name, as Duck Quill's `use` can.
- **Smooth shading.** Duck Forge is flat-shaded, so Duck Mold is too.
- **See-through and glowing colours.**
- **A JSON Schema** command, as Duck Echo has. The guide and the error
  messages cover it for now.
- **A warning for two faces in the same plane** that would flicker.
- **An MCP server.** The library is shaped so one can be added.

---

## 16. The decisions

Before any code was written, this document ended with five questions.
These are the answers that were given, and what each led to.

1. **Which way does a model face?** *Forward, +z.* `front` is +z and
   `left` is the model's own left, -x (section 3). The comment in Duck
   Forge's cube that calls its -z face "front" disagrees in wording
   only; `PHASE0_REPORT.md` and `docs/forge-handoff.md` note it for the
   Duck Forge session to fix.
2. **The Duck Forge side.** *A follow-up session in the Duck Forge
   repository.* Phase 0 gives it a tested reader
   (`docs/forge/duck_mold_glb.rs`) and a short note of exactly what the
   loader must support (`docs/forge-handoff.md`).
3. **Licence.** *Left out for now*, like the three sibling projects; to
   be decided for the whole suite at once.
4. **Importing glTF.** *OBJ stays in Phase 0; glTF import moves to
   Phase 1* (section 12).
5. **The names.** *`turn` became `rotate`; the rest stayed.* One field,
   `rotate`, was a single number on an attached part and `[x, y, z]` on a
   free one (section 4). Phase 1 gave the single number its own name,
   `twist`.

---

## 17. Phase 1

**Status: approved, with three changes, and built** (`PHASE1_REPORT.md`
says what came of it; the *As built* notes below say where the building
changed a detail). The three changes are folded in below: Phase 1 is
built on a new branch from `main`, with Duck Forge pinned to a commit on
its `main`; an imported material with no colour in the recipe brings its
own (step 6); and unnamed parts' sockets stay out of the `.glb`
(step 5). Phase 1 adds seven things, built in this order, one commit
each. Every step comes with tests, keeps
fmt and clippy clean, regenerates the golden files if the output
changes, updates the README, `docs/AI_GUIDE.md` and
`docs/recipe-format.md`, and adds an example. When a step changes
something an earlier section describes, that section is changed to
match, as in Phase 0. Not in Phase 1: editing in the window, booleans,
bones, animation, UVs, textures.

### Step 1. Tests on Windows and Linux for every push

A copy of Duck Quill's `.github/workflows/ci.yml`. On every push and
every pull request, on Windows and on Linux, with the same pinned Rust
(1.97.0), it builds everything, runs every test, and runs clippy with
warnings counted as errors. Two additions: a formatting check
(`cargo fmt --check`), and, from step 7 on, a second build and test run
with the window left out, the way a machine with no screen builds it.

Anything Windows finds is fixed in this step. Line endings are already
handled: `.gitattributes` keeps text files' Unix endings on Windows, and
step 2 adds pictures to its "not text" list.

### Step 2. `duck-mold render`

```bash
duck-mold render robot.toml                # writes robot.png next to the recipe
duck-mold render robot.toml -o sheet.png --size 384
```

**One picture, four views**, two by two, each view a square (256 pixels
unless `--size` says otherwise):

| | |
|---|---|
| **front**: looking the model in the face | **right**: from the model's right, so it faces right in the picture |
| **back** | **three-quarter**: from the front right, 45° round and 30° above |

Each view has its name in a corner, and a line under them gives the
model's name, size and triangle count, as `inspect` says them. A line
one pixel wide runs between the views, so with 256-pixel views the
sheet is 513 × 536 pixels. `--size` takes 128 (the smallest at which
the names fit) to 2048. `render` takes what `inspect` takes, so it also
draws an `.obj` or a `.glb` as it is. All four
views use the same distance and zoom, so the model is the same size in
each. The camera is far away and zoomed in (a 20° view, where Duck Forge
games usually use 60°), so the views look nearly flat, like drawings,
and parts can be compared by eye. The light comes from above and over
your shoulder, the same in every view. The background is a dark
blue-grey, so the light grey of an uncoloured face shows.

**Reusing Duck Forge's CPU renderer.** It is clean to reuse: it draws
into a picture in memory, needs no window and no graphics card, uses no
threads, and already saves PNGs. Duck Mold gets it the ordinary Rust
way: a dependency on the Duck Forge repository on GitHub, pinned to one
exact commit, with Duck Forge's window, graphics card and sound turned
off. Nothing is copied. The costs: the first build downloads Duck Forge
(with two small libraries of its own, for PNG and GIF files), and moving
to a newer Duck Forge is a deliberate one-line change, after which the
test pictures are checked again.

So that Duck Mold's library stays free of any game engine, the drawing
goes in a new small crate, `duck-mold-view`, which the command line
uses. The library (`duck-mold-core`) doesn't change and still builds
without Duck Forge.

**The same recipe always gives the same picture**, byte for byte, like
the `.glb`:

- Duck Mold turns the model for each view itself, with its own sine and
  cosine (the ones that already make every computer write the same
  `.glb`), and leaves Duck Forge's camera looking straight ahead, so Duck
  Forge does no turning of its own.
- Everything else Duck Forge does to draw is plain arithmetic, which
  gives the same answer on every computer. There is one exception: it
  works out its zoom with the computer's own `tan`, which could differ in
  the last digit between Windows and Linux and move an edge by a pixel.
- So the pictures of the five examples are kept in the repository, and
  the tests compare them byte for byte, on Windows and Linux, on every
  push. If the two ever disagree, the fix is a few lines in Duck Forge
  (a `tan` of its own, as Duck Mold has its own sine and cosine). I'd
  rather not change Duck Forge before that is shown to be needed.

`--json` prints where the picture went and what each view shows. The AI
guide tells agents to look at the picture to check a model.

### Step 3. Sockets that know the shape

**The rule: a built-in socket never floats.** A socket that is already
on the shape's surface doesn't move or turn. A socket that was in the
air beside the shape moves straight in until it touches the shape, and
turns to face straight out of the surface there.

That moves exactly these:

- **A cone's `left`, `right`, `front` and `back`**: from the box onto
  the slanted side, halfway up, tilted to face out of the slant (about
  27° above level on the default cone).
- **A wedge's `top` and `front`**: both onto the middle of the slope,
  facing out of it. The wedge also gets a new name for that place,
  **`slope`**, which says what it is.
- **Sockets made with `on` and `shift`** on a curved or sloped side:
  `shift` picks a point on the side of the box as before, then the
  socket moves straight in onto the surface. Eyes declared
  `on = "front", shift = [-0.4, 0.3, 0]` on a sphere now sit on the
  sphere and look out of it, instead of floating in front of it.

On a box, on a cylinder's top and bottom, on a wedge's back and
bottom, and wherever else the box and the shape already meet, nothing
changes. To stand something upright on a slope (a chimney on a roof),
give the socket a `facing`: it wins over the surface's, so
`{ on = "top", shift = [0.5, 0, 0.5], facing = "up" }` on a wedge is on
the slope, facing up.

Parts made from files (OBJ, glTF, other recipes) keep their sockets on
the box for now, as assemblies do; a later phase can give them the same
rule.

**What breaks**: recipes that join something to a socket that was in
the air.

1. Anything joined to a cone's `left`, `right`, `front` or `back`: it
   moves in onto the slant and tilts.
2. Anything joined to a wedge's `top` or `front`: it moves onto the
   slope and tilts (by 45° on a wedge as deep as it is tall).
3. Anything joined to an `on` + `shift` socket on a sphere, on a
   cylinder's round side, on a cone's sides or on a wedge's slope: it
   moves in onto the surface and tilts to match it.
4. A `shift` that puts a socket where the shape isn't at all (a corner
   of a cone's box, or high up on a wedge's side, above the slope) used
   to float; now it is an error that names the shift and says to bring
   its numbers closer to 0.
5. A wedge that declares a socket of its own called `slope` is now an
   error, since the name is built in.

None of the five examples changes: their `.glb` files stay byte for
byte the same, and the tests check that. Two of Duck Mold's own tests
join to a wedge's `top`, and are updated. `inspect` stops reporting
parts joined to those sockets as floating.

*As built:* the five examples did build to the same bytes. Then, as the
step's examples, the house gained a skylight on its roof's `slope`, and
a sixth example, `snowman.toml`, has eyes, buttons and arms on the
surface of balls and a badge on a cone's slant. How it works: the line
straight in from the box is followed through the shape's own faces, so
the socket is exactly on the faces drawn, also for shapes with few
`segments`.

### Step 4. `twist` for a joined part

- `twist = 90` turns a joined part around its join, as `rotate = 90`
  did, in the same direction.
- `rotate` is always `[x, y, z]`, and only on a part that isn't joined
  to anything.
- `rotate = 90` on a joined part is an error that shows the new form:
  *"a joined part turns around its join with `twist`: write
  `twist = 90`"*.
- `twist` on a part that isn't joined is an error too: *"this part
  isn't joined to anything, so there is nothing to twist around; to
  turn it, write `rotate = [0, 90, 0]`"*.

The recipe `version` stays 0: Duck Mold hasn't been released, and an
old recipe either still works or says exactly what to change. None of
the examples uses `rotate`.

*As built:* both errors are `conflict`s, like `move` on a joined part,
and each carries its fix as the suggestion (`twist = 90`,
`rotate = [0, 30, 0]`). `rotate = [x, y, z]` on a joined part, which
was already an error, now points to `twist` as well. The example is the
snowman's badge, twisted 45° into a diamond.

### Step 5. Sockets in the `.glb`

A game can ask where a socket is and put something there: a hat on
`head.top`, a sword in `right_arm.grip`. In Duck Forge:
`model.socket("head.top")` gives a point and the direction the socket
faces, in the model's own space.

- The `.glb`'s `extras.duck_mold` gains `sockets`: every socket of every
  named part, built-in and declared, where it ended up after building,
  as `{ "name": "head.top", "at": [x, y, z], "facing": [x, y, z] }`. The
  model's own sockets have bare names (`top`), a part's are
  `part.socket` (`head.top`), and parts inside assemblies keep their
  paths (`left_arm/hand.bottom`, `left_arm.grip`).
- **A part without a name has no sockets in the file**, and nor do the
  parts inside it: a game can't ask for a part it can't name, and the
  stand-in names (`#2`) change whenever the recipe's list does.
- They are stored in glTF's directions, like the corners (x negated), so
  the reader turns them back the same way it turns the corners.
- The robot has about 200 sockets, which makes its file roughly half as
  big again (about 15 KB of text). Small enough not to need a switch.
- The `version` inside the file stays 0: like `assemblies`, the new
  field changes the meaning of nothing old, and an older reader skips
  it.
- **The reader goes to version 3**, which reads `sockets` (none in older
  files). As the handoff note describes: change it here, raise the
  version in both marker lines, copy it into Duck Forge, and regenerate
  Duck Forge's test files. Duck Forge also gains `GlbModel::socket`, and
  its turntable example puts a hat on the robot's `head.top` while it
  turns. Then a pull request in each repository.
- `duck-mold inspect --sockets` lists every socket and where it is, to
  find the names to ask for.

*As built:* the robot has 187 named sockets, and its file went from
35.7 KB to 50 KB. Duck Mold reads a `.glb`'s sockets back too, so
`inspect --sockets robot.glb` lists the same ones as the recipe. The
Khronos validator still finds nothing to say about any example.

### Step 6. Importing glTF and `.glb`

`file = "lamp.glb"` or `file = "lamp.gltf"` makes a part that doesn't
move, exactly as `.obj` files do today. What it reads:

- `.glb` (one file) and `.gltf` (JSON text, with its numbers in a
  separate `.bin` file or written inside it as text).
- Every mesh in the file's main scene, each placed, turned and sized as
  the file says, merged into one part.
- Triangles in all three of glTF's layouts (separate, strips, fans), and
  corner numbers of any size.
- Material names become face names for `colors`, as `usemtl` names do
  for OBJ: `colors = { glass = "yellow", base = "steel" }`.
- **A material the recipe gives no colour keeps its own.** Its base
  colour is added to the palette under the material's name, and a
  warning lists those names, so nothing comes out grey by accident. To
  choose the colours yourself, give the part a `color` or `colors`. (If
  the palette already has a colour with that name, the palette's colour
  is used, and the warning says so.)
- A `.glb` that Duck Mold wrote keeps its colours, the way a recipe
  used as a part does.

What it leaves out, with a note in the report rather than an error:
textures, UVs, normals (Duck Mold works out its own), lines and points,
bones and skins, animations, shape keys (morph targets), cameras and
lights. Textures are not read, so a material's colour is its base
colour alone.

What it refuses, with a sentence saying why: compressed meshes (Draco,
meshopt, packed numbers), and any file that says it needs a glTF
extension Duck Mold doesn't know.

glTF's directions are the ones Duck Mold exports, so a model exported
and imported again comes back exactly where it was. A glTF metre is a
Duck Mold unit. No new libraries: Duck Mold already reads JSON, and
reading numbers written as text (base64) takes a few lines.

The example: a small lamp written as a `.gltf` by hand (two materials,
two pieces with their own positions), not by Duck Mold, so the import is
tested on a file Duck Mold didn't write, and a recipe that uses it
alongside a Duck Mold `.glb`.

*As built:* `crates/duck-mold-core/src/gltf.rs`, about 850 lines with
its own tests and no new library, reads into the same mesh type as OBJ, which now carries
each material's colour. The example is `examples/desk.toml`: a desk with
`parts/lamp.gltf` (three materials, four pieces: a box with 16-bit
corner numbers, one with 8-bit ones placed by a matrix, a shade as a
fan, a bulb with no corner numbers; the shade and bulb inside a tilted
arm; written by `examples/parts/make_lamp.py`, not by Duck Mold, and
checked clean by the Khronos validator) and `parts/crate.glb`. The
recipe paints the lamp's stand and leaves its shade and bulb to keep the
file's colours, so the desk is the one example with a warning, and the
tests expect it. The colour rule, as approved: a material the recipe
leaves uncoloured keeps its base colour, added to the palette under its
name; if the palette already has that name, the palette's colour is
used (and the warning says so, when the two differ). Faces with no
material, like an OBJ's, stay grey with the usual warning. A `.glb` that
Duck Mold wrote joins its palette to the model's at load time, exactly as
a recipe used as a part does, conflicts included. Base colours turn into
0-255 through Duck Mold's fixed sRGB table, so every computer gets the
same palette. `inspect` and `render` take `.gltf` and other `.glb` files
too.

### Step 7. The preview window

`duck-mold preview robot.toml` opens a window showing the model.

- Drag with the left mouse button to orbit; roll the wheel to zoom;
  Escape quits.
- Twice a second it checks whether the recipe, or any file the recipe
  uses, has been saved (by its time and size). If so, it rebuilds. If
  the new version has mistakes, they are shown in the window and the
  last good model stays up, so a half-typed recipe never blanks it.
- A line at the bottom gives the same summary as `render`.
- Nothing else: no editing, no menus.

Duck Forge runs the window, as it does for a game, through its `window`
feature, and draws with the same CPU renderer as `render`, so no
graphics card is needed. The preview is in the program by default;
building with `--no-default-features` leaves it out (and the window
libraries with it) for machines with no screen, and `render` still
works there. Duck Forge's own window keys come too, so F12 saves a
screenshot.

It is tested without a window by Duck Forge's `Simulation`, which feeds
it mouse drags, wheel turns and changed files and checks what it draws.

*As built:* `crates/duck-mold-view/src/preview.rs`. The model is drawn
in perspective (a 40° view), from the front right and a little above to
start, lit from over the viewer's shoulder whichever way it is turned.
Saving keeps the view where it was. Mistakes show in red and warnings in
amber at the top, wrapped to fit, and are printed to the terminal too.
`load` now lists every file it reads, the recipe first (palette files,
recipes used as parts, meshes and a `.gltf`'s `.bin`), and those are
what is watched. Built with `--no-default-features`, `duck-mold preview`
says it was built without its window and suggests `render`; without a
screen, it says so in one line. On Linux the window needs X11 and the
`libxkbcommon-x11` library when it opens, which Duck Forge loads then:
building needs neither. It was also tried for real on a virtual X
screen: the robot drawn, a colour change saved and shown, a broken save
shown in red over the last good model. CI builds and tests both ways.

Phase 1 ends with `PHASE1_REPORT.md`: what works, known limits, what
changed from this design, and what Phase 2 should start with.

---

## 18. Phase 2: the Studio

**Status: approved.** All five decisions at the end were answered yes,
with one condition and four additions, folded in below:

- *The condition:* when a part comes from a definition, the properties
  panel says so and names every part that will change, before the edit.
- *Placing a part anywhere on a surface:* an edit that declares a socket
  on a part, and in the Studio a click on any point of a face makes a
  socket there to join something to. Dragging a joined part along the
  surface changes that socket's `shift`.
- *Export and Render picture buttons* in the top bar, working without a
  terminal.
- *Two more edits:* `set-color`, and `duplicate --mirror`.
- *All of them in the agent guide*, like the other edits.

Phase 2 adds a window, the *Studio*, where a person builds a model with
the mouse instead of typing the recipe. Nine steps, one commit each, in
the order of the brief.

### The five rules, and how each is kept

1. **The recipe is still the model.** The Studio holds no model of its
   own. Every edit changes the recipe file at once, and the picture is
   then built from the file again. There is no "unsaved" state and no
   Save button.
2. **People and agents are equals.** Every edit is one value of one
   type in the library, `Edit`. The command line turns its words into an
   `Edit`; the Studio turns a click into an `Edit`; both call the same
   function. A test lists every kind of `Edit` and fails if one has no
   command, or isn't in the agent guide.
3. **Edits are gentle on the file.** An edit never writes the recipe out
   afresh. It works out a few small replacements in the text ("these
   characters become those") and leaves every other character alone.
4. **The file can change underneath.** The Studio checks the recipe and
   the files it uses twice a second, as the preview does, and builds
   again when one changes. "Both at once" is below.
5. **The same window library.** egui, opened by eframe, version 0.37:
   what Duck Quill and Duck Forge's Studio use. The viewport is drawn by
   Duck Forge's CPU renderer into a picture, which egui only shows.

### The edit commands

```bash
duck-mold edit robot.toml add hat --shape cone --to head.top --color red
duck-mold edit robot.toml set hat --radius 0.2 --height 0.3 --twist 45
duck-mold edit robot.toml join hat --to body.neck
duck-mold edit robot.toml rename hat cap
duck-mold edit robot.toml duplicate cap --name cap_2 --to left_arm.grip
duck-mold edit robot.toml remove cap_2
duck-mold edit robot.toml add-color gold "#F2C84B"
duck-mold edit robot.toml set-color gold "#E0B030"
duck-mold edit robot.toml duplicate left_arm --mirror
duck-mold edit robot.toml add-socket body badge --on front --shift 0.4,0.6,0
duck-mold edit robot.toml set-socket body.badge --shift 0.2,0.6,0
```

| Command | What it does |
|---|---|
| `add <name> --to <part.socket>` | Adds a part joined to a socket. What it is made of: `--shape box` (or cylinder, sphere, cone, wedge), `--use <defined part>`, or `--file <recipe or mesh>`. Optional: `--attach <its own socket>`, the size options, `--color`, `--twist`. |
| `remove <part>` | Removes a part. If other parts hang from it, it is refused and lists them; `--with-attached` removes those too. |
| `rename <part> <new name>` | Renames a part, and changes every `to`, `from` and `origin` that named it. |
| `join <part> --to <part.socket>` | Moves a part's join to another socket. `--attach` changes which of its own sockets it joins by. |
| `set <part> ...` | Changes a part: `--size x,y,z`, `--radius`, `--height`, `--segments`, `--shape`, `--color`, `--face-color top=plank`, `--twist`. Several at once are one edit. |
| `duplicate <part>` | Copies a part, comments and all, right after it, under a new name (`--name`, or `hat_2`). It joins the same socket unless `--to` says another. |
| `duplicate <part> --mirror` | The same, as a mirror image: a right arm from a left arm. The copy is flipped left to right, its twist is reversed, and it joins the socket with `left` and `right` swapped in its name (`body.right_arm` for `body.left_arm`) unless `--to` says another. Its name swaps them too. |
| `add-color <name> <#RRGGBB>` | Adds a colour to the recipe's palette. |
| `set-color <name> <#RRGGBB>` | Changes a colour that is already in the palette. Everything painted with it changes. |
| `add-socket <part> <name> --on <side>` | Declares a socket on a part, on one side of it, slid across that side by `--shift x,y,z`. It then sits on the surface, like any socket. |
| `set-socket <part.socket>` | Moves a socket declared that way: `--shift`, and `--on` for another side. Whatever is joined to it moves with it. |

Rules for all of them:

- **A part is named as `inspect` prints it**: `head`, `left_arm/hand`,
  or `#2` for a part with no name.
- **Each prints the same report as `inspect`**, for the recipe as it is
  after the edit, with a line saying what changed. `--json` gives it as
  data, with the lines that changed.
- **`--dry-run`** reports what would happen and writes nothing.
- **An edit that would break the recipe is refused.** If the recipe
  would have more mistakes after the edit than before, nothing is
  written, and the mistakes are listed. Exit codes are as everywhere:
  0, 1 for mistakes, 2 for a command that can't be carried out.
- **When `--attach` isn't given** for a new part, it joins by the side
  opposite the socket it goes on: its `bottom` on a `top`, its `left`
  on a `right`, otherwise its `bottom`.
- **A part inside a defined part** (`left_arm/hand`, where `left_arm`
  uses the definition `arm`) is edited in the definition, so every part
  that uses `arm` changes. The report lists them.
- **A part inside another recipe file** (`file = "wheel.toml"`) is
  refused, with a line saying to open that file.

Not edits in this phase: moving a part that isn't joined (`move`,
`rotate`), `scale`, `mirror` (except through `duplicate --mirror`),
sockets written with `at` or `from`, and `define`. They are still typed
in the recipe, and the Studio shows them.

*As built:* `crates/duck-mold-core/src/edit/`. `doc.rs` holds the
replacements (set a value, add or remove a field, add a table, add,
remove or copy an item of a list), each tested on its own in TOML and
JSON; `mod.rs` holds the `Edit` type, finds parts by the path `inspect`
prints, and checks the result. The TOML reader already gave the place of
every header, so a part's block is found from the tree, with no second
parser. Two things the design didn't say: an edit that changes nothing
writes nothing and says so; and `remove` is refused if it would leave
the recipe with no parts, since that is a new mistake.

### How an edit rewrites the file

Duck Mold already knows the line and column of everything in a recipe,
for its error messages. Phase 2 makes it remember where each thing
*ends* as well. An edit then becomes a short list of replacements:

- **Changing a value** (`radius = 0.5` to `radius = 0.2`) replaces the
  characters of the old value. The spacing around it and a comment
  after it on the same line stay.
- **Adding a field** puts one new line in the part, in the usual order
  of fields, and before the part's own sub-tables (in TOML a field
  written after `[parts.sockets]` would belong to the sockets).
- **Adding a part** puts a new block after the last part of the list it
  joins, written the way `docs/recipe-format.md` shows.
- **Removing a part** takes out its block, the comment lines directly
  above it, and its sub-tables.
- **Renaming** replaces the name and each place that refers to it.

Everything else in the file is untouched, byte for byte: comments, blank
lines, the order of parts, how numbers are written, Windows or Unix line
endings. Both TOML and JSON work this way, with the same code deciding
*what* to change and a small part per language deciding *how it is
written* (JSON's commas and braces, TOML's headers).

After the replacements, the new text is read again and built before
anything is written. If it doesn't come out as the edit meant (it reads
with a mistake, say), the edit is refused and the file stays as it was.

One limit: recipes can also write a list of parts on one line
(`parts = [{...}, {...}]` in TOML). Changing values there works; adding
or removing a part there is refused with a sentence suggesting the usual
`[[parts]]` form.

### The file changing underneath, and both at once

- The Studio never holds a change that isn't in the file, so there is
  nothing to merge and nothing to ask.
- **Writing** is done by writing a new file beside the recipe and
  swapping it in, so a program reading the recipe never sees half of it.
- **An outside change** (an agent's edit, a save in a text editor) is
  noticed within half a second. The model is built again, the view stays
  where it was, and the selected part stays selected if it still exists.
- **Both at once**: every edit, from the Studio or the command line,
  reads the file again just before it writes. If the file is no longer
  what the edit was worked out on, the edit is worked out again on the
  new text. Edits name parts, not lines, so this usually just works, and
  both changes are kept. If the edit no longer makes sense (its part was
  removed or renamed meanwhile), it is refused with a message, and the
  outside change stands.
- What is left is two programs writing within the same few thousandths
  of a second, where the later one wins. There are no lock files.
- A recipe saved with mistakes shows them in the problems panel; the
  last good model stays in the viewport, as in the preview.

### Undo and redo

- The Studio keeps, for each step, the file's text before and after: at
  most 500 steps, of every file the step changed. Undo writes the
  "before" text back; redo the "after". This is how Duck Forge's Studio
  does it.
- Because every edit is saved at once, undo always works "across a
  save": there is no moment when it doesn't.
- **A change from outside becomes a step too**, called "Changed outside
  the Studio". So undo always takes back the most recent change to the
  file, whoever made it, and the status line says which.
- Dragging a number (a twist, a size) writes the file as it moves, at
  most ten times a second, and counts as one step.
- The steps are kept while the Studio is open, and forgotten when it
  closes.
- Undo is the Studio's own: there is no `duck-mold edit undo`. An agent
  has `--dry-run` before an edit and git after it.

### The Studio's layout

```text
+------------------------------------------------------------------+
| robot.toml  Undo Redo  Add part v  Frame  Export  Render picture  |
+-------------+--------------------------------------+-------------+
| PARTS       |                                      | head        |
|  body       |                                      |  name  head |
|  head     < |              viewport                |  shape box  |
|   left_eye  |      (Duck Forge's CPU renderer)     |  size  ...  |
|   antenna   |                                      |  colour ... |
|  left_arm   |       grid on the ground             |  joined to  |
|   upper     |       sockets as small markers       |   body.neck |
|   hand      |                                      |  twist 0    |
+-------------+                                      |             |
| LIBRARY     |                                      |             |
| [pic] [pic] |                                      |             |
| lamp  crate |                                      |             |
+-------------+--------------------------------------+-------------+
| PROBLEMS  warning, line 70: the material "shade" keeps its ...   |
+------------------------------------------------------------------+
```

- **Viewport.**
  - Left-drag orbits. Right-drag or middle-drag pans (Shift + left-drag
    too, for laptops). The wheel zooms. "Frame" brings the whole model,
    or the selected part, back into view.
  - A grid lies on the ground, one square to a metre.
  - A click selects the part under the pointer. Duck Mold finds it by
    following a line from the eye through the pointer to the first
    triangle it meets, so the picture and the pick always agree.
  - The selected part is brightened, with the box around it drawn.
- **Parts** (the outliner): the tree `inspect` prints. A click selects.
  The selection is the same in both places.
- **Properties** of the selected part: name, what it is made of, its
  sizes, its colour (a list of the palette, with "new colour..."), what
  it is joined to, and its twist. Changing one is a `set`, `rename` or
  `join` edit.
  - **A part that comes from a definition says so**, above its fields:
    "From the definition `arm`. Changing it also changes:
    `right_arm/hand`", naming every part, before any edit is made.
- **Sockets** show as small markers on the selected part.
  - Clicking one opens a short list (the five shapes, the recipe's
    defined parts, the library), and choosing from it is an `add` edit.
  - **Clicking any point of the selected part's surface** offers the
    same list for a new socket there: an `add-socket` edit, then `add`.
  - Dragging the selected part shows every other part's sockets, and
    dropping it on one is a `join` edit. Dropping it on a part's surface
    away from a marker makes a socket there and joins it.
  - **Dragging a part that sits on a socket of its own along the
    surface** slides that socket: `set-socket` edits, one undo step.
- **Export** and **Render picture**, in the top bar, write the `.glb`
  and the four-view picture next to the recipe, as `duck-mold export`
  and `duck-mold render` do, and say where they went.
- **Library**: a folder of recipes and model files (`parts/` beside the
  recipe, or one you choose). Each has a small picture, drawn by the
  same code as `render`. Choosing one for a socket is an `add --file`
  edit.
- **Problems**: `inspect`'s errors and warnings, updated after every
  change. Clicking one selects the part it is about.
- With no file given, the Studio starts on a page with "Open" and "New
  model" (a recipe with one box) and the recipes opened lately.

### Where the code goes

| Crate | What Phase 2 adds |
|---|---|
| `duck-mold-core` | `edit`: the `Edit` type, working out the replacements, and applying one to a file. No window, no pictures. |
| `duck-mold-cli` | `duck-mold edit ...`, and `duck-mold studio <file>`, which starts the Studio program. |
| `duck-mold-view` | Everything the Studio *does*, with no window: the selection, the camera, picking, the undo steps, watching files, and drawing the viewport into a picture. |
| `duck-mold-studio` (new) | The window only: egui panels that show the above and pass clicks on. It builds `DuckMoldStudio.exe`. |

The Studio's screen is written as a function that draws into a space it
is given, not as the owner of the window. That, and using the same egui
as Duck Forge's Studio, is what lets it be docked there later.

The command-line program doesn't contain egui, so agents and machines
without a screen build it as quickly as before. `duck-mold studio`
starts the Studio program that sits beside it.

### Testing without a screen

- **Edits**:
  - Every edit is run on every example recipe, in TOML and in JSON. The
    test checks that only the expected lines changed and every other
    byte is the same.
  - It checks that the model built afterwards is what the edit meant.
  - It checks that an edit and its opposite (rename and rename back)
    give back the exact file.
  - It checks that comments, blank lines and Windows line endings
    survive.
- **What the Studio does** is plain code in `duck-mold-view`, so tests
  drive it directly: select, edit, undo, change the file from outside,
  and check the file's text and the Studio's state.
- **The viewport** is drawn by the CPU renderer, so its pictures are
  golden files compared byte for byte on Windows and Linux, like
  `render`'s: the grid, a selected part, the socket markers.
- **The whole window** is tested with egui's own test tool
  (`egui_kittest`, as in Duck Forge's Studio). It runs the real screens
  with no window, clicks things by their label, and can save a picture
  of the screen.
  - Tests that click through the screens run on Windows and Linux.
  - Pictures of whole screens are compared with stored ones on Linux
    only, allowing a tiny difference. egui draws its text and panels
    through the graphics driver, and drivers don't agree to the last
    pixel. Linux has a software driver that CI can use.

### What happens to `duck-mold preview`

It stays as a command and becomes the Studio in look-only mode: the
viewport and the problems panel, with no editing. The window code from
Phase 1 (Duck Forge's game window) is removed, so Duck Mold has one
window library instead of two, and `preview` gains pan and the grid.

*As built (step 9):*

- `DuckMoldStudio.exe` opens no console in a release build, and carries
  an icon: a picture of `examples/duck.toml`, a duck made in Duck Mold.
  A test draws the recipe again and compares it with the stored icon
  files byte for byte, so the icon can't drift from the recipe.
- `Start Duck Mold Studio.bat` and `Create desktop shortcut.bat` are in
  the repository's top folder; the download has its own `Create desktop
  shortcut.bat` and `READ ME FIRST.txt`, from `release/`.
- `.github/workflows/release.yml`, a copy of Duck Quill's, builds
  `DuckMold-<version>-windows.zip`: both programs, the examples, the
  recipe format and the agent guide. It publishes a release from the
  "Run workflow" button or a `v` tag, and on a branch only builds the
  zip to look at. It checks the zip: the files are there, `duck-mold`
  knows its version and reads an example, and finds the Studio next to
  it.
- The start page lists the recipes opened lately (eight at most), kept
  in one small text file in the system's settings folder
  (`%APPDATA%\Duck Mold\recent.txt`). Nothing else is stored outside the
  recipe files.
- The version is now 0.2.0.

*As built (step 8):* the library is the folder `parts` next to the
recipe, or one chosen with the Folder button (for this sitting; it isn't
remembered). Each file's small picture is the three-quarter view of
`render`'s sheet, 96 pixels square, drawn by the same code, so it is the
same on every computer; a test compares one byte for byte. A file that
doesn't build is listed without a picture and can't be chosen; its tip
says what is wrong with it. The library panel only shows what there is:
a part is chosen in the list a click on a socket opens, where the
library's parts come after the shapes and the recipe's own definitions.
A file outside the recipe's folder is written with the way to it
(`../shared/cap.toml`). The folder is looked at twice a second, like the
recipe. The Studio also names files from the recipe's folder everywhere
it shows them (`parts/lamp.gltf`, `Wrote robot.glb`), as the recipe
does, not by the whole way from the top of the disk.

*As built (step 7):* as designed. A step is the file's whole text
before and after a change; undo writes the "before" back, but only if
the file still holds the "after" (so nothing saved in that instant is
written over; the Studio then says to try again). Before every undo and
redo the Studio looks at the file, so a save from outside that it hadn't
seen yet becomes the newest step, and is the one taken back. Edits made
together (a socket and the part on it; a drag) are one step. Ctrl+Z and
Ctrl+Y work unless a text field is being typed in, where they belong to
the typing. There is no undo on the command line: an agent has
`--dry-run` before and version control after.

*As built (step 6):*

- **A drag writes once, when the part is dropped**, not ten times a
  second while it moves as first planned. While dragging, a marker under
  the pointer shows where the part will go. That is one write, one undo
  step, and no half-finished recipe for an agent to read in between.
- **Dropping on a surface slides or makes a socket.** If the part sits
  on a socket that was declared with `on` on that same part, and nothing
  else sits on it, that socket is moved (`set-socket`). Otherwise a new
  socket, `spot`, `spot_2`..., is declared at the point (`add-socket`)
  and the part joined to it (`join`): two edits, one undo step. A socket
  left behind with nothing on it stays in the recipe; there is no edit
  yet that removes one.
- **A point becomes "side and shift"** in the library
  (`edit::spot`), next to the code that places declared sockets, and a
  test checks on every face of four parts (a box, a twisted box, a ball,
  and a wedge that is turned, scaled and mirrored) that the socket lands
  on the point that was clicked. On a round part a point low on the side
  can be hidden behind a nearer face from the box side it is reached
  from; the socket then lands on that nearer face.
- **Markers on the far side of a part are drawn small and can't be
  clicked**: a click meant for the surface in front would hit them. Turn
  the view to reach them.
- **A part can't be dragged onto what hangs from it** (a head onto its
  own antenna): those sockets aren't offered.
- **The picture stays still through an edit.** A model's zero point is
  the middle of its footprint, so adding a part on one side moves every
  other part the other way. The camera goes with them.
- A click on the selected part itself now means "add here", so a part
  made of parts is still selected whole first, then the part inside,
  and only then does a click on it open the list.

*As built (step 5):* a part comes from a definition in two ways, and
the panel says which. A part *inside* a definition (`left_arm/hand`):
"Inside the definition "arm". Any change here also changes:
right_arm/hand." A part *made from* a definition (`left_eye`, with `use
= "eye"`): "Made from the definition "eye". Changing its shape, size or
colour also changes: right_eye." For the second kind the `set` edit
itself had to learn something: the eye's size and colour are written in
the definition, not on the part, so `set` now changes them there (on the
command line too), and reports the other parts. The part's name, join
and twist are its own. A size is typed or dragged and becomes an edit
when it is settled (Enter, a click elsewhere, the mouse let go), so a
drag is one edit, not hundreds.

*As built (step 3):* the Phase 1 window code and its `preview` switch
are gone, so every build is the same and CI no longer builds twice. The
Studio is the program `DuckMoldStudio`; `duck-mold studio` and `duck-mold
preview` look for it next to `duck-mold` and start it, and wait until
its window is closed, as `preview` always did. One command was added
that the design didn't list: `duck-mold new <recipe>` writes the starter
recipe (one box). The start page's "New model" calls the same function,
so that making a model is something an agent can do too.

### Step 2: surface sockets for assemblies and parts from files

The Phase 1 rule ("a socket that would float moves straight in onto the
surface and faces out of it") is extended to parts made of parts and
parts made from files. The search goes through every face inside the
part. Two choices:

- **A line that meets nothing** (the middle of a ring, say) leaves the
  socket on the box, as it is today.
- **The model's own seven sockets stay on its box.** They say how big
  the model is and where the ground is. Otherwise a table's `bottom`
  would move up between its legs to the underside of its top, and the
  table would sink into the floor.

What changes for existing recipes: a part joined to a built-in or
`on` + `shift` socket of an assembly or a file part, where that socket
was in the air, moves onto the surface. The tests will show whether any
example changes, and the report will list them.

*As built:* no example's shape changed: every golden picture and report
is the same. Three `.glb` files changed, in their sockets only: the
house's `roof.front` and `roof.back` are now on the roof's slopes, the
robot's arms' and legs' side sockets are on the limbs, and the desk
lamp's `top`, `front` and `back` are on the lamp. One more choice: a
recipe file used as a part keeps its own sockets on its box, like the
model, so that a recipe's sockets are in the same places however it is
used. A socket declared with `on` + `shift` over a gap in an assembly
stays on the box and is no mistake (on a single shape it still is).
Two of Duck Mold's own tests changed with the rule: their robot's arm
declares its shoulder on the arm as a whole, and that socket is now on
the upper arm itself, so the arm joins flush where it used to stop short
by the width of the hand.

### Step 9: starting it on Windows without a terminal

As Duck Quill does:

- `DuckMoldStudio.exe` opens no console window and has its own icon.
  Double-clicking it opens the start page; dropping a recipe on it opens
  that recipe.
- From a copy of the repository: `Start Duck Mold Studio.bat` builds it
  and starts it, and `create-shortcut.bat` puts a shortcut on the
  desktop.
- A release workflow builds a zip with `DuckMoldStudio.exe`,
  `duck-mold.exe`, the examples and a "read me first" note. No installer
  in this phase.

### The decisions

These were the five questions; all were answered yes.

1. **`preview` becomes the Studio in look-only mode**, and the Phase 1
   window code goes.
2. **Undo is the Studio's, and is forgotten when it closes.**
3. **Editing a part that comes from a definition changes the
   definition**, and so every part that uses it. *On one condition:* the
   properties panel says so and names every part that will change,
   before the edit.
4. **Whole-screen pictures are compared on Linux only**; the viewport's
   pictures and all the clicking tests run on both.
5. **A zip, not an installer**, and an icon made in Duck Mold itself.

## 19. Phase 3: a real 3D tool

**Status: approved**, with all eight decisions at the end as
recommended.

Phase 2's Studio could add a part at a socket and change its numbers. A
person who opened it found that it "looks like a very simple and weak
version of Blender": the cube couldn't be dragged, moved or formed, the
options were few, and the first page was a title and two buttons.

Phase 3 fixes those four things. Ten steps, one commit each, in the
order of the brief.

The four rules stay:

1. The recipe is the model. The Studio keeps no model of its own.
2. Every Studio action that changes the model is an edit command an
   agent can run too.
3. An edit changes only what it has to in the file.
4. The viewport is Duck Forge's CPU renderer.

*As built (step 10):* the first-run tour, as designed, with these
things settled in the building:

- **Each tip is a small box beside what it is about**, with a ring
  round the thing and a line to it. It is not in the way: the Studio
  works as usual while a tip is up.
- **The tips about the arrows and the dots need a part selected**, so
  from the third tip on the tour selects the first part if none is
  (selecting changes nothing in the model), and the third takes the
  Move tool.
- **Shown once** means once for the person, not once for each model:
  an empty file `tips-seen` beside the list of models opened lately
  says they were shown, or skipped. Where there is nowhere to keep
  that (the tests), no tips are shown.
- **Not when only looking** (`duck-mold preview`, a model file):
  there is nothing there to learn to do.
- The last tip's button says Done, and has no Skip beside it.

*As built (step 9):* the start page, as designed, with these things
settled in the building:

- **The examples are inside the program** (the recipes' text, and the
  two model files the desk uses), so the gallery and
  `duck-mold new <recipe> --example <name>` work wherever the program
  is, with no `examples` folder beside it.
- **The examples' pictures are stored files** (192 pixels square, in
  `assets/examples/`), put inside the program too. A test draws each
  again from its recipe with `render`'s code and fails if a stored one
  is no longer what the recipe draws, as the icon's test does. The
  models opened lately are drawn when the start page is shown; one
  that no longer builds is a plain tile with a question mark, and
  still opens, to show what is wrong.
- **The copies' folder** is `Duck Mold` in the person's Documents
  (`%USERPROFILE%\Documents` on Windows; `$XDG_DOCUMENTS_DIR` or
  `~/Documents` elsewhere). It is made the first time an example is
  opened, and the page says where it is. The desk's copy brings its
  `parts` folder; a file already there is left as it is.
- **The duck on the page** is the duck example's picture.
- **File > Open an example** and **File > Opened lately** offer the
  same two lists once a model is open, and **File > Back to the start
  page** closes it (the model is already in its file).
- Ctrl+N and Ctrl+O work on the start page.

*As built (step 8):* tidy sockets, with these things settled in the
building:

- **What counts as using a socket**: a part joined to it (`to`), the
  part's own join if it joins by it (`attach`), and a socket of what
  the part is inside that passes it on (`from`). `rename-socket`
  changes all three; `remove-socket` is refused while there is any,
  and says which.
- **Only declared sockets** can be renamed or removed: the seven every
  part has, and a wedge's `slope`, are built in.
- **The Studio clears away its own sockets by itself.** When an edit
  removes a part, joins it somewhere else or unjoins it, and that
  leaves a socket called `spot` (or `spot_2`...) with nothing on it,
  a `remove-socket` for it is added to the same step. The status line
  says so, and one undo brings the part and the socket back together.
  A socket with any other name stays: a person named it.
- **`tidy-sockets`** does the same for the whole recipe at once: it
  removes every unused socket with one of the Studio's names, and says
  which. Run again it changes nothing.
- **Removing a part's last socket** takes the empty `sockets` table
  with it, and the blank line above it, so the file is as it was
  before the socket came.
- **In the panel** each declared socket is a line: its name in a field
  to type in, how many things use it, and a "remove socket" button
  that only works when nothing does.
- **The "remove them too?" question** became a small window in the
  middle: with the palette under the properties, it could end up
  scrolled out of sight at the bottom of the panel.

*As built (step 7):* the palette panel and the Paint tool, with these
things settled in the building:

- **A part of the model knows the names of its faces** now: which of
  its faces is the `top`, which the `side`. That is what lets a click
  find the name to write in `colors`. (It is also the first thing
  Phase 4's face editing needs.)
- **Picking a colour takes the Paint tool.** With the tool in hand and
  no colour picked, the brush has the palette's first colour. Clicking
  the picked colour again puts the brush down and gives the arrows
  back.
- **With the Paint tool, a click paints and selects nothing**, and
  Shift+click paints the whole part rather than adding to the
  selection. Shift+click also takes away the colours the part's single
  faces had, so the part really is one colour.
- **`remove-color` is refused while the colour is in use**, naming
  everything painted with it, a part's `color` or a face's, anywhere in
  the recipe, definitions included. `--replace-with` repaints those
  first. The Studio asks in a small window, with the first other
  colour offered.
- **The two forms** (a new colour; what to paint with instead) are
  small windows in the middle, not rows of the panel: the panel is too
  short to hold them without scrolling.
- A colour square in the palette is the same colour picker the
  properties panel has; letting go of the mouse makes the `set-color`
  edit.
- A recipe whose palette is another file can be painted with, but not
  changed here: the edit says to change that file.

*As built (step 6):* views, wireframe, the shadow and the menus, with
these things settled in the building:

- **The shadow is a grid of cells on the ground**, 96 along the model's
  longer side. Every face darkens the cells straight under it, the
  more the nearer it is to the ground (a foot on the ground is darkest,
  a roof four metres up palest), and the edges are softened by
  averaging each cell with its neighbours three times. It is drawn as
  flat squares before the grid's lines, so the lines stay on top. From
  below the ground it isn't drawn. It is worked out again only when the
  picture is.
- **Wireframe draws each face's outline** in the face's own colour made
  lighter (a selected part's lighter still). A click still finds the
  part: the faces are there, only not filled.
- **The four views keep what is selected in view**, or the whole model
  when nothing is: they turn the camera and then frame, as F does.
  "Three-quarter" is how the Studio opens (30 degrees round, 20 up),
  so key 4 is the way back. From the top, the model's front is at the
  bottom of the picture.
- **The status line moved** from the top bar to the right end of the
  Problems row, as the design's sketch has it: the top bar needed the
  room for the views and the switches.
- **The top bar wraps** to a second row on a narrow window.
- **File > Open an example and Opened lately** come with the start
  page (step 9); **Edit > Tidy sockets** with step 8; **Help > Show
  the tips again** with step 10.
- **Ctrl+N, Ctrl+O and Ctrl+E** work when looking only too, as Frame,
  the views, the switches and hiding do: none of them changes the
  model.

*As built (step 5):* the right-click menu, the keys, several parts and
hiding, with these things settled in the building:

- **Which parts a group edit moves.** The free parts in the same list
  as the part selected last get an edit each. A part joined to one of
  them (or to one joined to one of them), and a part inside one of
  them, goes along with no edit. Any other selected part stays, and
  the status line names it: "Not moved: flag (joined to a part that
  isn't selected, or inside another part)".
- **Scale with several parts has one handle**, the box in the middle:
  bigger or smaller all round, and further from or nearer to the
  middle of them. Scaling several parts along one direction would need
  each part's own sides to line up with it, which they needn't.
- **A turn carries each part round the middle of the group** as well
  as turning it, so the group turns as one thing. The middle is the
  middle of the box round the parts that move.
- **Selecting a part and a part inside it** would move the inner one
  twice. Shift+clicking one takes the other out.
- **A copy of a free part stands beside the original**, one step past
  its width along x, because a copy in exactly the same place can't be
  seen or told apart. (This is the copy the first trial of the Studio
  couldn't move.) Several copied together keep their places relative
  to each other. A copy of a joined part is still on the same socket:
  drag it to where it goes.
- **Remove asks only about parts that aren't selected.** If what hangs
  from the selected parts is itself selected, nothing is asked.
- **The status line says a group edit in a few words** ("Moved 3
  parts: x +0.3 m"), and so does Undo's tip. The recipe still gets one
  `set` for each part.
- **Hide** is by part: a part joined to a hidden part is still drawn.
  The eye is the word "hide" (or "show") at the end of the part's line
  in the list, not a picture of an eye: the font the Studio ships has
  none that reads well at that size. A hidden part can't be clicked in
  the picture, isn't selected by Ctrl+A, and is in every export.
- **The right-click menu is the same in the picture and in the list.**
  From the picture, "Add a part here" adds at the point that was
  clicked; from the list, on the part's `top`.
- **Shift+A opens a small window** in the middle of the picture with
  the same choices as the Add menu, rather than the menu itself.
- **Looking only** (`duck-mold preview`, and model files): Frame, Hide
  and Select all work; nothing that changes the model does.
- The File, Edit, View and Help menus come with the views, in step 6,
  from the same list of actions the keys and the right-click menu use.

*As built (step 4):* the seven shapes, `bevel`, `bevel_steps` and
`taper`, as designed, with these things settled in the building:

- **Every face is still flat and bulges nowhere inwards**, which is
  what sockets and Phase 4's face editing count on. So an arch's front
  is a leg at each side and triangles from its two top corners to the
  round top; stairs' sides are one upright strip per step; a tube's
  ends are one four-cornered face per side. Every edge of every shape
  is shared by exactly two faces: no cracks.
- **Which way a face points** is found two ways. A shape that bulges
  outwards everywhere is turned by where its middle is, as before. The
  four that don't (torus, tube, stairs, arch) start from the face
  furthest to the right and go from face to face across shared edges.
- **A low arch** (less tall than half its width, plus its wall) has a
  flatter round top instead of being an error.
- **A capsule no taller than it is wide** is all ends: it has no
  `side` faces.
- **Taper bends some faces** (a cylinder narrowed more one way than the
  other, the upright cut of a bevelled box narrowed two ways). A face
  it bends is cut into triangles, which are always flat.
- **Bevel and taper together**: the bevel is made first, then the
  taper, so the widest place is where the bevel ends, a little above
  the bottom, and the shape is a hair narrower than its `size` there.
- **Sockets at a cut edge lean with the cut.** That is the old rule
  (a socket sits on the surface and faces out of it), but it surprised
  me twice in the examples: a socket declared at the foot of a bevelled
  box's side, and the join of two sockets that face the same way. The
  examples say `facing` there, and the docs say why.
- **The Size tool** has a second knob on a torus, on top of the ring,
  for `tube`; its `radius` knob is on the outside of the ring. `wall`,
  `steps`, `bevel` and `taper` are in the panel, not on knobs.
- **New shapes are added with every size said**, each a share of the
  size they are added at, because a default (a wall of 0.2 m) can be
  bigger than the whole shape.
- **The lists of shapes** (the Add menu, and the one a dot opens) are
  in two columns: twelve in one column didn't fit the screen with the
  library under them.
- `set --bevel 0` and `--taper 1` take the field out of the recipe
  rather than writing a 0 or a 1.

*As built (step 3):* the Add menu, and `add` with no `--to`.

- **Where the new part goes.** "In front of the camera" is the ground
  under the point the view looks at, snapped to the step. Usually the
  model is standing right there, so the Studio first builds the part
  to see how much room it takes, and then walks from that point
  towards the eye, a step at a time, until the part's box is one step
  clear of every other part's. So it lands just in front of the model
  as you see it, never inside it or behind it.
- **Its size and colour.** A new shape is half the model's smallest
  side across, to two figures (the same rule as a part added on a dot,
  which is half the part it sits on). It has the selected part's
  colour, or the palette's first.
- **The ground it stands on** is the ground the model stood on before:
  the underside of its lowest part. The edit works this out, not the
  Studio, so an agent's `add` with no `--to` stands on the ground too
  (`--stand x,z` says where; `--move x,y,z` gives the middle exactly).
- **A recipe with no parts** can be added to: the first part starts
  the list.
- **A free part that touches nothing gets the usual warning** ("it
  floats 0.125 from ..."), as any part that touches nothing always has.
  It goes when the part is moved against another, or joined.
- The Add menu shares its list with the one a dot opens. Shift+A comes
  with the other keys, in step 5.

*As built (step 2):* the Size tool, as designed. One thing to know: a
box that is joined is held by its join, so pulling a face changes its
size about its middle and the join then puts it back on its socket;
only a free part keeps the opposite face exactly where it was, by
moving its middle half as far in the same edit. A part made from a
definition has the definition's knobs, and pulling one changes every
part made from it (the status line names them).

*As built (step 1):*

- **Moves snap in steps from where the part was**, not onto the grid's
  lines. Shapes are placed by their middle, so a crate half a metre
  wide standing on a floor has its middle at 0.35 m; snapping it onto
  the lines would lift it off the floor every time it was touched. In
  steps of 0.1 m it stays on the floor. Scales do snap to whole
  tenths, and turns to whole steps of 15 degrees from where they were.
- **Lifting the lowest part** leaves it on the ground and brings the
  others down, and the status line says so. Only a model with one part
  shows nothing at all, and the status line says that instead.
- **A ring turns the part around the list's own x, y or z**, and the
  recipe's `rotate` is worked out afresh as the three numbers that give
  the same turn (to a hundredth of a degree). After a second turn
  around another direction the numbers can all change; the part is
  where the rings put it.
- **Rotate turns a free part about the point `move` places**, the
  shape's own middle, so only `rotate` changes. Move and Scale stand at
  the middle of the part's box.
- **`unjoin` was needed sooner than the right-click menu**, so it is a
  button in the properties panel already.
- A click on the selected part no longer opens the "add here" list
  (decision 3). Until the right-click menu of step 5, a part is added
  at a point of a surface from the command line only.
- The library gained what this needed: a matrix's inverse and its turn
  back to `rotate` numbers, each part's place in its own list, the
  model's zero in the recipe's measurements, and `apply_many`, which
  works out several edits as one and is what the preview uses.

### Why the cube couldn't be moved

A new model is one box. Duplicate it, and the copy is a *free* part: it
is joined to nothing, and its place is two fields in the recipe, `move`
and `rotate`. Phase 2 had no edit for those fields. Dragging only knew
how to drop a part on a socket.

So the heart of this phase is: **free parts can be moved, turned and
sized with the mouse**, and joined parts can be slid and twisted the
same way.

### The ground and the grid

Two things about the ground change, because handles need a ground that
stays put.

- **The grid no longer slides.** Today, adding a part on one side moves
  the model's middle, and the grid moves under everything. From now on
  the grid is fixed to the recipe's own measurements: a part with
  `move = [1, 0, 2]` stands over the grid's line 1 and line 2.
- **The ground is always under the lowest part.** This isn't new: a
  model's zero point is the middle of its underside, which is how a
  game stands it on the floor. It has one visible consequence. Lifting
  the lowest part, or the only part, changes nothing you can see: the
  ground comes up with it. The Studio will say so in the status line
  ("this is the lowest part: the model always stands on the ground").
  With a second part under it, lifting works as you expect.

A small marker on the ground shows the model's zero point: the point a
game holds the model by.

The grid itself was on by default, but was easy to miss. It becomes:
light lines every 10 cm near the model, heavier lines every metre, a
red line and a blue line through the middle, reaching well past the
model.

### The tools

A strip of five buttons down the left edge of the viewport. One is
active at a time. Each has a key.

| Tool | Key | What shows on the selected part | What dragging does |
|---|---|---|---|
| **Move** | G | Three arrows: red (left-right), green (up-down), blue (front-back) | Moves it. This is the tool the Studio opens with. |
| **Rotate** | R | Three rings in the same colours | Turns it. |
| **Scale** | S | Three small boxes on lines, and one in the middle | Makes it bigger or smaller, or stretches it one way. Works on any part. |
| **Size** | T | A round knob on each face of a shape | Pulls that face: changes the shape's own width, height, depth or radius. |
| **Paint** | P | Nothing; the face under the pointer lights up | A click colours that face (step 7). |

G, R and S are Blender's keys.

**What each handle writes, for a free part:**

| Handle | Recipe field | Snaps to |
|---|---|---|
| A move arrow | `move = [x, y, z]`, one number | The grid: 0.1 m, or 0.01 m when you are zoomed in close |
| Dragging the part itself | `move`, across the ground (x and z) | The same |
| A rotate ring | `rotate = [x, y, z]` | 15 degrees |
| A scale box on a line | `scale = [x, y, z]`, one number | 0.1 |
| The middle scale box | `scale`, all three alike | 0.1 |

**For a joined part**, the same tools do what a join allows:

| Handle | What it does | Recipe field |
|---|---|---|
| Move: two arrows lying on the surface | Slides the part along the surface it sits on | The socket's `shift` (a socket is made for it if it sat on a built-in one, as in Phase 2) |
| Dragging the part itself | Slides it the same way; dropped on another part's dot, it joins there | `shift`, or `to` |
| Rotate: one ring around the join | Twists it | `twist`, in steps of 15 degrees |
| Scale | The same as for a free part | `scale` |

A joined part can't be lifted off its surface: that is what joined
means. To move it freely, **Unjoin** it (right-click). It then stays
exactly where it is, as a free part.

**Size handles** (step 2), on a part that is one shape:

| Shape | Knobs | Field |
|---|---|---|
| box, wedge, pyramid, prism, stairs, arch | One on each of the six sides | `size`, one number |
| cylinder, cone, capsule, tube | Top and bottom; one on the side | `height`; `radius` |
| sphere | One | `radius` |
| torus | One on the outside; one on the tube | `radius`; `tube` |

Pulling a face moves that face and leaves the opposite one where it is.
(For a free part that means `size` and `move` change together, as one
step.) Sizes snap like moves.

**Snapping.** Snapping is on. Hold **Ctrl** while dragging to turn it
off. The step in use is written at the bottom of the viewport: "Snap
0.1 m, 15°".

**While you drag**, the part moves under the pointer, and a small label
shows the number ("x 1.2 m", "45°"). Nothing is written to the file
until you let go: what you see is what the recipe *would* build, the
same thing `--dry-run` shows an agent. Letting go writes once, as one
undo step. Escape during a drag drops it and writes nothing.

**Exact numbers.** The properties panel gets the fields the handles
write: Position, Rotation and Scale for a free part, beside what it
already has. Typing a number is the same edit as dragging.

### New edit commands

```bash
duck-mold edit model.toml add crate --shape box --move 1,0.5,0
duck-mold edit model.toml set crate --move 2,0.5,0 --rotate 0,45,0
duck-mold edit model.toml set crate --scale 1,2,1
duck-mold edit model.toml set crate --bevel 0.05 --taper 0.8
duck-mold edit model.toml unjoin hat
duck-mold edit model.toml set crate --face-color top=none
duck-mold edit model.toml remove-color gold --replace-with red
duck-mold edit model.toml rename-socket body.spot badge
duck-mold edit model.toml remove-socket body.spot_2
duck-mold edit model.toml tidy-sockets
duck-mold new castle.toml --example gate
```

| Command | What it does | Step |
|---|---|---|
| `set <part> --move x,y,z` | Puts a free part's middle there. Refused for a joined part, with what to do instead (slide its socket, or unjoin it). | 1 |
| `set <part> --rotate x,y,z` | Turns a free part: degrees around x, then y, then z. For a joined part the message says to use `--twist`. | 1 |
| `set <part> --scale n` or `x,y,z` | Makes any part bigger or smaller, or stretches it. | 1 |
| `unjoin <part>` | Makes a joined part free, exactly where it stands: writes its `move` and `rotate`, and takes away `attach`, `to` and `twist`. | 1 |
| `add <name> --shape box` with no `--to` | Adds a free part. `--move`, `--rotate`, `--scale` place it; without `--move` it stands on the ground in the middle. | 3 |
| `set <part> --tube`, `--wall`, `--steps` | The new shapes' own sizes. | 4 |
| `set <part> --bevel`, `--bevel-steps`, `--taper` | The two new options. | 4 |
| `set <part> --face-color top=none` | Takes a face's own colour away again. | 7 |
| `remove-color <name>` | Removes a palette colour. Refused if anything is painted with it, naming what; `--replace-with <other>` repaints those first. | 7 |
| `rename-socket <part.socket> <new name>` | Renames a socket, and every `to` and `from` that names it. | 8 |
| `remove-socket <part.socket>` | Removes a declared socket. Refused if a part sits on it, naming the part. | 8 |
| `tidy-sockets` | Removes the sockets the Studio made (`spot`, `spot_2`...) that nothing sits on. | 8 |
| `new <recipe> --example <name>` | Starts a recipe as a copy of an example. | 9 |

All of them follow Phase 2's rules: the same report as `inspect`,
`--json`, `--dry-run`, refused if the recipe would have more mistakes
afterwards, and a line each in the agent guide (a test fails if one is
missing).

Edits that already exist and get a new button or key: `duplicate`,
`duplicate --mirror`, `remove`, `rename`, `join`, `add-color`,
`set-color`, `set --face-color`.

**Not edit commands**, because they change how you look, not the model:
the tools, the camera views, wireframe, the shadow, hiding a part (see
the decisions), and the tour.

### New shapes

Seven, written like the five there are. All are centred on their own
middle, and all can be coloured face by face.

| `shape` | Fields (defaults) | What it is | Face names |
|---|---|---|---|
| `capsule` | `radius` (0.25), `height` (1), `segments` (16) | A cylinder with a round cap on each end. `height` is the whole length, caps included. | `side`, `top`, `bottom` |
| `torus` | `radius` (0.5), `tube` (0.15), `segments` (16) | A ring lying flat. `radius` is from the middle of the hole to the middle of the tube; `tube` is how thick the tube is, as a radius. | `side` |
| `tube` | `radius` (0.5), `wall` (0.1), `height` (1), `segments` (16) | A pipe standing up, open at both ends. `wall` is how thick it is. | `side`, `inside`, `top`, `bottom` |
| `pyramid` | `size` ([1, 1, 1]) | Four slopes from a rectangle up to a point. | `front`, `back`, `left`, `right`, `sides`, `bottom` |
| `prism` | `size` ([1, 1, 1]) | A roof: two slopes that meet in a ridge running front to back. | `left`, `right`, `front`, `back`, `bottom` |
| `stairs` | `size` ([1, 1, 1]), `steps` (4) | Steps that climb from the front to the back, like the wedge. | `top` (every tread), `front` (every riser), `back`, `left`, `right`, `bottom` |
| `arch` | `size` ([1, 1, 1]), `wall` (0.2), `segments` (8) | A wall with a round-topped doorway through it, front to back. `wall` is how thick the two legs are, and the stone over the top of the curve. | `front`, `back`, `left`, `right`, `top`, `bottom`, `inside` |

```toml
[[parts]]
name = "handle"
shape = "torus"
radius = 0.3
tube = 0.05
color = "steel"

[[parts]]
name = "steps"
shape = "stairs"
size = [1.2, 0.8, 1.6]
steps = 5
colors = { top = "plank", front = "dark" }
```

(A prism with more than three sides is already there: a cylinder with
`segments = 6`.)

**Their sockets sit on the surface**, by the rule there is: a socket
starts on the box around the shape and moves straight in until it
touches. Three places need saying:

- A torus's and a tube's `top` and `bottom` are in the middle of the
  opening, level with the rim: a lid sits there. To sit on the rim, use
  a socket with `on = "top"` and a `shift`.
- An arch's `bottom` is between its feet, at ground level, and its
  `front` and `back` are in the doorway's mouth: a door joined to
  `arch.front` stands in the doorway.
- A pyramid's `top` is its point; a prism's is the middle of its ridge.
  Their side sockets lie on the slopes and face out of them.

### Two options: bevel and taper

| Field | On | What it does | Range |
|---|---|---|---|
| `bevel = 0.05` | box, cylinder | Cuts that many metres off every edge, so they aren't sharp. | 0 to just under half the smallest side |
| `bevel_steps = 1` | with `bevel` | 1 is one flat cut. 2 to 4 make the edge rounder, with more triangles. | 1 to 4 |
| `taper = 0.5` | box, cylinder, tube | The top is half as wide as the bottom. 1 is straight; above 1 it widens. Two numbers, `[0.5, 1]`, taper width and depth apart. | 0.01 to 10 |

```toml
[[parts]]
name = "tower"
shape = "cylinder"
radius = 0.6
height = 2.4
taper = 0.8
bevel = 0.04
```

- The cut faces are called `bevel`, to colour them
  (`colors = { bevel = "trim" }`). Without that they have the part's
  colour.
- A tapered side leans, and its sockets lean with it, sitting on it.
- A box with `taper = 0.01` is nearly a pyramid; that is fine.

**Nothing changes in the `.glb` or in Duck Forge.** A new shape is only
more triangles, and the reader there already reads those. The recipe
format's `version` stays 0: these are additions, and every recipe there
is still builds the same, byte for byte. The golden files prove it.

**Seven new examples**, one for each shape, each a small model rather
than the bare shape: a dog (capsules), a doughnut (torus), a mug (tube),
a temple (pyramid), a barn (prism), a playground slide (stairs), a
castle gate (arch, with tapered and bevelled towers). Each gets the
stored model, report and picture the other examples have.

### The top bar and the menus

```text
+--------------------------------------------------------------------------------+
| (duck) robot.toml | File  Edit  View  Help |  Undo Redo | Add v |               |
|        Front Side Top 3/4 | Frame | Grid Wire Shadow | Export  Render picture   |
+--+----------+--------------------------------------------------+---------------+
|  | PARTS    |  [G] Move                                        | PROPERTIES    |
|  |  body    |  [R] Rotate                                      |  name  head   |
|  |  head  o |  [S] Scale        the viewport                   |  position ... |
|  |  hat   o |  [T] Size                                        |  size  ...    |
|  |          |  [P] Paint                                       |  colour ...   |
|  +----------+                                                  +---------------+
|  | LIBRARY  |                                                  | SOCKETS       |
|  | [] []    |                                                  |  neck  [x]    |
|  |          |          Snap 0.1 m, 15°  (Ctrl: off)            +---------------+
|  |          |                                                  | PALETTE       |
|  |          |                                                  |  [] steel  +  |
+--+----------+--------------------------------------------------+---------------+
| PROBLEMS  None                                       Moved "hat" to 1, 0.5, 0   |
+--------------------------------------------------------------------------------+
```

(The bar is one row on a wide window; it wraps to two on a narrow one.
The `o` beside a part in the list is its eye: click it to hide the
part.)

| Menu | What is in it |
|---|---|
| **File** | New model, Open, Open an example, Opened lately, Export, Render picture, Back to the start page |
| **Edit** | Undo, Redo, Duplicate, Mirror copy, Remove, Rename, Unjoin, Tidy sockets, Select all |
| **View** | Front, Side, Top, Three-quarter, Frame, Grid, Wireframe, Shadow, Show everything hidden |
| **Help** | Show the tips again, The keys (this list), About |
| **Add** | The twelve shapes with a small picture each, then the recipe's defined parts, then the library. The new part stands on the ground where the middle of your view meets it, joined to nothing, selected, with the Move tool on. |

Every menu line shows its key, so the menus teach the keys.

### The keys

| Key | Does |
|---|---|
| G, R, S, T, P | Move, Rotate, Scale, Size, Paint tool |
| Delete or Backspace | Remove the selected parts |
| Ctrl+D | Duplicate |
| Ctrl+Z; Ctrl+Y or Ctrl+Shift+Z | Undo; redo |
| F | Frame: bring what is selected (or everything) into view |
| 1, 2, 3, 4 | Front, side, top, three-quarter view |
| Z | Wireframe on and off |
| H; Shift+H | Hide what is selected; show everything |
| F2 | Rename |
| Shift+A | Open the Add menu |
| Shift+click | Add a part to the selection, or take it out |
| Ctrl+A | Select every part |
| Ctrl held while dragging | No snapping |
| Escape | Lets go: of a drag, then of a menu, then of the selection |
| Ctrl+N, Ctrl+O, Ctrl+E | New model, Open, Export |

Keys don't fire while you type in a field. The mouse is as before: drag
on empty space turns the view, right-drag (or Shift+drag, or
middle-drag) slides it, the wheel zooms.

### Right-click, several parts, hiding (step 5)

**Right-click a part**, in the picture or in the list:

```text
  Duplicate            Ctrl+D
  Mirror copy
  Rename               F2
  Remove               Delete
  ---------------------------
  Join to            > body > top, front, neck ...
  Unjoin
  Add a part here    > Box, Cylinder ...
  ---------------------------
  Frame                F
  Hide                 H
```

**Several parts.** Shift+click adds to the selection. With several
selected:

- Move moves them together. Rotate and Scale turn and size them around
  the middle of the group.
- A joined part in the group goes where the part it is joined to goes.
  If that part isn't in the group, it stays, and the status line says
  which stayed.
- The properties panel shows "3 parts" and what can be set for all:
  colour, and the Duplicate, Remove and Hide buttons.
- It is one undo step. In the file it is the same edit made once for
  each part; an agent runs the command once for each.

### Views and look (step 6)

- **Front, Side, Top, Three-quarter** turn the camera; the model stays
  in view.
- **Wireframe** draws every face's outline and doesn't fill it, so you
  can see through.
- **Shadow** darkens the ground softly under the model, straight down,
  so you can see what touches the ground and what hangs over it. It is
  in the Studio only: `render`'s pictures don't change.
- All are drawn by the CPU renderer, as lines and triangles.

### Colour (step 7)

- **A palette panel** lists the recipe's colours as squares with names.
  `+` adds one, a click on a square changes it (everything painted with
  it changes), and the bin removes it. If it is in use, the Studio asks
  which colour to repaint with.
- **The Paint tool**: pick a colour in the panel, click a face. A face
  here is one the recipe can name: a box's `top`, a cylinder's `side`,
  a file part's material. The face lights up under the pointer first.
  Shift+click paints the whole part.
- A single triangle can't be painted yet. That needs Phase 4.

### Sockets (step 8)

- The properties panel lists the selected part's declared sockets, each
  with rename and remove.
- When a Studio action leaves one of the sockets it made (`spot`,
  `spot_2`...) with nothing on it, it removes it in the same undo step.
  Sockets you named yourself are never removed for you.

### The start page (step 9)

```text
+--------------------------------------------------------------------------+
|   (duck)   Duck Mold Studio                                       0.3.0  |
|            Build 3D models from parts, for Duck Forge games.             |
|                                                                          |
|   [ New model ]   [ Open a model ]                                       |
|                                                                          |
|   Opened lately                                                          |
|   +--------+  +--------+  +--------+                                     |
|   | (pic)  |  | (pic)  |  | (pic)  |                                     |
|   +--------+  +--------+  +--------+                                     |
|   castle      my_robot    crate                                          |
|                                                                          |
|   Examples: click one to open a copy of your own                         |
|   +------+ +------+ +------+ +------+ +------+ +------+                  |
|   |robot | |house | |snow- | |desk  | |duck  | |dog   |                  |
|   |      | |      | | man  | |      | |      | |      |                  |
|   +------+ +------+ +------+ +------+ +------+ +------+                  |
|   +------+ +------+ +------+ +------+ +------+ +------+                  |
|   |dough-| |mug   | |temple| |barn  | |slide | |gate  |                  |
|   | nut  | |      | |      | |      | |      | |      |                  |
|   +------+ +------+ +------+ +------+ +------+ +------+                  |
|                                                                          |
|   How it works                                                           |
|   1. A model is made of parts: boxes, balls, tubes, arches...            |
|   2. Parts stand free, or join at dots, so they stay together.           |
|   3. Everything you do is written to a recipe file as you go.            |
|      Export turns it into the model file for your game.                  |
+--------------------------------------------------------------------------+
```

- The examples are inside the program, so the gallery is always there,
  wherever the program was started from.
- A click copies the example into `Documents\Duck Mold\` (as
  `robot.toml`, or `robot_2.toml` if that is taken) and opens the copy.
  The example itself can't be spoiled.
- Every picture is drawn by the same code as `render`. A recent model
  that no longer builds shows a plain tile with its name.

### The first-run tour (step 10)

Five small tips, shown once, on the first model you open. Each points
at the thing it is about, with **Next** and **Skip the tips**.

1. On the picture: "Drag to look around. Roll the wheel to come
   closer."
2. On a part: "Click a part to select it."
3. On the arrows: "Drag an arrow to move it. R turns it, S sizes it."
4. On a dot: "Dots are sockets. Click one to add a part that stays
   joined there."
5. On Add: "Add puts a new shape on the ground."

Help > Show the tips again brings them back. That they were seen is
kept in the Studio's settings file, with the recent list.

### How this leaves room for Phase 4

Phase 4 is moving single corners, edges and faces. Phase 3 is built so
that fits in, not so it has to be rebuilt:

- **The handles don't know what a part is.** Arrows, rings and boxes
  act on "a thing with a middle and three directions". In Phase 3 that
  thing is a part or a group of parts. In Phase 4 it can be a corner,
  an edge or a face, with the same handles and the same snapping.
- **A click already knows more than the part.** From this phase it
  knows the face, by name, and the nearest edge and corner. The Paint
  tool uses the face; Phase 4 uses the rest.
- **Every shape's corners and faces come in a fixed, tested order.**
  That is what will let a recipe say "this corner, moved this far" in
  Phase 4 and mean the same corner on every computer.
- **Shapes are changed in fixed stages**: the shape, then bevel, then
  taper, then scale and mirror, then its place. Phase 4's corner moves
  are one more stage, between taper and scale.
- **The tool strip has room** for a "whole part / corner / edge / face"
  switch above it.
- **Dragging shows a preview and writes once.** That works for any
  edit, so Phase 4's drags get it for nothing.

What Phase 4 will have to decide, and this phase doesn't: how a moved
corner is written in a recipe.

### Testing

- **Each new edit command**: on every example, in TOML and JSON, with
  only the expected lines changed.
- **Each new shape and option**: its triangle count, that it is closed
  (no holes), its sockets on its surface, and a stored picture. The old
  golden files must not change by one byte.
- **Handles without a screen**: tests press on a handle at its place in
  the picture, move the pointer, and let go, then read the recipe file.
  With snapping and without. For free parts, joined parts and groups.
  Escape writes nothing. One drag is one undo step.
- **Stored pictures of the viewport**, byte for byte on both systems:
  each tool's handles, the grid, wireframe, the shadow, each view.
- **Stored pictures of every new screen** (Linux, as before): the start
  page, the top bar with each menu open, the right-click menu, several
  parts selected, the palette panel, the sockets list, each tip of the
  tour.
- **The real screens clicked** on Windows and Linux: every menu line,
  every key in the list above.

### What still won't be in the Studio after this phase

Corners, edges and faces; extrude; sculpting; booleans; bones;
animation; UVs; textures (all by the brief). Also: making a definition
from a part, changing the order of parts, sockets written with `at` or
`from`, and docking inside Duck Forge's Studio.

### The decisions

Eight places where a choice was made. All eight were approved as
recommended.

1. **Hiding a part is like turning the camera: it isn't saved in the
   recipe.** A hidden part is still in the model and still exported.
   The bar says "2 hidden" so it isn't forgotten. *The other way:* a
   `hidden` field in the recipe, which would leave the part out of the
   exported model too. I think that is a trap: hide a wall to look
   inside, export, and the game has no wall.
2. **The ground stays under the lowest part**, so the only part of a
   model can't be lifted into the air (the Studio says why). *The other
   way:* new models get a fixed zero point, parts can float or sink,
   and a model can be exported hanging above the floor or half in it. I
   recommend keeping the promise that a model always stands on the
   ground.
3. **A click on the selected part no longer opens the "add here"
   list.** In Phase 2 it did. With handles, a click on the part is the
   start of a drag, and the list got in the way. "Add a part here"
   moves to the right-click menu. The dots work as before.
4. **`prism` is the roof shape, and `arch` is a wall with a doorway.**
   A prism could also mean a bar with any number of sides, and an arch
   a curved band standing free. Say if you meant those.
5. **Bevel is one flat cut unless `bevel_steps` asks for more.** Round
   edges cost triangles, and a game model should be light by default.
6. **Scale and Size are two tools**, as the brief lists them. Scale
   works on anything and writes `scale`; Size pulls one face of a shape
   and writes its own size. If two feels like one too many once you try
   it, they can share a button.
7. **Example copies go to `Documents\Duck Mold\`.**
8. **A zip to try after step 3.** How handles feel is the point of this
   phase, and only a person at a PC can judge it. After steps 1 to 3
   (handles, size handles, the Add menu) I will have GitHub build the
   Windows zip from the branch and give you the link, and keep building
   unless you tell me to stop.

The version becomes 0.3.0.

## 20. Phase 4: shaping by hand

**Status: approved**, with all twelve decisions at the end as
recommended.

The first complaint about the Studio was "I can't form it in any way".
Phase 3 answered half of it: a part can now be moved, turned, resized,
bevelled and tapered. But a box is still a box. Phase 4 is about
changing the shape itself: pulling a face out to make a leg, insetting
a face to make a window, moving a corner to make a rock.

The four rules stay:

1. The recipe is the model. The Studio keeps no model of its own.
2. Every Studio action that changes the model is an edit command an
   agent can run too.
3. An edit changes only what it has to in the file.
4. The viewport is Duck Forge's CPU renderer.

### The idea: a part keeps its shape, and a list of changes follows it

A shaped part is still written as a box with a size. Under it comes a
list called `changes`. The box is built first; then the changes are
made to it, one after the other, in the order they are written.

```toml
[[parts]]
name = "wall"
shape = "box"
size = [3, 2.4, 0.2]
color = "plaster"
changes = [
  { inset = "front", by = 0.6, name = "window" },
  { inset = "window", by = 0.06, name = "glass" },
  { pull = "glass", by = -0.08 },
]
colors = { window = "wood", glass = "sky" }
```

Read aloud: "a wall, 3 m by 2.4 m by 20 cm. Inset its front by 60 cm and
call the new face in the middle `window`. Inset the window by 6 cm and
call the face in the middle of that `glass`; what is left of `window`
is now a frame 6 cm wide. Push the glass 8 cm back into the wall."

Why this way, and not by writing every corner's position out:

- **The part stays a box you can resize.** Make the wall 4 m wide and
  the window is still 60 cm in from each side, with a 6 cm frame.
- **An agent, and a person, can read what was done**, and change one
  number of it.
- **An edit is still a small change to the file**: one line added to
  the list, or one number changed.

The same in JSON:

```json
{
  "name": "wall",
  "shape": "box",
  "size": [3, 2.4, 0.2],
  "changes": [
    { "inset": "front", "by": 0.6, "name": "window" },
    { "pull": "glass", "by": -0.08 }
  ]
}
```

### The changes

| Change | Written | What it does | Step |
|---|---|---|---|
| **Pull** | `{ pull = "top", by = 0.5 }` | Pulls a face out along its own direction, by 0.5 m, and fills in the gap with new side walls (Blender calls this "extrude"). A negative `by` pushes it in, making a hollow. | 1 |
| **Inset** | `{ inset = "front", by = 0.1 }` | Makes a smaller face inside the face, 0.1 m in from every edge, with a ring of faces around it. The shape doesn't change yet: there are only more faces to work with. | 1 |
| **Move** (slide) | `{ move = "top", by = [0, 0, 0.2] }` | Moves a face by x, y, z metres, and the faces round it stretch to follow. No new faces. | 3 |
| **Turn** | `{ turn = "top", by = [0, 15, 0] }` | Turns a face about its own middle: degrees around x, then y, then z. | 3 |
| **Scale** | `{ scale = "top", by = 0.5 }` | Makes a face bigger or smaller about its own middle: one number, or `[x, y, z]`. | 3 |
| **Move an edge** | `{ move = "top+front", by = [0, 0.1, 0] }` | Moves the edge where `top` meets `front`. | 4 |
| **Cut** | `{ cut = "right", at = 0.8 }` | Cuts all the way round the part, with a flat cut facing right, 0.8 of the way from the middle to the right side. Every face it crosses becomes two. Nothing moves: there are more faces, edges and corners to shape. | 4 |
| **Move a corner** | `{ move = "top+front+left", by = [0, 0.1, 0] }` | Moves the corner where `top`, `front` and `left` meet. | 5 |

- **Several at once.** Wherever one name goes, a list can go:
  `{ pull = ["top_left_back", "top_back", "top_right_back"], by = 0.5 }`
  pulls the three faces as one piece, with walls only round the
  outside of the three (not between them). The same for `move`,
  `turn`, `scale` and `inset`.
- **Amounts are metres; a cut's place is a share.** `by` is always in
  metres (and degrees for a turn), so a 6 cm frame stays 6 cm when the
  wall is resized. A cut's `at` is a share of the way from the middle
  (0) to the side (just under 1), like a socket's `shift`, so a cut at
  0.8 stays near the side when the part is resized.
- **Directions are the part's own.** `by = [0, 0, 0.2]` moves towards
  the part's front, whichever way the part is turned in the model.
  Changes are made in the shape's own measurements, before the part's
  `scale`, `mirror`, `rotate` and `move`, as its size is.
- **`name`** can be given to a pull, an inset or a cut, to choose the
  names of the faces it makes (below).

### How faces, edges and corners are named

**Never by a number of a triangle, and never by a measurement.** Every
name is made of words, and comes from the list of changes only.

**The box's own faces** keep the names they have always had: `top`,
`bottom`, `left`, `right`, `front`, `back`.

**An edge is named by the two faces that meet there**, joined with `+`:
`top+front` is the edge along the top of the front. **A corner is named
by three faces** (or more): `top+front+left`. The order doesn't matter:
`front+left+top` is the same corner. Moving or resizing never changes
which faces meet where, so these names stay put.

**Each change names the faces it makes**, by these rules:

| Change | The face it was done to | The new faces |
|---|---|---|
| **Pull** | Keeps its name: it is the end of what was pulled (the top of a chimney, the foot of a leg). | One wall along each edge, named `<face>_side_<word>`, where the word says which side of the face the wall is on: `top_side_front`, `top_side_left`... With `name = "leg"`: `leg_front`, `leg_left`... |
| **Inset** | Its name now stands for the ring that is left around the new face (the front of a wall, around its window). | The face in the middle: `<face>_inset`, or the `name` given (`window`). The ring's pieces each have a name too, `<middle>_rim_<word>`: `window_rim_top`, `window_rim_left`... |
| **Cut** | Keeps its name on the piece towards the middle of the part. | The slice towards the side gets the side's word: cutting `top` towards the right makes `top_right`. With `name = "rim"`: `top_rim`. |
| **Move, turn, scale** | Keeps its name. | None. |

**Where the words come from.** Every edge of every face knows which
side of its face it is on. A box's top has a front, a back, a left and
a right edge; its front has a top, a bottom, a left and a right edge.
A cut gives the new edges the word of the side they face; a pull's
walls and an inset's ring pieces pass the words on. So a word is never
worked out from which way something points (which a turn could
change); it is passed down from the box, change by change.

**A name can stand for several faces.** This is how `sides` already
stands for a box's four sides. The walls a pull makes along the same
side share a name (pull three faces in a row, and the three walls
along their front are all `backrest_front`), and the walls together
are named by the beginning: `backrest` is all the walls of the pull
named `backrest`. A name for several faces works everywhere a face
name does: to paint them, to pull them, in an edge name
(`backrest_front+top` is the three edges along the top of the back's
front).

**If a name is taken**, by a face an earlier change made, a number is
added: `top_right_2`. Given `name`s avoid that.

**A chair from one box**, to show the names in use:

```toml
[[parts]]
name = "chair"
shape = "box"
size = [0.5, 0.06, 0.5]             # the seat
color = "wood"
changes = [
  # A slice off each side of the seat. Where two slices cross, at
  # the corners, is where the legs go.
  { cut = "left", at = 0.8 },       # makes top_left, bottom_left, front_left, back_left
  { cut = "right", at = 0.8 },
  { cut = "front", at = 0.8 },      # makes top_front, top_left_front, top_right_front...
  { cut = "back", at = 0.8 },
  # Four legs, pulled down from the corners of the underside.
  { pull = "bottom_left_front", by = 0.42, name = "leg_lf" },
  { pull = "bottom_right_front", by = 0.42, name = "leg_rf" },
  { pull = "bottom_left_back", by = 0.42, name = "leg_lb" },
  { pull = "bottom_right_back", by = 0.42, name = "leg_rb" },
  # The back: the seat's back strip, pulled up as one piece.
  { pull = ["top_left_back", "top_back", "top_right_back"], by = 0.5, name = "backrest" },
]
colors = { top = "cushion" }
```

**To see a part's names**, `duck-mold inspect chair.toml --faces` lists
every face of every shaped part, with the way it faces, its size and
the faces next to it. The Studio shows the name of whatever is under
the pointer.

### When something changes underneath

| What changes | What happens to the changes |
|---|---|
| **The size** | They stay. Pulls, insets and moves keep their metres; cuts keep their share, so they move with the sides. Names don't change. |
| **The amount of an earlier change** (its `by` or `at`) | Later changes stay, and so do all names. |
| **`taper`** | Allowed: the changes are made to the tapered box. |
| **`bevel`** | A box with changes can't have a bevel, and a bevelled box can't have changes: the recipe says so, naming both. (A bevel adds dozens of small faces; giving each a name that lasts is for later.) |
| **The shape** (`set --shape sphere`) | Only a box can have changes, so they are dropped. The edit says so ("dropped 9 changes: only a box can be shaped"), and Undo brings them back. |
| **`segments`** | A box has none. (This is one reason why only boxes, below.) |
| **Adding, removing or reordering a change before others** | Names made by later changes can change, or vanish. Then a later change that names something no longer there is a mistake, which names it (below). |

**When a change no longer makes sense**, the recipe has a mistake, said
like every other mistake: the file, the line, which change, what is
wrong, and the likely fix. The model isn't built (the Studio keeps
showing the last one that worked), and an edit that would cause it is
refused and writes nothing.

```text
error: wall.toml:9: parts[0].changes[2]: there is no face "glas" on "wall". Did you mean "glass"?
error: wall.toml:8: parts[0].changes[1]: an inset of 0.8 m is too wide for "window": it is 1.2 m across, so the inset must be less than 0.6 m
error: chair.toml:14: parts[0].changes[6]: the cut towards "back" at 1.2 is outside the part: "at" goes from 0 (the middle) to just under 1 (the side)
error: wall.toml:10: parts[0].changes[2]: pulling "glass" in by 0.3 takes it through the other side of the part
error: rock.toml:12: parts[0].changes[5]: moving the corner "top+front+left" by [0, -0.9, 0] turns the shape inside out
```

**Which faces a cut goes through depends on where they are**: shrink a
part so that a cut misses a pulled leg, and the pieces it made of that
leg are gone. A later change that names one of them is then a mistake,
named as above.

### Which shapes can be shaped: boxes, in this phase

Only `box`, with or without `taper`, and without `bevel`. Why:

1. **Every face, edge and corner of a box has a name made of words**
   (`top`, `top+front`, `top+front+left`) that no size can change. A
   round shape's side is sixteen numbered pieces, and changing its
   `segments` to twelve would renumber them, so "the fourth piece of
   the side" would quietly become a different face.
2. **Every face of a box is a flat, four-sided face**, and a cut along
   the box's own directions keeps it so. That makes each change easy to
   check and easy to name.
3. **Everything the brief's examples need is a box**: a chair, a wall
   with a window, a rock, framed windows.

Asking to shape another shape says so: "only a box can be shaped in
this version: "post" is a cylinder". The flat shapes (wedge, pyramid,
prism) would be next; round shapes need another way of naming first.

### How a shape that is turned inside out, or through itself, is caught

After every change, four checks:

1. **Every face still has room**: none is squeezed to a line or a
   point.
2. **The shape still holds room inside it.** A shape turned inside out
   holds "less than nothing", and is caught here.
3. **No face passes through another face.** (Faces that touch along an
   edge or at a corner are neighbours, and fine.) A pull pushed through
   the far side of the part, or a corner dragged through a face, is
   caught here.
4. **The shape is still closed**: every edge is shared by exactly two
   faces. This can't fail by the rules above, but it is checked, so a
   mistake in Duck Mold itself can't make a broken model.

If one fails, that change is the mistake, as above. In the Studio the
drag shows the message while you hold it, and nothing is written when
you let go.

**A face that bends is fine.** Moving one corner of a flat face leaves
it bent. It is drawn as two triangles, folded the way that bulges out,
and keeps its name (as a tapered face already does).

### Inspect

`inspect` says how many changes a part has:

```text
  parts        1
    chair       box, 9 changes     118 triangles
```

`--json` adds `"changes": 9` to the part. `inspect --faces` lists the
faces of every shaped part:

```text
chair  (chair.toml): box, 9 changes, 30 faces
  top                 faces up      0.40 x 0.40 m   next to: top_left, top_right, top_front, top_back
  top_left            faces up      0.05 x 0.40 m   next to: top, left, top_left_front, top_left_back
  ...
  backrest_front      faces front   3 faces         next to: top, top_left_back, top_back, top_right_back ...
```

### New edit commands

```bash
duck-mold edit wall.toml inset wall front --by 0.6 --name window
duck-mold edit wall.toml pull wall glass --by -0.08
duck-mold edit chair.toml cut chair --toward left --at 0.8
duck-mold edit chair.toml pull chair top_left_back,top_back,top_right_back --by 0.5 --name backrest
duck-mold edit rock.toml move-corner rock top+front+left --by 0,0.1,-0.05
duck-mold edit chair.toml set-change chair 5 --by 0.45
duck-mold edit chair.toml remove-change chair 9
duck-mold save-part chair.toml chair parts/chair.toml
```

| Command | What it does | Step |
|---|---|---|
| `pull <part> <faces> --by <metres> [--name n]` | Adds a pull. Several faces are written with commas. | 1 |
| `inset <part> <faces> --by <metres> [--name n]` | Adds an inset. | 1 |
| `set-change <part> <number> --by ... [--at ...]` | Changes the amount of one change, counted from 1. | 2 |
| `remove-change <part> <number>` | Takes one change away (`--all`: every one). Refused if a later change, a colour or a socket names a face only it made, saying which. | 2 |
| `move-face <part> <faces> --by x,y,z` | Adds a move of a face. | 3 |
| `turn-face <part> <faces> --by x,y,z` | Adds a turn of a face. | 3 |
| `scale-face <part> <faces> --by n` or `x,y,z` | Adds a scale of a face. | 3 |
| `move-edge <part> <a+b> --by x,y,z` | Adds a move of an edge. | 4 |
| `cut <part> --toward <side> --at <share> [--name n]` | Adds a cut. | 4 |
| `move-corner <part> <a+b+c> --by x,y,z` | Adds a move of a corner. | 5 |
| `add-socket <part> <name> --on <any face name>` | A socket on a new face (`on` takes any face name of a shaped part). | 6 |
| `define <part> <name>` | Makes a definition from a part (its shape, size, changes, colours and sockets go to `[define.<name>]`) and the part uses it. | 7 |
| `set-socket <part.socket> --facing up` | Says which way a socket faces (`--facing out` takes it away again: out of the surface). Also on `add-socket`. | 8 |
| `duck-mold save-part <recipe> <part> <file>` | Writes the part alone, with its colours, as a recipe of its own: for the library. The recipe itself doesn't change, so this isn't an `edit`. | 7 |
| `duck-mold inspect <recipe> --faces` | Lists the faces of every shaped part. | 1 |

- **A move right after a move of the same thing adds to it**, instead
  of making a second line (and the same for turn and scale). Drag a
  corner three times and the recipe has one move of that corner.
  Pulls and insets are never added together: two pulls make two rings
  of walls, which is a different shape.
- **A part made from a definition** is shaped in the definition, as
  its size already is: every part made from it changes, and the edit
  names them.
- All follow Phase 2's rules: the same report as `inspect`, `--json`,
  `--dry-run`, refused if the recipe would have more mistakes
  afterwards, and a line each in the agent guide (a test fails if one
  is missing).

### The Studio: Face, Edge and Corner modes

Above the tool strip is a new row: **Part**, **Face**, **Edge**,
**Corner**. Part is what there is today. The other three shape the
selected part.

```text
+----------------------------------------------------------------------------------+
| chair.toml | File Edit View Help | Undo Redo | Add | Front Side Top 3/4 | Frame   |
+----------+-------------------------------------------------------+---------------+
| PARTS    | [Part] [FACE] [Edge] [Corner]          (Tab, E, C)     | FACE          |
|  chair   |                                                       |  top_back     |
|  table   | [T] Pull        ^                                     |  of "chair"   |
|          | [G] Move        |  the pull arrow                     |  faces up     |
|          | [R] Turn  +-----+-----+                               |  0.40 x 0.05  |
|          | [S] Scale |  top_back |  <- the selected face: yellow |               |
|          | [I] Inset +-----------+                               | CHANGES       |
|          | [K] Cut    the part's other faces outlined;           |  1 cut left   |
|          |            other parts faded                          |  2 cut right  |
|          |                                                       |  ...          |
|          |  Pull: drag the arrow.          Snap 0.1 m (Ctrl: off) |  9 pull 0.5 x |
+----------+-------------------------------------------------------+---------------+
| PROBLEMS  None                                      Pulled "top_back" up 0.5 m    |
+----------------------------------------------------------------------------------+
```

**Going in.** Select a box and press **Tab** (or click Face). The part
you are shaping is drawn with the outline of every face, so you can see
them; the other parts are drawn faded. The pointer lights up the face
under it, and the status line names it. A click selects a face;
Shift+click adds another. Tab again, or Escape with nothing selected,
goes back to Part.

**Face mode tools**, down the left as now:

| Tool | Key | Handle on the selected face | Writes | Snaps to |
|---|---|---|---|---|
| **Pull** | T | One arrow, along the face's own direction | `{ pull = ..., by = ... }` | 0.1 m, or 0.01 m zoomed in |
| **Move** | G | Three arrows: x, y, z | `{ move = ..., by = [...] }` | the same |
| **Turn** | R | Three rings | `{ turn = ..., by = [...] }` | 15 degrees |
| **Scale** | S | Three boxes and one in the middle | `{ scale = ..., by = ... }` | 0.1 |
| **Inset** | I | A knob on the middle of the face's nearest edge: drag it in | `{ inset = ..., by = ... }` | 0.01 m |
| **Cut** | K | A ring round the part follows the pointer, across the edge it is over; click to cut there | `{ cut = ..., at = ... }` | tenths of the way |

Face mode opens with Pull in hand: "an arrow pulls it along its own
direction". All of it works as Phase 3's handles do: while you drag,
the picture shows what the recipe would build and a label shows the
number; the file is written once when you let go, as one undo step;
Ctrl turns snapping off; Escape lets go without writing.

**Edge mode** (**E**): the part's edges are drawn as lines; the one
under the pointer is thick and yellow. Click to select one, Shift+click
for more. Move (G) gives three arrows at its middle. Cut (K) works
here too.

**Corner mode** (**C**): the part's corners are drawn as small dots.
Click to select, Shift+click for more, and Move (G) gives three arrows.

```text
  Edge mode                       Corner mode
     +---------+                     o---------o
    /         /|                    /         /|
   +=========+ |   <- the edge     o---------o |
   |         | +      under the    |         | o   <- corners are dots;
   |         |/       pointer      |         |/       the selected one
   +---------+                     o---------o        is yellow
```

**The panel in a shaping mode** shows what is selected (its name, the
way it faces, its size, and the part it is on) and, under it, the
part's **Changes**: one line each, in order, with its number field and
an x to take it away. Typing a number is `set-change`; the x is
`remove-change`. Changes made by a definition say so, as now.

**Things the modes refuse, and say why**: shaping a part that isn't a
box; Delete in a shaping mode ("faces can't be removed: a shape must
stay closed. To take a change back, Undo, or remove it from the
Changes list"); a drag that breaks a check (the label turns red and
says what is wrong; letting go writes nothing).

**Right-click** on a face, edge or corner, in a shaping mode, gives the
same tools as a menu, plus "Back to Part". In Part mode the part's menu
gains **Shape (Tab)**, **Make a definition...** and **Save to the
library**.

### The keys

New, for shaping:

| Key | Does |
|---|---|
| Tab | Face mode; again, back to Part |
| E | Edge mode |
| C | Corner mode |
| T | Pull (faces). In Part mode, T is still Size. |
| G, R, S | Move, Turn, Scale what is selected: a part, or faces, edges, corners |
| I | Inset (faces) |
| K | Cut |
| Shift+click | Add a face, edge or corner to the selection, or take it out |
| Ctrl held while dragging | No snapping |
| Escape | Lets go of a drag; then of the selection; then back to Part |

Kept from Phase 3, in every mode: Ctrl+Z, Ctrl+Y, F (frame what is
selected: a face, too), 1 to 4 (views), Z (wireframe), H and Shift+H,
Ctrl+N, Ctrl+O, Ctrl+E. Keys don't fire while you type in a field.
Help > The keys lists them all.

### New faces work like old ones (step 6)

- **Paint**: the Paint tool colours the face under the pointer by its
  name, including faces changes made (`colors = { glass = "sky" }`).
  Shift+click still paints the whole part.
- **Sockets**: `on` takes any face name of a shaped part:
  `seat = { on = "top" }`, `lamp = { on = "backrest_front", shift = [0, 0.4, 0] }`.
  The socket is at the face's middle (the middle of all of them, for a
  name that stands for several), facing out of it. `shift` works as on
  a side: shares of the face's own width, height and depth from its
  middle, with the number along the way it faces left at 0. The
  socket moves with the face when the face moves. A click on a new
  face in Part mode makes a socket there, as on any face today.
- The part's built-in sockets (`top`, `bottom`...) stay as they are: on
  the box around the part, landing on its surface.

### Using a shaped part again (step 7)

- **Make a definition**: right-click a part, "Make a definition...",
  give it a name. The part's shape, size, changes, colours and own
  sockets move to `[define.chair]`, and the part becomes
  `use = "chair"` where it stood. It is then in the Add menu, under
  "Defined in this recipe". The edit: `define <part> <name>`.
- **Save to the library**: right-click, "Save to the library". It
  writes `parts/chair.toml` next to the recipe: the part alone, with
  the colours it uses. It shows in the library at once, with its
  picture. The command: `duck-mold save-part`.

### Leftovers from Phase 3, and three fixes (step 8)

- **Pictures in the Add menu**: a small picture beside each shape,
  stored and tested like the examples' pictures.
- **A facing choice for sockets**: in the panel's Sockets list, each
  socket gets a list: out of the surface (as now), up, down, left,
  right, front, back. The edit: `set-socket --facing`.
- **Hidden parts are remembered**: the Studio keeps the hidden parts of
  each file in its settings (beside the list of models opened lately)
  and hides them again when the file is opened. The recipe still knows
  nothing of it (decision 1 of Phase 3 stands).
- **Copy a joined part to the next free socket**: see the fix below.

The three fixes:

1. **`duck-mold edit add` with no position put the part in the middle
   of the model, inside the other parts.** The Studio's Add menu walks
   from the middle towards the camera until the new part is clear of
   the others. That walk moves into the edit itself, so every `add`
   does it: towards the camera in the Studio, towards the model's front
   from the command line. `--stand x,z` and `--move` still put it
   exactly where they say.
2. **A copy of a joined part sat exactly on the original.** Now a copy
   goes to the next socket of the same part that is declared and has
   nothing on it (the next table leg's socket, say), in the order they
   are written. If there is none, a socket is made beside the
   original's, on the same face, one step past its width. If the face
   has no room for it, the copy stands free beside the model, as a copy
   of a free part does. The status line says which.
3. **A new free part was "floating" at once**, which reads like an
   error. A part that stands on the ground is no longer counted as
   floating: two things standing side by side on a floor are fine.
   Only a part up in the air that touches nothing floats. (No example's
   report changes: none has a floating part.)

*As built (step 8):* as above, with these differences. A copy goes
only to a free socket that faces the same way as the one it copies
(so a copied leg doesn't land on a socket meant for a lamp on the
top). With no room beside, the copy stands free in front of the model
(towards +z, the way `add` walks), not beside it. `add-socket` has no
`--facing`: add the socket, then `set-socket --facing`; the word `out`
takes `facing` away again. The socket row in the panel got narrower to
make room for the list ("x" removes a socket), so the panel keeps its
width. The hidden parts are kept in `hidden.txt` beside `recent.txt`.

### Examples (step 9)

Four, each with its stored model, report and picture:

- **`chair.toml`**: the chair above, made from one box by pulling
  faces.
- **`window_wall.toml`**: the wall above, with a framed window made by
  inset.
- **`rock.toml`**: a box cut through the middle three ways (so it has
  26 corners), with its corners moved in and out.
- **`house.toml`** gets framed windows: each window's front is inset
  to a frame and a pane, and the pane pushed back. Its stored files
  change on purpose; no other example's change by one byte.

*As built (step 9):* the four are as above. The chair is 108
triangles, not the 118 the design guessed: the cut faces under the legs
are pulled whole, and nothing is left over. The wall's frame is
painted by face name, and it has a socket on a face an inset made (the
strip above the window). The rock's corners are named as the cuts
name them (`top_right_front+front_right_top+right_top_front`); it moves
the eight outer corners in, the middles of five sides out and two top
edges in, and leaves the bottom flat. The house's two windows are one
`[define.window]`, used twice. The chair, the wall and the rock are on
the start page too, so it has fifteen examples.

### Nothing changes in the `.glb` or in Duck Forge

A shaped part is only more triangles, which the reader there already
reads. The recipe's `version` stays 0: `changes` is an addition, and
every recipe there is still builds exactly as before.

### Steps

In the order of the brief, one commit each:

1. Changes in the recipe and the library: pull and inset; the four
   checks; `inspect` reports them, `--faces`; `pull`, `inset`.
2. Face mode: Tab, selecting faces, the Pull arrow, the Inset knob, the
   Changes list; `set-change`, `remove-change`. Then a zip to try
   (decision 12).
3. Move, turn and scale a face.
4. Edges: Edge mode, moving an edge; cuts.
5. Corners: Corner mode, moving a corner.
6. New faces painted, and carrying sockets.
7. Make a definition; save to the library.
8. The Phase 3 leftovers and the three fixes.
9. The examples. Then PHASE4_REPORT.md.

### Testing

- **Every shape stays whole.** For each kind of change, and for
  thousands of random lists of changes (random faces, amounts, cuts,
  corner moves, in random orders), the result is either a closed shape
  that faces out everywhere, holds room and doesn't pass through
  itself, or a mistake that names the change that broke it. Never a
  broken model.
- **Names last.** The same changes on boxes of many sizes give the
  same names; changing an amount never changes a name.
- **Each change's names** are as written above, checked face by face
  on small cases.
- **Each new edit command**: in TOML and JSON, with only the expected
  lines changed, and a line in the agent guide.
- **Handles with no screen**: face, edge and corner drags pressed,
  moved and let go, then the recipe read; with snapping and without;
  one undo step each; Escape writes nothing; a drag that breaks a check
  writes nothing.
- **Stored pictures** of the viewport in each mode and with each tool,
  and of every new screen (each mode, the Changes list, the cut ring,
  a refused drag, the Add menu with pictures, the socket facing list).
- **Golden files** for the four examples; every other example byte for
  byte as it was.
- **The real screens clicked**, on Windows and Linux: every new key
  and menu line.

### What Phase 4 won't do

Sculpting, booleans (cutting one part out of another; a window that
goes right through is a later thing), bones, animation, UVs, textures
and docking in Duck Forge's Studio, as the brief says. Also: shaping
shapes other than boxes, a bevel on a shaped box, removing faces, and
reordering changes in the Studio (by hand in the recipe, it works).

### The decisions

Places where a choice was made. Each says what I recommend, and the
other way.

1. **Only boxes can be shaped, in this phase** (with `taper`, without
   `bevel`). *The other way:* the flat shapes too (wedge, pyramid,
   prism), which name well but need more checking; round shapes can't
   be named well yet.
2. **Names as above**: a pull's walls `<face>_side_<word>`, an inset's
   middle `<face>_inset`, a cut's slice `<face>_<side>`; edges and
   corners by the faces that meet there, joined with `+`; a `name` on a
   change chooses the new names. *The other way:* every new face gets a
   number (`face_7`), which is shorter but means nothing to a reader.
3. **Pull makes new walls** (Blender's extrude). Moving a face without
   new walls is Move. *The other way:* pull only moves the face, and
   extrude is a separate change.
4. **Amounts are metres; a cut's place is a share** from the middle to
   the side. *The other way:* cuts in metres too, which would keep a cut
   at the same distance from the middle when the part is resized, and
   could leave it outside a part made smaller.
5. **Changing a shaped box to another shape drops its changes**, saying
   so, and Undo brings them back. *The other way:* refuse, until the
   changes are removed first.
6. **The keys: Tab for faces, E for edges, C for corners**; T pulls, I
   insets, K cuts. *The other way:* Blender's 1, 2 and 3 for corners,
   edges and faces, which would take the keys the four views have now.
7. **A move right after a move of the same thing adds to it**, so
   dragging a corner several times leaves one line. *The other way:* a
   line for every drag, as Blender's history has.
8. **Hidden parts are remembered in the Studio's settings**, for each
   file, not in the recipe.
9. **The house example itself gets framed windows**, so its stored
   files change on purpose. *The other way:* a new example beside it,
   and the house left as it is.
10. **A copy of a joined part** goes to the next free declared socket;
    else to a new socket beside the original; else it stands free
    beside the model.
11. **A part that stands on the ground isn't floating.**
12. **A zip to try after step 2**, so you can feel Face mode early, as
    after Phase 3's step 3. I keep building unless you say stop.

The version becomes 0.4.0.

---

## 21. Phase 5: Motion

Phases 0 to 4 make models that stand still. Phase 5 makes them move: a
character that stands idle, walks, waves and jumps; a door that opens;
wheels that turn. Duck Forge plays these in a game.

The four rules hold. The recipe is the model, and now the motion too:
poses and animations are written in the recipe, in words. Every Studio
action is an edit command. An edit changes only what it must in the
file. The viewport is Duck Forge's CPU renderer. And Duck Forge's
loader stays in step: the same reader code in both repositories, a
raised version, shared test files, the handoff note updated.

Nothing in this section is built yet. It ends with the decisions I made
and, for each, the other way, so you can change any of them before I
start.

### In one picture

```toml
# A join may say how far it turns.
[[parts]]
name = "head"
attach = "bottom"
to = "body.neck"
limits = { x = [-30, 40], y = [-80, 80] }   # nods and turns; doesn't tilt

# A pose: which parts are turned, and how far, in degrees.
[poses.step_left]
left_leg  = [30, 0, 0]       # forward
right_leg = [-30, 0, 0]      # back
left_arm  = [-25, 0, 0]
right_arm = [25, 0, 0]

# An animation: poses at times, in seconds.
[animations.walk]
loop = true
keys = [
  { at = 0.0, pose = "step_left" },
  { at = 0.4, pose = "step_right" },
  { at = 0.8, pose = "step_left" },
]
```

Read aloud: "The head nods 30 degrees one way and 40 the other, and
turns 80 either way. In the pose `step_left` the left leg is 30 degrees
forward and the right leg 30 back, the arms the other way round.
Walking goes from `step_left` to `step_right` in 0.4 seconds, back in
another 0.4, and starts again."

### What a bone is: a join

There are no bones to place. **Every join is already a bone**: the
robot's head sits on `body.neck`, so turning that join turns the head,
and everything that hangs from the head (its eyes, its antenna) goes
with it. Every model made so far can be posed as it is.

The rules, all of them:

- **Every part can turn, about its pivot.** For a joined part the pivot
  is the socket it is joined to (the neck, the shoulder, the hinge).
  For a part that isn't joined, it is the part's own middle.
- **Turning a part carries what hangs from it**: the parts joined to
  it, the parts joined to those, and so on.
- **A part made of parts is a bone too.** The robot's `left_arm` is an
  arm made of an upper arm, an elbow, a forearm and a hand. Turning
  `left_arm` turns the whole arm at the shoulder. The parts inside are
  bones below it, named by their path: `left_arm/elbow` bends the arm
  at the elbow.
- **Parts turn rigidly.** Nothing bends or stretches (that is skinning,
  and not in this phase). An elbow is a ball between two boxes, as the
  robot's already is.

So the model is a tree: the body at the root, the head and the limbs
below it, the hand at the end of the arm. `duck-mold inspect --joins`
prints it:

```text
robot  (robot.toml): 20 joins
  body                    turns about its middle           free
    panel                 on body.chest                    fixed
    head                  on body.neck                     x -30..40, y -80..80
      left_eye            on head.left_eye                 free
      antenna             on head.antenna                  free
        antenna_tip       on antenna.top                   free
    left_arm              on body.left_arm                 x -60..180, z -170..10
      left_arm/upper      turns about its middle           free
        left_arm/elbow    on upper.bottom                  x 0..140
        ...
```

### Which way a turn goes

A turn is three numbers, `[x, y, z]`, in degrees, the same as `rotate`
has always been: around x, then y, then z.

| Number | Around | What it looks like on a character facing forward |
|---|---|---|
| x | the left-to-right direction | A nod. Positive swings a hanging arm or leg **forward**, and tips a head back. |
| y | the up direction | A turn. Positive turns a head to **its right**. A door swings around y. |
| z | the forward direction | A tilt. Positive leans the top to **its left**; it lifts the right arm out sideways (the left arm lifts with a negative z). |

**Whose x, y and z?** Those of the list the part is written in, as it
stands at rest. For a part of the model that is simply the model's own
x, y and z. For a part inside a definition (the elbow inside `arm`) it
is the definition's own, wherever that arm is put. Two things follow,
both wanted:

- When a parent turns, the child's directions turn with it. A forearm
  bends the same way whether the arm hangs down or points up.
- A mirrored arm mirrors its motion by itself: `"left_arm/elbow" = [40,
  0, 0]` and `"right_arm/elbow" = [40, 0, 0]` bend both elbows the same
  way, though the right arm is a mirror image.

You don't have to think about this in the Studio: the rings stand at
the join, the right way round.

### Limits: how far a join may turn

`limits` is written on the part, next to its join:

```toml
limits = { x = [-30, 40], y = [-80, 80] }   # a neck
limits = { x = [0, 140] }                   # a knee or an elbow: one way only
limits = { y = [0, 110] }                   # a door on its hinge
limits = { x = "free" }                     # a wheel on its axle: round and round
limits = "fixed"                            # never moves: a badge, a window
```

- **No `limits`**: the part may turn any way, any amount. That is what
  every existing recipe says, so every existing model can be posed.
- **With `limits`**: only the directions named may turn, each between
  its two numbers (the first is 0 or less, the second 0 or more, so
  that standing at rest is always allowed), or `"free"` for any amount.
  A direction not named doesn't turn.
- **`"fixed"`**: the part neither turns nor moves in any pose.
- Limits inside a definition go with it: both arms get the elbow's
  limit. A mirror copy of a part (`duplicate --mirror`) gets its limits
  mirrored.

**A pose that breaks a limit is a warning, not an error.** The model
still builds and exports, and `inspect` and the Studio's Problems list
say which pose turns which part how far past which limit. (An error
would stop the whole model from exporting because one pose is two
degrees over.) In the Studio a drag simply stops at the limit.

Limits are about turning. A pose may also move a part (below); that has
no limits in this phase, except that a `"fixed"` part can't.

*As built (step 2):* as above. Two things the design didn't say:
`limits = {}` names no direction, so the part doesn't turn but may still
be moved (a drawer); and the plain `inspect` report says a part's
limits beside it in the list of parts ("on body.neck, turns x -30..40,
y -80..80"), only for parts that have them, so no example's report
changes. `set-limits` takes `--x`, `--y` and `--z`, each two angles,
`free` or `none`; a direction not given stays as it is.

### Poses

```toml
[poses.wave]
left_arm = [0, 0, -150]                       # up and out to its side
"left_arm/elbow" = [0, 0, -30]                # a part inside a part: its path, in quotes
head = [0, -15, 0]

[poses.crouch]
body = { move = [0, -0.15, 0] }               # the whole robot sinks 15 cm
left_leg  = { turn = [40, 0, 0] }             # the long way of writing a turn
right_leg = [40, 0, 0]
```

- A pose has a name and a line for each part that isn't at rest. The
  line is the part's turn, or `{ turn = ..., move = ... }` when it also
  moves. `move` is in metres, in the same directions as the turn, from
  where the part stands at rest. Everything hanging from the part moves
  with it, so moving the body moves the whole robot: that is how it
  jumps.
- A part the pose doesn't name is at rest.
- **`rest`** is always there and can't be written: the model as the
  recipe builds it.
- A pose naming a part that doesn't exist is a mistake, with the
  closest name, like every other name in a recipe.
- Poses belong to the model: they are at the top of the recipe, not in
  a definition. (A recipe used as a part inside another recipe brings
  its limits, not its poses: see "What Phase 5 won't do".)

**No reaching (IK) in this phase.** A leg is posed by turning the hip
and the knee, not by putting the foot somewhere. So in a crouch the
feet are kept on the ground by hand: sink the body until they touch.
`inspect --pose` says how far the lowest point is above or below the
ground, so a person or an agent can get it right.

*As built (step 3):* as above, with these details. A turn's x, y and z
are those of the list the part is written in, as for limits, so a pose
of a mirrored arm uses the same numbers as its twin and comes out as
the mirror image. `mirror-pose` swaps `left` and `right` in the parts'
names and turns y and z the other way; for a part inside a mirrored
part the numbers stay as they are, because the mirroring is already in
the part. A move to one side goes to the other. `set-pose` takes
`--turn`, `--move` or both, and `--rest` takes the part's line out; a
pose with no lines left stays, as an empty pose. `add-pose` with no
`--from` is an empty pose. `remove-pose` has no `--with-keys` until
there are keys (step 4). A number in a turn may be from -36000 to
36000: a hundred times round. In `inspect --pose` the `ground` line and
the sockets of `--sockets` are of the posed model; a pose that leaves
the ground isn't warned about (a jump). The plain `inspect` report
lists the names of the poses on one line. `render --pose wave` writes
`robot.wave.png`.

### Animations

An animation is a list of **keys**: a pose, and the time it is reached.

```toml
[animations.wave]
keys = [
  { at = 0.0, pose = "rest" },
  { at = 0.3, pose = "wave" },
  { at = 0.6, pose = "wave_out" },
  { at = 0.9, pose = "wave" },
  { at = 1.3, pose = "rest" },
]

[animations.roll]            # a cart's wheels
loop = true
smooth = false
keys = [
  { at = 0, pose = "rest" },
  { at = 1, pose = "wheels_round" },   # each wheel = [360, 0, 0]: once round
]
```

- **`at`** is in seconds. The first key is at 0, and each key is later
  than the one before. The animation lasts until its last key.
- **Between two keys every number goes from one pose's to the next's.**
  A wheel at `[0, 0, 0]` and then `[360, 0, 0]` turns once round (the
  numbers are followed, so a full turn is a full turn, not "the same
  place").
- **`smooth`** (on, unless you write `smooth = false`): the motion
  flows through each pose without stopping there, and starts and ends
  gently. That suits arms, legs, doors. With `smooth = false` it goes
  straight from pose to pose at a steady speed: a wheel, a clock hand.
- **`loop`** (off, unless you write `loop = true`): it starts again
  after its last key. **A loop ends on the pose it starts with**, as
  `walk` does above. If it doesn't, the model would jump; that is a
  warning that says so. (A wheel at 360 is where it was at 0, so `roll`
  is fine.)
- The same pose can be used by many keys and many animations.

*As built (step 4):* as above, with these details. "Smooth" is a curve
through the keys that never goes past a key's value on its way to it
(so an animation whose poses are within the limits stays within them):
a number that keeps going the same way through a key passes it at a
speed between those of the two sides, and one that turns back stops
there. The two ends are at a stop, unless the animation loops: then the
end runs into the start, so a wheel turns at one speed all the way
round even with `smooth` on. An animation may have no keys (that is
what `add-animation` makes); it is a warning until it gets one. The
edit commands keep the first key at 0: `move-key` refuses to move key
1, and removing key 1 moves the others earlier. `move-key` past
another key changes places with it. `remove-pose --with-keys` is here
now. `inspect --animation` has its own report (not the model's with
lines added): length, loop, smooth, keys, what moves, how the lowest
point goes, and the worst floating, overlapping and broken limit of any
frame with its time. The strip is named `robot.walk.strip.png`, not
`robot.walk.png`, because a pose and an animation may share a name
(the robot's `wave`); each frame's time is in its corner, not under
it; and `--view` chooses where the frames are drawn from (a walk shows
best from the side). A loop's strip stops one step before its end,
which is its start again.

### What happens when things are renamed or removed

Poses name parts, and keys name poses. The edit commands keep them
right, as `rename` already does for joins:

| You do | Poses and animations |
|---|---|
| Rename a part (`rename head skull`) | Every pose line for it is renamed, and every line for a part inside it (`skull/jaw`). The same for a part inside a definition, in every part that uses the definition. |
| Remove a part | Its lines are removed from every pose, with the lines of what was removed with it. The summary says how many. |
| Duplicate a part | The copy starts at rest in every pose. |
| Join, unjoin, move a socket | Nothing to change: the part's pivot moves, and its turns now happen there. |
| Rename or remove a socket | Nothing to change: poses don't name sockets. (Removing a socket something is joined to is already refused.) |
| Make a definition of a part (`define`) | Nothing to change: the part keeps its name. |
| Rename a pose | Every key that uses it follows. |
| Remove a pose | Refused while a key uses it, naming the animations. `--with-keys` removes those keys too. |
| Change a shape so a part can no longer be found | Not possible: parts are found by name, never by number. |

A recipe changed by hand can still get it wrong; then it is a mistake
with the line and the closest name, as always.

### The new commands

All take `--json` and `--dry-run`, print the report after the edit, and
change only the lines they must.

**Limits**

```bash
duck-mold edit robot.toml set-limits head --x -30,40 --y -80,80
duck-mold edit cart.toml set-limits wheel_front_left --x free
duck-mold edit robot.toml set-limits panel --fixed
duck-mold edit robot.toml set-limits head --none        # no limits again
```

**Poses**

```bash
duck-mold edit robot.toml set-pose wave left_arm --turn 0,0,-150
duck-mold edit robot.toml set-pose crouch body --move 0,-0.15,0
duck-mold edit robot.toml set-pose wave head --rest      # the head is at rest in this pose again
duck-mold edit robot.toml add-pose wave_out --from wave  # a copy to change (without --from: empty)
duck-mold edit robot.toml rename-pose wave hello
duck-mold edit robot.toml remove-pose hello
duck-mold edit robot.toml mirror-pose step_left --as step_right
```

`set-pose` makes the pose if it isn't there. `mirror-pose` makes the
mirror image, left to right: what `left_arm` did, `right_arm` does, and
the other way round (names with `left` and `right` in them swap, as
`duplicate --mirror` already does), and a part in the middle (the head)
turns the other way. Without `--as` the pose itself is mirrored.

**Animations**

```bash
duck-mold edit robot.toml add-animation walk --loop
duck-mold edit robot.toml add-key walk step_left --at 0
duck-mold edit robot.toml add-key walk step_right --at 0.4
duck-mold edit robot.toml move-key walk 2 --at 0.5       # key 2, counted from 1 as inspect prints them
duck-mold edit robot.toml set-key walk 2 --pose crouch
duck-mold edit robot.toml remove-key walk 2
duck-mold edit robot.toml set-animation walk --loop false --smooth true --length 1.2
duck-mold edit robot.toml rename-animation walk stroll
duck-mold edit robot.toml remove-animation stroll
```

`--length` makes the whole animation faster or slower: every key's time
is scaled.

**Looking and checking, with no window**

```bash
duck-mold inspect robot.toml --joins              # the tree above
duck-mold inspect robot.toml --pose wave          # the usual report, of the model in that pose
duck-mold inspect robot.toml --animation walk     # every frame checked
duck-mold render robot.toml --pose wave           # the four-view picture, in that pose
duck-mold render robot.toml --animation walk      # a strip of frames: robot.walk.png
```

`inspect --pose wave` is the report you know, measured on the posed
model, plus what a pose can get wrong:

```text
robot, in the pose "wave"  (robot.toml)
  size         1.21 wide x 2.05 tall x 0.39 deep, in metres (x, y, z)
  ground       its lowest point is on the ground
  floating     none
  overlapping  left_arm/hand pokes 0.04 into head        (not so at rest)
  limits       head is turned 95 around y: its limit is -80 to 80
```

`inspect --animation walk` looks at every frame (30 a second) and says
the worst it found, and when:

```text
robot, "walk": 0.8 s, 3 keys, loops
  keys         1  0.0 s  step_left
               2  0.4 s  step_right
               3  0.8 s  step_left
  moves        left_arm, left_leg, right_arm, right_leg
  ground       its lowest point stays within 0.01 of the ground
  floating     none in any frame
  overlapping  left_arm/hand pokes 0.02 into left_leg/thigh at 0.2 s   (not so at rest)
  limits       none broken in any frame
  loop         ends as it starts
```

With `--json`, the same as data, for agents. The plain `inspect` report
gains one line each for joins with limits, poses and animations, only
when a recipe has them, so no existing example's report changes.

`render --animation` draws a strip: 8 frames (`--frames` for another
number), evenly spaced in time, side by side, all from the same place
and at the same size, each with its time under it. It is how an agent,
or a pull request, sees a motion.

### How the `.glb` changes

Today the file holds one object with one lump of triangles, and a note
of which triangles belong to which part. From Phase 5:

| | Before | From Phase 5 |
|---|---|---|
| Objects ("nodes") | One, the model. | The model, and under it **one for every part**, in a tree that follows the joins. An arm is under the body, the hand under the forearm. |
| Where a part's object is | - | **At its pivot**: the join it turns about. Its triangles are measured from there. Turning the object turns the part about its join, and everything under it follows. That is exactly what glTF means by an animated object, so other programs understand it too. |
| Triangles | One list. | One list per part, in the same order as before. Put together they are the old list. |
| Colours, directions, the palette, the part and assembly names, the sockets | As they are. | The same. Each socket also says which part carries it, so a hat stays on a nodding head. |
| Animations | None. | Each animation, as glTF animations (below). |
| Poses | None. | Each pose by name, so game code can set one. |
| The format's number (`extras.duck_mold.version`) | 0 | 1 |
| The reader's version | 3 | 4 |

**Models without animations load and look as before.** The reader puts
the parts back together at rest and gives Duck Forge the same mesh:
the same triangles in the same order, the same colours, the same part
ranges, the same sockets. One honest footnote: a corner's position is
now "its part's pivot plus its place in the part", added up on loading,
so it can differ from the old number in its last digit, far less than a
thousandth of a millimetre. Pictures don't change; a Duck Forge test
draws every example from an old file and a new one and compares them.

**Old and new together.**

- The new reader (version 4) reads both old files and new ones. Models
  already exported and sitting in a game keep loading.
- An old Duck Forge refuses a new file with a sentence, rather than
  drawing one part of it. So: update Duck Forge first (step 1 opens
  that pull request), then export again.
- Every example's stored `.glb` changes once, in step 1, on purpose.
  Their reports and pictures don't change by a byte.

**Animations in the file** are standard glTF animations: for each part
that moves, where it is turned to (and moved to) 30 times a second,
with straight steps between. Duck Mold works the smooth curve out; the
file holds the result. Why this way:

- Any glTF program plays them (Blender, the web viewers, the Windows 3D
  viewer), so you can check a file without Duck Forge, and the official
  validator still passes.
- Duck Forge's reader stays simple: it reads samples, it doesn't know
  about curves or limits.
- 30 a second is finer than the eye sees for these motions, and Duck
  Forge fills in between samples for a game running at 60.

The file also says, in Duck Mold's own corner of it, each animation's
name and whether it loops, and each named pose. A 1-second walk of the
robot adds about 10 KB.

*As built (step 1):* as above, with these details. A part's object is
named by its path (`left_arm/hand`). The objects are in the recipe's
order, a part made of parts before the parts inside it; an object's
children are the parts joined to it and, for a part made of parts, the
parts inside it that aren't joined. The four lists of numbers
(positions, directions, colours, triangle corners) are shared: each
part's lists point into them. The pivots are kept as 32-bit numbers and
each corner's distance from its pivot is chosen so that the reader's
sum gives the old number back wherever it can: measured on the eighteen
examples, about nineteen numbers in twenty are the same to the last
bit, the ground is at exactly 0 as before, and the rest differ by at
most a ten-thousandth of a millimetre. Two parts no longer share a
corner where they touch, so a few models have a handful more points
(the house 76 where it had 72). The robot's file grows from 50 KB to
64 KB. A socket passed on with `from` (an arm's `grip`, from its hand)
is carried by the part it came from. The reader is version 4 after this
step; animations (step 5) will make it version 5, because Duck Forge's
first pull request is opened before they exist.

*As built (step 5):* animations and poses are in the file as above.
Each animation's samples are at thirty a second from 0, and its very
end; all of an animation's parts share one list of times. A part gets
a list of turns only if the animation turns it, and a list of places
only if it moves it. An animation that moves nothing (every key at
rest) is named in Duck Mold's corner of the file with its length, and
isn't in glTF's own list, which has no way to write it; one with no
keys isn't written at all. A pose is written as an animation would
write one moment of it. Each part that isn't in the model's own list
(a part inside a part that is turned or a mirror image) also says the
three directions its turns are measured around, so that game code can
turn a join by degrees and get what a pose with the same numbers gets.
The format's number stays 1: nothing a version 4 reader reads has
changed, so it loads a file with animations as the same model,
standing still. The robot's three animations (4.3 seconds in all) and
six poses take its file from 64 KB to 78 KB. The official validator
reports nothing on it.

### Playing them in Duck Forge

A new type beside `Mesh` and `GlbModel`: **`AnimatedModel`**. It is a
loaded model that remembers what it is playing. The whole of it:

```rust
use duckforge::glb::AnimatedModel;

// in new(): load once
let mut robot = AnimatedModel::load("assets/robot.glb")?;
robot.play("idle");

// in update(): let time pass
if walking { robot.play("walk"); } else { robot.play("idle"); }
robot.update(ctx.dt());

// in draw(): it is a mesh, drawn like any other
canvas.draw_mesh(robot.mesh(), &Transform::at(self.position), &self.camera);
```

| | |
|---|---|
| `robot.play("walk")` | Plays it from its start. It loops if the recipe says `loop = true`; otherwise it stops on its last pose. Asking for the animation that is already playing changes nothing, so it is safe to call every frame, as above. |
| `robot.play_once("walk")`, `robot.play_looping("wave")` | The same, saying whether it loops instead of the recipe. |
| `robot.stop()` | Freezes it as it stands. `robot.resume()` carries on. |
| `robot.rest()` | Back to the rest pose, playing nothing. |
| `robot.set_speed(1.5)` | Faster or slower. 0 is still; a negative speed plays backwards (a door closing is `open` backwards). |
| `robot.set_pose("wave")` | Holds a pose from the recipe, playing nothing. |
| `robot.turn("head", vec3(0.0, 30.0, 0.0))` | Game code turns one join itself, in degrees, on top of whatever is playing: a head that looks at the player, a wheel that turns as fast as the cart drives. `robot.release("head")` gives the join back to the animation. |
| `robot.update(dt)` | Moves time on. Call it once in `update`. |
| `robot.mesh()` | The model as it stands now, to draw. |
| `robot.is_playing()`, `robot.finished()`, `robot.playing()` | Whether something is playing, whether a one-time animation has reached its end, and its name. |
| `robot.socket("left_arm/hand.bottom")` | Where a socket is **now**, and the way it faces: a sword stays in a waving hand, a hat on a nodding head. |
| `robot.animations()`, `robot.poses()` | The names in the file. |

- **Changing from one animation to another blends**, over 0.15 seconds,
  so a walking robot that stops doesn't snap to standing.
  `robot.set_blend(0.0)` turns that off.
- **A name that isn't in the file** doesn't crash a game: nothing
  changes, and Duck Forge says once, in its usual way, which name was
  asked for and which ones the file has.
- Time only moves in `update`, with the `dt` the game gives it, so a
  recorded playthrough plays back the same, as everything in Duck Forge
  does.
- `Mesh::load_glb` and `GlbModel::load` stay exactly as they are, for a
  model that only stands there.
- Each frame Duck Forge moves the corners of the parts that moved and
  draws one mesh, as now ("one draw instead of dozens").

Duck Forge gets an example, `examples/robot_walk/`: the robot idles;
the arrow keys walk it about; a key makes it wave; its hat stays on.
Duck Forge's tests play every animation of every example file, on the
CPU renderer, with stored pictures.

*As built (step 5):* as above, with these details. The reader is in
two halves. Where an animation has a join at a time, and where that
puts every corner and socket, is in the shared reader, so that Duck
Mold's tests can check it against Duck Mold's own posing: at every
sample of every animation, and in every pose, the two put every corner
of the test model in the same place to the last digits a file keeps
(a few millionths of a metre). Between two samples the reader goes
straight where Duck Mold follows its curve; they differ most where a
motion starts from a stop, by about a degree for a thirtieth of a
second. What is playing, speed, blending and the game's own turns are
`AnimatedModel`, Duck Forge's own. Added to the list: `restart()`
(play what is playing from its start again, since `play` of the same
name changes nothing), `time()`, `mesh_mut()` and `model()`. A join the
game turns is the game's until it is released: the animation's turn
for that join is replaced, not added to. Played backwards, an
animation starts from its end. A model at rest is, bit for bit, the
mesh `Mesh::load_glb` gives. Duck Forge's tests play the robot's three
animations frame by frame and check each against Duck Mold's report on
it (how far its lowest point leaves the ground), and compare fourteen
stored pictures pixel for pixel. Working out one frame of the robot
takes about 2.5 millionths of a second.

### Reusing animations: one walk for every character

An animation only names parts. So a walk made on the robot works on any
model whose parts have the same names. That is the "shared skeleton" of
the original plan: not a thing to build, just names that agree.

**The names for a two-legged character**: `body`, `head`, `left_arm`,
`right_arm`, `left_leg`, `right_leg`, each standing at rest with the
arms hanging down. (An elbow or a knee inside them is a bonus that
starter animations don't need.) The robot already has exactly these.

**Saving one.** `duck-mold save-animation robot.toml walk
animations/walk.toml` writes the animation and the poses it uses as a
small file of their own:

```toml
# Saved by `duck-mold save-animation` from robot.toml.
version = 0
name = "walk"

[poses.step_left]
left_leg = [30, 0, 0]
...

[animations.walk]
loop = true
keys = [ ... ]
```

**Using one.** `duck-mold edit duck.toml add-animation walk --from
animations/walk.toml` copies the animation and its poses into the
recipe. If the model lacks a part the animation names, the edit is
refused and lists them; `--skip-missing` takes the rest. A pose name
that is already taken by a different pose gets a number, as part names
do. Once copied in, it is the model's own: change a pose to suit
longer legs, and nothing else changes.

**In the Studio**, the library gets a second list, Animations: the
files in the folder `animations` beside the recipe, and the **starter
animations** that come inside the program:

| Starter | For | What it does |
|---|---|---|
| `idle` | two legs | Breathes: the body rises and sinks a little, the arms sway. Loops. |
| `walk` | two legs | Legs and arms swing against each other. Loops. In place: the game moves the character. |
| `wave` | two legs | Raises an arm, waves twice, lowers it. |
| `jump` | two legs | Crouches, rises 40 cm, lands, straightens. |
| `open` | a part called `door` | Swings 100 degrees on its hinge. Played backwards, it closes. |
| `roll` | parts called `wheel_front_left`, `wheel_front_right`, `wheel_back_left`, `wheel_back_right` | Each turns once round, steadily. Loops. |

### The Studio: Pose mode and the timeline

A fifth mode beside Part, Face, Edge and Corner: **Pose** (the key O).
It works on every model, not just boxes.

```text
+---------------------------------------------------------------------------------------------+
| robot.toml | File Edit View Help | Undo Redo | Add | Front Side Top 3/4 | Frame | Export    |
+------------+--------------------------------------------------------------+-----------------+
| Parts      | [Part][Face][Edge][Corner][Pose]                             | Poses           |
|  body      |                                                              |   rest          |
|  head      | [Turn]                     ___                               |   step_left     |
|  left_arm *| [Move]                    |o o|        (faint: the key       |   step_right    |
|   upper    |                         __|___|__       before and after)    | > wave          |
|   elbow    |                  \     |         |                           |   wave_out      |
|   forearm  |                   \    |   []    |                           | New  Copy       |
|   hand     |                 ( (O) )|         |                           | Mirror  Rename  |
|  right_arm |              rings at  |_________|                           | Remove          |
|  left_leg  |              the         ||   ||                             |-----------------|
|  right_leg |              shoulder    ||   ||                             | left_arm        |
|            |                         _||_ _||_                            |  Turn  0 0 -150 |
|            |                                                              |  Limits         |
|            | Drag a ring to turn the arm. It stops at its limit.          |   x  -60 .. 180 |
|            |                                              Snap 5 degrees  |   z -170 .. 10  |
|            |                                                              |  Back to rest   |
+------------+--------------------------------------------------------------+-----------------+
| Animation [wave v]  New   |>  Play   [x] Loop  [x] Smooth  [x] Ghosts      1.3 s    + Key   |
|  0.0       0.2       0.4       0.6       0.8       1.0       1.2                            |
|  <>rest         <>wave         <>wave_out     <>wave                    <>rest              |
|                  |                                                                          |
|                  ^ 0.30 s   (drag to move through time)                                     |
+---------------------------------------------------------------------------------------------+
| Problems  None                                         Turned "left_arm" in the pose "wave" |
+---------------------------------------------------------------------------------------------+
```

**Pose mode (step 6)**

- **The Poses list**, on the right, shows `rest` and every pose. Click
  one: the model stands in it, and it is the pose you are changing.
  New, Copy, Mirror, Rename and Remove are the commands above.
- **Click a part**, in the picture or the list. Rings appear at its
  join, one for each direction it may turn (a knee has one ring, a
  wheel one, a `fixed` part none, and says so). The parts list marks
  the parts this pose turns.
- **Drag a ring** to turn the part. What hangs from it follows as you
  drag. It snaps in steps of 5 degrees (Ctrl: free) and **stops at its
  limit**, and the label beside the pointer says the angle. Letting go
  is one `set-pose` edit and one step to undo.
- **Move** (G) gives arrows instead, to slide the part: a jump, a
  drawer.
- With `rest` selected a drag doesn't change the rest pose (that is the
  model itself, in Part mode). It asks for a name and starts a new
  pose.
- **The panel** shows the selected part's turn and move in this pose as
  numbers you can type, its limits (also to type: that is
  `set-limits`), and "Back to rest".
- A pose that breaks a limit, or that leaves a part floating, shows in
  Problems, as in `inspect --pose`.

*As built (step 6):* as above, with these details. The rings turn
exactly one of the pose's three numbers each: a turn is around x, then
y, then z, so the z ring turns around the part's z as its parent holds
it, the y ring around y once z has turned, and the x ring around x once
y and z have; the sign is found by trying a degree, so a mirrored part
turns the way the pointer goes too. A press on a ring is taken at the
ring's nearest point that is clear of the other rings. A drag in `rest`
doesn't ask for a name: it makes the pose `pose` (`pose_2`...) and says
so; Rename is on the right. Mirror on a pose named for one side makes
the other side's (`step_left` gives `step_right`); any other pose is
mirrored in place. Remove of a pose that keys use asks whether to take
the keys too (`--with-keys`). The panel's Limits fold in Part mode
(their words stay under the fold) and are open in Pose mode. Delete in
Pose mode doesn't remove the part: it says to use Back to rest, or go
back to Part mode. A pose that leaves most of the model away from what
stands on the ground (a foot pushed into it) is one line in Problems,
not a line for every part.

**The timeline (step 7)**, along the bottom in Pose mode:

- **Pick an animation**, or make one (New).
- **Keys** are the diamonds, each with its pose's name. Click one: time
  jumps there and its pose is selected, so turning a part now changes
  that pose. The panel says if other keys use the same pose, because
  they change with it.
- **Drag a key** left or right to change its time (it snaps to 0.05 s;
  it can't pass its neighbours). Right-click it for Remove and for
  another pose.
- **Drag in the ruler** to move through time; the model moves as you
  drag. Between two keys you see the in-between.
- **+ Key** puts a key at the time shown, with the selected pose.
  Turning a part between two keys asks first, then makes a new pose
  there (named after the animation: `wave_3`) with a key.
- **Play** (the space bar) plays it in the viewport, again and again if
  Loop is on. Loop and Smooth are the animation's own settings.
- **Ghosts**: the poses of the key before and the key after, drawn
  faintly, so you see where the motion comes from and goes to.
- The time ruler stretches with the window; a long animation scrolls.

Every one of these is an edit command from the list above. Selecting,
moving through time, playing and ghosts change nothing in the file.

*As built (step 7):* as above, with these details. A turn between two
keys doesn't ask: it makes the pose (`wave_7`, numbered by the keys
there are), holding every part the animation moves as it is at that
time (turns to a tenth of a degree) with the dragged part as dragged,
and a key with it, as one step to undo; the status line says so. A
turn at a key of `rest` makes a new pose (`wave_1`) and gives the key
that pose. Between two keys the list of poses highlights none. A
dragged key can't come within a hundredth of a second of its
neighbours, and key 1 stays at 0. New makes an animation with a first
key at 0 in `rest`, so it can be built on at once. The ghosts are
outlines of the parts the animation moves (and what hangs from them),
blue for the key before and orange for the key after; at a key, its
neighbours. They aren't drawn while it plays. The timeline fits the
whole animation to the window rather than scrolling. In Part mode a
model with animations has a Play button in the view's top right, with
the animation to play (the user's addition); playing in Part mode
shows the model moving, and taking hold of anything stops it.
Space plays and stops in either mode. Both the window and the engine
are tested: keys clicked and dragged with the mouse, the ruler
scrubbed, Loop, Space, Ghosts, and two screen pictures.

### Examples (step 9)

Each with its stored model, report, picture, and a frame strip of each
animation:

- **`robot.toml`** gets limits, and `idle`, `walk` and `wave`.
- **`dog.toml`** gets a `walk` on four legs, with its tail wagging.
- **`door.toml`** (new): a frame, a door on a hinge at its edge, a
  handle that turns; `open`.
- **`cart.toml`** (new): a box on four wheels with a handle; `roll`.

The robot's and the dog's stored files change on purpose. `door` and
`cart` join the start page.

### Steps

In the order of the brief, one commit each:

1. The `.glb` as a tree of parts; the reader, version 4, reading old
   and new files; Duck Forge's copy of it, with a pull request there.
   Every old model and test still passes.
2. `limits` in the recipe; `set-limits`; `inspect --joins`.
3. Poses; `set-pose`, `add-pose`, `rename-pose`, `remove-pose`,
   `mirror-pose`; `render --pose`, `inspect --pose`; renames and
   removals keep poses right.
4. Animations; `add-animation`, `set-animation`, `rename-animation`,
   `remove-animation`, `add-key`, `move-key`, `set-key`, `remove-key`;
   `render --animation`, `inspect --animation`.
5. Animations and poses in the `.glb`; `AnimatedModel` in Duck Forge
   and its `robot_walk` example, with a pull request there. The robot
   gets its three animations here, because the example needs them.
6. Pose mode in the Studio.
7. The timeline.
8. The animation library: `save-animation`, `add-animation --from`,
   the Animations list, the six starters.
9. The examples.

### One session, or two: 5a and 5b

This is the biggest phase so far. Phase 4 was nine steps in one
repository, and it filled a long session. Phase 5 is nine steps in two
repositories, with a file format change at the start and two new
Studio screens.

I recommend splitting it where a game can first use it:

- **5a: steps 1 to 5.** At the end, the robot idles, walks and waves in
  a Duck Forge example, made entirely by commands. Everything a game
  needs is there, and everything an agent needs. It ends with a short
  `PHASE5A_REPORT.md`, a frame strip of each of the robot's animations,
  and both Duck Forge pull requests open.
- **5b: steps 6 to 9.** Pose mode, the timeline, the library and
  starters, and the other examples. It ends with `PHASE5_REPORT.md`
  for the whole phase.

The split costs nothing: 5b only adds windows and files on top of
5a's commands. And it lets you try a walking robot in a game before
the Studio work starts, when the names and the Duck Forge calls are
still cheap to change. If you would rather have it in one go, say so
and I will carry straight on from step 5 to step 6.

*As built:* approved as two halves. 5a (steps 1 to 5) is built:
`PHASE5A_REPORT.md` has the frame strips, what works, the known limits
and what changed from this design. The version is 0.5.0. "What Duck
Mold draws is what Duck Forge plays" (in Testing, below) is checked on
the model, not on pictures: at every frame of every animation, the
robot as Duck Forge's player has it stands in the same box, with its
sockets in the same places, as Duck Mold poses it; and the shared
reader puts every corner of a test model where Duck Mold does. 5b
(steps 6 to 9) is to come.

### Testing

- **The tree**: every example, put back together at rest by the
  reader, has the same triangles, colours, parts and sockets as the old
  file gave. Old files still load. The glTF validator passes on files
  with animations.
- **The two readers**: byte for byte the same between the markers, as
  now, with the one-line check in the handoff note.
- **Limits, poses, animations**: every field and every message; every
  edit command, with a line in the agent guide (a test fails
  otherwise); renames and removals; `mirror-pose` done twice gives the
  pose back.
- **Posing never breaks a model**: after any pose, every part has the
  same shape and size as at rest (only where it is changes), and what
  hangs from a part is still joined to it. Checked on every example
  with random poses.
- **What Duck Mold draws is what Duck Forge plays**: for every
  animation of every example, frames from `render --animation` and
  from Duck Forge's `AnimatedModel` at the same times, compared.
- **Golden files**: the stored model, report, picture and frame strips
  of every example; the strips are compared byte for byte on Windows
  and Linux like the pictures.
- **Picture tests of every new screen**: Pose mode with rings, a drag
  stopped at a limit, the Poses list, the timeline with keys, a key
  being dragged, ghosts, the Animations list.
- **Duck Forge's own tests**: play every example animation; `play`,
  `stop`, speed, blending, `set_pose`, `turn`, sockets that follow; a
  recorded playthrough of `robot_walk` plays back the same.

### What Phase 5 won't do

By the brief: parts that bend (skinning), face shapes and expressions,
IK, physics, sound, UVs and textures. Also:

- **Poses don't come along with a recipe used as a part.** A wheel
  recipe with its own `spin`, used four times in a cart, brings its
  limits but not its animation; the cart animates its wheels itself.
- **No blending of two animations at once** (walking while waving).
  Game code can turn single joins on top of an animation with `turn`.
- **No events in animations** ("a footstep sound here").
- **Parts don't change size or colour** in a pose: only turn and move.
- **A wheel faster than 15 turns a second** can't be told from a
  slower one at 30 samples a second.
- **Duck Forge's Studio and scene files** don't place animated models
  yet; that is Duck Forge's own next step, from game code they work.

### The decisions

Places where a choice was made. Each says what I recommend, and the
other way.

1. **A bone is a join** (yours). A part made of parts is a bone too,
   and a part that isn't joined turns about its own middle.
2. **A turn's x, y and z are those of the list the part is written
   in.** So limits in a definition mean the same wherever it is used,
   and a mirrored arm mirrors its motion by itself. *The other way:*
   always the model's own x, y and z, which is simpler to say but makes
   a limit inside a turned or mirrored definition mean something
   different in each use.
3. **`limits` names the directions that may turn**, each with two
   numbers or `"free"`; no `limits` means free; `"fixed"` means never.
   **Breaking one is a warning.** *The other way:* an error, which
   stops the export of a model over one pose.
4. **Animations are poses at times**, the poses named (yours). *The
   other way:* each key holds its own numbers, which is how Blender
   stores it; shorter to make, but a walk's two steps couldn't share a
   pose, and nothing could be set by name from a game.
5. **A loop ends on the pose it starts with**, written out, and a
   warning says when it doesn't. *The other way:* a `length`, and the
   loop goes back to its first pose by itself; one key fewer, and a
   rule you have to know.
6. **`smooth` flows through the poses; `smooth = false` is steady.**
   One switch for the whole animation. *The other way:* a choice on
   every key, which is more control than a first game needs.
7. **In the `.glb`, each part is an object with its own triangles, at
   its pivot** (yours). Format version 1, reader version 4; old Duck
   Forge refuses new files; positions can differ in their last digit.
   *The other way:* keep the one lump of triangles exactly as it is and
   add glTF's "skin" on top. Old files and new would be the same to the
   last digit, but it is the machinery for parts that bend, used for
   parts that don't, and harder to read.
8. **Animations are stored as standard glTF, 30 samples a second**, and
   that is what Duck Forge plays. *The other way:* store Duck Mold's
   own keys and let Duck Forge work the curves out; smaller files and
   exact, but other programs would show a model standing still, and
   the reader would be three times the size.
9. **Duck Forge gets `AnimatedModel`**, with a short blend when the
   animation changes and `turn` for one join from game code. *The
   other way:* leave out the blend and `turn` to keep it smaller; a
   character would snap between animations, and a wheel couldn't follow
   the cart's speed.
10. **Using a saved animation copies it into the recipe.** *The other
    way:* the recipe points at the file, as it does for parts, so one
    fix reaches every model; but then changing a walk for one model
    changes it for all, or needs a "make it my own" step.
11. **Removing a pose that a key uses is refused**, unless
    `--with-keys`.
12. **The six names for a two-legged character**, and the six starter
    animations, as above.
13. **The keys: O for Pose mode, Space to play**; G and R for Move and
    Turn inside it, as everywhere.
14. **Two sessions, 5a and 5b**, split after step 5. *The other way:*
    one session, straight through.
15. **A zip to try** at the end of 5a (it has the commands, not yet the
    Studio screens) and again after step 6, when Pose mode can be
    felt. I keep building unless you say stop.

The version becomes 0.5.0.
