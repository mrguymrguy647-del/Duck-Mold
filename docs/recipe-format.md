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
| `shape` | text | One of the built-in shapes: `box`, `cylinder`, `sphere`, `cone`, `wedge`. |
| `use` | text | A copy of the part with that name under `define` (in the same file). |
| `file` | text | Another recipe (`.toml`, `.json`) or a mesh (`.obj`). The path starts from the recipe's folder. |
| `parts` | list of parts | An assembly: a part made of parts. |

**Its other fields.**

| Field | Kind | Default | What it is |
|---|---|---|---|
| `name` | text | none | Letters, digits, `_`, `-`. Names in one list must differ. An unnamed part is called by its place in the list, counting from 0 as error messages do: `#2` is `parts[2]`. Only a named part can be attached to. |
| `color` | text | none | A palette name: the colour of the whole part. |
| `colors` | table | none | Colours for single faces, by face name: `{ top = "grass" }`. |
| `attach` | text | none | The part's own socket to join. Needs `to`. |
| `to` | text | none | `"part.socket"`: the socket of another part **in the same list** to join it to. Needs `attach`. |
| `rotate` | number, or three | `0` | With `attach`: degrees to twist around the join. Without: `[x, y, z]` degrees, around x, then y, then z. |
| `move` | three numbers | `[0, 0, 0]` | Without `attach` only: where the part's middle goes. -10000 to 10000. |
| `scale` | number, or three | `1` | How much bigger: one number, or `[x, y, z]`. 0.001 to 1000. |
| `mirror` | text | none | `"x"`, `"y"` or `"z"`: flips the part across that direction, its sockets and face names with it. |
| `sockets` | table of sockets | none | The part's own named sockets. |
| `note` | anything | none | For people. Ignored. |
| `bones`, `colliders`, `lods` | | | Reserved. |

A part under `define` has no place of its own, so it can't have `name`,
`attach`, `to`, `rotate`, `move`, `scale` or `mirror`. Those go on the
part that uses it.

**Order.** A part is mirrored, then scaled, then either turned and moved
(`rotate`, `move`) or joined (`attach`, `to`, `rotate`).

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

A size field that the shape doesn't have (`radius` on a box) is an
error. So is a size field on a part that isn't a built-in shape: use
`scale` there.

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
| an `.obj` file | its material names (`default` for faces before any material) | as in the file |

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

**On the surface.** A socket of one of the five shapes sits on the
shape's surface. It starts on the side of the box around the shape; if
the shape isn't there, it moves straight in until it touches the shape,
and faces straight out of the surface there (on an edge between two
faces, halfway between them). A socket already on the surface doesn't
move. So a cone's `left`, `right`, `front` and `back` are halfway up
its slant, and a wedge's `top` and `front` are the middle of its slope.
An assembly, and a part made from a file, keep their sockets on the box
around them.

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
- `rotate` then twists the part around the join, clockwise seen from
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
- glTF files can't be used as parts yet.

## Numbers in the output

Every position is rounded to a thousandth of a millimetre. `inspect`
treats surfaces within a millimetre of each other as touching.
