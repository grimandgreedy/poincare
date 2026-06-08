#!/usr/bin/env bash
set -euo pipefail

profile="${1:-release}"
target_triple="${2:-}"
case "$profile" in
  debug|release) ;;
  *)
    echo "Usage: $0 [debug|release] [target-triple]" >&2
    exit 1
    ;;
esac

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
target_dir="$repo_root/target"
if [[ -n "$target_triple" ]]; then
  target_dir="$target_dir/$target_triple"
fi
binary_path="$target_dir/$profile/poincare"
version="$(grep '^version = ' "$repo_root/crates/poincare-app/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')"
bundle_root="$target_dir/$profile/poincare-$version-linux-${target_triple:-native}"

if [[ ! -x "$binary_path" ]]; then
  echo "Missing binary: $binary_path" >&2
  exit 1
fi

rm -rf "$bundle_root"
mkdir -p "$bundle_root/bin" "$bundle_root/share/applications" "$bundle_root/share/icons/hicolor/512x512/apps"
cp "$binary_path" "$bundle_root/bin/poincare"
cp "$repo_root/crates/poincare-app/assets/icon.png" "$bundle_root/share/icons/hicolor/512x512/apps/poincare.png"

cat >"$bundle_root/share/applications/poincare.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Poincare
Comment=Interactive 3D mathematical graphing application
Exec=poincare
Icon=poincare
Terminal=false
Categories=Education;Science;Math;
DESKTOP

cat >"$bundle_root/README.txt" <<README
Poincare $version

Run with:
  ./bin/poincare

On Debian/Ubuntu, install the system graphics/UI libraries required by wgpu,
fontconfig, and the native file dialog stack if your desktop does not already
provide them.
README

tar -C "$(dirname "$bundle_root")" -czf "$bundle_root.tar.gz" "$(basename "$bundle_root")"
echo "Created $bundle_root.tar.gz"
