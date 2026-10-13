#!/bin/sh
# Build Hangar and install it for the current user (macOS, Linux).
set -e
cd "$(dirname "$0")"
cargo build --release

mkdir -p "$HOME/.local/bin"
cp target/release/hangar "$HOME/.local/bin/hangar"
if [ "$(uname)" = Darwin ]; then
  mkdir -p "$HOME/Library/Application Support/hangar"
else
  mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/hangar"
fi
echo "Installed to ~/.local/bin/hangar (make sure ~/.local/bin is on PATH)."
