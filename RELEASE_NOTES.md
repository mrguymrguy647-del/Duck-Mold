# Duck Mold release notes

What changed in each version, in plain words. The newest is first. The
section for a version is also the text of its page on GitHub's Releases.

## 0.7.0

**Installing.** Download `DuckMoldSetup-0.7.0.exe` and run it. It
installs for you alone, so it doesn't ask for an administrator, puts
Duck Mold Studio in the Start menu, and can add a desktop shortcut. A
ticked box lets the command line and AI agents find `duck-mold`.
Windows may say "Windows protected your PC" the first time: Duck Mold
isn't signed with a paid certificate. Click "More info", then "Run
anyway". If you'd rather not install anything, the zip is still there:
unzip it and double-click `DuckMoldStudio.exe`.

Your models stay where they are, in Documents\Duck Mold. Installing a
newer version, or uninstalling, never touches them.

### New

- **Nothing is lost.** Every file is written beside the old one and
  swapped in, so a full disk or a crash never leaves half a recipe.
  Undo is kept on disk and carries on after the Studio is closed. A
  change made by another program can't be undone over by mistake. A
  recipe deleted while open can be saved again. If the Studio ever
  crashes, it writes a note and says so next time.
- **A tidier Studio.** One toolbar of tools with icons, a status bar,
  one panel on the right that scrolls, an axis gizmo, a bigger start
  page, and View > Interface size (90 to 200%).
- **Faster with big models**: a frame about twice as fast at 2560 x
  1440, an edit from the panel about twice as fast.
- **Far-away versions.** `lods = "auto"` gives a model two simpler
  versions for a game to draw when it is far off. View > Far version
  shows them; Duck Forge picks one by distance.
- **Picking by hit shape.** With Hits on, a click picks what a game's
  click would. `duck-mold inspect --ray` asks the same from the command
  line.
- **Fit to ground also fits between keys**, adding a key where a jump
  would dip into the ground.
- **A physics body** for Duck Forge games, made from the hit shapes.
- **For AI agents:** `--json` is one line (`--pretty` to read it), `edit
  --json` leads with what changed, `edit --batch` makes a list of edits
  at once, and a misspelt part name lists every part, nearest first.

### Fixed

- A copy of an example went to the old Documents folder if Documents had
  been moved (to OneDrive, say).
- The wireframe drew the back of the model as brightly as the front.
- Many small things the audit found: messages in plainer words, greyed
  buttons that say why, a Remove that asks about the parts hanging from
  it.

### Good to know

- The program is still unsigned, so Windows asks once (above).
- Dragging a part of a very big model (20,000 triangles) is smooth but
  not yet as quick as we'd like: 15 to 19 ms a step.
- `--json` output is now one line. A script that read it line by line
  should read it whole, or pass `--pretty`.

## 0.6.0

Hit shapes (which part a click or a bullet found), events on an
animation's keys (footsteps), and Fit to ground, so a walk copied onto
another model stands on the ground.

## 0.5.0

Models that move: limits on each part's joint, poses, and animations
made of poses, with Pose mode and a timeline in the Studio, and a
library of ready-made animations.

## 0.4 and earlier

The recipe format, the command line (`inspect`, `export`, `render`,
`edit`), the Studio with its handles, tools, palette and start page, and
shaping a box's faces, edges and corners.
