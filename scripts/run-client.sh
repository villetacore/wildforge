#!/usr/bin/env sh
# Build the Rust GDExtension and launch the Godot client.
# Set GODOT to your Godot 4 binary if it is not on PATH.
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GODOT="${GODOT:-$(command -v godot || command -v godot4 || echo /Applications/Godot.app/Contents/MacOS/Godot)}"
(cd "$ROOT/client/rust" && cargo build)
if [ ! -f "$ROOT/client/godot/.godot/extension_list.cfg" ]; then
  "$GODOT" --headless --path "$ROOT/client/godot" --import >/dev/null 2>&1 || true
fi
exec "$GODOT" --path "$ROOT/client/godot" "$@"
