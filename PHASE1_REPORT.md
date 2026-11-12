# Duck Mold: Phase 1 report

Phase 1 is built, as designed in `DESIGN.md` section 17 and approved
with three changes, in seven steps, one commit each, on the branch
`claude/duck-mold-phase-1`.

## In short

- `duck-mold render` saves a picture of a model with four views on one
  sheet, without a window. The same recipe gives the same picture, byte
  for byte, on Windows and Linux.
- `duck-mold preview` opens a window to orbit and zoom around a model.
  It builds the model again every time the recipe is saved.
- Sockets sit on a shape's surface instead of floating beside it, and a
  wedge has a `slope` socket.
- A joined part turns with `twist`; `rotate` is only for parts that
  aren't joined.
- The `.glb` keeps every named socket, so a game can ask where
  `head.top` is. Duck Forge's copy of the reader is updated, and its
  turntable robot now wears a hat.
- glTF and `.glb` files from other programs can be used as parts, with
  their material colours.
- Tests run on Windows and Linux for every push.

Two new examples: `snowman.toml` (sockets on curved surfaces, a twisted
badge) and `desk.toml` (a lamp from a glTF file and a crate from a Duck
Mold `.glb`). The house has a skylight on its roof's `slope`.

**One thing to know first:** both Phase 0 pull requests were still open
when this phase started (mrguymrguy647-del/Duck-Mold#1 and
mrguymrguy647-del/Duck-Forge#9), although the brief said they were
merged. So Phase 1 is *stacked* on them:

- The Duck Mold branch starts from Phase 0's last commit. Its pull
  request, mrguymrguy647-del/Duck-Mold#2, targets the Phase 0 branch, so
  it shows only Phase 1.
- Duck Forge's sockets pull request, mrguymrguy647-del/Duck-Forge#10,
  targets the loader branch of #9 in the same way.

Merge #1 and #9 first. Then change the base of #2 and #10 to `main`
(GitHub may do it for you when the old branches are deleted).

## What works

### 1. Tests on Windows and Linux for every push

`.github/workflows/ci.yml` is Duck Quill's workflow (the same pinned
Rust, 1.97.0, on `windows-latest` and `ubuntu-latest`) with three
additions:

- a formatting check;
- from step 7, a second test run and a second clippy run without the
  preview window (`--no-default-features`).

Every step was green on both systems, the last one with and without
the window.

### 2. `duck-mold render`

```bash
duck-mold render examples/robot.toml              # examples/robot.png
duck-mold render examples/robot.toml -o sheet.png --size 384 --json
```

- **The sheet**:
  - Front, right side, back and three-quarter views, two by two, each a
    square (256 pixels by default; `--size` takes 128 to 2048), with a
    line of the model's name, size and triangle count under them.
  - Every view is at the same scale, and is seen from far away through
    a narrow 20° view, so it looks nearly flat.
  - `render` also draws `.obj`, `.gltf` and `.glb` files as they are.
- **Drawn by Duck Forge's CPU renderer**:
  - It lives in a new crate, `duck-mold-view`. Duck Forge is a git
    dependency pinned to commit `76850ee` on its `main`, with its GPU
    and sound off.
  - `duck-mold-core` still knows nothing of Duck Forge.
- **Same picture every time**:
  - Duck Mold turns the model itself, with its exact sine and cosine, so
    Duck Forge's camera never turns.
  - The one call left that depends on the computer is Duck Forge's
    `tan` for its zoom. The seven examples' pictures are golden files,
    and CI compared them byte for byte on Windows and Linux at every
    step: they have always agreed.

### 3. Sockets that know the shape

**The rule:** a built-in socket that was in the air beside a shape
moves straight in until it touches the shape's faces, and then faces
straight out of the surface there. One that was already on the surface
doesn't move.

- A cone's `left`, `right`, `front` and `back` are now halfway up its
  slant.
- A wedge's `top` and `front` are now the middle of its slope, which
  also has a name of its own: `slope`.
- `on` + `shift` sockets on round or sloped sides follow the same rule.
  So eyes on a ball sit on it and look out of it.
- A `facing` written in the recipe still wins over the surface's
  direction, which stands a chimney upright on a slope.
- A shift that puts a socket beside the shape, where there's nothing to
  sit on, is now an error.

The search goes through the faces actually drawn, so it is exact for
any number of `segments`.

The five Phase 0 examples built to exactly the same bytes. Two tests
changed, as the design said: they used a wedge's `top` and `front` to
check the joining rules, and now use its unchanged sockets.

### 4. `twist`

- `twist = 90` turns a joined part around its join.
- `rotate = 90` on a joined part is an error that shows `twist = 90`.
- `twist` on a free part is an error that shows `rotate = [0, 90, 0]`.
- Both errors carry the fix as their suggestion. The recipe version
  stays 0.

### 5. Sockets in the `.glb`, reader version 3

- `extras.duck_mold.sockets` lists every **named** part's sockets, where
  they ended up. Following your change, unnamed parts and the parts
  inside them are left out.
  - Naming: `top` for the model's own, `head.top` for a part's,
    `left_arm/hand.bottom` inside an assembly, `left_arm.grip` for an
    assembly's own.
  - Positions are in glTF's directions, like the corners.
- The robot has 187 named sockets, and its file grew from 35.7 KB to
  50 KB.
- Duck Mold reads the sockets back from a `.glb`.
- `duck-mold inspect --sockets` lists them, with where each is and the
  way it faces.
- The extras version stays 0, and the Khronos validator still reports
  nothing on any example.
- **The reader went to version 3.** It is copied into Duck Forge byte
  for byte; the handoff note's `diff` prints nothing.
- **Duck Forge** (mrguymrguy647-del/Duck-Forge#10):
  - `GlbModel::socket(name)`, and `position()` and `direction()` on a
    socket.
  - New fixtures from Duck Mold, and a test against the robot's full
    socket list.
  - The turntable robot wears a top hat put on `head.top` at run time.
    It is tall enough to cover the antenna that stands there too. A test
    checks it stays centred on the head as the robot turns.
  - The cheat-sheet test, the guide, the README, the skill and the
    decisions note are updated.
  - Locally, 620 tests pass with everything on and 577 headless.

### 6. glTF and `.glb` import

- `file = "lamp.gltf"` or `"lamp.glb"` makes a static part. A new
  reader, with no new library, handles:
  - `.glb` files, and `.gltf` files with their numbers in a `.bin`
    beside them or as base64 inside;
  - every mesh in the main scene, placed by its objects (by numbers or
    by a matrix, nested, mirrored);
  - triangles as lists, strips or fans, with corner numbers of any size
    or none.
- **Colour, with your change:**
  - Material names are face names for `colors`.
  - A material the recipe leaves uncoloured keeps its own base colour.
    That colour is added to the palette under the material's name, and
    a warning lists what was kept.
  - If the palette already has that name, the palette's colour is used,
    and the warning says so when the two differ.
  - A `.glb` Duck Mold wrote brings its palette, as a recipe used as a
    part does (conflicts included).
- **Left out with a note:** lines, points, skins, animations, shape keys,
  cameras and textures.
- **Refused with a sentence:** files that require an extension
  (compression), packed corners, sparse accessors, and glTF 1.
- `inspect` and `render` read glTF too.
- **The example:**
  - `examples/desk.toml` uses `parts/lamp.gltf`, a lamp written by a
    short script (`make_lamp.py`) rather than by Duck Mold, so the
    import is tested on a file Duck Mold didn't write. It has four
    pieces laid out four different ways, and Khronos's validator finds
    it clean.
  - It also uses `parts/crate.glb`.
  - The recipe paints the lamp's stand and leaves its shade and bulb to
    keep the file's colours. That makes the desk the one example with a
    warning, on purpose.

### 7. `duck-mold preview`

```bash
duck-mold preview examples/robot.toml
```

- **Controls:** drag with the left button to orbit, roll the wheel to
  zoom, Escape to quit.
- **Watching files:**
  - Twice a second it checks the recipe and every file it read
    (palettes, recipes used as parts, meshes, a `.gltf`'s `.bin`), by
    time and size. `load` now lists those files.
  - When one changes, it builds the model again and keeps the view
    where it was.
  - Mistakes show in red over the last good model, wrapped to fit, and
    warnings in amber. Both are printed in the terminal too.
- **How it's built:**
  - Duck Forge runs the window and draws with its CPU renderer.
  - The window is a feature (`preview`), on by default. Built without
    it, everything else still works, and CI tests both ways.
  - Without a screen it says so in one line, and points to `render`.
- **Tests:** eight headless tests drive it through Duck Forge's
  `Simulation`: drags, the wheel, saves, broken saves, palette files and
  Escape.
- **Tried for real** on a virtual X screen: the robot drawn, a colour
  change saved and shown, and a broken save shown in red over the last
  good robot (`docs/preview.png` is from that run).

## How to check it yourself

```bash
cargo test --workspace
```

```bash
cargo run --release -- render examples/desk.toml
```

```bash
cargo run --release -- preview examples/snowman.toml
```

```bash
cargo run --release -- inspect examples/robot.toml --sockets
```

## Numbers

| | |
|---|---|
| Tests | 260, all passing, with the window and without (Phase 0 ended with 199) |
| Examples | 7: crate, table, tree, house (88 triangles now), robot, snowman (600), desk (114) |
| Golden files | 21: a `.glb`, a report and a picture for each example |
| Phase 1's change | 62 files, about 8,200 lines added (code, tests, docs, golden files) |
| The robot's `.glb` | 50 KB with its 187 sockets (35.7 KB before) |
| Duck Forge | 620 tests with everything on, 577 headless; none fail |

## Known limits

- **Pictures:**
  - Duck Forge's `tan` is the one maths call that could, in theory,
    differ between computers. Windows and Linux agree; macOS isn't in
    CI. If they ever differ, the fix is a `tan` of Duck Forge's own.
  - The sheet has no ground or shadow, and a long thin model is small
    in it.
- **Surface sockets:**
  - Only the five shapes have them. A part made from a file, and an
    assembly, keep their sockets on the box around them.
  - `shift = ±1` on a round side lands on an edge, and faces halfway
    between the two faces that meet there.
- **Sockets in the `.glb`:**
  - A socket has a point and a direction, but no "up". A game that puts
    something on it chooses its own roll; Duck Mold's join rule is only
    in Duck Mold.
  - There is no switch to leave sockets out of a file.
- **glTF import:**
  - Only a material's base colour is used. A textured model, which
    usually has a white base colour, comes in white unless the recipe
    colours it.
  - A non-Duck-Mold file's vertex colours are not read.
  - OBJ files still don't bring colours: their `.mtl` file isn't read,
    so the new keep-the-file's-colour rule only applies to glTF.
- **The preview:**
  - On Linux it needs X11 and the `libxkbcommon-x11` library when the
    window opens; building needs neither. This container lacked the
    library until it was installed for the real-window check, and Duck
    Forge's error names the package.
  - There is no panning; it orbits the model's middle.
  - It notices saves by checking twice a second, not instantly.
- **`twist`** changed the recipe language without raising `version`.
  That's fine while nothing is released, and old recipes get an error
  that shows the fix.

## What changed from the approved design

- **Render:**
  - A line one pixel wide runs between the views, so the sheet is
    513 × 536, not 512 square.
  - `--size` takes 128 to 2048 (below 128 the names don't fit).
  - The caption is the model's name, size and triangle count, taken
    from `inspect`'s summary.
- **Surface sockets:** standing something upright on a slope uses `on`
  with `facing = "up"`, not only `at`, because a given `facing` wins.
- **Twist:** both errors are `conflict`s (like `move` on a joined
  part), with the fix as their suggestion.
- **Sockets in the `.glb`:**
  - As you asked, unnamed parts' sockets are left out.
  - Also added: Duck Mold reads sockets back from a `.glb`, so
    `inspect --sockets` works on a file as well as a recipe.
- **glTF:**
  - As you asked, uncoloured materials keep their own colour, added to
    the palette by name, with a warning.
  - Also added: when the palette already has that name, its colour is
    used.
  - `inspect` and `render` read glTF, not only recipes.
  - The lamp is written by a committed script, so it can be made again.
- **Preview:**
  - A perspective view (40°), starting from the front right and a
    little above, lit from over your shoulder.
  - Warnings show as well as mistakes, and long lines wrap.
- **Examples:** a snowman was added for steps 3 and 4, and the house
  gained a skylight. So the house's golden files changed in step 3 on
  purpose, after the five Phase 0 examples had been shown to build to
  the same bytes.
- **Branches:** Phase 1 is a new branch from Phase 0's last commit, not
  from `main`, because the Phase 0 pull requests weren't merged. Duck
  Forge is pinned to a commit on its `main` as asked. That commit
  doesn't have the loader yet, but `render` and `preview` don't need
  it.

## What Phase 2 should start with

1. **Merge the stack**: Duck-Mold #1 then #2, and Duck-Forge #9 then
   #10. Then move Duck Mold's Duck Forge pin to a commit that has the
   loader, so the two can share fixtures.
2. **Sockets with an "up"**, written into the `.glb`, and a Duck Forge
   helper that gives the `Transform` to put a mesh on a socket. Today a
   game has to work out the roll itself.
3. **Surface sockets for mesh parts and assemblies**: the same
   "straight in from the box" search through all their faces, with a
   clear fallback where the line misses.
4. **OBJ colours from `.mtl`**, under the same keep-the-file's-colour
   rule as glTF.
5. **The preview, a little further**: show sockets (the names a game
   asks for), pan, and a ground grid. Still no editing.
6. **macOS in CI**, to put the `tan` question to rest. Or a `tan` of
   Duck Forge's own, if you'd rather not wait for a disagreement.
7. Then the bigger items `DESIGN.md` keeps room for: bones from socket
   joins, colliders and LODs.
