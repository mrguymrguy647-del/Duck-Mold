#!/bin/sh
# On Linux: adds Duck Mold Studio to your applications menu, and puts a
# shortcut on your desktop if you have one. Keep this folder where it is
# afterwards: the shortcut points into it.
#
# Works beside a downloaded DuckMoldStudio, or in the source folder after
# ./start-duck-mold-studio.sh has built it.
set -eu

here=$(cd "$(dirname "$0")" && pwd)

if [ -x "$here/DuckMoldStudio" ]; then
    program="$here/DuckMoldStudio"
elif [ -x "$here/target/release/DuckMoldStudio" ]; then
    program="$here/target/release/DuckMoldStudio"
else
    echo "Duck Mold Studio isn't built yet."
    echo "Run ./start-duck-mold-studio.sh first, then run this again."
    exit 1
fi

if [ -f "$here/icon.png" ]; then
    icon="$here/icon.png"
else
    icon="$here/crates/duck-mold-studio/assets/icon.png"
fi

applications="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$applications"
shortcut="$applications/duck-mold-studio.desktop"

cat >"$shortcut" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Duck Mold Studio
Comment=Build 3D models from parts, for Duck Forge games
Exec="$program" %f
Path=$here
Icon=$icon
Terminal=false
Categories=Graphics;3DGraphics;
DESKTOP
chmod +x "$shortcut"
echo "Added Duck Mold Studio to your applications menu."

desktop="${XDG_DESKTOP_DIR:-$HOME/Desktop}"
if [ -d "$desktop" ]; then
    cp "$shortcut" "$desktop/duck-mold-studio.desktop"
    chmod +x "$desktop/duck-mold-studio.desktop"
    echo "Put a Duck Mold Studio shortcut on your desktop."
    echo "(Some desktops ask once whether to trust it: choose Allow Launching.)"
fi
