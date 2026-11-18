# Duck Mold: Phase 7a report

Phase 7a was polish and getting ready for games: everything the audit
(`POLISH_AUDIT.md`) found, plus far-away versions, a physics body,
picking by hit shape, a command line made for agents, and Duck Mold's
first real release, **0.7.0**, with an installer.

The design is `DESIGN.md` section 23. Where the build differs from it,
the section's *As built* notes say how and why; the main ones are under
"Known limits" below. Phase 7b (Duck Quill textures, Duck Mold inside
Duck Forge's Studio, an example using all three apps) has its own design,
still to be approved.

## In short

| Step | What it gives | Where to see it |
|---|---|---|
| 1. Never lose work | Files written beside the old one and swapped in; Undo kept on disk and carried across restarts; a wall in Undo where another program changed the file; a recipe deleted while open can be saved again; a crash note | Edit menu's Undo tooltip, the bar over a deleted recipe |
| 2. Layout and messages | A status bar and a problems list above it; one scrolling right column; the Model section; plain TOML messages; greyed buttons that say why; Remove that asks | Every screen |
| 3. The look | 40 drawn icons, one toolbar, an axis gizmo, View > Interface size (90 to 200%), a wider start page | Every screen |
| 4. Speed | Frames about twice as fast at 2560 x 1440, edits about twice as fast | "Speed" below |
| 5. Fit between keys | `fit-ground` adds a key where the curve between two keys dips into the ground; a `dips_between_keys` warning | The robot's and gnome's jump, the cart's roll |
| 6. A physics body | `GlbModel::physics_body(feet)` in Duck Forge, a block round the hit shapes | Duck Forge's `robot_click` walks into a wall |
| 7. Picking by hit shape | With Hits on, a click picks what a game's click would; `inspect --ray` | Status bar: "A game's click here hits: head" |
| 8. Far-away versions | `lods = "auto"`: two simpler versions, in the `.glb` (`MSFT_lod`), drawn by distance in Duck Forge | View > Far version; `render --far 2` |
| 9. The command line for agents | One-line `--json`, `edit --json` that leads with the edit, `edit --batch`, nearest-first part suggestions, a grouped `edit --help` | `duck-mold edit --help` |
| 10. The release | Version 0.7.0, `DuckMoldSetup-0.7.0.exe`, `RELEASE_NOTES.md`, an install test | GitHub Releases |

## What works

### Never lose work (step 1)

- **Safe writes.** Every file Duck Mold writes (recipes, `.glb`, pictures)
  goes beside the real one, is flushed to disk, then swapped in. A full
  disk or a locked file leaves the old file whole, and says why in plain
  words.
- **Undo survives.** 100 steps (at most 5 MB) per recipe are kept on disk
  and carry on after the Studio is closed and opened again.
- **Undo never writes over someone else's change.** A change from another
  program (or made while the Studio was closed) is a wall: Undo stops
  there and says so.
- **A recipe deleted or moved while open** keeps its model on screen, with
  a bar to save it again, save it elsewhere, or close it.
- **A file another program is still saving** isn't shown as a mistake
  until it has stayed the same for a moment.
- **A crash** writes `crash-<date>.txt`; the next start says so once.

### The Studio (steps 2 and 3)

- A status bar: "No problems", "1 warning", "2 mistakes"; a click opens
  the list above it, and a new mistake opens it by itself.
- One right column that scrolls: the part's properties, sockets, limits,
  hit shape, far versions, then the palette. With nothing selected, the
  Model section: name, size, parts and triangles, poses, moves, hit
  shapes, far versions.
- Drawn icons (the same on every computer, sharp at every size), a
  toolbar down the picture's left, an axis gizmo and labelled views at its
  right, switches (Grid, Wire, Shadow, Hits) at its bottom right.
- Export, Render picture and Save again say what they wrote, with "Show in
  folder".
- View > Interface size, remembered; text never under 12 points.
- The start page says why a recent model doesn't build, and "not found"
  with Forget for one that's gone.

### Games (steps 5 to 8)

- **`fit-ground`** stands every key's pose on the ground, then plays the
  animation 30 times a second and adds a key wherever it still sinks more
  than 3 mm (at most 6). The robot's jump gained `jump_2` and `jump_3`.
- **A physics body** (Duck Forge): `robot.model().physics_body(feet)` is a
  block round the hit shapes as the model stands at rest;
  `stands_at(&body)` says where to draw it.
- **Picking by hit shape**: with Hits on, a click in the Studio picks the
  part whose hit shape the ray from the eye meets first, using Duck
  Forge's own ray test. `duck-mold inspect --ray x,y,z --toward x,y,z`
  answers the same for an agent, posed or mid-animation.
- **Far-away versions**: `lods = "auto"` gives Far 1 (half the sides on
  round shapes, one-step bevels) at 8 times the model's largest side and
  Far 2 (a quarter, square edges, stairs as a wedge, parts under 4% left
  out) at 20 times. Only triangles change: every part stays exactly
  where the model has it. `lod = "keep"` keeps a part whole. They are in
  the `.glb` as glTF's `MSFT_lod`; Khronos' validator reports no errors or
  warnings (only "info" notes that it can't check the extension). Duck
  Forge's `robot.draw_mesh(canvas, &place, &camera)` picks the version by
  distance. The temple, barn, robot and gnome have them; new models start
  with them.

| The robot | Far 1 (from 14.6 m) | Far 2 (from 36.6 m) |
|---|---|---|
| ![The robot](tests/golden/robot.png) | ![Far 1](tests/golden/robot.far1.png) | ![Far 2](tests/golden/robot.far2.png) |
| 384 triangles | 288 | 276 |

| The temple | Far 1 (from 40.3 m) | Far 2 (from 100.8 m) |
|---|---|---|
| ![The temple](tests/golden/temple.png) | ![Far 1](tests/golden/temple.far1.png) | ![Far 2](tests/golden/temple.far2.png) |
| 686 triangles | 398 | 150 |

### The command line for agents (step 9)

- `--json` prints one line; `--pretty` spreads it out. The robot's
  `inspect --json` is 1 line instead of 882.
- `edit --json` is `{"ok":true,"edit":{"command":"add",...},"summary":...,
  "lines":[198,206],"model":"robot: ...","warnings":[],"errors":[]}`;
  `--report` adds `inspect`'s report. A text `edit` prints one line and
  any problems.
- `edit --batch edits.txt` (or `-`): one edit a line, made in turn, the
  file written once or not at all; a refusal names its line
  (`edits.txt line 3: ...`).
- A misspelt part lists every part, nested ones too, nearest first, with
  the nearest as "Did you mean".
- `edit --help` lists all 47 edits, grouped; `add-socket --facing`.

### The release (step 10)

- **Version 0.7.0**, shown in the title bar, About, the top bar, the start
  page, the crash note and `--version`.
- **`DuckMoldSetup-0.7.0.exe`** (Inno Setup, from Duck Forge Studio's
  script): installs for the person alone, without an administrator, in
  `%LOCALAPPDATA%\Programs\Duck Mold`; a Start menu entry; an offered
  desktop shortcut; the examples and guides; a ticked box, *"Let the
  command line and AI agents find duck-mold"*, that adds the folder to
  the person's PATH (and uninstalling takes it away); an uninstaller; no
  file association. The zip stays.
- **`RELEASE_NOTES.md`**, in plain words. The release's text is its
  version's section; a test fails if the current version has none.
- **The install test** in the release workflow, on GitHub's Windows
  machine: install silently, check the files, the Start menu entry, the
  Settings > Apps entry and the PATH; run the installed `duck-mold
  --version`, `inspect` on an installed example, and the Studio's
  `--version`; install again over it and check a model in Documents is
  kept and the PATH entry isn't doubled; uninstall and check the folder,
  the Start menu entry and the PATH entry are gone and the model stays.
  It passed on its first run.

## Speed

The speed test (`crates/duck-mold-view/tests/speed.rs`) uses a model of
50 parts and 21,660 triangles, release build, on this 4-core 2.1 GHz
machine. Its times wander by about 20% from run to run; these are
typical. CI prints the same test's times on GitHub's Windows machine.

| What | 0.6.0 (before) | After step 4 | 0.7.0 (final) | Target |
|---|---|---|---|---|
| A frame, 1280 x 720 window (780 x 590 picture) | 10 to 14 ms | 7.5 to 9 ms | 6 to 7.5 ms | under 10 ms: **met** |
| A frame, 2560 x 1440 window (1560 x 1180 picture) | 30 to 37 ms | 15 to 17.5 ms | 13 to 16 ms | under 16 ms: **met** |
| One step of a drag, with its frame | 23 to 24 ms | 15 to 19 ms | 14 to 16 ms | under 12 ms: **not met** |
| One edit from the panel | 57 to 69 ms | 33 to 35 ms | 34 ms | under 40 ms: **met** |
| One edit from the panel, model with `lods = "auto"` | (no far versions) | 52 ms | 39 to 44 ms | under 40 ms: **about met** |
| A click that picks | | | 1.1 to 1.3 ms | |

What made the difference: keeping the canvas between frames, working the
shadow out once per build, working an edit out from the model the Studio
already has, and drawing smaller "draft" frames while a drag is slow.

**A bug the drafts brought, fixed while finishing:** the pointer was
measured in the picture's pixels, and a draft's picture is smaller, so on
a slow computer (CI's Linux machine) a handle dragged partly in draft
frames moved by the wrong amount. The pointer is now always measured at
full size; a test drags handles with every frame a draft.

Far versions made a model that has them take half as long again to
build. Checking an edit now builds the model without them (a drag's
preview never shows them), which took a drag step's model work from 10.4
back to 7 ms. A panel edit still pays for them once, when the Studio
reads the file back, because the Model section shows their triangle
counts.

## Tests

- **695 tests pass** in Duck Mold on Linux (2 ignored on purpose: the
  speed test and the audit, run by hand). CI runs them on Windows and
  Linux, with `cargo fmt --check` and `cargo clippy -D warnings`.
- New this phase: safe writes and the crash note; Undo on disk and its
  walls; settling files; every new control in the screen tests; pictures
  of every screen the tests look at (69 stored); `fit-ground` between keys; picking
  by hit shape against Duck Forge's ray test; far versions in the recipe,
  the assembler, `inspect`, the `.glb`, the shared reader, `render --far`
  and the Studio, with golden pictures of every far version; the CLI's
  one-line JSON, `edit --json`, `--batch`, the suggestions, the grouped
  help and `add-socket --facing`; the release notes' section.
- **Golden files**: every example's `.glb`, report and picture, with the
  far versions' pictures; files without `lods` are byte for byte as before.
- **Duck Forge** (pull requests [#16](https://github.com/mrguymrguy647-del/Duck-Forge/pull/16)
  and [#17](https://github.com/mrguymrguy647-del/Duck-Forge/pull/17),
  both merged): the physics body, far versions read and drawn by
  distance, the general glTF reader drawing only the scene; its fixtures
  are Duck Mold's current goldens. Duck Mold is pinned to that main
  (`3a21f0a`), and the shared reader (version 8) is the same, byte for
  byte, in both.

## Known limits

- **A drag step on a very big model is 14 to 16 ms**, not under 12.
  Getting there needs the picture drawn on several cores, or Duck Forge
  drawing faster. Drafts keep it smooth.
- **A panel edit on a big model with far versions is 39 to 44 ms**, at
  the 40 ms target rather than under it.
- **The program is unsigned.** Windows says "Windows protected your PC"
  once; the release notes and READ ME FIRST say what to click.
- **The installer is tested by GitHub's Windows machine, silently.** No
  person has clicked through it on a real PC yet.
- **The house has no far versions.** It is all boxes, so its far versions
  came out the same as itself: four examples have them, not five.
- **No example leaves a part out of Far 2**: their smallest parts are just
  over 4% of their size. The rule is tested on a made-up figure.
- **A far version can open a gap of millimetres** between parts that
  touch in the model (the gnome's nose and its rounder beard, 7 mm in
  Far 1). Invisible at that distance; `render --far` doesn't warn about
  it, and `--json` still lists it under `floating`.
- **In Duck Forge, every far version is posed as the model moves**, not
  only the one drawn (posing only one would need a lock in
  `AnimatedModel`). `mesh_mut` changes only the model itself.
- **Other glTF programs and `MSFT_lod`.** The extension doesn't say
  whether a far object replaces a node's children too. Duck Mold's far
  objects hold only their own part's triangles; a program that swaps the
  whole subtree would drop the parts hanging from it when far off.
- **`--batch` checks each line as it is made**, so a list whose middle
  would be a mistake (fixed by a later line) is refused; the design said
  "checked once at the end". Each line sees the model the earlier ones
  made, which is what most lists need.
- **One-line `--json` changes the output** for any script that read it
  line by line: read it whole, or pass `--pretty`.
- From earlier phases, unchanged and listed in the audit: faces where two
  parts meet are kept; a socket has no "up"; only a box can be shaped;
  hiding isn't saved; no IK; a stretched model is hit-tested by its
  largest scale.

## Every screen, before and after

The audit drew 79 screens at 1280 x 720 and at 2560 x 1440 with Duck Mold 0.6.0 (`docs/audit/`). The same test, `crates/duck-mold-studio/tests/audit.rs`, drew them again with 0.7.0 (`docs/audit-7a/`), both cut to 128 colours. Every one of the 158 pictures changed. Click a picture for it at full size.

### The start page

**01. Start first time**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/01-start-first-time.png" width="400">](docs/audit/1280x720/01-start-first-time.png) | [<img src="docs/audit-7a/1280x720/01-start-first-time.png" width="400">](docs/audit-7a/1280x720/01-start-first-time.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/01-start-first-time.png" width="400">](docs/audit/2560x1440/01-start-first-time.png) | [<img src="docs/audit-7a/2560x1440/01-start-first-time.png" width="400">](docs/audit-7a/2560x1440/01-start-first-time.png) |

**02. Start recent with missing and broken**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/02-start-recent-with-missing-and-broken.png" width="400">](docs/audit/1280x720/02-start-recent-with-missing-and-broken.png) | [<img src="docs/audit-7a/1280x720/02-start-recent-with-missing-and-broken.png" width="400">](docs/audit-7a/1280x720/02-start-recent-with-missing-and-broken.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/02-start-recent-with-missing-and-broken.png" width="400">](docs/audit/2560x1440/02-start-recent-with-missing-and-broken.png) | [<img src="docs/audit-7a/2560x1440/02-start-recent-with-missing-and-broken.png" width="400">](docs/audit-7a/2560x1440/02-start-recent-with-missing-and-broken.png) |

**03. Start file not there**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/03-start-file-not-there.png" width="400">](docs/audit/1280x720/03-start-file-not-there.png) | [<img src="docs/audit-7a/1280x720/03-start-file-not-there.png" width="400">](docs/audit-7a/1280x720/03-start-file-not-there.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/03-start-file-not-there.png" width="400">](docs/audit/2560x1440/03-start-file-not-there.png) | [<img src="docs/audit-7a/2560x1440/03-start-file-not-there.png" width="400">](docs/audit-7a/2560x1440/03-start-file-not-there.png) |

### The examples, opened

**10. Example barn**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-barn.png" width="400">](docs/audit/1280x720/10-example-barn.png) | [<img src="docs/audit-7a/1280x720/10-example-barn.png" width="400">](docs/audit-7a/1280x720/10-example-barn.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-barn.png" width="400">](docs/audit/2560x1440/10-example-barn.png) | [<img src="docs/audit-7a/2560x1440/10-example-barn.png" width="400">](docs/audit-7a/2560x1440/10-example-barn.png) |

**10. Example cart**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-cart.png" width="400">](docs/audit/1280x720/10-example-cart.png) | [<img src="docs/audit-7a/1280x720/10-example-cart.png" width="400">](docs/audit-7a/1280x720/10-example-cart.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-cart.png" width="400">](docs/audit/2560x1440/10-example-cart.png) | [<img src="docs/audit-7a/2560x1440/10-example-cart.png" width="400">](docs/audit-7a/2560x1440/10-example-cart.png) |

**10. Example chair**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-chair.png" width="400">](docs/audit/1280x720/10-example-chair.png) | [<img src="docs/audit-7a/1280x720/10-example-chair.png" width="400">](docs/audit-7a/1280x720/10-example-chair.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-chair.png" width="400">](docs/audit/2560x1440/10-example-chair.png) | [<img src="docs/audit-7a/2560x1440/10-example-chair.png" width="400">](docs/audit-7a/2560x1440/10-example-chair.png) |

**10. Example crate**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-crate.png" width="400">](docs/audit/1280x720/10-example-crate.png) | [<img src="docs/audit-7a/1280x720/10-example-crate.png" width="400">](docs/audit-7a/1280x720/10-example-crate.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-crate.png" width="400">](docs/audit/2560x1440/10-example-crate.png) | [<img src="docs/audit-7a/2560x1440/10-example-crate.png" width="400">](docs/audit-7a/2560x1440/10-example-crate.png) |

**10. Example desk**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-desk.png" width="400">](docs/audit/1280x720/10-example-desk.png) | [<img src="docs/audit-7a/1280x720/10-example-desk.png" width="400">](docs/audit-7a/1280x720/10-example-desk.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-desk.png" width="400">](docs/audit/2560x1440/10-example-desk.png) | [<img src="docs/audit-7a/2560x1440/10-example-desk.png" width="400">](docs/audit-7a/2560x1440/10-example-desk.png) |

**10. Example dog**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-dog.png" width="400">](docs/audit/1280x720/10-example-dog.png) | [<img src="docs/audit-7a/1280x720/10-example-dog.png" width="400">](docs/audit-7a/1280x720/10-example-dog.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-dog.png" width="400">](docs/audit/2560x1440/10-example-dog.png) | [<img src="docs/audit-7a/2560x1440/10-example-dog.png" width="400">](docs/audit-7a/2560x1440/10-example-dog.png) |

**10. Example door**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-door.png" width="400">](docs/audit/1280x720/10-example-door.png) | [<img src="docs/audit-7a/1280x720/10-example-door.png" width="400">](docs/audit-7a/1280x720/10-example-door.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-door.png" width="400">](docs/audit/2560x1440/10-example-door.png) | [<img src="docs/audit-7a/2560x1440/10-example-door.png" width="400">](docs/audit-7a/2560x1440/10-example-door.png) |

**10. Example doughnut**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-doughnut.png" width="400">](docs/audit/1280x720/10-example-doughnut.png) | [<img src="docs/audit-7a/1280x720/10-example-doughnut.png" width="400">](docs/audit-7a/1280x720/10-example-doughnut.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-doughnut.png" width="400">](docs/audit/2560x1440/10-example-doughnut.png) | [<img src="docs/audit-7a/2560x1440/10-example-doughnut.png" width="400">](docs/audit-7a/2560x1440/10-example-doughnut.png) |

**10. Example duck**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-duck.png" width="400">](docs/audit/1280x720/10-example-duck.png) | [<img src="docs/audit-7a/1280x720/10-example-duck.png" width="400">](docs/audit-7a/1280x720/10-example-duck.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-duck.png" width="400">](docs/audit/2560x1440/10-example-duck.png) | [<img src="docs/audit-7a/2560x1440/10-example-duck.png" width="400">](docs/audit-7a/2560x1440/10-example-duck.png) |

**10. Example gate**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-gate.png" width="400">](docs/audit/1280x720/10-example-gate.png) | [<img src="docs/audit-7a/1280x720/10-example-gate.png" width="400">](docs/audit-7a/1280x720/10-example-gate.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-gate.png" width="400">](docs/audit/2560x1440/10-example-gate.png) | [<img src="docs/audit-7a/2560x1440/10-example-gate.png" width="400">](docs/audit-7a/2560x1440/10-example-gate.png) |

**10. Example gnome**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-gnome.png" width="400">](docs/audit/1280x720/10-example-gnome.png) | [<img src="docs/audit-7a/1280x720/10-example-gnome.png" width="400">](docs/audit-7a/1280x720/10-example-gnome.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-gnome.png" width="400">](docs/audit/2560x1440/10-example-gnome.png) | [<img src="docs/audit-7a/2560x1440/10-example-gnome.png" width="400">](docs/audit-7a/2560x1440/10-example-gnome.png) |

**10. Example house**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-house.png" width="400">](docs/audit/1280x720/10-example-house.png) | [<img src="docs/audit-7a/1280x720/10-example-house.png" width="400">](docs/audit-7a/1280x720/10-example-house.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-house.png" width="400">](docs/audit/2560x1440/10-example-house.png) | [<img src="docs/audit-7a/2560x1440/10-example-house.png" width="400">](docs/audit-7a/2560x1440/10-example-house.png) |

**10. Example mug**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-mug.png" width="400">](docs/audit/1280x720/10-example-mug.png) | [<img src="docs/audit-7a/1280x720/10-example-mug.png" width="400">](docs/audit-7a/1280x720/10-example-mug.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-mug.png" width="400">](docs/audit/2560x1440/10-example-mug.png) | [<img src="docs/audit-7a/2560x1440/10-example-mug.png" width="400">](docs/audit-7a/2560x1440/10-example-mug.png) |

**10. Example robot**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-robot.png" width="400">](docs/audit/1280x720/10-example-robot.png) | [<img src="docs/audit-7a/1280x720/10-example-robot.png" width="400">](docs/audit-7a/1280x720/10-example-robot.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-robot.png" width="400">](docs/audit/2560x1440/10-example-robot.png) | [<img src="docs/audit-7a/2560x1440/10-example-robot.png" width="400">](docs/audit-7a/2560x1440/10-example-robot.png) |

**10. Example rock**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-rock.png" width="400">](docs/audit/1280x720/10-example-rock.png) | [<img src="docs/audit-7a/1280x720/10-example-rock.png" width="400">](docs/audit-7a/1280x720/10-example-rock.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-rock.png" width="400">](docs/audit/2560x1440/10-example-rock.png) | [<img src="docs/audit-7a/2560x1440/10-example-rock.png" width="400">](docs/audit-7a/2560x1440/10-example-rock.png) |

**10. Example slide**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-slide.png" width="400">](docs/audit/1280x720/10-example-slide.png) | [<img src="docs/audit-7a/1280x720/10-example-slide.png" width="400">](docs/audit-7a/1280x720/10-example-slide.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-slide.png" width="400">](docs/audit/2560x1440/10-example-slide.png) | [<img src="docs/audit-7a/2560x1440/10-example-slide.png" width="400">](docs/audit-7a/2560x1440/10-example-slide.png) |

**10. Example snowman**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-snowman.png" width="400">](docs/audit/1280x720/10-example-snowman.png) | [<img src="docs/audit-7a/1280x720/10-example-snowman.png" width="400">](docs/audit-7a/1280x720/10-example-snowman.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-snowman.png" width="400">](docs/audit/2560x1440/10-example-snowman.png) | [<img src="docs/audit-7a/2560x1440/10-example-snowman.png" width="400">](docs/audit-7a/2560x1440/10-example-snowman.png) |

**10. Example table**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-table.png" width="400">](docs/audit/1280x720/10-example-table.png) | [<img src="docs/audit-7a/1280x720/10-example-table.png" width="400">](docs/audit-7a/1280x720/10-example-table.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-table.png" width="400">](docs/audit/2560x1440/10-example-table.png) | [<img src="docs/audit-7a/2560x1440/10-example-table.png" width="400">](docs/audit-7a/2560x1440/10-example-table.png) |

**10. Example temple**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-temple.png" width="400">](docs/audit/1280x720/10-example-temple.png) | [<img src="docs/audit-7a/1280x720/10-example-temple.png" width="400">](docs/audit-7a/1280x720/10-example-temple.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-temple.png" width="400">](docs/audit/2560x1440/10-example-temple.png) | [<img src="docs/audit-7a/2560x1440/10-example-temple.png" width="400">](docs/audit-7a/2560x1440/10-example-temple.png) |

**10. Example tree**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-tree.png" width="400">](docs/audit/1280x720/10-example-tree.png) | [<img src="docs/audit-7a/1280x720/10-example-tree.png" width="400">](docs/audit-7a/1280x720/10-example-tree.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-tree.png" width="400">](docs/audit/2560x1440/10-example-tree.png) | [<img src="docs/audit-7a/2560x1440/10-example-tree.png" width="400">](docs/audit-7a/2560x1440/10-example-tree.png) |

**10. Example window wall**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/10-example-window_wall.png" width="400">](docs/audit/1280x720/10-example-window_wall.png) | [<img src="docs/audit-7a/1280x720/10-example-window_wall.png" width="400">](docs/audit-7a/1280x720/10-example-window_wall.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/10-example-window_wall.png" width="400">](docs/audit/2560x1440/10-example-window_wall.png) | [<img src="docs/audit-7a/2560x1440/10-example-window_wall.png" width="400">](docs/audit-7a/2560x1440/10-example-window_wall.png) |

### Parts, sockets and tools

**20. Part selected**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/20-part-selected.png" width="400">](docs/audit/1280x720/20-part-selected.png) | [<img src="docs/audit-7a/1280x720/20-part-selected.png" width="400">](docs/audit-7a/1280x720/20-part-selected.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/20-part-selected.png" width="400">](docs/audit/2560x1440/20-part-selected.png) | [<img src="docs/audit-7a/2560x1440/20-part-selected.png" width="400">](docs/audit-7a/2560x1440/20-part-selected.png) |

**21. Part from definition**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/21-part-from-definition.png" width="400">](docs/audit/1280x720/21-part-from-definition.png) | [<img src="docs/audit-7a/1280x720/21-part-from-definition.png" width="400">](docs/audit-7a/1280x720/21-part-from-definition.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/21-part-from-definition.png" width="400">](docs/audit/2560x1440/21-part-from-definition.png) | [<img src="docs/audit-7a/2560x1440/21-part-from-definition.png" width="400">](docs/audit-7a/2560x1440/21-part-from-definition.png) |

**22. Part inside definition**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/22-part-inside-definition.png" width="400">](docs/audit/1280x720/22-part-inside-definition.png) | [<img src="docs/audit-7a/1280x720/22-part-inside-definition.png" width="400">](docs/audit-7a/1280x720/22-part-inside-definition.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/22-part-inside-definition.png" width="400">](docs/audit/2560x1440/22-part-inside-definition.png) | [<img src="docs/audit-7a/2560x1440/22-part-inside-definition.png" width="400">](docs/audit-7a/2560x1440/22-part-inside-definition.png) |

**23. Socket add list**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/23-socket-add-list.png" width="400">](docs/audit/1280x720/23-socket-add-list.png) | [<img src="docs/audit-7a/1280x720/23-socket-add-list.png" width="400">](docs/audit-7a/1280x720/23-socket-add-list.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/23-socket-add-list.png" width="400">](docs/audit/2560x1440/23-socket-add-list.png) | [<img src="docs/audit-7a/2560x1440/23-socket-add-list.png" width="400">](docs/audit-7a/2560x1440/23-socket-add-list.png) |

**24. Socket add list with library**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/24-socket-add-list-with-library.png" width="400">](docs/audit/1280x720/24-socket-add-list-with-library.png) | [<img src="docs/audit-7a/1280x720/24-socket-add-list-with-library.png" width="400">](docs/audit-7a/1280x720/24-socket-add-list-with-library.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/24-socket-add-list-with-library.png" width="400">](docs/audit/2560x1440/24-socket-add-list-with-library.png) | [<img src="docs/audit-7a/2560x1440/24-socket-add-list-with-library.png" width="400">](docs/audit-7a/2560x1440/24-socket-add-list-with-library.png) |

**25. Tool move**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/25-tool-move.png" width="400">](docs/audit/1280x720/25-tool-move.png) | [<img src="docs/audit-7a/1280x720/25-tool-move.png" width="400">](docs/audit-7a/1280x720/25-tool-move.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/25-tool-move.png" width="400">](docs/audit/2560x1440/25-tool-move.png) | [<img src="docs/audit-7a/2560x1440/25-tool-move.png" width="400">](docs/audit-7a/2560x1440/25-tool-move.png) |

**25. Tool rotate**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/25-tool-rotate.png" width="400">](docs/audit/1280x720/25-tool-rotate.png) | [<img src="docs/audit-7a/1280x720/25-tool-rotate.png" width="400">](docs/audit-7a/1280x720/25-tool-rotate.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/25-tool-rotate.png" width="400">](docs/audit/2560x1440/25-tool-rotate.png) | [<img src="docs/audit-7a/2560x1440/25-tool-rotate.png" width="400">](docs/audit-7a/2560x1440/25-tool-rotate.png) |

**25. Tool scale**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/25-tool-scale.png" width="400">](docs/audit/1280x720/25-tool-scale.png) | [<img src="docs/audit-7a/1280x720/25-tool-scale.png" width="400">](docs/audit-7a/1280x720/25-tool-scale.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/25-tool-scale.png" width="400">](docs/audit/2560x1440/25-tool-scale.png) | [<img src="docs/audit-7a/2560x1440/25-tool-scale.png" width="400">](docs/audit-7a/2560x1440/25-tool-scale.png) |

**25. Tool size**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/25-tool-size.png" width="400">](docs/audit/1280x720/25-tool-size.png) | [<img src="docs/audit-7a/1280x720/25-tool-size.png" width="400">](docs/audit-7a/1280x720/25-tool-size.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/25-tool-size.png" width="400">](docs/audit/2560x1440/25-tool-size.png) | [<img src="docs/audit-7a/2560x1440/25-tool-size.png" width="400">](docs/audit-7a/2560x1440/25-tool-size.png) |

**26. Add menu**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/26-add-menu.png" width="400">](docs/audit/1280x720/26-add-menu.png) | [<img src="docs/audit-7a/1280x720/26-add-menu.png" width="400">](docs/audit-7a/1280x720/26-add-menu.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/26-add-menu.png" width="400">](docs/audit/2560x1440/26-add-menu.png) | [<img src="docs/audit-7a/2560x1440/26-add-menu.png" width="400">](docs/audit-7a/2560x1440/26-add-menu.png) |

**27. Several selected**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/27-several-selected.png" width="400">](docs/audit/1280x720/27-several-selected.png) | [<img src="docs/audit-7a/1280x720/27-several-selected.png" width="400">](docs/audit-7a/1280x720/27-several-selected.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/27-several-selected.png" width="400">](docs/audit/2560x1440/27-several-selected.png) | [<img src="docs/audit-7a/2560x1440/27-several-selected.png" width="400">](docs/audit-7a/2560x1440/27-several-selected.png) |

**28. Right click menu**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/28-right-click-menu.png" width="400">](docs/audit/1280x720/28-right-click-menu.png) | [<img src="docs/audit-7a/1280x720/28-right-click-menu.png" width="400">](docs/audit-7a/1280x720/28-right-click-menu.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/28-right-click-menu.png" width="400">](docs/audit/2560x1440/28-right-click-menu.png) | [<img src="docs/audit-7a/2560x1440/28-right-click-menu.png" width="400">](docs/audit-7a/2560x1440/28-right-click-menu.png) |

**29. Hidden part**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/29-hidden-part.png" width="400">](docs/audit/1280x720/29-hidden-part.png) | [<img src="docs/audit-7a/1280x720/29-hidden-part.png" width="400">](docs/audit-7a/1280x720/29-hidden-part.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/29-hidden-part.png" width="400">](docs/audit/2560x1440/29-hidden-part.png) | [<img src="docs/audit-7a/2560x1440/29-hidden-part.png" width="400">](docs/audit-7a/2560x1440/29-hidden-part.png) |

### Renaming, removing, looking only, a .glb

**30. Rename**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/30-rename.png" width="400">](docs/audit/1280x720/30-rename.png) | [<img src="docs/audit-7a/1280x720/30-rename.png" width="400">](docs/audit-7a/1280x720/30-rename.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/30-rename.png" width="400">](docs/audit/2560x1440/30-rename.png) | [<img src="docs/audit-7a/2560x1440/30-rename.png" width="400">](docs/audit-7a/2560x1440/30-rename.png) |

**31. Remove asks**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/31-remove-asks.png" width="400">](docs/audit/1280x720/31-remove-asks.png) | [<img src="docs/audit-7a/1280x720/31-remove-asks.png" width="400">](docs/audit-7a/1280x720/31-remove-asks.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/31-remove-asks.png" width="400">](docs/audit/2560x1440/31-remove-asks.png) | [<img src="docs/audit-7a/2560x1440/31-remove-asks.png" width="400">](docs/audit-7a/2560x1440/31-remove-asks.png) |

**32. Look only**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/32-look-only.png" width="400">](docs/audit/1280x720/32-look-only.png) | [<img src="docs/audit-7a/1280x720/32-look-only.png" width="400">](docs/audit-7a/1280x720/32-look-only.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/32-look-only.png" width="400">](docs/audit/2560x1440/32-look-only.png) | [<img src="docs/audit-7a/2560x1440/32-look-only.png" width="400">](docs/audit-7a/2560x1440/32-look-only.png) |

**33. .glb opened**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/33-glb-opened.png" width="400">](docs/audit/1280x720/33-glb-opened.png) | [<img src="docs/audit-7a/1280x720/33-glb-opened.png" width="400">](docs/audit-7a/1280x720/33-glb-opened.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/33-glb-opened.png" width="400">](docs/audit/2560x1440/33-glb-opened.png) | [<img src="docs/audit-7a/2560x1440/33-glb-opened.png" width="400">](docs/audit-7a/2560x1440/33-glb-opened.png) |

### Menus, windows, tips and notes

**40. Menu edit**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/40-menu-edit.png" width="400">](docs/audit/1280x720/40-menu-edit.png) | [<img src="docs/audit-7a/1280x720/40-menu-edit.png" width="400">](docs/audit-7a/1280x720/40-menu-edit.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/40-menu-edit.png" width="400">](docs/audit/2560x1440/40-menu-edit.png) | [<img src="docs/audit-7a/2560x1440/40-menu-edit.png" width="400">](docs/audit-7a/2560x1440/40-menu-edit.png) |

**40. Menu file**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/40-menu-file.png" width="400">](docs/audit/1280x720/40-menu-file.png) | [<img src="docs/audit-7a/1280x720/40-menu-file.png" width="400">](docs/audit-7a/1280x720/40-menu-file.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/40-menu-file.png" width="400">](docs/audit/2560x1440/40-menu-file.png) | [<img src="docs/audit-7a/2560x1440/40-menu-file.png" width="400">](docs/audit-7a/2560x1440/40-menu-file.png) |

**40. Menu help**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/40-menu-help.png" width="400">](docs/audit/1280x720/40-menu-help.png) | [<img src="docs/audit-7a/1280x720/40-menu-help.png" width="400">](docs/audit-7a/1280x720/40-menu-help.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/40-menu-help.png" width="400">](docs/audit/2560x1440/40-menu-help.png) | [<img src="docs/audit-7a/2560x1440/40-menu-help.png" width="400">](docs/audit-7a/2560x1440/40-menu-help.png) |

**40. Menu view**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/40-menu-view.png" width="400">](docs/audit/1280x720/40-menu-view.png) | [<img src="docs/audit-7a/1280x720/40-menu-view.png" width="400">](docs/audit-7a/1280x720/40-menu-view.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/40-menu-view.png" width="400">](docs/audit/2560x1440/40-menu-view.png) | [<img src="docs/audit-7a/2560x1440/40-menu-view.png" width="400">](docs/audit-7a/2560x1440/40-menu-view.png) |

**41. Open example submenu**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/41-open-example-submenu.png" width="400">](docs/audit/1280x720/41-open-example-submenu.png) | [<img src="docs/audit-7a/1280x720/41-open-example-submenu.png" width="400">](docs/audit-7a/1280x720/41-open-example-submenu.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/41-open-example-submenu.png" width="400">](docs/audit/2560x1440/41-open-example-submenu.png) | [<img src="docs/audit-7a/2560x1440/41-open-example-submenu.png" width="400">](docs/audit-7a/2560x1440/41-open-example-submenu.png) |

**42. Keys window**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/42-keys-window.png" width="400">](docs/audit/1280x720/42-keys-window.png) | [<img src="docs/audit-7a/1280x720/42-keys-window.png" width="400">](docs/audit-7a/1280x720/42-keys-window.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/42-keys-window.png" width="400">](docs/audit/2560x1440/42-keys-window.png) | [<img src="docs/audit-7a/2560x1440/42-keys-window.png" width="400">](docs/audit-7a/2560x1440/42-keys-window.png) |

**43. About window**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/43-about-window.png" width="400">](docs/audit/1280x720/43-about-window.png) | [<img src="docs/audit-7a/1280x720/43-about-window.png" width="400">](docs/audit-7a/1280x720/43-about-window.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/43-about-window.png" width="400">](docs/audit/2560x1440/43-about-window.png) | [<img src="docs/audit-7a/2560x1440/43-about-window.png" width="400">](docs/audit-7a/2560x1440/43-about-window.png) |

**44. Tips 1**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/44-tips-1.png" width="400">](docs/audit/1280x720/44-tips-1.png) | [<img src="docs/audit-7a/1280x720/44-tips-1.png" width="400">](docs/audit-7a/1280x720/44-tips-1.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/44-tips-1.png" width="400">](docs/audit/2560x1440/44-tips-1.png) | [<img src="docs/audit-7a/2560x1440/44-tips-1.png" width="400">](docs/audit-7a/2560x1440/44-tips-1.png) |

**44. Tips 2**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/44-tips-2.png" width="400">](docs/audit/1280x720/44-tips-2.png) | [<img src="docs/audit-7a/1280x720/44-tips-2.png" width="400">](docs/audit-7a/1280x720/44-tips-2.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/44-tips-2.png" width="400">](docs/audit/2560x1440/44-tips-2.png) | [<img src="docs/audit-7a/2560x1440/44-tips-2.png" width="400">](docs/audit-7a/2560x1440/44-tips-2.png) |

**44. Tips 3**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/44-tips-3.png" width="400">](docs/audit/1280x720/44-tips-3.png) | [<img src="docs/audit-7a/1280x720/44-tips-3.png" width="400">](docs/audit-7a/1280x720/44-tips-3.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/44-tips-3.png" width="400">](docs/audit/2560x1440/44-tips-3.png) | [<img src="docs/audit-7a/2560x1440/44-tips-3.png" width="400">](docs/audit-7a/2560x1440/44-tips-3.png) |

**44. Tips 4**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/44-tips-4.png" width="400">](docs/audit/1280x720/44-tips-4.png) | [<img src="docs/audit-7a/1280x720/44-tips-4.png" width="400">](docs/audit-7a/1280x720/44-tips-4.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/44-tips-4.png" width="400">](docs/audit/2560x1440/44-tips-4.png) | [<img src="docs/audit-7a/2560x1440/44-tips-4.png" width="400">](docs/audit-7a/2560x1440/44-tips-4.png) |

**44. Tips 5**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/44-tips-5.png" width="400">](docs/audit/1280x720/44-tips-5.png) | [<img src="docs/audit-7a/1280x720/44-tips-5.png" width="400">](docs/audit-7a/1280x720/44-tips-5.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/44-tips-5.png" width="400">](docs/audit/2560x1440/44-tips-5.png) | [<img src="docs/audit-7a/2560x1440/44-tips-5.png" width="400">](docs/audit-7a/2560x1440/44-tips-5.png) |

**45. Export said**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/45-export-said.png" width="400">](docs/audit/1280x720/45-export-said.png) | [<img src="docs/audit-7a/1280x720/45-export-said.png" width="400">](docs/audit-7a/1280x720/45-export-said.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/45-export-said.png" width="400">](docs/audit/2560x1440/45-export-said.png) | [<img src="docs/audit-7a/2560x1440/45-export-said.png" width="400">](docs/audit-7a/2560x1440/45-export-said.png) |

**46. Render said**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/46-render-said.png" width="400">](docs/audit/1280x720/46-render-said.png) | [<img src="docs/audit-7a/1280x720/46-render-said.png" width="400">](docs/audit-7a/1280x720/46-render-said.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/46-render-said.png" width="400">](docs/audit/2560x1440/46-render-said.png) | [<img src="docs/audit-7a/2560x1440/46-render-said.png" width="400">](docs/audit-7a/2560x1440/46-render-said.png) |

### Views, hit shapes and colour

**50. Wireframe**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/50-wireframe.png" width="400">](docs/audit/1280x720/50-wireframe.png) | [<img src="docs/audit-7a/1280x720/50-wireframe.png" width="400">](docs/audit-7a/1280x720/50-wireframe.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/50-wireframe.png" width="400">](docs/audit/2560x1440/50-wireframe.png) | [<img src="docs/audit-7a/2560x1440/50-wireframe.png" width="400">](docs/audit-7a/2560x1440/50-wireframe.png) |

**51. View top**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/51-view-top.png" width="400">](docs/audit/1280x720/51-view-top.png) | [<img src="docs/audit-7a/1280x720/51-view-top.png" width="400">](docs/audit-7a/1280x720/51-view-top.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/51-view-top.png" width="400">](docs/audit/2560x1440/51-view-top.png) | [<img src="docs/audit-7a/2560x1440/51-view-top.png" width="400">](docs/audit-7a/2560x1440/51-view-top.png) |

**52. Hits on**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/52-hits-on.png" width="400">](docs/audit/1280x720/52-hits-on.png) | [<img src="docs/audit-7a/1280x720/52-hits-on.png" width="400">](docs/audit-7a/1280x720/52-hits-on.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/52-hits-on.png" width="400">](docs/audit/2560x1440/52-hits-on.png) | [<img src="docs/audit-7a/2560x1440/52-hits-on.png" width="400">](docs/audit-7a/2560x1440/52-hits-on.png) |

**55. Paint tool**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/55-paint-tool.png" width="400">](docs/audit/1280x720/55-paint-tool.png) | [<img src="docs/audit-7a/1280x720/55-paint-tool.png" width="400">](docs/audit-7a/1280x720/55-paint-tool.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/55-paint-tool.png" width="400">](docs/audit/2560x1440/55-paint-tool.png) | [<img src="docs/audit-7a/2560x1440/55-paint-tool.png" width="400">](docs/audit-7a/2560x1440/55-paint-tool.png) |

**56. New colour form**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/56-new-colour-form.png" width="400">](docs/audit/1280x720/56-new-colour-form.png) | [<img src="docs/audit-7a/1280x720/56-new-colour-form.png" width="400">](docs/audit-7a/1280x720/56-new-colour-form.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/56-new-colour-form.png" width="400">](docs/audit/2560x1440/56-new-colour-form.png) | [<img src="docs/audit-7a/2560x1440/56-new-colour-form.png" width="400">](docs/audit-7a/2560x1440/56-new-colour-form.png) |

### Shaping: faces, edges, corners

**60. Face mode**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/60-face-mode.png" width="400">](docs/audit/1280x720/60-face-mode.png) | [<img src="docs/audit-7a/1280x720/60-face-mode.png" width="400">](docs/audit-7a/1280x720/60-face-mode.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/60-face-mode.png" width="400">](docs/audit/2560x1440/60-face-mode.png) | [<img src="docs/audit-7a/2560x1440/60-face-mode.png" width="400">](docs/audit-7a/2560x1440/60-face-mode.png) |

**61. Face selected**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/61-face-selected.png" width="400">](docs/audit/1280x720/61-face-selected.png) | [<img src="docs/audit-7a/1280x720/61-face-selected.png" width="400">](docs/audit-7a/1280x720/61-face-selected.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/61-face-selected.png" width="400">](docs/audit/2560x1440/61-face-selected.png) | [<img src="docs/audit-7a/2560x1440/61-face-selected.png" width="400">](docs/audit-7a/2560x1440/61-face-selected.png) |

**62. Edge mode**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/62-edge-mode.png" width="400">](docs/audit/1280x720/62-edge-mode.png) | [<img src="docs/audit-7a/1280x720/62-edge-mode.png" width="400">](docs/audit-7a/1280x720/62-edge-mode.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/62-edge-mode.png" width="400">](docs/audit/2560x1440/62-edge-mode.png) | [<img src="docs/audit-7a/2560x1440/62-edge-mode.png" width="400">](docs/audit-7a/2560x1440/62-edge-mode.png) |

**63. Corner mode**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/63-corner-mode.png" width="400">](docs/audit/1280x720/63-corner-mode.png) | [<img src="docs/audit-7a/1280x720/63-corner-mode.png" width="400">](docs/audit-7a/1280x720/63-corner-mode.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/63-corner-mode.png" width="400">](docs/audit/2560x1440/63-corner-mode.png) | [<img src="docs/audit-7a/2560x1440/63-corner-mode.png" width="400">](docs/audit-7a/2560x1440/63-corner-mode.png) |

**64. Face mode on a sphere**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/64-face-mode-on-a-sphere.png" width="400">](docs/audit/1280x720/64-face-mode-on-a-sphere.png) | [<img src="docs/audit-7a/1280x720/64-face-mode-on-a-sphere.png" width="400">](docs/audit-7a/1280x720/64-face-mode-on-a-sphere.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/64-face-mode-on-a-sphere.png" width="400">](docs/audit/2560x1440/64-face-mode-on-a-sphere.png) | [<img src="docs/audit-7a/2560x1440/64-face-mode-on-a-sphere.png" width="400">](docs/audit-7a/2560x1440/64-face-mode-on-a-sphere.png) |

### Posing and animation

**70. Pose mode cart**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/70-pose-mode-cart.png" width="400">](docs/audit/1280x720/70-pose-mode-cart.png) | [<img src="docs/audit-7a/1280x720/70-pose-mode-cart.png" width="400">](docs/audit-7a/1280x720/70-pose-mode-cart.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/70-pose-mode-cart.png" width="400">](docs/audit/2560x1440/70-pose-mode-cart.png) | [<img src="docs/audit-7a/2560x1440/70-pose-mode-cart.png" width="400">](docs/audit-7a/2560x1440/70-pose-mode-cart.png) |

**70. Pose mode dog**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/70-pose-mode-dog.png" width="400">](docs/audit/1280x720/70-pose-mode-dog.png) | [<img src="docs/audit-7a/1280x720/70-pose-mode-dog.png" width="400">](docs/audit-7a/1280x720/70-pose-mode-dog.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/70-pose-mode-dog.png" width="400">](docs/audit/2560x1440/70-pose-mode-dog.png) | [<img src="docs/audit-7a/2560x1440/70-pose-mode-dog.png" width="400">](docs/audit-7a/2560x1440/70-pose-mode-dog.png) |

**70. Pose mode door**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/70-pose-mode-door.png" width="400">](docs/audit/1280x720/70-pose-mode-door.png) | [<img src="docs/audit-7a/1280x720/70-pose-mode-door.png" width="400">](docs/audit-7a/1280x720/70-pose-mode-door.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/70-pose-mode-door.png" width="400">](docs/audit/2560x1440/70-pose-mode-door.png) | [<img src="docs/audit-7a/2560x1440/70-pose-mode-door.png" width="400">](docs/audit-7a/2560x1440/70-pose-mode-door.png) |

**70. Pose mode gnome**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/70-pose-mode-gnome.png" width="400">](docs/audit/1280x720/70-pose-mode-gnome.png) | [<img src="docs/audit-7a/1280x720/70-pose-mode-gnome.png" width="400">](docs/audit-7a/1280x720/70-pose-mode-gnome.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/70-pose-mode-gnome.png" width="400">](docs/audit/2560x1440/70-pose-mode-gnome.png) | [<img src="docs/audit-7a/2560x1440/70-pose-mode-gnome.png" width="400">](docs/audit-7a/2560x1440/70-pose-mode-gnome.png) |

**70. Pose mode robot**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/70-pose-mode-robot.png" width="400">](docs/audit/1280x720/70-pose-mode-robot.png) | [<img src="docs/audit-7a/1280x720/70-pose-mode-robot.png" width="400">](docs/audit-7a/1280x720/70-pose-mode-robot.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/70-pose-mode-robot.png" width="400">](docs/audit/2560x1440/70-pose-mode-robot.png) | [<img src="docs/audit-7a/2560x1440/70-pose-mode-robot.png" width="400">](docs/audit-7a/2560x1440/70-pose-mode-robot.png) |

**71. Pose ring**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/71-pose-ring.png" width="400">](docs/audit/1280x720/71-pose-ring.png) | [<img src="docs/audit-7a/1280x720/71-pose-ring.png" width="400">](docs/audit-7a/1280x720/71-pose-ring.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/71-pose-ring.png" width="400">](docs/audit/2560x1440/71-pose-ring.png) | [<img src="docs/audit-7a/2560x1440/71-pose-ring.png" width="400">](docs/audit-7a/2560x1440/71-pose-ring.png) |

**72. Timeline walk**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/72-timeline-walk.png" width="400">](docs/audit/1280x720/72-timeline-walk.png) | [<img src="docs/audit-7a/1280x720/72-timeline-walk.png" width="400">](docs/audit-7a/1280x720/72-timeline-walk.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/72-timeline-walk.png" width="400">](docs/audit/2560x1440/72-timeline-walk.png) | [<img src="docs/audit-7a/2560x1440/72-timeline-walk.png" width="400">](docs/audit-7a/2560x1440/72-timeline-walk.png) |

**73. Key menu**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/73-key-menu.png" width="400">](docs/audit/1280x720/73-key-menu.png) | [<img src="docs/audit-7a/1280x720/73-key-menu.png" width="400">](docs/audit-7a/1280x720/73-key-menu.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/73-key-menu.png" width="400">](docs/audit/2560x1440/73-key-menu.png) | [<img src="docs/audit-7a/2560x1440/73-key-menu.png" width="400">](docs/audit-7a/2560x1440/73-key-menu.png) |

**74. Playing**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/74-playing.png" width="400">](docs/audit/1280x720/74-playing.png) | [<img src="docs/audit-7a/1280x720/74-playing.png" width="400">](docs/audit-7a/1280x720/74-playing.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/74-playing.png" width="400">](docs/audit/2560x1440/74-playing.png) | [<img src="docs/audit-7a/2560x1440/74-playing.png" width="400">](docs/audit-7a/2560x1440/74-playing.png) |

**75. Animations library**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/75-animations-library.png" width="400">](docs/audit/1280x720/75-animations-library.png) | [<img src="docs/audit-7a/1280x720/75-animations-library.png" width="400">](docs/audit-7a/1280x720/75-animations-library.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/75-animations-library.png" width="400">](docs/audit/2560x1440/75-animations-library.png) | [<img src="docs/audit-7a/2560x1440/75-animations-library.png" width="400">](docs/audit-7a/2560x1440/75-animations-library.png) |

**76. No animation yet**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/76-no-animation-yet.png" width="400">](docs/audit/1280x720/76-no-animation-yet.png) | [<img src="docs/audit-7a/1280x720/76-no-animation-yet.png" width="400">](docs/audit-7a/1280x720/76-no-animation-yet.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/76-no-animation-yet.png" width="400">](docs/audit/2560x1440/76-no-animation-yet.png) | [<img src="docs/audit-7a/2560x1440/76-no-animation-yet.png" width="400">](docs/audit-7a/2560x1440/76-no-animation-yet.png) |

### Mistakes

**80. Recipe with mistake**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/80-recipe-with-mistake.png" width="400">](docs/audit/1280x720/80-recipe-with-mistake.png) | [<img src="docs/audit-7a/1280x720/80-recipe-with-mistake.png" width="400">](docs/audit-7a/1280x720/80-recipe-with-mistake.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/80-recipe-with-mistake.png" width="400">](docs/audit/2560x1440/80-recipe-with-mistake.png) | [<img src="docs/audit-7a/2560x1440/80-recipe-with-mistake.png" width="400">](docs/audit-7a/2560x1440/80-recipe-with-mistake.png) |

**81. Broken from start**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/81-broken-from-start.png" width="400">](docs/audit/1280x720/81-broken-from-start.png) | [<img src="docs/audit-7a/1280x720/81-broken-from-start.png" width="400">](docs/audit-7a/1280x720/81-broken-from-start.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/81-broken-from-start.png" width="400">](docs/audit/2560x1440/81-broken-from-start.png) | [<img src="docs/audit-7a/2560x1440/81-broken-from-start.png" width="400">](docs/audit-7a/2560x1440/81-broken-from-start.png) |

**82. Not a TOML**

| | Before (0.6.0) | After (0.7.0) |
|---|---|---|
| 1280 x 720 | [<img src="docs/audit/1280x720/82-not-a-toml.png" width="400">](docs/audit/1280x720/82-not-a-toml.png) | [<img src="docs/audit-7a/1280x720/82-not-a-toml.png" width="400">](docs/audit-7a/1280x720/82-not-a-toml.png) |
| 2560 x 1440 | [<img src="docs/audit/2560x1440/82-not-a-toml.png" width="400">](docs/audit/2560x1440/82-not-a-toml.png) | [<img src="docs/audit-7a/2560x1440/82-not-a-toml.png" width="400">](docs/audit-7a/2560x1440/82-not-a-toml.png) |
