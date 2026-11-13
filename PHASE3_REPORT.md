# Duck Mold: Phase 3 report

Phase 3 is built, as designed in `DESIGN.md` section 19 and approved
with all eight decisions as recommended, in ten steps, one commit each,
on the branch `claude/duck-mold-phase-3`. No pull request has been
opened: say so when you want one.

The four rules from Phase 2 still hold. The recipe is the model. Every
Studio action that changes the model is an edit command an agent can
run too. An edit changes only what it has to in the file. The viewport
is Duck Forge's CPU renderer.

## In short

Your four findings, and what answers each:

| You found | Now |
|---|---|
| "I can't drag the cube with my mouse or move it or form it in any way." | A part that isn't joined has arrows to move it, rings to turn it, boxes to scale it, and knobs on its faces to resize it. You can also just drag the part. A joined part slides along the surface it sits on. |
| "The options are very little." | Twelve shapes instead of five, with bevel and taper. An Add menu. A right-click menu. Keys. Several parts at once. Hiding. Four views, wireframe, a ground shadow. A palette and a Paint tool. Menus. |
| "It looks like a very simple and weak version of Blender." | It now has the things that make a 3D tool feel like one: handles on the selection, snapping, a grid that stays put, tools with keys, a menu bar. It is still much smaller than Blender, on purpose. |
| "The welcoming screen looks weak." | A start page with the duck, your recent models with pictures, and twelve examples with pictures that open as copies of your own. |

The copy that said "Joined to: nothing" and wouldn't move: a copy of a
free part now stands beside the original, selected, with arrows on it.

## A picture of each new screen

Every picture below is a stored test picture: the tests draw the screen
again and fail if it no longer looks like this.

**The start page** (step 9), with models opened lately and the
examples:

![The start page](crates/duck-mold-studio/tests/screens/start-recent.png)

**Handles** (step 1): the Move tool's arrows on a free part. The grid
has fine lines every 10 cm and stays put when a part moves.

![Move arrows on a crate](crates/duck-mold-studio/tests/screens/handles-move.png)

The Rotate tool's rings:

![Rotate rings on a crate](crates/duck-mold-studio/tests/screens/handles-rotate.png)

**Resize by dragging** (step 2): the Size tool's knobs, one on each
face of a box.

![Size knobs on a crate](crates/duck-mold-studio/tests/screens/handles-size.png)

**Add without a socket** (step 3): the Add menu, and the cone it put on
the ground, clear of the other parts, with the arrows on it.

![The Add menu](crates/duck-mold-studio/tests/screens/add-menu.png)

![A cone added on the ground](crates/duck-mold-studio/tests/screens/add-free.png)

**More shapes** (step 4): an arch, with its sizes in the panel.

![An arch between two towers](crates/duck-mold-studio/tests/screens/shape-arch.png)

A tower's shaft with a bevel and a taper:

![A tapered, bevelled box](crates/duck-mold-studio/tests/screens/shape-bevel-taper.png)

A torus, with a knob for how big it is and one for how thick:

![A doughnut](crates/duck-mold-studio/tests/screens/shape-torus.png)

The seven new examples, as `duck-mold render` draws them:

| | | | |
|---|---|---|---|
| ![dog](crates/duck-mold-studio/assets/examples/dog.png) | ![doughnut](crates/duck-mold-studio/assets/examples/doughnut.png) | ![mug](crates/duck-mold-studio/assets/examples/mug.png) | ![temple](crates/duck-mold-studio/assets/examples/temple.png) |
| dog (capsule) | doughnut (torus) | mug (tube) | temple (pyramid, stairs) |
| ![barn](crates/duck-mold-studio/assets/examples/barn.png) | ![slide](crates/duck-mold-studio/assets/examples/slide.png) | ![gate](crates/duck-mold-studio/assets/examples/gate.png) | |
| barn (prism) | slide (stairs) | gate (arch, bevel, taper) | |

**More control** (step 5): two parts selected, with arrows that move
them together and a panel for both.

![Two parts selected](crates/duck-mold-studio/tests/screens/several.png)

The right-click menu:

![The right-click menu](crates/duck-mold-studio/tests/screens/menu.png)

A hidden part: struck through in the list, and counted in the top bar.

![A hidden part](crates/duck-mold-studio/tests/screens/hidden.png)

**Views and look** (step 6): wireframe, the view from the top, and the
View menu.

![Wireframe](crates/duck-mold-studio/tests/screens/wireframe.png)

![The view from the top](crates/duck-mold-studio/tests/screens/view-top.png)

![The View menu](crates/duck-mold-studio/tests/screens/menu-view.png)

**Colour** (step 7): the palette, with a colour picked and the Paint
tool in hand, and the question asked when a colour in use is removed.

![The palette and the Paint tool](crates/duck-mold-studio/tests/screens/paint.png)

![Removing a colour that is in use](crates/duck-mold-studio/tests/screens/palette-remove.png)

**Tidy sockets** (step 8): the sockets a part declares, listed in the
panel.

![A part's sockets in the panel](crates/duck-mold-studio/tests/screens/sockets.png)

**The first-run tour** (step 10): the tip about the arrows, and the tip
about Add.

![Tip 3 points at an arrow](crates/duck-mold-studio/tests/screens/tour-3.png)

![Tip 5 points at Add](crates/duck-mold-studio/tests/screens/tour-5.png)

## What works

### 1. Handles on the selected part

- Four tools, down the left of the picture, each with a key: Move (G),
  Rotate (R), Scale (S), Size (T). Paint (P) came in step 7.
- A free part: three arrows move it, three rings turn it, three boxes
  scale it along its own sides and a fourth scales it all round.
  Dragging the part itself slides it over the ground, or onto another
  part's dot to join it there.
- A joined part: two arrows slide it along the surface it sits on, and
  one ring twists it around its join. Unjoin, in the panel, makes it
  free right where it is.
- Moves snap in steps of 10 cm (1 cm when zoomed in close), turns in
  steps of 15 degrees. Hold Ctrl to turn snapping off.
- While you drag, the picture shows what the recipe would build and a
  label shows the number. The file is written once, when you let go:
  one drag is one undo step. Escape lets go and writes nothing.
- The grid no longer slides under the model. Its lines are the
  recipe's own measurements.

### 2. Resize by dragging

- A box, wedge, pyramid, prism, stairs or arch: a knob on each of six
  sides. A cylinder, cone, capsule or tube: top, bottom and side. A
  sphere: one. A torus: one for how big, one for how thick.
- Pulling a knob changes the size field in the recipe. On a free part
  the opposite face stays where it was.

### 3. Add without a socket

- Add, in the top bar (or Shift+A), lists the shapes, the parts the
  recipe defines, and the library.
- The new part stands on the ground under the middle of the view, or
  just in front of what already stands there. It is selected, with the
  arrows on it.

### 4. More shapes, bevel and taper

- Seven shapes: `capsule`, `torus`, `tube`, `pyramid`, `prism` (a
  ridged roof), `stairs`, `arch`.
- `bevel` (with `bevel_steps`) cuts the edges of a box or the rims of
  a cylinder. `taper` narrows or widens the top of a box, a cylinder or
  a tube.
- Sockets sit on the surface of each. A torus's and a tube's `top` are
  in the middle of the opening; an arch's `bottom` is on the ground
  between its feet.
- Seven example recipes, each with stored model, report and picture.
  No older stored file changed.

### 5. More control

- Shift+click adds a part to the selection, in the picture or in the
  list. Ctrl+A selects all. The handles then move, turn and scale them
  together, as one undo step.
- A right-click on a part opens its menu: Duplicate, Mirror copy,
  Rename, Remove, Join to, Unjoin, Add a part here, Frame, Hide.
- Keys: Delete, Ctrl+D, F, H and Shift+H, F2, Shift+A, Ctrl+A. They
  don't fire while you type in a field.
- Hide stops drawing a part. The recipe doesn't know, and a hidden
  part is exported like any other.

### 6. Views and look

- Front, Side, Top and 3/4 buttons, and keys 1 to 4.
- Wire (Z) draws outlines only, so you can see through the model.
- Shadow darkens the ground under the model, darkest where something
  stands on it. On to begin with, like the grid.
- File, Edit, View and Help menus. Every line shows its key. Help >
  The keys lists them all.

### 7. Colour

- The palette panel lists the colours. Click a name to paint with it;
  click its square to change the colour itself; New colour adds one;
  remove takes one out, asking what to paint with instead if it is in
  use.
- The Paint tool colours the face under the pointer, which lights up
  first. Shift+click colours the whole part.

### 8. Tidy sockets

- The panel lists the sockets a part declares, each with how many
  things use it, a field to rename it and a button to remove it.
- When the Studio leaves one of its own sockets (`spot`, `spot_2`...)
  with nothing on it, it removes the socket in the same step.
  Edit > Tidy sockets does the whole recipe.

### 9. A proper start page

- The duck, the name, the version. New model and Open a model.
- Models opened lately, each with its picture.
- Twelve examples, each with its picture. A click opens a copy in
  `Documents\Duck Mold\`. The examples are inside the program, so they
  can't be spoiled and are always there.
- Three lines on how it works.

### 10. A first-run tour

- Five tips, one at a time, on the first model you open to work on.
  Each points at what it is about.
- Shown once. Help > Show the tips again brings them back.

## Every new action is an edit command

All are in the agent guide (`docs/AI_GUIDE.md`, section 10). A test
fails if an edit exists without a command and a line in the guide.

| Command | What it does | Step |
|---|---|---|
| `set <part> --move x,y,z` | Moves a free part. | 1 |
| `set <part> --rotate x,y,z` | Turns a free part. | 1 |
| `set <part> --scale n` or `x,y,z` | Scales a part. | 1 |
| `unjoin <part>` | Makes a joined part free, exactly where it stands. | 1 |
| `add <name>` with no `--to` | Adds a free part, stood on the ground. `--stand x,z`, `--move x,y,z`, `--rotate`, `--scale`. | 3 |
| `set` and `add` with `--tube`, `--wall`, `--steps` | Sizes of the new shapes. | 4 |
| `set` and `add` with `--bevel`, `--bevel-steps`, `--taper` | The two options. `--bevel 0` and `--taper 1` take them away. | 4 |
| `set <part> --face-color top=none` | Takes a face's own colour away again. | 7 |
| `remove-color <name> [--replace-with <other>]` | Removes a palette colour. Refused while in use, naming what uses it. | 7 |
| `rename-socket <part.socket> <new name>` | Renames a socket and everything that names it. | 8 |
| `remove-socket <part.socket>` | Removes a declared socket. Refused while in use. | 8 |
| `tidy-sockets` | Removes the Studio's own sockets that nothing uses. | 8 |
| `new <recipe> --example <name>` | Starts from a copy of an example. | 9 |

Selecting several parts, hiding, the views, wireframe, the shadow and
the tour change nothing in the file, so they have no command. A group
edit in the Studio is the same edit, once for each part.

## Tests

434 tests, all passing, with `cargo fmt` and `cargo clippy` clean.

| Where | Tests | What they cover |
|---|---|---|
| The library | 285 | Recipes, shapes, sockets, colours, edits, model files. Every shape is checked to be closed, flat-faced and facing outwards. |
| The command line | 38 | Every command, the exit codes, the stored files of all fifteen examples. |
| The Studio with no screen | 72 | The handles, driven by pointer positions: move, turn, scale, size, groups, snapping. Hiding, painting, the palette, sockets, views, wireframe, shadow. Fourteen stored pictures of the viewport. |
| The Studio's real screen | 39 | Clicks, drags, Shift+click, right-click, keys, typing and every menu, through the same code the window runs. Thirty stored pictures of whole screens. One test checks the examples' pictures still match their recipes. |

## Known limits

- **Nobody has tried it on real Windows yet.** The tests drive the
  real screen code, but how the handles feel under a hand is for you to
  say. The zip is there for that.
- **Several parts**: only free parts in the same list move together. A
  part joined to something outside the selection stays, and the Studio
  says which. Scale with several parts is all round only, not along one
  direction. The Size tool works on one part.
- **A copy of a joined part** is still on the same socket, exactly on
  the original. Drag it to where it goes.
- **A free part that touches nothing** gets a warning ("it floats 0.1
  from ..."). That is the old check, and it is true, but a new part
  from the Add menu gets it straight away.
- **A socket at the very edge of a bevelled face** sits on the cut and
  leans with it. Two of my own examples tripped on this. The recipe
  can say `facing = "front"`; the Studio has no button for it.
- **The Size tool has no knob** for `wall`, `steps`, `bevel` or
  `taper`: those are in the panel. A pyramid's and a prism's knobs sit
  on the box around them, not on the slope.
- **Paint** colours faces a recipe can name. One triangle, or half a
  face, can't be painted. That needs Phase 4.
- **Hiding** is by part: a part joined to a hidden part is still
  drawn. Hiding isn't saved: it is gone when the model is opened again.
- **The shadow** is a patch straight under the model, not a cast
  shadow, and isn't drawn from below. `render`'s pictures have none.
- **Wireframe** draws every edge, also those at the back.
- **The folder for copies of examples** is `Documents` under your user
  folder. If your Documents are moved elsewhere (some OneDrive
  set-ups), the copies go to the old place.
- **The program is still unsigned**, so Windows asks "More info, Run
  anyway" the first time.

## What changed from the design

Each is also in `DESIGN.md` section 19, under "As built" for its step.

- **Moves snap in steps from where the part was**, not onto the grid's
  lines. A crate on a floor has its middle at 0.35 m; snapping to the
  lines would lift it off the floor every time.
- **The Add menu has no small picture beside each shape.** The names
  are in two columns instead. Pictures there are a small job for later.
- **The eye in the parts list is the word "hide" or "show"**, not a
  picture of an eye: the Studio's font has none that reads at that
  size.
- **Scale with several parts has one handle**, all round. The design
  said "scale them around the middle of the group" and that is what it
  does; it doesn't scale them along one direction.
- **The status line moved** to the bottom right, beside Problems, as
  the design's sketch had it. The top bar needed the room.
- **The questions** ("remove them too?", "paint with what instead?",
  a new colour) are small windows in the middle, not rows in a panel.
  In a panel they could be scrolled out of sight.
- **A low arch gets a flatter top** instead of an error.
- **The Studio removes its own unused sockets one at a time**, as part
  of the step that leaves them unused, rather than by running
  `tidy-sockets` after every edit. So a `spot` socket an agent wrote by
  hand and hasn't used yet is left alone.
- **Shift+A opens a small window** with the Add menu's choices, not
  the menu itself.
- **Tips are for the person, not for each model**: seen once, they
  don't come back for the next model.
- **New in the model**: every part knows the names of its faces. The
  design didn't list this. Paint needs it, and Phase 4 will.

## What Phase 4 should start with

Phase 4 is corner, edge and face editing. This phase left room for it:

- Every face of every shape is flat, bulges nowhere inwards, and
  shares each edge with exactly one other face. Tests hold all twelve
  shapes, bevelled and tapered, to that.
- A part knows which of its faces is which. A click already finds a
  face.
- The tools are a list with one handle code path. A new tool is a new
  entry, a set of grips, and what a drag of each means.

What I would do first, in this order:

1. **Try Phase 3 on Windows and say what feels wrong.** Handle sizes,
   snapping steps and which key does what are cheap to change now.
2. **Decide how a moved corner is written in the recipe.** This is the
   hard question, because the recipe is the model. My suggestion: a
   shape keeps its sizes, and a list of changes follows it
   (`move the top face up 0.2`, `pull this edge out 0.1`), applied
   after the shape is built. Then a part stays a box you can resize,
   an agent can read what was done, and an edit is still a small
   change to the file. The other way, writing every corner out, is
   simpler to build but turns the recipe into a list of numbers nobody
   can read.
3. **Start with faces**, the easiest to click and the most useful:
   select a face, move it along its own direction (that is "extrude"
   and "inset" in one), with the same arrows and snapping.
4. Then edges, then corners. Then painting one face of a changed shape.

Small things left from this phase, if you want them first: pictures in
the Add menu; a "facing" choice for a socket in the panel; saving which
parts are hidden; a way to copy a joined part to the next free socket.
