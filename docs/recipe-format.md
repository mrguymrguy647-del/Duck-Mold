# The recipe format, version 0

A recipe is a text file that fully describes one model. The same fields
can be written as TOML (`.toml`) or as JSON (`.json`); the file's ending
says which. This page lists every field, its default and its range.

New to recipes? Start with the [README](../README.md) or, for an AI
agent, the [guide](AI_GUIDE.md). The reasons behind the format are in
[DESIGN.md](../DESIGN.md).

Rules that hold everywhere:

- **A misspelt field is an error**, never silently ignored. The message
  names the line and suggests the field that was probably meant.
- **Every mistake is reported at once**, not only the first.
- **The same recipe always gives the same output**, byte for byte.
- Lengths are metres. Angles are degrees. A point or a size is
  `[x, y, z]`: x is right, y is up, z is forward. A model faces
  forward (+z).

## The top of the file

| Field | Kind | Default | What it is |
|---|---|---|---|
| `version` | whole number | required | `0`. Any other number is refused. |
| `name` | text | the file's name | The model's name, in reports and inside the `.glb`. |
| `description` | text | none | A sentence for people and agents. |
| `note` | anything | none | For people. Ignored. JSON has no comments; this is its comment. |
| `palette` | table, or text | no colours | The colours (below), or the path of a palette file. |
| `define` | table of parts | none | Parts defined once, to `use` many times. |
| `parts` | list of parts | required | The model's parts. At least one. |
| `sockets` | table of sockets | none | Extra sockets on the whole model, for `origin` and for when another recipe uses this one as a part. |
| `origin` | text | `"bottom"` | Where the model's zero point is (below). |
| `bones`, `colliders`, `lods` | | | Reserved for later phases. Using one is an error that says so. |

### `palette`

```toml
[palette]
steel = "#8A939B"
red   = "#D0453A"
```

- A name is letters, digits, `_` and `-`.
- A colour is `"#RRGGBB"` or the short form `"#RGB"`. Capitals or not.
  See-through colours (`"#RRGGBBAA"`) are not available yet.
- At most 256 colours.
- A colour that nothing uses is a warning.

**A shared palette file.** `palette = "palette.toml"` reads the colours
from a file, so that several models can share one. The path starts from
the recipe's folder. The file is a plain table of names:

```toml
steel = "#8A939B"
red   = "#D0453A"
```

Colours from a shared file are never "unused" warnings.

When a recipe uses another recipe as a part, the other recipe's colours
join the model's palette. The same name with two different colours is an
error that names both files.

### `origin`

The model's zero point: the point a game places it by.

- `"bottom"` (the default): the middle of the underside of the box
  around the whole model. The model stands on the ground (y = 0).
- Any other socket of the model: `"center"`, `"top"`, or one declared
  under the top-level `sockets`.
- A socket of one part: `"pole.bottom"`.

## A part

Each entry of `parts`, and each entry of `define`, is a part.

**Where its shape comes from: exactly one of these.**

| Field | Kind | The part is |
|---|---|---|
| `shape` | text | One of the built-in shapes: `box`, `cylinder`, `sphere`, `cone`, `wedge`, `capsule`, `torus`, `tube`, `pyramid`, `prism`, `stairs`, `arch`. |
| `use` | text | A copy of the part with that name under `define` (in the same file). |
| `file` | text | Another recipe (`.toml`, `.json`) or a mesh (`.obj`, `.gltf`, `.glb`). The path starts from the recipe's folder. |
| `parts` | list of parts | An assembly: a part made of parts. |

**Its other fields.**

| Field | Kind | Default | What it is |
|---|---|---|---|
| `name` | text | none | Letters, digits, `_`, `-`. Names in one list must differ. An unnamed part is called by its place in the list, counting from 0 as error messages do: `#2` is `parts[2]`. Only a named part can be attached to. |
| `color` | text | none | A palette name: the colour of the whole part. |
| `colors` | table | none | Colours for single faces, by face name: `{ top = "grass" }`. |
| `attach` | text | none | The part's own socket to join. Needs `to`. |
| `to` | text | none | `"part.socket"`: the socket of another part **in the same list** to join it to. Needs `attach`. |
| `twist` | number | `0` | With `attach` only: degrees to turn the part around its join. -36000 to 36000. |
| `rotate` | three numbers | `[0, 0, 0]` | Without `attach` only: `[x, y, z]` degrees, around x, then y, then z. -36000 to 36000. |
| `move` | three numbers | `[0, 0, 0]` | Without `attach` only: where the part's middle goes. -10000 to 10000. |
| `scale` | number, or three | `1` | How much bigger: one number, or `[x, y, z]`. 0.001 to 1000. |
| `mirror` | text | none | `"x"`, `"y"` or `"z"`: flips the part across that direction, its sockets and face names with it. |
| `sockets` | table of sockets | none | The part's own named sockets. |
| `note` | anything | none | For people. Ignored. |
| `bones`, `colliders`, `lods` | | | Reserved. |

A part under `define` has no place of its own, so it can't have `name`,
`attach`, `to`, `twist`, `rotate`, `move`, `scale` or `mirror`. Those go
on the part that uses it.

**Order.** A part is mirrored, then scaled, then either turned and moved
(`rotate`, `move`) or joined (`attach`, `to`, `twist`).

`rotate` on a joined part, and `twist` on a part that isn't joined, are
errors that show the other: `rotate = 90` on a joined part says to
write `twist = 90`, and `twist = 30` on a free part says to write
`rotate = [0, 30, 0]`. (Phase 0 wrote the twist as `rotate = 90`.)

**Turning.** `rotate = [x, y, z]` turns around x, then y, then z, in
Duck Forge's directions: a positive x angle turns forward towards up; a
positive y angle turns forward towards right; a positive z angle turns
right towards up.

### The shapes

All are centred on their own middle.

| `shape` | Field | Default | Range | |
|---|---|---|---|---|
| `box` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | Width (x), height (y), depth (z). |
| `cylinder` | `radius` | `0.5` | 0.001 to 1000 | To the middle of a flat side. |
| | `height` | `1` | 0.001 to 1000 | It stands upright. |
| | `segments` | `16` | 3 to 64 | How many flat sides. |
| `sphere` | `radius` | `0.5` | 0.001 to 1000 | |
| | `segments` | `12` | 4 to 64 | How many faces round its middle; half as many rings from top to bottom. |
| `cone` | `radius` | `0.5` | 0.001 to 1000 | Of its base, to the middle of a flat side. |
| | `height` | `1` | 0.001 to 1000 | Point at the top. |
| | `segments` | `16` | 3 to 64 | How many flat sides. |
| `wedge` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | A ramp: full height at the back, sloping down to the front edge. |
| `capsule` | `radius` | `0.25` | 0.001 to 1000 | A cylinder with a round end at top and bottom: a pill. |
| | `height` | `1` | at least twice the radius | From the lowest point to the highest, round ends included. |
| | `segments` | `16` | 3 to 64 | How many flat sides round it. |
| `torus` | `radius` | `0.5` | 0.001 to 1000 | A ring lying flat. From the middle of the hole to the middle of the ring. |
| | `tube` | `0.15` | less than `radius` | How thick the ring is: from its middle to its surface. It is `radius + tube` out to its outside. |
| | `segments` | `16` | 3 to 64 | How many flat sides go round the hole. |
| `tube` | `radius` | `0.5` | 0.001 to 1000 | An upright pipe, open at both ends. To its outside. |
| | `wall` | `0.1` | less than `radius` | How thick its wall is. |
| | `height` | `1` | 0.001 to 1000 | |
| | `segments` | `16` | 3 to 64 | How many flat sides. |
| `pyramid` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | Four slopes meeting in a point above the middle. |
| `prism` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | A roof: a slope on the left and one on the right, meeting in a ridge that runs from front to back. |
| `stairs` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | Steps going up from the front edge to the full height at the back. |
| | `steps` | `4` | 1 to 64 | How many. |
| `arch` | `size` | `[1, 1, 1]` | 0.001 to 1000 each | A wall with a doorway through it from front to back. |
| | `wall` | `0.2` | less than half the width, and less than the height | How thick the arch is beside the doorway and above it. |
| | `segments` | `8` | 2 to 64 | How many flat pieces the doorway's round top has. Where the arch is too low for a half circle, the top is flatter. |

A size field that the shape doesn't have (`radius` on a box) is an
error. So is a size field on a part that isn't a built-in shape: use
`scale` there. Sizes that don't fit each other (a capsule shorter than
it is wide, a ring thicker than its hole) are errors that say what
would fit.

**Two options.**

| Field | On | Default | Range | |
|---|---|---|---|---|
| `bevel` | `box`, `cylinder` | `0` | 0 to just under half the smallest side (a cylinder: its radius, and half its height) | Cuts the edges off: every edge of a box, the top and bottom rims of a cylinder. The number is how much is cut, in metres. The new faces are called `bevel`. |
| `bevel_steps` | `box`, `cylinder` | `1` | 1 to 4 | How many flat cuts go round each edge. 1 is one flat cut; more are rounder (and more triangles). |
| `taper` | `box`, `cylinder`, `tube` | `1` | 0.01 to 10 | How wide the top is, as a share of the bottom: `0.5` is half as wide, `2` twice. One number, or `[x, z]` to narrow the two ways differently. The bottom stays as it is. |

A shape with a bevel or a taper is still as big as its sizes say at its
widest. A socket on a side still sits on the surface: on a tapered
side it leans with the side, and at the very edge of a bevelled face it
is on the cut and leans with that. Give such a socket a `facing` of its
own (`facing = "front"`) where it should look straight out.

**Shaping a box: `changes`.** A `box` (with or without `taper`, without
`bevel`) can have a list of changes, made to it after it is built, one
after the other, in the order they are written:

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

Each change is a table with one of these, and the fields it takes:

| Change | Fields | What it does |
|---|---|---|
| `pull` | `pull` (a face name, or a list of them), `by` (metres, not 0: -1000 to 1000), `name` (optional) | Pulls the faces out the way they face by `by`, and fills the gap with new walls. A negative `by` pushes them in, making a hollow. Faces in the list that share an edge are pulled as one piece, with walls only round its outside, the way the faces face together. |
| `inset` | `inset` (a face name, or a list), `by` (metres, above 0), `name` (optional) | Makes a smaller face inside each face, `by` in from every edge, with a ring of four-sided faces round it. `by` must be less than half the face's width. Nothing moves: there are just more faces. |
| `move` | `move` (a face name, or a list), `by` (`[x, y, z]` metres, not all 0) | Moves the faces; the faces round them stretch to follow. No new faces. |
| `turn` | `turn` (a face name, or a list), `by` (`[x, y, z]` degrees, not all 0) | Turns the faces about their middle (the middle of them all, for several): around x, then y, then z. Faces round them may bend: a bent four-sided face is drawn as two triangles, folded the way that bulges out, and keeps its name. |
| `scale` | `scale` (a face name, or a list), `by` (one number above 0, or `[x, y, z]`; not 1) | Makes the faces bigger or smaller about their middle. |

Amounts are metres, in the part's own directions, before its `scale`,
`mirror`, `rotate` and `move`. Resizing the box keeps them: a frame
6 cm wide stays 6 cm wide.

**Names.** Every face has a name made of words, never a number, and no
size or amount changes it:

| Change | The face it is made to | The new faces |
|---|---|---|
| `pull` | Keeps its name: it is the end of what was pulled. | One wall along each edge, `<face>_side_<side>`, where the side is the edge's side of the face (`top_side_front`). With `name = "leg"`: `leg_front`, `leg_left`... `<face>_side`, or the `name`, stands for all the walls. |
| `inset` | Its name now stands for the ring left round the new face. | The face in the middle: `<face>_inset`, or the `name`. The ring's pieces: `<middle>_rim_<side>` (`window_rim_top`). |
| `move`, `turn`, `scale` | Keeps its name. | None. |

The side words (`top`, `left`, `front`...) are passed down from the
box's faces to the faces the changes make, not measured, so a turned
face keeps them. A name can stand for several faces: walls along the
same side share one, and `sides`, a pull's name and an inset face's old
name stand for groups. Such names work wherever a face name does, in
`colors` and in later changes. In `colors`, a name for fewer faces wins
over one for more (`front` over `sides`), whichever is written first.
A name a change would make that an earlier change made already gets a
number: `top_side_front_2`, `top_inset_2`. A `name` you give is used as
it is.

`duck-mold inspect <recipe> --faces` lists every face of every shaped
part: its name, the way it faces, its size, and the faces next to it.

**Mistakes.** A change that names a face that isn't there
(`unknown_face`, with the closest name), an inset wider than the face
allows (`out_of_range`, saying how wide it may be), or a change that
would squeeze a face flat, turn the shape inside out or make it pass
through itself (`bad_shape`) is an error on that change's line. Nothing
is built. `changes` on any other shape, on a bevelled box, or on a part
that `use`s a definition (shape the definition instead) is an error too.

A round shape's first flat side faces right (+x). When `segments` is a
multiple of 4, flat sides face left, right, front and back, and a
`radius` of 0.5 is exactly 1 wide.

**Face names**, for `colors`:

| `shape` | Faces | Triangles |
|---|---|---|
| `box` | `top`, `bottom`, `left`, `right`, `front`, `back`; and `sides` for the last four together | 12 |
| `cylinder` | `top`, `bottom`, `side` | 4 × segments − 4 |
| `sphere` | `side` | segments × (segments − 2), for even counts |
| `cone` | `side`, `bottom` | 2 × segments − 2 |
| `wedge` | `slope`, `bottom`, `back`, `left`, `right` | 8 |
| `capsule` | `side`, `top`, `bottom` (the two round ends) | 2 × segments, and 4 × segments × rings − 2 × segments for the ends, where rings is segments ÷ 4 (at least 2): 224 for 16 |
| `torus` | `side` | 2 × segments × round, where round is how many sides go round the ring itself: half of segments, up to the next multiple of 4 (at least 4): 256 for 16 |
| `tube` | `side` (the outside), `inside`, `top`, `bottom` | 8 × segments |
| `pyramid` | `front`, `back`, `left`, `right`, `bottom`; and `sides` for the four slopes together | 6 |
| `prism` | `left`, `right` (the slopes), `front`, `back` (the ends), `bottom` | 8 |
| `stairs` | `top` (where you step), `front` (the risers), `back`, `left`, `right`, `bottom` | 12 × steps |
| `arch` | `front`, `back`, `left`, `right`, `top`, `bottom` (under its two feet), `inside` (the doorway) | 4 × segments + 24 |
| a bevelled `box` | the box's, and `bevel` | 12 + 24 × steps + 8 × steps × steps: 44 for one step |
| a bevelled `cylinder` | the cylinder's, and `bevel` | 4 × segments × steps more than the cylinder |
| a `box` with `changes` | the box's, and those the changes make (above) | 2 for each four-sided face: a pull adds a wall for each edge round the outside, an inset a ring of as many faces as the face has edges |
| an `.obj` file | its material names (`default` for faces before any material) | as in the file |
| a `.gltf` or `.glb` file | its material names (`material_N` for one with no name, `default` for faces with no material); for a `.glb` Duck Mold wrote, its palette names | as in the file |

A model can have at most 1,000,000 triangles, and parts can be nested
32 deep.

### Colour

Every face gets one palette colour, from the nearest thing that names
one:

1. A face name in `colors`: on the shape itself first, then on the part
   that uses it. Face names don't reach through an assembly.
2. A whole-part `color`: the part's own first, then the `color` of each
   part it is inside, outwards.

So `colors` beats `color`, and an assembly's `color` is the colour of
everything inside it that has none of its own.

- A `color` or `colors` value must be a palette name. A colour code
  there is an error.
- A face name the shape doesn't have is an error that lists the ones it
  has.
- `colors` on an assembly is an error: an assembly has no faces of its
  own.
- A face with no colour at all is built plain grey (`#B4B4B4`), with a
  warning.
- A `color` that changes nothing (every face inside already has one) is
  a warning.

## Sockets

A socket is a named point with a direction it faces.

**Built in.** Every part has seven: `top`, `bottom`, `left`, `right`,
`front`, `back` (the middle of each side, facing out) and `center` (the
middle, facing up). A wedge has an eighth, `slope`: the middle of its
slope, facing straight out of it. These names can't be declared again
(`slope` only on a wedge).

**On the surface.** A socket of one of the built-in shapes sits on the
shape's surface. It starts on the side of the box around the shape; if
the shape isn't there, it moves straight in until it touches the shape,
and faces straight out of the surface there (on an edge between two
faces, halfway between them). A socket already on the surface doesn't
move. So a cone's `left`, `right`, `front` and `back` are halfway up
its slant, and a wedge's `top` and `front` are the middle of its slope.
A pyramid's `top` is its point and a prism's the middle of its ridge;
their side sockets are halfway up their slopes, leaning with them.
Where nothing lies straight behind the box side, the socket stays on
the box: a torus's and a tube's `top` and `bottom` are in the middle of
the opening, level with the rim (where a lid goes, or what it stands
on), and an arch's `front` and `back` are in the mouth of its doorway.
One exception: an arch's `bottom` is on the ground between its feet,
not up under the top of the doorway.
A part made of parts and a part made from a mesh file follow the same
rule, through every face inside them; where nothing lies straight behind
the box side, the socket stays on the box. A whole recipe's own sockets
(the model's, and those of a recipe file used as a part) stay on the box
around it: they say how big it is and where the ground is.

**Declared.** Under `sockets`, each name is one of three forms:

| Form | Fields | |
|---|---|---|
| On a side | `on` | `top`, `bottom`, `left`, `right`, `front`, `back` or `center`. |
| | `shift` (default `[0, 0, 0]`) | Slides the socket across the side of the box: each number from -1 (the left, bottom or back edge) to 1 (the right, top or front edge). The number for the direction the side faces must be 0. With `center`, all three are free. Then, on a shape, the socket moves straight in onto the surface, as above; a shift that puts it beside the shape, where there is nothing to sit on, is an error. |
| | `facing` (default: out of the surface) | A direction. Given, it wins over the surface's. |
| At a point | `at` | `[x, y, z]` in the part's own space. For a shape, zero is its middle. For an assembly, zero is where a part of its list goes when it is neither attached nor moved. For a recipe used as a part, zero is its `origin`. |
| | `facing` (default `"up"`) | A direction. |
| Passed on | `from` | `"part.socket"`: a socket of a part inside this assembly, offered to the outside under a new name. |

A direction is `up`, `down`, `left`, `right`, `front`, `back`, or three
numbers `[x, y, z]` that aren't all zero.

### Joining

`attach = "bottom"` and `to = "trunk.top"` move and turn the part so
that its `bottom` socket meets the trunk's `top`, the two facing each
other.

- The part is turned **as little as possible** to make the sockets face
  each other.
- If the two sockets face the same way, the part must turn right round.
  It turns around the upright axis; if the sockets face up or down, it
  rolls over sideways around the front-to-back axis instead.
- `twist` then turns the part around the join, clockwise seen from
  the side the other socket faces. On a socket that faces up it is the
  same turn as `rotate = [0, n, 0]` on a free part.
- `mirror` and `scale` are done first, sockets included.
- With `attach`, `move` is an error.

The parts of one list form a tree. A part attached to itself, or parts
attached in a ring, are errors.

## Files as parts

- **A recipe** (`file = "wheel.toml"`): the whole model, with its zero
  point where its `origin` says, its own `define`s, and its top-level
  `sockets` to attach by. A recipe that ends up using itself is an
  error.
- **An OBJ mesh** (`file = "rock.obj"`): points, faces and material
  names are read; faces with more than three corners are cut into
  triangles. The file is read as y up, facing +z, the way glTF means it
  and Blender writes OBJ. For a file in other units or directions, use
  `scale` and `rotate` on the part. The file's own colours and textures
  are not used: colour it with `color`, and with `colors` by material
  name. `duck-mold inspect rock.obj` lists a file's size and materials.
- **A glTF file** (`file = "lamp.gltf"` or `file = "lamp.glb"`): every
  mesh in the file's main scene, each placed, turned and sized as its
  object says (by numbers or by a matrix), merged into one part.
  Triangles in any of glTF's three layouts, corner numbers of any size
  or none; a `.gltf`'s numbers in a `.bin` file beside it or written
  inside it as base64. Read as y up, facing +z, x to the model's left,
  which is what Duck Mold exports, so an exported model imported again
  comes back where it was.
  - **Colour:** `color` and `colors` (by material name) as for OBJ. A
    material they leave uncoloured keeps the file's own base colour: it
    is added to the palette under the material's name (or, if the
    palette already has that name, the palette's colour is used), and a
    `no_color` warning lists what was kept. A `.glb` Duck Mold wrote
    brings its palette, as a recipe used as a part does: its face names
    are its palette names.
  - **Left out, with a note:** lines and points, skins, animations,
    shape keys (morph targets), cameras and textures (a material's
    colour is its base colour). Normals and texture coordinates are
    passed over.
  - **Refused, with a sentence:** a file that says it needs an extension
    to be read (Draco or meshopt compression, for example), corners
    packed into whole numbers, sparse accessors, and glTF 1.

## Numbers in the output

Every position is rounded to a thousandth of a millimetre. `inspect`
treats surfaces within a millimetre of each other as touching.
