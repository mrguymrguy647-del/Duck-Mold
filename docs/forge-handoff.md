# Handoff: loading Duck Mold models in Duck Forge

For the session that adds a `.glb` loader to Duck Forge. Duck Forge can't
load model files yet; this note says exactly what the loader must
support to read what Duck Mold writes, and no more.

A working reader is in [`docs/forge/duck_mold_glb.rs`](forge/duck_mold_glb.rs).
It uses only the standard library and `serde_json` (already a Duck Forge
dependency, so `tests/dependencies.rs` stays green), shares no code with
Duck Mold's exporter, and is run by Duck Mold's tests against every
example. Copy it in and adapt it; the list below is what it does.

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
   - `version`: 0.
   - `name`: the model's name.
   - `palette`: `[{ "name": "steel", "hex": "#8A939B" }, ...]`.
   - `parts`: `[{ "name": "left_arm/hand", "first_triangle": 24, "triangles": 12 }, ...]`.
     Parts are in order and together own every triangle.

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
`mesh.add_triangle(a, b, c, Color::rgb(r, g, b))` directly.

Optional: every face has corners of its own in the file, so a box
arrives with 24 points. Merging points that are in the same place gives
its 8 back. The reader does this.

## What the loader can ignore

`NORMAL` (Duck Forge works out a face's direction from its corners),
`materials` (one plain matt material, always the same), `asset`,
`scenes`, `nodes`, the accessors' `min` and `max`, and anything else.
There are no textures, UVs, cameras, lights, animations or skins.

## What a model is, once loaded

- x right, y up, z forward; 1 unit is about a metre.
- **It faces forward (+z).** A default `Camera3D` looks along +z, so it
  sees a newly loaded model from behind. To face the camera, turn the
  model half round: `rotated(vec3(0.0, PI, 0.0))`.
- **It stands on y = 0**, centred over its zero point, unless the recipe
  set `origin`.
- A face nobody coloured is the grey `180, 180, 180`, which is not in
  the palette.
- Every face is flat-coloured and unlit in the file; Duck Forge lights
  it.

## Suggested shape in Duck Forge

```rust
let robot = Mesh::load_glb("assets/robot.glb")?;       // one mesh, ready to draw
canvas.draw_mesh(&robot, &Transform::at(position), &camera);
```

A refusal should be a sentence, like the rest of Duck Forge:
`robot.glb wasn't written by Duck Mold (no extras.duck_mold): only Duck Mold's .glb files can be loaded`.

Worth keeping from `extras`, if cheap: the part list (to hide, recolour
or swap a part by its triangle range) and the palette by name (to swap
team colours).

## Files to test with

`tests/golden/*.glb` in this repository: crate (12 triangles), table
(60), tree (70), house (76), robot (384). Each has a
`*.inspect.json` beside it with the size, triangle count, palette and
colour counts the loader should find. The official Khronos glTF
validator reports no errors, warnings, infos or hints on any of them.

A good first test in Duck Forge: load `crate.glb` and run Duck Forge's
own `assert_faces_point_outward` on it; then check it has 8 vertices,
12 triangles, 8 of them `#B07A3F` and 4 `#8A5A2B`.

## One thing to fix in Duck Forge while there

`Mesh::cube` in `src/render3d.rs` labels its faces in comments as
`front (-z)` and `back (+z)`, because -z is the face a default camera
sees. Duck Mold, by decision, says a model **faces forward (+z)**, so
its `front` is +z: the opposite word for the same face. Nothing breaks
(they are only comments, and the cube is symmetric), but the two
projects should use one word. Suggested: change the cube's comments to
`near (-z)` and `far (+z)`, and say in Duck Forge's guide, section 11,
that a model faces +z.
