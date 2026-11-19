#!/bin/sh
# Runs a duck-mold program the way someone with nothing else installed
# would: on every example, inspect, check, edit, render and export, and
# the guide, the schema and the MCP server. Used by the release workflow
# inside bare containers (Debian slim, Alpine) with no network at all.
#
#   release/test-bare.sh <duck-mold program> <examples folder> [version]
#
# Plain sh (Alpine has no bash). Works on a copy of the examples, in a
# folder of its own.
set -eu

program=$1
examples=$2
version=${3:-}

work=$(mktemp -d)
cp -R "$examples" "$work/examples"
cp "$program" "$work/duck-mold"
chmod +x "$work/duck-mold"
cd "$work/examples"
dm=$work/duck-mold

if [ -n "$version" ]; then
    said=$("$dm" --version)
    [ "$said" = "duck-mold $version" ] || { echo "it says '$said', not version $version"; exit 1; }
fi
"$dm" guide | grep -q "Duck Mold: start here"
"$dm" guide shaping | grep -q "Duck Mold guide: shaping"
"$dm" schema | grep -q '"title": "Duck Mold recipe"'

count=0
for recipe in *.toml *.json; do
    [ -f "$recipe" ] || continue
    name=${recipe%.*}
    "$dm" inspect "$recipe" --json > "$work/report.json"
    grep -q '"ok":true' "$work/report.json" || { echo "$recipe: inspect isn't ok"; cat "$work/report.json"; exit 1; }
    "$dm" check "$recipe" --json --no-render > "$work/check.json"
    "$dm" render "$recipe" -o "$work/$name.png" > /dev/null
    [ -s "$work/$name.png" ] || { echo "$recipe: no picture"; exit 1; }
    "$dm" export "$recipe" -o "$work/$name.glb" > /dev/null
    [ -s "$work/$name.glb" ] || { echo "$recipe: no .glb"; exit 1; }
    "$dm" inspect "$work/$name.glb" > /dev/null
    count=$((count + 1))
done
[ "$count" -ge 20 ] || { echo "only $count examples"; exit 1; }

# An edit, on a TOML recipe: written, and the recipe still builds.
"$dm" edit robot.toml add-color bare_test "#123456" > /dev/null
grep -q 'bare_test = "#123456"' robot.toml
"$dm" edit robot.toml remove-color bare_test > /dev/null
"$dm" inspect robot.toml > /dev/null
# A new recipe, from nothing.
"$dm" new "$work/lamp.toml" > /dev/null
"$dm" check "$work/lamp.toml" -o "$work/lamp.png" > /dev/null

# The MCP server says hello.
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"bare","version":"1"}}}' \
    | "$dm" mcp | grep -q '"name":"duck-mold"'

echo "ok: $count examples inspected, checked, drawn and exported; edit, new, guide, schema and mcp work"
rm -rf "$work"
