# Duck Mold models in Duck Forge

Duck Forge loads Duck Mold's `.glb` files with `Mesh::load_glb`, in its
`src/glb.rs`. This note says exactly what that loader supports, and how
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
`part`, `GlbPart::range`, `Mesh::load_glb`). The first marker says the
reader's version; this note describes **version 2**.

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

## What a Duck Mold `.glb` is

One file, glTF 2.0 binary. Always the same narrow shape:

| | |
|---|---|
| Header | 12 bytes: the letters `glTF`, the number 2, the file's length (32-bit, smallest byte first). |
| Block 1 | Its length, the letters `JSON`, then JSON text padded with spaces to a multiple of 4 bytes. |
| Block 2 | Its length, the letters `BIN` and a zero byte, then the numbers. |
| Contents | One scene, one node, one mesh, one primitive, `mode` 4 (triangles). |
| Numbers | All little-endian. Decimals are 32-bit (`componentType` 5126), whole numbers 32-bit unsigned (5125). |

## What the loader must read

1. **The two blocks**, as above.
2. **`meshes[0].primitives[0]`**: `attributes.POSITION`,
   `attributes.COLOR_0` and `indices`. Each is a number that picks an
   entry of `accessors`.
3. **An accessor**: `count`, `type` and `componentType`, and
   `bufferView`, which picks an entry of `bufferViews`; that has
   `byteOffset` and `byteLength` into block 2.
   - `POSITION`: `VEC3` of 5126. Three decimals per corner.
   - `COLOR_0`: `VEC3` of 5126. Red, green, blue per corner, 0 to 1, as
     **linear light**.
   - `indices`: `SCALAR` of 5125. Three per triangle.
4. **`extras.duck_mold`** (optional for drawing, useful for games):
   - `version`: 0. (It counts changes to the meaning of these fields.
     Adding `assemblies` changed none, so files with them are still 0,
     and an older reader still reads them.)
   - `name`: the model's name.
   - `palette`: `[{ "name": "steel", "hex": "#8A939B" }, ...]`.
   - `parts`: `[{ "name": "left_arm/hand", "first_triangle": 208, "triangles": 12 }, ...]`:
     the parts with a shape of their own. They are in order and together
     own every triangle, once each.
   - `assemblies`: `[{ "name": "left_arm", "first_triangle": 136, "triangles": 84 }, ...]`:
     the parts made of parts, outer ones before the ones inside them.
     Each owns the triangles of every part inside it, which are always
     next to each other. A part inside an assembly is named after it:
     `left_arm/hand` is in `left_arm`. Files written before version 2 of
     the reader have no `assemblies`; read that as none.

Sizes and counts come from the file, so check them before adding or
multiplying them.

## What the loader must do to the numbers

1. **Negate x.** glTF's "right" is -x; Duck Forge's is +x. Up (+y) and
   forward (+z) are the same in both.
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
points. The reader merges points that are in the same place, which gives
its 8 back.

## What the loader can ignore

`NORMAL` (Duck Forge works out a face's direction from its corners),
`materials` (one plain matt material, always the same), `asset`,
`scenes`, `nodes`, the accessors' `min` and `max`, and anything else.
There are no textures, UVs, cameras, lights, animations or skins.

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
parts and the assemblies; `model.part("left_arm")` finds a part or an
assembly by name, and `mesh.triangles[part.range()]` are its triangles
(to recolour an arm, say). A file that isn't a Duck Mold model is a
one-sentence `LoadError`.

## Files to test with

`tests/golden/*.glb` in this repository: crate (12 triangles), table
(60), tree (70), house (76, with one assembly, its roof), robot (384,
with four: its arms and legs). Each has a `*.inspect.json` beside it
with the size, box, triangle count, palette and colour counts the loader
should find, and the tree of parts. The report's numbers are 64-bit and
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
