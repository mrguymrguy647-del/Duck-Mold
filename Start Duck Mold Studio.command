#!/bin/sh
# On a Mac: double-click this file in Finder to build and start Duck Mold
# Studio. (The first time, macOS may ask whether to open it: choose Open.
# If it says it can't, right-click it and choose Open.) The first build
# takes a few minutes; after that it starts in seconds.
cd "$(dirname "$0")" || exit 1
exec ./start-duck-mold-studio.sh "$@"
