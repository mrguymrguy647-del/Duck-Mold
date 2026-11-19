# Duck Mold release notes

What changed in each version, in plain words. The newest is first. The
section for a version is also the text of its page on GitHub's Releases.

## 1.0.0

**Installing.** Download `DuckMoldSetup-1.0.0.exe` and run it. It
installs for you alone, so it doesn't ask for an administrator, puts
Duck Mold Studio in the Start menu, and can add a desktop shortcut. A
ticked box lets the command line and AI agents find `duck-mold`.
Windows may say "Windows protected your PC" the first time: Duck Mold
isn't signed with a paid certificate. Click "More info", then "Run
anyway". If you'd rather not install anything, the zip is still there:
unzip it and double-click `DuckMoldStudio.exe`. Installing over 0.7.0
keeps everything of yours.

To paint a model's images you will want **Duck Quill 0.6.0** or newer
(any picture program that saves PNG files works too). To see them in a
game, and to change a model while the game runs, **Duck Forge** from
the same day.

### New

- **Images on models.** A part, or one face of it, can show a picture:
  a face, a label, wood planks. In a recipe it is one line, `image =
  "textures/planks.png"` or `images = { front = "textures/face.png" }`.
  The image lies over the part's colour, so a face is a small picture
  with only a mouth on it. Pixels stay sharp, in Duck Mold and in a
  game.
- **In the Studio:** a row of the model's images under the palette, to
  paint a face or a whole part with one; each part lists its images;
  *New image...* makes one the right shape and size.
- **Paint in Duck Quill.** One button opens the image in Duck Quill,
  with thin lines showing the faces it covers and paper the colour of
  the part. Save there and the model has it. Without Duck Quill, the
  image opens in whatever program you use for pictures: it is a plain
  PNG.
- **Models from Blender keep their pictures.** A glTF file used as a
  part brings each material's picture (PNG or JPEG), laid on the way the
  file says.
- **Straight into a game.** A recipe can say where its `.glb` belongs
  (`export = "../assets/robot.glb"`). *Export after every change* then
  keeps a running Duck Forge game up to date: lengthen the robot's arm
  and the robot in the game, still walking, has the longer arm.
- **Inside Duck Forge Studio.** Duck Forge Studio opens a game's models
  in tabs, and each tab is Duck Mold's Studio itself, with the same
  Undo.
- **New examples:** the robot has a painted face and a label; a crate
  with wood planks; a signpost made in another program that brings its
  own pictures; and Pip, a small explorer with a face painted in Duck
  Quill, who walks in a Duck Forge game.
- **For AI agents:** `set --image` and `--face-image`; `new-image`,
  `paint` and `image-guide`; `set-export`; `inspect` lists a model's
  images and how many pixels each has to the metre, and warns when
  they differ a lot.

### Changed

- A `.glb` with images needs a Duck Forge that reads them. An older
  Duck Forge says "update Duck Forge" instead of drawing the painted
  faces white. A model without images is written exactly as before.
- The robot example is painted, so a copy of it now comes with two
  small pictures in a `textures` folder beside it.

### Good to know

- The program is still unsigned, so Windows asks once (above).
- An image is a PNG of at most 1024 by 1024 pixels, and it is stretched
  to fit its face: it doesn't repeat. (A picture that came inside a
  model from another program can repeat.)
- Not in this version: parts that bend, shiny or see-through surfaces,
  bump maps.

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
