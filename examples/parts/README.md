# Parts made elsewhere

Files that `examples/desk.toml` uses as parts, to show importing.

| File | What it is |
|---|---|
| `lamp.gltf` | A small desk lamp, written as glTF without Duck Mold by `make_lamp.py`: three materials (`stand`, `shade`, `bulb`) with their own colours, and four pieces laid out four different ways (corner numbers of 16 and 8 bits, a fan of triangles, no corner numbers), placed by a matrix or by numbers, two of them inside a tilted arm. Its numbers are written inside it, as base64. |
| `make_lamp.py` | The script that wrote `lamp.gltf` (`python3 make_lamp.py lamp.gltf`). |
| `crate.glb` | Duck Mold's crate example, exported: a `.glb` Duck Mold wrote brings its palette with it. |

`duck-mold inspect examples/parts/lamp.gltf` reports on the lamp on its
own; `duck-mold render examples/parts/lamp.gltf` draws it.
