#!/bin/sh
# Makes duck-mold-<version>-source.tar.gz: the source of the duck-mold
# program with every library it uses inside it ("vendored"), so that it
# builds on a machine with no internet at all:
#
#   tar xzf duck-mold-<version>-source.tar.gz
#   cd duck-mold-<version>
#   cargo build --offline --release -p duck-mold-cli
#
# Only the program, not the Studio (whose window libraries are many times
# bigger). Run from the top of a git checkout:
#
#   release/make-source.sh <version> <folder to put the archive in>
set -eu

version=$1
out=$2
name="duck-mold-$version"
top=$(pwd)
case $out in
    /*) ;;
    *) out="$top/$out" ;;
esac
work=$(mktemp -d)

# What git has, nothing else (no target/, no files of yours).
git archive --format=tar --prefix="$name/" HEAD | tar -x -C "$work"
cd "$work/$name"

# The workspace without the Studio.
rm -rf crates/duck-mold-studio crates/duck-mold-studio-app
sed -i.bak \
    -e '/^    "crates\/duck-mold-studio",$/d' \
    -e '/^    "crates\/duck-mold-studio-app",$/d' \
    -e '/^duck-mold-studio = { path = "crates\/duck-mold-studio" }$/d' \
    Cargo.toml
rm Cargo.toml.bak
if grep -hv '^[[:space:]]*#' Cargo.toml crates/*/Cargo.toml | grep -q 'duck-mold-studio'; then
    echo "the Studio is still named in a Cargo.toml"
    exit 1
fi

# Every library, inside. cargo vendor prints the lines that tell cargo
# to use them; they go in .cargo/config.toml, which cargo reads itself.
mkdir -p .cargo
cargo vendor --versioned-dirs vendor > .cargo/config.toml

cat > BUILDING-OFFLINE.txt <<TEXT
Duck Mold $version: the duck-mold program, with every library it uses.

It builds with no internet at all. You need Rust (1.97 or newer):

    cargo build --offline --release -p duck-mold-cli

The program is then target/release/duck-mold (duck-mold.exe on Windows).
Copy it anywhere on your PATH. "duck-mold guide" says how to use it.

The Studio, the window for people, isn't in here: its libraries are many
times bigger. Get it from the releases page, or build it from the full
source on GitHub.
TEXT

cd "$work"
mkdir -p "$out"
tar -czf "$out/$name-source.tar.gz" "$name"
rm -rf "$work"
ls -l "$out/$name-source.tar.gz"
