#!/bin/sh
# Build Hangar and install it for the current user (macOS, Linux).
set -e
cd "$(dirname "$0")"
cargo build --release

mkdir -p "$HOME/.local/bin"
cp target/release/hangar "$HOME/.local/bin/hangar"
if [ "$(uname)" = Darwin ]; then
  mkdir -p "$HOME/Library/Application Support/hangar" "$HOME/Applications"
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
  sh packaging/macos-app.sh target/release/hangar "$version" "$HOME/Applications"
else
  mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/hangar"
fi
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *)
    if [ "$(uname)" = Darwin ]; then rc="$HOME/.zshrc"; else rc="$HOME/.profile"; fi
    grep -qs 'HOME/.local/bin' "$rc" || echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rc"
    echo "Added ~/.local/bin to PATH in $rc."
    ;;
esac
echo "Installed to ~/.local/bin/hangar. Open a new terminal and run 'hangar', 'hangar scan' or 'hangar list'."
