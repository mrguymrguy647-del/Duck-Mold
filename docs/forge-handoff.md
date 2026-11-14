# Duck Mold models in Duck Forge

Duck Forge loads Duck Mold's `.glb` files with `Mesh::load_glb`, and
plays their animations with `AnimatedModel`, both in its `src/glb.rs`. This note says exactly what that loader supports, and how
to keep it in step with Duck Mold. It was first written for the session
that added the loader; what that session found is folded in below.

## The reader, and keeping its two copies the same

The reader is [`docs/forge/duck_mold_glb.rs`](forge/duck_mold_glb.rs).
It uses only the standard library and `serde_json` (already a Duck Forge
dependency, so Duck Forge's `tests/dependencies.rs` stays green), shares
no code with Duck Mold's exporter, and is run by Duck Mold's tests
against every example.

Everything between its two `// ====` marker lines is the reader, and
Duck Forge's `src/glb.rs` holds the same lines, byte for byte. Each
project keeps its own introduction and imports above the markers, and
Duck Forge its own functions below them (`GlbModel::load`, `to_mesh`,
`part`, `socket`, `GlbPart::range`, `GlbSocket::position` and
`direction`, `Mesh::load_glb`, and `AnimatedModel`). The first marker
says the reader's version; this note describes **version 5**.

To change the reader: change it here, raise the version in both marker
lines, run Duck Mold's tests, then copy the lines between the markers
into Duck Forge. With both repositories side by side, this prints
nothing when the two copies are the same:

```bash
diff <(sed -n '/^\/\/ ==== Duck Mold/,/^\/\/ ==== end of Duck Mold/p' Duck-Mold/docs/forge/duck_mold_glb.rs) <(sed -n '/^\/\/ ==== Duck Mold/,/^\/\/ ==== end of Duck Mold/p' duck-forge/src/glb.rs)
```

| Version | What changed |
|---|---|
| 1 | Duck Mold's Phase 0. |
| 2 | Assemblies (below). Sizes read from the file are checked before they are added or multiplied, so a damaged file that claims more numbers or triangles than it holds is refused with a sentence instead of overflowing (which a debug build turns into a crash). |
| 3 | Sockets (below): `GlbModel.sockets`, so a game can ask where `head.top` is. Duck Mold's Phase 1. |
| 4 | Files laid out as a **tree of parts** (below): `GlbModel.joins` (what every part turns about and hangs from) and `GlbSocket.join` (the part that carries a socket). Files from before are read exactly as they were. A file from a newer Duck Mold than the reader knows is refused with a sentence that says to update Duck Forge. Duck Mold's Phase 5, step 1. |
| 5 | **Animations and poses** (below): `GlbModel.animations` and `poses`, each join's `turns_in`, and what it takes to play them: `GlbAnimation::at` (how an animation has every join at a time), `GlbPlace` (how a join stands, and `towards`, a place between two), `GlbModel::stands` and `posed` (where that puts every corner and socket) and `turn_by` (a join's turn from degrees). Duck Mold's Phase 5, step 5. |

**Which Duck Forge reads which file.** The file says how it is laid out
in `extras.duck_mold.version`: 0 for one object with one list of
triangles (Duck Mold up to 0.4), 1 for a tree (Duck Mold 0.5). Reader
versions 4 and 5 read both. A reader from before refuses a tree with a
sentence, rather than drawing one part of it, so **update Duck Forge
before exporting with Duck Mold 0.5**. Animations and poses didn't
change that number: they are added beside the model, so reader version
4 loads a file that has them as the same model, standing still.

## What a Duck Mold `.glb` is

One file, glTF 2.0 binary. Always the same narrow shape:

| | |
|---|---|
| Header | 12 bytes: the letters `glTF`, the number 2, the file's length (32-bit, smallest byte first). |
| Block 1 | Its length, the letters `JSON`, then JSON text padded with spaces to a multiple of 4 bytes. |
| Block 2 | Its length, the letters `BIN` and a zero byte, then the numbers. |
| Contents, as a tree (version 1) | One scene with one node, the model (node 0). Under it, **one node for every part**, in a tree that follows the joins. A part with a shape of its own has a mesh with one primitive, `mode` 4 (triangles); a part made of parts has none. |
| Contents, before (version 0) | One scene, one node, one mesh, one primitive, `mode` 4. |
| Numbers | All little-endian. Decimals are 32-bit (`componentType` 5126), whole numbers 32-bit unsigned (5125). |

### The tree

- **A node per part.** Node 0 is the model. Then every part, in the
  recipe's order, a part made of parts before the parts inside it. A
  node's `name` is the part's path (`left_arm/hand`).
- **`children` follow the joins.** A part joined to `body.neck` is a
  child of `body`. A part that isn't joined is a child of the part it
  is inside, or of the model.
- **`translation` is the part's pivot**, measured from the pivot of the
  node it hangs from: the socket it is joined to or, for a part that
  isn't joined, its own middle. At rest nothing is turned or
  stretched: no node has `rotation`, `scale` or `matrix`. So turning a
  node turns the part about its join, and everything under it follows.
  That is what animation will do (Phase 5, step 5).
- **A part's corners are measured from its pivot.** Its mesh's
  `POSITION`s are added to where its node is.
- **The lists are shared.** There are four buffer views (positions,
  directions, colours, triangle corners); each part's accessors point
  into them with a `byteOffset`. The triangles' corner numbers count
  from the part's own first corner.
- The parts' triangles, taken in the order of `extras.duck_mold.parts`,
  are the one list a file from before had, in the same order.

## What the loader must read

1. **The two blocks**, as above.
2. **A primitive** (`meshes[0].primitives[0]` in a file from before;
   in a tree, for each entry of `parts` in order, the first primitive
   of the mesh of its `node`): `attributes.POSITION`,
   `attributes.COLOR_0` and `indices`. Each is a number that picks an
   entry of `accessors`.
3. **An accessor**: `count`, `type` and `componentType`, its own
   `byteOffset` (0 if absent) and `bufferView`, which picks an entry of
   `bufferViews`; that has `byteOffset` and `byteLength` into block 2.
   - `POSITION`: `VEC3` of 5126. Three decimals per corner.
   - `COLOR_0`: `VEC3` of 5126. Red, green, blue per corner, 0 to 1, as
     **linear light**.
   - `indices`: `SCALAR` of 5125. Three per triangle.
4. **`extras.duck_mold`** (optional for drawing, useful for games):
   - `version`: 1 for a tree, 0 for a file from before. (It counts
     changes an older reader couldn't read through. Adding `assemblies`
     and `sockets` changed none. Splitting the triangles between the
     parts' nodes did.)
   - `name`: the model's name.
   - `palette`: `[{ "name": "steel", "hex": "#8A939B" }, ...]`.
   - `parts`: `[{ "name": "left_arm/hand", "first_triangle": 208, "triangles": 12, "node": 13 }, ...]`:
     the parts with a shape of their own. They are in order and together
     own every triangle, once each. `node` (in a tree) is the part's
     node, where its triangles are.
   - `assemblies`: `[{ "name": "left_arm", "first_triangle": 136, "triangles": 84, "node": 9 }, ...]`:
     the parts made of parts, outer ones before the ones inside them.
     Each owns the triangles of every part inside it, which are always
     next to each other. A part inside an assembly is named after it:
     `left_arm/hand` is in `left_arm`. Files written before version 2 of
     the reader have no `assemblies`; read that as none.
   - `sockets`: `[{ "name": "head.top", "at": [x, y, z], "facing": [x, y, z] }, ...]`:
     every named socket, where it ended up. The model's own have bare
     names (`top`), a part's are `part.socket`, a part inside an
     assembly keeps its path (`left_arm/hand.bottom`), an assembly's own
     come after its name (`left_arm.grip`). Parts without a name, and
     the parts inside them, have none here. `at` and `facing` are in
     glTF's directions like the positions: **negate x** for both.
     `facing` is 1 long. Files written before version 3 of the reader
     have no `sockets`; read that as none. In a tree a socket also has
     `node`: the node that carries it when the model moves (0, the
     model, for a socket no part carries). A socket an arm passes on
     from its hand (`left_arm.grip`) is carried by the hand.
   - `turns_in`, on an entry of `parts` or `assemblies`, when it is
     there: `[[x, y, z], [x, y, z], [x, y, z]]`, the three directions
     the part's turns are measured around (what x, y and z mean in a
     pose's `[x, y, z]` for that part), in glTF's directions: **negate
     x** of each. Absent, they are the model's own right, up and
     forward. It is there for a part inside one that is turned or a
     mirror image (the robot's right arm).
   - `animations`: `[{ "name": "walk", "animation": 1, "loop": true, "length": 0.8 }, ...]`:
     the model's animations in the recipe's order. `animation` picks an
     entry of the file's own `animations` (below); it is absent for an
     animation that moves nothing. `length` is in seconds.
   - `poses`: `[{ "name": "wave", "joints": [{ "node": 8, "rotation": [x, y, z, w], "translation": [x, y, z] }, ...] }, ...]`:
     each named pose, as the nodes that aren't as they are at rest.
     `rotation` and `translation` are written as an animation writes
     them (below), and either can be absent. `rest` isn't listed: it is
     every node as the file has it.
5. **The file's `animations`** (standard glTF; absent in a model that
   doesn't move). Each has `samplers` and `channels`.
   - A channel's `target` is a `node` and a `path`: `"rotation"` or
     `"translation"`. Nothing else is written. Its `sampler` picks a
     sampler.
   - A sampler has `input` (an accessor: `SCALAR` of 5126, the times in
     seconds, from 0, thirty a second and the animation's very end),
     `output` (an accessor: `VEC4` of 5126 for a rotation, `VEC3` of
     5126 for a translation, one for each time) and `interpolation`,
     always `"LINEAR"`: between two times, go straight from one value
     to the next. Every sampler of an animation has the same `input`.
   - A **rotation** is a unit quaternion `[x, y, z, w]`: how the node
     is turned about its own origin, which is the part's pivot. In
     Duck Forge's directions it is `[x, -y, -z, w]`. Consecutive
     samples are written alike (their dot product isn't negative), so
     going "the short way" between two is right. A wheel can turn at
     most half round between two samples, 15 turns a second.
   - A **translation** is the node's whole place, measured from the
     node it hangs from: its `translation` at rest plus how far the
     pose moves it. Take the node's rest `translation` away (as 32-bit
     numbers) and negate x to get the move.
   - A node turned and moved carries the nodes under it, as in any
     glTF file: a corner of a part is at `parent's place + parent's turn
     × (own translation + own turn × corner)`, all the way up to the
     model.

Sizes and counts come from the file, so check them before adding or
multiplying them.

## What the loader must do to the numbers

0. **In a tree, add up where each node is**: its `translation` and
   those of every node it hangs from, kept as a 32-bit number at each
   node; then a corner is its node's place plus its `POSITION`, added
   with more digits than are kept and stored as a 32-bit number. Duck
   Mold chooses the numbers it writes so that these sums come out as a
   file from before held them, wherever 32 bits allow: about nineteen
   in twenty to the last bit, the ground at exactly 0, and the rest
   within about a ten-thousandth of a millimetre.
1. **Negate x.** glTF's "right" is -x; Duck Forge's is +x. Up (+y) and
   forward (+z) are the same in both. (Translations too.)
2. **Swap the second and third corner of every triangle.** Negating x
   turns triangles inside out; the swap gives Duck Forge's order,
   clockwise seen from the front.
3. **Turn each colour back into 0 to 255.** For each of red, green,
   blue: if the value is at most 0.0031308, multiply by 12.92; otherwise
   `1.055 × value^(1/2.4) − 0.055`. Then multiply by 255 and round.
   This gives back the palette's exact numbers (a Duck Mold test checks
   all 256).
4. **Take a triangle's colour from its first corner.** All three corners
   of a triangle always carry the same colour: that is Duck Forge's "one
   flat colour per triangle".

That gives `mesh.add_vertex(vec3(x, y, z))` and
`mesh.add_triangle(a, b, c, Color::rgb(r, g, b))` directly. Keep the
triangles in the file's order: then a part's or an assembly's range of
triangles picks out its triangles in the mesh too.

Every face has corners of its own in the file, so a box arrives with 24
points. The reader merges a part's points that are in the same place,
which gives its 8 back. In a tree, two parts never share a point (so a
part can move by itself); a file from before is merged as a whole, as
it always was.

**The joins.** For a tree the reader also gives `joins`: for every
part and assembly, its name, its `pivot` in the model at rest, the
`parent` it hangs from (a place in `joins`; none for a part that hangs
from the model), and which of `vertices` are its own (`first_vertex`,
`vertices`; none for an assembly). A join always comes after the one
it hangs from, so one pass from first to last can place them all. A
file from before has no joins: it can be drawn, not posed.

## What the loader can ignore

`NORMAL` (Duck Forge works out a face's direction from its corners),
`materials` (one plain matt material, always the same), `asset`,
`scenes`, the accessors' `min` and `max`, the views' `byteStride`, and
anything else. There are no textures, UVs, cameras, lights or skins.
A reader that only draws the model can ignore `animations`, `poses`
and `turns_in` too.

## What a model is, once loaded

- x right, y up, z forward; 1 unit is about a metre.
- **It faces forward (+z).** A default `Camera3D` looks along +z, so it
  sees a newly loaded model from behind. Put the camera on the model's
  +z side looking back at it (Duck Forge's `examples/turntable/`), or
  turn the model half round: `rotated(vec3(0.0, PI, 0.0))`.
- **It stands on y = 0**, centred over its zero point, unless the recipe
  set `origin`.
- A face nobody coloured is the grey `180, 180, 180`, which is not in
  the palette.
- Every face is flat-coloured and unlit in the file; Duck Forge lights
  it.

## In Duck Forge

```rust
let robot = Mesh::load_glb("assets/robot.glb")?;       // one mesh, ready to draw
canvas.draw_mesh(&robot, &Transform::at(position), &camera);
```

`duckforge::glb::GlbModel::load(path)?` gives the name, the palette, the
parts, the assemblies and the sockets; `model.part("left_arm")` finds a
part or an assembly by name, and `mesh.triangles[part.range()]` are its
triangles (to recolour an arm, say). `model.socket("head.top")` finds a
socket: `position()` is where it is and `direction()` the way it faces,
in the model's own space, so a hat goes at
`transform.apply(socket.position())`. `duck-mold inspect --sockets`
lists every name. A file that isn't a Duck Mold model is a one-sentence
`LoadError`.

A model that moves:

```rust
let mut robot = AnimatedModel::load("assets/robot.glb")?; // in new()
robot.play("idle");
if walking { robot.play("walk") } else { robot.play("idle") } // in update()
robot.update(ctx.dt());
canvas.draw_mesh(robot.mesh(), &Transform::at(position), &camera); // in draw()
```

`play`, `play_once`, `play_looping`, `restart`, `stop`, `resume`,
`rest`, `set_pose`, `set_speed`, `set_blend`, `turn`, `release`,
`update`, `mesh`, `socket` (where a socket is now), `is_playing`,
`finished`, `playing`, `time`, `animations` and `poses`: Duck Forge's
README and guide say what each does. `examples/robot_walk/` is the
robot standing idle, walking and waving, with a hat on.

**Which half is whose.** Where an animation has a join at a time, and
where that puts the model's corners and sockets, is in the shared
reader (`GlbAnimation::at`, `GlbModel::stands`, `posed`, `turn_by`,
`GlbPlace::towards`): Duck Mold's tests (`tests/glb_motion.rs`) check
it against Duck Mold's own posing, corner by corner, for every pose
and every sample of every animation. What is playing, how fast, the
change-over from one animation to another and the game's own turns are
`AnimatedModel`, Duck Forge's own, below the markers.

## Files to test with

`tests/golden/*.glb` in this repository, all laid out as a tree: crate
(12 triangles), table (60), tree (70), house (120, with one assembly,
its roof), robot (384, with four: its arms and legs; three animations,
`idle`, `walk` and `wave`, and six poses), snowman (600),
and the other examples. For each of the robot's animations there is a
`robot.<animation>.inspect.json` (what `duck-mold inspect --animation
<name> --json` says: its length, whether it loops, how many frames, how
far the lowest point leaves the ground) and a `robot.<animation>.strip.png`
(eight frames of it, side by side). `tests/before-phase5/` holds the crate, the
house and the robot as Duck Mold 0.4 wrote them, one list of triangles
each, to test that files from before still load as the same models;
Duck Forge keeps the same three in `tests/fixtures/duck_mold_before_tree/`.
Each has a
`*.inspect.json` beside it with the size, box, triangle count, palette
and colour counts the loader should find, and the tree of parts;
`duck-mold inspect examples/robot.toml --sockets --json` lists the
sockets it should find. The report's numbers are 64-bit and
the file holds 32-bit ones, so positions agree to about 0.00001, not
exactly. The official Khronos glTF validator reports no errors,
warnings, infos or hints on any of them.

A good first test: load `crate.glb` and check that every triangle's
front faces away from the crate's middle. That's Duck Forge's own
`assert_faces_point_outward`, with one change: the crate stands on
y = 0, so its middle is (0, 0.5, 0), not the origin the helper measures
from (from the origin, its two bottom triangles fail). The helper is
private to Duck Forge's `render3d.rs` tests, so a test elsewhere writes
the same few lines. Then check it has 8 vertices, 12 triangles, 8 of
them `#B07A3F` and 4 `#8A5A2B`.

## The words for directions

`Mesh::cube` in Duck Forge's `src/render3d.rs` used to call its -z face
`front`, because that's the face a default camera sees. Duck Mold says a
model **faces forward (+z)**, so its `front` is +z. The cube's comments
(and `Mesh::block`'s) now say `near (-z)` and `far (+z)`, and Duck
Forge's guide, section 11, says a model faces +z.
