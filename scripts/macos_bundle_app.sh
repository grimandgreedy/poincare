#!/usr/bin/env bash
set -euo pipefail

if [[ "${OSTYPE:-}" != darwin* ]]; then
  echo "This script is for macOS only." >&2
  exit 1
fi

profile="${1:-debug}"
case "$profile" in
  debug|release) ;;
  *)
    echo "Usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
binary_path="$repo_root/target/$profile/poincare"
app_root="$repo_root/target/$profile/Poincare.app"
contents_dir="$app_root/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
plist_path="$contents_dir/Info.plist"
icon_src="$repo_root/assets/icon.icns"

if [[ ! -x "$binary_path" ]]; then
  echo "Missing binary: $binary_path" >&2
  echo "Build it first with: cargo build${profile/release/ --release}" >&2
  exit 1
fi

if [[ ! -f "$icon_src" ]]; then
  echo "Missing icon: $icon_src" >&2
  exit 1
fi

rm -rf "$app_root"
mkdir -p "$macos_dir" "$resources_dir"
cp "$binary_path" "$macos_dir/Poincare"
cp "$icon_src" "$resources_dir/icon.icns"

cat >"$plist_path" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>Poincare</string>
  <key>CFBundleExecutable</key>
  <string>Poincare</string>
  <key>CFBundleIconFile</key>
  <string>icon</string>
  <key>CFBundleIdentifier</key>
  <string>com.grimandgreedy.poincare</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Poincare</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.6.0</string>
  <key>CFBundleVersion</key>
  <string>0.6.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

touch "$app_root"
echo "Created $app_root"
