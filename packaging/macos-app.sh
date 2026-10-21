#!/bin/sh
# Wrap the hangar binary in Hangar.app with the app icon (macOS only: needs sips and iconutil).
# Usage: packaging/macos-app.sh <binary> <version> <dest-dir>
# SIGN_IDENTITY: a Developer ID in the keychain to sign with; unset signs ad hoc.
set -e
bin="$1"; version="$2"; dest="$3"
root="$(cd "$(dirname "$0")/.." && pwd)"
app="$dest/Hangar.app/Contents"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rm -rf "$dest/Hangar.app"
mkdir -p "$app/MacOS" "$app/Resources" "$work/hangar.iconset"
cp "$bin" "$app/MacOS/hangar"

for s in 16 32 128 256; do
  sips -z "$s" "$s" "$root/assets/hangar.png" --out "$work/hangar.iconset/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  if [ "$d" -le 256 ]; then
    sips -z "$d" "$d" "$root/assets/hangar.png" --out "$work/hangar.iconset/icon_${s}x${s}@2x.png" >/dev/null
  fi
done
iconutil -c icns "$work/hangar.iconset" -o "$app/Resources/hangar.icns"

cat > "$app/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Hangar</string>
  <key>CFBundleDisplayName</key><string>Hangar</string>
  <key>CFBundleIdentifier</key><string>io.github.evandervecht.hangar</string>
  <key>CFBundleExecutable</key><string>hangar</string>
  <key>CFBundleIconFile</key><string>hangar</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Hangar opens Terminal to resume a session.</string>
</dict>
</plist>
EOF
if [ -n "$SIGN_IDENTITY" ]; then
  # Notarization needs the hardened runtime and a secure timestamp; the entitlement lets Resume drive Terminal.
  codesign --force --options runtime --timestamp --entitlements "$root/packaging/macos-entitlements.plist"     --sign "$SIGN_IDENTITY" "$dest/Hangar.app"
else
  # Ad-hoc signature: an unsigned bundle downloaded on Apple Silicon is reported as "damaged";
  # signed ad hoc it gets the normal "Open Anyway" prompt.
  codesign --force --deep --sign - "$dest/Hangar.app"
fi
echo "Built $dest/Hangar.app"
