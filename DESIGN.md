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
| Layout | One scene, one object (named after the recipe), one mesh, one list of triangles. Every part is merged into it, as Duck Forge's guide recommends ("one draw instead of dozens"). |
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
