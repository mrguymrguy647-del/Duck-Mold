# Duck Mold: Phase 2 report

Phase 2 is built, as designed in `DESIGN.md` section 18 and approved
with one condition and four additions, in nine steps, one commit each,
on the branch `claude/duck-mold-phase-2`. No pull request has been
opened: say so when you want one.

## In short

- **Duck Mold Studio** is a window where a model is built with the
  mouse. It keeps no model of its own: every change is written to the
  recipe file at once, and the model is built from the file again.
- **Every change the Studio makes is a command too.** `duck-mold edit
  <recipe> add | remove | rename | join | set | duplicate | add-color |
  set-color | add-socket | set-socket`, with `--json`. The Studio and the
  command line call the same function.
- **An edit changes only what it has to.** Comments, order, spacing and
  line endings stay, in TOML and in JSON.
- **The file can change underneath.** A recipe saved by an agent or a
  text editor while the Studio is open is picked up within half a
  second, and is a step that undo can take back.
- **A part can be put anywhere on a surface**: click a point of a part,
  or drag another part onto it.
- **Undo and redo** for every change, 500 steps back.
- **A parts library** with small pictures.
- **Starts on Windows without a terminal**: a zip to download, a duck
  for an icon, a script that makes a desktop shortcut.

One new example: `examples/duck.toml`. Its picture is the Studio's icon.

## What works

### 1. The edit commands

| Command | What it does |
|---|---|
| `add <name> --to <part.socket>` | A new part on a socket: `--shape`, `--use` or `--file`, with sizes, colour, twist. |
| `remove <part>` | Removes a part; refuses if others hang from it, unless `--with-attached`. |
| `rename <part> <new>` | Renames it, and everything that names it. |
| `join <part> --to <part.socket>` | Moves its join. |
| `set <part> ...` | Shape, sizes, colour, face colours, twist. |
| `duplicate <part>` | A copy. `--mirror` makes a right arm from a left arm. |
| `add-color`, `set-color` | A new palette colour; a new value for one. |
| `add-socket <part> <name> --on <side> --shift x,y,z` | Declares a socket on a part's surface. |
| `set-socket <part.socket>` | Moves one. Whatever sits on it moves too. |

- Each prints the same report as `inspect`. `--json` adds what the edit
  did; `--dry-run` writes nothing.
- The result is built before it is written. An edit that would leave
  the recipe with more mistakes than it had is refused, and nothing is
  written.
- If the file was saved by someone else while the edit was being worked
  out, the edit is worked out again on the new text.
- All ten are in the agent guide (`docs/AI_GUIDE.md`, section 10), and a
  test fails if one is added without a command and a line in the guide.
- Also new: `duck-mold new <recipe>` writes a starter recipe (one box),
  and `duck-mold studio [recipe]` starts the Studio.

### 2. Sockets on the surface of assemblies and of parts from files

Carried over from Phase 1. A socket on a part made of parts, or on a
mesh from a file, now sits on its surface. No example's shape changed.

### 3. The Studio window

- The viewport is drawn by Duck Forge's CPU renderer, so a model looks
  the same as in a game. Drag turns it, right-drag (or Shift and drag)
  slides it, the wheel zooms, **Frame** brings it back. A grid lies on
  the ground, one square to a metre.
- The parts are listed as a tree on the left.
- **Problems** at the bottom shows `inspect`'s mistakes and warnings as
  they change. With a mistake in the file, the last version that built
  stays up.
- **Export** and **Render picture** in the top bar write the `.glb` and
  the four-view picture next to the recipe. No terminal.
- `duck-mold preview` now opens the same window to look only. The
  Phase 1 window code is gone: one window library instead of two.

### 4. Selecting

Click a part in the picture or in the list; it is marked in both. A part
made of parts is selected whole first; click again for the part inside.

### 5. Properties

Name, what it is made of, sizes, colour (from the palette, or a new
one; the palette colour itself can be changed too), what it is joined
to, twist; and buttons for Duplicate, Mirror copy and Remove.

**Your condition on decision 3 is met.** A part that comes from a
definition says so above its fields, before any edit, and names every
part that would change:

> Inside the definition "arm". Any change here also changes:
> right_arm/hand.

> Made from the definition "eye". Changing its shape, size or colour
> also changes: right_eye.

### 6. Sockets

- The selected part's sockets show as small round markers.
- Click one and pick a shape, a part the recipe defines, or a library
  part: it is joined there.
- **Click any point of the selected part** to add a part at that point.
  The Studio declares a socket there (`on` + `shift`) and joins the new
  part to it.
- **Drag the selected part** to move it. Dropped on a marker, it joins
  there. Dropped on a part's surface, it goes to that point; if it
  already has a socket of its own on that part, that socket slides
  (its `shift` changes) and no new one is made.

### 7. Undo and redo

Buttons, Ctrl+Z and Ctrl+Y. A step is the whole file before and after a
change, so undo is exact to the byte. A save from outside the Studio is
a step called "Changed outside the Studio".

### 8. The parts library

The folder `parts` next to the recipe (or one you choose): every recipe
and model file in it, with a small picture drawn by the same code as
`render`. Try it with `examples/desk.toml`.

### 9. Windows, without a terminal

- `DuckMoldStudio.exe` opens no console and has the duck as its icon.
- `Start Duck Mold Studio.bat` builds and starts it from a copy of the
  repository. `Create desktop shortcut.bat` makes a desktop shortcut.
- A release workflow builds `DuckMold-0.2.0-windows.zip` with both
  programs, the examples, the docs and a read-me.
- The start page has Open, New model and the recipes opened lately.
- A recipe dropped on the program, its shortcut or its window opens.

## The five rules

| Rule | How it is kept | What checks it |
|---|---|---|
| 1. The recipe is the model | The Studio holds the file's text and what it builds, nothing else. Every edit goes to the file first. | Tests read the file after each action and compare it with what the Studio shows. |
| 2. People and agents are equals | Each Studio action builds an `Edit` and calls the function `duck-mold edit` calls. | A test fails if a kind of edit has no command or no line in the agent guide. |
| 3. Edits are gentle | An edit is a few replacements in the text. | Every edit is run on every example, in TOML and JSON; only the expected lines may differ. Comments, Windows line endings and the byte-order mark are tested. |
| 4. The file can change underneath | The files a recipe uses are checked twice a second. Before an edit or an undo, the Studio looks first. | Tests save the file from outside, then edit and undo. |
| 5. Same window library | egui 0.37, as Duck Quill and Duck Forge's Studio. The screen is one function that draws into the space it is given. | The screen tests run it that way. |

**If both change at once.** The Studio never writes over a file it
hasn't just read. An edit is worked out on what the file holds at that
moment, and the write is refused if the file changed during it (the edit
is then worked out again). So both changes end up in the file. If they
can't both hold (you rename a part the agent has just removed), the edit
is refused with the reason, and nothing is written.

## How to check it yourself

On Windows, double-click `Start Duck Mold Studio.bat`. Then:

1. **Open a model**, choose `examples\robot.toml`. Drag to turn it.
2. Click the head. Change its colour on the right. Open `robot.toml` in
   Notepad: one line changed.
3. Click an eye. Read the notice above its fields.
4. Click the head again, then a round marker on it, then **Cone**.
5. Click a bare spot on the head, then **Box**.
6. Drag the cone onto the body.
7. Press Ctrl+Z until the robot is as it was. Notepad shows the file is
   back to the byte.
8. With the Studio open, change a number in Notepad and save. The Studio
   follows. Undo takes that back too.
9. Open `examples\desk.toml`, click the desk top, click a marker: the
   library's crate and lamp are in the list.
10. **Export**, then **Render picture**: two files appear next to the
    recipe.

## Numbers

- 358 tests, on Windows and Linux for every push.
- 4 crates. The command-line program still builds without any window
  library.
- 8 examples, each with a stored model, report and picture.
- 7 stored pictures of whole Studio screens.

## Known limits

**Not tried on a real Windows PC by me.** The tests run on GitHub's
Windows machines, and the release workflow builds the zip there and
checks it. But nobody has double-clicked the `.bat` files, looked at the
icon in File Explorer, used the Open and Folder dialogs, or dropped a
file on the window. Those need a person. The window itself I ran on
Linux, on a pretend screen.

**What the Studio can't change yet** (type these in the recipe):

- A part placed by hand with `move` and `rotate`. There is no handle to
  drag it by; joining it to a socket replaces its `move`.
- `scale` and `mirror` (except through Mirror copy).
- The colour of a single face. The command line has `--face-color`.
- Sockets written with `at` or `from`.
- Making a definition from a part.
- Removing or renaming a socket. A socket that the Studio made (`spot`,
  `spot_2`) and that nothing sits on any more stays in the recipe.
- The order of the parts, and the model's own name.
- Removing a palette colour.

**How it behaves:**

- One part is selected at a time.
- While a part is dragged, a marker shows where it will go; the part
  itself moves when it is dropped.
- Markers on the far side of a part are small and can't be clicked. Turn
  the view.
- On a round part, a point low on its side can land on a nearer face
  than the one clicked.
- The library folder you choose is forgotten when the Studio closes.
- Undo is forgotten when the Studio closes. There is none on the
  command line.
- New model makes a TOML recipe. JSON recipes open and edit the same.
- A very large model will turn slowly: the viewport is drawn on the
  processor.
- `duck-mold studio` waits until the window is closed.

**Tests:** pictures of whole screens are compared on Linux only, as
agreed. They are drawn one after another by one test, because the
software graphics driver on GitHub's machine crashed when several were
drawn at once.

## What changed from the approved design

- **A drag writes once, on the drop**, not about ten times a second
  while it moves. One write, one undo step, and no half-finished recipe
  for an agent to read in between.
- **`set` on a part made with `use`** changes shape, size and colour in
  the definition, where they are written. Before, the command wrote them
  on the part, which the recipe refuses. Found while building the
  properties panel; it applies to the command line too.
- **`duck-mold new`** was added, so that New model is a command like
  everything else.
- **The library panel only shows** what is in the folder. A part is
  chosen in the list that a click on a socket opens.
- **The Studio names files from the recipe's folder** (`parts/lamp.gltf`)
  everywhere, not by their whole path.
- **The picture stays still through an edit.** Adding a part on one side
  moves a model's middle; the camera now goes with it.
- **The version is 0.2.0.**

Each is in `DESIGN.md` section 18, under "As built".

## What Phase 3 should start with

1. **Try it on your PC**, and tell me what feels wrong. A tool for the
   mouse is only half tested until a person has used it.
2. **A handle to move and turn a part that isn't joined**, and to twist
   one that is. This is the biggest gap in the Studio.
3. **Tidy sockets**: remove and rename, and clear away ones nothing
   uses.
4. **Face colours in the properties panel.**
5. **"Make this a definition"**, so a part made once can be used twice
   without typing.
6. **Docking in Duck Forge's Studio.** The screen is already a function
   that draws into a given space; what is left is on Duck Forge's side.
7. Then the things this phase left out on purpose: bones and animation,
   UVs and textures, booleans.
