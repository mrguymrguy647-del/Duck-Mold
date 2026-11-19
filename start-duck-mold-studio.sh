#!/bin/sh
# Builds and starts Duck Mold Studio, on Linux or a Mac.
#
#   ./start-duck-mold-studio.sh               build it, then start it
#   ./start-duck-mold-studio.sh robot.toml    ...and open a recipe
#   ./start-duck-mold-studio.sh --build-only  build it, and don't start it
#
# The first time takes a few minutes: Rust downloads and builds the
# libraries it uses. After that it starts in seconds. On a Mac you can
# double-click "Start Duck Mold Studio.command" instead.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"

if ! command -v cargo >/dev/null 2>&1; then
    # rustup puts cargo here; a new terminal finds it, this one may not.
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
    else
        echo "Rust is not installed yet."
        echo "Install it from https://rustup.rs, then run this again."
        exit 1
    fi
fi

build_only=no
if [ "${1:-}" = "--build-only" ]; then
    build_only=yes
    shift
fi

echo "Building Duck Mold Studio... (the first build takes a few minutes)"
if ! cargo build --release; then
    echo
    echo "Something went wrong while building."
    echo "Copy what is above and send it to Claude."
    exit 1
fi

if [ "$build_only" = yes ]; then
    echo "Built: target/release/DuckMoldStudio"
    exit 0
fi

# A recipe named from the folder this was started in still opens.
exec "$here/target/release/DuckMoldStudio" "$@"
