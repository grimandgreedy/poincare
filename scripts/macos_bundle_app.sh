#!/usr/bin/env bash
set -euo pipefail

if [[ "${OSTYPE:-}" != darwin* ]]; then
  echo "This script is for macOS only." >&2
  exit 1
fi

profile="${1:-release}"
target_triple="${2:-}"
case "$profile" in
  debug|release) ;;
  *)
    echo "Usage: $0 [debug|release]" >&2
    exit 1
    ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
target_dir="$repo_root/target"
if [[ -n "$target_triple" ]]; then
  target_dir="$target_dir/$target_triple"
fi
binary_path="$target_dir/$profile/poincare"
app_root="$target_dir/$profile/Poincare.app"
contents_dir="$app_root/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
plist_path="$contents_dir/Info.plist"
icon_png="$repo_root/crates/poincare-app/assets/icon_macos.png"
icon_src="$target_dir/$profile/icon.icns"
version="$(grep '^version = ' "$repo_root/crates/poincare-app/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')"

if [[ ! -x "$binary_path" ]]; then
  echo "Missing binary: $binary_path" >&2
  if [[ "$profile" == "release" ]]; then
    echo "Build it first with: cargo build -p poincare-app --release${target_triple:+ --target $target_triple}" >&2
  else
    echo "Build it first with: cargo build -p poincare-app${target_triple:+ --target $target_triple}" >&2
  fi
  exit 1
fi

if [[ ! -f "$icon_png" ]]; then
  echo "Missing icon: $icon_png" >&2
  exit 1
fi

rm -rf "$app_root"
mkdir -p "$macos_dir" "$resources_dir"
cp "$binary_path" "$macos_dir/Poincare"

iconset="$(mktemp -d)/Poincare.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$icon_png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$icon_png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$icon_src"
cp "$icon_src" "$resources_dir/icon.icns"

cat >"$plist_path" <<PLIST
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
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

touch "$app_root"
echo "Created $app_root"
