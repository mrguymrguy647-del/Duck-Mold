# Duck Mold: agent kit report (1.1.0)

The agent kit makes Duck Mold, and the rest of the Duck suite, easier
for AI agents, and makes Duck Mold work on Linux and Macs as well as
Windows. It is version **1.1.0**. The design is `DESIGN.md` section 25,
approved with seven decisions and two answers (no `--http`: ChatGPT
uses the Linux file instead; AGENTS.md in Duck Echo too).

## In short

| Item | What it gives | Where to see it |
|---|---|---|
| 1. Linux and Mac | Static `duck-mold` for Linux x86_64 and arm64, one Mac file for both processors, the Studio for Linux, a one-line install, `cargo install`, start files | The Releases page; `install.sh`; `start-duck-mold-studio.sh` |
| 2. No internet | `duck-mold-<version>-source.tar.gz`, with every library inside | `cargo build --offline --release -p duck-mold-cli` |
| 3. MCP | `duck-mold mcp`: guide, inspect, check, edit (and batches), render (with the picture), export, new | `claude mcp add duck-mold -- duck-mold mcp` |
| 4. A shorter guide | One page to start; `guide <topic>` for eight topics; `guide --list`; `guide all` | `duck-mold guide` |
| 5. `check` | Inspect, every animation, the images and the sheet in one call; one list, worst first, each with its fix | `duck-mold check robot.toml --json` |
| 6. `schema` | The recipe format as a JSON Schema, kept in step with the reader by a test | `duck-mold schema` |
| 7. AGENTS.md, CLAUDE.md | In Duck Mold, Duck Forge, Duck Quill and Duck Echo; a test in each checks what they name | The top of each repository |
| 8. Start files | `start-duck-mold-studio.sh`, `Start Duck Mold Studio.command`, `create-desktop-shortcut.sh` | The top of the repository |

## What works

### Linux and Mac (items 1 and 8)

- **The downloads.** The release workflow now makes every file of a
  release, tests each the way the person who gets it would use it, and
  publishes them together only when every job passed:

  | File | Built on | Tested by |
  |---|---|---|
  | `DuckMoldSetup-<v>.exe`, `DuckMold-<v>-windows.zip` | Windows | Installed, installed over and uninstalled, as before |
  | `duck-mold-<v>-linux-x86_64` | Ubuntu, musl (static) | `file` says static; every example inspected, checked, drawn, exported, an edit, `new`, the guide, the schema and an MCP hello, on the runner and in bare Debian and Alpine containers with `--network none` |
  | `duck-mold-<v>-linux-arm64` | Ubuntu on ARM, musl | The same, on ARM |
  | `duck-mold-<v>-macos` | macOS, both processors joined with `lipo` | `lipo -archs` has arm64 and x86_64; the same test on the runner |
  | `DuckMoldStudio-<v>-linux-x86_64.tar.gz` | Ubuntu 22.04 | What is inside; `DuckMoldStudio --version`; `duck-mold` finds the Studio beside it; the bare test; the desktop shortcut |
  | `duck-mold-<v>-source.tar.gz` | Any Linux | Built in a `rust` container with `--network none`, then the bare test |
  | `SHA256SUMS` | The publish job | Every file listed |

  After publishing, the workflow runs `install.sh` against the new
  release, by version and as "the newest", and runs what it installed.
- **The ChatGPT test the approval asked for**: the Linux file runs every
  example in containers with no network at all (`--network none`), on
  both processors.
- **`install.sh`**: picks the file for the machine (Linux x86_64 or
  arm64, or a Mac), checks it against `SHA256SUMS`, installs it in
  `~/.local/bin` (or `DUCK_MOLD_DIR`), takes a Mac's quarantine mark
  off, and says how to put the folder on the PATH. `DUCK_MOLD_VERSION`
  picks a version. A test runs it against a release in a folder, a
  tampered one (nothing is installed) and a missing version.
- **`cargo install`**: `--git` from GitHub or `--path` from a copy. CI
  runs the `--path` one.
- **Start files**: `start-duck-mold-studio.sh` builds and starts the
  Studio on Linux or a Mac (`--build-only` only builds); `Start Duck
  Mold Studio.command` is the same for a double-click in Finder;
  `create-desktop-shortcut.sh` adds the Studio to a Linux applications
  menu and desktop, beside a downloaded Studio or in a built source
  folder. CI shellchecks every script and runs the build part on Linux
  and macOS.
- **CI on macOS**: every push is now built and tested on Windows, Linux
  and macOS.

### Building with no internet (item 2)

`release/make-source.sh` takes what git has, leaves the Studio out (its
window libraries are many times bigger), and puts every library the
program uses inside with `cargo vendor`, with the `.cargo/config.toml`
that makes cargo use them. The archive is about 46 MB and builds
`duck-mold` with `--offline` and an empty cargo folder: tried here with
the network cut off (`unshare -n`), and in CI in a container with no
network.

### `duck-mold mcp` (item 3)

- An MCP server on standard input and output, written without a
  library: JSON-RPC 2.0, one message a line.
- Seven tools: `guide`, `inspect`, `check`, `edit`, `render`, `export`,
  `new`. Each takes its command's arguments, named the same, is turned
  into that command's words, and the program runs itself with them, so
  a tool can never do what the command can't. `edit` takes one edit as
  its words (`"set trunk --color bark"`, quotes kept) or a list, made as
  one `--batch`. `render` returns the PNG itself as an image, as well
  as where it wrote it. Every tool that works on a file takes it as
  `file` or as `recipe`, whichever the agent writes; two different files
  is a mistake, said in words.
- Speaks MCP 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05; answers
  batches; a failed command is a result with `isError` for the AI to
  read; broken lines, unknown methods and unknown tools get JSON-RPC
  errors, and it keeps going.
- `--folder` says where file names start, for Claude Desktop, which
  starts servers in a folder nobody chose.
- Tests start it as its own program and drive it as an app does: hello
  and the version agreement, the tool list, every tool, a batch that is
  refused and changes nothing, a picture that decodes to the PNG on
  disk, broken and unknown messages, `--folder`.
- The README gives the set-up for Claude Code, Claude Desktop and Codex,
  and "Using Duck Mold in ChatGPT" (upload the Linux file and the
  recipe, `chmod +x`, `./duck-mold guide`). The guide has both too.

### A shorter guide (item 4)

`duck-mold guide` prints `docs/guide-start.md`: 97 lines, with the
commands, a recipe, the loop, the rules and the topics. `duck-mold guide
<topic>` prints recipes, edits, shaping, motion, hits, images, export or
mistakes, cut from `docs/AI_GUIDE.md` at marks round its passages, so
there is one text to keep right; passages can be in several topics.
`guide --list` lists them, `guide all` prints the whole guide. Tests:
every command the start page names exists, every topic's marks are
balanced and it holds what it should, an unknown topic is exit 2.

### `duck-mold check` (item 5)

One call builds and inspects the model, looks at every animation frame
by frame, looks at the images, draws the four-view sheet, and gives one
list of problems: errors, then warnings a game would see, then small
things. Each problem has `fix`, the command that surely fixes it, or
`advice`, in words, naming the part it is about (`set head --color
<colour>`, `join arm/hand --to ...`). Warnings now carry `part`, the
path `edit` takes, when they are about one part. It never guesses. The
fixes it can be sure of are
now part of the warning itself, so `inspect` and `edit` give them too:
an unused colour (`remove-color`), an event heard twice where a loop
joins (`remove-event`), an animation that dips into the ground
(`fit-ground`). Tests: the order, that running each `fix` removes its
problem, mistakes first with exit 1, a clean model, a missing file.

### `duck-mold schema` (item 6)

`docs/recipe.schema.json` is written by hand, with a description on
each field. A test keeps it in step with the reader: the reader's field
lists are now public constants, and the test compares each with the
schema (the recipe, a part, a change, a socket, a collider, a pose line,
an animation, a key; the shape, hit-shape and axis names). A small
validator in the test checks every example recipe against it, and that
misspelt fields and wrong values are caught; it stops on any schema word
it doesn't know, so nothing is silently skipped.

### AGENTS.md and CLAUDE.md (item 7)

In each of the four repositories: AGENTS.md says what the project is,
how to build and test it (with no internet too), the first commands,
and the rules; CLAUDE.md starts with `@AGENTS.md`, so there is one text.
A test in each checks that every command, crate, test and file they name
is there.

## Known limits

- **No Studio download for a Mac.** A Mac program from the internet
  must be signed and checked by Apple (a paid account) or macOS warns
  about it at every turn. The command-line file is fine (the install
  line takes the quarantine mark off); the Studio is built from the
  source with `Start Duck Mold Studio.command`.
- **No Studio for Linux on ARM**, and the Linux Studio needs a Vulkan
  driver (most machines have one).
- **The Mac file's Intel half** is built and joined, but CI's Macs have
  Apple chips, so only the Apple half is run there.
- **ChatGPT through MCP** would need an `https://` server on the
  internet; as decided, ChatGPT uses the Linux file in its sandbox.
- **`check`'s `advice`** is written per kind of problem. A problem with
  no advice has only its message (which, for an error, says where and
  usually what to write).
