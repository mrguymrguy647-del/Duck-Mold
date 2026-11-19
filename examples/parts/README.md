# Parts made elsewhere

Files that `examples/desk.toml` and `examples/signpost.toml` use as parts, to show importing.

| File | What it is |
|---|---|
| `lamp.gltf` | A small desk lamp, written as glTF without Duck Mold by `make_lamp.py`: three materials (`stand`, `shade`, `bulb`) with their own colours, and four pieces laid out four different ways (corner numbers of 16 and 8 bits, a fan of triangles, no corner numbers), placed by a matrix or by numbers, two of them inside a tilted arm. Its numbers are written inside it, as base64. |
| `make_lamp.py` | The script that wrote `lamp.gltf` (`python3 make_lamp.py lamp.gltf`). |
| `crate.glb` | Duck Mold's crate example, exported: a `.glb` Duck Mold wrote brings its palette with it. |
| `sign.gltf` | A signpost with pictures, written as glTF without Duck Mold by `make_sign.py`, for `examples/signpost.toml`: three materials (`post`, `board`, `cap`), each with a picture kept a different way. The post's is `sign_post.png`, a file beside it, repeated seven and a half times up the post; the board's is a PNG inside the file; the cap's is a JPEG written inside it as text. |
| `sign_post.png`, `sign_board.png`, `sign_cap.jpg` | The sign's three pictures. Only `sign_post.png` is read when the sign is used: the other two are inside `sign.gltf`, and are kept here to look at and for the script. |
| `make_sign.py` | The script that drew the two PNGs and wrote `sign.gltf` (`python3 make_sign.py sign.gltf`). With `--jpeg` it makes the JPEG again too, which needs the Pillow library. |

`duck-mold inspect examples/parts/lamp.gltf` reports on the lamp on its
own; `duck-mold render examples/parts/lamp.gltf` draws it.
