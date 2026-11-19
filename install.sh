#!/bin/sh
# Installs duck-mold, the Duck Mold program, on Linux or a Mac:
#
#   curl -fsSL https://raw.githubusercontent.com/mrguymrguy647-del/Duck-Mold/main/install.sh | sh
#
# It downloads the newest release's duck-mold for this machine, checks it
# against the release's SHA256SUMS (a fingerprint of every file), and
# puts it in ~/.local/bin. Nothing needs administrator rights.
#
# To choose:
#   DUCK_MOLD_DIR=/some/folder       where to put it (default ~/.local/bin)
#   DUCK_MOLD_VERSION=1.1.0          a version other than the newest
#   DUCK_MOLD_DOWNLOAD_URL=...       where the releases are (for tests;
#                                    default the GitHub releases page)
set -eu

repo="mrguymrguy647-del/Duck-Mold"
dir="${DUCK_MOLD_DIR:-$HOME/.local/bin}"
base="${DUCK_MOLD_DOWNLOAD_URL:-https://github.com/$repo/releases/download}"

say() { printf '%s\n' "$*"; }
fail() { printf 'duck-mold install: %s\n' "$*" >&2; exit 1; }

# Downloads $1 to the file $2, with curl or wget, whichever is there.
fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2" || fail "couldn't download $1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$1" -O "$2" || fail "couldn't download $1"
    else
        fail "this needs curl or wget to download, and has neither"
    fi
}

# Which file is for this machine.
case "$(uname -s)" in
    Linux)
        case "$(uname -m)" in
            x86_64 | amd64) asset_end="linux-x86_64" ;;
            aarch64 | arm64) asset_end="linux-arm64" ;;
            *) fail "there is no duck-mold for $(uname -m) Linux yet: build it from the source (cargo install, in the README)" ;;
        esac
        ;;
    Darwin) asset_end="macos" ;;
    *) fail "this installs on Linux or a Mac; on Windows, use DuckMoldSetup.exe from the releases page" ;;
esac

# Which version: the one asked for, or the newest release's.
version="${DUCK_MOLD_VERSION:-}"
if [ -z "$version" ]; then
    if command -v curl >/dev/null 2>&1; then
        latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest") \
            || fail "couldn't find the newest release"
    else
        latest=$(wget -q --max-redirect=5 -S -O /dev/null "https://github.com/$repo/releases/latest" 2>&1 \
            | sed -n 's/^ *Location: //p' | tail -n 1)
    fi
    version=${latest##*/v}
    case "$version" in
        [0-9]*.[0-9]*.[0-9]*) ;;
        *) fail "couldn't tell the newest version from $latest: set DUCK_MOLD_VERSION" ;;
    esac
fi
version=${version#v}
asset="duck-mold-$version-$asset_end"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
say "Downloading Duck Mold $version ($asset)..."
fetch "$base/v$version/$asset" "$work/$asset"
fetch "$base/v$version/SHA256SUMS" "$work/SHA256SUMS"

# The fingerprint must be the one the release lists.
wanted=$(awk -v name="$asset" '$2 == name || $2 == "*" name { print $1 }' "$work/SHA256SUMS")
[ -n "$wanted" ] || fail "SHA256SUMS doesn't list $asset"
if command -v sha256sum >/dev/null 2>&1; then
    got=$(sha256sum "$work/$asset" | awk '{ print $1 }')
elif command -v shasum >/dev/null 2>&1; then
    got=$(shasum -a 256 "$work/$asset" | awk '{ print $1 }')
else
    fail "this needs sha256sum or shasum to check the download"
fi
[ "$got" = "$wanted" ] || fail "the download's fingerprint is wrong: it may be damaged. Nothing was installed"

mkdir -p "$dir"
chmod +x "$work/$asset"
# Moved into place in one step, so a duck-mold that is running isn't
# left half written.
cp "$work/$asset" "$dir/.duck-mold.new"
mv -f "$dir/.duck-mold.new" "$dir/duck-mold"
# A Mac marks a downloaded program as such; this one has been checked.
if [ "$asset_end" = macos ]; then
    xattr -d com.apple.quarantine "$dir/duck-mold" 2>/dev/null || true
fi

say "Installed: $("$dir/duck-mold" --version) in $dir"
case ":$PATH:" in
    *":$dir:"*) say "Start with: duck-mold guide" ;;
    *)
        say "$dir isn't on your PATH yet. Add this line to ~/.profile (or ~/.zshrc on a Mac), then open a new terminal:"
        say "  export PATH=\"$dir:\$PATH\""
        say "Then start with: duck-mold guide"
        ;;
esac
