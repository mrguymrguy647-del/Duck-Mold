# Duck Mold: polish audit (Phase 7a)

An honest look at Duck Mold 0.6.0 before Phase 7a's design. I opened every
screen and mode of the Studio, with every example, at 1280 x 720 and at
2560 x 1440, and saved a picture of each (`docs/audit/`, 79 screens at each
size). I timed the Studio with a big model, tried the ways work can be lost,
and used the command line as an agent would.

How the pictures were made: `crates/duck-mold-studio/tests/audit.rs` drives
the real Studio on a pretend screen, the same way the screen tests do, and
saves what it draws. Run it with:

```bash
DUCK_MOLD_AUDIT=<folder> cargo test -p duck-mold-studio --test audit -- --ignored
```

The two native dialogs (Open and New model's "save as") are Windows' own and
aren't in the pictures.

Each finding has a number, used by the Phase 7a design in `DESIGN.md`. Worst
first: things that lose work, then what is broken or misleading, then what
makes it look unfinished, then speed, then smaller things, then the command
line. The last part goes through every "Known limits" item of the earlier
reports.

## 1. Things that can lose work

**L1. Undo throws away another program's change.** I edited the robot in the
Studio, then changed the file from outside, as an agent or a text editor
would, and pressed Undo. The Studio undid the outside change: the file went
back to before it, and nothing said so. An agent's careful edit, made while
the Studio was open, can be lost to one Ctrl+Z.
(Picture: `80-recipe-with-mistake`, where Undo is lit after an outside change.)

**L2. A recipe deleted or moved while it is open can't be saved again.** The
Studio still shows the model, but every change is refused with "can't read
robot.toml: there is no such file". There is no "Save it again" or "Save as".
The model the person can see on the screen is only in memory, and is gone when
the window closes.

**L3. A failed write leaves half a file behind.** A recipe is written beside
itself and then swapped in, so a reader never sees half of it. That is good.
But if writing the new file fails (a full disk, say), the half-written
`robot.toml.duck-mold-saving` stays in the folder, and the message only says
"can't write robot.toml: it didn't work". The recipe itself is safe.

**L4. Nothing is forced to the disk before the swap.** The new file isn't
flushed before it replaces the old one. After a power cut at the wrong moment,
some file systems keep the new name with no content: an empty recipe.

**L5. A crash leaves no trace and no help.** If the Studio panics, the window
just disappears. Every change is already in the recipe, so the model is safe,
but the person isn't told that, there is no note of what happened to send in,
and the undo history is gone.

**L6. `.glb` and pictures are written in place.** Export writes `robot.glb`
straight over the old one. A Duck Forge game that reloads the file at that
moment reads half a model. (Recipes don't have this problem: see L3.)

**L7. Undo is forgotten when the Studio closes.** It is a known limit since
Phase 2. Together with L1, a mistake made just before closing can't be taken
back, except by hand in the file.

## 2. Broken or misleading

**B1. Panels hide what is in them.** At 1280 x 720 (and at the window's usual
1280 x 800):

- The right column is split in two at a fixed height. The properties of a
  selected part are cut off at the bottom: the robot's arm shows half of its
  Duplicate, Mirror copy and Unjoin buttons behind the palette (`20-part-selected`).
- The palette lists are cut off too: the house's last colour shows half a word
  (`55-paint-tool`), and the gnome's and the snowman's as well.
- The parts list of the robot ends on a half row. Nothing shows there is more:
  there is no scroll bar until the mouse is on it (`20-part-selected`).
- Socket names in the panel are cut to `leg_left_fr` (`24-socket-add-list-with-library`).

**B2. The keys window doesn't fit.** At 720 pixels high its title and its last
line are off the screen, and it can't be scrolled (`42-keys-window`).

**B3. A recipe that never built is called "the last version that worked".**
`broken.toml`, opened with a mistake in it, shows an empty grid and the message
"The picture is of the last version that worked" (`81-broken-from-start`).
There was no such version.

**B4. Raw parser messages.** A file that isn't TOML at all says
"line 1: key with no value, expected `=`" (`82-not-a-toml`). Every other message
in Duck Mold is plain words; this one is the TOML library's.

**B5. A sphere can't be shaped, and the Face button doesn't say so.** Press
Tab with the snowman's body selected: nothing happens in the picture. The
reason, "Only a box can be shaped in this version", appears in small grey
letters at the far bottom right (`64-face-mode-on-a-sphere`). Face, Edge and
Corner look clickable for any part.

**B6. Pose mode forgets the animation shown in Part mode.** The cart in Part
mode shows `roll` beside Play. Press O and the timeline says "none: choose an
animation" (`70-pose-mode-cart`).

**B7. A missing recent file disappears without a word.** A model opened lately
that has since been deleted simply isn't on the start page. A broken one is a
black tile with a "?" and no reason (`02-start-recent-with-missing-and-broken`).

**B8. A stray white ring in the middle of models.** The model's zero point is
drawn as a small white ring. On the door, the gate, the table, the chair and
the cart, it sits on the floor in the middle of the model and looks like a
mistake (`10-example-table`, `70-pose-mode-cart`).

**B9. Opening a `.glb` offers Export.** A `.glb` opens "looking only", but the
top bar still offers Export (a `.glb` from a `.glb`) and an Edit menu whose
lines all do nothing (`33-glb-opened`).

**B10. "Keep them" is unclear.** Removing the robot's head asks "4 other parts
hang from this one. Remove them too?" with "Remove them all" and "Keep them".
It isn't clear what happens to the head itself, and there is no Cancel
(`31-remove-asks`).

## 3. It looks unfinished

These are what made the Studio feel like "a very simple and weak version of
Blender".

**U1. No icons anywhere.** The top bar is 20 grey text buttons of the same
weight in one row, with no grouping beyond thin lines. The tools (Move, Rotate,
Scale, Size, Paint) are plain egui buttons stacked in the corner of the
picture. Every mature 3D tool uses icons with the key letter, and a few clear
groups.

**U2. Big empty areas, tiny crowded text.** At 1280 x 720:

- The Problems bar takes 95 pixels (13% of the window) to say "None".
- The properties panel with nothing selected is one line and a list box, over
  400 pixels of nothing.
- Meanwhile the useful text is 9 to 11 pixels high: part kinds ("box",
  "eye (box)"), the "hide" buttons, the palette's "remove", the hint lines.
  Much of it is grey on dark grey and hard to read.

**U3. It doesn't grow with the screen.** At 2560 x 1440 at 100%:

- Every panel keeps its 1280 size, and the text is as small as ever.
- The start page is a column in the upper middle with the bottom 40% empty
  (`2560x1440/02-start-...`).
- There is no setting for the size of the interface. It follows Windows'
  display scaling, so at 150% it looks right. At 100% on a big screen it
  doesn't.

**U4. The start page is plain.**

- Two small buttons (New model, Open a model) above a grid of examples.
- Example names are file names (`window_wall`), with no line about what each
  shows.
- "How it works" is cut off at 720 pixels.
- Recent files show no date or folder.

It is better than Phase 2's, but it doesn't say "finished product".

**U5. The viewport has no sense of direction.**

- There is no axis gizmo (the little x, y, z in a corner every 3D program has).
- There is no outline of the selected part, only its box.
- The view buttons (Front, Side, Top, 3/4) are text in the top bar, far from
  the picture.

**U6. Status messages are far away and fleeting.** What an action did
("Wrote robot.glb (87152 bytes)", "Hid head...") is grey text at the far right
of the Problems bar, under the properties panel. It's easy to miss that Export
worked, and there's no way to open the folder it wrote to (`45-export-said`).

**U7. Each row shows a "hide" button, always.** Twenty faint "hide" words down
the parts list add noise. The palette's "remove" words are the same, and look
risky so close to the colour names.

**U8. Ghosts clutter the timeline.** With an animation chosen in Pose mode,
every part's ghost before and after is drawn as overlapping orange and blue
wire boxes over the whole model (`2560x1440/72-timeline-walk`). With the robot
it is hard to see the pose itself.

**U9. Inconsistent words and looks.**

- The model's hit-shape line says "none (the model's)", "fitted: none", or
  "none: a game can't tell which part was hit", depending on where you look.
- The bevel's step count shows as a grey "1 cut" next to the bevel size.
- The Add menu shows shapes with icons, but defined parts (`arm`, `leg`) as
  bare words.
- Some dialogs have a title bar with a ×; menus and the key menu don't.

## 4. Speed

Measured in a release build on this machine (4 cores at 2.1 GHz, close to an
ordinary laptop), with a model of 50 parts and 21,660 triangles. The test is
`crates/duck-mold-view/tests/speed.rs`. Times are the middle of several tries.

| What | Time | Smooth? |
|---|---|---|
| Open the file and build it | 26 to 37 ms | fine |
| Draw one frame of the viewport, 1280 x 720 window | 10 to 14 ms | 70 to 100 frames a second |
| Draw one frame, 2560 x 1440 window | 30 to 37 ms | **27 to 33 frames a second** |
| ... of which the ground shadow | ~6 ms | worked out again every frame |
| One edit from the panel (write, read, build, check) | 57 to 69 ms | a small hitch |
| One step of a drag (work out the edit, build the preview, draw) | 23 to 24 ms | **about 40 a second** |
| A click that picks a part | 1.3 ms | fine |

**S1.** On a big screen, turning the view of a big model is under 30 frames a
second. On a slower laptop it will be around 20. It also uploads a
1560 x 1180 picture to the graphics card every frame, which these numbers
don't count.

**S2.** A drag rebuilds and checks the whole model at every step. With 50
parts it is at the edge of smooth.

**S3.** The ground shadow and the grid are worked out again every frame,
though they only change when the model changes.

**S4.** Duck Forge's renderer works out each mesh's bounding ball from every
vertex on every draw (the survey of Duck Forge found this). It's small, but
it's free to fix.

The examples themselves (at most 1,500 triangles) are smooth everywhere.

## 5. Smaller things

- **M1.** Rename gives no hint. F2 puts the cursor in the Name field; nothing
  says Enter renames and Escape doesn't (`30-rename`).
- **M2.** The About window is a paragraph. It has no picture, no link to the
  guide or the website, and no "copy version" (`43-about-window`).
- **M3.** Pose mode shows the palette, which does nothing there.
- **M4.** Look-only mode (`32-look-only`) shows every field greyed out. It
  would be clearer as plain text.
- **M5.** The animation library list is squeezed into the bottom of the left
  column (`75-animations-library`).
- **M6.** Hiding isn't saved with the model. It's remembered for 8 files.
  That's fine; it's said here only so it isn't mistaken for a fault.
- **M7.** The window title is the file name only. There is no folder, no
  "looking only", and no version.

## 6. The command line, for agents

- **C1. JSON is long.** `--json` is pretty-printed with every number on its
  own line: 868 lines for the robot's `inspect`. Agents pay for every line.
- **C2. `edit --json` buries the edit.** It prints the whole 15 KB report. The
  `"edit"` part (what changed) sits in the middle because the keys are sorted
  by name. An agent has to dig for it.
- **C3. Unknown part: the suggestions leave out nested parts.** `rename nosuch`
  lists `body`, `head`, `left_arm`... but not `left_arm/hand`, a part `edit`
  does accept.
- **C4. `edit --help` is out of date.** Its first line lists the 15 commands
  of Phase 2; there are 45 now.
- **C5. One change per run.** Twenty edits means twenty runs, twenty reads,
  twenty builds and twenty writes, and twenty separate changes if one fails
  halfway. Nothing applies a list of edits together.
- **C6. No compact form of an edit's result in text either.** A successful
  `edit` prints the whole `inspect` report after one line saying what
  changed.

## 7. The known limits of earlier phases, today

Every "Known limits" item from `PHASE0_REPORT.md` to `PHASE6_REPORT.md` that is
still true, and whether it's worth fixing now. "Now" means in 7a; "7b" means
it belongs with connecting the suite; "later" means not in this phase.

| Limit | From | Still true | Fix now? |
|---|---|---|---|
| Faces where two parts meet are both kept, hidden inside | 0 | yes | later: a few triangles each |
| Two faces in the same plane that flicker aren't found | 0 | yes | later |
| `inspect` overlap depth is measured at sample points | 0 | yes | no: good enough |
| A high-detail cluster of overlapping parts is slow to check | 0 | yes | **now**, with S2: checks are skipped during a drag |
| Names are plain letters, digits, `_` and `-` | 0 | yes | no: on purpose |
| `--json` spread over many lines | 0 | yes | **now** (C1) |
| No licence | 0 | yes | your call |
| `tan` could differ on macOS | 1 | yes | no: no macOS users |
| A socket has no "up"; no switch to leave sockets out | 1 | yes | later |
| glTF import: only base colour; OBJ `.mtl` not read | 1 | yes | 7b (textures) |
| Only five shapes have surface sockets | 1 | yes | later |
| Not tried on a real Windows PC by me | 2 | yes | **now**: release testing on GitHub's Windows; a person still has to click |
| Undo forgotten on close | 2 | yes | **now** (L7) |
| A very large model turns slowly | 2 | yes | **now** (S1 to S4) |
| Markers on the far side can't be clicked | 2 | yes | no |
| A copy of a joined part sits on the original | 3 | yes | **now**: it goes on a free socket of the same kind if there is one, else beside |
| A new free part gets a "floats" warning at once | 3 | yes | **now**: a part standing on the ground isn't floating |
| Hiding is by part; not saved | 3 | yes | no (M6) |
| The shadow isn't drawn from below; `render` has none | 3 | yes | no |
| Wireframe draws back edges | 3 | yes | **now**: back edges dimmer |
| Copies of examples go to the old Documents folder if Documents moved | 3 | yes | **now**: use Windows' known-folder lookup |
| The program is unsigned | 3 | yes | no: a certificate costs money; the installer says so plainly |
| Only a box can be shaped | 4 | yes | no: say it better (B5) |
| Long names after cuts | 4 | yes | no |
| "Save to the library" isn't undone | 4 | yes | no |
| `add-socket` has no `--facing` | 4 | yes | **now**: small |
| Ghosts not drawn while playing | 5 | yes | no: right as it is (U8 asks for fewer, not more) |
| No IK; rigid parts | 5 | yes | no: not in this phase |
| `mirror-pose` goes by names | 5 | yes | no |
| A capsule's rim can poke out of its ends | 6 | yes | no |
| A stretched model is hit-tested by its largest scale | 6 | yes | no |
| Hit shapes aren't physics | 6 | yes | **now** (item 2) |
| `fit-ground` fits keys, not between them | 6 | yes | **now** (item 1) |
| Ten rounds at most per update | 6 | yes | no: on purpose |
| The properties panel is long and scrolls | 6 | yes | **now** (B1) |

## What this adds up to

The Studio works, and almost everything it does is tested. What it lacks is
the last 20%:

- **Safety at the edges** (L1 to L7). L1 is the one I'd fix first: it can
  silently throw away an agent's work.
- **A layout that holds its contents** (B1, B2, U2, U3).
- **A finished look**: icons, grouping, readable text, a start page that
  fills the screen, an axis gizmo (U1, U4, U5).
- **Feedback where the eye is** (B5, U6).
- **Speed for big models on big screens** (S1 to S4).

The Phase 7a section of `DESIGN.md` says which of these are fixed and how.
